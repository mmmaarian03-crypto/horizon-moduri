// Escape Rope inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs): the
// player tick that remembers the last place under the sky (`LastSkyCoordProcedure`), the rope's use
// (`EscapeRopeUseProcedure`) and the tooltip line. The item itself, its model and recipe are content
// (content/mods/escape_rope).
//
// Where it runs: everything the mod does is server side (the procedures run on the logical server). Since
// Horizon #1514 single player is an integrated server and the window is one of its guests, so:
// - the tick of every guest (single player's window included) runs in `tick_guests` on the server, and the
//   in-frame host's in `tick_blocks`;
// - the use runs inside the guest's turn (`as_guest`: `self.player`, the inventory and the player's data are
//   the guest's); the teleport reaches the guest's client through `leave_guest`, the action bar line through
//   `guest_packets` (kit_notice).
// A server's client (`remote`) does nothing but send the click.
#[allow(unused_imports)]
use super::{Game, Result, lan::LanHost};
use horizon_world::nbt::{JavaString, Tag};

#[allow(dead_code)]
mod engine {
    include!("escape_rope_engine.rs");
}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
#[allow(dead_code)]
const MOD_ID: &str = "escape_rope";

include!("kit_blocks.rs");
include!("kit_notice.rs");
include!("kit_sound.rs");
include!("kit_state.rs");

/// The key of the place in the player's data (`PlayerVariables` of the original, a capability saved with the
/// player): `{LastSkyX, LastSkyY, LastSkyZ}` as doubles. Kept in the player's own compound (the host's
/// `level.player_extra`, a guest's `saved`), so each player has his own place and it follows him through
/// dimensions and deaths.
const KEY: &str = "EscapeRopeVariables";
/// The place an older version kept for the in-frame host: `<world>/data/escape_rope/player.txt`. Read once,
/// when the host's data has no place yet.
const OLD_FILE: &str = "player.txt";
thread_local! {
    /// The world whose old file was already looked at.
    static OLD_TRIED: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}

fn double(t: Option<&Tag>) -> Option<f64> {
    match t? {
        Tag::Double(bits) => Some(f64::from_bits(*bits)),
        Tag::Float(bits) => Some(f64::from(f32::from_bits(*bits))),
        Tag::Int(v) => Some(f64::from(*v)),
        _ => None,
    }
}

/// The stored place in a player's compound (`None`: the player has none yet).
fn read_last(extra: &Option<Tag>) -> Option<engine::Last> {
    let Some(Tag::Compound(map)) = extra else { return None };
    let Some(v) = map.get(&JavaString::from(KEY)) else { return None };
    let (x, y, z) = (double(v.get("LastSkyX"))?, double(v.get("LastSkyY"))?, double(v.get("LastSkyZ"))?);
    (x.is_finite() && y.is_finite() && z.is_finite()).then_some(engine::Last { x, y, z, set: true })
}

/// Writes the place into a player's compound (the other keys stay).
fn write_last(extra: &mut Option<Tag>, last: &engine::Last) {
    let e = extra.get_or_insert_with(|| Tag::Compound(Default::default()));
    let Tag::Compound(map) = e else { return };
    map.insert(
        JavaString::from(KEY),
        Tag::compound([
            ("LastSkyX", Tag::Double(last.x.to_bits())),
            ("LastSkyY", Tag::Double(last.y.to_bits())),
            ("LastSkyZ", Tag::Double(last.z.to_bits())),
        ]),
    );
}

/// The world as the sky tests read it.
struct Live<'a, 'g>(&'a Game<'g>);
impl engine::Sky for Live<'_, '_> {
    fn surface(&self, x: i32, z: i32) -> i32 {
        // getHeight(WORLD_SURFACE, x, z)
        self.0.world_surface(x, z)
    }
    fn blocks_light_not_liquid(&self, p: engine::Pos) -> bool {
        // state.getLightBlock(level, pos) > 0 && !state.liquid(): water, lava and bubble columns are liquid.
        let facts = self.0.res.facts().get(self.0.world.block(p[0], p[1], p[2]));
        facts.dampening > 0
            && !matches!(
                &*facts.name,
                "minecraft:water" | "minecraft:lava" | "minecraft:bubble_column"
            )
    }
}

impl Game<'_> {
    /// The place of the player whose data is in `level.player_extra` (the in-frame host, or the guest whose turn
    /// it is). The in-frame host's place of an older version (player.txt) is taken over once.
    fn er_last(&mut self) -> Option<engine::Last> {
        if let Some(last) = read_last(&self.level.player_extra) {
            return Some(last);
        }
        // Once per world: the file is not read again every tick of a player who has no place yet.
        let root = self.world_root.clone();
        let first = |root: std::path::PathBuf| OLD_TRIED.with(|t| t.borrow_mut().replace(root.clone()).as_ref() != Some(&root));
        if self.lan_acting.is_none() && kit_state::dir(self).is_some() && first(root) {
            let old = engine::Last::load(kit_state::read(self, OLD_FILE));
            if old.set {
                write_last(&mut self.level.player_extra, &old);
                return Some(old);
            }
        }
        None
    }

    /// HEAD on `tick_blocks` (once per game tick): `LastSkyCoordProcedure` for the in-frame host (the player
    /// tick, END). A server has no player of its own (`self.player` is `None` on the integrated server).
    pub(super) fn escape_rope_tick(&mut self) {
        if self.remote.is_some() || self.lan_acting.is_some() {
            return;
        }
        let Some(p) = self.player.as_ref().filter(|p| !p.health.dead_or_dying()) else {
            return;
        };
        let feet = [p.position.x, p.position.y, p.position.z];
        let mut last = self.er_last().unwrap_or_default();
        if engine::track(&Live(self), feet, &mut last) {
            write_last(&mut self.level.player_extra, &last);
        }
    }

    /// HEAD on `tick_guests` (the guests' tick on the server, single player's window included):
    /// `LastSkyCoordProcedure` for each living guest in this level. The guest's data is its `saved`
    /// compound outside its turn, so no turn is needed.
    pub(super) fn escape_rope_guests_tick(&mut self, host: &mut LanHost) {
        if self.remote.is_some() {
            return;
        }
        for i in 0..host.hub.guests.len() {
            let g = &host.hub.guests[i];
            if !(host.hub.here(g) && g.loaded && g.placed && !g.dead) {
                continue;
            }
            let feet = g.position;
            let mut last = read_last(&g.saved).unwrap_or_default();
            if engine::track(&Live(self), feet, &mut last) {
                write_last(&mut host.hub.guests[i].saved, &last);
            }
        }
    }

    /// HEAD on `throw_item` (an item used, in the air or against a block, after the block's own use):
    /// `EscapeRopeItem.use` -> `EscapeRopeUseProcedure`. The original returns PASS, so the click is
    /// not consumed (the helper answers `false` to the game, the other hand may still act).
    /// Runs on the in-frame host's game and on a server inside the guest's turn; a server's client only
    /// sends the click to its server.
    pub(super) fn escape_rope_use(&mut self, stack: &horizon_client::items::ItemStack) {
        if &*stack.item != engine::ITEM || self.remote.is_some() {
            return;
        }
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let feet = [p.position.x, p.position.y, p.position.z];
        let last = self.er_last().unwrap_or_default();
        match engine::use_rope(&Live(self), feet, &last) {
            engine::Rope::Blocked | engine::Rope::NowhereToGo => {
                kit_notice::action_bar(self, engine::BLOCKED.to_owned());
            }
            engine::Rope::Teleport { to } => {
                // entity.teleportTo(x, y, z) and the connection's teleport(x, y, z, yRot, xRot): the
                // same dimension, the same facing. For a guest, `leave_guest` sees the body moved and
                // teleports its client (ServerPlayer.teleport).
                let v = horizon_core::geometry::Vec3::new(to[0], to[1], to[2]);
                if let Some(p) = self.player.as_mut() {
                    p.position = v;
                    p.velocity = horizon_core::geometry::Vec3::new(0.0, 0.0, 0.0);
                    p.ground.on_ground = false;
                }
                // The window's interpolation start is the in-frame host's own.
                if self.lan_acting.is_none() {
                    self.previous_position = v;
                }
                // itemstack.shrink(1) (the creative player's count is restored by the game).
                self.consume_held();
                kit_notice::action_bar(self, engine::USED.to_owned());
                // level.playSound(null, BlockPos.containing(x, y, z), escape_rope:item_use, PLAYERS, 1, 1)
                // where the player stood when he used it.
                let at = engine::containing(feet);
                kit_sound::at_block(self, "escape_rope:item_use", at, 1.0, 1.0);
            }
        }
    }

    /// HEAD on `load_world_state` (no bindings): a world opens. Nothing is kept between worlds (the places
    /// live in the players' data).
    pub(super) fn escape_rope_world_opened() {
        OLD_TRIED.with(|t| *t.borrow_mut() = None);
    }
}

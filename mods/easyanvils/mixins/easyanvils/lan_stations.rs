// Easy Anvils inside the guests' workstations (`game/lan_stations.rs`): a server's guest (single player's
// window included) at an anvil. A child of `lan_stations`, so it reaches the guest's open station menu
// (`GuestStation` keeps it private). The anvils' contents are `game::easyanvils`'s.
use super::{Game, LanHost, StationKind};
use horizon_client::items::ItemStack;

/// The anvil menu's two input slots (`menu_of`: 0..2).
const INPUTS: std::ops::Range<usize> = 0..2;

impl Game<'_> {
    /// HEAD on `lan_send_station` (the guest's station opened, or changed by a click): a new anvil menu takes
    /// the anvil's items (`ModAnvilMenu` over the block entity), a held one is copied into the anvil.
    pub(in crate::game) fn easyanvils_guest_content(&mut self, host: &mut LanHost, conn: u32) {
        let Some(gs) = host.play.get_mut(&conn).and_then(|p| p.station.as_mut()) else {
            return;
        };
        if gs.kind != StationKind::Anvil {
            return;
        }
        let (window, pos) = (gs.window, gs.pos);
        let inputs: Vec<Option<ItemStack>> = INPUTS.map(|s| gs.menu.slot(s).cloned()).collect();
        if self.easyanvils_guest_bound(conn) == Some((pos, window)) {
            self.easyanvils_mirror_guest(conn, &inputs);
            return;
        }
        let empty = inputs.iter().all(Option::is_none);
        if let Some(items) = self.easyanvils_bind_guest(conn, pos, window, empty) {
            let env = self.menu_env();
            for (i, item) in items.into_iter().enumerate().take(INPUTS.end) {
                gs.menu.set_slot(&env, i, item);
            }
        }
    }

    /// HEAD on `lan_crafting_closed` (a guest's ContainerClose tries it before `lan_station_closed`, which
    /// cannot be hooked; window `None` = whichever menu, as `lan_open_crafting` closes them) and
    /// `lan_crafting_left`: a held anvil keeps its items; the guest's menu is emptied first, so nothing goes
    /// back to him.
    pub(in crate::game) fn easyanvils_guest_closed(&mut self, host: &mut LanHost, conn: u32, window: Option<i32>) {
        let Some(bound) = self.easyanvils_guest_bound(conn) else {
            return;
        };
        let Some(gs) = host.play.get_mut(&conn).and_then(|p| p.station.as_mut()) else {
            self.easyanvils_unbind_guest(conn);
            return;
        };
        if window.is_some_and(|w| w != gs.window) {
            return;
        }
        if gs.kind != StationKind::Anvil || bound != (gs.pos, gs.window) {
            self.easyanvils_unbind_guest(conn);
            return;
        }
        let env = self.menu_env();
        let inputs: Vec<Option<ItemStack>> = INPUTS.map(|s| gs.menu.slot(s).cloned()).collect();
        for s in INPUTS {
            gs.menu.set_slot(&env, s, None);
        }
        self.easyanvils_stash_guest(conn, &inputs);
    }

    /// HEAD on `tick_guests` (a server's level in place): every held anvil of this level is copied into the
    /// anvil. One whose block is gone, or out of the guest's reach, keeps the items (spilled by the next block
    /// entity tick when gone) and the guest's menu is emptied before `lan_stations_tick` closes it
    /// (`lan_station_closed` itself cannot be hooked: it holds a macro). A held anvil whose menu went by
    /// another way (opening a container, a trade... over it) gave its items back to the guest: the anvil's
    /// copy goes, so nothing is doubled. Then the clients get the anvils that changed.
    pub(in crate::game) fn easyanvils_guests_tick(&mut self, host: &mut LanHost) {
        if self.remote.is_some() {
            return;
        }
        for (conn, pos, window) in self.easyanvils_guests_here() {
            let reach = host.hub.guest(conn).is_none_or(|g| g.within_block_range(pos, 4.0));
            let open = host
                .play
                .get_mut(&conn)
                .and_then(|p| p.station.as_mut())
                .filter(|gs| gs.kind == StationKind::Anvil && gs.window == window && gs.pos == pos);
            let Some(gs) = open else {
                log::info!("easyanvils: guest {conn}'s anvil at {pos:?} closed with its items given back");
                self.easyanvils_forget_guest(conn);
                continue;
            };
            let inputs: Vec<Option<ItemStack>> = INPUTS.map(|s| gs.menu.slot(s).cloned()).collect();
            let gone = self.world.loaded(pos[0], pos[2]) && !self.easyanvils_is_anvil_at(pos);
            if gone || !reach {
                let env = self.menu_env();
                for s in INPUTS {
                    gs.menu.set_slot(&env, s, None);
                }
                self.easyanvils_stash_guest(conn, &inputs);
            } else {
                self.easyanvils_mirror_guest(conn, &inputs);
            }
        }
        self.easyanvils_send_anvils(host);
    }
}

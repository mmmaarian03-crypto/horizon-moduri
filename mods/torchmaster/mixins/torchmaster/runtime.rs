// Torchmaster inside Horizon's game (nightly 142388e1c136). The rules are the
// engine below; this part keeps each level's registry (the original's level
// capability) in the world folder, answers the spawn hooks, ticks the Feral
// Flare Lanterns, uses the Frozen Pearl and draws the range display.
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    // Torchmaster for Horizon: the rules, without any game code.
// Written from Torchmaster 20.1.9 for Forge 1.20.1 (xalcon, MIT,
// github.com/xalcon/TorchMaster branch mc/1.20.1, the jar the pack plays) and
// the pack's config/torchmaster.toml. The Java classes each rule follows are
// named next to it.
use std::collections::{BTreeMap, BTreeSet};

pub type Pos = [i32; 3];

/// A block that blocks spawns in a cube around it (`IEntityBlockingLight`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Light {
    /// `MegatorchEntityBlockingLight`: hostile mobs.
    MegaTorch,
    /// `DreadLampEntityBlockingLight`: passive and neutral mobs.
    DreadLamp,
}

/// What stands at a position, as far as Torchmaster cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    MegaTorch,
    DreadLamp,
    /// `torchmaster:feral_flare_lantern`.
    Lantern,
    /// `torchmaster:invisible_light`, the lantern's lights.
    Light,
    Other,
}
impl Kind {
    pub fn light(self) -> Option<Light> {
        match self {
            Kind::MegaTorch => Some(Light::MegaTorch),
            Kind::DreadLamp => Some(Light::DreadLamp),
            _ => None,
        }
    }
}

/// `MobCategory` (1.20.1 values, checked in the client jar's `<clinit>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Monster,
    Creature,
    Ambient,
    Axolotls,
    UndergroundWaterCreature,
    WaterCreature,
    WaterAmbient,
    Misc,
}
impl Category {
    pub fn from_name(name: &str) -> Self {
        match name {
            "monster" => Self::Monster,
            "creature" => Self::Creature,
            "ambient" => Self::Ambient,
            "axolotls" => Self::Axolotls,
            "underground_water_creature" => Self::UndergroundWaterCreature,
            "water_creature" => Self::WaterCreature,
            "water_ambient" => Self::WaterAmbient,
            _ => Self::Misc,
        }
    }
    /// `MobCategory.isFriendly`: false for MONSTER only (MISC is friendly).
    pub fn is_friendly(self) -> bool {
        self != Self::Monster
    }
}

/// `MobSpawnType`: why a mob is being spawned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Natural,
    ChunkGeneration,
    Spawner,
    Structure,
    Breeding,
    MobSummoned,
    Jockey,
    Event,
    Conversion,
    Reinforcement,
    Triggered,
    Bucket,
    SpawnEgg,
    Command,
    Dispenser,
    Patrol,
}
/// `EntityBlockingEventHandler.isIntentionalSpawn`: player-made spawns are never blocked.
pub fn is_intentional(r: Reason) -> bool {
    use Reason::*;
    matches!(
        r,
        Breeding | Dispenser | Bucket | Conversion | SpawnEgg | Triggered | Command | Event
    )
}
/// `EntityBlockingEventHandler.isNaturalSpawn`: low light spawns, chunk
/// generation and patrols.
pub fn is_natural(r: Reason) -> bool {
    matches!(r, Reason::Natural | Reason::ChunkGeneration | Reason::Patrol)
}

/// the pack's `config/torchmaster.toml` (all of it at the mod's defaults,
/// `TorchmasterConfig.General`).
#[derive(Clone, Debug)]
pub struct Settings {
    pub beginner_tooltips: bool,
    pub block_only_natural_spawns: bool,
    pub block_village_sieges: bool,
    pub mega_torch_radius: i32,
    pub dread_lamp_radius: i32,
    pub mega_torch_overrides: Vec<String>,
    pub dread_lamp_overrides: Vec<String>,
    pub feral_flare_radius: i32,
    pub feral_flare_tick_rate: i32,
    pub feral_flare_min_light_level: i32,
    pub feral_flare_light_hardcap: i32,
    /// Read by nothing in 20.1.9: `ModItems` gives the pearl no durability.
    pub frozen_pearl_durability: i32,
    pub aggressive_spawn_checks: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            beginner_tooltips: true,
            block_only_natural_spawns: true,
            block_village_sieges: true,
            mega_torch_radius: 64,
            dread_lamp_radius: 64,
            mega_torch_overrides: Vec::new(),
            dread_lamp_overrides: Vec::new(),
            feral_flare_radius: 16,
            feral_flare_tick_rate: 5,
            feral_flare_min_light_level: 10,
            feral_flare_light_hardcap: 255,
            frozen_pearl_durability: 1024,
            aggressive_spawn_checks: false,
        }
    }
}
impl Settings {
    pub fn radius(&self, light: Light) -> i32 {
        match light {
            Light::MegaTorch => self.mega_torch_radius,
            Light::DreadLamp => self.dread_lamp_radius,
        }
    }
}

/// `EntityFilterRegistry.applyListOverrides` applied to `VanillaCompat`'s
/// lists: the last applicable `+` or `-` of an entity wins.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overrides(BTreeMap<String, bool>);
impl Overrides {
    /// `exists`: `ForgeRegistries.ENTITY_TYPES.containsKey`, which a `+`
    /// needs (unknown entities are skipped with a warning).
    pub fn parse(list: &[String], exists: &dyn Fn(&str) -> bool) -> Self {
        let mut map = BTreeMap::new();
        for entry in list {
            // The minimum is a prefix and a resource location, `+a:b`.
            if entry.len() < 4 {
                continue;
            }
            let (prefix, rest) = entry.split_at(1);
            let id = if rest.contains(':') {
                rest.to_owned()
            } else {
                format!("minecraft:{rest}")
            };
            match prefix {
                "+" if exists(&id) => {
                    map.insert(id, true);
                }
                "-" => {
                    map.insert(id, false);
                }
                _ => {}
            }
        }
        Self(map)
    }
}

/// Which entities each light blocks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filters {
    pub mega_torch: Overrides,
    pub dread_lamp: Overrides,
}
impl Filters {
    pub fn new(s: &Settings, exists: &dyn Fn(&str) -> bool) -> Self {
        Self {
            mega_torch: Overrides::parse(&s.mega_torch_overrides, exists),
            dread_lamp: Overrides::parse(&s.dread_lamp_overrides, exists),
        }
    }
    /// `VanillaCompat.registerTorchEntities` (every type that is not
    /// friendly) and `registerDreadLampEntities` (friendly, not MISC).
    pub fn blocks(&self, light: Light, entity: &str, category: Category) -> bool {
        let (list, listed) = match light {
            Light::MegaTorch => (&self.mega_torch, !category.is_friendly()),
            Light::DreadLamp => (
                &self.dread_lamp,
                category != Category::Misc && category.is_friendly(),
            ),
        };
        list.0.get(entity).copied().unwrap_or(listed)
    }
}

/// `new BlockPos((int) x, (int) y, (int) z)`: Java's cast truncates toward
/// zero (so -0.5 is block 0, not -1), saturates and makes NaN 0, as `as` does.
pub fn block_pos(at: [f64; 3]) -> Pos {
    [at[0] as i32, at[1] as i32, at[2] as i32]
}

/// `DistanceLogics.Cubic`: `radius` blocks below the torch and `radius + 1`
/// above it on each axis (the original compares block positions with the
/// torch's corner at `pos - radius` and `pos + radius + 1`, both inclusive).
pub fn in_cube(p: Pos, torch: Pos, radius: i32) -> bool {
    (0..3).all(|i| {
        let (p, t, r) = (i64::from(p[i]), i64::from(torch[i]), i64::from(radius));
        t - r <= p && p <= t + r + 1
    })
}

/// A Feral Flare Lantern's block entity (`FeralFlareLanternTileEntity`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lantern {
    pub ticks: i32,
    /// `childLights`, in placement order (a spot can repeat).
    pub lights: Vec<Pos>,
}

/// One level's lights (`LightsRegistryCapability.RegistryContainer`), its
/// lanterns, and the lights of removed lanterns still to clear.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Registry {
    pub lights: BTreeMap<Pos, Light>,
    pub lanterns: BTreeMap<Pos, Lantern>,
    /// Lights of removed lanterns whose chunks were not loaded: vanilla's
    /// `removeChildLights` loads them; here they go when they load.
    pub orphans: BTreeSet<Pos>,
    /// `RegistryContainer.tickCounter` (not saved by the original either).
    pub cleanup_counter: u32,
}

/// Reads what stands where (`None`: the chunk is not loaded).
pub trait Blocks {
    fn kind(&self, p: Pos) -> Option<Kind>;
}

/// Changes the lantern rules ask of the world.
pub trait LightEditor: Blocks {
    /// `level.removeBlock(pos, false)`.
    fn remove_light(&mut self, p: Pos);
}

/// The world a lantern ticks in.
pub trait LanternWorld {
    /// `Level.isLoaded`: inside the build height, chunk present.
    fn loaded(&self, p: Pos) -> bool;
    /// `Level.getHeight(WORLD_SURFACE, x, z)`: one above the highest block
    /// that is not air; the invisible lights are air (an `AirBlock`).
    fn surface(&self, x: i32, z: i32) -> i32;
    /// `Level.getHeight()`: the dimension's build height (384 in the overworld).
    fn height(&self) -> i32;
    /// `Level.isEmptyBlock`: air, including the invisible lights.
    fn empty(&self, p: Pos) -> bool;
    /// `getBrightness(LightLayer.BLOCK, pos)`.
    fn block_light(&self, p: Pos) -> i32;
    /// `setBlock(pos, invisible_light, 3)`: whether the block changed.
    fn place_light(&mut self, p: Pos) -> bool;
    /// `level.random.nextInt(bound)`.
    fn next_int(&mut self, bound: i32) -> i32;
}

impl Registry {
    /// `EntityBlockingLightBlock.onPlace`: `registerLight`.
    pub fn register(&mut self, p: Pos, light: Light) {
        self.lights.insert(p, light);
    }
    /// `EntityBlockingLightBlock.onRemove`: `unregisterLight` of its own key.
    pub fn unregister(&mut self, p: Pos, light: Light) {
        if self.lights.get(&p) == Some(&light) {
            self.lights.remove(&p);
        }
    }

    /// `onFinalizeSpawn` (priority HIGHEST): whether the spawn of `entity`
    /// at the mob's position `at` is cancelled.
    pub fn should_block_spawn(
        &self,
        s: &Settings,
        f: &Filters,
        entity: &str,
        category: Category,
        reason: Reason,
        at: [f64; 3],
    ) -> bool {
        if is_intentional(reason) {
            return false;
        }
        if s.block_only_natural_spawns && !is_natural(reason) {
            return false;
        }
        let p = block_pos(at);
        // RegistryContainer.shouldBlockEntity: any light.
        self.lights.iter().any(|(torch, light)| {
            f.blocks(*light, entity, category) && in_cube(p, *torch, s.radius(*light))
        })
    }

    /// `onVillageSiegeEvent`: a mega torch's cube holds the siege's spot.
    pub fn should_block_siege(&self, s: &Settings, at: [f64; 3]) -> bool {
        if !s.block_village_sieges {
            return false;
        }
        let p = block_pos(at);
        self.lights.iter().any(|(torch, light)| {
            *light == Light::MegaTorch && in_cube(p, *torch, s.mega_torch_radius)
        })
    }

    /// `RegistryContainer.onGlobalTick`: every 201st tick, the lights whose
    /// chunk is loaded and whose block is gone leave (`cleanupCheck`).
    /// Returns whether it ran.
    pub fn global_tick(&mut self, w: &impl Blocks) -> bool {
        let counter = self.cleanup_counter;
        self.cleanup_counter += 1;
        if counter < 200 {
            return false;
        }
        self.cleanup_counter = 0;
        self.lights
            .retain(|p, light| w.kind(*p).is_none_or(|k| k.light() == Some(*light)));
        true
    }

    /// A block write at `p`, in order (`LevelChunk.setBlockState` calls the
    /// old state's `onRemove`, then the new one's `onPlace`). Returns the
    /// child lights of a lantern that went away, to remove.
    pub fn block_changed(&mut self, p: Pos, old: Kind, new: Kind) -> Vec<Pos> {
        let mut gone = Vec::new();
        if old == new {
            return gone;
        }
        if let Some(light) = old.light() {
            self.unregister(p, light);
        }
        if old == Kind::Lantern
            && let Some(l) = self.lanterns.remove(&p)
        {
            gone = l.lights;
        }
        if let Some(light) = new.light() {
            self.register(p, light);
        }
        if new == Kind::Lantern {
            // A new block entity.
            self.lanterns.insert(p, Lantern::default());
        }
        gone
    }

    /// A Torchmaster block found in a chunk that arrived: in vanilla it was
    /// registered when placed, so the registry learns of it now if it lost it
    /// (a world from before the mod, or one carried over from Forge).
    pub fn found(&mut self, p: Pos, kind: Kind) -> bool {
        match kind {
            Kind::MegaTorch | Kind::DreadLamp => {
                let light = kind.light().unwrap_or(Light::MegaTorch);
                self.lights.insert(p, light) != Some(light)
            }
            Kind::Lantern if !self.lanterns.contains_key(&p) => {
                self.lanterns.insert(p, Lantern::default());
                true
            }
            _ => false,
        }
    }

    /// `FeralFlareLanternTileEntity.removeChildLights`: each child that is
    /// still an invisible light is removed; those not loaded wait.
    pub fn remove_lights(&mut self, lights: Vec<Pos>, w: &mut impl LightEditor) {
        for p in lights {
            match w.kind(p) {
                None => {
                    self.orphans.insert(p);
                }
                Some(Kind::Light) => w.remove_light(p),
                Some(_) => {}
            }
        }
    }

    /// The waiting lights whose chunks have loaded.
    pub fn clear_orphans(&mut self, w: &mut impl LightEditor) -> bool {
        let loaded: Vec<Pos> = self
            .orphans
            .iter()
            .copied()
            .filter(|p| w.kind(*p).is_some())
            .collect();
        for p in &loaded {
            self.orphans.remove(p);
            if w.kind(*p) == Some(Kind::Light) {
                w.remove_light(*p);
            }
        }
        !loaded.is_empty()
    }
}

/// `FeralFlareLanternTileEntity.tick` for the lantern at `at` (line of
/// sight is off: the original has no way to turn it on). Returns where it
/// placed a light.
pub fn tick_lantern(
    at: Pos,
    l: &mut Lantern,
    s: &Settings,
    w: &mut impl LanternWorld,
) -> Option<Pos> {
    l.ticks = l.ticks.wrapping_add(1);
    if s.feral_flare_tick_rate <= 0 || l.ticks % s.feral_flare_tick_rate != 0 {
        return None;
    }
    // `childLights.size() > hardcap`: one more than the cap gets placed.
    if l.lights.len() as i64 > i64::from(s.feral_flare_light_hardcap) {
        return None;
    }
    l.ticks = 0;
    let radius = s.feral_flare_radius;
    let diameter = radius * 2;
    let x = (radius - w.next_int(diameter)) + at[0];
    let y = (radius - w.next_int(diameter)) + at[1];
    let z = (radius - w.next_int(diameter)) + at[2];
    let mut target = [x, y, z];
    if !w.loaded(target) {
        return None;
    }
    let surface = w.surface(x, z);
    if target[1] > surface + 4 {
        target[1] = surface + 4;
    }
    let height = w.height();
    if target[1] > height {
        target[1] = height - 1;
    }
    if w.empty(target) && w.block_light(target) < s.feral_flare_min_light_level && w.place_light(target)
    {
        l.lights.push(target);
        return Some(target);
    }
    None
}

/// `FrozenPearlItem.use`: from -15 to 15 around the player's block on each
/// axis, x outermost, then y, then z.
pub const FROZEN_PEARL_REACH: i32 = 15;
pub fn frozen_pearl_area(center: Pos) -> impl Iterator<Item = Pos> {
    let r = FROZEN_PEARL_REACH;
    (-r..=r).flat_map(move |x| {
        (-r..=r).flat_map(move |y| (-r..=r).map(move |z| [center[0] + x, center[1] + y, center[2] + z]))
    })
}

/// The tooltip line `TMItemBlock` and `FrozenPearlItem.appendHoverText`
/// add with `beginnerTooltips`: the item's description id plus `.tooltip`.
pub fn tooltip_key(item: &str) -> Option<&'static str> {
    Some(match item {
        "torchmaster:megatorch" => "block.torchmaster.megatorch.tooltip",
        "torchmaster:dreadlamp" => "block.torchmaster.dreadlamp.tooltip",
        "torchmaster:feral_flare_lantern" => "block.torchmaster.feral_flare_lantern.tooltip",
        "torchmaster:frozen_pearl" => "item.torchmaster.frozen_pearl.tooltip",
        _ => return None,
    })
}

/// The registry as a text file in the world folder (the original keeps it
/// in a level capability and the lanterns' lights in their block entities).
pub fn save(r: &Registry) -> String {
    let mut out = String::from("torchmaster 1\n");
    let pos = |p: &Pos| format!("{} {} {}", p[0], p[1], p[2]);
    for (p, light) in &r.lights {
        let name = match light {
            Light::MegaTorch => "megatorch",
            Light::DreadLamp => "dreadlamp",
        };
        out.push_str(&format!("light {name} {}\n", pos(p)));
    }
    for (p, l) in &r.lanterns {
        out.push_str(&format!("lantern {} {}\n", pos(p), l.ticks));
        for c in &l.lights {
            out.push_str(&format!("child {}\n", pos(c)));
        }
    }
    for p in &r.orphans {
        out.push_str(&format!("orphan {}\n", pos(p)));
    }
    out
}

/// Reads [`save`]'s text; lines it does not know are skipped.
pub fn load(text: &str) -> Registry {
    let mut r = Registry::default();
    let mut lantern: Option<Pos> = None;
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let int = |i: usize| words.get(i).and_then(|w| w.parse::<i32>().ok());
        let pos = |i: usize| Some([int(i)?, int(i + 1)?, int(i + 2)?]);
        match words.first().copied() {
            Some("light") => {
                let light = match words.get(1).copied() {
                    Some("megatorch") => Light::MegaTorch,
                    Some("dreadlamp") => Light::DreadLamp,
                    _ => continue,
                };
                if let Some(p) = pos(2) {
                    r.lights.insert(p, light);
                }
            }
            Some("lantern") => {
                lantern = pos(1);
                if let Some(p) = lantern {
                    r.lanterns.insert(
                        p,
                        Lantern {
                            ticks: int(4).unwrap_or(0),
                            lights: Vec::new(),
                        },
                    );
                }
            }
            Some("child") => {
                if let (Some(owner), Some(p)) = (lantern, pos(1))
                    && let Some(l) = r.lanterns.get_mut(&owner)
                {
                    l.lights.push(p);
                }
            }
            Some("orphan") => {
                if let Some(p) = pos(1) {
                    r.orphans.insert(p);
                }
            }
            _ => {}
        }
    }
    r
}

}
use engine::{Category, Filters, Kind, Light, Pos, Reason, Registry, Settings};
use horizon_core::random::RandomSource;
use horizon_world::registry::BlockStateId;
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
};

/// The blocks of the mod (`ModBlocks`), as the content pack names them.
const BLOCKS: [(&str, Kind); 4] = [
    ("torchmaster:megatorch", Kind::MegaTorch),
    ("torchmaster:dreadlamp", Kind::DreadLamp),
    ("torchmaster:feral_flare_lantern", Kind::Lantern),
    ("torchmaster:invisible_light", Kind::Light),
];
const FROZEN_PEARL: &str = "torchmaster:frozen_pearl";

/// The game's states of the mod's blocks.
struct Ids {
    len: usize,
    kinds: Vec<Kind>,
    /// Every state of the four blocks (watched for writes).
    mine: Vec<u32>,
    /// The invisible light (an `AirBlock` in the original: no states).
    light: Option<BlockStateId>,
    /// The mega torch standing up: its template rod points where it was
    /// placed against, the original torch always stands.
    mega_up: Option<BlockStateId>,
}
impl Ids {
    fn build(game: &Game<'_>) -> Self {
        let registry = game.pack.assets.registry;
        let len = registry.len();
        let mut kinds = vec![Kind::Other; len];
        let mut mine = Vec::new();
        let mut mega_up = None;
        for (i, kind) in kinds.iter_mut().enumerate() {
            let Some(s) = registry.state(BlockStateId(i as u32)) else {
                continue;
            };
            if let Some((_, k)) = BLOCKS.iter().find(|(n, _)| *n == s.name.as_str()) {
                *kind = *k;
                mine.push(i as u32);
                if *k == Kind::MegaTorch
                    && s.properties.get("facing").is_none_or(|f| f.as_str() == "up")
                    && mega_up.is_none()
                {
                    mega_up = Some(BlockStateId(i as u32));
                }
            }
        }
        Self {
            len,
            kinds,
            mine,
            light: registry.default_state("torchmaster:invisible_light"),
            mega_up,
        }
    }
    fn kind(&self, state: BlockStateId) -> Kind {
        self.kinds
            .get(state.0 as usize)
            .copied()
            .unwrap_or(Kind::Other)
    }
}

/// A world change the registry asks for once the game can be changed.
enum Action {
    /// Stand a mega torch up.
    StandUp(Pos),
    /// `removeChildLights` of a lantern that went away.
    RemoveLights(Vec<Pos>),
}

/// One level of one world: its registry and what has been read of the
/// world's watched writes.
struct LevelState {
    root: PathBuf,
    dim: &'static str,
    reg: Registry,
    dirty: bool,
    /// How many of `World::watched_writes` were read.
    seen: usize,
    known: HashSet<(i32, i32)>,
    known_revision: Option<u64>,
    ticks: u64,
    actions: Vec<Action>,
}

/// The range displays this client shows (`EntityBlockingVolumeRenderer`).
#[derive(Default)]
struct Volumes {
    dim: &'static str,
    /// Torch position: range and colour.
    shown: BTreeMap<Pos, (i32, u32)>,
}

#[derive(Default)]
struct Runtime {
    ids: Option<Rc<Ids>>,
    filters: Option<Rc<Filters>>,
    levels: Vec<LevelState>,
    /// A siege spot of this setup was inside a mega torch's cube.
    siege_blocked: bool,
    /// `fishing_line` is being asked for the game's own lines.
    in_lines: bool,
    volumes: Volumes,
}

thread_local! {
    static RT: RefCell<Runtime> = RefCell::new(Runtime::default());
}
/// The runtime of this thread (a world's server thread, or the window).
/// A nested call (which none of the hooks makes) gets `None`, not a panic.
fn with_rt<T>(f: impl FnOnce(&mut Runtime) -> T) -> Option<T> {
    RT.with(|rt| rt.try_borrow_mut().ok().map(|mut rt| f(&mut rt)))
}

fn file_of(root: &Path, dim: &str) -> PathBuf {
    root.join("data")
        .join("torchmaster")
        .join(format!("{}.txt", dim.replace([':', '/'], "_")))
}

impl Runtime {
    fn level(&mut self, game: &Game<'_>) -> &mut LevelState {
        let dim = game.dim.id();
        let index = match self
            .levels
            .iter()
            .position(|l| l.dim == dim && l.root == game.world_root)
        {
            Some(i) => i,
            None => {
                let reg = if game.world_root.as_os_str().is_empty() {
                    Registry::default()
                } else {
                    std::fs::read_to_string(file_of(&game.world_root, dim))
                        .map(|t| engine::load(&t))
                        .unwrap_or_default()
                };
                self.levels.push(LevelState {
                    root: game.world_root.clone(),
                    dim,
                    reg,
                    dirty: false,
                    seen: 0,
                    known: HashSet::new(),
                    known_revision: None,
                    ticks: 0,
                    actions: Vec::new(),
                });
                self.levels.len() - 1
            }
        };
        &mut self.levels[index]
    }
}

/// The level as the registry's rules read it.
struct Live<'a, 'g> {
    game: &'a Game<'g>,
    ids: &'a Ids,
}
impl engine::Blocks for Live<'_, '_> {
    fn kind(&self, p: Pos) -> Option<Kind> {
        let g = self.game;
        if p[1] < g.dim.min_y() || p[1] > g.dim.max_y() || !g.world.loaded(p[0], p[2]) {
            return None;
        }
        Some(self.ids.kind(g.world.block(p[0], p[1], p[2])))
    }
}

/// The level as the lantern rules change it.
struct LiveMut<'a, 'g> {
    game: &'a mut Game<'g>,
    ids: &'a Ids,
}
impl engine::Blocks for LiveMut<'_, '_> {
    fn kind(&self, p: Pos) -> Option<Kind> {
        engine::Blocks::kind(
            &Live {
                game: self.game,
                ids: self.ids,
            },
            p,
        )
    }
}
impl engine::LightEditor for LiveMut<'_, '_> {
    fn remove_light(&mut self, p: Pos) {
        // Level.removeBlock(pos, false): the fluid's block, with updates.
        let at = horizon_core::position::BlockPos::new(p[0], p[1], p[2]);
        self.game
            .with_level(|b, level, ticks| b.remove_block(level, ticks, at, false));
        let _ = self.game.process_level_events();
    }
}
impl LiveMut<'_, '_> {
    fn is_empty(&self, s: BlockStateId) -> bool {
        self.game.res.facts().is_air(s) || self.ids.kind(s) == Kind::Light
    }
}
impl engine::LanternWorld for LiveMut<'_, '_> {
    fn loaded(&self, p: Pos) -> bool {
        let g = &*self.game;
        p[1] >= g.dim.min_y() && p[1] <= g.dim.max_y() && g.world.loaded(p[0], p[2])
    }
    fn surface(&self, x: i32, z: i32) -> i32 {
        // The game's heightmap counts a light as a block (only air, cave air
        // and void air are air to it); vanilla's WORLD_SURFACE skips it.
        let g = &*self.game;
        g.world
            .chunks
            .get(&(x >> 4, z >> 4))
            .and_then(|c| {
                c.highest(g.world.air.0, |y| {
                    !self.is_empty(c.block(x & 15, y, z & 15))
                })
            })
            .map_or(g.dim.min_y(), |y| y + 1)
    }
    fn height(&self) -> i32 {
        self.game.dim.max_y() - self.game.dim.min_y() + 1
    }
    fn empty(&self, p: Pos) -> bool {
        self.is_empty(self.game.world.block(p[0], p[1], p[2]))
    }
    fn block_light(&self, p: Pos) -> i32 {
        i32::from(self.game.world.light(p[0], p[1], p[2]) & 15)
    }
    fn place_light(&mut self, p: Pos) -> bool {
        let Some(light) = self.ids.light else {
            return false;
        };
        let before = self.game.world.block(p[0], p[1], p[2]);
        if self.game.level_set(p, light).is_err() {
            return false;
        }
        self.game.world.block(p[0], p[1], p[2]) != before
    }
    fn next_int(&mut self, bound: i32) -> i32 {
        self.game.world.random.next_int_bound(bound).unwrap_or(0)
    }
}

/// The block writes since the last read: `onRemove` / `onPlace` of the
/// mod's blocks, in the order they happened.
fn capture(game: &Game<'_>, ids: &Ids, ls: &mut LevelState) {
    let writes = &game.world.watched_writes;
    if writes.len() < ls.seen {
        ls.seen = 0;
    }
    for &((x, y, z), old, new) in &writes[ls.seen..] {
        let (o, n) = (ids.kind(BlockStateId(old)), ids.kind(BlockStateId(new)));
        let p = [x, y, z];
        if n == Kind::MegaTorch && Some(BlockStateId(new)) != ids.mega_up {
            ls.actions.push(Action::StandUp(p));
        }
        if o == n {
            continue;
        }
        let tracked = |k: Kind| matches!(k, Kind::MegaTorch | Kind::DreadLamp | Kind::Lantern);
        if tracked(o) || tracked(n) {
            ls.dirty = true;
        }
        let gone = ls.reg.block_changed(p, o, n);
        if !gone.is_empty() {
            ls.actions.push(Action::RemoveLights(gone));
        }
    }
    ls.seen = writes.len();
}

/// The dye colours a right-click with a dye gives the range display
/// (`DyeColor.getTextColor`, 1.20.1 values).
fn dye_color(item: &str) -> Option<u32> {
    Some(match item {
        "minecraft:white_dye" => 16777215,
        "minecraft:orange_dye" => 16738335,
        "minecraft:magenta_dye" => 16711935,
        "minecraft:light_blue_dye" => 10141901,
        "minecraft:yellow_dye" => 16776960,
        "minecraft:lime_dye" => 12582656,
        "minecraft:pink_dye" => 16738740,
        "minecraft:gray_dye" => 8421504,
        "minecraft:light_gray_dye" => 13882323,
        "minecraft:cyan_dye" => 65535,
        "minecraft:purple_dye" => 10494192,
        "minecraft:blue_dye" => 255,
        "minecraft:brown_dye" => 9127187,
        "minecraft:green_dye" => 65280,
        "minecraft:red_dye" => 16711680,
        "minecraft:black_dye" => 0,
        _ => return None,
    })
}
/// `EntityBlockingLightBlock.use`'s default colour.
const VOLUME_COLOR: u32 = 0xA0FF20;

fn box_edges(min: [f64; 3], max: [f64; 3], color: u32, out: &mut Vec<horizon_client::gizmos::Line>) {
    let corner = |i: usize| {
        [
            if i & 1 == 0 { min[0] } else { max[0] },
            if i & 2 == 0 { min[1] } else { max[1] },
            if i & 4 == 0 { min[2] } else { max[2] },
        ]
    };
    for a in 0..8usize {
        for bit in [1usize, 2, 4] {
            if a & bit == 0 {
                out.push(horizon_client::gizmos::Line {
                    start: corner(a),
                    end: corner(a | bit),
                    color: 0xFF00_0000 | color,
                    width: horizon_client::gizmos::STROKE_WIDTH,
                });
            }
        }
    }
}

impl Game<'_> {
    fn torchmaster_ids(&self) -> Option<Rc<Ids>> {
        with_rt(|rt| {
            let len = self.pack.assets.registry.len();
            if rt.ids.as_ref().is_none_or(|i| i.len != len) {
                rt.ids = Some(Rc::new(Ids::build(self)));
            }
            rt.ids.clone()
        })
        .flatten()
        .filter(|ids| !ids.mine.is_empty())
    }
    fn torchmaster_filters(&self) -> Rc<Filters> {
        with_rt(|rt| {
            rt.filters
                .get_or_insert_with(|| {
                    let exists = |id: &str| {
                        self.res.mob_types().contains_key(id) || self.entity_type_exists(id)
                    };
                    Rc::new(Filters::new(&Settings::default(), &exists))
                })
                .clone()
        })
        .unwrap_or_default()
    }
    /// `World::watched`: the writes of the mod's blocks are recorded (the
    /// sculk index's table, which keeps the sculk blocks it holds).
    fn torchmaster_watch(&mut self, ids: &Ids) {
        let w = &mut self.world.watched;
        if ids.mine.iter().all(|&i| w.get(i as usize) == Some(&true)) {
            return;
        }
        if w.len() < ids.len {
            w.resize(ids.len, false);
        }
        for &i in &ids.mine {
            w[i as usize] = true;
        }
    }

    /// `onFinalizeSpawn`: whether the spawn of `entity` at `at` is cancelled.
    fn torchmaster_blocks_spawn(&self, entity: &str, at: [f64; 3], reason: Reason) -> bool {
        if self.remote.is_some() {
            return false;
        }
        let Some(ids) = self.torchmaster_ids() else {
            return false;
        };
        let filters = self.torchmaster_filters();
        let category = self
            .res
            .mob_types()
            .get(entity)
            .map_or(Category::Misc, |t| Category::from_name(&t.category));
        with_rt(|rt| {
            let ls = rt.level(self);
            capture(self, &ids, ls);
            ls.reg
                .should_block_spawn(&Settings::default(), &filters, entity, category, reason, at)
        })
        .unwrap_or(false)
    }
    /// A NATURAL spawn (`PhantomSpawner`, `CatSpawner`).
    pub(super) fn torchmaster_blocks_natural(&self, entity: &str, at: [f64; 3]) -> bool {
        self.torchmaster_blocks_spawn(entity, at, Reason::Natural)
    }
    /// A PATROL spawn (`PatrolSpawner.spawnPatrolMember`: the member stands
    /// on the block's corner, `setPos(pos.getX(), pos.getY(), pos.getZ())`).
    pub(super) fn torchmaster_blocks_patrol(&self, entity: &str, at: [f64; 3]) -> bool {
        self.torchmaster_blocks_spawn(entity, at, Reason::Patrol)
    }
    /// The game's spawn reason as Torchmaster's.
    fn torchmaster_reason(r: horizon_client::mobs::spawn_rules::Reason) -> Reason {
        use horizon_client::mobs::spawn_rules::Reason as R;
        match r {
            R::Natural => Reason::Natural,
            R::ChunkGeneration => Reason::ChunkGeneration,
            R::Spawner | R::TrialSpawner => Reason::Spawner,
            R::Reinforcement => Reason::Reinforcement,
            R::Patrol => Reason::Patrol,
            R::Structure => Reason::Structure,
            R::Event => Reason::Event,
            R::Command => Reason::Command,
            R::SpawnItemUse => Reason::SpawnEgg,
            R::MobSummoned => Reason::MobSummoned,
            R::Conversion => Reason::Conversion,
            R::Breeding => Reason::Breeding,
            R::Triggered => Reason::Triggered,
            R::Jockey => Reason::Jockey,
            R::Dispenser => Reason::Dispenser,
            R::Bucket => Reason::Bucket,
            // Loaded or travelling entities never pass through finalizeSpawn.
            R::Load | R::DimensionTravel => Reason::Command,
        }
    }
    /// A mob the natural spawner (or chunk generation) accepted: blocked,
    /// it still counts for its pack and the spawn state as in Forge (the
    /// cancelled mob is created, then `addFreshEntity` refuses it), but it
    /// is not added and its box is nowhere.
    pub(super) fn torchmaster_natural(
        &self,
        r: &horizon_client::mobs::spawn_selection::SpawnRequest<'_>,
    ) -> Option<horizon_client::mobs::spawn_selection::Spawned> {
        let reason = Self::torchmaster_reason(r.reason);
        if !self.torchmaster_blocks_spawn(r.entity, r.position, reason) {
            return None;
        }
        let nowhere = horizon_core::geometry::Vec3::new(0.0, -1.0e30, 0.0);
        Some(horizon_client::mobs::spawn_selection::Spawned {
            bounds: horizon_core::geometry::Aabb::new(nowhere, nowhere),
            school: true,
        })
    }
    /// `onVillageSiegeEvent`: the siege's spot is in a mega torch's cube.
    fn torchmaster_blocks_siege(&self, at: [f64; 3]) -> bool {
        if self.remote.is_some() {
            return false;
        }
        let Some(ids) = self.torchmaster_ids() else {
            return false;
        };
        with_rt(|rt| {
            let ls = rt.level(self);
            capture(self, &ids, ls);
            ls.reg.should_block_siege(&Settings::default(), at)
        })
        .unwrap_or(false)
    }
    /// `VillageSiege.tryToSetupSiege`'s `findRandomSpawnPos`: a blocked spot
    /// cancels the setup (`return false`), the remaining tries find nothing.
    pub(super) fn torchmaster_siege_spot(&self, found: Option<[f64; 3]>) -> Option<[f64; 3]> {
        if with_rt(|rt| rt.siege_blocked).unwrap_or(false) {
            return None;
        }
        let at = found?;
        if self.torchmaster_blocks_siege(at) {
            with_rt(|rt| rt.siege_blocked = true);
            return None;
        }
        Some(at)
    }
    /// A spot of this setup was blocked: the remaining tries look no more.
    pub(super) fn torchmaster_siege_blocked() -> bool {
        with_rt(|rt| rt.siege_blocked).unwrap_or(false)
    }
    /// The siege setup's result: false when a spot was blocked.
    pub(super) fn torchmaster_siege_result(value: bool) -> bool {
        let blocked = with_rt(|rt| std::mem::take(&mut rt.siege_blocked)).unwrap_or(false);
        value && !blocked
    }

    /// The writes the sculk index is about to drain.
    pub(super) fn torchmaster_before_drain(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let Some(ids) = self.torchmaster_ids() else {
            return;
        };
        self.torchmaster_watch(&ids);
        with_rt(|rt| {
            let ls = rt.level(self);
            capture(self, &ids, ls);
            ls.seen = 0;
        });
    }

    /// Once a tick, with the block entities (`Level.tickBlockEntities`) and
    /// `ServerTickEvent` for the registry.
    pub(super) fn torchmaster_tick(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let Some(ids) = self.torchmaster_ids() else {
            return;
        };
        self.torchmaster_watch(&ids);
        // The writes since, and what they ask of the world.
        let Some((actions, scan)) = with_rt(|rt| {
            let ls = rt.level(self);
            capture(self, &ids, ls);
            ls.ticks += 1;
            let revision = self.world.chunks.membership_revision();
            let scan = ls.ticks % 10 == 1 && ls.known_revision != Some(revision);
            (std::mem::take(&mut ls.actions), scan)
        }) else {
            return;
        };
        let mut orphans = Vec::new();
        for action in actions {
            match action {
                Action::StandUp(p) => {
                    let state = self.world.block(p[0], p[1], p[2]);
                    if ids.kind(state) == Kind::MegaTorch
                        && let Some(up) = ids.mega_up.filter(|up| *up != state)
                    {
                        let _ = self.level_set(p, up);
                    }
                }
                Action::RemoveLights(lights) => {
                    let mut reg = Registry::default();
                    reg.remove_lights(lights, &mut LiveMut { game: self, ids: &ids });
                    orphans.extend(reg.orphans);
                }
            }
        }
        // The chunks that arrived: the blocks the registry does not know.
        let mut found = Vec::new();
        let mut arrived_keys = None;
        if scan {
            let loaded: HashSet<(i32, i32)> = self.world.chunks.keys().copied().collect();
            let known = with_rt(|rt| std::mem::take(&mut rt.level(self).known)).unwrap_or_default();
            for key in loaded.difference(&known) {
                self.torchmaster_scan(*key, &ids, &mut found);
            }
            arrived_keys = Some((loaded, self.world.chunks.membership_revision()));
        }
        // The lanterns tick in block-ticking chunks.
        let Some(mut lanterns) = with_rt(|rt| {
            let ls = rt.level(self);
            if let Some((loaded, revision)) = arrived_keys.take() {
                ls.known = loaded;
                ls.known_revision = Some(revision);
            }
            for (p, kind) in &found {
                ls.dirty |= ls.reg.found(*p, *kind);
            }
            ls.reg.orphans.extend(orphans);
            std::mem::take(&mut ls.reg.lanterns)
        }) else {
            return;
        };
        let settings = Settings::default();
        let levels = self.tickets.levels();
        let mut gone: Vec<Pos> = Vec::new();
        let mut removed: Vec<Vec<Pos>> = Vec::new();
        let mut changed = false;
        for (p, lantern) in lanterns.iter_mut() {
            let kind = engine::Blocks::kind(&Live { game: self, ids: &ids }, *p);
            match kind {
                None => continue,
                Some(Kind::Lantern) => {}
                Some(_) => {
                    gone.push(*p);
                    removed.push(std::mem::take(&mut lantern.lights));
                    continue;
                }
            }
            if !levels.block_ticking_at(p[0], p[2])
                || !self.border.contains(f64::from(p[0]), f64::from(p[2]))
            {
                continue;
            }
            let before = lantern.lights.len();
            engine::tick_lantern(*p, lantern, &settings, &mut LiveMut { game: self, ids: &ids });
            changed |= lantern.lights.len() != before;
        }
        for p in &gone {
            lanterns.remove(p);
        }
        let mut reg = Registry::default();
        for lights in removed {
            reg.remove_lights(lights, &mut LiveMut { game: self, ids: &ids });
        }
        // The lights of removed lanterns whose chunks are here now.
        let mut waiting = with_rt(|rt| std::mem::take(&mut rt.level(self).reg.orphans))
            .unwrap_or_default();
        waiting.extend(reg.orphans);
        let mut pending = Registry {
            orphans: waiting,
            ..Registry::default()
        };
        let cleared = pending.clear_orphans(&mut LiveMut { game: self, ids: &ids });
        // RegistryContainer.onGlobalTick.
        with_rt(|rt| {
            let ls = rt.level(self);
            ls.reg.lanterns = lanterns;
            ls.reg.orphans = pending.orphans;
            ls.dirty |= changed || cleared || !gone.is_empty();
            let before = ls.reg.lights.len();
            ls.reg.global_tick(&Live { game: self, ids: &ids });
            ls.dirty |= ls.reg.lights.len() != before;
        });
    }
    /// The mod's blocks in a chunk, from the sections whose palette holds one.
    fn torchmaster_scan(&self, key: (i32, i32), ids: &Ids, found: &mut Vec<(Pos, Kind)>) {
        let Some(chunk) = self.world.chunks.get(&key) else {
            return;
        };
        let tracked = |v: u32| {
            matches!(
                ids.kind(BlockStateId(v)),
                Kind::MegaTorch | Kind::DreadLamp | Kind::Lantern
            )
        };
        let mut values = [0u32; 4096];
        for (s, section) in chunk.sections.iter().enumerate() {
            if !section.blocks.maybe_has(tracked) {
                continue;
            }
            let y0 = horizon_client::chunk::MIN_Y + s as i32 * 16;
            section.unpack(&mut values);
            for (i, v) in values.iter().enumerate() {
                if tracked(*v) {
                    let (x, z, y) = ((i & 15) as i32, ((i >> 4) & 15) as i32, (i >> 8) as i32);
                    found.push((
                        [key.0 * 16 + x, y0 + y, key.1 * 16 + z],
                        ids.kind(BlockStateId(*v)),
                    ));
                }
            }
        }
    }

    /// The world is saved: its registries go into its folder.
    pub(super) fn torchmaster_save(&mut self) {
        if self.remote.is_some() || self.world_root.as_os_str().is_empty() {
            return;
        }
        let root = self.world_root.clone();
        let files: Vec<(PathBuf, String)> = with_rt(|rt| {
            rt.levels
                .iter_mut()
                .filter(|l| l.root == root && l.dirty)
                .map(|l| {
                    l.dirty = false;
                    (file_of(&l.root, l.dim), engine::save(&l.reg))
                })
                .collect()
        })
        .unwrap_or_default();
        for (path, text) in files {
            let tmp = path.with_extension("txt.tmp");
            let written = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&tmp, text))
                .and_then(|()| std::fs::rename(&tmp, &path));
            if let Err(e) = written {
                log::warn!("torchmaster: {} not saved: {e}", path.display());
            }
        }
    }
    /// A world opens: its registries come from its folder again.
    pub(super) fn torchmaster_open() {
        with_rt(|rt| {
            rt.levels.clear();
            rt.siege_blocked = false;
            rt.volumes.shown.clear();
        });
    }

    /// `PlayerEventsHandler`: logging in clears the range displays.
    pub(super) fn torchmaster_forget_volumes() {
        with_rt(|rt| rt.volumes.shown.clear());
    }

    /// `EntityBlockingLightBlock.use` on the server, past use_hand's
    /// isSecondaryUseActive check: the click is taken (the caller swings) and
    /// the player gets the message; the range display itself is the client's.
    pub(super) fn torchmaster_use(&mut self, pos: [i32; 3]) -> bool {
        if self.remote.is_some() {
            return false;
        }
        let Some(ids) = self.torchmaster_ids() else {
            return false;
        };
        let Some(light) = ids.kind(self.world.block(pos[0], pos[1], pos[2])).light() else {
            return false;
        };
        // `show = !player.isShiftKeyDown()`.
        let show = !self.keys.contains(&super::KeyCode::ShiftLeft);
        let key = if show {
            "torchmaster.torch_volume.on_show"
        } else {
            "torchmaster.torch_volume.on_hide"
        };
        if self.lan_acting.is_some() {
            // displayClientMessage(..., true): the action bar of the acting player.
            self.guest_packets
                .push(horizon_protocol::packets::play::Clientbound::SystemChat(
                    horizon_protocol::packets::play::SystemChat {
                        content: horizon_protocol::text::translatable(key, &[]),
                        overlay: true,
                    },
                ));
        } else {
            // This window is the server: the display is ours too.
            let text = self.translate(key, key);
            self.message = Some((text, std::time::Instant::now()));
            self.torchmaster_toggle_volume(pos, light, show);
        }
        true
    }
    /// The client's half of `use` on a server (the main hand, past the
    /// sneak check): the range display, and the block reacts, so nothing is
    /// placed against it.
    pub(super) fn torchmaster_client_use(&self, pos: [i32; 3]) -> bool {
        let Some(ids) = self.torchmaster_ids() else {
            return false;
        };
        let Some(light) = ids.kind(self.world.block(pos[0], pos[1], pos[2])).light() else {
            return false;
        };
        self.torchmaster_toggle_volume(pos, light, !self.keys.contains(&super::KeyCode::ShiftLeft));
        true
    }
    /// `VolumeDisplayMessageHandler.handle`: show (with the main hand's dye
    /// colour) or hide the cube of the torch at `pos`.
    fn torchmaster_toggle_volume(&self, pos: Pos, light: Light, show: bool) {
        let color = self
            .stack_in(self.selected)
            .and_then(|s| dye_color(&s.item))
            .unwrap_or(VOLUME_COLOR);
        let range = Settings::default().radius(light);
        let dim = self.dim.id();
        with_rt(|rt| {
            if rt.volumes.dim != dim {
                rt.volumes = Volumes {
                    dim,
                    shown: BTreeMap::new(),
                };
            }
            if show {
                rt.volumes.shown.insert(pos, (range, color));
            } else {
                rt.volumes.shown.remove(&pos);
            }
        });
    }
    /// The depth-tested debug lines of a frame (the fishing line's), with
    /// the shown cubes' edges added: `None` while the game's own are asked.
    pub(super) fn torchmaster_lines(&self, alpha: f64, size: [u32; 2]) -> Option<Vec<horizon_client::gizmos::Line>> {
        if with_rt(|rt| rt.in_lines).unwrap_or(true) {
            return None;
        }
        let none_shown = with_rt(|rt| rt.volumes.shown.is_empty()).unwrap_or(true);
        if none_shown {
            return None;
        }
        with_rt(|rt| rt.in_lines = true);
        let mut lines = self.fishing_line(alpha, size);
        with_rt(|rt| rt.in_lines = false);
        let dim = self.dim.id();
        let camera = self.player.as_ref().map(|p| {
            [
                p.position.x.floor() as i32,
                p.position.y.floor() as i32,
                p.position.z.floor() as i32,
            ]
        });
        let reach = self.settings.render_distance.max(2) * 16;
        with_rt(|rt| {
            if rt.volumes.dim != dim {
                // PlayerChangedDimensionEvent: clearAll.
                rt.volumes = Volumes {
                    dim,
                    shown: BTreeMap::new(),
                };
            }
            for (p, (range, color)) in &rt.volumes.shown {
                // createVolume(pos, range) and the camera's render distance
                // box must meet.
                if let Some(c) = camera
                    && (0..3).any(|i| p[i] + range + 1 < c[i] - reach || p[i] - range > c[i] + reach + 1)
                {
                    continue;
                }
                let lo = [0, 1, 2].map(|i| f64::from(p[i] - range));
                let hi = [0, 1, 2].map(|i| f64::from(p[i] + range + 1));
                box_edges(lo, hi, *color, &mut lines);
                let at = [0, 1, 2].map(|i| f64::from(p[i]));
                box_edges(at, at.map(|v| v + 1.0), *color, &mut lines);
            }
        });
        Some(lines)
    }

    /// `FrozenPearlItem.use`: every invisible light within 15 blocks on each
    /// axis goes; the pearl has no durability in 20.1.9.
    pub(super) fn torchmaster_pearl(&mut self, stack: &horizon_client::items::ItemStack) -> Option<Result<bool>> {
        if &*stack.item != FROZEN_PEARL || self.remote.is_some() {
            return None;
        }
        let ids = self.torchmaster_ids()?;
        let p = self.player.as_ref()?.position;
        let center = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
        // level.playSound(player, blockPosition, LAVA_EXTINGUISH, BLOCKS, 0.7, 0.6).
        self.audio.play(
            "minecraft:block.lava.extinguish",
            Some(center.map(f64::from)),
            0.7,
            0.6,
        );
        let r = engine::FROZEN_PEARL_REACH;
        let lo = center.map(|v| v - r);
        let hi = center.map(|v| v + r);
        let light = ids.light;
        if self.world.may_hold(lo, hi, |s| Some(s) == light) {
            for at in engine::frozen_pearl_area(center) {
                if at[1] < self.dim.min_y() || at[1] > self.dim.max_y() {
                    continue;
                }
                if ids.kind(self.world.block(at[0], at[1], at[2])) == Kind::Light {
                    let pos = horizon_core::position::BlockPos::new(at[0], at[1], at[2]);
                    self.with_level(|b, level, ticks| b.remove_block(level, ticks, pos, false));
                }
            }
            let _ = self.process_level_events();
        }
        self.swing_arm();
        Some(Ok(true))
    }
}

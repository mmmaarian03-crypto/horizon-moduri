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

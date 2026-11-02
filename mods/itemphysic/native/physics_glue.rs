// ItemPhysic's item entity, in the game library (module `horizon_client::entities::itemphysic`): the rules are
// `engine` (src/lib.rs), this part runs them on `ItemEntity`. A child of `entities`, so it sees the item's
// private helpers (`update_fluids`).
use super::{ItemEntity, ItemEnvironmentTick};
use crate::{
    blocks::{BlockFacts, Fluid},
    collisions::SceneCollisions,
    items::Items,
    world::World,
};
use horizon_core::geometry::Vec3;
use horizon_world::block_collision::BlockCollisionRegistry;
use horizon_world::fluid::{FluidHeightWorld, FluidKind, FluidState};
use horizon_world::registry::BlockRegistry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// Each item's class (swim, burn, undestroyable), built from the item catalog the first time an item's
/// environment ticks (`Items` is not passed to `ItemEntity::tick`), again when the catalog changes size.
static CLASSES: RwLock<Option<(usize, HashMap<Arc<str>, u8>)>> = RwLock::new(None);
/// Fall distance per item entity id (`Entity.fallDistance`, which Horizon's item has no field for).
static FALLS: Mutex<Option<HashMap<u32, f32>>> = Mutex::new(None);
/// Landing sounds to play (position, volume, pitch), taken by the game after the items' tick.
pub static SOUNDS: Mutex<Vec<([f64; 3], f32, f32)>> = Mutex::new(Vec::new());
/// Items that burned in a fluid this tick (ItemPhysic's 3 lava damage): their burn sound is played.
static BURNED: Mutex<Vec<u32>> = Mutex::new(Vec::new());

struct Facts<'a> {
    items: &'a Items,
    registry: &'a BlockRegistry,
}
impl engine::ItemFacts for Facts<'_> {
    fn block_of(&self, item: &str) -> Option<String> {
        let state = self.items.items.get(item)?.block?;
        Some(self.registry.state(state)?.name.clone())
    }
    fn block_in_tag(&self, tag: &str, block: &str) -> bool {
        self.items.block_tags.get(tag.trim_start_matches('#')).is_some_and(|t| t.contains(block))
    }
    fn is_fuel(&self, item: &str) -> bool {
        self.items.items.get(item).is_some_and(|i| i.fuel.is_some())
    }
}

fn ensure_classes(items: &Items, registry: &BlockRegistry) {
    let n = items.items.len();
    if CLASSES.read().ok().and_then(|c| c.as_ref().map(|(len, _)| *len)) == Some(n) {
        return;
    }
    let facts = Facts { items, registry };
    let table = items
        .items
        .keys()
        .map(|id| (id.clone(), engine::classify(id, &facts).bits()))
        .collect::<HashMap<_, _>>();
    if let Ok(mut c) = CLASSES.write() {
        *c = Some((n, table));
    }
}
/// The class of an item, once the catalog was seen.
pub fn class_of(item: &str) -> Option<engine::Class> {
    let c = CLASSES.read().ok()?;
    let (_, table) = c.as_ref()?;
    Some(engine::Class::from_bits(table.get(item).copied().unwrap_or(0)))
}

struct FluidLook<'a> {
    world: &'a World,
    fluids: &'a crate::motion::FluidResources,
}
impl FluidHeightWorld for FluidLook<'_> {
    type Error = std::convert::Infallible;
    fn fluid(&mut self, p: [i32; 3]) -> Result<FluidState, Self::Error> {
        Ok(self.fluids.fluid(self.world.block(p[0], p[1], p[2])))
    }
    fn sturdy_down(&mut self, p: [i32; 3]) -> Result<bool, Self::Error> {
        let state = self.world.block(p[0], p[1], p[2]);
        Ok(self.fluids.sturdy(state, horizon_core::geometry::Direction::Down))
    }
}

/// `CommonPhysic.getFluid(item)`.
fn fluid_of(item: &ItemEntity, world: &World, fluids: &crate::motion::FluidResources) -> Option<engine::InFluid> {
    let p = [item.position.x.floor() as i32, item.position.y.floor() as i32, item.position.z.floor() as i32];
    let mut look = FluidLook { world, fluids };
    let state = look.fluid(p).ok()?;
    let kind = match state.kind() {
        FluidKind::Water | FluidKind::FlowingWater => engine::InFluid::Water,
        FluidKind::Lava | FluidKind::FlowingLava => engine::InFluid::Lava,
        FluidKind::Empty => return None,
    };
    let height = state.height_at(&mut look, p).unwrap_or(0.0);
    engine::in_fluid(Some(kind), item.position.y, p[1], height)
}

/// `ItemEntity.tick` with ItemPhysic's mixins (`updatePre` at `isInWater`, no vanilla water/lava/gravity
/// branch, `update` in place of the friction). `None` = the catalog is not known yet: vanilla this tick.
pub fn tick(
    item: &mut ItemEntity,
    world: &World,
    facts: &BlockFacts,
    collision: &BlockCollisionRegistry<'_>,
    no_physics: bool,
    fluids: &crate::motion::FluidResources,
) -> Option<bool> {
    let class = class_of(&item.stack.item)?;
    item.ticks += 1;
    item.previous = item.position;
    if item.pickup_delay > 0 && item.pickup_delay != u32::MAX {
        item.pickup_delay -= 1;
    }
    // Entity.baseTick: immersion and currents (vanilla's `updateFluidHeightAndDoFluidPushing`).
    let (water_height, lava_height) = item.update_fluids(world, fluids);
    let fluid = fluid_of(item, world, fluids);
    item.velocity.y = engine::pre_motion(item.velocity.y, fluid, class.swim);
    if engine::burns(fluid, lava_height, item.fire_ticks > 0, class.burn) {
        // `item.hurt(lava, 3)`: ItemPhysic's `hurt` lets lava through for an item that burns.
        item.health -= 3;
        if let Ok(mut b) = BURNED.lock() {
            b.push(item.id);
        }
    }
    let horizontal = item.velocity.x * item.velocity.x + item.velocity.z * item.velocity.z;
    if !item.on_ground || horizontal > 1.0e-5 || (item.age + item.id).is_multiple_of(4) {
        let mut scene = SceneCollisions { world, binding: collision, subject: None };
        let wanted = item.velocity;
        let moved = if no_physics {
            wanted
        } else {
            scene.resolve(item.bounds(), wanted, 0.0, item.on_ground).unwrap_or(Vec3::ZERO)
        };
        item.position = Vec3::new(item.position.x + moved.x, item.position.y + moved.y, item.position.z + moved.z);
        if !no_physics {
            item.on_ground = wanted.y < 0.0 && moved.y != wanted.y;
        }
        // Entity.checkFallDamage (ItemPhysicServer.checkFallDamage plays the landing sound first).
        if let Ok(mut f) = FALLS.lock() {
            let map = f.get_or_insert_with(HashMap::new);
            let before = map.get(&item.id).copied().unwrap_or(0.0);
            let (after, sound) = engine::fall(before, moved.y, item.on_ground, water_height > 0.0);
            if after > 0.0 {
                map.insert(item.id, after);
            } else {
                map.remove(&item.id);
            }
            if let Some(volume) = sound {
                // `(float) Math.random() * 1.0F + 1.0F`
                let r = (item.id.wrapping_mul(2_654_435_761) ^ item.ticks.wrapping_mul(40_503)) % 1000;
                if let Ok(mut s) = SOUNDS.lock() {
                    s.push(([item.position.x, item.position.y, item.position.z], volume, 1.0 + r as f32 / 1000.0));
                }
            }
        }
        if moved.x != wanted.x {
            item.velocity.x = 0.0;
        }
        if moved.z != wanted.z {
            item.velocity.z = 0.0;
        }
        if moved.y != wanted.y {
            item.velocity.y = 0.0;
        }
        let mut friction = 0.6f32;
        if item.on_ground {
            // ItemPhysicServer.update: `BlockPos.containing(x, y - 1, z)`.
            let below = world.block(
                item.position.x.floor() as i32,
                (item.position.y - 1.0).floor() as i32,
                item.position.z.floor() as i32,
            );
            friction = facts.get(below).friction;
        }
        let v = engine::post_motion([item.velocity.x, item.velocity.y, item.velocity.z], item.on_ground, friction, fluid);
        item.velocity = Vec3::new(v[0], v[1], v[2]);
    }
    item.age += 1;
    item.update_fluids(world, fluids);
    Some(item.age < 6000 && item.position.y > -128.0)
}

/// `ItemEntity::environment_tick` with ItemPhysic's `fireImmune` (an item that does not burn) and `hurt` (the
/// undestroyable list, no cactus damage, fire and lava only for items that burn). Also where the catalog is read.
pub fn environment(
    item: &mut ItemEntity,
    world: &World,
    facts: &BlockFacts,
    registry: &BlockRegistry,
    items: &Items,
) -> ItemEnvironmentTick {
    ensure_classes(items, registry);
    let class = class_of(&item.stack.item).unwrap_or_default();
    let may = |source: engine::Source, vanilla_resists: bool| engine::may_hurt(class, source) && !vanilla_resists;
    let at = world.block(
        item.position.x.floor() as i32,
        (item.position.y + 0.1).floor() as i32,
        item.position.z.floor() as i32,
    );
    let fluid = facts.get(at).fluid;
    let fire_immune = !class.burn || items.resists_fire(&item.stack, "minecraft:in_fire");
    if fluid == Fluid::Water || fire_immune {
        item.fire_ticks = 0;
    }
    if item.fire_ticks > 0 {
        if item.fire_ticks % 20 == 0
            && fluid != Fluid::Lava
            && may(engine::Source::Fire, items.resists_fire(&item.stack, "minecraft:on_fire"))
        {
            item.health -= 1;
        }
        item.fire_ticks -= 1;
    }
    let mut lava_hurt = false;
    if fluid == Fluid::Lava && !fire_immune {
        item.fire_ticks = item.fire_ticks.max(300);
        if may(engine::Source::Lava, items.resists_fire(&item.stack, "minecraft:lava")) {
            item.health -= 4;
            lava_hurt = true;
        }
    }
    let fire_damage = crate::motion::environment::fire_contact(world, registry, item.bounds());
    if fire_damage > 0.0 && !fire_immune {
        item.fire_ticks = item.fire_ticks.max(160);
        if may(engine::Source::Fire, items.resists_fire(&item.stack, "minecraft:in_fire")) {
            item.health -= fire_damage as i32;
        }
    }
    if crate::motion::environment::cactus_contact(world, registry, item.bounds())
        && may(engine::Source::Cactus, items.resists_cactus(&item.stack))
    {
        item.health -= 1;
    }
    // ItemPhysic's own burn in a fluid this tick: the burn sound (GENERIC_BURN, 0.4, 2 + rand * 0.4).
    if let Ok(mut b) = BURNED.lock()
        && let Some(i) = b.iter().position(|id| *id == item.id)
    {
        b.swap_remove(i);
        lava_hurt = true;
    }
    if item.health <= 0
        && let Ok(mut f) = FALLS.lock()
        && let Some(map) = f.as_mut()
    {
        map.remove(&item.id);
    }
    ItemEnvironmentTick { alive: item.health > 0, lava_hurt }
}

/// The ids of the items still in the level: forget the fall distances of the others.
pub fn forget_except(alive: &[u32]) {
    if let Ok(mut f) = FALLS.lock()
        && let Some(map) = f.as_mut()
    {
        map.retain(|id, _| alive.contains(id));
    }
    if let Ok(mut b) = BURNED.lock() {
        b.clear();
    }
}

/// The undestroyable items (bedrock, obsidian, barrier...) an explosion must not destroy: the explosion code
/// spares only nether stars, so these go through it as nether stars and come back afterwards.
pub fn undestroyable(item: &str) -> bool {
    class_of(item).is_some_and(|c| c.undestroyable)
}

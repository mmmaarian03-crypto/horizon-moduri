// ItemPhysic (CreativeMD, LGPL-3.0; `ItemPhysic_FORGE_v1.8.17_mc1.20.1.jar` + CreativeCore 2.12.40) for Horizon: the
// server rules, without game code. Written from the 1.20.1 jar (decompiled: `ItemPhysicServer`, `ItemEntityMixin`,
// `CommonPhysic`, `ItemPhysicConfig`, CreativeCore's `CreativeForgeLoader.getFluidViscosityMultiplier`) and the pack's
// `config/itemphysic.json`. The engine is pasted into the game library twice: as `engine` in the item module
// (`mixins/itemphysic/physics.rs`, made by `tools/bundle_physics.py`) and in the game (`tools/hnmod.py bundle`).

/// the pack's `config/itemphysic.json` (server half; the client file only draws, see README).
pub mod config {
    use super::Filter;
    pub const GENERAL_FALL_SOUNDS: bool = true; // itemphysic.json [general] fallSounds
    pub const GENERAL_DISABLE_CACTUS_DAMAGE: bool = true; // itemphysic.json [general] disableCactusDamage
    pub const GENERAL_VANILLA_FLOW_BEHAVIOR: bool = false; // itemphysic.json [general] vanillaFlowBehavior
    pub const PICKUP_CUSTOM_PICKUP: bool = false; // itemphysic.json [pickup] customPickup (false: vanilla pickup)
    pub const THROW_ENABLED: bool = true; // itemphysic.json [throw] enabled (client key; see README)
    pub const THROW_MAX_STAGES: i64 = 6; // itemphysic.json [throw] maxStages
    /// itemphysic.json [general.swimmingItems] entries (isWhitelist = true), in file order.
    pub const SWIMMING: &[Filter] = &[
        Filter::BlockTag("minecraft:mineable/axe"),
        Filter::BlockTag("minecraft:logs"),
        Filter::BlockTag("minecraft:planks"),
        Filter::Block("minecraft:sponge"),
        Filter::Block("minecraft:wet_sponge"),
        Filter::BlockTag("minecraft:ice"),
        Filter::BlockTag("minecraft:leaves"),
        Filter::BlockTag("minecraft:flowers"),
        Filter::BlockTag("minecraft:small_flowers"),
        Filter::BlockTag("minecraft:tall_flowers"),
        Filter::BlockTag("minecraft:snow"),
        Filter::Block("minecraft:cactus"),
        Filter::Block("minecraft:cake"),
        Filter::Block("minecraft:cobweb"),
        Filter::Block("minecraft:snow"),
        Filter::Item("minecraft:apple"),
        Filter::Item("minecraft:bow"),
        Filter::Item("minecraft:bowl"),
        Filter::Item("minecraft:arrow"),
        Filter::Item("minecraft:apple"),
        Filter::Block("minecraft:tripwire"),
        Filter::Item("minecraft:feather"),
        Filter::Item("minecraft:wheat"),
        Filter::Item("minecraft:bread"),
        Filter::Item("minecraft:painting"),
        Filter::Item("minecraft:acacia_boat"),
        Filter::Block("minecraft:acacia_door"),
        Filter::Item("minecraft:dark_oak_boat"),
        Filter::Block("minecraft:dark_oak_door"),
        Filter::Item("minecraft:birch_boat"),
        Filter::Block("minecraft:birch_door"),
        Filter::Item("minecraft:jungle_boat"),
        Filter::Block("minecraft:jungle_door"),
        Filter::Item("minecraft:oak_boat"),
        Filter::Block("minecraft:oak_door"),
        Filter::Item("minecraft:spruce_boat"),
        Filter::Block("minecraft:spruce_door"),
        Filter::Item("minecraft:saddle"),
        Filter::Item("minecraft:bone"),
        Filter::Item("minecraft:sugar"),
        Filter::Item("minecraft:egg"),
        Filter::Item("minecraft:fishing_rod"),
        Filter::Block("minecraft:cake"),
        Filter::Block("minecraft:melon"),
        Filter::Item("minecraft:shears"),
        Filter::Block("minecraft:carrots"),
        Filter::Block("minecraft:potatoes"),
        Filter::Item("minecraft:poisonous_potato"),
        Filter::Item("minecraft:baked_potato"),
        Filter::Item("minecraft:pumpkin_pie"),
        Filter::Item("minecraft:elytra"),
        Filter::Item("minecraft:mutton"),
        Filter::Item("minecraft:cooked_mutton"),
        Filter::Item("minecraft:rabbit"),
        Filter::Item("minecraft:cooked_rabbit"),
        Filter::Item("minecraft:rabbit_stew"),
        Filter::Item("minecraft:beetroot"),
        Filter::Block("minecraft:beetroots"),
        Filter::Item("minecraft:beetroot_soup"),
        Filter::Item("minecraft:shield"),
        Filter::Block("minecraft:wheat"),
        Filter::Block("minecraft:pumpkin_stem"),
        Filter::Block("minecraft:melon_stem"),
        Filter::Item("minecraft:snowball"),
    ];
    /// itemphysic.json [general.burningItems] entries (isWhitelist = true), in file order.
    pub const BURNING: &[Filter] = &[
        Filter::BlockTag("minecraft:mineable/axe"),
        Filter::BlockTag("minecraft:logs_that_burn"),
        Filter::BlockTag("minecraft:planks"),
        Filter::Block("minecraft:sponge"),
        Filter::Block("minecraft:wet_sponge"),
        Filter::BlockTag("minecraft:ice"),
        Filter::BlockTag("minecraft:leaves"),
        Filter::BlockTag("minecraft:wool"),
        Filter::BlockTag("minecraft:wool_carpets"),
        Filter::BlockTag("minecraft:flowers"),
        Filter::BlockTag("minecraft:small_flowers"),
        Filter::BlockTag("minecraft:tall_flowers"),
        Filter::BlockTag("minecraft:snow"),
        Filter::Block("minecraft:cactus"),
        Filter::Block("minecraft:cake"),
        Filter::Block("minecraft:cobweb"),
        Filter::Block("minecraft:snow"),
        Filter::Item("minecraft:apple"),
        Filter::Item("minecraft:bow"),
        Filter::Item("minecraft:bowl"),
        Filter::Item("minecraft:arrow"),
        Filter::Item("minecraft:apple"),
        Filter::Block("minecraft:tripwire"),
        Filter::Item("minecraft:feather"),
        Filter::Item("minecraft:wheat"),
        Filter::Item("minecraft:bread"),
        Filter::Item("minecraft:leather"),
        Filter::Item("minecraft:leather_boots"),
        Filter::Item("minecraft:leather_chestplate"),
        Filter::Item("minecraft:leather_helmet"),
        Filter::Item("minecraft:leather_leggings"),
        Filter::Item("minecraft:lead"),
        Filter::Item("minecraft:painting"),
        Filter::Item("minecraft:acacia_boat"),
        Filter::Block("minecraft:acacia_door"),
        Filter::Item("minecraft:dark_oak_boat"),
        Filter::Block("minecraft:dark_oak_door"),
        Filter::Item("minecraft:birch_boat"),
        Filter::Block("minecraft:birch_door"),
        Filter::Item("minecraft:jungle_boat"),
        Filter::Block("minecraft:jungle_door"),
        Filter::Item("minecraft:oak_boat"),
        Filter::Block("minecraft:oak_door"),
        Filter::Item("minecraft:spruce_boat"),
        Filter::Block("minecraft:spruce_door"),
        Filter::Item("minecraft:saddle"),
        Filter::Item("minecraft:bone"),
        Filter::Item("minecraft:sugar"),
        Filter::Item("minecraft:paper"),
        Filter::Item("minecraft:book"),
        Filter::Item("minecraft:egg"),
        Filter::Item("minecraft:fishing_rod"),
        Filter::Block("minecraft:cake"),
        Filter::Block("minecraft:melon"),
        Filter::Item("minecraft:shears"),
        Filter::Item("minecraft:writable_book"),
        Filter::Item("minecraft:written_book"),
        Filter::Block("minecraft:carrots"),
        Filter::Block("minecraft:potatoes"),
        Filter::Item("minecraft:poisonous_potato"),
        Filter::Item("minecraft:baked_potato"),
        Filter::Item("minecraft:map"),
        Filter::Item("minecraft:filled_map"),
        Filter::Item("minecraft:pumpkin_pie"),
        Filter::Item("minecraft:name_tag"),
        Filter::Item("minecraft:enchanted_book"),
        Filter::Item("minecraft:elytra"),
        Filter::Item("minecraft:mutton"),
        Filter::Item("minecraft:cooked_mutton"),
        Filter::Item("minecraft:rabbit"),
        Filter::Item("minecraft:cooked_rabbit"),
        Filter::Item("minecraft:rabbit_stew"),
        Filter::Item("minecraft:beetroot"),
        Filter::Block("minecraft:beetroots"),
        Filter::Item("minecraft:beetroot_soup"),
        Filter::Item("minecraft:shield"),
        Filter::Block("minecraft:wheat"),
        Filter::Block("minecraft:pumpkin_stem"),
        Filter::Block("minecraft:melon_stem"),
        Filter::Fuel,
        Filter::Item("minecraft:spider_eye"),
        Filter::Item("minecraft:rotten_flesh"),
        Filter::Item("minecraft:snowball"),
    ];
    /// itemphysic.json [general.undestroyableItems] entries (isWhitelist = true), in file order.
    pub const UNDESTROYABLE: &[Filter] = &[
        Filter::Item("minecraft:nether_star"),
        Filter::Block("minecraft:bedrock"),
        Filter::Block("minecraft:obsidian"),
        Filter::Block("minecraft:barrier"),
    ];
}

/// One entry of a CreativeCore item filter (`{id:"blocktag",tag:..}`, `{id:"block",block:..}`, `{id:"item",item:..}`,
/// `{id:"fuel"}`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    /// A block item whose block is in the block tag.
    BlockTag(&'static str),
    /// A block item of this block (`BlockItem.getBlock() == block`: wheat seeds for `minecraft:wheat`).
    Block(&'static str),
    Item(&'static str),
    /// `ForgeHooks.getBurnTime(stack) > 0`.
    Fuel,
}

/// What a filter needs to know about an item.
pub trait ItemFacts {
    /// The block the item places, by name (a `BlockItem`).
    fn block_of(&self, item: &str) -> Option<String>;
    fn block_in_tag(&self, tag: &str, block: &str) -> bool;
    fn is_fuel(&self, item: &str) -> bool;
}

/// A whitelist filter (`isWhitelist = true`): any entry matches.
pub fn passes(filters: &[Filter], item: &str, facts: &impl ItemFacts) -> bool {
    let block = facts.block_of(item);
    filters.iter().any(|f| match *f {
        Filter::BlockTag(tag) => block.as_deref().is_some_and(|b| facts.block_in_tag(tag, b)),
        Filter::Block(b) => block.as_deref() == Some(b),
        Filter::Item(i) => item == i,
        Filter::Fuel => facts.is_fuel(item),
    })
}

/// What ItemPhysic decides once per item (`ItemEntityMixin.onSyncedDataUpdated`: `swim`, `burn`), plus the
/// undestroyable list (`ItemPhysicServer.hurt`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Class {
    pub swim: bool,
    pub burn: bool,
    pub undestroyable: bool,
}
pub fn classify(item: &str, facts: &impl ItemFacts) -> Class {
    Class {
        swim: passes(config::SWIMMING, item, facts),
        burn: passes(config::BURNING, item, facts),
        undestroyable: passes(config::UNDESTROYABLE, item, facts),
    }
}
impl Class {
    pub fn bits(self) -> u8 {
        u8::from(self.swim) | u8::from(self.burn) << 1 | u8::from(self.undestroyable) << 2
    }
    pub fn from_bits(b: u8) -> Self {
        Class { swim: b & 1 != 0, burn: b & 2 != 0, undestroyable: b & 4 != 0 }
    }
}

/// The fluid an item is in (`CommonPhysic.getFluid`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InFluid {
    Water,
    Lava,
}
/// `getFluid`: the fluid of the item's block when the item is at most 0.2 above its surface:
/// `y - blockY - 0.2 <= height` (`FluidState.getHeight`).
pub fn in_fluid(fluid: Option<InFluid>, y: f64, block_y: i32, height: f32) -> Option<InFluid> {
    fluid.filter(|_| y - f64::from(block_y) - 0.2 <= f64::from(height))
}

/// CreativeCore `getFluidViscosityMultiplier` = `FluidType.getViscosity() / 1000`: Forge water 1000, lava 6000.
pub fn viscosity(f: InFluid) -> f64 {
    match f {
        InFluid::Water => 1.0,
        InFluid::Lava => 6.0,
    }
}

/// `ItemPhysicServer.updatePre` on the vertical speed (vanilla's water/lava/gravity branch is skipped).
pub fn pre_motion(vy: f64, fluid: Option<InFluid>, swim: bool) -> f64 {
    let Some(f) = fluid else {
        return vy - 0.04;
    };
    let mut vy = vy;
    let mut force = -0.02 / viscosity(f).max(1.0);
    if swim && f != InFluid::Lava {
        let max_speed = 0.1;
        if vy < max_speed {
            force = (0.04f64).min(max_speed - vy);
        }
    } else if vy < -0.1 {
        force = 0.0;
        vy *= 0.8;
    }
    vy + force
}

/// `updatePre`'s burn: in a fluid, lava over the eyes (`fluidHeight(LAVA) > eyeHeight - 0.11111111`, item eye
/// height 0.2125) or burning, and the item burns: 3 lava damage, the burn sound, smoke.
pub fn burns(fluid: Option<InFluid>, lava_height: f64, on_fire: bool, burn: bool) -> bool {
    const EYE: f32 = 0.25 * 0.85;
    fluid.is_some() && burn && (lava_height > f64::from(EYE - 0.111_111_11) || on_fire)
}

/// `ItemPhysicServer.update` (in place of vanilla's friction): on the ground the block's friction * 0.98; in air
/// `(f, 0.98, f)` and the bounce; in a fluid only the horizontal speed slows, by `1 / (1.2 * viscosity)`.
pub fn post_motion(v: [f64; 3], on_ground: bool, block_friction: f32, fluid: Option<InFluid>) -> [f64; 3] {
    let mut f = 0.98f32;
    if on_ground {
        f = block_friction * 0.98;
    }
    let f = f64::from(f);
    match fluid {
        None => {
            let mut out = [v[0] * f, v[1] * 0.98, v[2] * f];
            if on_ground && out[1] < 0.0 {
                out[1] *= -0.5;
            }
            out
        }
        Some(fl) => {
            let k = 1.0 / (1.2 * viscosity(fl));
            [v[0] * k, v[1], v[2] * k]
        }
    }
}

/// `Entity.checkFallDamage`'s fall distance (`fallDistance -= dy` while falling in air, reset on the ground or in
/// water) and ItemPhysic's landing sound (`WOOL_FALL`, volume `min(1, fallDistance / 10)`, pitch 1..2).
pub fn fall(distance: f32, dy: f64, on_ground: bool, in_water: bool) -> (f32, Option<f32>) {
    if in_water {
        return (0.0, None);
    }
    if on_ground {
        let sound = (config::GENERAL_FALL_SOUNDS && distance > 0.0).then(|| (distance / 10.0).min(1.0));
        return (0.0, sound);
    }
    if dy < 0.0 {
        return (distance - dy as f32, None);
    }
    (distance, None)
}
pub const FALL_SOUND: &str = "minecraft:block.wool.fall";

/// `ItemPhysicServer.hurt`'s answer for one damage source: may it hurt the item?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Fire,
    Lava,
    Cactus,
    Explosion,
    Other,
}
pub fn may_hurt(class: Class, source: Source) -> bool {
    if class.undestroyable {
        return false;
    }
    match source {
        Source::Fire | Source::Lava => class.burn,
        Source::Cactus => !config::GENERAL_DISABLE_CACTUS_DAMAGE,
        Source::Explosion | Source::Other => true,
    }
}

/// The original's plain data, read from the 1.20.1 jar by tools/jar_data.py: constants, enum values, static
/// tables, numbers given to builders, constant getters, chances, the numbers of each method, and pure static
/// methods translated from bytecode (each checked against the JVM). Use `jar_data::NAME` and cite it; delete what
/// the port does not need.
#[allow(dead_code, non_upper_case_globals, non_camel_case_types, non_snake_case, clippy::all)]
pub mod jar_data {
    // Data read from the 1.20.1 jar `ItemPhysic_FORGE_v1.8.17_mc1.20.1.jar` by tools/jar_data.py (first pass, generated).
    // Each constant names the Java class and the field / method it comes from. Values that depend on a
    // branch, a parameter or the config are left out. Keep what the rules need, cite it, delete the rest.

    // ---- static final constants (ConstantValue)
    /// `team.creative.itemphysic.ItemPhysic.MODID`
    pub const ITEM_PHYSIC_MODID: &'static str = "itemphysic";
    /// `team.creative.itemphysic.client.ItemPhysicClient.RANDOM_Y_OFFSET_SCALE`
    pub const ITEM_PHYSIC_CLIENT_RANDOM_Y_OFFSET_SCALE: f64 = 0.007957747154594767;

    // ---- numbers passed to builders and constructors (durability, damage, speed, cooldowns...)
    /// `team.creative.itemphysic.common.packet.DropPacket.createEntity`: `ItemEntity.setPickUpDelay(40)`
    pub const DROP_PACKET_SET_PICK_UP_DELAY: i32 = 40;

    // ---- methods that only return a constant (data overrides)
    /// `team.creative.itemphysic.mixin.ItemEntityMixin.isInWaterRedirect(ItemEntity)` returns it
    pub const ITEM_ENTITY_MIXIN_IS_IN_WATER_REDIRECT: bool = false;
    /// `team.creative.itemphysic.mixin.ItemEntityMixin.isInLavaRedirect(ItemEntity)` returns it
    pub const ITEM_ENTITY_MIXIN_IS_IN_LAVA_REDIRECT: bool = false;
    /// `team.creative.itemphysic.mixin.ItemEntityMixin.isNoGravityRedirect(ItemEntity)` returns it
    pub const ITEM_ENTITY_MIXIN_IS_NO_GRAVITY_REDIRECT: bool = true;

    // ---- numbers in each method body (bytecode order, once each, without 0 / 1 / -1): a checklist of the
    // values the original uses there, not named constants. Ints exact; f32 values written with their f32 digits.
    /// `team.creative.itemphysic.ItemPhysicConfig$General.<init>()`: integers
    pub const GENERAL_INIT_INTS: &[i64] = &[64, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48];
    /// `team.creative.itemphysic.ItemPhysicConfig$Pickup.<init>()`: floats
    pub const PICKUP_INIT_FLOATS: &[f64] = &[5.0, 0.2];
    /// `team.creative.itemphysic.ItemPhysicConfig$Throw.<init>()`: integers
    pub const THROW_INIT_INTS: &[i64] = &[6, 10];
    /// `team.creative.itemphysic.client.ItemPhysicClient.renderTooltip(GuiGraphics)`: integers
    pub const ITEM_PHYSIC_CLIENT_RENDER_TOOLTIP_INTS: &[i64] = &[2, 10, 9, 16579836];
    /// `team.creative.itemphysic.client.ItemPhysicClient.render(ItemEntity, F, F, PoseStack, MultiBufferSource, I, ItemRenderer, RandomSource)`: integers
    pub const ITEM_PHYSIC_CLIENT_RENDER_INTS: &[i64] = &[187, 4, 90];
    /// `team.creative.itemphysic.client.ItemPhysicClient.render(ItemEntity, F, F, PoseStack, MultiBufferSource, I, ItemRenderer, RandomSource)`: floats
    pub const ITEM_PHYSIC_CLIENT_RENDER_FLOATS: &[f64] = &[2.0e8, 0.2, 1.5707964, 2.0, 5.0, 9.0e1, 1.8e2, 2.7e2, -0.2, -0.08, -0.14, 0.007957747154594767, -0.04, 0.5, -0.09375, 0.15, 0.09375];
    /// `team.creative.itemphysic.client.ItemPhysicClient.getModelCount(ItemStack)`: integers
    pub const ITEM_PHYSIC_CLIENT_GET_MODEL_COUNT_INTS: &[i64] = &[48, 5, 32, 4, 16, 3, 2];
    /// `team.creative.itemphysic.common.CommonPhysic.getFluid(ItemEntity, Z)`: floats
    pub const COMMON_PHYSIC_GET_FLUID_FLOATS: &[f64] = &[0.2];
    /// `team.creative.itemphysic.common.CommonPhysic.getReachDistance(Player)`: floats
    pub const COMMON_PHYSIC_GET_REACH_DISTANCE_FLOATS: &[f64] = &[5.0];
    /// `team.creative.itemphysic.common.packet.DropPacket.createEntity(ItemStack, Player, Z, Z)`: integers
    pub const DROP_PACKET_CREATE_ENTITY_INTS: &[i64] = &[40];
    /// `team.creative.itemphysic.common.packet.DropPacket.createEntity(ItemStack, Player, Z, Z)`: floats
    pub const DROP_PACKET_CREATE_ENTITY_FLOATS: &[f64] = &[0.30000001192092896, 0.5, 6.2831855, 0.20000000298023224, 0.017453292, 0.02, 0.1];
    /// `team.creative.itemphysic.server.ItemPhysicServer.updatePre(ItemEntity, RandomSource)`: integers
    pub const ITEM_PHYSIC_SERVER_UPDATE_PRE_INTS: &[i64] = &[100];
    /// `team.creative.itemphysic.server.ItemPhysicServer.updatePre(ItemEntity, RandomSource)`: floats
    pub const ITEM_PHYSIC_SERVER_UPDATE_PRE_FLOATS: &[f64] = &[-0.04, -0.02, 0.1, 0.04, -0.1, 0.8, 0.11111111, 0.4, 2.0, 0.05, 0.2, 3.0];
    /// `team.creative.itemphysic.server.ItemPhysicServer.updateFluidHeightAndDoFluidPushing(ItemEntity, TagKey, D)`: floats
    pub const ITEM_PHYSIC_SERVER_UPDATE_FLUID_HEIGHT_AND_DO_FLUID_PUSHING_FLOATS: &[f64] = &[-0.001, 0.3, 0.4, 0.003, 0.0045];
    /// `team.creative.itemphysic.server.ItemPhysicServer.update(ItemEntity)`: floats
    pub const ITEM_PHYSIC_SERVER_UPDATE_FLOATS: &[f64] = &[0.98, 0.98, -0.5, 0.10000000149011612, 1.2];
    /// `team.creative.itemphysic.server.ItemPhysicServer.checkFallDamage(ItemEntity, D, Z, BlockState, BlockPos)`: floats
    pub const ITEM_PHYSIC_SERVER_CHECK_FALL_DAMAGE_FLOATS: &[f64] = &[1.0e1];
    /// `team.creative.itemphysic.server.ItemPhysicServer.playerTouch(ItemEntity, Player)`: floats
    pub const ITEM_PHYSIC_SERVER_PLAYER_TOUCH_FLOATS: &[f64] = &[0.5];
    /// `team.creative.itemphysic.server.ItemPhysicServer.playerPickup(ItemEntity, Player)`: floats
    pub const ITEM_PHYSIC_SERVER_PLAYER_PICKUP_FLOATS: &[f64] = &[0.2, 1.4, 2.0];
}

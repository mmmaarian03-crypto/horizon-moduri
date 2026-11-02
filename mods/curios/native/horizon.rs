// Curios API inside Horizon's game (nightly 142388e1c). The rules are the engine (src/lib.rs); this
// part reads the game. It is also the shared API other native mods call (`crate::game::curios::...`,
// see its README); the Curios screen is a forja menu (crate::game::forja, mods/forja/API.md
// section 10), so this mod needs forja in the same build.
#[allow(unused_imports)]
use super::{Game, Result, Screen, lan::LanHost};
use horizon_client::{
    containers::BlockEntity,
    hud::{HudBatch, rgba},
    items::ItemStack,
};
use horizon_core::random::RandomSource;
use horizon_world::nbt::{JavaString, Tag};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[allow(dead_code)]
pub(in crate::game) mod engine {
    include!("curios_engine.rs");
}
#[allow(unused_imports)]
pub(in crate::game) use engine::{DropRule, LegacyMessage, SlotCtx, SlotModifier, SlotType};

/// Used by kit_state for `<world>/data/<MOD_ID>/` (not used: the curios live in the player data).
#[allow(dead_code)]
const MOD_ID: &str = "curios";
include!("kit_notice.rs");
include!("kit_sound.rs");

/// The forja menu kind of the Curios screen (`curios:curios_container_v2`).
const MENU_KIND: &str = "curios:menu";
/// The mod channel for what a client asks outside the menu (open, commands).
const NET: &str = "curios:net";
/// Server -> the player's own client: what they wear (`SPacketSyncCurios` to the player).
const SYNC: &str = "curios:sync";
/// `ForgeCaps` -> `curios:inventory` of the player data: the 1.20.1 layout, so a world's curios
/// keep their place (`CurioInventoryCapability.Provider`).
const CAPS: &str = "ForgeCaps";
const CAP: &str = "curios:inventory";
/// The in-frame host's key among the players (guests are their connection).
const HOST: u32 = u32::MAX;
const INVENTORY_REVAMP: &str = "curios:block/gui/inventory_revamp";
const INVENTORY_LEGACY: &str = "curios:block/gui/inventory";
/// `Curios.KeyRegistry.openCurios`: `key.curios.open.desc`, G.
const KEY_OPEN: &str = "open";
const KEY_DEFAULT: &str = "key.keyboard.g";

impl engine::Stack for ItemStack {
    fn count(&self) -> u32 {
        self.count
    }
    fn set_count(&mut self, n: u32) {
        self.count = n;
    }
}

// =================================================================================================
// The API other native mods use (Forge: CuriosApi, ICurioItem, InterModComms SlotTypeMessage)
// =================================================================================================

/// An attribute modifier a worn curio gives (`ICurio.getAttributeModifiers`). `attribute` =
/// `"curios:<slot>"` changes that slot type's size instead (`CuriosApi.addSlotModifier`).
#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) struct Modifier {
    /// `minecraft:armor`, `irons_spellbooks:max_mana`, or `curios:<slot>`.
    pub attribute: String,
    /// The modifier's name (a resource location); curios makes it `<name>/<slot>/<index>`.
    pub name: String,
    pub amount: f64,
    /// `add_value`, `add_multiplied_base` or `add_multiplied_total`.
    pub operation: &'static str,
}

pub(in crate::game) type CurioFn = for<'a, 'g, 'c, 's> fn(&'a mut Game<'g>, &'c SlotCtx, &'s ItemStack);
pub(in crate::game) type CurioChangeFn =
    for<'a, 'g, 'c, 's, 'o> fn(&'a mut Game<'g>, &'c SlotCtx, Option<&'o ItemStack>, &'s ItemStack);
pub(in crate::game) type CurioTestFn = for<'a, 'g, 'c, 's> fn(&'a Game<'g>, &'c SlotCtx, &'s ItemStack) -> bool;
pub(in crate::game) type CurioModifiersFn =
    for<'a, 'g, 'c, 's> fn(&'a Game<'g>, &'c SlotCtx, &'s ItemStack) -> Vec<Modifier>;
pub(in crate::game) type CurioLevelFn = for<'a, 'g, 'c, 's> fn(&'a Game<'g>, &'c SlotCtx, &'s ItemStack) -> i32;
pub(in crate::game) type CurioDropFn = for<'a, 'g, 'c, 's> fn(&'a Game<'g>, &'c SlotCtx, &'s ItemStack) -> DropRule;

/// A curio item's behaviour (`ICurioItem` / `ICurio`). Every callback runs as the wearer: on the
/// server inside the guest's turn (`as_guest`), or for the in-frame host. Unset = the default of
/// the original.
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub(in crate::game) struct CurioItem {
    /// The item id (`ars_nouveau:ring_of_potential`), or `#tag` for every item of an item tag.
    pub item: &'static str,
    /// `curioTick`: every tick while worn (not for cosmetic slots).
    pub tick: Option<CurioFn>,
    /// `onEquip(slotContext, prevStack)`: (ctx, the stack that was there, the new stack).
    pub equip: Option<CurioChangeFn>,
    /// `onUnequip(slotContext, newStack)`: (ctx, the stack now there, the old stack).
    pub unequip: Option<CurioChangeFn>,
    /// `onEquipFromUse`: put in by the player (menu click or right click).
    pub equip_from_use: Option<CurioFn>,
    /// `canEquip` (default true).
    pub can_equip: Option<CurioTestFn>,
    /// `canUnequip` (default true).
    pub can_unequip: Option<CurioTestFn>,
    /// `canEquipFromUse` (default false: right click does nothing).
    pub can_equip_from_use: Option<CurioTestFn>,
    /// `getAttributeModifiers(slotContext, uuid)`.
    pub modifiers: Option<CurioModifiersFn>,
    /// `getDropRule` (default `DEFAULT`).
    pub drop_rule: Option<CurioDropFn>,
    /// `getFortuneLevel` / `getLootingLevel` (default 0).
    pub fortune: Option<CurioLevelFn>,
    pub looting: Option<CurioLevelFn>,
    /// `makesPiglinsNeutral` (default: a piglin-safe armour item). `isEnderMask` and
    /// `canWalkOnPowderedSnow` are kept for the consumers but not wired yet (listed in its README);
    /// worn freeze immunity is the `freeze_immune_wearables` tag.
    pub piglin_neutral: Option<CurioTestFn>,
    pub ender_mask: Option<CurioTestFn>,
    pub powder_snow: Option<CurioTestFn>,
}
#[allow(dead_code)]
impl CurioItem {
    pub const fn new(item: &'static str) -> Self {
        CurioItem {
            item,
            tick: None,
            equip: None,
            unequip: None,
            equip_from_use: None,
            can_equip: None,
            can_unequip: None,
            can_equip_from_use: None,
            modifiers: None,
            drop_rule: None,
            fortune: None,
            looting: None,
            piglin_neutral: None,
            ender_mask: None,
            powder_snow: None,
        }
    }
}

struct ApiState {
    items: Vec<CurioItem>,
    /// (sender mod, message, modify) of the legacy registrations.
    legacy: Vec<(String, LegacyMessage, bool)>,
    /// Validators other mods add (`CuriosApi.registerCurioPredicate`).
    predicates: BTreeMap<String, CurioTestFn>,
}
static API: Mutex<ApiState> = Mutex::new(ApiState { items: Vec::new(), legacy: Vec::new(), predicates: BTreeMap::new() });
static API_GEN: AtomicU64 = AtomicU64::new(1);
fn api<R>(f: impl FnOnce(&mut ApiState) -> R) -> R {
    let mut g = API.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut g)
}

/// Registers (or replaces) a curio item's behaviour. Idempotent: call it from your load hook.
#[allow(dead_code)]
pub(in crate::game) fn register_item(item: CurioItem) {
    api(|a| {
        a.items.retain(|x| x.item != item.item);
        a.items.push(item);
    });
}
/// `InterModComms.sendTo("curios", "register_type", SlotTypeMessage)`: a slot type for players.
#[allow(dead_code)]
pub(in crate::game) fn register_slot_type(sender: &str, msg: LegacyMessage) {
    api(|a| {
        if !a.legacy.iter().any(|(s, m, modify)| s == sender && *m == msg && !modify) {
            a.legacy.push((sender.to_owned(), msg, false));
            API_GEN.fetch_add(1, Ordering::Relaxed);
        }
    });
}
/// `modify_type`: changes a slot type that exists.
#[allow(dead_code)]
pub(in crate::game) fn modify_slot_type(sender: &str, msg: LegacyMessage) {
    api(|a| {
        if !a.legacy.iter().any(|(s, m, modify)| s == sender && *m == msg && *modify) {
            a.legacy.push((sender.to_owned(), msg, true));
            API_GEN.fetch_add(1, Ordering::Relaxed);
        }
    });
}
/// `CuriosApi.registerCurioPredicate(id, predicate)`: a validator slot types can name.
#[allow(dead_code)]
pub(in crate::game) fn register_predicate(id: &str, f: CurioTestFn) {
    api(|a| {
        a.predicates.insert(id.to_owned(), f);
    });
}
fn curio_of(game: &Game<'_>, s: &ItemStack) -> Option<CurioItem> {
    let items = game.res.items();
    api(|a| {
        a.items.iter().find(|c| *c.item == *s.item).or_else(|| {
            a.items.iter().find(|c| {
                c.item.strip_prefix('#').is_some_and(|t| items.item_tags.get(t).is_some_and(|m| m.contains(&*s.item)))
            })
        }).copied()
    })
}

/// The player's slot types (`CuriosApi.getPlayerSlots`), in order.
#[allow(dead_code)]
pub(in crate::game) fn player_slots(game: &Game<'_>) -> Vec<SlotType> {
    registry(game).player_slots()
}
/// Every slot type (`CuriosApi.getSlots`).
#[allow(dead_code)]
pub(in crate::game) fn all_slots(game: &Game<'_>) -> Vec<SlotType> {
    registry(game).slots.values().cloned().collect()
}
/// The stacks of slot type `slot` of the player in context (the in-frame host, or the guest whose
/// turn it is): `getStacksHandler(id).getStacks()`.
#[allow(dead_code)]
pub(in crate::game) fn stacks(game: &Game<'_>, slot: &str) -> Vec<Option<ItemStack>> {
    let inv = game.curios_read();
    inv.handler(slot).map(|h| h.stacks.clone()).unwrap_or_default()
}
/// Every worn stack of the player in context (cosmetic ones included). On a server's client (the
/// single player window) it is the copy the server last sent for this player.
#[allow(dead_code)]
pub(in crate::game) fn worn(game: &Game<'_>) -> Vec<(SlotCtx, ItemStack)> {
    if game.remote.is_some() {
        return CLIENT_WORN.lock().unwrap_or_else(|e| e.into_inner()).clone();
    }
    let inv = game.curios_read();
    inv.worn().into_iter().map(|(hi, i, c, s)| (inv.ctx(hi, i, c), s.clone())).collect()
}
/// `findCurios(predicate)`: every worn stack (cosmetic ones included) that passes.
#[allow(dead_code)]
pub(in crate::game) fn find(game: &Game<'_>, pred: impl Fn(&ItemStack) -> bool) -> Vec<(SlotCtx, ItemStack)> {
    worn(game).into_iter().filter(|(_, s)| pred(s)).collect()
}
/// `ICuriosHelper.getCurioTags(item)`: the player's slot types a stack fits (the tooltip's list).
#[allow(dead_code)]
pub(in crate::game) fn item_slots(game: &Game<'_>, stack: &ItemStack) -> Vec<String> {
    let reg = registry(game);
    engine::tooltip_slots(&GameItems { game }, &reg.player_slots(), stack)
}
/// The window's copy of its player's worn curios (from `curios:sync`).
static CLIENT_WORN: Mutex<Vec<(SlotCtx, ItemStack)>> = Mutex::new(Vec::new());

fn sync_bytes(list: &[(SlotCtx, ItemStack)]) -> Vec<u8> {
    use horizon_world::nbt::{Limits, NamedTag, write_named};
    let mut out = Vec::new();
    out.extend_from_slice(&(list.len() as u32).to_le_bytes());
    for (ctx, s) in list {
        let nbt = write_named(&NamedTag { name: JavaString::from(""), tag: horizon_client::save::item_to_nbt(s) }, Limits::default()).unwrap_or_default();
        out.extend_from_slice(&(ctx.id.len() as u32).to_le_bytes());
        out.extend_from_slice(ctx.id.as_bytes());
        out.extend_from_slice(&(ctx.index as u32).to_le_bytes());
        out.push(u8::from(ctx.cosmetic) | (u8::from(ctx.visible) << 1));
        out.extend_from_slice(&(nbt.len() as u32).to_le_bytes());
        out.extend_from_slice(&nbt);
    }
    out
}
fn sync_list(b: &[u8]) -> Option<Vec<(SlotCtx, ItemStack)>> {
    use horizon_world::nbt::{Limits, read_named};
    let mut at = 0usize;
    let mut take = |n: usize| -> Option<&[u8]> {
        let r = b.get(at..at.checked_add(n)?)?;
        at += n;
        Some(r)
    };
    let u32_of = |x: &[u8]| u32::from_le_bytes([x[0], x[1], x[2], x[3]]) as usize;
    let count = u32_of(take(4)?);
    let mut out = Vec::new();
    for _ in 0..count.min(4096) {
        let n = u32_of(take(4)?);
        let id = String::from_utf8(take(n)?.to_vec()).ok()?;
        let index = u32_of(take(4)?);
        let flags = take(1)?[0];
        let m = u32_of(take(4)?);
        let (named, _) = read_named(take(m)?, Limits::default()).ok()?;
        let stack = horizon_client::save::item_from_nbt(&named.tag)?;
        out.push((SlotCtx { id, index, cosmetic: flags & 1 != 0, visible: flags & 2 != 0 }, stack));
    }
    Some(out)
}
/// `findFirstCurio(item)` among the main (not cosmetic) slots.
#[allow(dead_code)]
pub(in crate::game) fn find_first(game: &Game<'_>, pred: impl Fn(&ItemStack) -> bool) -> Option<(SlotCtx, ItemStack)> {
    find(game, pred).into_iter().find(|(c, _)| !c.cosmetic)
}
/// `isEquipped(item)`.
#[allow(dead_code)]
pub(in crate::game) fn is_equipped(game: &Game<'_>, item: &str) -> bool {
    find_first(game, |s| &*s.item == item).is_some()
}
/// `setEquippedCurio(id, index, stack)` (or the cosmetic one): false when the slot does not exist.
#[allow(dead_code)]
pub(in crate::game) fn set_stack(game: &mut Game<'_>, ctx: &SlotCtx, stack: Option<ItemStack>) -> bool {
    game.curios_edit(|_, inv| {
        let Some(h) = inv.handler_mut(&ctx.id) else { return false };
        let list = if ctx.cosmetic { &mut h.cosmetics } else { &mut h.stacks };
        match list.get_mut(ctx.index) {
            Some(cell) => {
                *cell = stack;
                true
            }
            None => false,
        }
    })
}
/// `ICuriosItemHandler.getFortuneLevel` / `getLootingLevel`: the worn curios' sum.
#[allow(dead_code)]
pub(in crate::game) fn fortune_level(game: &Game<'_>) -> i32 {
    game.curios_level(|c| c.fortune)
}
#[allow(dead_code)]
pub(in crate::game) fn looting_level(game: &Game<'_>) -> i32 {
    game.curios_level(|c| c.looting)
}

// =================================================================================================
// Items as the rules see them
// =================================================================================================

struct GameItems<'a, 'g> {
    game: &'a Game<'g>,
}
impl engine::Items<ItemStack> for GameItems<'_, '_> {
    fn max_stack(&self, s: &ItemStack) -> u32 {
        self.game.res.items().get(&s.item).map_or(64, |i| i.max_stack)
    }
    fn same_item(&self, a: &ItemStack, b: &ItemStack) -> bool {
        a.item == b.item
    }
    fn same_components(&self, a: &ItemStack, b: &ItemStack) -> bool {
        a.same_item_same_components(b)
    }
    fn stackable(&self, s: &ItemStack) -> bool {
        let info = self.game.res.items().get(&s.item);
        let max = info.map_or(64, |i| i.max_stack);
        let damaged = info.is_some_and(|i| i.max_damage > 0) && s.damage > 0;
        max > 1 && !damaged
    }
    fn in_tag(&self, s: &ItemStack, tag: &str) -> bool {
        self.game.res.items().item_tags.get(tag).is_some_and(|t| t.contains(&*s.item))
    }
    fn has_curios_tag(&self, s: &ItemStack) -> bool {
        self.game.res.items().item_tags.iter().any(|(t, m)| t.starts_with("curios:") && m.contains(&*s.item))
    }
    fn armor_slot(&self, s: &ItemStack) -> Option<usize> {
        let i = self.game.res.items().get(&s.item)?;
        let slot = match i.equip_slot.as_deref()? {
            "head" => 0,
            "chest" => 1,
            "legs" => 2,
            "feet" => 3,
            _ => return None,
        };
        i.equip_allowed.as_ref().is_none_or(|a| a.contains("minecraft:player")).then_some(slot)
    }
    fn offhand_item(&self, s: &ItemStack) -> bool {
        self.game.res.items().get(&s.item).is_some_and(|i| i.equip_slot.as_deref() == Some("offhand")) || &*s.item == "minecraft:shield"
    }
    fn binding_curse(&self, s: &ItemStack) -> bool {
        s.enchantment_level("minecraft:binding_curse") > 0
    }
    fn vanishing_curse(&self, s: &ItemStack) -> bool {
        s.enchantment_level("minecraft:vanishing_curse") > 0
    }
    fn is_curio(&self, s: &ItemStack) -> bool {
        curio_of(self.game, s).is_some()
    }
    fn can_equip(&self, ctx: &SlotCtx, s: &ItemStack) -> bool {
        curio_of(self.game, s).and_then(|c| c.can_equip).is_none_or(|f| f(self.game, ctx, s))
    }
    fn can_unequip(&self, ctx: &SlotCtx, s: &ItemStack) -> bool {
        curio_of(self.game, s).and_then(|c| c.can_unequip).is_none_or(|f| f(self.game, ctx, s))
    }
    fn predicate(&self, validator: &str, ctx: &SlotCtx, s: &ItemStack) -> Option<bool> {
        let f = api(|a| a.predicates.get(validator).copied())?;
        Some(f(self.game, ctx, s))
    }
}

// =================================================================================================
// The slot types (data of the resource root + legacy registrations + the pack's config)
// =================================================================================================

static REGISTRY: Mutex<Option<(std::path::PathBuf, u64, Arc<engine::Registry>)>> = Mutex::new(None);

fn data_files(root: &std::path::Path, kind: &str) -> Vec<engine::DataFile> {
    let mut out = Vec::new();
    let Ok(namespaces) = std::fs::read_dir(root.join("data")) else { return out };
    for ns in namespaces.flatten() {
        let namespace = ns.file_name().to_string_lossy().into_owned();
        let base = ns.path().join("curios").join(kind);
        let mut stack = vec![base.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "json") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&p) else { continue };
                let rel = p.strip_prefix(&base).unwrap_or(&p).with_extension("");
                let path = rel.to_string_lossy().replace('\\', "/");
                out.push(engine::DataFile { namespace: namespace.clone(), path, text });
            }
        }
    }
    out
}

/// The slot types, read once per resource root and again when a mod registers a legacy one.
fn registry(game: &Game<'_>) -> Arc<engine::Registry> {
    let root = game.options.resources.clone();
    let generation = API_GEN.load(Ordering::Relaxed);
    let mut g = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((r, gen_, reg)) = g.as_ref()
        && *r == root
        && *gen_ == generation
    {
        return reg.clone();
    }
    let (mut register, modify) = api(|a| {
        let r: Vec<(String, LegacyMessage)> = a.legacy.iter().filter(|x| !x.2).map(|x| (x.0.clone(), x.1.clone())).collect();
        let m: Vec<(String, LegacyMessage)> = a.legacy.iter().filter(|x| x.2).map(|x| (x.0.clone(), x.1.clone())).collect();
        (r, m)
    });
    // The legacy registrations of the pack's mods whose content is installed (their items are in
    // the catalog), unless their own native port already sent them.
    let items = game.res.items();
    for sender in engine::KNOWN_LEGACY_SENDERS {
        let prefix = format!("{sender}:");
        if register.iter().any(|(s, _)| s == sender) || !items.order.iter().any(|i| i.starts_with(&prefix)) {
            continue;
        }
        register.extend(engine::known_legacy(sender).into_iter().map(|m| (sender.to_owned(), m)));
    }
    let src = engine::Sources {
        slots: data_files(&root, "slots"),
        entities: data_files(&root, "entities"),
        legacy_register: register,
        legacy_modify: modify,
        config: engine::config::SLOTS.iter().map(|s| s.to_string()).collect(),
    };
    let reg = Arc::new(engine::build_registry(&src));
    for e in &reg.errors {
        log::warn!("Curios: {e}");
    }
    *g = Some((root, generation, reg.clone()));
    reg
}

// =================================================================================================
// The player's curios in the player data (ForgeCaps.curios:inventory, the 1.20.1 layout)
// =================================================================================================

fn jstr(s: &str) -> Tag {
    Tag::String(JavaString::from(s))
}
fn get_bool(t: &Tag, k: &str) -> Option<bool> {
    horizon_client::save::number(t.get(k)).map(|v| v != 0)
}
fn get_int(t: &Tag, k: &str) -> Option<i32> {
    horizon_client::save::number(t.get(k)).map(|v| v as i32)
}
/// `ItemStackHandler.deserializeNBT`: `Size` slots, the items by `Slot`.
fn read_handler_items(t: &Tag) -> Vec<Option<ItemStack>> {
    let size = get_int(t, "Size").unwrap_or(0).clamp(0, 4096) as usize;
    let mut out = vec![None; size];
    if let Some(Tag::List(items)) = t.get("Items") {
        for it in items {
            let slot = get_int(it, "Slot").unwrap_or(-1);
            if slot >= 0
                && let Some(cell) = out.get_mut(slot as usize)
            {
                *cell = horizon_client::save::item_from_nbt(it);
            }
        }
    }
    out
}
/// `ItemStackHandler.serializeNBT`.
fn write_handler_items(items: &[Option<ItemStack>]) -> Tag {
    let list: Vec<Tag> = items
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let s = s.as_ref()?;
            let mut t = horizon_client::save::item_to_nbt(s);
            if let Tag::Compound(m) = &mut t {
                m.insert(JavaString::from("Slot"), Tag::Int(i as i32));
            }
            Some(t)
        })
        .collect();
    Tag::compound([("Items", Tag::List(list)), ("Size", Tag::Int(items.len() as i32))])
}
fn read_modifier(t: &Tag) -> Option<SlotModifier> {
    let name = horizon_client::save::text(t.get("Name")).unwrap_or_default();
    let amount = horizon_client::save::float(t.get("Amount"))?;
    let operation = get_int(t, "Operation").unwrap_or(0).clamp(0, 2) as u8;
    let uuid = match t.get("UUID") {
        Some(Tag::IntArray(v)) if v.len() == 4 => Some([v[0], v[1], v[2], v[3]]),
        _ => None,
    };
    let id = match uuid {
        Some(u) if u == engine::LEGACY_UUID => engine::LEGACY_ID.to_owned(),
        Some(u) => format!("u:{:08x}{:08x}{:08x}{:08x}", u[0], u[1], u[2], u[3]),
        None => name.clone(),
    };
    Some(SlotModifier { id, name, amount, operation, uuid })
}
fn write_modifier(m: &SlotModifier) -> Tag {
    let mut e = vec![
        ("Name", jstr(&m.name)),
        ("Amount", Tag::Double(m.amount.to_bits())),
        ("Operation", Tag::Int(i32::from(m.operation))),
    ];
    if let Some(u) = m.uuid {
        e.push(("UUID", Tag::IntArray(u.to_vec())));
    }
    Tag::compound(e)
}
fn read_saved(tag: Option<&Tag>) -> Vec<engine::Saved<ItemStack>> {
    let Some(Tag::List(list)) = tag.and_then(|t| t.get(CAPS)).and_then(|c| c.get(CAP)).and_then(|c| c.get("Curios")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in list {
        let id = horizon_client::save::text(entry.get("Identifier")).unwrap_or_default();
        let Some(h) = entry.get("StacksHandler") else { continue };
        let renders = h.get("Renders").map(|r| {
            let size = get_int(r, "Size").map(|s| s.max(0) as usize);
            let pairs = match r.get("Renders") {
                Some(Tag::List(l)) => l
                    .iter()
                    .filter_map(|x| Some((get_int(x, "Slot")?.max(0) as usize, get_bool(x, "Render").unwrap_or(true))))
                    .collect(),
                _ => Vec::new(),
            };
            (size, pairs)
        });
        let mods = |k: &str| match h.get(k) {
            Some(Tag::List(l)) => l.iter().filter_map(read_modifier).collect(),
            _ => Vec::new(),
        };
        out.push(engine::Saved {
            id,
            base_size: get_int(h, "SavedBaseSize"),
            stacks: h.get("Stacks").map(read_handler_items),
            cosmetics: h.get("Cosmetics").map(read_handler_items),
            renders,
            size_shift: get_int(h, "SizeShift"),
            cosmetic: get_bool(h, "HasCosmetic"),
            visible: get_bool(h, "Visible"),
            render_toggle: get_bool(h, "RenderToggle"),
            drop_rule: horizon_client::save::text(h.get("DropRule")),
            persistent: mods("PersistentModifiers"),
            cached: mods("CachedModifiers"),
        });
    }
    out
}
/// `CurioInventoryWrapper.writeTag` into the player data, the other ForgeCaps kept.
fn write_saved(extra: &mut Option<Tag>, inv: &engine::Inventory<ItemStack>) {
    let list: Vec<Tag> = inv
        .handlers
        .iter()
        .map(|h| {
            let renders: Vec<Tag> = h
                .renders
                .iter()
                .enumerate()
                .map(|(i, r)| Tag::compound([("Slot", Tag::Int(i as i32)), ("Render", Tag::Byte(i8::from(*r)))]))
                .collect();
            let mut e = vec![
                ("SavedBaseSize", Tag::Int(h.base_size)),
                ("Stacks", write_handler_items(&h.stacks)),
                ("Cosmetics", write_handler_items(&h.cosmetics)),
                ("Renders", Tag::compound([("Renders", Tag::List(renders)), ("Size", Tag::Int(h.renders.len() as i32))])),
                ("HasCosmetic", Tag::Byte(i8::from(h.cosmetic))),
                ("Visible", Tag::Byte(i8::from(h.visible))),
                ("RenderToggle", Tag::Byte(i8::from(h.render_toggle))),
                ("DropRule", jstr(h.drop_rule.name())),
            ];
            if !h.persistent.is_empty() {
                e.push(("PersistentModifiers", Tag::List(h.persistent.iter().map(write_modifier).collect())));
            }
            if !h.transient.is_empty() {
                e.push(("CachedModifiers", Tag::List(h.transient.iter().map(write_modifier).collect())));
            }
            Tag::compound([("StacksHandler", Tag::compound(e)), ("Identifier", jstr(&h.id))])
        })
        .collect();
    let root = extra.get_or_insert_with(|| Tag::Compound(Default::default()));
    let Tag::Compound(map) = root else { return };
    let caps = map.entry(JavaString::from(CAPS)).or_insert_with(|| Tag::Compound(Default::default()));
    if !matches!(caps, Tag::Compound(_)) {
        *caps = Tag::Compound(Default::default());
    }
    if let Tag::Compound(c) = caps {
        c.insert(JavaString::from(CAP), Tag::compound([("Curios", Tag::List(list))]));
    }
}
/// The curios of the player data `extra` over the current slot types, and what no longer fits.
fn load_inventory(game: &Game<'_>, extra: Option<&Tag>) -> (engine::Inventory<ItemStack>, Vec<ItemStack>) {
    let reg = registry(game);
    let slots = reg.player_slots();
    let all: Vec<SlotType> = reg.slots.values().cloned().collect();
    let items = GameItems { game };
    let saved = read_saved(extra);
    engine::Inventory::load(&slots, saved, |ctx, s| engine::may_equip(&items, &slots, &all, ctx, s))
}

// =================================================================================================
// Per-player runtime state (not saved, as in the original)
// =================================================================================================

/// The open Curios menu of one player (server side, or the in-frame host).
struct MenuRt {
    pos: [i32; 3],
    page: i32,
    cosmetics: bool,
    quick: engine::QuickCraft,
    view: engine::View,
    /// The mirror: the page's curio slots, the armour (head..feet), the off hand.
    snapshot: Vec<Option<ItemStack>>,
}
#[derive(Default)]
struct PlayerRt {
    previous: engine::Previous<ItemStack>,
    /// (attribute, id) -> (amount, operation) applied to the body.
    applied: BTreeMap<(String, String), (f64, &'static str)>,
    menu: Option<MenuRt>,
    /// The client got this player's worn curios since they joined.
    synced: bool,
}
static PLAYERS: Mutex<Option<HashMap<u32, PlayerRt>>> = Mutex::new(None);
fn players<R>(f: impl FnOnce(&mut HashMap<u32, PlayerRt>) -> R) -> R {
    let mut g = PLAYERS.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(HashMap::new))
}
/// A menu's mirror: whose it is and the block state that stood at its position when it was put
/// there (a block placed there since makes the block entity someone else's: never touched).
#[derive(Clone, Copy, Debug)]
struct Mirror {
    owner: u32,
    state: u32,
}
/// The mirrors, by (dimension, position): every dimension's server shares this table.
static MIRRORS: Mutex<BTreeMap<(&'static str, [i32; 3]), Mirror>> = Mutex::new(BTreeMap::new());
fn mirror_at(game: &Game<'_>, p: [i32; 3]) -> Option<Mirror> {
    MIRRORS.lock().unwrap_or_else(|e| e.into_inner()).get(&(game.dim.id(), p)).copied()
}
/// The mirror of `owner` at `p`, the block under it unchanged since.
fn own_mirror(game: &Game<'_>, owner: u32, p: [i32; 3]) -> bool {
    mirror_at(game, p).is_some_and(|m| m.owner == owner && game.world.block(p[0], p[1], p[2]).0 == m.state)
}
fn mirrors_here(game: &Game<'_>) -> Vec<([i32; 3], Mirror)> {
    let dim = game.dim.id();
    MIRRORS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .filter(|((d, _), _)| *d == dim)
        .map(|((_, p), m)| (*p, *m))
        .collect()
}
/// The empty block entity a guest's mirror position holds between turns (no slots: hoppers, chunk
/// saves and spills find nothing in it).
fn placeholder() -> BlockEntity {
    BlockEntity::DaylightDetector
}
static REGISTERED: AtomicBool = AtomicBool::new(false);
fn curios_register() {
    if REGISTERED.swap(true, Ordering::Relaxed) {
        return;
    }
    super::forja::register_server_channel(NET, curios_net_server);
    super::forja::register_menu(super::forja::MenuKind {
        enter: Some(curios_menu_enter),
        leave: Some(curios_menu_leave),
        sections: Some(curios_menu_sections),
        input: Some(curios_menu_input),
        closed: Some(curios_menu_closed),
        valid: Some(curios_menu_valid),
        ..super::forja::MenuKind::new(MENU_KIND)
    });
}

// ---- forja menu kind ----

fn curios_menu_enter(game: &mut Game<'_>, conn: u32, p: [i32; 3]) {
    if !own_mirror(game, conn, p) {
        return;
    }
    let key = (p[0], p[1], p[2]);
    if !matches!(game.world.block_entities.get(&key), Some(BlockEntity::DaylightDetector)) {
        return;
    }
    let snapshot = players(|m| m.get(&conn).and_then(|r| r.menu.as_ref()).map(|mm| mm.snapshot.clone()));
    if let Some(s) = snapshot {
        game.world.block_entities.insert(key, BlockEntity::Chest(s));
    }
}
fn curios_menu_leave(game: &mut Game<'_>, conn: u32, p: [i32; 3]) {
    if !own_mirror(game, conn, p) {
        return;
    }
    let key = (p[0], p[1], p[2]);
    if matches!(game.world.block_entities.get(&key), Some(BlockEntity::Chest(_))) {
        game.world.block_entities.insert(key, placeholder());
    }
}
fn curios_menu_sections(_game: &mut Game<'_>, conn: u32, _p: [i32; 3]) -> Vec<Vec<u8>> {
    players(|m| m.get(&conn).and_then(|r| r.menu.as_ref()).map(|mm| vec![mm.view.encode()])).unwrap_or_default()
}
fn curios_menu_input(game: &mut Game<'_>, _conn: u32, _p: [i32; 3], input: &super::forja::MenuInput) -> bool {
    let super::forja::MenuInput::Button { id, arg } = input else { return false };
    game.curios_button(*id, *arg)
}
fn curios_menu_valid(game: &Game<'_>, conn: u32, p: [i32; 3]) -> bool {
    own_mirror(game, conn, p)
        && matches!(
            game.world.block_entities.get(&(p[0], p[1], p[2])),
            Some(BlockEntity::DaylightDetector | BlockEntity::Chest(_))
        )
}
fn curios_menu_closed(game: &mut Game<'_>, conn: u32, p: [i32; 3]) {
    game.curios_drop_mirror(conn, p);
}
/// A client's message outside the menu: 0 = open the Curios menu, 1 + text = a `/curios` command.
fn curios_net_server(game: &mut Game<'_>, host: &mut LanHost, conn: u32, body: &[u8]) {
    match body.first() {
        Some(0) => game.curios_open_guest(host, conn),
        Some(1) => {
            let cmd = String::from_utf8_lossy(&body[1..]).into_owned();
            let level = host.hub.guest(conn).map_or(0, |g| g.permission);
            game.as_guest(host, conn, |g| {
                if let Some(r) = engine::parse_command(&cmd) {
                    g.curios_command(r, i32::from(level));
                }
            });
        }
        _ => {}
    }
}

// The client's copy of the screen (decoded once per generation of the forja sections).
thread_local! {
    static CLIENT_VIEW: std::cell::RefCell<Option<(u64, Option<engine::View>)>> = const { std::cell::RefCell::new(None) };
}

fn input_click(i: horizon_client::menu::Input) -> engine::Click {
    use horizon_client::menu::Input;
    match i {
        Input::Pickup => engine::Click::Pickup,
        Input::QuickMove => engine::Click::QuickMove,
        Input::Swap => engine::Click::Swap,
        Input::Clone => engine::Click::Clone,
        Input::Throw => engine::Click::Throw,
        Input::QuickCraft => engine::Click::QuickCraft,
        Input::PickupAll => engine::Click::PickupAll,
    }
}

impl Game<'_> {
    /// The player in context (`lan_acting` inside a guest's turn, else the in-frame host).
    fn curios_key(&self) -> u32 {
        self.lan_acting.unwrap_or(HOST)
    }
    /// The player in context's curios, read only (stacks that no longer fit stay in the data until
    /// the next tick gives them back).
    fn curios_read(&self) -> engine::Inventory<ItemStack> {
        load_inventory(self, self.level.player_extra.as_ref()).0
    }
    /// Change the player in context's curios: read, give back what no longer fits, run `f`, write.
    fn curios_edit<R>(&mut self, f: impl FnOnce(&mut Game<'_>, &mut engine::Inventory<ItemStack>) -> R) -> R {
        let (mut inv, lost) = load_inventory(self, self.level.player_extra.as_ref());
        let r = f(self, &mut inv);
        write_saved(&mut self.level.player_extra, &inv);
        for s in lost {
            self.curios_give(s);
        }
        r
    }
    /// `ItemHandlerHelper.giveItemToPlayer`: into the inventory, else dropped at the feet.
    fn curios_give(&mut self, stack: ItemStack) {
        if let Some(rest) = self.give(stack)
            && let Some(eye) = self.eye_position()
        {
            self.throw_from_eyes(rest, eye, false);
        }
    }
    /// Players get at least one slot type (else a player without curios data has nothing to do).
    fn curios_has_slots(&self) -> bool {
        !registry(self).player_slots().is_empty()
    }
    fn curios_level(&self, f: impl Fn(&CurioItem) -> Option<CurioLevelFn>) -> i32 {
        let inv = self.curios_read();
        let mut sum = 0;
        for (hi, i, cosmetic, s) in inv.worn() {
            if cosmetic {
                continue;
            }
            if let Some(g) = curio_of(self, s).and_then(|c| f(&c)) {
                sum += g(self, &inv.ctx(hi, i, false), s);
            }
        }
        sum
    }

    // ---- tick (CuriosEventHandler.tick) ----

    /// HEAD `tick_blocks`: the in-frame host's curios.
    pub(super) fn curios_host_tick(&mut self) {
        if self.remote.is_some() || self.player.is_none() || self.lan_acting.is_some() {
            return;
        }
        self.curios_restore_mirrors();
        self.curios_player_tick();
        self.curios_refresh_menu();
    }
    /// HEAD `lan_living_flags` (every tick_lan pass, paused or not, the hub in hand, before forja
    /// checks its menus): the mod channel and menu kind, the mirrors a save took out, who left.
    pub(super) fn curios_guests_pass(&mut self, host: &mut LanHost) {
        if self.remote.is_some() {
            return;
        }
        curios_register();
        // Who left: their runtime state goes (a new connection starts fresh, like a new login).
        let all: Vec<u32> = host.hub.guests.iter().map(|g| g.conn).collect();
        players(|m| m.retain(|k, r| *k == HOST || all.contains(k) || r.menu.is_some()));
        self.curios_restore_mirrors();
    }
    /// HEAD `tick_guests` (the guests' tick, not while single player is paused): each guest's
    /// curios, as the guest (single player's window is one).
    pub(super) fn curios_guests_tick(&mut self, host: &mut LanHost) {
        if self.remote.is_some() {
            return;
        }
        curios_register();
        let conns: Vec<u32> = host
            .hub
            .guests
            .iter()
            .filter(|g| host.hub.here(g) && g.placed && !g.dead)
            .map(|g| g.conn)
            .collect();
        for conn in conns {
            let Some(g) = host.hub.guest(conn) else { continue };
            // No curios data, nothing worn last tick and no menu: nothing to do this tick.
            let has_data = g.saved.as_ref().and_then(|t| t.get(CAPS)).and_then(|c| c.get(CAP)).is_some();
            let busy = players(|m| m.get(&conn).is_some_and(|r| !r.previous.is_empty() || !r.applied.is_empty() || r.menu.is_some()));
            if !has_data && !busy && !self.curios_has_slots() {
                continue;
            }
            self.as_guest(host, conn, |g| {
                g.curios_player_tick();
                g.curios_refresh_menu();
            });
        }
    }
    /// One tick of the player in context's curios: slot modifiers, equip / unequip, attribute
    /// modifiers, `curioTick`.
    fn curios_player_tick(&mut self) {
        let key = self.curios_key();
        let reg = registry(self);
        if reg.player_slots().is_empty() && self.level.player_extra.as_ref().and_then(|t| t.get(CAPS)).and_then(|c| c.get(CAP)).is_none() {
            return;
        }
        let mut changes = Vec::new();
        let mut wanted_attr: BTreeMap<(String, String), (f64, &'static str)> = BTreeMap::new();
        let mut ticks: Vec<(SlotCtx, ItemStack, CurioFn)> = Vec::new();
        self.curios_edit(|game, inv| {
            // The slot modifiers the worn curios give (handleInvalidStacks gives back what is lost).
            let mut slot_mods: BTreeMap<String, Vec<SlotModifier>> = BTreeMap::new();
            for (hi, i, cosmetic, s) in inv.worn() {
                if cosmetic {
                    continue;
                }
                let ctx = inv.ctx(hi, i, false);
                for m in curios_modifiers(game, &ctx, s) {
                    let id = format!("{}/{}/{}", m.name, ctx.id, ctx.index);
                    if let Some(target) = m.attribute.strip_prefix("curios:") {
                        let op = match m.operation {
                            "add_multiplied_base" => 1,
                            "add_multiplied_total" => 2,
                            _ => 0,
                        };
                        slot_mods.entry(target.to_owned()).or_default().push(SlotModifier {
                            id: id.clone(),
                            name: id,
                            amount: m.amount,
                            operation: op,
                            uuid: None,
                        });
                    } else {
                        wanted_attr.insert((m.attribute.clone(), id), (m.amount, m.operation));
                    }
                }
            }
            let (_, lost) = inv.set_transient(&slot_mods);
            for s in lost {
                game.curios_give(s);
            }
            let items = GameItems { game: &*game };
            let mut previous = players(|m| std::mem::take(&mut m.entry(key).or_default().previous));
            changes = engine::changes(inv, &mut previous, &items);
            players(|m| m.entry(key).or_default().previous = previous);
            for (hi, i, cosmetic, s) in inv.worn() {
                if cosmetic {
                    continue;
                }
                if let Some(f) = curio_of(game, s).and_then(|c| c.tick) {
                    ticks.push((inv.ctx(hi, i, false), s.clone(), f));
                }
            }
        });
        // Equip / unequip (main slots), in the original's order: the old one's unequip, then the new one's equip.
        for c in &changes {
            if c.ctx.cosmetic {
                continue;
            }
            if let Some(old) = &c.old
                && let Some(f) = curio_of(self, old).and_then(|x| x.unequip)
            {
                f(self, &c.ctx, c.new.as_ref(), old);
            }
            if let Some(new) = &c.new
                && let Some(f) = curio_of(self, new).and_then(|x| x.equip)
            {
                f(self, &c.ctx, c.old.as_ref(), new);
            }
        }
        // The attribute modifiers on the body: removed, added, or put back on a new body.
        let applied = players(|m| std::mem::take(&mut m.entry(key).or_default().applied));
        if let Some(body) = self.player.as_mut() {
            for ((attr, id), _) in applied.iter() {
                if !wanted_attr.contains_key(&(attr.clone(), id.clone())) {
                    let _ = body.set_modifier(attr, id, 0.0, "add_value", false);
                }
            }
            for ((attr, id), (amount, op)) in &wanted_attr {
                let same = applied.get(&(attr.clone(), id.clone())) == Some(&(*amount, *op));
                let present = body
                    .attributes
                    .key(attr)
                    .and_then(|k| body.attributes.has_modifier(Some(k), id).ok())
                    .unwrap_or(false);
                if !same || !present {
                    let _ = body.set_modifier(attr, id, *amount, op, true);
                }
            }
        }
        players(|m| m.entry(key).or_default().applied = wanted_attr);
        for (ctx, s, f) in ticks {
            f(self, &ctx, &s);
        }
        // SPacketSyncCurios to the player itself: what they wear, when it changed (and once at first).
        if key != HOST {
            let first = players(|m| !std::mem::replace(&mut m.entry(key).or_default().synced, true));
            if first || !changes.is_empty() {
                let list = worn(self);
                super::forja::send_to_acting(self, SYNC, &sync_bytes(&list));
            }
        }
    }
    /// HEAD `native_tick` (every Game, the window too): a server's client reads what its player
    /// wears.
    pub(super) fn curios_client_tick(&mut self) {
        if self.remote.is_none() {
            return;
        }
        for body in super::forja::client_take(SYNC) {
            if let Some(list) = sync_list(&body) {
                *CLIENT_WORN.lock().unwrap_or_else(|e| e.into_inner()) = list;
            }
        }
    }

    // ---- death (CuriosEventHandler.playerDrops) ----

    /// HEAD `drop_inventory(at)`: `Player.dropEquipment` of the player who died (host or guest, as
    /// the guest): the curios follow keepInventory / keepCurios and the drop rules.
    pub(super) fn curios_death(&mut self, at: [f64; 3]) {
        if self.remote.is_some() {
            return;
        }
        let keep_rule = self.ui.rules.bool("keep_inventory");
        let keep = engine::KeepCurios::parse(engine::config::KEEP_CURIOS);
        let drops = self.curios_edit(|game, inv| {
            let items = GameItems { game: &*game };
            let rule = |ctx: &SlotCtx, s: &ItemStack| {
                curio_of(game, s).and_then(|c| c.drop_rule).map_or(DropRule::Default, |f| f(game, ctx, s))
            };
            engine::death_drops(inv, &items, keep_rule, keep, rule).0
        });
        for stack in drops {
            // getDroppedItem: from the eyes less 0.3, pickup delay 40, a random push.
            let r = &mut self.world.random;
            let f = f64::from(r.next_float() * 0.5);
            let f1 = f64::from(r.next_float() * std::f32::consts::TAU);
            let v = [-f1.sin() * f, 0.2, f1.cos() * f];
            self.spawn_item(stack, at, v, engine::date_jar::DROP_PICKUP_DELAY);
        }
    }

    // ---- mending, piglins, freezing, right click (the mixins and events of the original) ----

    /// HEAD cancellable `enchant_effects::mending(amount)` (`PlayerXpEvent.PickupXp`): a damaged
    /// mending curio takes the orb first; the rest goes on as experience.
    pub(super) fn curios_mending(&mut self, amount: u32) -> Option<u32> {
        if self.remote.is_some() {
            return None;
        }
        let inv = self.curios_read();
        let damaged = inv.worn().into_iter().find(|(_, _, _, s)| s.damage > 0 && s.enchantment_level("minecraft:mending") > 0);
        let (hi, i, cosmetic, _) = damaged?;
        let id = inv.handlers[hi].id.clone();
        let rest = self.curios_edit(|_, inv| {
            let Some(h) = inv.handler_mut(&id) else { return amount };
            let list = if cosmetic { &mut h.cosmetics } else { &mut h.stacks };
            let Some(Some(s)) = list.get_mut(i) else { return amount };
            // handleMending: repair min(value * 2, damage), the orb keeps value - repaired / 2.
            let repair = (amount * 2).min(s.damage);
            s.damage -= repair;
            amount - repair / 2
        });
        Some(rest)
    }
    /// HEAD cancellable `piglins::wearing_gold` (`MixinPiglinAi`): a worn curio that makes piglins
    /// neutral (the API's, else a piglin-safe armour item).
    pub(super) fn curios_wearing_gold(&self) -> bool {
        if self.remote.is_some() {
            return false;
        }
        let inv = self.curios_read();
        let tag = self.res.items().item_tags.get("minecraft:piglin_safe_armor");
        inv.worn().into_iter().filter(|w| !w.2).any(|(hi, i, _, s)| match curio_of(self, s).and_then(|c| c.piglin_neutral) {
            Some(f) => f(self, &inv.ctx(hi, i, false), s),
            None => tag.is_some_and(|t| t.contains(&*s.item)),
        })
    }
    /// HEAD cancellable `wears_freeze_immune` (`MixinLivingEntity.canFreeze`): a worn curio in
    /// `freeze_immune_wearables`.
    pub(super) fn curios_freeze_immune(&self) -> bool {
        if self.remote.is_some() {
            return false;
        }
        let tag = self.res.items().item_tags.get("minecraft:freeze_immune_wearables");
        let inv = self.curios_read();
        inv.worn().into_iter().filter(|w| !w.2).any(|(_, _, _, s)| tag.is_some_and(|t| t.contains(&*s.item)))
    }
    /// HEAD cancellable `creatures::throw_item` (`PlayerInteractEvent.RightClickItem`): a curio whose
    /// `canEquipFromUse` says yes goes into the first free slot, or swaps with the first one.
    pub(super) fn curios_right_click(&mut self, stack: &ItemStack) -> Option<bool> {
        if self.remote.is_some() {
            return None;
        }
        let curio = curio_of(self, stack)?;
        let can = curio.can_equip_from_use?;
        let reg = registry(self);
        let slots = reg.player_slots();
        let all: Vec<SlotType> = reg.slots.values().cloned().collect();
        let inv = self.curios_read();
        let target = {
            let items = GameItems { game: &*self };
            let g: &Game<'_> = self;
            engine::equip_from_use(&inv, &items, &slots, &all, stack, |ctx| can(g, ctx, stack))?
        };
        let (hi, i) = match target {
            Ok(x) | Err(x) => x,
        };
        let id = inv.handlers[hi].id.clone();
        let creative = self.creative;
        let present = self.curios_edit(|_, inv| {
            let h = inv.handler_mut(&id)?;
            let cell = h.stacks.get_mut(i)?;
            cell.replace(stack.clone())
        });
        match (target, present) {
            (Err(_), Some(p)) => *self.held_mut() = Some(p),
            _ => {
                if !creative {
                    *self.held_mut() = None;
                }
            }
        }
        let ctx = SlotCtx { id, index: i, cosmetic: false, visible: true };
        if let Some(f) = curio.equip_from_use {
            f(self, &ctx, stack);
        }
        Some(true)
    }

    // ---- tooltip (ClientEventHandler.onTooltip) ----

    /// RETURN `stations::stack_tooltip`: "Slot: Ring" after the name, then what the curio gives
    /// when worn there.
    pub(super) fn curios_tooltip(&self, stack: &ItemStack, mut lines: Vec<(horizon_client::text::Text, u32)>) -> Vec<(horizon_client::text::Text, u32)> {
        use horizon_client::text::Text;
        if lines.is_empty() {
            return lines;
        }
        let reg = registry(self);
        let slots = reg.player_slots();
        let items = GameItems { game: self };
        let ids = engine::tooltip_slots(&items, &slots, stack);
        if ids.is_empty() {
            return lines;
        }
        let tr = |k: &str| self.pack.names.display_get(k);
        // `curios.tooltip.slot` gold, the names yellow (`curios.identifier.<id>`, or the key itself).
        let mut line = Text::literal(format!("{} ", tr("curios.tooltip.slot").unwrap_or_else(|| "Slot:".into()))).with_color(0xFFAA00);
        let n = ids.len();
        for (j, id) in ids.iter().enumerate() {
            let key = format!("curios.identifier.{id}");
            let mut name = tr(&key).unwrap_or(key);
            if j + 1 < n {
                name.push_str(", ");
            }
            line = line.append(Text::literal(name).with_color(0xFFFF55));
        }
        let mut extra = vec![(line, u32::MAX)];
        // The modifiers per slot (`curios.modifiers.<id>`, then vanilla's "+1 Armor" lines).
        for id in &ids {
            let ctx = SlotCtx { id: id.clone(), index: 0, cosmetic: false, visible: true };
            let mods = curios_modifiers(self, &ctx, stack);
            if mods.is_empty() {
                continue;
            }
            extra.push((Text::literal(""), u32::MAX));
            let head = tr(&format!("curios.modifiers.{id}")).unwrap_or_else(|| format!("curios.modifiers.{id}"));
            extra.push((Text::literal(head).with_color(0xAAAAAA), u32::MAX));
            // Collapsed per attribute and operation, like the original.
            let mut collapsed: BTreeMap<(String, u8), f64> = BTreeMap::new();
            for m in &mods {
                let op = match m.operation {
                    "add_multiplied_base" => 1,
                    "add_multiplied_total" => 2,
                    _ => 0,
                };
                *collapsed.entry((m.attribute.clone(), op)).or_default() += m.amount;
            }
            for ((attr, op), amount) in collapsed {
                if amount == 0.0 {
                    continue;
                }
                let shown = if op == 0 { amount } else { amount * 100.0 };
                let name = match attr.strip_prefix("curios:") {
                    Some(slot) => tr(&format!("curios.identifier.{slot}")).unwrap_or_else(|| slot.to_owned()),
                    None => {
                        let (ns, path) = attr.split_once(':').unwrap_or(("minecraft", &attr));
                        tr(&format!("attribute.name.{path}"))
                            .or_else(|| tr(&format!("attribute.name.{ns}.{path}")))
                            .or_else(|| tr(&format!("attribute.{ns}.{path}")))
                            .unwrap_or_else(|| attr.clone())
                    }
                };
                let num = if shown.fract() == 0.0 { format!("{}", shown.abs() as i64) } else { format!("{:.2}", shown.abs()).trim_end_matches('0').to_owned() };
                let pct = if op == 0 { "" } else { "%" };
                let slots_word = if attr.starts_with("curios:") { " Slots" } else { "" };
                let (sign, color) = if amount > 0.0 { ("+", 0x5555FF) } else { ("-", 0xFF5555) };
                extra.push((Text::literal(format!("{sign}{num}{pct} {name}{slots_word}")).with_color(color), u32::MAX));
            }
        }
        let at = 1.min(lines.len());
        let tail = lines.split_off(at);
        lines.extend(extra);
        lines.extend(tail);
        lines
    }

    // ---- commands (CuriosCommand) ----

    /// HEAD `native_command` (the window's chat): `/curios ...`; on a server's client it goes to the
    /// server, which runs it as this player.
    pub(super) fn curios_native_command(&mut self, command: &str, ran: &mut bool) {
        let Some(parsed) = engine::parse_command(command) else { return };
        *ran = true;
        if self.remote.is_some() {
            if super::forja::server_has_forja() {
                let mut body = vec![1u8];
                body.extend_from_slice(command.as_bytes());
                super::forja::send_to_server(self, NET, &body);
            } else {
                kit_notice::chat(self, "Curios: this server does not run the Horizon port".into());
            }
            return;
        }
        let level = if self.gamemaster() || self.allow_commands() { 2 } else { 0 };
        self.curios_command(parsed, level);
    }
    fn curios_command(&mut self, parsed: std::result::Result<engine::Command, String>, level: i32) {
        let cmd = match parsed {
            Ok(c) => c,
            Err(e) => return kit_notice::chat(self, e),
        };
        if level < engine::date_jar::COMMAND_PERMISSION {
            return kit_notice::chat(self, "You do not have permission to use this command".into());
        }
        let me = self.lan_acting_name.clone().unwrap_or_else(|| "@s".to_owned());
        let target_ok = |t: &str| t == "@s" || t == "@p" || t.eq_ignore_ascii_case(&me);
        let tr = |g: &Game<'_>, key: &str, fallback: &str| g.pack.names.display_get(key).unwrap_or_else(|| fallback.to_owned());
        use engine::Command as C;
        let target = match &cmd {
            C::List => None,
            C::Replace { target, .. } | C::Set { target, .. } | C::Add { target, .. } | C::Remove { target, .. } => Some(target.clone()),
            C::Clear { target, .. } | C::Drop { target, .. } | C::Reset { target } => Some(target.clone()),
        };
        if let Some(t) = &target
            && !target_ok(t)
        {
            return kit_notice::chat(self, format!("Curios: only your own player ({me}, @s) can be named here"));
        }
        let slot_known = |g: &Game<'_>, s: &str| registry(g).slots.contains_key(s);
        let fill = |s: String, args: &[String]| {
            let mut out = s;
            for a in args {
                out = out.replacen("%s", a, 1).replacen("%d", a, 1);
            }
            out
        };
        match cmd {
            C::List => {
                let ids: Vec<String> = registry(self).slots.keys().cloned().collect();
                kit_notice::chat(self, ids.join(", "));
            }
            C::Set { slot, amount, .. } | C::Add { slot, amount, .. } | C::Remove { slot, amount, .. } if !slot_known(self, &slot) => {
                let _ = amount;
                let msg = fill(tr(self, "argument.curios.type.unknown", "Unknown curio slot type %s"), &[slot]);
                kit_notice::chat(self, msg);
            }
            C::Set { slot, amount, .. } => {
                self.curios_edit(|_, inv| {
                    if let Some(h) = inv.handler_mut(&slot) {
                        engine::set_slots(h, amount);
                    }
                    let lost = inv.update();
                    lost
                })
                .into_iter()
                .for_each(|s| self.curios_give(s));
                let msg = fill(tr(self, "commands.curios.set.success", "Type %s now has been set to %d slots for %s"), &[slot, amount.to_string(), me]);
                kit_notice::chat(self, msg);
            }
            C::Add { slot, amount, .. } | C::Remove { slot, amount, .. } if false => {
                let _ = (slot, amount);
            }
            C::Add { slot, amount, .. } => {
                self.curios_edit(|_, inv| {
                    if let Some(h) = inv.handler_mut(&slot) {
                        h.legacy_change(amount.max(0));
                    }
                    inv.update()
                })
                .into_iter()
                .for_each(|s| self.curios_give(s));
                let msg = fill(tr(self, "commands.curios.add.success", "%d slots of type %s have been added to %s"), &[amount.to_string(), slot, me]);
                kit_notice::chat(self, msg);
            }
            C::Remove { slot, amount, .. } => {
                self.curios_edit(|_, inv| {
                    if let Some(h) = inv.handler_mut(&slot) {
                        let n = (amount.max(0) as usize).min(h.slots()) as i32;
                        h.legacy_change(-n);
                    }
                    inv.update()
                })
                .into_iter()
                .for_each(|s| self.curios_give(s));
                let msg = fill(tr(self, "commands.curios.remove.success", "%d slots of type %s have been removed from %s"), &[amount.to_string(), slot, me]);
                kit_notice::chat(self, msg);
            }
            C::Replace { slot, index, item, count, .. } => {
                if self.res.items().get(&item).is_none() {
                    return kit_notice::chat(self, format!("Unknown item '{item}'"));
                }
                let stack = ItemStack::new(&item, count);
                let done = self.curios_edit(|_, inv| {
                    let cell = inv.handler_mut(&slot).and_then(|h| h.stacks.get_mut(index))?;
                    Some(cell.replace(stack))
                });
                match done {
                    Some(_) => {
                        let msg = fill(tr(self, "commands.curios.replace.success", "Replaced slot %s for %s with %s"), &[slot, me, item]);
                        kit_notice::chat(self, msg);
                    }
                    None => kit_notice::chat(self, format!("No slot {slot} {index}")),
                }
            }
            C::Clear { slot, .. } => {
                self.curios_edit(|_, inv| {
                    for h in inv.handlers.iter_mut().filter(|h| slot.as_ref().is_none_or(|s| *s == h.id)) {
                        h.stacks.iter_mut().for_each(|c| *c = None);
                        h.cosmetics.iter_mut().for_each(|c| *c = None);
                    }
                });
                let msg = match slot {
                    Some(s) => fill(tr(self, "commands.curios.clear.success", "Slot %s has been cleared for %s"), &[s, me]),
                    None => fill(tr(self, "commands.curios.clearAll.success", "All slots have been cleared for %s"), &[me]),
                };
                kit_notice::chat(self, msg);
            }
            C::Drop { slot, .. } => {
                let dropped = self.curios_edit(|_, inv| {
                    let mut out = Vec::new();
                    for h in inv.handlers.iter_mut().filter(|h| slot.as_ref().is_none_or(|s| *s == h.id)) {
                        out.extend(h.stacks.iter_mut().filter_map(Option::take));
                        out.extend(h.cosmetics.iter_mut().filter_map(Option::take));
                    }
                    out
                });
                for s in dropped {
                    // CuriosCommand.drop: player.drop(stack, true, false).
                    if let Some(eye) = self.eye_position() {
                        self.throw_from_eyes(s, eye, false);
                    }
                }
                let msg = match slot {
                    Some(s) => fill(tr(self, "commands.curios.drop.success", "Dropped items in slot %s for %s"), &[s, me]),
                    None => fill(tr(self, "commands.curios.dropAll.success", "Dropped items in all slots for %s"), &[me]),
                };
                kit_notice::chat(self, msg);
            }
            C::Reset { .. } => {
                // CuriosCommand.reset: the slots as the types make them; every stack goes back.
                let back = self.curios_edit(|game, inv| {
                    let mut out = Vec::new();
                    for h in inv.handlers.iter_mut() {
                        out.extend(h.stacks.iter_mut().filter_map(Option::take));
                        out.extend(h.cosmetics.iter_mut().filter_map(Option::take));
                    }
                    *inv = engine::Inventory::new(&registry(game).player_slots());
                    out
                });
                for s in back {
                    self.curios_give(s);
                }
                let msg = fill(tr(self, "commands.curios.reset.success", "Reset slots for %s"), &[me]);
                kit_notice::chat(self, msg);
            }
        }
    }

    // ---- the menu: opening, mirrors ----

    /// A free position near the player for the menu's mirror (no block entity there).
    fn curios_mirror_pos(&self, feet: [f64; 3], eye: f64) -> Option<[i32; 3]> {
        let b = |y: f64| [feet[0].floor() as i32, y.floor() as i32, feet[2].floor() as i32];
        let head = b(feet[1] + eye);
        let foot = b(feet[1]);
        let used: Vec<[i32; 3]> = mirrors_here(self).into_iter().map(|(p, _)| p).collect();
        let mut candidates = vec![head, foot, [head[0], head[1] + 1, head[2]]];
        for dx in -1..=1 {
            for dz in -1..=1 {
                for p in [head, foot] {
                    candidates.push([p[0] + dx, p[1], p[2] + dz]);
                }
            }
        }
        candidates
            .into_iter()
            .find(|p| !self.world.block_entities.contains_key(&(p[0], p[1], p[2])) && !used.contains(p))
    }
    /// The menu state of the player in context, with its view and mirror built now.
    fn curios_menu_state(&self, pos: [i32; 3], page: i32, cosmetics: bool) -> MenuRt {
        let reg = registry(self);
        let inv = self.curios_read();
        let mut m = MenuRt { pos, page, cosmetics, quick: Default::default(), view: Default::default(), snapshot: Vec::new() };
        curios_fill_view(&reg, &inv, &self.armor, &self.offhand, &mut m);
        m
    }
    /// A guest asked for the Curios menu (`CPacketOpenCurios`): a forja menu over a mirror near them.
    fn curios_open_guest(&mut self, host: &mut LanHost, conn: u32) {
        curios_register();
        if !super::forja::guest_has_forja(conn) {
            return;
        }
        let Some(g) = host.hub.guest(conn) else { return };
        if g.dead {
            return;
        }
        let (feet, eye) = (g.position, g.eye_height());
        // ServerPlayer.openMenu closes what was open (our own mirror goes with it, in `closed`).
        if super::forja::server_menu_of(conn).is_some() {
            self.forja_menu_close(host, conn, false);
        }
        if let Some(old) = players(|m| m.get_mut(&conn).and_then(|r| r.menu.take())) {
            self.curios_drop_mirror(conn, old.pos);
        }
        let Some(pos) = self.curios_mirror_pos(feet, eye) else { return };
        // The view and the mirror as the guest sees them.
        let state = self.as_guest(host, conn, |g| g.curios_menu_state(pos, 0, false));
        let Some(state) = state else { return };
        players(|m| m.entry(conn).or_default().menu = Some(state));
        let state = self.world.block(pos[0], pos[1], pos[2]).0;
        MIRRORS.lock().unwrap_or_else(|e| e.into_inner()).insert((self.dim.id(), pos), Mirror { owner: conn, state });
        self.world.block_entities.insert((pos[0], pos[1], pos[2]), placeholder());
        if !self.forja_menu_open(host, conn, MENU_KIND, pos) {
            self.curios_drop_mirror(conn, pos);
        }
    }
    /// The menu of `key` closed: its mirror goes (only our own block entity: the placeholder of a
    /// guest, the in-frame host's mirror chest, and only while the block there is unchanged).
    fn curios_drop_mirror(&mut self, key: u32, p: [i32; 3]) {
        if own_mirror(self, key, p) {
            let k = (p[0], p[1], p[2]);
            let ours = match self.world.block_entities.get(&k) {
                Some(BlockEntity::DaylightDetector) => true,
                Some(BlockEntity::Chest(_)) => key == HOST,
                _ => false,
            };
            if ours {
                self.world.block_entities.remove(&k);
            }
        }
        let dim = self.dim.id();
        let mut mirrors = MIRRORS.lock().unwrap_or_else(|e| e.into_inner());
        if mirrors.get(&(dim, p)).is_some_and(|m| m.owner == key) {
            mirrors.remove(&(dim, p));
        }
        drop(mirrors);
        players(|m| {
            if let Some(r) = m.get_mut(&key)
                && r.menu.as_ref().is_some_and(|mm| mm.pos == p)
            {
                r.menu = None;
            }
        });
    }
    /// After a save took them out, the mirrors of this dimension are put back: the placeholder for
    /// a guest, the mirror chest for the in-frame host (only where the block is unchanged and no
    /// other block entity stands).
    fn curios_restore_mirrors(&mut self) {
        for (p, m) in mirrors_here(self) {
            let k = (p[0], p[1], p[2]);
            // A mirror whose menu is gone (closed from another dimension's server): it goes too.
            let open = players(|x| x.get(&m.owner).and_then(|r| r.menu.as_ref()).is_some_and(|mm| mm.pos == p));
            if !open {
                self.curios_drop_mirror(m.owner, p);
                continue;
            }
            if self.world.block(p[0], p[1], p[2]).0 != m.state || self.world.block_entities.contains_key(&k) {
                continue;
            }
            if m.owner == HOST {
                let snapshot = players(|x| x.get(&HOST).and_then(|r| r.menu.as_ref()).map(|mm| mm.snapshot.clone()));
                if let Some(s) = snapshot {
                    self.world.block_entities.insert(k, BlockEntity::Chest(s));
                }
            } else {
                self.world.block_entities.insert(k, placeholder());
            }
        }
    }
    /// The in-frame host opens the Curios menu (no forja: the window is the server).
    fn curios_open_local(&mut self) {
        let Some(feet) = self.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z]) else { return };
        let Some(pos) = self.curios_mirror_pos(feet, f64::from(self.eye)) else { return };
        let state = self.curios_menu_state(pos, 0, false);
        let snapshot = state.snapshot.clone();
        if let Some(old) = players(|m| m.get_mut(&HOST).and_then(|r| r.menu.take())) {
            self.curios_drop_mirror(HOST, old.pos);
        }
        players(|m| m.entry(HOST).or_default().menu = Some(state));
        let state = self.world.block(pos[0], pos[1], pos[2]).0;
        MIRRORS.lock().unwrap_or_else(|e| e.into_inner()).insert((self.dim.id(), pos), Mirror { owner: HOST, state });
        let key = (pos[0], pos[1], pos[2]);
        self.world.block_entities.insert(key, BlockEntity::Chest(snapshot));
        self.reset_inventory_clicks();
        self.station = None;
        self.craft_size = 2;
        self.container = Some(key);
        self.screen = Screen::Inventory;
        self.capture(false);
    }
    /// The view and the mirror again from the real state (after a click, every tick).
    fn curios_refresh_menu(&mut self) {
        let key = self.curios_key();
        let Some((pos, page, cosmetics)) = players(|m| m.get(&key).and_then(|r| r.menu.as_ref()).map(|mm| (mm.pos, mm.page, mm.cosmetics))) else {
            return;
        };
        let mut state = self.curios_menu_state(pos, page, cosmetics);
        players(|m| {
            if let Some(mm) = m.get_mut(&key).and_then(|r| r.menu.as_mut()) {
                state.quick = std::mem::take(&mut mm.quick);
                *mm = state;
            }
        });
        // The in-frame host draws the mirror itself: it is the block entity while open.
        if key == HOST {
            let snapshot = players(|m| m.get(&HOST).and_then(|r| r.menu.as_ref()).map(|mm| mm.snapshot.clone()));
            let k = (pos[0], pos[1], pos[2]);
            if let Some(s) = snapshot
                && self.container == Some(k)
                && own_mirror(self, HOST, pos)
                && matches!(self.world.block_entities.get(&k), None | Some(BlockEntity::Chest(_) | BlockEntity::DaylightDetector))
            {
                self.world.block_entities.insert(k, BlockEntity::Chest(s));
            }
        }
    }
    /// HEAD `close_container`: the in-frame host's Curios menu closes.
    pub(super) fn curios_close(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let Some(k) = self.container else { return };
        let p = [k.0, k.1, k.2];
        if self.lan_acting.is_none() && mirror_at(self, p).is_some_and(|m| m.owner == HOST) {
            // AbstractContainerMenu.removed gives the carried stack back (close_container does).
            self.curios_drop_mirror(HOST, p);
            self.container = None;
        }
    }
    /// HEAD `save_level` / `save_game_metadata`: no mirror is written with the world (put back on
    /// the next tick, before forja checks the menus).
    pub(super) fn curios_before_save(&mut self) {
        if self.remote.is_some() {
            return;
        }
        for (p, m) in mirrors_here(self) {
            let k = (p[0], p[1], p[2]);
            if self.world.block(p[0], p[1], p[2]).0 != m.state {
                continue;
            }
            let ours = match self.world.block_entities.get(&k) {
                Some(BlockEntity::DaylightDetector) => true,
                Some(BlockEntity::Chest(_)) => m.owner == HOST,
                _ => false,
            };
            if ours {
                self.world.block_entities.remove(&k);
            }
        }
    }
    /// HEAD cancellable `machines::try_move_in_item` / `take_one`: hoppers never reach a mirror.
    pub(super) fn curios_mirror_view(&self, parts: &[([i32; 3], usize)]) -> bool {
        if self.remote.is_some() {
            return false;
        }
        let here = mirrors_here(self);
        !here.is_empty() && parts.iter().any(|(p, _)| here.iter().any(|(q, _)| q == p))
    }

    // ---- the menu: clicks and buttons (run as the player: the server in the guest's turn, or the
    //      in-frame host) ----

    /// The open Curios menu of the player in context, when `self.container` is its mirror.
    fn curios_menu_open_here(&self) -> bool {
        let Some(k) = self.container else { return false };
        mirror_at(self, [k.0, k.1, k.2]).is_some_and(|m| m.owner == self.curios_key())
    }
    /// HEAD cancellable `menu_ops`: every click of the Curios menu (the engine's V2 menu); always
    /// taken while it is open, so no other menu code touches the mirror.
    pub(super) fn curios_menu_ops(&mut self, ops: &[(Option<usize>, i32, horizon_client::menu::Input)]) -> Option<bool> {
        if self.remote.is_some() || !self.curios_menu_open_here() {
            return None;
        }
        let actions: Vec<(Option<usize>, i32, engine::Click, Option<ItemStack>)> =
            ops.iter().map(|&(slot, button, input)| (slot, button, input_click(input), None)).collect();
        Some(self.curios_menu_apply(&actions))
    }
    /// The clicks of the Curios menu, as the player in context: (game slot, button, click, the
    /// stack a Shift double click gathers).
    fn curios_menu_apply(&mut self, actions: &[(Option<usize>, i32, engine::Click, Option<ItemStack>)]) -> bool {
        let key = self.curios_key();
        let reg = registry(self);
        let slots = reg.player_slots();
        let all: Vec<SlotType> = reg.slots.values().cloned().collect();
        let Some((mut page, cosmetics, mut quick)) =
            players(|m| m.get_mut(&key).and_then(|r| r.menu.as_mut()).map(|mm| (mm.page, mm.cosmetics, std::mem::take(&mut mm.quick))))
        else {
            return false;
        };
        let creative = self.creative;
        let mut inventory = std::mem::take(&mut self.inventory);
        let mut armor = std::mem::take(&mut self.armor);
        let mut offhand = self.offhand.take();
        let mut carried = self.carried.take();
        let mut thrown = Vec::new();
        let mut equipped: Vec<(SlotCtx, ItemStack)> = Vec::new();
        // Not curios_edit: the inventory is out of `self` here, so the stacks that no longer fit
        // are given back only once it is in place again.
        let (mut inv_data, lost) = load_inventory(self, self.level.player_extra.as_ref());
        {
            let game: &Game<'_> = self;
            let inv = &mut inv_data;
            let items = GameItems { game };
            for (slot, button, click, gather) in actions {
                let (slot, button) = (*slot, *button);
                let layout = engine::layout(inv, page, cosmetics, engine::config::MAX_SLOTS_PER_PAGE, engine::config::MINIMUM_COLUMNS);
                let n = layout.slots.len();
                let menu_slot = match slot {
                    None => engine::OUTSIDE,
                    Some(g @ 0..=8) => (engine::MENU_HOTBAR + g) as i32,
                    Some(g @ 9..=35) => g as i32,
                    Some(g @ 500..=503) => (engine::MENU_ARMOR + g - 500) as i32,
                    Some(504) => engine::MENU_OFFHAND as i32,
                    Some(g) if g >= 300 && g < 300 + n => (engine::MENU_CURIOS + g - 300) as i32,
                    Some(g) if g >= 300 + n && g < 304 + n => (engine::MENU_ARMOR + g - 300 - n) as i32,
                    Some(g) if g == 304 + n => engine::MENU_OFFHAND as i32,
                    Some(_) => continue,
                };
                let mut player = engine::Player {
                    inventory: &mut inventory,
                    armor: &mut armor,
                    offhand: &mut offhand,
                    carried: &mut carried,
                    creative,
                    thrown: Vec::new(),
                };
                let (move_to, put) = {
                    let mut menu = engine::Menu::new(&items, &mut player, inv, &layout, &slots, &all, &mut quick);
                    match gather {
                        Some(moved) if menu_slot >= 0 => menu.quick_move_all(menu_slot as usize, moved),
                        _ => menu.click(menu_slot, button, *click),
                    }
                    (menu.move_to_page, menu.equipped.clone())
                };
                for j in put {
                    if let Some(p) = layout.slots.get(j) {
                        let ctx = inv.ctx(p.handler, p.index, p.cosmetic);
                        let h = &inv.handlers[p.handler];
                        let list = if p.cosmetic { &h.cosmetics } else { &h.stacks };
                        if let Some(Some(s)) = list.get(p.index) {
                            equipped.push((ctx, s.clone()));
                        }
                    }
                }
                // checkQuickMove: the stack fits only on another page: that page, and the move again.
                if let Some((to, from)) = move_to {
                    page = to;
                    let layout = engine::layout(inv, page, cosmetics, engine::config::MAX_SLOTS_PER_PAGE, engine::config::MINIMUM_COLUMNS);
                    let mut menu = engine::Menu::new(&items, &mut player, inv, &layout, &slots, &all, &mut quick);
                    menu.quick_move_stack(from);
                }
                thrown.extend(player.thrown);
            }
        }
        write_saved(&mut self.level.player_extra, &inv_data);
        self.inventory = inventory;
        self.armor = armor;
        self.offhand = offhand;
        self.carried = carried;
        for s in lost {
            self.curios_give(s);
        }
        for s in thrown {
            self.player_drop(s);
        }
        players(|m| {
            if let Some(mm) = m.get_mut(&key).and_then(|r| r.menu.as_mut()) {
                mm.page = page;
                mm.quick = quick;
            }
        });
        for (ctx, s) in equipped {
            if let Some(f) = curio_of(self, &s).and_then(|c| c.equip_from_use) {
                f(self, &ctx, &s);
            }
        }
        self.curios_refresh_menu();
        true
    }
    /// A button of the Curios screen, run as the player (`CPacketPage`, `CPacketToggleCosmetics`,
    /// `CPacketToggleRender`).
    fn curios_button(&mut self, id: u32, arg: i64) -> bool {
        let key = self.curios_key();
        let Some((page, total, slot)) = players(|m| {
            m.get(&key).and_then(|r| r.menu.as_ref()).map(|mm| {
                let slot = usize::try_from(arg).ok().and_then(|j| mm.view.layout.slots.get(j).cloned());
                (mm.page, mm.view.layout.total_pages, slot)
            })
        }) else {
            return false;
        };
        match id {
            engine::button::PAGE => {
                let to = if arg != 0 { (page + 1).min(total - 1) } else { (page - 1).max(0) };
                players(|m| {
                    if let Some(mm) = m.get_mut(&key).and_then(|r| r.menu.as_mut()) {
                        mm.page = to.max(0);
                    }
                });
            }
            engine::button::COSMETICS => players(|m| {
                if let Some(mm) = m.get_mut(&key).and_then(|r| r.menu.as_mut()) {
                    mm.cosmetics = !mm.cosmetics;
                }
            }),
            engine::button::RENDER => {
                let Some(p) = slot else { return false };
                self.curios_edit(|_, inv| {
                    if let Some(r) = inv.handler_mut(&p.id).and_then(|h| h.renders.get_mut(p.index)) {
                        *r = !*r;
                    }
                });
            }
            _ => return false,
        }
        self.curios_refresh_menu();
        true
    }

    // ---- the screen (CuriosScreenV2), drawn by the window from the mirror and the view ----

    /// The Curios screen this window shows: (view, mirror position).
    fn curios_screen(&self) -> Option<(engine::View, [i32; 3])> {
        if self.screen != Screen::Inventory {
            return None;
        }
        if self.remote.is_some() {
            let (kind, pos) = super::forja::guest_menu(self)?;
            if kind != MENU_KIND {
                return None;
            }
            let (generation, sections) = super::forja::guest_sections(self)?;
            let view = CLIENT_VIEW.with(|c| {
                let mut c = c.borrow_mut();
                if c.as_ref().is_none_or(|(g, _)| *g != generation) {
                    *c = Some((generation, sections.first().and_then(|b| engine::View::decode(b))));
                }
                c.as_ref().and_then(|(_, v)| v.clone())
            })?;
            return Some((view, pos));
        }
        let k = self.container?;
        let p = [k.0, k.1, k.2];
        if !mirror_at(self, p).is_some_and(|m| m.owner == HOST) {
            return None;
        }
        let view = players(|m| m.get(&HOST).and_then(|r| r.menu.as_ref()).map(|mm| mm.view.clone()))?;
        Some((view, p))
    }
    /// HEAD cancellable `machines::screen_height`: the inventory's 166.
    pub(super) fn curios_screen_height(&self) -> Option<f32> {
        self.curios_screen().map(|_| engine::screen::HEIGHT)
    }
    /// HEAD cancellable `machines::machine_slot_position`: the mirror's slots (curios, armour, off hand).
    pub(super) fn curios_slot_position(&self, i: usize) -> Option<Option<(f32, f32)>> {
        if !(300..400).contains(&i) {
            return None;
        }
        let (view, _) = self.curios_screen()?;
        let n = view.layout.slots.len();
        let j = i - 300;
        Some(if j < n {
            let s = &view.layout.slots[j];
            Some((s.x as f32, s.y as f32))
        } else if j < n + 4 {
            Some((8.0, 8.0 + (j - n) as f32 * 18.0))
        } else if j == n + 4 {
            Some((77.0, 62.0))
        } else {
            None
        })
    }
    /// HEAD cancellable `quick_craft_accepts`: a drag never adds a slot the menu would refuse.
    pub(super) fn curios_drag_accepts(&self, slot: usize) -> Option<bool> {
        let (view, pos) = self.curios_screen()?;
        if !(300..400).contains(&slot) {
            return None;
        }
        let carried = self.carried.as_ref()?;
        let n = view.layout.slots.len();
        let j = slot - 300;
        let items = GameItems { game: self };
        if j < n {
            let p = &view.layout.slots[j];
            let reg = registry(self);
            let slots = reg.player_slots();
            let all: Vec<SlotType> = reg.slots.values().cloned().collect();
            let ctx = SlotCtx { id: p.id.clone(), index: p.index, cosmetic: p.cosmetic, visible: p.render };
            let empty = match self.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
                Some(BlockEntity::Chest(v)) => v.get(j).is_none_or(|s| s.is_none()),
                _ => true,
            };
            return Some(empty && engine::may_equip(&items, &slots, &all, &ctx, carried));
        }
        if j < n + 4 {
            return Some(engine::Items::armor_slot(&items, carried) == Some(j - n));
        }
        None
    }
    fn curios_blit(&self, batch: &mut HudBatch, texture: &str, x: f32, y: f32, u: f32, v: f32, w: f32, h: f32) {
        let atlas = &self.gui.atlas;
        let Some(mut r) = atlas.sprites.get(texture).copied() else { return };
        let t = engine::screen::TEXTURE_SIZE;
        let (du, dv) = (r.u1 - r.u0, r.v1 - r.v0);
        let (u0, v0) = (r.u0, r.v0);
        r.u0 = u0 + du * u / t;
        r.u1 = u0 + du * (u + w) / t;
        r.v0 = v0 + dv * v / t;
        r.v1 = v0 + dv * (v + h) / t;
        batch.rect_uv(x, y, w, h, r, u32::MAX);
    }
    /// The GUI sprite of a slot type's icon (`curios:slot/empty_ring_slot` is carried as
    /// `curios:block/slot/...`), the generic curio icon when the atlas has neither.
    fn curios_icon(&self, icon: &str) -> Option<String> {
        let sprites = &self.gui.atlas.sprites;
        let (ns, path) = icon.split_once(':').unwrap_or(("minecraft", icon));
        [icon.to_owned(), format!("{ns}:block/{path}"), format!("{ns}:{}", path.trim_start_matches("item/")), "curios:block/slot/empty_curio_slot".to_owned()]
            .into_iter()
            .find(|n| sprites.contains_key(n))
    }
    /// HEAD cancellable `machines::draw_machine_screen`: `CuriosScreenV2.renderBg` (the inventory
    /// background without the 2x2 crafting grid, the player, the panel, the empty slot icons).
    pub(super) fn curios_draw_screen(&self, batch: &mut HudBatch, ox: f32, oy: f32) -> bool {
        let Some((view, pos)) = self.curios_screen() else { return false };
        let atlas = &self.gui.atlas;
        // The vanilla inventory background.
        if let Some(mut r) = atlas.sprites.get("gui/container/inventory").copied() {
            let (du, dv) = ((r.u1 - r.u0) / r.w as f32, (r.v1 - r.v0) / r.h as f32);
            r.u1 = r.u0 + 176.0 * du;
            r.v1 = r.v0 + 166.0 * dv;
            batch.rect_uv(ox, oy, 176.0, 166.0, r, u32::MAX);
        }
        // No crafting grid in this port: its area painted over like the background (README).
        batch.fill(atlas, ox + 96.0, oy + 5.0, 76.0, 50.0, rgba(198, 198, 198, 255));
        self.draw_inventory_player(batch, ox, oy);
        let l = &view.layout;
        let (i, j) = (ox, oy);
        let mut x_offset = -33.0f32;
        let mut y_offset = j;
        let page_offset = l.total_pages > 1;
        if l.has_cosmetics {
            self.curios_blit(batch, INVENTORY_REVAMP, i + x_offset + 2.0, y_offset - 23.0, 32.0, 0.0, 28.0, 24.0);
        }
        let grid = &l.grid;
        let cols = grid.len() as f32;
        if !grid.is_empty() {
            x_offset -= (cols - 1.0) * 18.0;
            let rows = grid[0] as f32;
            for r in 0..grid.len() {
                let mut upper = 7.0 + rows * 18.0;
                let mut tex = 91.0;
                if page_offset {
                    upper += 8.0;
                }
                if r != 0 {
                    tex += 7.0;
                }
                self.curios_blit(batch, INVENTORY_REVAMP, i + x_offset, y_offset, tex, 0.0, 25.0, upper);
                self.curios_blit(batch, INVENTORY_REVAMP, i + x_offset, y_offset + upper, tex, 159.0, 25.0, 7.0);
                if grid.len() == 1 {
                    tex += 7.0;
                    self.curios_blit(batch, INVENTORY_REVAMP, i + x_offset + 7.0, y_offset, tex, 0.0, 25.0, upper);
                    self.curios_blit(batch, INVENTORY_REVAMP, i + x_offset + 7.0, y_offset + upper, tex, 159.0, 25.0, 7.0);
                }
                x_offset += if r == 0 { 25.0 } else { 18.0 };
            }
            x_offset -= cols * 18.0;
            if page_offset {
                y_offset += 8.0;
            }
            for rows in grid {
                self.curios_blit(batch, INVENTORY_REVAMP, i + x_offset, y_offset + 7.0, 7.0, 7.0, 18.0, *rows as f32 * 18.0);
                x_offset += 18.0;
            }
        }
        // The cosmetic slots' frame, then the empty slots' icons (the slot type's background).
        let mirror = match self.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
            Some(BlockEntity::Chest(v)) => v.clone(),
            _ => Vec::new(),
        };
        for (n, s) in l.slots.iter().enumerate() {
            if s.cosmetic {
                self.curios_blit(batch, INVENTORY_REVAMP, s.x as f32 + i - 1.0, s.y as f32 + j - 1.0, 32.0, 50.0, 18.0, 18.0);
            }
            if mirror.get(n).is_none_or(|x| x.is_none()) {
                let icon = view.icons.get(&s.id).cloned().unwrap_or_else(|| engine::date_jar::DEFAULT_ICON.to_owned());
                if let Some(name) = self.curios_icon(&icon) {
                    batch.sprite(atlas, &name, i + s.x as f32, j + s.y as f32);
                }
            }
        }
        // The armour and shield slot icons (InventoryMenu's backgrounds).
        let n = l.slots.len();
        for (k, icon) in ["helmet", "chestplate", "leggings", "boots"].iter().enumerate() {
            if mirror.get(n + k).is_none_or(|x| x.is_none()) {
                batch.sprite(atlas, &format!("gui/sprites/container/slot/{icon}"), i + 8.0, j + 8.0 + k as f32 * 18.0);
            }
        }
        if mirror.get(n + 4).is_none_or(|x| x.is_none()) {
            batch.sprite(atlas, "gui/sprites/container/slot/shield", i + 77.0, j + 62.0);
        }
        true
    }
    /// HEAD `draw_carried` (over the slots, under the carried stack): the Curios button on the
    /// inventory screens; on the Curios screen its buttons and tooltips.
    pub(super) fn curios_draw_over(&self, batch: &mut HudBatch) {
        let (mx, my) = self.cursor;
        if let Some((view, pos)) = self.curios_screen() {
            let (ox, oy) = self.inventory_origin(batch);
            let (rx, ry) = (mx - ox, my - oy);
            let hovered = engine::widget_at(&view, rx, ry);
            let l = &view.layout;
            // The Curios button (back to the inventory).
            if engine::config::CLIENT_ENABLE_BUTTON {
                let (bx, by) = engine::button_offset(false);
                let v = if hovered == Some(engine::Widget::Vanilla) { 14.0 } else { 0.0 };
                self.curios_blit(batch, INVENTORY_LEGACY, ox + bx as f32, oy + by as f32, 50.0, v, 14.0, 14.0);
            }
            if l.has_cosmetics {
                let u = if l.viewing_cosmetics { 143.0 } else { 123.0 };
                let v = if hovered == Some(engine::Widget::Cosmetics) { 17.0 } else { 0.0 };
                let (x, y, w, h) = engine::screen::COSMETIC;
                self.curios_blit(batch, INVENTORY_REVAMP, ox + x, oy + y, u, v, w, h);
            }
            if l.total_pages > 1 {
                for (rect, next) in [(engine::screen::PAGE_NEXT, true), (engine::screen::PAGE_PREV, false)] {
                    let active = if next { l.page + 1 < l.total_pages } else { l.page > 0 };
                    let mut u = if next { 43.0 } else { 32.0 };
                    let mut v = 25.0;
                    let over = hovered == Some(if next { engine::Widget::PageNext } else { engine::Widget::PagePrev });
                    if !active {
                        v += 12.0;
                    } else if over {
                        u += 22.0;
                    }
                    self.curios_blit(batch, INVENTORY_REVAMP, ox + rect.0, oy + rect.1, u, v, rect.2, rect.3);
                }
            }
            for s in l.slots.iter().filter(|s| !s.cosmetic && s.render_toggle) {
                let u = if s.render { 75.0 } else { 83.0 };
                let (x, y) = (ox + s.x as f32 + engine::screen::RENDER_DX, oy + s.y as f32 + engine::screen::RENDER_DY);
                self.curios_blit(batch, INVENTORY_REVAMP, x, y, u, 0.0, 8.0, 8.0);
            }
            if self.carried.is_some() {
                return;
            }
            let tr = |k: &str| self.pack.names.display_get(k);
            match hovered {
                Some(engine::Widget::Render(_)) => {
                    let t = tr("gui.curios.toggle").unwrap_or_else(|| "Toggle Visibility".into());
                    self.tooltip(batch, &t, mx, my);
                }
                Some(engine::Widget::Cosmetics) => {
                    let t = tr("gui.curios.toggle.cosmetics").unwrap_or_else(|| "Toggle Cosmetics".into());
                    self.tooltip(batch, &t, mx, my);
                }
                Some(engine::Widget::PageNext | engine::Widget::PagePrev) => {
                    let t = tr("gui.curios.page").unwrap_or_else(|| "Page %d / %d".into());
                    let t = t.replacen("%d", &(l.page + 1).to_string(), 1).replacen("%d", &l.total_pages.to_string(), 1);
                    self.tooltip(batch, &t, mx, my);
                }
                _ => {
                    // An empty curio slot under the cursor: its name (CurioSlot.getSlotName).
                    let mirror = match self.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
                        Some(BlockEntity::Chest(v)) => v.clone(),
                        _ => Vec::new(),
                    };
                    for (n, s) in l.slots.iter().enumerate() {
                        let (x, y) = (ox + s.x as f32, oy + s.y as f32);
                        if mx >= x && mx < x + 16.0 && my >= y && my < y + 16.0 && mirror.get(n).is_none_or(|x| x.is_none()) {
                            let name = engine::slot_name(&s.id, s.cosmetic, &tr);
                            self.tooltip(batch, &name, mx, my);
                        }
                    }
                }
            }
            return;
        }
        // The Curios button on the inventory and on the creative inventory tab (GuiEventHandler).
        if let Some((x, y, size, u)) = self.curios_inventory_button(batch) {
            let over = mx >= x && mx < x + size && my >= y && my < y + size;
            self.curios_blit(batch, INVENTORY_LEGACY, x, y, u, if over { size } else { 0.0 }, size, size);
        }
    }
    /// Where the Curios button is on the open inventory screen: (x, y, size, u).
    fn curios_inventory_button(&self, batch: &HudBatch) -> Option<(f32, f32, f32, f32)> {
        if !engine::config::CLIENT_ENABLE_BUTTON {
            return None;
        }
        // Only where the Curios menu can open: a Horizon server with forja, or the in-frame host.
        if self.remote.is_some() && !super::forja::server_has_forja() {
            return None;
        }
        let creative_tab = self.screen == Screen::Creative && self.creative_screen.tab().inventory;
        let inventory = self.screen == Screen::Inventory && self.container.is_none() && self.craft_size == 2 && self.station.is_none() && !self.mount_active();
        if !creative_tab && !inventory {
            return None;
        }
        let (ox, oy) = self.inventory_origin(batch);
        let (bx, by) = engine::button_offset(creative_tab);
        let (size, u) = if creative_tab { (engine::screen::BUTTON_CREATIVE, 64.0) } else { (engine::screen::BUTTON, 50.0) };
        Some((ox + bx as f32, oy + by as f32, size, u))
    }
    /// HEAD cancellable `tweaks::sort_mouse_press` (every press first): the buttons.
    pub(super) fn curios_mouse(&mut self, button: winit::event::MouseButton) -> bool {
        if button != winit::event::MouseButton::Left {
            return false;
        }
        let (mx, my) = self.cursor;
        if let Some((view, _)) = self.curios_screen() {
            let Some(batch) = super::forja::hud_batch(self) else { return false };
            // The in-frame host: a Shift double click would run Horizon's own quick_move_all on
            // the mirror; it is the menu's (`mouseReleased`) here.
            if self.remote.is_none()
                && self.has_shift_down()
                && let Some(slot) = self.slot_at(&batch, mx, my)
                && slot != 200
            {
                let now = std::time::Instant::now();
                let double = self
                    .last_inv_click
                    .is_some_and(|(at, last, right)| last == slot && !right && now.duration_since(at).as_millis() < 250);
                if double {
                    self.last_inv_click = None;
                    if let Some(moved) = self.last_quick_moved.clone() {
                        self.curios_menu_apply(&[(Some(slot), 0, engine::Click::QuickMove, Some(moved))]);
                    }
                    return true;
                }
            }
            let (ox, oy) = self.inventory_origin(&batch);
            let Some(w) = engine::widget_at(&view, mx - ox, my - oy) else { return false };
            let (id, arg) = match w {
                engine::Widget::Vanilla => {
                    // CuriosButton on the Curios screen: back to the inventory (CPacketOpenVanilla).
                    self.audio.play_button_click();
                    self.close_container();
                    self.reset_inventory_clicks();
                    self.screen = Screen::Inventory;
                    self.capture(false);
                    return true;
                }
                engine::Widget::Cosmetics => (engine::button::COSMETICS, 0),
                engine::Widget::PageNext => {
                    if view.layout.page + 1 >= view.layout.total_pages {
                        return true;
                    }
                    (engine::button::PAGE, 1)
                }
                engine::Widget::PagePrev => {
                    if view.layout.page <= 0 {
                        return true;
                    }
                    (engine::button::PAGE, 0)
                }
                engine::Widget::Render(j) => (engine::button::RENDER, j as i64),
            };
            self.audio.play_button_click();
            if self.remote.is_some() {
                super::forja::guest_send(self, super::forja::MenuInput::Button { id, arg });
            } else {
                self.curios_button(id, arg);
            }
            return true;
        }
        let Some(batch) = super::forja::hud_batch(self) else { return false };
        if let Some((x, y, size, _)) = self.curios_inventory_button(&batch)
            && mx >= x
            && mx < x + size
            && my >= y
            && my < y + size
        {
            self.audio.play_button_click();
            self.curios_request_open();
            return true;
        }
        false
    }
    /// Open the Curios menu: ask the server (a server's client), or open it here (in-frame host).
    fn curios_request_open(&mut self) {
        if self.remote.is_some() {
            if super::forja::server_has_forja() {
                super::forja::send_to_server(self, NET, &[0]);
            }
        } else if self.lan_acting.is_none() {
            self.curios_open_local();
        }
    }
    /// HEAD cancellable `tweaks::tweak_scroll`: the wheel over the panel turns the page.
    pub(super) fn curios_wheel(&mut self, delta: f32) -> bool {
        let Some((view, _)) = self.curios_screen() else { return false };
        let Some(batch) = super::forja::hud_batch(self) else { return false };
        let (ox, oy) = self.inventory_origin(&batch);
        if !engine::wheel_turns_page(&view, self.cursor.0 - ox, self.cursor.1 - oy) {
            return false;
        }
        let next = delta < 0.0;
        let can = if next { view.layout.page + 1 < view.layout.total_pages } else { view.layout.page > 0 };
        if can {
            let input = super::forja::MenuInput::Button { id: engine::button::PAGE, arg: i64::from(next) };
            if self.remote.is_some() {
                super::forja::guest_send(self, input);
            } else {
                self.curios_button(engine::button::PAGE, i64::from(next));
            }
        }
        true
    }
    /// HEAD `native_key`: G opens the Curios menu while playing (`ClientEventHandler.onClientTick`)
    /// and closes it on its screen (`CuriosScreenV2.keyPressed`).
    pub(super) fn curios_key_event(&mut self, key: &str, pressed: bool) {
        if !pressed || !self.native_on(MOD_ID) {
            return;
        }
        let name = self.native_mod_key(MOD_ID, KEY_OPEN, KEY_DEFAULT);
        if key != name {
            return;
        }
        if self.native_playing() {
            self.curios_request_open();
        } else if self.curios_screen().is_some() {
            self.close_container();
            self.screen = Screen::Playing;
            self.capture(true);
        }
    }
}

/// HEAD `native_mods`: Curios in Options > Mods, with its key.
pub(super) fn curios_native_mods(mods: &mut Vec<super::native_hooks::NativeMod>) {
    mods.push(super::native_hooks::NativeMod {
        id: MOD_ID.into(),
        title: "Curios API 5.14.1".into(),
        description: "Accessory slots (rings, necklaces, belts...) for the mods that use them; the Curios screen opens from the inventory or with its key.".into(),
        keys: vec![[KEY_OPEN.into(), "Open/Close Curios Inventory".into(), KEY_DEFAULT.into()]],
    });
}

/// HEAD `load_world_state`: a world opens: the per-player state and mirrors of the last one go.
pub(super) fn curios_world_opened() {
    *PLAYERS.lock().unwrap_or_else(|e| e.into_inner()) = None;
    MIRRORS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    *REGISTRY.lock().unwrap_or_else(|e| e.into_inner()) = None;
    REGISTERED.store(false, Ordering::Relaxed);
    curios_register();
}

/// The modifiers a worn curio gives in `ctx` (`CuriosApi.getAttributeModifiers`).
fn curios_modifiers(game: &Game<'_>, ctx: &SlotCtx, s: &ItemStack) -> Vec<Modifier> {
    curio_of(game, s).and_then(|c| c.modifiers).map(|f| f(game, ctx, s)).unwrap_or_default()
}

/// The menu's view and mirror: the page's curio slots, then armour (head..feet) and the off hand.
fn curios_fill_view(
    reg: &engine::Registry,
    inv: &engine::Inventory<ItemStack>,
    armor: &[Option<ItemStack>; 4],
    offhand: &Option<ItemStack>,
    m: &mut MenuRt,
) {
    let layout = engine::layout(inv, m.page, m.cosmetics, engine::config::MAX_SLOTS_PER_PAGE, engine::config::MINIMUM_COLUMNS);
    let mut page = m.page;
    if layout.total_pages > 0 && page >= layout.total_pages {
        page = layout.total_pages - 1;
    }
    let layout = if page != m.page {
        m.page = page;
        engine::layout(inv, page, m.cosmetics, engine::config::MAX_SLOTS_PER_PAGE, engine::config::MINIMUM_COLUMNS)
    } else {
        layout
    };
    let mut snapshot: Vec<Option<ItemStack>> = layout
        .slots
        .iter()
        .map(|p| {
            let h = &inv.handlers[p.handler];
            let list = if p.cosmetic { &h.cosmetics } else { &h.stacks };
            list.get(p.index).cloned().flatten()
        })
        .collect();
    snapshot.extend(armor.iter().cloned());
    snapshot.push(offhand.clone());
    let icons = layout.slots.iter().filter_map(|p| reg.slots.get(&p.id).map(|t| (p.id.clone(), t.icon.clone()))).collect();
    m.view = engine::View { layout, icons };
    m.snapshot = snapshot;
}

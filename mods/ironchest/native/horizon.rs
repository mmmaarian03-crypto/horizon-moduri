// Iron Chests inside Horizon's game (nightly 142388e1c). The rules are the engine below;
// this part is the game's side: the chests' items are Horizon's own
// `BlockEntity::Chest(Vec<..>)` of each tier's size (54, 81, 108, 45, 1 slots), kept in
// `world.block_entities`, so the vanilla code for hoppers, comparators, saving, LAN
// containers and spills works on them. Later mods follow the same pattern.
//
// The hooks (mixins/ironchest/horizon-mixin.toml) call the `ironchest_*` methods here;
// the game library's hooks use `horizon_client::ironchest` (mixins/ironchest/shared.rs).
use super::{DragView, Game, Screen, lan::LanHost};
use horizon_client::{
    containers::BlockEntity,
    hud::{HudBatch, rgba},
    items::ItemStack,
    menu::{Env, Input, Kind, Menu},
};
use horizon_core::position::BlockPos;
use horizon_world::level::flags as level_flags;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// Some of the rules (row counts...) only the tests use.
#[allow(dead_code)]
mod engine {
    include!("ironchest_engine.rs");
}
use engine::{Block, Chest, Facing, Half, Tier, UpgradeOutcome};

impl engine::Stack for ItemStack {
    fn item(&self) -> &str {
        &self.item
    }
    fn count(&self) -> u32 {
        self.count
    }
    fn set_count(&mut self, count: u32) {
        self.count = count;
    }
    fn same_kind(&self, other: &Self) -> bool {
        self.same_item_same_components(other)
    }
}

/// The level as the rules see it.
struct Live<'a, 'g> {
    game: &'a mut Game<'g>,
}
impl engine::World for Live<'_, '_> {
    type Stack = ItemStack;
    fn block(&self, p: engine::Pos) -> Block {
        let g = &*self.game;
        let state = g.world.block(p[0], p[1], p[2]);
        let name = g.res.facts().get(state).name.clone();
        let props = g.pack.assets.registry.state(state).map(|s| &s.properties);
        let prop = |k: &str| props.and_then(|p| p.get(k)).map(String::as_str);
        let facing = Facing::parse(prop("facing").unwrap_or("north"));
        if let Some(chest) = Chest::parse(&name) {
            Block::Iron {
                chest,
                facing,
                half: Half::parse(prop("type").unwrap_or("single")),
                waterlogged: prop("waterlogged") == Some("true"),
            }
        } else if &*name == "minecraft:chest" || &*name == "minecraft:trapped_chest" {
            Block::Vanilla {
                trapped: &*name == "minecraft:trapped_chest",
                facing,
            }
        } else {
            Block::Other
        }
    }
    fn set_chest(
        &mut self,
        p: engine::Pos,
        chest: Chest,
        facing: Facing,
        half: Half,
        waterlogged: bool,
    ) {
        let g = &mut *self.game;
        let Some(base) = g.pack.assets.registry.default_state(&chest.name()) else {
            return;
        };
        let wl = if waterlogged { "true" } else { "false" };
        let Some(state) = g.with_props(
            base,
            &[
                ("facing", facing.name()),
                ("type", half.name()),
                ("waterlogged", wl),
            ],
        ) else {
            return;
        };
        let at = BlockPos::new(p[0], p[1], p[2]);
        // The block entity was taken before: nothing may spill here.
        let flags = level_flags::UPDATE_ALL | level_flags::UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS;
        g.with_level(|b, level, ticks| b.set_block(level, ticks, at, state, flags));
        let _ = g.process_level_events();
    }
    fn items(&self, p: engine::Pos) -> Option<Vec<Option<ItemStack>>> {
        match self.game.world.block_entities.get(&(p[0], p[1], p[2])) {
            Some(BlockEntity::Chest(items)) => Some(items.clone()),
            _ => None,
        }
    }
    fn take_items(&mut self, p: engine::Pos) -> Option<Vec<Option<ItemStack>>> {
        let key = (p[0], p[1], p[2]);
        if !matches!(
            self.game.world.block_entities.get(&key),
            Some(BlockEntity::Chest(_))
        ) {
            return None;
        }
        match self.game.world.block_entities.remove(&key) {
            Some(BlockEntity::Chest(items)) => Some(items),
            _ => None,
        }
    }
    fn put_items(&mut self, p: engine::Pos, items: Vec<Option<ItemStack>>) {
        let key = (p[0], p[1], p[2]);
        self.game
            .world
            .block_entities
            .insert(key, BlockEntity::Chest(items));
        self.game.world.modified.insert((key.0 >> 4, key.2 >> 4));
    }
    fn custom_name(&self, p: engine::Pos) -> Option<String> {
        self.game.world.block_names.get(&(p[0], p[1], p[2])).cloned()
    }
    fn set_custom_name(&mut self, p: engine::Pos, name: Option<String>) {
        let key = (p[0], p[1], p[2]);
        match name {
            Some(n) => self.game.world.block_names.insert(key, n),
            None => self.game.world.block_names.remove(&key),
        };
    }
    fn openers(&self, p: engine::Pos) -> i32 {
        self.game
            .lid_openers
            .get(&(p[0], p[1], p[2]))
            .copied()
            .unwrap_or(0)
    }
}

/// The DirtChest 9000's guide (`Util.createDirtGuideBook`).
fn guide_book() -> ItemStack {
    let mut book = ItemStack::new("minecraft:written_book", 1);
    let pages = engine::book_pages();
    let page_tags = pages
        .iter()
        .map(|key| {
            horizon_world::nbt::Tag::compound([(
                "translate",
                horizon_world::nbt::Tag::String(key.as_str().into()),
            )])
        })
        .collect();
    book.data_mut().written = Some(horizon_items::item_data::WrittenBook {
        title: engine::BOOK_TITLE.to_owned(),
        author: engine::BOOK_AUTHOR.to_owned(),
        generation: engine::BOOK_GENERATION,
        pages,
        page_tags,
    });
    book
}
fn is_guide_book(stack: &ItemStack) -> bool {
    &*stack.item == "minecraft:written_book"
        && stack
            .data()
            .and_then(|d| d.written.as_ref())
            .is_some_and(|w| w.title == engine::BOOK_TITLE && w.author == engine::BOOK_AUTHOR)
}

/// The block id of one of the chests, for the hooks.
pub(crate) fn ironchest_is_chest(name: &str) -> bool {
    engine::size_of(name).is_some()
}

/// The chests' foundry menu (mods/foundry/API.md section 10, mod networking): the chest's block-entity slots are
/// mirrored to the guest, its lid counts the guest among its openers, and every click runs this
/// mod's own menu code on the server (`ironchest_menu_ops`, through the game's `menu_ops`).
const MENU_KIND: &str = "ironchest:chest";
fn ironchest_menu_claims(game: &mut Game<'_>, _conn: u32, p: [i32; 3]) -> bool {
    if game.ironchest_at(p).is_none() {
        return false;
    }
    engine::ensure_entity(&mut Live { game }, p);
    true
}
fn ironchest_menu_valid(game: &Game<'_>, _conn: u32, p: [i32; 3]) -> bool {
    game.ironchest_at(p).is_some()
}
fn ironchest_register_menu() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    super::foundry::register_menu(super::foundry::MenuKind {
        lid: true,
        claims: Some(ironchest_menu_claims),
        valid: Some(ironchest_menu_valid),
        ..super::foundry::MenuKind::new(MENU_KIND)
    });
}

/// The last shift-click on a chest slot and when, for the double click that moves
/// every stack of its item (`AbstractContainerScreen.mouseReleased`, 250 ms).
static LAST_QUICK_MOVE: Mutex<Option<(Instant, usize)>> = Mutex::new(None);

impl Game<'_> {
    /// The chest at `p`, if one of ours.
    fn ironchest_at(&self, p: [i32; 3]) -> Option<Chest> {
        Chest::parse(&self.res.facts().get(self.world.block(p[0], p[1], p[2])).name)
    }
    /// The chest whose menu is open here (not a station). On a server's client (single player's
    /// window, a LAN guest) it is the chest's foundry menu: the server runs it, this window draws it
    /// from foundry's mirror of its slots and sends the clicks back (foundry's own `menu_ops` hook).
    fn ironchest_open_menu(&self) -> Option<(Chest, (i32, i32, i32))> {
        if self.remote.is_some() {
            let (kind, p) = super::foundry::guest_menu(self)?;
            if kind != MENU_KIND {
                return None;
            }
            return Some((self.ironchest_at(p)?, (p[0], p[1], p[2])));
        }
        if self.station.is_some() || self.screen != Screen::Inventory {
            return None;
        }
        let key = self.container?;
        let chest = self.ironchest_at([key.0, key.1, key.2])?;
        Some((chest, key))
    }

    // ---- placing (trigger_block "minecraft:placed_block", BlockItem.place's end) ----

    pub(super) fn ironchest_placed(&mut self, trigger: &str, pos: [i32; 3], stack: &ItemStack) {
        if trigger != "minecraft:placed_block" {
            return;
        }
        let Some(chest) = self.ironchest_at(pos) else {
            return;
        };
        let Some(created) = engine::placed(&mut Live { game: self }, pos) else {
            return;
        };
        if created {
            // BlockEntity::for_block knows no Iron Chests, so the placing code made no
            // block entity and skipped BlockItem.updateCustomBlockEntityTag: the item's
            // name and block entity data go on the new one now.
            let game_master = self.can_use_game_master_blocks();
            self.apply_placed_item(pos, stack, game_master);
            engine::ensure_entity(&mut Live { game: self }, pos);
        }
        if chest.tier == Tier::Dirt {
            // DirtChestBlockEntity.wasPlaced: an item without data gets the book; one with
            // data only when its BlockEntityTag lacks ChestPlacedAlready (so a renamed
            // DirtChest 9000 gets none, as in the original).
            let placed_before = match stack.data().filter(|d| !d.is_empty()) {
                None => false,
                Some(d) => d
                    .other(horizon_client::block_entity_item::BLOCK_ENTITY_DATA)
                    .is_none_or(|t| t.get("ChestPlacedAlready").is_some()),
            };
            engine::give_guide_book(&mut Live { game: self }, pos, placed_before, guide_book());
        }
    }

    // ---- opening (open_menu's open_station and resolve_container_maps) ----

    /// `AbstractIronChestBlock.use` for one of the chests (open_menu reaches its
    /// `open_station` for a block `BlockEntity::for_block` does not know). The
    /// blocked check (`isChestBlockedAt`) came first, in `chest_menu_blocked`.
    pub(super) fn ironchest_open(&mut self, pos: [i32; 3], name: &str) -> bool {
        let Some(chest) = Chest::parse(name) else {
            return false;
        };
        let key = (pos[0], pos[1], pos[2]);
        engine::ensure_entity(&mut Live { game: self }, pos);
        self.container = Some(key);
        let centre = [
            f64::from(key.0) + 0.5,
            f64::from(key.1) + 0.5,
            f64::from(key.2) + 0.5,
        ];
        self.resolve_container_maps(key, centre);
        // ContainerOpenersCounter.incrementOpeners: the lid, the open sound once.
        let first = self.container_opener_event(key, true);
        self.post_block_game_event(
            "minecraft:container_open",
            pos,
            Some(horizon_client::mobs::Who::Player),
            None,
        );
        self.stats.custom(chest.stat(), 1);
        // guarded_by_piglins holds the six plain chests (IronChestsBlockTags).
        if self.guarded_by_piglins(name) {
            self.anger_piglins(true);
        }
        if first {
            self.container_edge_sound(key, true);
        }
        true
    }
    /// The vanilla chest path took `ironchest:copper_chest` for a 27-slot copper chest
    /// (its name ends like theirs): the chest gets all its 45 slots before the menu
    /// shows, and the statistic open_menu has no name for.
    pub(super) fn ironchest_vanilla_opened(&mut self, key: (i32, i32, i32)) {
        if self.container == Some(key) {
            return;
        }
        let Some(chest) = self.ironchest_at([key.0, key.1, key.2]) else {
            return;
        };
        engine::ensure_entity(&mut Live { game: self }, [key.0, key.1, key.2]);
        self.stats.custom(chest.stat(), 1);
    }
    /// Each tick: the open chest keeps all its slots (one made by `/setblock` or a
    /// vanilla path may have fewer).
    pub(super) fn ironchest_tick(&mut self) {
        ironchest_register_menu();
        if self.remote.is_some() {
            // A guest's window: the slots are the server's (foundry's mirror).
            return;
        }
        if let Some(key) = self.container
            && self.ironchest_at([key.0, key.1, key.2]).is_some()
        {
            engine::ensure_entity(&mut Live { game: self }, [key.0, key.1, key.2]);
        }
    }

    // ---- the lid of trapped chests (TrappedChestBlockEntity.signalOpenCount) ----

    /// `lid_openers_changed` for a trapped chest of ours: the vanilla code updates the
    /// redstone around only for `minecraft:trapped_chest`.
    pub(super) fn ironchest_lid(&mut self, key: (i32, i32, i32), open: bool) -> Option<bool> {
        let chest = self.ironchest_at([key.0, key.1, key.2])?;
        if !chest.trapped {
            return None;
        }
        let count = self.lid_openers.entry(key).or_default();
        let before = *count;
        *count = (before + if open { 1 } else { -1 }).max(0);
        let after = *count;
        if after == 0 {
            self.lid_openers.remove(&key);
        }
        let pos = [key.0, key.1, key.2];
        self.block_entity_event(pos, 1, after);
        self.lan_block_event(pos, 1, after);
        if before != after {
            let state = self.world.block(pos[0], pos[1], pos[2]);
            let p = BlockPos::new(pos[0], pos[1], pos[2]);
            self.with_level(|b, level, ticks| {
                let block = b.block(state);
                b.update_neighbors_at(level, ticks, p, block);
                b.update_neighbors_at(level, ticks, p.offset(0, -1, 0), block);
            });
        }
        Some((before == 0) != (after == 0))
    }

    // ---- LAN guests (ServerPlayer.openMenu over the protocol) ----

    /// A guest opened one of the chests. A guest whose client speaks foundry (single player's own
    /// window, a LAN friend on Horizon with these mods) gets the chest's own menu and screen as a
    /// foundry menu, every tier: the server runs this mod's menu code as the guest
    /// (`ironchest_menu_ops`), the client mirrors the slots. Any other client gets the protocol's
    /// vanilla menus: the iron chest (54) as generic_9x6 and the copper chest (45) as
    /// generic_9x5, with all their slots; the others have no vanilla menu, and the guest is told so.
    pub(super) fn ironchest_lan_open(
        &mut self,
        host: &mut LanHost,
        conn: u32,
        pos: (i32, i32, i32),
    ) -> Option<bool> {
        let chest = self.ironchest_at([pos.0, pos.1, pos.2])?;
        ironchest_register_menu();
        if super::foundry::guest_has_foundry(conn) {
            engine::ensure_entity(&mut Live { game: self }, [pos.0, pos.1, pos.2]);
            return Some(self.foundry_menu_open(host, conn, MENU_KIND, [pos.0, pos.1, pos.2]));
        }
        let menu = match chest.tier {
            Tier::Iron => "minecraft:generic_9x6",
            Tier::Copper => "minecraft:generic_9x5",
            _ => {
                let text = format!(
                    "{}: only the host can open it on Horizon for now",
                    chest.title_fallback()
                );
                host.hub
                    .send_to(conn, &horizon_protocol::packets::play::Clientbound::SystemChat(
                        horizon_protocol::packets::play::SystemChat {
                            content: horizon_protocol::text::literal(&text),
                            overlay: true,
                        },
                    ));
                return Some(false);
            }
        };
        engine::ensure_entity(&mut Live { game: self }, [pos.0, pos.1, pos.2]);
        // Exactly the menu's slots, or nothing: a guest never sees a part of a chest.
        if !matches!(
            self.world.block_entities.get(&pos),
            Some(BlockEntity::Chest(items)) if items.len() == chest.tier.size()
        ) {
            return Some(false);
        }
        let slots = horizon_client::lan::menu::Slots::Block(pos);
        let Some(items) = host.hub.guest(conn).and_then(|g| slots.read(&self.world, g)) else {
            return Some(false);
        };
        self.guest_container_closed(host, conn);
        self.lan_station_closed(host, conn, None);
        let title = match self.world.block_names.get(&pos) {
            Some(name) => horizon_protocol::text::literal(name),
            None => horizon_protocol::text::translatable(&chest.title_key(), &[]),
        };
        host.hub.open_container(conn, menu, title, pos, slots, items);
        Some(true)
    }

    // ---- the menu (IronChestMenu over the verified menu engine) ----

    /// `menu_session` for an open chest: the engine's chest menu with all the chest's
    /// slots (`Kind::Chest(rows)`, nine to a row as the engine counts them; the screen
    /// lays them out by the tier's own row length). The game's own session caps a chest
    /// menu at six rows, which would mix the slots of the bigger chests.
    fn ironchest_session(
        &mut self,
        tier: Tier,
        quick_move_from: Option<usize>,
        run: impl FnOnce(&mut Menu, &Env, &dyn Fn(usize) -> Option<usize>) -> Option<bool>,
    ) -> bool {
        let rows = engine::menu_rows(tier);
        let total = engine::menu_container(tier) + 36;
        let game_of: Vec<Option<usize>> = (0..total).map(|i| engine::to_game(tier, i)).collect();
        let before: Vec<Option<ItemStack>> = game_of
            .iter()
            .map(|g| g.and_then(|g| self.slot_mut(g).clone()))
            .collect();
        let carried = self.carried.clone();
        let offhand = self.offhand.clone();
        let (outcome, after, carried, offhand, dropped, sounds, equipped) = {
            let to_menu = |g: usize| engine::to_menu(tier, g);
            let stations = self.res.stations();
            let env = Env {
                items: self.res.items(),
                recipes: self.res.recipes(),
                brewing: &stations.brewing,
                banners: &stations.banners,
                station_recipes: &stations.recipes,
                enchantments: &stations.enchantments,
                names: &self.pack.names,
                trade: None,
                maps: None,
            };
            let mut menu = Menu::new(Kind::Chest(rows));
            menu.creative = self.creative;
            menu.selected = self.selected;
            menu.levels = self.experience.level;
            for (i, stack) in before.iter().enumerate() {
                menu.set_slot(&env, i, stack.clone());
            }
            // The off hand is part of the inventory, with no slot in this menu.
            menu.set_player_item(40, offhand);
            menu.settle(&env);
            menu.carried = carried;
            menu.drops.clear();
            let outcome = run(&mut menu, &env, &to_menu);
            let after: Vec<Option<ItemStack>> =
                (0..total).map(|i| menu.slot(i).cloned()).collect();
            (
                outcome,
                after,
                menu.carried.take(),
                menu.player_item(40).cloned(),
                std::mem::take(&mut menu.dropped),
                std::mem::take(&mut menu.player_sounds),
                std::mem::take(&mut menu.equipped),
            )
        };
        match outcome {
            None => return false,
            Some(false) => return true,
            Some(true) => {}
        }
        let after = if tier == Tier::Dirt {
            // DirtChestSlot: only dirt, and only the one slot.
            match engine::dirt_settle(&before, &after, quick_move_from) {
                Some(after) => after,
                None => return true,
            }
        } else {
            after
        };
        for sound in sounds {
            self.player_menu_sound(sound);
        }
        for (i, stack) in after.into_iter().enumerate() {
            if let Some(g) = game_of[i] {
                *self.slot_mut(g) = stack;
            }
        }
        self.carried = carried;
        self.offhand = offhand;
        for stack in dropped {
            self.player_drop(stack);
        }
        for (slot, stack) in equipped {
            self.player_equipped(slot, stack.as_ref());
        }
        self.apply_equipment();
        if let Some(key) = self.container {
            self.world.modified.insert((key.0 >> 4, key.2 >> 4));
        }
        true
    }
    /// `menu_ops` on an open chest's menu (clicks, drags, keys).
    pub(super) fn ironchest_menu_ops(&mut self, ops: &[(Option<usize>, i32, Input)]) -> Option<bool> {
        let (chest, _) = self.ironchest_open_menu()?;
        let tier = chest.tier;
        // BundleMouseActions.onSlotClicked: a Shift click or a swap clears the selection.
        for &(slot, _, input) in ops {
            if let (Some(slot), Input::QuickMove | Input::Swap) = (slot, input) {
                self.unselect_bundle(slot);
            }
        }
        // DirtChestSlot.mayPlace: a drag of anything but dirt passes the dirt slot by.
        let carried_dirt = self
            .carried
            .as_ref()
            .is_some_and(|c| engine::dirt_accepts(&c.item));
        let ops: Vec<(Option<usize>, i32, Input)> = ops
            .iter()
            .copied()
            .filter(|&(slot, button, input)| {
                !(tier == Tier::Dirt
                    && input == Input::QuickCraft
                    && button & 3 == 1
                    && slot == Some(engine::CONTAINER_SLOT)
                    && !carried_dirt)
            })
            .collect();
        let quick_move = match ops.as_slice() {
            [(Some(slot), _, Input::QuickMove)] => engine::to_menu(tier, *slot),
            _ => None,
        };
        let handled = self.ironchest_session(tier, quick_move, |menu, env, to_menu| {
            let mut clicks = Vec::new();
            for &(slot, button, input) in &ops {
                let menu_slot = match slot {
                    Some(g) => to_menu(g)? as i32,
                    None => -999,
                };
                clicks.push((menu_slot, button, input));
            }
            for (menu_slot, button, input) in clicks {
                if menu.click(env, menu_slot, button, input).is_err() {
                    // Vanilla's server throws: nothing of this click is kept.
                    return Some(false);
                }
            }
            Some(true)
        });
        // A second shift-click on the slot within 250 ms moves every stack of the item
        // (the game's own double-click step would run its capped session: done here).
        if let [(Some(slot), _, Input::QuickMove)] = ops.as_slice() {
            let now = Instant::now();
            let double = match LAST_QUICK_MOVE.lock() {
                Ok(mut last) => {
                    let double = last.is_some_and(|(at, s)| {
                        s == *slot && now.duration_since(at) < Duration::from_millis(250)
                    });
                    *last = if double { None } else { Some((now, *slot)) };
                    double
                }
                Err(_) => false,
            };
            // The game's own double-click step after this would run its capped session on
            // this menu (for the dirt chest it maps slots twice): the stack it remembers is
            // always taken, so it never runs, and the gathering happens here (the one-slot
            // dirt chest has nothing to gather into).
            let moved = self.last_quick_moved.take();
            if double && let Some(moved) = moved && tier != Tier::Dirt {
                let slot = *slot;
                let from = engine::to_menu(tier, slot);
                self.ironchest_session(tier, from, |menu, env, to_menu| {
                    Some(menu.quick_move_all(env, to_menu(slot)?, &moved).is_ok())
                });
            }
        }
        Some(handled)
    }
    /// The quick craft drag's preview on an open chest.
    pub(super) fn ironchest_drag_view(&mut self) -> Option<Option<DragView>> {
        let (chest, _) = self.ironchest_open_menu()?;
        let Some((ty, slots)) = self.inv_drag.clone() else {
            return Some(None);
        };
        let mut view = None;
        self.ironchest_session(chest.tier, None, |menu, env, to_menu| {
            let menu_slots: Option<Vec<usize>> = slots.iter().map(|&g| to_menu(g)).collect();
            let shown = match menu.quick_craft_preview(env, ty, &menu_slots?) {
                Some((shown, left)) => DragView {
                    slots: slots.iter().copied().zip(shown).collect(),
                    left: Some(left),
                },
                None => DragView {
                    slots: slots.iter().map(|&g| (g, None)).collect(),
                    left: None,
                },
            };
            view = Some(shown);
            Some(false)
        });
        Some(view)
    }
    /// `shouldAddSlotToQuickCraft` on an open chest.
    pub(super) fn ironchest_quick_craft_accepts(
        &mut self,
        slot: usize,
        ty: i32,
        dragged: usize,
    ) -> Option<bool> {
        let (chest, _) = self.ironchest_open_menu()?;
        if chest.tier == Tier::Dirt
            && slot == engine::CONTAINER_SLOT
            && !self
                .carried
                .as_ref()
                .is_some_and(|c| engine::dirt_accepts(&c.item))
        {
            return Some(false);
        }
        let mut accepts = true;
        self.ironchest_session(chest.tier, None, |menu, env, to_menu| {
            accepts = to_menu(slot).is_some_and(|s| menu.quick_craft_accepts(env, s, ty, dragged));
            Some(false)
        });
        Some(accepts)
    }

    // ---- the screen (IronChestScreen) ----

    fn ironchest_gui(&self) -> Option<(Chest, engine::Gui)> {
        let (chest, _) = self.ironchest_open_menu()?;
        Some((chest, chest.tier.gui()))
    }
    /// `(width - imageWidth) / 2`, `(height - imageHeight) / 2`.
    pub(super) fn ironchest_origin(&self, batch: &HudBatch) -> Option<(f32, f32)> {
        let (_, gui) = self.ironchest_gui()?;
        Some(batch.centered_origin(gui.width as f32, gui.height as f32))
    }
    pub(super) fn ironchest_screen_height(&self) -> Option<f32> {
        self.ironchest_gui().map(|(_, gui)| gui.height as f32)
    }
    /// `hasClickedOutside` measures the chest's own image (184 or 238 wide): a click
    /// on its right part is a click in the window, not a throw.
    pub(super) fn ironchest_image(
        &self,
        batch: &HudBatch,
    ) -> Option<horizon_client::container_screen::Image> {
        let (_, gui) = self.ironchest_gui()?;
        let (ox, oy) = batch.centered_origin(gui.width as f32, gui.height as f32);
        Some(horizon_client::container_screen::Image {
            left: ox as i32,
            top: oy as i32,
            width: gui.width,
            height: gui.height,
        })
    }
    /// The slot layout of `IronChestMenu`.
    pub(super) fn ironchest_slot_position(&self, i: usize) -> Option<Option<(f32, f32)>> {
        let (chest, _) = self.ironchest_gui()?;
        Some(engine::slot_position(chest.tier, i).map(|(x, y)| (x as f32, y as f32)))
    }
    /// `IronChestScreen.renderBg` and `renderLabels`.
    pub(super) fn ironchest_draw_screen(&self, batch: &mut HudBatch, ox: f32, oy: f32) -> bool {
        let Some((chest, gui)) = self.ironchest_gui() else {
            return false;
        };
        let atlas = &self.gui.atlas;
        let (w, h) = (gui.width as f32, gui.height as f32);
        if atlas.sprites.contains_key(gui.sprite) {
            batch.sprite_region(
                atlas,
                gui.sprite,
                [ox, oy, w, h],
                gui.texture_width as f32,
                gui.texture_height as f32,
            );
        } else {
            // The mod's screen texture is missing from the GUI atlas: a plain panel.
            batch.fill(atlas, ox, oy, w, h, rgba(198, 198, 198, 255));
        }
        let c = engine::LABEL_COLOR;
        let grey = rgba((c >> 16) as u8, (c >> 8) as u8, c as u8, 255);
        let [(tx, ty), (ix, iy)] = engine::label_positions(chest.tier);
        let title = self.container_title(self.translate(&chest.title_key(), chest.title_fallback()));
        batch.text(atlas, &title, ox + tx as f32, oy + ty as f32, grey, false);
        let inventory = self.translate("container.inventory", "Inventory");
        batch.text(atlas, &inventory, ox + ix as f32, oy + iy as f32, grey, false);
        true
    }
    /// The chest items' model sprites (`{"type": "minecraft:chest", "texture":
    /// "ironchest:<block>"}` reads `entity/chest/ironchest:<block>`), put in the GUI
    /// atlas's names after each (re)load of it.
    pub(super) fn ironchest_sprites(&mut self) {
        let sprites = &mut self.gui.atlas.sprites;
        let first = Chest {
            tier: Tier::Iron,
            trapped: false,
        };
        if sprites.contains_key(&first.item_sprite()) {
            return;
        }
        for chest in Chest::all() {
            if let Some(rect) = sprites.get(&chest.texture()).copied() {
                sprites.insert(chest.item_sprite(), rect);
            }
        }
    }

    // ---- the crystal chest's top items (IronChestRenderer.renderItem) ----

    pub(super) fn ironchest_crystal_items(
        &self,
        camera: glam::DVec3,
        out: &mut Vec<horizon_client::mesher::Vertex>,
    ) {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let rotation = engine::top_item_rotation(millis, self.clock.fraction() as f32);
        for (key, entity) in &self.world.block_entities {
            let BlockEntity::Chest(items) = entity else {
                continue;
            };
            if items.len() != Tier::Crystal.size() {
                continue;
            }
            if self
                .ironchest_at([key.0, key.1, key.2])
                .is_none_or(|c| !c.tier.transparent())
            {
                continue;
            }
            let corner = glam::DVec3::new(f64::from(key.0), f64::from(key.1), f64::from(key.2));
            if (corner + glam::DVec3::splat(0.5)).distance_squared(camera)
                >= engine::TOP_ITEM_DISTANCE * engine::TOP_ITEM_DISTANCE
            {
                continue;
            }
            let light = horizon_client::block_entity_render::vertex_light(
                self.world.light(key.0, key.1, key.2),
            );
            for (j, stack) in engine::top_stacks(items).iter().enumerate() {
                let item = self.resolve_item(
                    stack,
                    horizon_assets::item_model::DisplayContext::None,
                    None,
                    0,
                );
                if item.is_empty() {
                    continue;
                }
                let spot = engine::TOP_ITEM_SPOTS[j];
                let pose = glam::Mat4::from_translation(
                    (corner - camera).as_vec3() + glam::Vec3::from_array(spot),
                ) * glam::Mat4::from_rotation_y(rotation.to_radians())
                    * glam::Mat4::from_scale(glam::Vec3::splat(engine::TOP_ITEM_SCALE));
                self.submit_item_world(&item, pose, light, out);
            }
        }
    }

    // ---- upgrades (ChestUpgradeItem.onItemUseFirst) ----

    /// An upgrade item used on a chest: the blocks' own uses come after.
    pub(super) fn ironchest_upgrade(&mut self, pos: [i32; 3], held: Option<&ItemStack>) -> bool {
        let Some(up) = held.and_then(|s| engine::Upgrade::parse(&s.item)) else {
            return false;
        };
        match engine::upgrade(&mut Live { game: self }, pos, up) {
            UpgradeOutcome::Pass => false,
            UpgradeOutcome::Upgraded { .. } => {
                // if (!instabuild) itemStack.shrink(1)
                self.consume_held();
                true
            }
        }
    }

    // ---- breaking (AbstractIronChestBlock.onRemove's removeAdornments) ----

    pub(super) fn ironchest_breaking(&mut self, pos: [i32; 3]) {
        if self.ironchest_at(pos).is_none_or(|c| c.tier != Tier::Dirt) {
            return;
        }
        // Only when the break goes ahead (break_block_as_player's own checks).
        if !self.may_build()
            || !self
                .res
                .items()
                .can_destroy_block(self.held(), self.creative)
        {
            return;
        }
        engine::strip_guide_book(&mut Live { game: self }, pos, is_guide_book);
    }
}

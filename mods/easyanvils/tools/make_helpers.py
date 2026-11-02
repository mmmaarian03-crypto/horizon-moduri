#!/usr/bin/env python3
"""Writes the glue and the small helper files of the mixin bundle (kept as one script so they stay together).
  py -I -X utf8 tools/make_helpers.py && py -I -X utf8 tools/sync.py
"""
import pathlib

R = pathlib.Path(__file__).resolve().parents[1]
M = R / "mixins" / "easyanvils"
FILES = {
    "tools/glue.rs": r'''
// ---- the game library's side (inside `horizon_client::stations`) ----

struct Live<'a> {
    e: &'a crate::enchant::Enchantments,
    items: &'a crate::items::Items,
}
impl Rules for Live<'_> {
    fn can_enchant(&self, id: &str, item: &str) -> bool {
        self.e.can_enchant(self.items, id, item)
    }
    fn compatible(&self, a: &str, b: &str) -> bool {
        self.e.compatible(a, b)
    }
    fn max_level(&self, id: &str) -> i32 {
        self.e.get(id).map_or(0, |d| d.max_level as i32)
    }
    fn anvil_cost(&self, id: &str) -> i32 {
        self.e.get(id).map_or(1, |d| d.anvil_cost as i32)
    }
    fn valid_repair(&self, stack: &Stack, material: &Stack) -> bool {
        self.e.props(&stack.item).is_some_and(|p| {
            crate::enchant::Enchantments::item_in(self.items, &p.repairable, &material.item)
        })
    }
}

fn view(items: &crate::items::Items, s: &crate::items::ItemStack) -> Stack {
    Stack {
        item: s.item.to_string(),
        count: s.count,
        damage: s.damage as i32,
        max_damage: if super::is_damageable(items, s) {
            items.get(&s.item).map_or(0, |i| i.max_damage as i32)
        } else {
            0
        },
        repair_cost: s.repair_cost() as i32,
        custom_name: s.custom_name().map(str::to_owned),
        enchants: s
            .crafting_enchantments()
            .iter()
            .map(|(id, l)| (id.to_string(), *l as i32))
            .collect(),
    }
}

/// The result stack: the left item with what the rules changed.
fn apply(base: &crate::items::ItemStack, out: &Stack) -> crate::items::ItemStack {
    let mut r = base.clone();
    r.damage = out.damage.max(0) as u32;
    r.data_mut().custom_name = out.custom_name.clone();
    r.data_mut().repair_cost = out.repair_cost.max(0) as u32;
    r.set_crafting_enchantments(
        out.enchants
            .iter()
            .map(|(id, l)| (std::sync::Arc::from(id.as_str()), (*l).max(0) as u32))
            .collect(),
    );
    r.tidy();
    r
}

/// `stations::anvil_result` with Easy Anvils' rules (`ModAnvilMenu.createResult`).
pub fn result(
    e: &crate::enchant::Enchantments,
    items: &crate::items::Items,
    input: Option<&crate::items::ItemStack>,
    addition: Option<&crate::items::ItemStack>,
    name: Option<&str>,
    hover_name: &str,
    creative: bool,
) -> super::AnvilResult {
    let live = Live { e, items };
    let left = input.map(|s| view(items, s));
    let right = addition.map(|s| view(items, s));
    let out = anvil_result(&live, &Config::MARIAN, left.as_ref(), right.as_ref(), name, hover_name, creative);
    super::AnvilResult {
        result: match (&out.result, input) {
            (Some(r), Some(base)) => Some(apply(base, r)),
            _ => None,
        },
        cost: out.cost,
        repair_item_count_cost: out.repair_item_count_cost,
        only_renaming: out.only_renaming,
    }
}
''',
    "mixins/easyanvils/result.rs": r'''fn easyanvils_result(
    e: &Enchantments,
    items: &Items,
    input: Option<&ItemStack>,
    addition: Option<&ItemStack>,
    name: Option<&str>,
    hover_name: &str,
    creative: bool,
) -> ::core::ops::ControlFlow<AnvilResult> {
    ::core::ops::ControlFlow::Break(easyanvils::result(
        e, items, input, addition, name, hover_name, creative,
    ))
}
''',
    "mixins/easyanvils/pickup.rs": r'''fn easyanvils_pickup(menu: &Menu, s: usize) -> ::core::ops::ControlFlow<bool> {
    // AnvilMenu.mayPickup of Easy Anvils: free renames (cost 0) may be taken.
    if menu.kind != Kind::Anvil {
        return ::core::ops::ControlFlow::Continue(());
    }
    match menu.slots[s].role {
        Role::CombineResult => ::core::ops::ControlFlow::Break(
            (menu.creative || menu.levels >= menu.data[0]) && menu.data[0] >= 0,
        ),
        _ => ::core::ops::ControlFlow::Continue(()),
    }
}
''',
    "mixins/easyanvils/take.rs": r'''fn easyanvils_take(game: &Game<'_>) -> ::core::ops::ControlFlow<bool> {
    let Some(s) = game.station.as_ref() else {
        return ::core::ops::ControlFlow::Continue(());
    };
    if s.kind != Kind::Anvil {
        return ::core::ops::ControlFlow::Continue(());
    }
    ::core::ops::ControlFlow::Break(
        (game.creative || game.experience.level >= s.anvil.cost) && s.anvil.cost >= 0,
    )
}
''',
    "mixins/easyanvils/expensive.rs": r'''fn easyanvils_expensive(
    game: &Game<'_>,
    key: &str,
    fallback: &str,
) -> ::core::ops::ControlFlow<String> {
    // The screen of Easy Anvils shows the cost, not "Too Expensive!", when the limit is off.
    let _ = fallback;
    if key != "container.repair.expensive" {
        return ::core::ops::ControlFlow::Continue(());
    }
    let Some(s) = game.station.as_ref() else {
        return ::core::ops::ControlFlow::Continue(());
    };
    if s.kind != Kind::Anvil || s.anvil.cost < 40 {
        return ::core::ops::ControlFlow::Continue(());
    }
    ::core::ops::ControlFlow::Break(
        horizon_client::lang::get_or("container.repair.cost", "Enchantment Cost: %1$s")
            .replace("%1$s", &s.anvil.cost.to_string()),
    )
}
''',
    "mixins/easyanvils/typing.rs": r'''fn easyanvils_typing(edit: &mut EditBox, text: &str) -> ::core::ops::ControlFlow<()> {
    // The rename box of the anvil (103 x 12, no border, 50 characters): `§` formatting codes
    // are typed and kept (`renaming_supports_formatting`).
    if edit.max_length != 50 || edit.bordered || edit.multiline || edit.width != 103.0 {
        return ::core::ops::ControlFlow::Continue(());
    }
    let (a, b) = edit.selected();
    let filtered: String = text.chars().filter(|&c| c >= ' ' && c != '\u{7f}').collect();
    let room = edit.max_length.saturating_sub(char_len(&edit.value) - (b - a));
    let insert: String = filtered.chars().take(room).collect();
    let (ba, bb) = (byte_index(&edit.value, a), byte_index(&edit.value, b));
    edit.value.replace_range(ba..bb, &insert);
    let pos = a + char_len(&insert);
    edit.set_cursor(pos, false);
    ::core::ops::ControlFlow::Break(())
}
''',
    "mixins/easyanvils/opened.rs": r'''fn easyanvils_opened(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    name: &str,
    value: bool,
) -> bool {
    game.easyanvils_opened(pos, name, value);
    value
}
''',
    "mixins/easyanvils/close.rs": r'''fn easyanvils_close(game: &mut crate::game::Game<'_>) {
    game.easyanvils_close();
}
''',
    "mixins/easyanvils/tick.rs": r'''fn easyanvils_tick(game: &mut crate::game::Game<'_>) {
    game.easyanvils_tick();
}
''',
    "mixins/easyanvils/save.rs": r'''fn easyanvils_save(game: &mut crate::game::Game<'_>) {
    game.easyanvils_save();
}
''',
    "mixins/easyanvils/open.rs": r'''fn easyanvils_open() {
    crate::game::Game::easyanvils_open();
}
''',
    "mixins/easyanvils/geometry.rs": r'''fn easyanvils_geometry(
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    game.easyanvils_geometry(camera, out);
}
''',
    "mixins/easyanvils/guest.rs": r'''fn easyanvils_guest(
    game: &mut Game<'_>,
    host: &mut LanHost,
    conn: u32,
    gs: &mut GuestStation,
) -> ::core::ops::ControlFlow<()> {
    // A guest's anvil take: as the game does it, with Easy Anvils' wear.
    if gs.kind == StationKind::Smithing {
        return ::core::ops::ControlFlow::Continue(());
    }
    let change = std::mem::take(&mut gs.menu.level_change);
    if change != 0 {
        game.as_guest(host, conn, |g| g.give_levels(change));
    }
    if !std::mem::take(&mut gs.menu.combined) {
        return ::core::ops::ControlFlow::Break(());
    }
    Game::easyanvils_took(change);
    let creative = gs.menu.creative;
    game.easyanvils_used(gs.pos, creative, |g, p, c| g.anvil_used(p, c));
    ::core::ops::ControlFlow::Break(())
}
''',
    "mixins/easyanvils/use-block.rs": r'''fn easyanvils_use(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    held: Option<&ItemStack>,
) -> ::core::ops::ControlFlow<bool> {
    if game.easyanvils_use(pos, held) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
''',
    "mixins/easyanvils/dispense.rs": r'''fn easyanvils_dispense(
    game: &mut Game<'_>,
    pos: Pos,
    facing: Direction,
    stack: &ItemStack,
) -> ::core::ops::ControlFlow<Result<Option<ItemStack>>> {
    let s = step(facing);
    match game.easyanvils_dispense([pos.0, pos.1, pos.2], s, stack) {
        Some((left, event)) => {
            game.dispenser_event(event, pos, 0);
            game.dispenser_event(2000, pos, data_value(facing));
            ::core::ops::ControlFlow::Break(Ok(left))
        }
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
''',
}
for name, text in FILES.items():
    p = R / name
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8", newline="\n")
print(len(FILES), "files")

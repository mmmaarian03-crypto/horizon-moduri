
// ---- the part that needs the game library's types (this file follows src/lib.rs in shared.rs) ----

use crate::items::ItemStack;
use horizon_world::nbt::{JavaString, Tag};
use std::sync::RwLock;

const CUSTOM_DATA: &str = "minecraft:custom_data";
const MODEL_DATA: &str = "minecraft:custom_model_data";

/// A plain `Component.Serializer.toJson(Component.literal(name))`.
fn name_json(name: &str) -> String {
    let mut s = String::from("\"");
    for c in name.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            c if (c as u32) < 0x20 => s.push_str(&format!("\\u{:04x}", c as u32)),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}
/// The text of a component's JSON, when it is a string or has a `text`.
fn name_of_json(json: &str) -> String {
    fn unquote(s: &str) -> Option<String> {
        let s = s.trim().strip_prefix('"')?;
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => return Some(out),
                '\\' => match chars.next()? {
                    'n' => out.push('\n'),
                    'u' => {
                        let hex: String = chars.by_ref().take(4).collect();
                        out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                    }
                    c => out.push(c),
                },
                c => out.push(c),
            }
        }
        None
    }
    if let Some(name) = unquote(json) {
        return name;
    }
    if let Some(at) = json.find("\"text\"")
        && let Some(colon) = json[at..].find(':')
        && let Some(name) = unquote(&json[at + colon + 1..])
    {
        return name;
    }
    json.to_owned()
}

/// `Frequency.write_internal`: the colours as wool metas, the owner as a UUID and the player's name.
pub fn frequency_tag(f: &Frequency) -> Tag {
    let mut entries = vec![
        ("left", Tag::Int(f.left.wool_meta())),
        ("middle", Tag::Int(f.middle.wool_meta())),
        ("right", Tag::Int(f.right.wool_meta())),
    ];
    if let Some(o) = &f.owner {
        entries.push(("owner", Tag::IntArray(o.uuid.to_vec())));
        entries.push(("owner_name", Tag::String(name_json(&o.name).as_str().into())));
    }
    Tag::compound(entries)
}

/// `Frequency.read_internal` (a colour out of range is white, as a missing one is).
pub fn frequency_from_tag(tag: &Tag) -> Frequency {
    let colour = |key: &str| {
        match tag.get(key) {
            Some(Tag::Int(i)) => Colour::from_wool_meta(*i),
            Some(Tag::Byte(i)) => Colour::from_wool_meta(i32::from(*i)),
            Some(Tag::Short(i)) => Colour::from_wool_meta(i32::from(*i)),
            _ => None,
        }
        .unwrap_or(Colour::White)
    };
    let mut f = Frequency::new(colour("left"), colour("middle"), colour("right"));
    if let Some(Tag::IntArray(uuid)) = tag.get("owner")
        && let [a, b, c, d] = uuid[..]
    {
        // `hasOwner` needs both the UUID and the name.
        if let Some(Tag::String(name)) = tag.get("owner_name") {
            f.owner = Some(Owner {
                uuid: [a, b, c, d],
                name: name_of_json(&name.to_string_lossy()),
            });
        }
    }
    f
}

/// `Frequency.readFromStack`: the stack's `Frequency` in its custom data (the default when none).
pub fn read_frequency(stack: &ItemStack) -> Frequency {
    stack
        .data()
        .and_then(|d| d.other(CUSTOM_DATA))
        .and_then(|t| t.get("Frequency"))
        .map(frequency_from_tag)
        .unwrap_or_default()
}

/// `Frequency.writeToStack`; the pouch also gets the custom model data its item model reads
/// (strings: left, middle, right colour; flags: owned, open).
pub fn write_frequency(stack: &mut ItemStack, f: &Frequency) {
    let mut custom = match stack.data().and_then(|d| d.other(CUSTOM_DATA)) {
        Some(Tag::Compound(map)) => map.clone(),
        _ => Default::default(),
    };
    custom.insert(JavaString::from("Frequency"), frequency_tag(f));
    let pouch = &*stack.item == POUCH;
    let data = stack.data_mut();
    data.set_other(CUSTOM_DATA, Some(Tag::Compound(custom)));
    if pouch {
        let strings = f
            .model_strings()
            .iter()
            .map(|s| Tag::String((*s).into()))
            .collect();
        data.set_other(
            MODEL_DATA,
            Some(Tag::compound([
                ("strings", Tag::List(strings)),
                (
                    "flags",
                    Tag::List(vec![Tag::Byte(i8::from(f.has_owner())), Tag::Byte(0)]),
                ),
            ])),
        );
    }
}

/// The result of the mod's crafting recipes for a grid, when it is one of them: the three
/// `create_recipe`s (the colour of the wool) and the three `recolour_recipe`s (dyes above the item).
/// Called first by the recipe search; `None` leaves it to the game's own.
pub fn crafting(grid: &[Option<ItemStack>], size: usize) -> Option<(&'static str, ItemStack)> {
    if size != 3 || grid.len() != 9 {
        return None;
    }
    let cells: Vec<Option<&str>> = grid.iter().map(|c| c.as_ref().map(|s| &*s.item)).collect();
    if let Some(item) = create_recipe(&cells) {
        let mut stack = ItemStack::new(item, 1);
        write_frequency(&mut stack, &create_frequency(&cells));
        return Some((
            match item {
                CHEST => "enderstorage:ender_chest",
                TANK => "enderstorage:ender_tank",
                _ => "enderstorage:ender_pouch",
            },
            stack,
        ));
    }
    for (item, name) in [
        (CHEST, "enderstorage:recolour_ender_chest"),
        (TANK, "enderstorage:recolour_ender_tank"),
        (POUCH, "enderstorage:recolour_ender_pouch"),
    ] {
        if recolour_matches(&cells, item) {
            let freq = recolour_result(&cells, item, |i| {
                grid[i].as_ref().map(read_frequency).unwrap_or_default()
            });
            let mut stack = ItemStack::new(item, 1);
            write_frequency(&mut stack, &freq);
            return Some((name, stack));
        }
    }
    None
}

/// `appendHoverText`: the three colours (the owner's name goes after them: the game's tooltip
/// hook gives one line).
pub fn tooltip(stack: &ItemStack, translate: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    if !matches!(&*stack.item, CHEST | TANK | POUCH) {
        return None;
    }
    let f = read_frequency(stack);
    let [a, b, c] = f.tooltip_keys().map(|k| translate(&k).unwrap_or(k));
    Some(match &f.owner {
        Some(o) => format!("{a}/{b}/{c} ({})", o.name),
        None => format!("{a}/{b}/{c}"),
    })
}

// ---- the chest as the block entity renderer and the lid animation know it ----

/// `RenderTileEnderChest`'s texture (`textures/enderchest.png`, the vanilla chest's layout), where the
/// carried content keeps it so that the GUI atlas packs it.
pub const CHEST_TEXTURE: &str = "enderstorage:block/model/ender_chest";
/// The facings in the order of the renderer's y rotation (south 0, west 90, north 180, east 270).
pub const FACINGS: [&str; 4] = ["south", "west", "north", "east"];

/// Each block state's code: 0 not the chest, else 1 + the facing's index.
static STATES: RwLock<Vec<u8>> = RwLock::new(Vec::new());

pub fn state_code(name: &str, facing: Option<&str>) -> u8 {
    if name != CHEST {
        return 0;
    }
    let f = FACINGS.iter().position(|x| Some(*x) == facing).unwrap_or(2);
    1 + f as u8
}
pub fn set_states(codes: Vec<u8>) {
    if let Ok(mut s) = STATES.write() {
        *s = codes;
    }
}
/// The facing's index when the state is the chest.
pub fn chest_state(state: u32) -> Option<usize> {
    let s = STATES.read().ok()?;
    match s.get(state as usize).copied() {
        None | Some(0) => None,
        Some(c) => Some(usize::from(c) - 1),
    }
}

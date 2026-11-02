// Capsule inside Horizon's game (nightly 142388e1c). The rules are the engine (src/lib.rs); this part reads and
// changes the world. Everything that changes the world runs where the world is: on the integrated server in single
// player (the window is its guest; the right click arrives in the guest's turn), never on a remote client.
// Templates are vanilla structure files in `<world>/capsules/<name>.nbt`, like the original's CapsuleTemplateManager.
#[allow(unused_imports)]
use super::{Game, Result, lan::LanHost};
#[allow(dead_code)]
mod engine {
    include!("capsule_engine.rs");
}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
#[allow(dead_code)]
const MOD_ID: &str = "capsule";

include!("kit_blocks.rs");
include!("kit_notice.rs");
include!("kit_sound.rs");
include!("kit_state.rs");

use horizon_client::items::ItemStack;
use horizon_core::position::BlockPos;
use horizon_world::nbt::{Compound, JavaString, Tag};
use std::collections::{BTreeMap, HashMap, HashSet};

const CAPSULE: &str = "capsule:capsule";
const MARKER: &str = "capsule:capsulemarker";
const CLICK_ON: &str = "minecraft:block.stone_button.click_on";
const CLICK_OFF: &str = "minecraft:block.stone_button.click_off";
const ARROW_SHOOT: &str = "minecraft:entity.arrow.shoot";
/// `CapsuleLootEntry.COLOR_PALETTE`.
const PALETTE: [i32; 11] = [0xCCCCCC, 0x549b57, 0xe08822, 0x5e8eb7, 0x6c6c6c, 0xbd5757, 0x99c33d, 0x4a4cba, 0x7b2e89, 0x95d5e7, 0xffffff];

thread_local! {
    /// The world's counter and the players who got their starter (`CapsuleSavedData`, `capsule:receivedStarter`):
    /// `<world>/data/capsule/capsule.txt`, for the whole world (all dimensions).
    static SAVED: std::cell::RefCell<Option<(std::path::PathBuf, engine::State2, bool)>> = const { std::cell::RefCell::new(None) };
    /// Thrown capsules: the last horizontal speed (for the wall hit) and whether the lifetime was extended.
    static THROWN: std::cell::RefCell<HashMap<u32, ([f64; 2], bool)>> = std::cell::RefCell::new(HashMap::new());
}

// ---- the stack ----

fn cd(stack: &ItemStack) -> Option<&Compound> {
    match stack.data()?.other("minecraft:custom_data")? {
        Tag::Compound(c) => Some(c),
        _ => None,
    }
}
fn int(c: &Compound, k: &str) -> Option<i32> {
    match c.get(&JavaString::from(k))? {
        Tag::Int(v) => Some(*v),
        Tag::Short(v) => Some(i32::from(*v)),
        Tag::Byte(v) => Some(i32::from(*v)),
        Tag::Long(v) => Some(*v as i32),
        _ => None,
    }
}
fn long(c: &Compound, k: &str) -> Option<i64> {
    match c.get(&JavaString::from(k))? {
        Tag::Long(v) => Some(*v),
        Tag::Int(v) => Some(i64::from(*v)),
        _ => None,
    }
}
fn text(c: &Compound, k: &str) -> Option<String> {
    match c.get(&JavaString::from(k))? {
        Tag::String(s) => Some(s.to_string_lossy()),
        _ => None,
    }
}
fn sub<'a>(c: &'a Compound, k: &str) -> Option<&'a Compound> {
    match c.get(&JavaString::from(k))? {
        Tag::Compound(s) => Some(s),
        _ => None,
    }
}
fn s(v: &str) -> Tag {
    Tag::String(v.into())
}

/// The capsule's tag (1.20.1 names), as the engine sees it.
fn read(stack: &ItemStack) -> engine::Capsule {
    let empty = Compound::new();
    let c = cd(stack).unwrap_or(&empty);
    let spawn = sub(c, "spawnPosition").and_then(|p| {
        Some(([int(p, "x")?, int(p, "y")?, int(p, "z")?], text(p, "dim").unwrap_or_else(|| "minecraft:overworld".into())))
    });
    engine::Capsule {
        state: int(c, "state").unwrap_or(0),
        size: int(c, "size").unwrap_or(1),
        material_color: int(c, "color"),
        base_color: stack.data().and_then(|d| d.dyed_color),
        structure: text(c, "structureName"),
        label: text(c, "label"),
        author: text(c, "author"),
        overpowered: int(c, "overpowered") == Some(1),
        one_use: int(c, "oneUse").is_some_and(|v| v != 0),
        is_reward: int(c, "isReward").is_some_and(|v| v != 0),
        blueprint: c.contains_key(&JavaString::from("sourceInventory")),
        spawn,
        timer_start: sub(c, "activetimer").and_then(|t| long(t, "starttime")),
        deploy_at: long(c, "deployAt"),
        undeploy_at: long(c, "undeployAt").unwrap_or(0),
        upgraded: int(c, "upgraded").unwrap_or(0),
        y_offset: int(c, "yOffset").unwrap_or(0),
    }
}

/// Writes the engine's capsule back into the stack's tag (unknown keys stay), plus what the 26.3 item model reads
/// (`custom_model_data`: the state's model, the material colour, the third layer's colour) and the name
/// (`item_name`, `CapsuleItem.getName`).
fn write(game: &Game<'_>, stack: &mut ItemStack, c: &engine::Capsule) {
    let mut tag = cd(stack).cloned().unwrap_or_default();
    let mut put = |k: &str, v: Option<Tag>| match v {
        Some(v) => {
            tag.insert(JavaString::from(k), v);
        }
        None => {
            tag.remove(&JavaString::from(k));
        }
    };
    put("state", Some(Tag::Int(c.state)));
    put("size", Some(Tag::Int(c.size())));
    put("color", c.material_color.map(Tag::Int));
    put("structureName", c.structure.as_deref().map(s));
    put("label", c.label.as_deref().map(s));
    put("author", c.author.as_deref().map(s));
    put("overpowered", c.overpowered.then_some(Tag::Byte(1)));
    put("oneUse", c.one_use.then_some(Tag::Byte(1)));
    put("isReward", c.is_reward.then_some(Tag::Byte(1)));
    put(
        "spawnPosition",
        c.spawn.as_ref().map(|(p, d)| {
            Tag::compound([("x", Tag::Int(p[0])), ("y", Tag::Int(p[1])), ("z", Tag::Int(p[2])), ("dim", s(d))])
        }),
    );
    put("activetimer", c.timer_start.map(|t| Tag::compound([("starttime", Tag::Long(t))])));
    put("deployAt", c.deploy_at.map(Tag::Long));
    put("undeployAt", (c.undeploy_at != 0).then_some(Tag::Long(c.undeploy_at)));
    put("upgraded", (c.upgraded != 0).then_some(Tag::Int(c.upgraded)));
    put("yOffset", (c.y_offset != 0).then_some(Tag::Int(c.y_offset)));
    let d = stack.data_mut();
    d.set_other("minecraft:custom_data", Some(Tag::Compound(tag)));
    // `getColorFromItemstack`: layer 1 the material colour (a deployed blueprint 0x7CC4EA), layer 2 white (a
    // blueprint 0x3BB3FC); layer 0 is the dye (`display.color`, now `dyed_color`).
    let deployed_blueprint = c.blueprint && c.has(engine::State::Deployed);
    let model = if deployed_blueprint { "deployed" } else { c.state().model() };
    let layer1 = if deployed_blueprint { 0x7CC4EA } else { c.material_color.unwrap_or(0xFFFFFF) };
    let layer2 = if c.blueprint { 0x3BB3FC } else { 0xFFFFFF };
    d.set_other(
        "minecraft:custom_model_data",
        Some(Tag::compound([
            ("strings", Tag::List(vec![s(model)])),
            ("colors", Tag::List(vec![Tag::Int(layer1), Tag::Int(layer2)])),
        ])),
    );
    d.set_other("minecraft:item_name", Some(name_component(game, c)));
}

/// `CapsuleItem.getName` as a text component: `[state ][content ]name`.
fn name_component(_game: &Game<'_>, c: &engine::Capsule) -> Tag {
    let (state, content, name) = engine::name_parts(c);
    let tr = |k: &str| Tag::compound([("translate", s(k))]);
    let mut extra = Vec::new();
    if let Some(st) = state {
        extra.push(Tag::compound([("translate", s(st)), ("color", s("dark_green"))]));
        extra.push(s(" "));
    }
    match content {
        Some(engine::Content::Label(l)) => {
            extra.push(s("«"));
            extra.push(Tag::compound([("text", s(&l)), ("italic", Tag::Byte(1))]));
            extra.push(s("»"));
            extra.push(s(" "));
        }
        Some(engine::Content::Empty) => {
            extra.push(tr("items.capsule.content_empty"));
            extra.push(s(" "));
        }
        Some(engine::Content::Unlabeled) => {
            extra.push(tr("items.capsule.content_unlabeled"));
            extra.push(s(" "));
        }
        None => {}
    }
    extra.push(tr(name));
    Tag::compound([("text", s("")), ("extra", Tag::List(extra))])
}

fn is_capsule(stack: &ItemStack) -> bool {
    &*stack.item == CAPSULE
}

fn dir_of(name: &str) -> Option<[i32; 3]> {
    Some(match name {
        "down" => [0, -1, 0],
        "up" => [0, 1, 0],
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        _ => return None,
    })
}

/// What a template holds, read back.
struct Template {
    size: [i32; 3],
    /// (relative position, state, block entity nbt without x/y/z).
    blocks: Vec<([i32; 3], horizon_world::registry::BlockStateId, Option<Compound>)>,
    /// (relative position, block position, nbt).
    entities: Vec<([f64; 3], [i32; 3], Compound)>,
    occupied: Option<BTreeMap<engine::Pos, String>>,
    author: Option<String>,
}

impl Game<'_> {
    // ---- files ----

    fn capsule_dir(&self) -> Option<std::path::PathBuf> {
        (self.remote.is_none() && !self.world_root.as_os_str().is_empty()).then(|| self.world_root.join("capsules"))
    }
    fn capsule_template_path(&self, name: &str) -> Option<std::path::PathBuf> {
        // Names come from `getUniqueName` (id characters only): never a path out of the folder.
        if name.is_empty() || name.contains("..") || name.contains(':') || name.starts_with('/') || name.contains('\\') {
            return None;
        }
        Some(self.capsule_dir()?.join(format!("{name}.nbt")))
    }
    fn capsule_saved<R>(&mut self, f: impl FnOnce(&mut engine::State2) -> R) -> Option<R> {
        let root = self.world_root.clone();
        if self.remote.is_some() || root.as_os_str().is_empty() {
            return None;
        }
        let fresh = SAVED.with(|cell| cell.borrow().as_ref().is_none_or(|(r, _, _)| *r != root));
        if fresh {
            let text = kit_state::read(self, "capsule.txt");
            SAVED.with(|cell| *cell.borrow_mut() = Some((root.clone(), engine::State2::load(text), false)));
        }
        SAVED.with(|cell| {
            let mut cell = cell.borrow_mut();
            let (_, st, dirty) = cell.as_mut()?;
            let before = st.clone();
            let r = f(st);
            if *st != before {
                *dirty = true;
            }
            Some(r)
        })
    }
    /// HEAD `save_game_metadata`.
    pub(super) fn capsule_save(&mut self) {
        let snapshot = SAVED.with(|cell| {
            let mut cell = cell.borrow_mut();
            match cell.as_mut() {
                Some((root, st, dirty)) if *dirty && *root == self.world_root => {
                    *dirty = false;
                    Some(st.save())
                }
                _ => None,
            }
        });
        if let Some(text) = snapshot {
            kit_state::write(self, "capsule.txt", &text);
        }
    }
    /// HEAD `load_world_state`: another world opens.
    pub(super) fn capsule_world_opened() {
        SAVED.with(|c| *c.borrow_mut() = None);
        THROWN.with(|t| t.borrow_mut().clear());
    }

    /// Reads a template file (vanilla structure NBT: `Name`/`Properties` or `id`/`properties` palettes).
    fn capsule_read_template(&self, path: &std::path::Path) -> Option<Template> {
        let tag = horizon_client::save::read_dat(path)?;
        let Tag::Compound(root) = tag else { return None };
        let list = |c: &Compound, k: &str| match c.get(&JavaString::from(k)) {
            Some(Tag::List(l)) => l.clone(),
            _ => Vec::new(),
        };
        let ints = |t: Option<&Tag>| -> [i32; 3] {
            match t {
                Some(Tag::List(l)) if l.len() == 3 => std::array::from_fn(|i| match &l[i] {
                    Tag::Int(v) => *v,
                    _ => 0,
                }),
                Some(Tag::IntArray(v)) if v.len() == 3 => [v[0], v[1], v[2]],
                _ => [0; 3],
            }
        };
        let registry = self.pack.assets.registry;
        let mut palette = Vec::new();
        let first_palette = match root.get(&JavaString::from("palettes")) {
            Some(Tag::List(ps)) => match ps.first() {
                Some(Tag::List(p)) => p.clone(),
                _ => Vec::new(),
            },
            _ => list(&root, "palette"),
        };
        for entry in first_palette {
            let Tag::Compound(e) = entry else { continue };
            let name = text(&e, "Name").or_else(|| text(&e, "id")).unwrap_or_else(|| "minecraft:air".into());
            let props = sub(&e, "Properties").or_else(|| sub(&e, "properties"));
            let id = registry.default_state(&name).and_then(|d| {
                let mut st = registry.state(d)?.clone();
                if let Some(props) = props {
                    for (k, v) in props {
                        let (k, Tag::String(v)) = (k.to_string_lossy(), v) else { continue };
                        if st.properties.contains_key(&k) {
                            st.properties.insert(k, v.to_string_lossy());
                        }
                    }
                }
                registry.resolve(&st).ok()
            });
            palette.push(id.unwrap_or(self.res.facts().air));
        }
        let mut blocks = Vec::new();
        for b in list(&root, "blocks") {
            let Tag::Compound(b) = b else { continue };
            let Some(state) = int(&b, "state").and_then(|i| palette.get(i as usize).copied()) else { continue };
            let nbt = sub(&b, "nbt").cloned();
            blocks.push((ints(b.get(&JavaString::from("pos"))), state, nbt));
        }
        let mut entities = Vec::new();
        for e in list(&root, "entities") {
            let Tag::Compound(e) = e else { continue };
            let pos = match e.get(&JavaString::from("pos")) {
                Some(Tag::List(l)) if l.len() == 3 => std::array::from_fn(|i| match &l[i] {
                    Tag::Double(v) => f64::from_bits(*v),
                    _ => 0.0,
                }),
                _ => [0.0; 3],
            };
            let Some(nbt) = sub(&e, "nbt").cloned() else { continue };
            entities.push((pos, ints(e.get(&JavaString::from("blockPos"))), nbt));
        }
        let occupied = match root.get(&JavaString::from("capsule_occupiedSources")) {
            Some(Tag::List(l)) => Some(
                l.iter()
                    .filter_map(|t| match t {
                        Tag::Compound(c) => Some((engine::pos_of_long(long(c, "pos")?), text(c, "block")?)),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        };
        Some(Template {
            size: ints(root.get(&JavaString::from("size"))),
            blocks,
            entities,
            occupied,
            author: text(&root, "author"),
        })
    }

    /// Writes a template (`CapsuleTemplate.save`): size, one palette, blocks, entities, DataVersion, the occupied
    /// positions of a deployment (by block name), author.
    fn capsule_write_template(&self, path: &std::path::Path, t: &Template) -> bool {
        let registry = self.pack.assets.registry;
        let mut index: HashMap<u32, i32> = HashMap::new();
        let mut palette = Vec::new();
        let mut blocks = Vec::new();
        for (pos, state, nbt) in &t.blocks {
            let i = *index.entry(state.0).or_insert_with(|| {
                let st = registry.state(*state);
                let mut e = Compound::new();
                e.insert("Name".into(), s(st.map_or("minecraft:air", |b| b.name.as_str())));
                if let Some(st) = st
                    && !st.properties.is_empty()
                {
                    let mut p = Compound::new();
                    for (k, v) in &st.properties {
                        p.insert(k.as_str().into(), s(v));
                    }
                    e.insert("Properties".into(), Tag::Compound(p));
                }
                palette.push(Tag::Compound(e));
                palette.len() as i32 - 1
            });
            let mut b = Compound::new();
            b.insert("pos".into(), Tag::List(pos.iter().map(|v| Tag::Int(*v)).collect()));
            b.insert("state".into(), Tag::Int(i));
            if let Some(n) = nbt {
                b.insert("nbt".into(), Tag::Compound(n.clone()));
            }
            blocks.push(Tag::Compound(b));
        }
        let entities = t
            .entities
            .iter()
            .map(|(pos, bp, nbt)| {
                Tag::compound([
                    ("pos", Tag::List(pos.iter().map(|v| Tag::Double(v.to_bits())).collect())),
                    ("blockPos", Tag::List(bp.iter().map(|v| Tag::Int(*v)).collect())),
                    ("nbt", Tag::Compound(nbt.clone())),
                ])
            })
            .collect();
        let mut root = Compound::new();
        root.insert("size".into(), Tag::List(t.size.iter().map(|v| Tag::Int(*v)).collect()));
        root.insert("palette".into(), Tag::List(palette));
        root.insert("blocks".into(), Tag::List(blocks));
        root.insert("entities".into(), Tag::List(entities));
        root.insert("DataVersion".into(), Tag::Int(horizon_world::DATA_VERSION));
        if let Some(o) = &t.occupied {
            let list = o
                .iter()
                .map(|(p, b)| Tag::compound([("pos", Tag::Long(engine::pos_to_long(*p))), ("block", s(b))]))
                .collect();
            root.insert("capsule_occupiedSources".into(), Tag::List(list));
        }
        root.insert("author".into(), s(t.author.as_deref().unwrap_or("?")));
        match horizon_client::save::write_dat(path, Tag::Compound(root)) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("capsule: template {} not written: {e}", path.display());
                false
            }
        }
    }

    // ---- the world ----

    fn capsule_block_name(&self, p: engine::Pos) -> Option<String> {
        kit_blocks::name_at(self, p).map(|n| n.to_string())
    }
    fn capsule_is_air(&self, name: &str) -> bool {
        matches!(name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
    }
    /// The full block entity data at `p` (`blockentity.saveWithId()` without x/y/z): a modelled block entity with
    /// its custom name, else the raw tag Horizon keeps for block entities it does not model.
    fn capsule_block_entity_nbt(&self, p: engine::Pos) -> Option<Compound> {
        let key = (p[0], p[1], p[2]);
        let mut tag = if let Some(e) = self.world.block_entities.get(&key) {
            let name = self.capsule_block_name(p).unwrap_or_default();
            let mut t = horizon_client::save::encode_block_entity(key, e, &name);
            if let Some(chunk) = self.world.chunks.get(&(p[0] >> 4, p[2] >> 4))
                && let Some((_, n)) = chunk.block_names.iter().find(|(q, _)| *q == key)
                && let Tag::Compound(m) = &mut t
            {
                m.insert("CustomName".into(), s(n));
            }
            t
        } else {
            let chunk = self.world.chunks.get(&(p[0] >> 4, p[2] >> 4))?;
            chunk.saved.as_ref()?.block_entities.iter().find(|t| raw_pos(t) == Some(p))?.clone()
        };
        if let Tag::Compound(m) = &mut tag {
            for k in ["x", "y", "z"] {
                m.remove(&JavaString::from(k));
            }
            return Some(m.clone());
        }
        None
    }
    /// Removes the block entity at `p` without spilling (its data is in the template already).
    fn capsule_take_block_entity(&mut self, p: engine::Pos) {
        let key = (p[0], p[1], p[2]);
        self.world.block_entities.remove(&key);
        let ck = (p[0] >> 4, p[2] >> 4);
        let touched = self.world.chunks.get(&ck).is_some_and(|c| {
            c.block_names.iter().any(|(q, _)| *q == key)
                || c.saved.as_ref().is_some_and(|x| x.block_entities.iter().any(|t| raw_pos(t) == Some(p)))
        });
        if !touched {
            return;
        }
        if let Some(chunk) = self.world.chunks.get_mut(&ck) {
            let chunk = std::sync::Arc::make_mut(chunk);
            chunk.block_names.retain(|(q, _)| *q != key);
            if let Some(saved) = chunk.saved.as_mut() {
                let extras = std::sync::Arc::make_mut(saved);
                extras.block_entities.retain(|t| raw_pos(t) != Some(p));
            }
        }
    }
    /// Puts a template's block entity at `p`: decoded when Horizon models it, else kept raw for the chunk's save.
    fn capsule_put_block_entity(&mut self, p: engine::Pos, nbt: &Compound) {
        let mut tag = nbt.clone();
        tag.insert("x".into(), Tag::Int(p[0]));
        tag.insert("y".into(), Tag::Int(p[1]));
        tag.insert("z".into(), Tag::Int(p[2]));
        let name = text(&tag, "CustomName");
        let key = (p[0], p[1], p[2]);
        if let Some((_, e)) = horizon_client::save::decode_block_entity(&Tag::Compound(tag.clone())) {
            self.world.block_entities.insert(key, e);
            if let Some(n) = name
                && let Some(chunk) = self.world.chunks.get_mut(&(p[0] >> 4, p[2] >> 4))
            {
                let chunk = std::sync::Arc::make_mut(chunk);
                chunk.block_names.retain(|(q, _)| *q != key);
                chunk.block_names.push((key, n));
            }
        } else if let Some(chunk) = self.world.chunks.get_mut(&(p[0] >> 4, p[2] >> 4)) {
            let chunk = std::sync::Arc::make_mut(chunk);
            let saved = chunk.saved.get_or_insert_with(Default::default);
            let extras = std::sync::Arc::make_mut(saved);
            extras.block_entities.retain(|t| raw_pos(t) != Some(p));
            extras.block_entities.push(Tag::Compound(tag));
        }
    }
    fn capsule_set(&mut self, p: engine::Pos, state: horizon_world::registry::BlockStateId, flags: i32) {
        let at = BlockPos::new(p[0], p[1], p[2]);
        self.with_level(|b, level, ticks| b.set_block(level, ticks, at, state, flags));
    }

    /// The non-living entities of a box, and armor stands (`snapshotNonLivingEntitiesFromWorld`): (target, nbt).
    fn capsule_entities_in(&mut self, lo: [f64; 3], hi: [f64; 3]) -> Vec<(super::commands::Target, Compound)> {
        use super::commands::Target;
        let mut targets: Vec<Target> = Vec::new();
        targets.extend(self.armor_stands.iter().map(|a| Target::Stand(a.id)));
        targets.extend(self.item_frames.iter().map(|f| Target::Frame(f.id)));
        targets.extend(self.paintings.iter().map(|p| Target::Painting(p.id)));
        targets.extend(self.boats.iter().map(|b| Target::Boat(b.id)));
        targets.extend(self.carts.iter().map(|c| Target::Cart(c.id)));
        targets.extend(self.others.iter().map(|o| Target::Other(o.id)));
        let mut out = Vec::new();
        for t in targets {
            let Some(Tag::Compound(nbt)) = self.entity_nbt(t) else { continue };
            let Some(p) = horizon_client::entity_nbt::position(&Tag::Compound(nbt.clone())) else { continue };
            if p.x >= lo[0] && p.x <= hi[0] && p.y >= lo[1] && p.y <= hi[1] && p.z >= lo[2] && p.z <= hi[2] {
                out.push((t, nbt));
            }
        }
        out
    }
    fn capsule_remove_entity(&mut self, t: super::commands::Target) {
        use super::commands::Target;
        match t {
            Target::Stand(id) => self.armor_stands.retain(|a| a.id != id),
            Target::Frame(id) => self.item_frames.retain(|f| f.id != id),
            Target::Painting(id) => self.paintings.retain(|p| p.id != id),
            Target::Boat(id) => self.boats.retain(|b| b.id != id),
            Target::Cart(id) => self.carts.retain(|c| c.id != id),
            Target::Other(id) => self.others.retain(|o| o.id != id),
            _ => {}
        }
    }
    /// `EntityType.loadEntityRecursive` + `addFreshEntity` (as `/summon` and `/place template` do).
    fn capsule_spawn_entity(&mut self, tag: Tag) {
        let Some(p) = horizon_client::entity_nbt::position(&tag) else { return };
        let key = ((p.x.floor() as i32) >> 4, (p.z.floor() as i32) >> 4);
        let mut waiting = std::mem::take(&mut self.world.loaded_entities);
        self.world.loaded_entities.push((key, vec![tag]));
        self.spawn_loaded_entities();
        waiting.append(&mut self.world.loaded_entities);
        self.world.loaded_entities = waiting;
    }

    /// A chat line (translated here, as the original's `Component.translatable` for the player who acts).
    fn capsule_tell(&mut self, key: &str, args: &[String]) {
        let pattern = self.pack.names.display_get(key).unwrap_or_else(|| key.to_owned());
        let mut out = String::new();
        let mut rest = pattern.as_str();
        let mut i = 0;
        while let Some(at) = rest.find("%s") {
            out.push_str(&rest[..at]);
            out.push_str(args.get(i).map_or("", String::as_str));
            i += 1;
            rest = &rest[at + 2..];
        }
        out.push_str(rest);
        kit_notice::chat(self, out);
    }

    // ---- capture and deploy ----

    /// `StructureSaver.undeploy` + `Capsule.captureAtPosition` / `resentToCapsule`: the area into the template
    /// `name`, written before anything is removed; then the blocks go (no drops, no neighbour updates: flag 2),
    /// block entities first, then the captured entities.
    fn capsule_capture(&mut self, c: &mut engine::Capsule, start: engine::Pos, name: &str, occupied: Option<&BTreeMap<engine::Pos, String>>, author: Option<String>) -> bool {
        let Some(path) = self.capsule_template_path(name) else { return false };
        let size = c.size();
        let overpowered = c.overpowered;
        let positions = {
            let at = |p: engine::Pos| self.capsule_block_name(p).unwrap_or_else(|| "minecraft:air".into());
            let air = |b: &str| self.capsule_is_air(b);
            let ex = |b: &str| engine::excluded(b, overpowered, kit_blocks::block_in_tag(self, "capsule:excluded", b));
            engine::capture_positions(start, size, at, air, ex, occupied)
        };
        let mut blocks = Vec::with_capacity(positions.len());
        for p in &positions {
            let state = self.world.block(p[0], p[1], p[2]);
            let rel = [p[0] - start[0], p[1] - start[1], p[2] - start[2]];
            blocks.push((rel, state, self.capsule_block_entity_nbt(*p)));
        }
        let lo = start.map(f64::from);
        let hi = [lo[0] + f64::from(size), lo[1] + f64::from(size), lo[2] + f64::from(size)];
        let entities = self.capsule_entities_in(lo, hi);
        let template = Template {
            size: [size; 3],
            blocks,
            entities: entities
                .iter()
                .map(|(_, nbt)| {
                    let p = horizon_client::entity_nbt::position(&Tag::Compound(nbt.clone())).map_or([0.0; 3], |v| [v.x, v.y, v.z]);
                    let rel = [p[0] - lo[0], p[1] - lo[1], p[2] - lo[2]];
                    (rel, rel.map(|v| v.floor() as i32), nbt.clone())
                })
                .collect(),
            occupied: None,
            author,
        };
        if !self.capsule_write_template(&path, &template) {
            self.capsule_tell("capsule.error.technicalError", &[]);
            c.revert_from_activated();
            return false;
        }
        let air = self.res.facts().air;
        for p in &positions {
            self.capsule_take_block_entity(*p);
            self.capsule_set(*p, air, 2);
        }
        let _ = self.process_level_events();
        for (t, _) in entities {
            self.capsule_remove_entity(t);
        }
        true
    }

    /// `Capsule.captureAtPosition`: a new template for an empty capsule.
    fn capsule_capture_new(&mut self, c: &mut engine::Capsule, start: engine::Pos, actor: Option<String>) -> bool {
        let who = actor.clone().unwrap_or_else(|| "CapsuleMod".into());
        let Some(n) = self.capsule_saved(|s| s.next()) else { return false };
        let mut name = engine::unique_name(&who, n);
        while self.capsule_template_path(&name).is_some_and(|p| p.exists()) {
            let Some(n) = self.capsule_saved(|s| s.next()) else { return false };
            name = engine::unique_name(&who, n);
        }
        if self.capsule_capture(c, start, &name, None, actor) {
            c.state = engine::State::Linked.value();
            c.structure = Some(name);
            c.timer_start = None;
            true
        } else {
            c.revert_from_activated();
            false
        }
    }

    /// `Capsule.resentToCapsule`: the deployed content back into its template.
    fn capsule_undeploy(&mut self, c: &mut engine::Capsule) -> bool {
        let (Some((start, dim)), Some(name)) = (c.spawn.clone(), c.structure.clone()) else { return false };
        if dim != self.dim.id() {
            return false;
        }
        let Some(path) = self.capsule_template_path(&name) else { return false };
        let template = self.capsule_read_template(&path);
        let occupied = template.as_ref().and_then(|t| t.occupied.clone());
        let author = template.and_then(|t| t.author);
        if !self.capsule_capture(c, start, &name, occupied.as_ref(), author) {
            return false;
        }
        c.state = engine::State::Linked.value();
        c.spawn = None;
        true
    }

    /// `Capsule.deployCapsule` + `StructureSaver.deploy`: the template at the area above `anchor` (or around
    /// `deployAt`), merged with what is there (overridable blocks give way, others block the deploy).
    fn capsule_deploy(&mut self, c: &mut engine::Capsule, anchor: engine::Pos) -> bool {
        let extend = c.extend();
        let dest = engine::deploy_dest(anchor, extend, c.deploy_at.map(engine::pos_of_long));
        c.deploy_at = None;
        let Some(name) = c.structure.clone() else { return false };
        let Some(path) = self.capsule_template_path(&name) else { return false };
        let Some(mut template) = self.capsule_read_template(&path) else {
            self.capsule_tell("capsule.error.templateNotFound", &[name]);
            c.revert_from_activated();
            return false;
        };
        let size = c.size();
        let registry = self.pack.assets.registry;
        let by_pos: HashMap<engine::Pos, String> = template
            .blocks
            .iter()
            .map(|(rel, st, _)| {
                ([dest[0] + rel[0], dest[1] + rel[1], dest[2] + rel[2]], registry.state(*st).map_or_else(String::new, |b| b.name.clone()))
            })
            .collect();
        let checked = {
            let tat = |p: engine::Pos| by_pos.get(&p).cloned();
            let wat = |p: engine::Pos| self.capsule_block_name(p);
            let air = |b: &str| self.capsule_is_air(b);
            let over = |b: &str| kit_blocks::block_in_tag(self, "capsule:overridable", b);
            let mob = |p: engine::Pos| {
                let cell = horizon_core::geometry::Aabb::new(
                    horizon_core::geometry::Vec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2])),
                    horizon_core::geometry::Vec3::new(f64::from(p[0] + 1), f64::from(p[1] + 1), f64::from(p[2] + 1)),
                );
                self.mobs.iter().find(|m| !m.removed && m.health > 0.0 && m.bounds().intersects(cell)).map(|m| {
                    let key = format!("entity.{}", m.kind.id.replace(':', "."));
                    self.pack.names.display_get(&key).unwrap_or_else(|| m.kind.id.clone())
                })
            };
            engine::check_destination(dest, size, tat, wat, air, over, mob)
        };
        let occupied = match checked {
            Ok(o) => o,
            Err(e) => {
                match e {
                    engine::DeployError::NotLoaded => self.capsule_tell("capsule.error.areaNotLoaded", &[]),
                    engine::DeployError::Block(p) => {
                        self.capsule_tell("capsule.error.cantMergeWithDestination", &[format!("BlockPos{{x={}, y={}, z={}}}", p[0], p[1], p[2])])
                    }
                    engine::DeployError::Entity(n) => self.capsule_tell("capsule.error.cantMergeWithDestinationEntity", &[n]),
                }
                c.revert_from_activated();
                return false;
            }
        };
        // `canPlace`: inside the build height.
        let (min_y, max_y) = (horizon_client::chunk::MIN_Y, horizon_client::chunk::MIN_Y + horizon_client::chunk::HEIGHT as i32);
        if by_pos.keys().any(|p| p[1] >= max_y || p[1] <= min_y) {
            self.capsule_tell("capsule.error.notAllowed", &[]);
            c.revert_from_activated();
            return false;
        }
        // `template.saveOccupiedPositions` + write: an undeploy leaves what was there before.
        template.occupied = Some(occupied.clone());
        if !c.is_reward() && !self.capsule_write_template(&path, &template) {
            self.capsule_tell("capsule.error.technicalError", &[]);
            c.revert_from_activated();
            return false;
        }
        let barrier = registry.default_state("minecraft:barrier");
        let over = |g: &Game<'_>, b: &str| kit_blocks::block_in_tag(g, "capsule:overridable", b);
        for (rel, state, nbt) in &template.blocks {
            let p = [dest[0] + rel[0], dest[1] + rel[1], dest[2] + rel[2]];
            if !engine::deploy_places(&occupied, p, |b| over(self, b)) {
                continue;
            }
            // `Clearable.tryClear(blockentity)`, barrier (flags 20), then the block (flags 2).
            self.capsule_take_block_entity(p);
            if let Some(b) = barrier {
                self.capsule_set(p, b, 20);
            }
            self.capsule_set(p, *state, 2);
            if let Some(n) = nbt {
                self.capsule_put_block_entity(p, n);
            }
        }
        let _ = self.process_level_events();
        for (pos, _, nbt) in &template.entities {
            let at = [f64::from(dest[0]) + pos[0], f64::from(dest[1]) + pos[1], f64::from(dest[2]) + pos[2]];
            let mut tag = nbt.clone();
            tag.insert("Pos".into(), Tag::List(at.iter().map(|v| Tag::Double(v.to_bits())).collect()));
            tag.remove(&JavaString::from("UUID"));
            if tag.contains_key(&JavaString::from("TileX")) {
                tag.insert("TileX".into(), Tag::Int(at[0].floor() as i32));
                tag.insert("TileY".into(), Tag::Int(at[1].floor() as i32));
                tag.insert("TileZ".into(), Tag::Int(at[2].floor() as i32));
            }
            self.capsule_spawn_entity(Tag::Compound(tag));
        }
        if !c.is_reward() {
            c.spawn = Some((dest, self.dim.id().to_owned()));
            c.state = engine::State::Deployed.value();
            c.timer_start = None;
            // `clearTemplate`: the content is in the world now (no copy through a recovery capsule).
            template.blocks.clear();
            template.entities.clear();
            self.capsule_write_template(&path, &template);
        }
        true
    }

    // ---- the right click ----

    /// HEAD `creatures::Game::throw_item` (`CapsuleItem.use`, in the air or after a block's use). `None` = not a
    /// capsule.
    pub(super) fn capsule_use(&mut self, stack: &ItemStack) -> Option<bool> {
        if !is_capsule(stack) {
            return None;
        }
        if self.remote.is_some() {
            // A server's client: the server answers the click.
            return None;
        }
        // `use` with the offhand: InteractionResult.FAIL, nothing changes.
        if !matches!(self.hand, horizon_client::first_person::Hand::Main) {
            return Some(false);
        }
        let main = true;
        let now = self.world.game_time;
        let mut c = read(stack);
        // `inventoryTick` may not have run since the timer ran out.
        if c.timer_tick(now) == Some(true) {
            self.capsule_sound_at_player(CLICK_OFF, 0.2, 0.4);
        }
        let action = engine::use_action(&c, main, kit_blocks::sneaking(self), now);
        let mut used_up = false;
        match action {
            engine::UseAction::Nothing => {}
            engine::UseAction::Label => {
                // The label screen is not ported (README): nothing changes.
            }
            engine::UseAction::Activate(st) => {
                c.state = st.value();
                c.timer_start = Some(now);
                self.capsule_sound_at_player(CLICK_ON, 0.2, 0.9);
            }
            engine::UseAction::Undeploy => {
                if self.capsule_undeploy(&mut c) {
                    self.capsule_sound_at_player(CLICK_OFF, 0.2, 0.4);
                }
            }
            engine::UseAction::Instant => used_up = self.capsule_instant(&mut c),
            engine::UseAction::Throw => {
                let dest = if c.has_structure_link() { self.capsule_aim(c.size()).map(|(h, f)| engine::deploy_target(h, f, c.y_offset)) } else { None };
                self.capsule_throw(&mut c, dest);
                return Some(main);
            }
        }
        let mut updated = self.capsule_held_cell().and_then(|cell| cell.clone());
        if let Some(s2) = updated.as_mut() {
            write(self, s2, &c);
        }
        if let Some(cell) = self.capsule_held_cell() {
            // `heldItem.shrink(1)` for a one-use capsule that deployed (capsules stack to 1).
            *cell = if used_up { None } else { updated };
        }
        Some(main)
    }
    fn capsule_held_cell(&mut self) -> Option<&mut Option<ItemStack>> {
        let slot = self.selected;
        let cell = self.inventory.get_mut(slot)?;
        cell.as_ref().is_some_and(is_capsule).then_some(cell)
    }
    fn capsule_sound_at_player(&mut self, sound: &str, volume: f32, pitch: f32) {
        if let Some(p) = self.player.as_ref() {
            let at = [p.position.x.floor() + 0.5, p.position.y.floor() + 0.5, p.position.z.floor() + 0.5];
            kit_sound::at(self, sound, at, volume, pitch);
        }
    }
    /// `Spacial.clientRayTracePreview`: the block aimed at within 18 + size (outline shapes), and the hit face.
    fn capsule_aim(&self, size: i32) -> Option<(engine::Pos, engine::Pos)> {
        let eye = self.eye_position()?;
        let hit = self.clip_ray(eye, super::view_vector(self.yaw, self.pitch), engine::PREVIEW_REACH + f64::from(size))?;
        Some((hit.pos, hit.face))
    }

    /// `CapsuleThrowQueryToServer` with `instant`: an empty size-1 capsule captures the aimed block, a linked one
    /// deploys on the aimed face.
    fn capsule_instant(&mut self, c: &mut engine::Capsule) -> bool {
        let Some((hit, face)) = self.capsule_aim(c.size()) else { return false };
        let actor = Some(self.acting_player_name().to_owned());
        if c.has(engine::State::Empty) {
            let dest = [hit[0], hit[1] + c.y_offset, hit[2]];
            self.capsule_capture_new(c, dest, actor);
        } else if c.has_structure_link() {
            let dest = engine::deploy_target(hit, face, c.y_offset);
            let anchor = [dest[0], dest[1] - 1, dest[2]];
            if self.capsule_deploy(c, anchor) {
                c.undeploy_at = self.world.game_time + engine::UNDEPLOY_DELAY;
                kit_sound::at(self, ARROW_SHOOT, [f64::from(dest[0]) + 0.5, f64::from(dest[1]) + 0.5, f64::from(dest[2]) + 0.5], 0.4, 0.1);
                return c.one_use;
            }
        }
        false
    }

    /// `Capsule.throwCapsule`: the capsule leaves the hand as an item thrown from the eyes (pickup delay 20),
    /// toward `dest` when there is one.
    fn capsule_throw(&mut self, c: &mut engine::Capsule, dest: Option<engine::Pos>) {
        let Some(p) = self.player.as_ref() else { return };
        let feet = [p.position.x, p.position.y, p.position.z];
        let motion = [p.velocity.x, p.velocity.y, p.velocity.z];
        let Some(eye) = self.eye_position() else { return };
        let start = [feet[0], eye[1] - 0.3, feet[2]];
        let block = feet.map(|v| v.floor() as i32);
        c.deploy_at = dest.map(engine::pos_to_long);
        let v = engine::throw_velocity(block, dest, start, self.yaw, self.pitch, motion);
        let Some(mut stack) = self.capsule_held_cell().and_then(|cell| cell.take()) else { return };
        write(self, &mut stack, c);
        self.spawn_item(stack, start, v, 20);
        let thrower = self.acting_player_uuid();
        if let Some(item) = self.dropped.last_mut() {
            item.thrower = Some(thrower);
            THROWN.with(|t| t.borrow_mut().insert(item.id, ([v[0], v[2]], false)));
        }
        kit_sound::at(self, ARROW_SHOOT, [block[0] as f64 + 0.5, block[1] as f64 + 0.5, block[2] as f64 + 0.5], 0.2, 0.1);
    }

    // ---- each tick ----

    /// HEAD `tick_block_entities`: thrown capsules (`onEntityItemUpdate`, `RecallEnchant`), the in-frame host's
    /// capsules (`inventoryTick`) and its starter capsule.
    pub(super) fn capsule_tick(&mut self) {
        if self.remote.is_some() || self.world_root.as_os_str().is_empty() {
            return;
        }
        self.capsule_thrown_tick();
        if self.player.is_some() && self.lan_acting.is_none() {
            self.capsule_inventory_tick();
            let uuid = self.acting_player_uuid();
            self.capsule_starter(uuid);
        }
    }

    /// `inventoryTick` for the player whose inventory is current.
    fn capsule_inventory_tick(&mut self) {
        let now = self.world.game_time;
        let mut sound = false;
        let mut changed: Vec<(usize, ItemStack)> = Vec::new();
        for (i, cell) in self.inventory.iter().enumerate() {
            let Some(stack) = cell.as_ref().filter(|s| is_capsule(s)) else { continue };
            let mut c = read(stack);
            if let Some(timed_out) = c.timer_tick(now) {
                sound |= timed_out;
                let mut s2 = stack.clone();
                write(self, &mut s2, &c);
                changed.push((i, s2));
            }
        }
        for (i, s2) in changed {
            self.inventory[i] = Some(s2);
        }
        if sound {
            self.capsule_sound_at_player(CLICK_OFF, 0.2, 0.4);
        }
    }

    /// HEAD `lan_actions::Game::tick_guests`: the guests' capsules (single player's window included) and their
    /// starter capsule. A guest's turn is entered only when there is work.
    pub(super) fn capsule_guests_tick(&mut self, host: &mut LanHost) {
        if self.remote.is_some() || self.world_root.as_os_str().is_empty() {
            return;
        }
        let now = self.world.game_time;
        for i in 0..host.hub.guests.len() {
            let g = &host.hub.guests[i];
            if !(host.hub.here(g) && g.loaded && g.placed && !g.dead) {
                continue;
            }
            let uuid = horizon_client::lan::playerdata::uuid_ints(g.uuid);
            let needs_starter = self.capsule_saved(|s| !s.starters.contains(&uuid_text(uuid))).unwrap_or(false);
            let timers = guest_has_due_capsule(&g.saved, now);
            if !needs_starter && !timers {
                continue;
            }
            let conn = g.conn;
            let _ = self.as_guest(host, conn, |game| {
                if timers {
                    game.capsule_inventory_tick();
                }
                if needs_starter {
                    game.capsule_starter(uuid);
                }
            });
        }
    }

    /// `StarterLoot.playerLogin` (starterMode = random): a player who never got one receives one starter capsule,
    /// a linked capsule of a copy of one random template of `config/capsule/starters` (the carry's
    /// `initialconfig/starters`), random colours, labelled after the file.
    fn capsule_starter(&mut self, uuid: [i32; 4]) {
        let key = uuid_text(uuid);
        if self.capsule_saved(|s| s.starters.contains(&key)) != Some(false) {
            return;
        }
        let dir = self.options.resources.join("data/capsule/initialconfig/starters");
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "nbt")).collect())
            .unwrap_or_default();
        files.sort();
        self.capsule_saved(|s| s.starters.push(key.clone()));
        if files.is_empty() {
            return;
        }
        use horizon_core::random::RandomSource;
        let pick = self.world.random.next_int_bound(files.len() as i32).unwrap_or(0) as usize;
        let src = files[pick].clone();
        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("starter").to_owned();
        let Some(template) = self.capsule_read_template(&src) else { return };
        let player = self.acting_player_name().to_owned();
        let base = format!("{player}-config_capsule_starters_{stem}");
        let Some(n) = self.capsule_saved(|s| s.next()) else { return };
        let name = engine::unique_name(&base, n);
        let Some(dest) = self.capsule_template_path(&name) else { return };
        if !self.capsule_write_template(&dest, &template) {
            return;
        }
        let color = |g: &mut Game<'_>| PALETTE[g.world.random.next_int_bound(PALETTE.len() as i32).unwrap_or(0) as usize];
        let base_color = color(self);
        let material = color(self);
        let c = engine::Capsule {
            state: engine::State::Linked.value(),
            size: engine::size_of_template(template.size),
            material_color: Some(material),
            base_color: Some(base_color),
            structure: Some(name),
            label: Some(engine::label_from_path(&format!("config/capsule/starters/{stem}"))),
            upgraded: 0,
            ..Default::default()
        };
        let mut stack = ItemStack::new(CAPSULE, 1);
        stack.data_mut().dyed_color = Some(base_color);
        write(self, &mut stack, &c);
        self.give_or_drop(stack);
    }

    /// Thrown capsules each tick: they fly toward their target, act when they touch something, and come back
    /// to the thrower with Recall.
    fn capsule_thrown_tick(&mut self) {
        let candidates: Vec<u32> = self.dropped.iter().filter(|i| is_capsule(&i.stack) && read(&i.stack).is_activated()).map(|i| i.id).collect();
        let mut seen = HashSet::new();
        for id in candidates {
            seen.insert(id);
            let Some(i) = self.dropped.iter().position(|d| d.id == id) else { continue };
            let item = &self.dropped[i];
            let c = read(&item.stack);
            let (last, extended) = THROWN.with(|t| t.borrow().get(&id).copied()).unwrap_or(([0.0, 0.0], false));
            let v = item.velocity;
            // `horizontalCollision`: Horizon zeroes a speed the move could not make.
            let wall = (last[0].abs() > 1e-4 && v.x == 0.0) || (last[1].abs() > 1e-4 && v.z == 0.0);
            let block_here = self.world.block(item.position.x.floor() as i32, item.position.y.floor() as i32, item.position.z.floor() as i32);
            let in_liquid = self.res.facts().get(block_here).fluid != horizon_client::blocks::Fluid::None;
            let thrower_in_liquid = false;
            let touched = item.ticks > 2 && (item.on_ground || wall || (in_liquid && !thrower_in_liquid));
            if touched {
                // The thrower acts (claims, messages): a guest's landing runs in the guest's turn.
                let thrower = self.dropped[i].thrower;
                let conn = thrower.and_then(|u| {
                    self.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| horizon_client::lan::playerdata::uuid_ints(g.uuid) == u).map(|g| g.conn))
                });
                match conn {
                    Some(conn) if self.lan_acting.is_none() => {
                        if let Some(mut host) = self.lan.take() {
                            if self.as_guest(&mut host, conn, |g| g.capsule_landed(id)).is_none() {
                                self.lan = Some(host);
                                self.capsule_landed(id);
                            } else {
                                self.lan = Some(host);
                            }
                        }
                    }
                    _ => self.capsule_landed(id),
                }
            } else if let Some(d) = c.deploy_at.map(engine::pos_of_long) {
                let item = &mut self.dropped[i];
                let (vx, vz) = engine::move_toward([item.position.x, item.position.y, item.position.z], [item.velocity.x, item.velocity.y, item.velocity.z], d, true);
                item.velocity.x = vx;
                item.velocity.z = vz;
            }
            // `setExtendedLifetime`: a thrown capsule lasts 12000 ticks, not 6000.
            if let Some(item) = self.dropped.iter_mut().find(|d| d.id == id) {
                let mut ext = extended;
                if !ext && item.age >= 5999 {
                    item.age = 0;
                    ext = true;
                }
                let v = [item.velocity.x, item.velocity.z];
                THROWN.with(|t| t.borrow_mut().insert(id, (v, ext)));
            }
        }
        // Recall: a landed capsule with the enchantment goes back to its thrower (`pickupItemBack`).
        let recall: Vec<u32> = self
            .dropped
            .iter()
            .filter(|i| is_capsule(&i.stack) && i.stack.enchantment_level("capsule:recall") > 0 && i.thrower.is_some() && i.ticks > 2 && i.on_ground)
            .map(|i| i.id)
            .collect();
        for id in recall {
            let Some(i) = self.dropped.iter().position(|d| d.id == id) else { continue };
            let thrower = self.dropped[i].thrower;
            if let Some(at) = thrower.and_then(|u| self.capsule_player_position(u)) {
                let item = &mut self.dropped[i];
                item.position = horizon_core::geometry::Vec3::new(at[0], at[1], at[2]);
                item.pickup_delay = 0;
            }
        }
        THROWN.with(|t| t.borrow_mut().retain(|id, _| seen.contains(id) || self.dropped.iter().any(|d| d.id == *id)));
    }
    fn capsule_player_position(&self, uuid: [i32; 4]) -> Option<[f64; 3]> {
        if let Some(h) = self.lan.as_ref() {
            for g in &h.hub.guests {
                if horizon_client::lan::playerdata::uuid_ints(g.uuid) == uuid && h.hub.here(g) {
                    return Some(g.position);
                }
            }
        }
        let p = self.player.as_ref()?;
        (self.level.player_uuid == Some(uuid)).then(|| [p.position.x, p.position.y, p.position.z])
    }

    /// `Capsule.handleItemEntityOnGround`: a linked capsule deploys on the block under it (or at its target), an
    /// empty one captures the area of a capture base within size + 2.
    fn capsule_landed(&mut self, id: u32) {
        let Some(i) = self.dropped.iter().position(|d| d.id == id) else { return };
        self.dropped[i].velocity = horizon_core::geometry::Vec3::ZERO;
        let (pos, stack, thrower) = {
            let it = &self.dropped[i];
            ([it.position.x, it.position.y, it.position.z], it.stack.clone(), it.thrower)
        };
        let mut c = read(&stack);
        let size = c.size();
        let mut remove = false;
        if c.has_structure_link() {
            let anchor = engine::find_bottom_block(pos[0], pos[1], pos[2]);
            if self.capsule_deploy(&mut c, anchor) {
                kit_sound::at(self, ARROW_SHOOT, pos, 0.4, 0.1);
                remove = c.one_use;
            }
        } else {
            let base = engine::search_order(pos[0], pos[1], pos[2], size + 2)
                .find(|p| self.capsule_block_name(*p).as_deref() == Some(MARKER));
            match base {
                Some(b) => {
                    let state = self.world.block(b[0], b[1], b[2]);
                    let facing = kit_blocks::prop(self, state, "facing").and_then(|f| dir_of(&f)).unwrap_or([0, 1, 0]);
                    let anchor = engine::anchor(b, facing, size);
                    let who = thrower.and_then(|u| self.capsule_player_name(u));
                    let source = engine::capture_source(anchor, c.extend());
                    self.capsule_capture_new(&mut c, source, who);
                }
                None => {
                    c.revert_from_activated();
                    if let Some(u) = thrower {
                        self.capsule_tell_player(u, "capsule.error.noCaptureBase");
                    }
                }
            }
        }
        let Some(i) = self.dropped.iter().position(|d| d.id == id) else { return };
        if remove {
            self.dropped.remove(i);
        } else {
            let mut s2 = stack;
            write(self, &mut s2, &c);
            self.dropped[i].stack = s2;
        }
    }
    fn capsule_player_name(&self, uuid: [i32; 4]) -> Option<String> {
        if let Some(h) = self.lan.as_ref() {
            for g in &h.hub.guests {
                if horizon_client::lan::playerdata::uuid_ints(g.uuid) == uuid {
                    return Some(g.name.clone());
                }
            }
        }
        (self.level.player_uuid == Some(uuid)).then(|| super::chat::PLAYER_NAME.to_owned())
    }
    fn capsule_tell_player(&mut self, uuid: [i32; 4], key: &str) {
        let conn = self.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| horizon_client::lan::playerdata::uuid_ints(g.uuid) == uuid).map(|g| g.conn));
        match conn {
            Some(conn) if self.lan_acting.is_none() => {
                let Some(mut host) = self.lan.take() else { return };
                let _ = self.as_guest(&mut host, conn, |g| g.capsule_tell(key, &[]));
                self.lan = Some(host);
            }
            _ => self.capsule_tell(key, &[]),
        }
    }

    // ---- dispensers (DispenseCapsuleBehavior) ----

    /// HEAD cancellable `dispenser::Game::dispense_item`: a deployed capsule comes back, a linked one deploys in
    /// front of the dispenser (capture base); the capsule stays in the slot (a one-use one is used up).
    pub(super) fn capsule_dispense(&mut self, pos: [i32; 3], facing: [i32; 3], stack: &ItemStack) -> Option<Option<ItemStack>> {
        if !is_capsule(stack) || self.remote.is_some() {
            return None;
        }
        let mut c = read(stack);
        let mut out = Some(stack.clone());
        let center = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5];
        if c.has(engine::State::Deployed) && c.spawn.is_some() {
            if self.capsule_undeploy(&mut c) {
                kit_sound::at(self, CLICK_OFF, center, 0.2, 0.4);
            }
        } else if c.has_structure_link() {
            let size = c.size();
            let a = engine::anchor(pos, facing, size);
            let anchor = [a[0], a[1] + c.y_offset, a[2]];
            if self.capsule_deploy(&mut c, anchor) {
                kit_sound::at(self, ARROW_SHOOT, center, 0.2, 0.4);
                if c.one_use {
                    out = None;
                }
            }
        }
        if let Some(s2) = out.as_mut() {
            write(self, s2, &c);
        }
        // `DefaultDispenseItemBehavior.playSound`: the dispenser click.
        kit_sound::level_event(self, 1000, pos, 0);
        Some(out)
    }

    // ---- the tooltip ----

    /// RETURN `stations::Game::stack_tooltip` (`CapsuleItem.appendHoverText`).
    pub(super) fn capsule_tooltip(&self, stack: &ItemStack, mut lines: Vec<(horizon_client::text::Text, u32)>) -> Vec<(horizon_client::text::Text, u32)> {
        if !is_capsule(stack) {
            return lines;
        }
        let c = read(stack);
        let tr = |k: &str| self.pack.names.display_get(k).unwrap_or_else(|| k.to_owned());
        let mut add = Vec::new();
        for line in engine::tooltip(&c) {
            match line {
                engine::Line::Author(a) => add.push((format!("§3§o{} {a}§r", tr("capsule.tooltip.author")), u32::MAX)),
                engine::Line::Key(k) => add.push((tr(k).trim().to_owned(), u32::MAX)),
                engine::Line::Overpowered => add.push((format!("§5{}§r", tr("capsule.tooltip.overpowered")), u32::MAX)),
                engine::Line::Size { size, upgraded, instant } => {
                    let mut t = format!("{size}×{size}×{size}");
                    if upgraded > 0 {
                        t.push_str(&format!(" ({upgraded}/{} {})", engine::config::UPGRADE_LIMIT, tr("capsule.tooltip.upgraded")));
                    }
                    if instant {
                        t.push_str(&format!(" ({})", tr("capsule.tooltip.instant").trim()));
                    }
                    add.push((format!("{}: {t}", tr("capsule.tooltip.size")), u32::MAX));
                }
            }
        }
        let at = lines.len().min(1);
        for (i, (t, color)) in add.into_iter().enumerate() {
            lines.insert(at + i, (horizon_client::text::Text::legacy(&t), color));
        }
        lines
    }
}

/// The position of a raw block entity tag (`x`, `y`, `z`).
fn raw_pos(t: &Tag) -> Option<engine::Pos> {
    let n = |k: &str| horizon_client::save::number(t.get(k)).map(|v| v as i32);
    Some([n("x")?, n("y")?, n("z")?])
}

fn uuid_text(u: [i32; 4]) -> String {
    format!("{}-{}-{}-{}", u[0], u[1], u[2], u[3])
}

/// A guest's saved inventory holds an activated capsule whose timer ran out (or has none).
fn guest_has_due_capsule(saved: &Option<Tag>, now: i64) -> bool {
    let Some(Tag::List(inv)) = saved.as_ref().and_then(|t| t.get("Inventory")) else { return false };
    inv.iter().any(|slot| {
        let is = matches!(slot.get("id"), Some(Tag::String(s)) if s.to_string_lossy() == CAPSULE);
        let data = slot.get("components").and_then(|c| c.get("minecraft:custom_data"));
        let state = data.and_then(|d| match d.get("state") {
            Some(Tag::Int(v)) => Some(*v),
            _ => None,
        });
        let start = data.and_then(|d| d.get("activetimer")).and_then(|t| match t.get("starttime") {
            Some(Tag::Long(v)) => Some(*v),
            _ => None,
        });
        is && matches!(state, Some(1 | 4 | 6))
            && start.is_none_or(|s| now >= s + engine::config::PREVIEW_DISPLAY_DURATION || s > now)
    })
}

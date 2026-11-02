// Curios for Horizon: the rules, without any game code.
// Ported from Curios API 5.14.1+1.20.1 (LGPL-3.0-or-later, sources in sources/curios, branch 1.20.x,
// commit a91da6b "5.14.1"): slot types (CuriosSlotManager, LegacySlotManager, SlotType.Builder,
// CuriosEntityManager), the inventory (CurioInventoryCapability, CurioStacksHandler,
// DynamicStackHandler), the menu (CuriosContainerV2 + AbstractContainerMenu.doClick), the drops
// (CuriosEventHandler.handleDrops) and the tooltip line (ClientEventHandler.onTooltip).
// This file is pasted into the game as `crate::game::curios::engine` (tools/hnmod.py bundle) and is also the
// `horizon_curios` crate the tests use: plain Rust, no game types, no inner attributes.
use std::collections::{BTreeMap, BTreeSet};

pub type Pos = [i32; 3];

/// the pack's values (profile the 1.20.1 pack), with the file and line.
pub mod config {
    // ---- config/curios-client.toml
    /// renderCurios (curios-client.toml:5): rendering on the player model (not ported, see README).
    pub const CLIENT_RENDER_CURIOS: bool = true;
    /// enableButton (curios-client.toml:7): the Curios button on the inventory screens.
    pub const CLIENT_ENABLE_BUTTON: bool = true;
    /// buttonXOffset / buttonYOffset (curios-client.toml:10, :13), range -100..100.
    pub const CLIENT_BUTTON_XOFFSET: i32 = 0;
    pub const CLIENT_BUTTON_YOFFSET: i32 = 0;
    /// creativeButtonXOffset / creativeButtonYOffset (curios-client.toml:16, :19).
    pub const CLIENT_CREATIVE_BUTTON_XOFFSET: i32 = 0;
    pub const CLIENT_CREATIVE_BUTTON_YOFFSET: i32 = 0;
    /// buttonCorner (curios-client.toml:22).
    pub const CLIENT_BUTTON_CORNER: &str = "TOP_LEFT";
    // ---- config/curios-common.toml
    /// slots (curios-common.toml:4): the pack's list is empty.
    pub const SLOTS: &[&str] = &[];
    // ---- saves/<world>/serverconfig/curios-server.toml
    /// keepCurios (curios-server.toml:6): ON / DEFAULT / OFF.
    pub const KEEP_CURIOS: &str = "DEFAULT";
    /// menu.enableLegacyMenu (curios-server.toml:11): false = CuriosContainerV2 / CuriosScreenV2.
    pub const ENABLE_LEGACY_MENU: bool = false;
    /// menu.experimental.minimumColumns (curios-server.toml:17), range 1..8.
    pub const MINIMUM_COLUMNS: i32 = 1;
    /// menu.experimental.maxSlotsPerPage (curios-server.toml:21), range 1..48.
    pub const MAX_SLOTS_PER_PAGE: i32 = 48;
}

/// Plain data of the 1.20.1 jar (tools/jar_data.py), the part the port uses.
#[allow(dead_code)]
pub mod jar_data {
    /// `SlotTypePreset` (id, priority): `getMessageBuilder` = priority + icon `curios:slot/empty_<id>_slot`.
    pub const SLOT_TYPE_PRESETS: [(&str, i32); 10] = [
        ("head", 40),
        ("necklace", 60),
        ("back", 80),
        ("body", 100),
        ("bracelet", 120),
        ("hands", 140),
        ("ring", 160),
        ("belt", 180),
        ("charm", 200),
        ("curio", 20),
    ];
    /// `CuriosClientConfig.Client.ButtonCorner` (name, xoffset, yoffset, creativeXoffset, creativeYoffset).
    pub const BUTTON_CORNERS: [(&str, i32, i32, i32, i32); 4] = [
        ("TOP_LEFT", 26, -75, 73, -62),
        ("TOP_RIGHT", 61, -75, 95, -62),
        ("BOTTOM_LEFT", 26, -20, 73, -29),
        ("BOTTOM_RIGHT", 61, -20, 95, -29),
    ];
    /// `CuriosEventHandler.getDroppedItem`: `setPickUpDelay(40)`.
    pub const DROP_PICKUP_DELAY: u32 = 40;
    /// `SlotType.Builder.build`: order when none is given.
    pub const DEFAULT_ORDER: i32 = 1000;
    /// `SlotType.Builder.icon` default.
    pub const DEFAULT_ICON: &str = "curios:slot/empty_curio_slot";
    /// `CuriosCommand`: `hasPermission(2)`.
    pub const COMMAND_PERMISSION: i32 = 2;
}

// =================================================================================================
// Slot types
// =================================================================================================

/// `ICurio.DropRule` (ordinals 0..3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, PartialOrd, Ord)]
pub enum DropRule {
    #[default]
    Default,
    AlwaysDrop,
    AlwaysKeep,
    Destroy,
}
impl DropRule {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "DEFAULT" => DropRule::Default,
            "ALWAYS_DROP" => DropRule::AlwaysDrop,
            "ALWAYS_KEEP" => DropRule::AlwaysKeep,
            "DESTROY" => DropRule::Destroy,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            DropRule::Default => "DEFAULT",
            DropRule::AlwaysDrop => "ALWAYS_DROP",
            DropRule::AlwaysKeep => "ALWAYS_KEEP",
            DropRule::Destroy => "DESTROY",
        }
    }
}

/// `SlotType` once built.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotType {
    pub id: String,
    pub order: i32,
    pub size: i32,
    pub native_gui: bool,
    pub cosmetic: bool,
    pub render_toggle: bool,
    pub icon: String,
    pub drop_rule: DropRule,
    pub validators: BTreeSet<String>,
}
impl SlotType {
    /// `SlotType.compareTo`: order, then identifier.
    pub fn key(&self) -> (i32, &str) {
        (self.order, self.id.as_str())
    }
}

/// `SlotType.Builder` (all its merge rules).
#[derive(Clone, Debug)]
pub struct SlotBuilder {
    pub id: String,
    order: Option<i32>,
    size: Option<i32>,
    size_mod: i32,
    native_gui: Option<bool>,
    cosmetic: Option<bool>,
    render_toggle: Option<bool>,
    icon: String,
    drop_rule: DropRule,
    validators: Option<BTreeSet<String>>,
}
impl SlotBuilder {
    pub fn new(id: &str) -> Self {
        SlotBuilder {
            id: id.to_owned(),
            order: None,
            size: None,
            size_mod: 0,
            native_gui: None,
            cosmetic: None,
            render_toggle: None,
            icon: jar_data::DEFAULT_ICON.to_owned(),
            drop_rule: DropRule::Default,
            validators: None,
        }
    }
    /// `Builder.apply`: another builder's set values over this one (the icon and the drop rule
    /// always, as in the original, where they are never null).
    pub fn apply(&mut self, other: &SlotBuilder) {
        if other.id != self.id {
            return;
        }
        if let Some(o) = other.order {
            self.order(o, false);
        }
        if let Some(s) = other.size {
            self.size(s, "SET", false);
        }
        if let Some(v) = other.native_gui {
            self.native_gui(v, false);
        }
        if let Some(v) = other.cosmetic {
            self.cosmetic(v, false);
        }
        if let Some(v) = other.render_toggle {
            self.render_toggle(v, false);
        }
        self.icon = other.icon.clone();
        self.drop_rule = other.drop_rule;
        if let Some(v) = &other.validators {
            self.validators = Some(v.clone());
        }
    }
    pub fn icon(&mut self, icon: &str) {
        self.icon = icon.to_owned();
    }
    pub fn order(&mut self, order: i32, replace: bool) {
        self.order = Some(match self.order {
            Some(o) if !replace => o.min(order),
            _ => order,
        });
    }
    pub fn size(&mut self, size: i32, operation: &str, replace: bool) {
        match operation {
            "SET" => {
                self.size = Some(match self.size {
                    Some(s) if !replace => s.max(size),
                    _ => size,
                });
                if replace {
                    self.size_mod = 0;
                }
            }
            "ADD" => self.size_mod = if replace { size } else { self.size_mod + size },
            "REMOVE" => self.size_mod = if replace { -size } else { self.size_mod - size },
            _ => {}
        }
    }
    pub fn native_gui(&mut self, v: bool, replace: bool) {
        self.native_gui = Some(match self.native_gui {
            Some(o) if !replace => o && v,
            _ => v,
        });
    }
    pub fn render_toggle(&mut self, v: bool, replace: bool) {
        self.render_toggle = Some(match self.render_toggle {
            Some(o) if !replace => o && v,
            _ => v,
        });
    }
    pub fn cosmetic(&mut self, v: bool, replace: bool) {
        self.cosmetic = Some(match self.cosmetic {
            Some(o) if !replace => o || v,
            _ => v,
        });
    }
    pub fn drop_rule(&mut self, rule: DropRule) {
        self.drop_rule = rule;
    }
    pub fn validator(&mut self, v: &str) {
        self.validators.get_or_insert_with(BTreeSet::new).insert(v.to_owned());
    }
    pub fn build(&self) -> SlotType {
        let mut size = self.size.unwrap_or(1);
        size += self.size_mod;
        SlotType {
            id: self.id.clone(),
            order: self.order.unwrap_or(jar_data::DEFAULT_ORDER),
            size: size.max(0),
            native_gui: self.native_gui.unwrap_or(true),
            cosmetic: self.cosmetic.unwrap_or(false),
            render_toggle: self.render_toggle.unwrap_or(true),
            icon: self.icon.clone(),
            drop_rule: self.drop_rule,
            validators: self.validators.clone().unwrap_or_else(|| BTreeSet::from(["curios:tag".to_owned()])),
        }
    }
}

/// A resource location written without a namespace is `minecraft:` (`new ResourceLocation(s)`).
pub fn full_id(s: &str) -> String {
    if s.contains(':') { s.to_owned() } else { format!("minecraft:{s}") }
}

/// `CuriosSlotManager.fromJson`.
pub fn slot_from_json(b: &mut SlotBuilder, v: &serde_json::Value) -> Result<(), String> {
    let o = v.as_object().ok_or("top element is not an object")?;
    let int = |k: &str| -> Result<Option<i32>, String> {
        match o.get(k) {
            None => Ok(None),
            Some(x) => x.as_i64().map(|n| Some(n as i32)).ok_or_else(|| format!("{k} is not a number")),
        }
    };
    let boolean = |k: &str| -> Result<Option<bool>, String> {
        match o.get(k) {
            None => Ok(None),
            Some(x) => x.as_bool().map(Some).ok_or_else(|| format!("{k} is not a boolean")),
        }
    };
    let string = |k: &str| o.get(k).and_then(|x| x.as_str()).unwrap_or("").to_owned();
    let size = int("size")?;
    if size.is_some_and(|s| s < 0) {
        return Err("Size cannot be less than 0!".into());
    }
    let operation = o.get("operation").and_then(|x| x.as_str()).unwrap_or("SET").to_owned();
    if !matches!(operation.as_str(), "SET" | "ADD" | "REMOVE") {
        return Err(format!("{operation} is not a valid operation!"));
    }
    let drop = string("drop_rule");
    let drop_rule = if drop.is_empty() {
        None
    } else {
        Some(DropRule::parse(&drop).ok_or_else(|| format!("{drop} is not a valid drop rule!"))?)
    };
    let replace = boolean("replace")?.unwrap_or(false);
    let order = int("order")?;
    let icon = string("icon");
    let toggle = boolean("render_toggle")?;
    let cosmetic = boolean("add_cosmetic")?;
    let native = boolean("use_native_gui")?;
    if let Some(o) = order {
        b.order(o, replace);
    }
    if !icon.is_empty() {
        b.icon(&full_id(&icon));
    }
    if let Some(r) = drop_rule {
        b.drop_rule(r);
    }
    if let Some(s) = size {
        b.size(s, &operation, replace);
    }
    if let Some(c) = cosmetic {
        b.cosmetic(c, replace);
    }
    if let Some(n) = native {
        b.native_gui(n, replace);
    }
    if let Some(t) = toggle {
        b.render_toggle(t, replace);
    }
    if let Some(list) = o.get("validators").and_then(|x| x.as_array()) {
        for x in list {
            if let Some(s) = x.as_str() {
                b.validator(&full_id(s));
            }
        }
    }
    Ok(())
}

/// `CuriosSlotManager.fromConfig`: `id=x;size=n;operation=..;order=..;icon=..;drop_rule=..;
/// render_toggle=..;add_cosmetic=..;use_native_gui=..`, always with `replace`.
pub fn slots_from_config(map: &mut BTreeMap<String, SlotBuilder>, entries: &[&str]) -> Result<BTreeSet<String>, String> {
    let mut parsed = Vec::new();
    for s in entries {
        let mut sub = BTreeMap::new();
        for token in s.split(';').filter(|t| !t.is_empty()) {
            let mut kv = token.split('=');
            let k = kv.next().unwrap_or("");
            let v = kv.next().ok_or_else(|| format!("Cannot load config entry {s}"))?;
            sub.insert(k.to_owned(), v.to_owned());
        }
        if !sub.contains_key("id") {
            return Err(format!("Cannot load config entry {s} due to missing id field"));
        }
        parsed.push(sub);
    }
    let mut results = BTreeSet::new();
    for e in parsed {
        let id = e["id"].clone();
        let size = match e.get("size") {
            Some(v) => Some(v.parse::<i32>().map_err(|_| format!("{v} is not a number"))?),
            None => None,
        };
        if size.is_some_and(|s| s < 0) {
            return Err("Size cannot be less than 0!".into());
        }
        let operation = e.get("operation").cloned().unwrap_or_else(|| "SET".into());
        if !matches!(operation.as_str(), "SET" | "ADD" | "REMOVE") {
            return Err(format!("{operation} is not a valid operation!"));
        }
        let drop = e.get("drop_rule").cloned().unwrap_or_default();
        if !drop.is_empty() && DropRule::parse(&drop).is_none() {
            return Err(format!("{drop} is not a valid drop rule!"));
        }
        results.insert(id.clone());
        let b = map.entry(id.clone()).or_insert_with(|| SlotBuilder::new(&id));
        if let Some(o) = e.get("order").and_then(|v| v.parse().ok()) {
            b.order(o, true);
        }
        if let Some(icon) = e.get("icon").filter(|s| !s.is_empty()) {
            b.icon(&full_id(icon));
        }
        if let Some(r) = DropRule::parse(&drop) {
            b.drop_rule(r);
        }
        if let Some(s) = size {
            b.size(s, &operation, true);
        }
        if let Some(c) = e.get("add_cosmetic") {
            b.cosmetic(c == "true", true);
        }
        if let Some(n) = e.get("use_native_gui") {
            b.native_gui(n == "true", true);
        }
        if let Some(t) = e.get("render_toggle") {
            b.render_toggle(t == "true", true);
        }
    }
    Ok(results)
}

/// `SlotTypeMessage` (the legacy InterModComms registration: `register_type` / `modify_type`).
#[derive(Clone, Debug, PartialEq)]
pub struct LegacyMessage {
    pub id: String,
    /// `Builder.size`, 1 by default.
    pub size: i32,
    /// `Builder.priority` (the order).
    pub priority: Option<i32>,
    pub icon: Option<String>,
    /// `Builder.cosmetic()`.
    pub cosmetic: bool,
    /// `Builder.hide()` makes it false.
    pub visible: bool,
}
impl LegacyMessage {
    /// `new SlotTypeMessage.Builder(id)`.
    pub fn new(id: &str) -> Self {
        LegacyMessage { id: id.to_owned(), size: 1, priority: None, icon: None, cosmetic: false, visible: true }
    }
    /// `SlotTypePreset.X.getMessageBuilder()` (None: not a preset id).
    pub fn preset(id: &str) -> Option<Self> {
        let (pid, priority) = jar_data::SLOT_TYPE_PRESETS.iter().find(|(p, _)| *p == id)?;
        Some(LegacyMessage {
            priority: Some(*priority),
            icon: Some(format!("curios:slot/empty_{pid}_slot")),
            ..LegacyMessage::new(id)
        })
    }
    pub fn with_size(mut self, size: i32) -> Self {
        self.size = size;
        self
    }
}

/// The `register_type` messages the mods of the pack send (read from their 1.20.1 bytecode), so
/// their slot types exist as soon as their content is installed, before (or without) their own
/// native port: ars_nouveau-1.20.1-4.12.7 `ModSetup.sendImc`, goety-2.5.59.2 `CuriosIntegration`
/// (`new Builder(id)` for each slot of its item table), occultism-1.20.1-1.158.0 `Occultism.enqueueIMC`.
pub fn known_legacy(sender: &str) -> Vec<LegacyMessage> {
    let p = |id: &str| LegacyMessage::preset(id).expect("a preset");
    match sender {
        "ars_nouveau" => vec![
            p("necklace"),
            LegacyMessage { icon: Some("curios:slot/empty_curio_slot".into()), ..LegacyMessage::new("an_focus") },
            p("belt"),
            p("ring").with_size(2),
            p("charm"),
            p("head"),
        ],
        "goety" => ["ring", "head", "necklace", "body", "back", "belt", "charm", "hands"].iter().map(|id| LegacyMessage::new(id)).collect(),
        "occultism" => vec![p("hands"), p("ring"), p("head"), p("belt")],
        _ => Vec::new(),
    }
}
pub const KNOWN_LEGACY_SENDERS: [&str; 3] = ["ars_nouveau", "goety", "occultism"];

/// `LegacySlotManager.processImc` for the register (create) and modify messages, by sender mod id.
pub fn legacy_builders(register: &[(String, LegacyMessage)], modify: &[(String, LegacyMessage)]) -> BTreeMap<String, SlotBuilder> {
    let mut builders: BTreeMap<String, SlotBuilder> = BTreeMap::new();
    for (list, create) in [(register, true), (modify, false)] {
        // TreeMap by sender mod id, each mod's messages in the order sent.
        let mut by_mod: BTreeMap<&str, Vec<&LegacyMessage>> = BTreeMap::new();
        for (m, msg) in list {
            by_mod.entry(m.as_str()).or_default().push(msg);
        }
        for msgs in by_mod.values() {
            for msg in msgs {
                if !builders.contains_key(&msg.id) {
                    if !create {
                        continue;
                    }
                    builders.insert(msg.id.clone(), SlotBuilder::new(&msg.id));
                }
                let b = builders.get_mut(&msg.id).expect("inserted");
                b.size(msg.size, "SET", false);
                b.native_gui(msg.visible, false);
                b.cosmetic(msg.cosmetic, false);
                let preset = LegacyMessage::preset(&msg.id);
                match (&msg.icon, preset.as_ref().and_then(|p| p.icon.as_ref())) {
                    (None, Some(p)) => b.icon(p),
                    (Some(i), _) => b.icon(&full_id(i)),
                    _ => {}
                }
                match (msg.priority, preset.and_then(|p| p.priority)) {
                    (None, Some(p)) => b.order(p, false),
                    (Some(p), _) => b.order(p, false),
                    _ => {}
                }
            }
        }
    }
    builders
}

/// A data file of the resource root: `data/<namespace>/curios/<kind>/<path>.json`.
#[derive(Clone, Debug)]
pub struct DataFile {
    pub namespace: String,
    pub path: String,
    pub text: String,
}

/// Everything the slot registry is built from.
#[derive(Clone, Debug, Default)]
pub struct Sources {
    pub slots: Vec<DataFile>,
    pub entities: Vec<DataFile>,
    /// (sender mod, message) of `register_type`.
    pub legacy_register: Vec<(String, LegacyMessage)>,
    /// (sender mod, message) of `modify_type`.
    pub legacy_modify: Vec<(String, LegacyMessage)>,
    /// curios-common.toml `slots`.
    pub config: Vec<String>,
}

/// The slot types and the ones each entity gets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Registry {
    pub slots: BTreeMap<String, SlotType>,
    /// Entity id (`minecraft:player`) -> slot ids.
    pub entities: BTreeMap<String, BTreeSet<String>>,
    /// Parse errors (the original logs them and skips the file).
    pub errors: Vec<String>,
}
pub const PLAYER: &str = "minecraft:player";

/// `CuriosSlotManager.apply` + `CuriosEntityManager.apply`. Files are taken in (namespace, path)
/// order (the original's pack order; the result does not depend on it unless `replace` is used).
pub fn build_registry(src: &Sources) -> Registry {
    let mut reg = Registry::default();
    let mut files: Vec<&DataFile> = src.slots.iter().collect();
    files.sort_by(|a, b| (&a.namespace, &a.path).cmp(&(&b.namespace, &b.path)));
    let mut map: BTreeMap<String, SlotBuilder> = BTreeMap::new();
    let load = |f: &DataFile, map: &mut BTreeMap<String, SlotBuilder>, reg: &mut Registry| {
        let v: serde_json::Value = match serde_json::from_str(&f.text) {
            Ok(v) => v,
            Err(e) => {
                reg.errors.push(format!("{}:{}: {e}", f.namespace, f.path));
                return;
            }
        };
        if v.get("conditions").and_then(|c| c.as_array()).is_some_and(|c| !c.is_empty()) {
            // Forge conditions are not evaluated here: such a file is skipped (see README).
            reg.errors.push(format!("{}:{}: conditions skipped", f.namespace, f.path));
            return;
        }
        let id = f.path.clone();
        let b = map.entry(id.clone()).or_insert_with(|| SlotBuilder::new(&id));
        if let Err(e) = slot_from_json(b, &v) {
            reg.errors.push(format!("{}:{}: {e}", f.namespace, f.path));
        }
    };
    for f in files.iter().filter(|f| f.namespace == "curios") {
        load(f, &mut map, &mut reg);
    }
    let legacy = legacy_builders(&src.legacy_register, &src.legacy_modify);
    for (id, lb) in &legacy {
        map.entry(id.clone()).or_insert_with(|| SlotBuilder::new(id)).apply(lb);
    }
    for f in files.iter().filter(|f| f.namespace != "curios" && !f.path.starts_with('_')) {
        load(f, &mut map, &mut reg);
    }
    let config: Vec<&str> = src.config.iter().map(String::as_str).collect();
    let config_slots = match slots_from_config(&mut map, &config) {
        Ok(s) => s,
        Err(e) => {
            reg.errors.push(format!("config: {e}"));
            BTreeSet::new()
        }
    };
    reg.slots = map.into_iter().map(|(k, b)| (k, b.build())).collect();
    // Entities: the legacy slots go to players, then the files, then the config slots.
    let mut ents: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for id in legacy.keys() {
        if reg.slots.contains_key(id) {
            ents.entry(PLAYER.into()).or_default().insert(id.clone());
        }
    }
    let mut efiles: Vec<&DataFile> = src.entities.iter().filter(|f| !f.path.starts_with('_')).collect();
    efiles.sort_by(|a, b| (&a.namespace, &a.path).cmp(&(&b.namespace, &b.path)));
    for f in efiles {
        let v: serde_json::Value = match serde_json::from_str(&f.text) {
            Ok(v) => v,
            Err(e) => {
                reg.errors.push(format!("{}:{}: {e}", f.namespace, f.path));
                continue;
            }
        };
        if v.get("conditions").and_then(|c| c.as_array()).is_some_and(|c| !c.is_empty()) {
            reg.errors.push(format!("{}:{}: conditions skipped", f.namespace, f.path));
            continue;
        }
        let names: Vec<String> = v
            .get("entities")
            .and_then(|e| e.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()).map(full_id).collect())
            .unwrap_or_default();
        let slots: BTreeSet<String> = v
            .get("slots")
            .and_then(|e| e.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()).filter(|s| reg.slots.contains_key(*s)).map(str::to_owned).collect())
            .unwrap_or_default();
        let replace = v.get("replace").and_then(|x| x.as_bool()).unwrap_or(false);
        for e in names.iter().filter(|e| !e.starts_with('#')) {
            let set = ents.entry(e.clone()).or_default();
            if replace {
                set.clear();
            }
            set.extend(slots.iter().cloned());
        }
    }
    for id in config_slots {
        if reg.slots.contains_key(&id) {
            ents.entry(PLAYER.into()).or_default().insert(id);
        }
    }
    reg.entities = ents;
    reg
}

impl Registry {
    /// The entity's slot types in `SlotType.compareTo` order (the TreeSet `reset` walks).
    pub fn entity_slots(&self, entity: &str) -> Vec<SlotType> {
        let mut v: Vec<SlotType> = self
            .entities
            .get(entity)
            .into_iter()
            .flatten()
            .filter_map(|id| self.slots.get(id).cloned())
            .collect();
        v.sort_by(|a, b| a.key().cmp(&b.key()));
        v
    }
    pub fn player_slots(&self) -> Vec<SlotType> {
        self.entity_slots(PLAYER)
    }
}

// =================================================================================================
// Items, as the rules see them
// =================================================================================================

/// An item stack of the game (count and copy are all the rules touch directly).
pub trait Stack: Clone + std::fmt::Debug {
    fn count(&self) -> u32;
    fn set_count(&mut self, n: u32);
}

/// Where a curio is: `SlotContext` (identifier, index, cosmetic, visible).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SlotCtx {
    pub id: String,
    pub index: usize,
    pub cosmetic: bool,
    pub visible: bool,
}

/// What the rules ask about items. The adapter answers from the game's catalog, tags and the
/// curio items other native mods registered (the `ICurio` of the original).
pub trait Items<S: Stack> {
    fn max_stack(&self, s: &S) -> u32;
    /// `ItemStack.isSameItem`.
    fn same_item(&self, a: &S, b: &S) -> bool;
    /// `ItemStack.isSameItemSameTags` (item and components).
    fn same_components(&self, a: &S, b: &S) -> bool;
    /// `ItemStack.matches`: same components and count.
    fn matches(&self, a: &S, b: &S) -> bool {
        self.same_components(a, b) && a.count() == b.count()
    }
    /// `ItemStack.isStackable`: a max stack over 1 and not damaged.
    fn stackable(&self, s: &S) -> bool;
    fn in_tag(&self, s: &S, tag: &str) -> bool;
    /// Any item tag of the `curios` namespace.
    fn has_curios_tag(&self, s: &S) -> bool;
    /// The armour slot the item equips in: 0 head .. 3 feet (`ItemStack.canEquip` for a player).
    fn armor_slot(&self, s: &S) -> Option<usize>;
    /// `Mob.getEquipmentSlotForItem == OFFHAND` (shields).
    fn offhand_item(&self, s: &S) -> bool;
    fn binding_curse(&self, s: &S) -> bool;
    fn vanishing_curse(&self, s: &S) -> bool;
    /// `CuriosApi.getCurio(stack).isPresent()`: a curio item another mod registered.
    fn is_curio(&self, _s: &S) -> bool {
        false
    }
    /// `CurioEquipEvent`: Some(true) = ALLOW, Some(false) = DENY.
    fn equip_event(&self, _ctx: &SlotCtx, _s: &S) -> Option<bool> {
        None
    }
    /// `ICurio.canEquip` (true by default).
    fn can_equip(&self, _ctx: &SlotCtx, _s: &S) -> bool {
        true
    }
    /// `CurioUnequipEvent`: Some(true) = ALLOW, Some(false) = DENY.
    fn unequip_event(&self, _ctx: &SlotCtx, _s: &S) -> Option<bool> {
        None
    }
    /// `ICurio.canUnequip` (true by default).
    fn can_unequip(&self, _ctx: &SlotCtx, _s: &S) -> bool {
        true
    }
    /// A validator other than `curios:all`, `curios:none`, `curios:tag` (`registerCurioPredicate`).
    fn predicate(&self, _validator: &str, _ctx: &SlotCtx, _s: &S) -> Option<bool> {
        None
    }
}

/// `CuriosApi.testCurioPredicates` for one slot type.
pub fn passes_validators<S: Stack>(items: &impl Items<S>, t: &SlotType, ctx: &SlotCtx, s: &S) -> bool {
    t.validators.iter().any(|v| match v.as_str() {
        "curios:all" => true,
        "curios:none" => false,
        "curios:tag" => items.in_tag(s, &format!("curios:{}", t.id)) || items.in_tag(s, "curios:curio"),
        other => items.predicate(other, ctx, s).unwrap_or(false),
    })
}

/// `CuriosApi.getItemStackSlots(stack, entity)`: the entity's slot types the stack passes.
pub fn stack_slots<'a, S: Stack>(items: &impl Items<S>, slots: &'a [SlotType], s: &S) -> Vec<&'a SlotType> {
    slots
        .iter()
        .filter(|t| {
            let ctx = SlotCtx { id: t.id.clone(), index: 0, cosmetic: false, visible: true };
            passes_validators(items, t, &ctx, s)
        })
        .collect()
}

/// `CuriosImplMixinHooks.isStackValid`.
pub fn stack_valid<S: Stack>(items: &impl Items<S>, entity_slots: &[SlotType], all_slots: &[SlotType], ctx: &SlotCtx, s: &S) -> bool {
    let slots = stack_slots(items, entity_slots, s);
    if !slots.is_empty() {
        return ctx.id == "curio" || slots.iter().any(|t| t.id == ctx.id || t.id == "curio");
    }
    if ctx.id == "curio" {
        if items.has_curios_tag(s) {
            return true;
        }
        for t in all_slots {
            for v in &t.validators {
                let ok = match v.as_str() {
                    "curios:all" => true,
                    "curios:none" => false,
                    "curios:tag" => items.in_tag(s, &format!("curios:{}", t.id)) || items.in_tag(s, "curios:curio"),
                    other => items.predicate(other, ctx, s).unwrap_or(false),
                };
                if ok {
                    return true;
                }
            }
        }
        return items.is_curio(s);
    }
    false
}

/// `DynamicStackHandler.isItemValid`.
pub fn may_equip<S: Stack>(items: &impl Items<S>, entity_slots: &[SlotType], all_slots: &[SlotType], ctx: &SlotCtx, s: &S) -> bool {
    match items.equip_event(ctx, s) {
        Some(r) => r,
        None => stack_valid(items, entity_slots, all_slots, ctx, s) && items.can_equip(ctx, s),
    }
}

/// `DynamicStackHandler.extractItem(.., simulate)` not empty: the stack may leave its slot.
pub fn may_unequip<S: Stack>(items: &impl Items<S>, ctx: &SlotCtx, s: &S, creative: bool) -> bool {
    match items.unequip_event(ctx, s) {
        Some(r) => r,
        None => (creative || !items.binding_curse(s)) && items.can_unequip(ctx, s),
    }
}

// =================================================================================================
// The inventory (CurioStacksHandler, CurioInventoryWrapper)
// =================================================================================================

/// An `AttributeModifier` on a slot type's size (`SlotAttribute`).
#[derive(Clone, Debug, PartialEq)]
pub struct SlotModifier {
    /// The modifier's identity: its name, or the UUID it was saved with (`u:<4 ints>`).
    pub id: String,
    pub name: String,
    pub amount: f64,
    /// 0 ADDITION, 1 MULTIPLY_BASE, 2 MULTIPLY_TOTAL.
    pub operation: u8,
    /// The UUID of a modifier read from a 1.20.1 save (written back unchanged).
    pub uuid: Option<[i32; 4]>,
}
/// `CurioStacksHandler.LEGACY_UUID` = 0b0eabbd-4220-4e9f-bafb-34100da2bd7e: the `/curios add|set|remove`
/// and `SizeShift` modifier.
pub const LEGACY_UUID: [i32; 4] = [0x0b0eabbd, 0x42204e9f, 0xbafb3410_u32 as i32, 0x0da2bd7e];
pub const LEGACY_ID: &str = "u:0b0eabbd-4220-4e9f-bafb-34100da2bd7e";

/// One slot type of a player (`CurioStacksHandler`).
#[derive(Clone, Debug)]
pub struct Handler<S> {
    pub id: String,
    pub base_size: i32,
    pub stacks: Vec<Option<S>>,
    pub cosmetics: Vec<Option<S>>,
    pub renders: Vec<bool>,
    pub visible: bool,
    pub cosmetic: bool,
    pub render_toggle: bool,
    pub drop_rule: DropRule,
    /// `persistentModifiers` (commands, `SizeShift`).
    pub persistent: Vec<SlotModifier>,
    /// The transient modifiers: from the worn curios (and, until the first tick after loading, the
    /// `CachedModifiers` of the save).
    pub transient: Vec<SlotModifier>,
}
impl<S: Stack> Handler<S> {
    /// `new CurioStacksHandler(handler, id, size, visible, cosmetic, toggle, dropRule)`.
    pub fn new(t: &SlotType) -> Self {
        let n = t.size.max(0) as usize;
        Handler {
            id: t.id.clone(),
            base_size: t.size,
            stacks: vec![None; n],
            cosmetics: vec![None; n],
            renders: vec![true; n],
            visible: t.native_gui,
            cosmetic: t.cosmetic,
            render_toggle: t.render_toggle,
            drop_rule: t.drop_rule,
            persistent: Vec::new(),
            transient: Vec::new(),
        }
    }
    pub fn slots(&self) -> usize {
        self.stacks.len()
    }
    /// `CurioStacksHandler.update`'s size: base, + additions, + base x multiply-base, x multiply-total.
    pub fn wanted_size(&self) -> usize {
        let all: Vec<&SlotModifier> = self.persistent.iter().chain(self.transient.iter()).collect();
        let mut base = f64::from(self.base_size);
        for m in all.iter().filter(|m| m.operation == 0) {
            base += m.amount;
        }
        let mut size = base;
        for m in all.iter().filter(|m| m.operation == 1) {
            size += f64::from(self.base_size) * m.amount;
        }
        for m in all.iter().filter(|m| m.operation == 2) {
            size *= m.amount;
        }
        size.max(0.0) as usize
    }
    /// `resize`: grows with empty slots, shrinks losing the last ones (returned, never dropped here).
    pub fn resize(&mut self, n: usize) -> Vec<S> {
        let mut lost = Vec::new();
        if n < self.stacks.len() {
            lost.extend(self.stacks.drain(n..).flatten());
            lost.extend(self.cosmetics.drain(n..).flatten());
        } else {
            self.stacks.resize(n, None);
            self.cosmetics.resize(n, None);
        }
        self.cosmetics.resize(n, None);
        self.renders.resize(n, true);
        lost
    }
    /// The size again from the modifiers (lost stacks returned).
    pub fn update(&mut self) -> Vec<S> {
        let n = self.wanted_size();
        if n != self.slots() { self.resize(n) } else { Vec::new() }
    }
    /// `addLegacyChange` (`grow` / `shrink` and the commands).
    pub fn legacy_change(&mut self, shift: i32) {
        let current = self.persistent.iter().find(|m| m.id == LEGACY_ID).map_or(0.0, |m| m.amount);
        self.persistent.retain(|m| m.id != LEGACY_ID);
        self.persistent.push(SlotModifier {
            id: LEGACY_ID.into(),
            name: "legacy".into(),
            amount: current + f64::from(shift),
            operation: 0,
            uuid: Some(LEGACY_UUID),
        });
    }
}

/// What a save holds for one slot type (`CurioStacksHandler.serializeNBT`), each key optional.
#[derive(Clone, Debug)]
pub struct Saved<S> {
    pub id: String,
    pub base_size: Option<i32>,
    pub stacks: Option<Vec<Option<S>>>,
    pub cosmetics: Option<Vec<Option<S>>>,
    /// `Renders.Size` and the (slot, render) pairs.
    pub renders: Option<(Option<usize>, Vec<(usize, bool)>)>,
    pub size_shift: Option<i32>,
    pub cosmetic: Option<bool>,
    pub visible: Option<bool>,
    pub render_toggle: Option<bool>,
    pub drop_rule: Option<String>,
    pub persistent: Vec<SlotModifier>,
    pub cached: Vec<SlotModifier>,
}

impl<S> Default for Saved<S> {
    fn default() -> Self {
        Saved {
            id: String::new(),
            base_size: None,
            stacks: None,
            cosmetics: None,
            renders: None,
            size_shift: None,
            cosmetic: None,
            visible: None,
            render_toggle: None,
            drop_rule: None,
            persistent: Vec::new(),
            cached: Vec::new(),
        }
    }
}

/// A player's curios (`CurioInventoryWrapper.curios`, in slot-type order).
#[derive(Clone, Debug)]
pub struct Inventory<S> {
    pub handlers: Vec<Handler<S>>,
}
impl<S: Stack> Default for Inventory<S> {
    fn default() -> Self {
        Inventory { handlers: Vec::new() }
    }
}
impl<S: Stack> Inventory<S> {
    /// `reset`: one handler per slot type, in order.
    pub fn new(types: &[SlotType]) -> Self {
        let mut t: Vec<&SlotType> = types.iter().collect();
        t.sort_by(|a, b| a.key().cmp(&b.key()));
        Inventory { handlers: t.into_iter().map(Handler::new).collect() }
    }
    pub fn handler(&self, id: &str) -> Option<&Handler<S>> {
        self.handlers.iter().find(|h| h.id == id)
    }
    pub fn handler_mut(&mut self, id: &str) -> Option<&mut Handler<S>> {
        self.handlers.iter_mut().find(|h| h.id == id)
    }
    /// `getSlots`.
    pub fn slots(&self) -> usize {
        self.handlers.iter().map(Handler::slots).sum()
    }
    /// `getVisibleSlots`.
    pub fn visible_slots(&self) -> usize {
        self.handlers.iter().filter(|h| h.visible).map(Handler::slots).sum()
    }
    /// Every worn stack: (handler index, slot index, cosmetic, stack).
    pub fn worn(&self) -> Vec<(usize, usize, bool, &S)> {
        let mut out = Vec::new();
        for (hi, h) in self.handlers.iter().enumerate() {
            for (i, s) in h.stacks.iter().enumerate() {
                if let Some(s) = s {
                    out.push((hi, i, false, s));
                }
            }
            for (i, s) in h.cosmetics.iter().enumerate() {
                if let Some(s) = s {
                    out.push((hi, i, true, s));
                }
            }
        }
        out
    }
    /// The slot context of a handler's slot.
    pub fn ctx(&self, hi: usize, index: usize, cosmetic: bool) -> SlotCtx {
        let h = &self.handlers[hi];
        SlotCtx {
            id: h.id.clone(),
            index,
            cosmetic,
            visible: cosmetic || h.renders.get(index).copied().unwrap_or(true),
        }
    }
    /// `readTag`: the saved handlers over the current slot types. Stacks that no longer fit (slot
    /// type gone, slot index gone, no longer valid) are returned (`loseInvalidStack`), never lost.
    pub fn load(types: &[SlotType], saved: Vec<Saved<S>>, valid: impl Fn(&SlotCtx, &S) -> bool) -> (Self, Vec<S>) {
        let mut inv = Inventory::new(types);
        let mut lost = Vec::new();
        if saved.is_empty() {
            return (inv, lost);
        }
        for sv in saved {
            // prevStacksHandler = new CurioStacksHandler(this, identifier).deserializeNBT(tag)
            let base = sv.base_size.unwrap_or(1);
            let prev_stacks = sv.stacks.unwrap_or_else(|| vec![None; base.max(0) as usize]);
            let prev_cos = sv.cosmetics.unwrap_or_default();
            let mut prev_renders = vec![true; prev_stacks.len()];
            if let Some((size, pairs)) = &sv.renders {
                prev_renders = vec![true; size.unwrap_or(prev_stacks.len())];
                for (slot, r) in pairs {
                    if let Some(x) = prev_renders.get_mut(*slot) {
                        *x = *r;
                    }
                }
            }
            let mut persistent = sv.persistent.clone();
            if let Some(shift) = sv.size_shift.filter(|s| *s != 0) {
                let mut tmp: Handler<S> = Handler {
                    id: sv.id.clone(),
                    base_size: base,
                    stacks: Vec::new(),
                    cosmetics: Vec::new(),
                    renders: Vec::new(),
                    visible: true,
                    cosmetic: false,
                    render_toggle: true,
                    drop_rule: DropRule::Default,
                    persistent: persistent.clone(),
                    transient: Vec::new(),
                };
                tmp.legacy_change(shift);
                persistent = tmp.persistent;
            }
            let Some(hi) = inv.handlers.iter().position(|h| h.id == sv.id) else {
                lost.extend(prev_stacks.into_iter().flatten());
                lost.extend(prev_cos.into_iter().flatten());
                continue;
            };
            {
                let h = &mut inv.handlers[hi];
                h.persistent = persistent;
                h.transient = sv.cached.clone();
                let n = h.wanted_size();
                h.resize(n);
            }
            let n = inv.handlers[hi].slots();
            let mut index = 0;
            while index < n && index < prev_stacks.len() {
                if let Some(s) = prev_stacks[index].clone() {
                    let ctx = inv.ctx(hi, index, false);
                    if valid(&ctx, &s) {
                        inv.handlers[hi].stacks[index] = Some(s);
                    } else {
                        lost.push(s);
                    }
                }
                if let Some(s) = prev_cos.get(index).cloned().flatten() {
                    // The original checks the cosmetic stack against the main stacks' validity.
                    let ctx = inv.ctx(hi, index, false);
                    if valid(&ctx, &s) {
                        inv.handlers[hi].cosmetics[index] = Some(s);
                    } else {
                        lost.push(s);
                    }
                }
                index += 1;
            }
            while index < prev_stacks.len().max(prev_cos.len()) {
                lost.extend(prev_stacks.get(index).cloned().flatten());
                lost.extend(prev_cos.get(index).cloned().flatten());
                index += 1;
            }
            let h = &mut inv.handlers[hi];
            for j in 0..h.renders.len().min(prev_renders.len()) {
                h.renders[j] = prev_renders[j];
            }
        }
        (inv, lost)
    }
    /// The transient slot modifiers the worn curios give now (target slot -> modifiers): set them
    /// (the cached ones of a save go) and resize. Returns the stacks the shrinking slots lost.
    pub fn set_transient(&mut self, wanted: &BTreeMap<String, Vec<SlotModifier>>) -> (bool, Vec<S>) {
        let mut changed = false;
        let mut lost = Vec::new();
        for h in &mut self.handlers {
            let w = wanted.get(&h.id).cloned().unwrap_or_default();
            if w != h.transient {
                h.transient = w;
                changed = true;
            }
            let before = h.slots();
            lost.extend(h.update());
            changed |= before != h.slots();
        }
        (changed, lost)
    }
    /// Resize every handler from its modifiers.
    pub fn update(&mut self) -> Vec<S> {
        self.handlers.iter_mut().flat_map(|h| h.update()).collect()
    }
}

/// A change of a worn stack since the last tick (`CuriosEventHandler.tick`).
#[derive(Clone, Debug)]
pub struct Change<S> {
    pub ctx: SlotCtx,
    pub old: Option<S>,
    pub new: Option<S>,
}
/// The stacks of the last tick, per slot (`previousStacks`, not saved: after loading every worn
/// curio counts as just equipped, as in the original).
pub type Previous<S> = BTreeMap<(String, usize, bool), S>;

/// `ItemStack.matches(stack, prevStack)` for every slot; `previous` is brought up to date.
pub fn changes<S: Stack>(inv: &Inventory<S>, previous: &mut Previous<S>, items: &impl Items<S>) -> Vec<Change<S>> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<(String, usize, bool)> = BTreeSet::new();
    for (hi, h) in inv.handlers.iter().enumerate() {
        for cosmetic in [false, true] {
            let list = if cosmetic { &h.cosmetics } else { &h.stacks };
            for (i, s) in list.iter().enumerate() {
                let key = (h.id.clone(), i, cosmetic);
                seen.insert(key.clone());
                let prev = previous.get(&key);
                let same = match (s, prev) {
                    (None, None) => true,
                    (Some(a), Some(b)) => items.matches(a, b),
                    _ => false,
                };
                if !same {
                    out.push(Change { ctx: inv.ctx(hi, i, cosmetic), old: prev.cloned(), new: s.clone() });
                    match s {
                        Some(s) => {
                            previous.insert(key, s.clone());
                        }
                        None => {
                            previous.remove(&key);
                        }
                    }
                }
            }
        }
    }
    // Slots that went away: what was worn there is unequipped.
    let gone: Vec<(String, usize, bool)> = previous.keys().filter(|k| !seen.contains(*k)).cloned().collect();
    for key in gone {
        if let Some(old) = previous.remove(&key) {
            out.push(Change {
                ctx: SlotCtx { id: key.0.clone(), index: key.1, cosmetic: key.2, visible: true },
                old: Some(old),
                new: None,
            });
        }
    }
    out
}

// =================================================================================================
// Death (CuriosEventHandler.playerDrops / handleDrops)
// =================================================================================================

/// `CuriosConfig.KeepCurios`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeepCurios {
    On,
    Default,
    Off,
}
impl KeepCurios {
    pub fn parse(s: &str) -> Self {
        match s {
            "ON" => KeepCurios::On,
            "OFF" => KeepCurios::Off,
            _ => KeepCurios::Default,
        }
    }
}

/// What dies with the player: the stacks to drop (in order), the ones destroyed. Kept stacks stay.
/// `rule` = the curio's own rule or a `DropRulesEvent` override (Default when none).
pub fn death_drops<S: Stack>(
    inv: &mut Inventory<S>,
    items: &impl Items<S>,
    keep_inventory_rule: bool,
    keep: KeepCurios,
    rule: impl Fn(&SlotCtx, &S) -> DropRule,
) -> (Vec<S>, Vec<S>) {
    let keep_inventory = match keep {
        KeepCurios::Default => keep_inventory_rule,
        KeepCurios::On => true,
        KeepCurios::Off => false,
    };
    let mut drops = Vec::new();
    let mut destroyed = Vec::new();
    for hi in 0..inv.handlers.len() {
        for cosmetic in [false, true] {
            let n = inv.handlers[hi].slots();
            for i in 0..n {
                let ctx = inv.ctx(hi, i, cosmetic);
                let h = &mut inv.handlers[hi];
                let list = if cosmetic { &mut h.cosmetics } else { &mut h.stacks };
                let Some(stack) = list[i].clone() else { continue };
                let mut r = rule(&ctx, &stack);
                if r == DropRule::Default {
                    r = h.drop_rule;
                }
                if (r == DropRule::Default && keep_inventory) || r == DropRule::AlwaysKeep {
                    continue;
                }
                let list = if cosmetic { &mut h.cosmetics } else { &mut h.stacks };
                list[i] = None;
                if !items.vanishing_curse(&stack) && r != DropRule::Destroy {
                    drops.push(stack);
                } else {
                    destroyed.push(stack);
                }
            }
        }
    }
    (drops, destroyed)
}

// =================================================================================================
// The menu (CuriosContainerV2 over CuriosContainer, AbstractContainerMenu)
// =================================================================================================

/// A curio slot shown on the current page.
#[derive(Clone, Debug, PartialEq)]
pub struct PageSlot {
    pub handler: usize,
    pub id: String,
    pub index: usize,
    pub cosmetic: bool,
    /// Relative to the screen's left/top (`CurioSlot.x/y`).
    pub x: i32,
    pub y: i32,
    pub render_toggle: bool,
    pub render: bool,
}

/// `CuriosContainerV2.setPage`'s result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub page: i32,
    pub total_pages: i32,
    pub columns: i32,
    pub panel_width: i32,
    /// Slots per column.
    pub grid: Vec<i32>,
    pub slots: Vec<PageSlot>,
    /// The slots of the other pages (page, slot).
    pub proxies: Vec<(i32, PageSlot)>,
    pub has_cosmetics: bool,
    pub viewing_cosmetics: bool,
}

/// `setPage(page)`.
pub fn layout<S: Stack>(inv: &Inventory<S>, page: i32, viewing_cosmetics: bool, max_per_page: i32, min_columns: i32) -> Layout {
    let max_per_page = max_per_page.clamp(1, 48);
    let visible = inv.visible_slots() as i32;
    let starting = page * max_per_page;
    let on_page = max_per_page.min(visible - starting);
    let calculated = (f64::from(on_page) / 8.0).ceil() as i32;
    let minimum = on_page.min(min_columns.clamp(1, 8));
    let columns = calculated.max(minimum).min(8);
    let panel_width = 14 + 18 * columns;
    let total_pages = (f64::from(visible) / f64::from(max_per_page)).ceil() as i32;
    let mut l = Layout {
        page,
        total_pages,
        columns,
        panel_width,
        viewing_cosmetics,
        ..Layout::default()
    };
    let mut y_offset = 8;
    if total_pages > 1 {
        y_offset += 8;
    }
    let (mut column, mut row, mut slots, mut current_page, mut index) = (1, 1, 0, 0, 0);
    let ending = starting + max_per_page;
    for (hi, h) in inv.handlers.iter().enumerate() {
        let mut cosmetic = false;
        if h.cosmetic {
            l.has_cosmetics = true;
            if viewing_cosmetics {
                cosmetic = true;
            }
        }
        if !h.visible {
            continue;
        }
        for i in 0..h.slots() {
            let slot = PageSlot {
                handler: hi,
                id: h.id.clone(),
                index: i,
                cosmetic,
                x: (column - 1) * 18 + 7 - panel_width,
                y: y_offset + (row - 1) * 18,
                render_toggle: h.render_toggle,
                render: h.renders.get(i).copied().unwrap_or(true),
            };
            if index >= starting && index < ending {
                l.slots.push(slot);
                if (l.grid.len() as i32) < column {
                    l.grid.push(1);
                } else {
                    l.grid[(column - 1) as usize] += 1;
                }
                if column == columns {
                    column = 1;
                    row += 1;
                } else {
                    column += 1;
                }
            } else {
                l.proxies.push((current_page, slot));
            }
            slots += 1;
            if slots >= max_per_page {
                slots = 0;
                current_page += 1;
            }
            index += 1;
        }
    }
    l
}

/// The V2 menu's slot numbers: 0 result, 1-4 crafting (not in this port), 5-8 armour (head..feet),
/// 9-35 inventory, 36-44 hotbar, 45 off hand, 46.. the page's curio slots.
pub const MENU_ARMOR: usize = 5;
pub const MENU_INVENTORY: usize = 9;
pub const MENU_HOTBAR: usize = 36;
pub const MENU_OFFHAND: usize = 45;
pub const MENU_CURIOS: usize = 46;
pub const OUTSIDE: i32 = -999;

/// `ClickType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    Pickup,
    QuickMove,
    Swap,
    Clone,
    Throw,
    QuickCraft,
    PickupAll,
}

/// `quickcraftStatus`, `quickcraftType`, `quickcraftSlots` of a menu.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QuickCraft {
    pub status: i32,
    pub kind: i32,
    pub slots: Vec<usize>,
}

/// The player's side of the menu: everything a click may change.
pub struct Player<'a, S> {
    /// 36 slots: 0..9 hotbar, 9..36 inventory.
    pub inventory: &'a mut [Option<S>],
    /// 4 slots: head, chest, legs, feet.
    pub armor: &'a mut [Option<S>],
    pub offhand: &'a mut Option<S>,
    pub carried: &'a mut Option<S>,
    pub creative: bool,
    /// What the player threw (`player.drop(stack, true)`), in order.
    pub thrown: Vec<S>,
}

/// One open Curios menu: the player's slots, the curios, the page.
pub struct Menu<'a, 'p, S: Stack, I: Items<S>> {
    pub items: &'a I,
    pub player: &'a mut Player<'p, S>,
    pub inv: &'a mut Inventory<S>,
    pub layout: &'a Layout,
    pub entity_slots: &'a [SlotType],
    pub all_slots: &'a [SlotType],
    pub quick: &'a mut QuickCraft,
    /// `findAvailableSlot`'s page when the item fits only on another page (`moveToPage`).
    pub move_to_page: Option<(i32, usize)>,
    /// Page slots a click put a new stack in (`CurioSlot.set` -> `onEquipFromUse`).
    pub equipped: Vec<usize>,
}
impl<'a, 'p, S: Stack, I: Items<S>> Menu<'a, 'p, S, I> {
    pub fn new(
        items: &'a I,
        player: &'a mut Player<'p, S>,
        inv: &'a mut Inventory<S>,
        layout: &'a Layout,
        entity_slots: &'a [SlotType],
        all_slots: &'a [SlotType],
        quick: &'a mut QuickCraft,
    ) -> Self {
        Menu { items, player, inv, layout, entity_slots, all_slots, quick, move_to_page: None, equipped: Vec::new() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Absent,
    Armor(usize),
    Plain(usize),
    Offhand,
    Curio(usize),
}

impl<'a, 'p, S: Stack, I: Items<S>> Menu<'a, 'p, S, I> {
    pub fn slot_count(&self) -> usize {
        MENU_CURIOS + self.layout.slots.len()
    }
    fn kind(&self, i: usize) -> Kind {
        match i {
            0..=4 => Kind::Absent,
            5..=8 => Kind::Armor(i - 5),
            9..=35 => Kind::Plain(i),
            36..=44 => Kind::Plain(i - 36),
            45 => Kind::Offhand,
            _ if i - MENU_CURIOS < self.layout.slots.len() => Kind::Curio(i - MENU_CURIOS),
            _ => Kind::Absent,
        }
    }
    fn curio_ctx(&self, j: usize) -> SlotCtx {
        let p = &self.layout.slots[j];
        self.inv.ctx(p.handler, p.index, p.cosmetic)
    }
    pub fn get(&self, i: usize) -> Option<&S> {
        match self.kind(i) {
            Kind::Absent => None,
            Kind::Armor(k) => self.player.armor.get(k).and_then(|s| s.as_ref()),
            Kind::Plain(k) => self.player.inventory.get(k).and_then(|s| s.as_ref()),
            Kind::Offhand => self.player.offhand.as_ref(),
            Kind::Curio(j) => {
                let p = &self.layout.slots[j];
                let h = self.inv.handlers.get(p.handler)?;
                let list = if p.cosmetic { &h.cosmetics } else { &h.stacks };
                list.get(p.index).and_then(|s| s.as_ref())
            }
        }
    }
    fn cell(&mut self, i: usize) -> Option<&mut Option<S>> {
        match self.kind(i) {
            Kind::Absent => None,
            Kind::Armor(k) => self.player.armor.get_mut(k),
            Kind::Plain(k) => self.player.inventory.get_mut(k),
            Kind::Offhand => Some(&mut *self.player.offhand),
            Kind::Curio(j) => {
                let p = self.layout.slots[j].clone();
                let h = self.inv.handlers.get_mut(p.handler)?;
                let list = if p.cosmetic { &mut h.cosmetics } else { &mut h.stacks };
                list.get_mut(p.index)
            }
        }
    }
    fn set(&mut self, i: usize, s: Option<S>) {
        let s = s.filter(|s| s.count() > 0);
        if let Kind::Curio(j) = self.kind(i) {
            let changed = match (self.get(i), s.as_ref()) {
                (None, None) => false,
                (Some(a), Some(b)) => !self.items.matches(a, b),
                _ => true,
            };
            // CurioSlot.set: a stack put in by the player (`onEquipFromUse`).
            if changed && s.is_some() && !self.equipped.contains(&j) {
                self.equipped.push(j);
            }
        }
        if let Some(c) = self.cell(i) {
            *c = s;
        }
    }
    fn has(&self, i: usize) -> bool {
        self.get(i).is_some()
    }
    /// `Slot.getMaxStackSize()`: armour 1, the rest 64 (`SlotItemHandler` = the handler's 64).
    fn slot_max(&self, i: usize) -> u32 {
        match self.kind(i) {
            Kind::Armor(_) => 1,
            _ => 64,
        }
    }
    /// `Slot.getMaxStackSize(stack)`.
    fn slot_max_for(&self, i: usize, s: &S) -> u32 {
        self.slot_max(i).min(self.items.max_stack(s))
    }
    /// `Slot.mayPlace`.
    pub fn may_place(&self, i: usize, s: &S) -> bool {
        match self.kind(i) {
            Kind::Absent => false,
            Kind::Armor(k) => self.items.armor_slot(s) == Some(k),
            Kind::Plain(_) | Kind::Offhand => true,
            Kind::Curio(j) => {
                let ctx = self.curio_ctx(j);
                may_equip(self.items, self.entity_slots, self.all_slots, &ctx, s)
            }
        }
    }
    /// `Slot.mayPickup`.
    pub fn may_pickup(&self, i: usize) -> bool {
        match self.kind(i) {
            Kind::Absent => false,
            Kind::Armor(_) => match self.get(i) {
                None => true,
                Some(s) => self.player.creative || !self.items.binding_curse(s),
            },
            Kind::Plain(_) | Kind::Offhand => true,
            Kind::Curio(j) => match self.get(i) {
                // SlotItemHandler.mayPickup: extractItem(.., 1, true) not empty.
                None => false,
                Some(s) => may_unequip(self.items, &self.curio_ctx(j), s, self.player.creative),
            },
        }
    }
    /// `Slot.safeInsert(stack, count)`: the rest of `stack`.
    fn safe_insert(&mut self, i: usize, mut stack: S, count: u32) -> Option<S> {
        if stack.count() == 0 || !self.may_place(i, &stack) {
            return Some(stack);
        }
        let have = self.get(i).cloned();
        let room = self.slot_max_for(i, &stack).saturating_sub(have.as_ref().map_or(0, Stack::count));
        let n = count.min(stack.count()).min(room);
        match have {
            None => {
                let mut put = stack.clone();
                put.set_count(n);
                self.set(i, Some(put));
                stack.set_count(stack.count() - n);
            }
            Some(mut h) if self.items.same_components(&h, &stack) => {
                stack.set_count(stack.count() - n);
                h.set_count(h.count() + n);
                self.set(i, Some(h));
            }
            Some(_) => {}
        }
        (stack.count() > 0).then_some(stack)
    }
    /// `Slot.tryRemove(count, decrement)` (+ `onTake`).
    fn try_remove(&mut self, i: usize, count: u32, decrement: u32) -> Option<S> {
        if !self.may_pickup(i) {
            return None;
        }
        let count = count.min(decrement);
        let mut have = self.get(i).cloned()?;
        let n = count.min(have.count());
        if n == 0 {
            return None;
        }
        let mut taken = have.clone();
        taken.set_count(n);
        have.set_count(have.count() - n);
        self.set(i, (have.count() > 0).then_some(have));
        Some(taken)
    }
    /// `moveItemStackTo(stack, start, end, reverse)`.
    fn move_to(&mut self, stack: &mut S, start: usize, end: usize, reverse: bool) -> bool {
        let mut moved = false;
        let end = end.min(self.slot_count());
        if start >= end {
            return false;
        }
        let order: Vec<usize> = if reverse { (start..end).rev().collect() } else { (start..end).collect() };
        if self.items.stackable(stack) {
            for &i in &order {
                if stack.count() == 0 {
                    break;
                }
                let Some(mut have) = self.get(i).cloned() else { continue };
                if !self.items.same_components(stack, &have) {
                    continue;
                }
                let total = have.count() + stack.count();
                let max = self.slot_max(i).min(self.items.max_stack(stack));
                if total <= max {
                    stack.set_count(0);
                    have.set_count(total);
                    self.set(i, Some(have));
                    moved = true;
                } else if have.count() < max {
                    stack.set_count(stack.count() - (max - have.count()));
                    have.set_count(max);
                    self.set(i, Some(have));
                    moved = true;
                }
            }
        }
        if stack.count() > 0 {
            for &i in &order {
                if self.has(i) || !self.may_place(i, stack) {
                    continue;
                }
                let max = self.slot_max(i);
                let n = stack.count().min(max);
                let mut put = stack.clone();
                put.set_count(n);
                stack.set_count(stack.count() - n);
                self.set(i, Some(put));
                moved = true;
                break;
            }
        }
        moved
    }
    /// `findAvailableSlot`: the page of another page's slot that takes the stack (-1 none).
    fn find_available_page(&self, stack: &S) -> Option<i32> {
        let get = |p: &PageSlot| {
            let h = &self.inv.handlers[p.handler];
            let list = if p.cosmetic { &h.cosmetics } else { &h.stacks };
            list.get(p.index).cloned().flatten()
        };
        if self.items.stackable(stack) {
            for (page, p) in &self.layout.proxies {
                if let Some(have) = get(p)
                    && self.items.same_components(stack, &have)
                {
                    let max = 64.min(self.items.max_stack(stack));
                    if have.count() + stack.count() <= max || have.count() < max {
                        return Some(*page);
                    }
                }
            }
        }
        for (page, p) in &self.layout.proxies {
            if get(p).is_none() {
                let ctx = self.inv.ctx(p.handler, p.index, p.cosmetic);
                if may_equip(self.items, self.entity_slots, self.all_slots, &ctx, stack) {
                    return Some(*page);
                }
            }
        }
        None
    }
    /// `CuriosContainerV2.quickMoveStack`: what was moved (None = nothing).
    pub fn quick_move_stack(&mut self, index: usize) -> Option<S> {
        let mut stack = self.get(index).cloned()?;
        let original = stack.clone();
        let armor = self.items.armor_slot(&stack);
        let n = self.slot_count();
        if index < 5 {
            if !self.move_to(&mut stack, 9, 45, false) {
                return None;
            }
        } else if index < 9 {
            if !self.move_to(&mut stack, 9, 45, false) {
                return None;
            }
        } else if let Some(k) = armor.filter(|k| !self.has(MENU_ARMOR + *k)) {
            let i = MENU_ARMOR + k;
            if !self.move_to(&mut stack, i, i + 1, false) {
                return None;
            }
        } else if index < 46 && !stack_slots(self.items, self.entity_slots, &stack).is_empty() {
            if !self.move_to(&mut stack, 46, n, false) {
                match self.find_available_page(&stack) {
                    Some(page) => {
                        self.move_to_page = Some((page, index));
                    }
                    None => return None,
                }
            }
        } else if self.items.offhand_item(&stack) && !self.has(MENU_OFFHAND) {
            if !self.move_to(&mut stack, 45, 46, false) {
                return None;
            }
        } else if index < 36 {
            if !self.move_to(&mut stack, 36, 45, false) {
                return None;
            }
        } else if index < 45 {
            if !self.move_to(&mut stack, 9, 36, false) {
                return None;
            }
        } else if !self.move_to(&mut stack, 9, 45, false) {
            return None;
        }
        self.set(index, (stack.count() > 0).then_some(stack.clone()));
        if stack.count() == original.count() {
            return None;
        }
        Some(original)
    }
    /// `canItemQuickReplace(slot, stack, true)`.
    fn quick_replace(&self, i: usize, carried: &S) -> bool {
        match self.get(i) {
            None => true,
            Some(s) => self.items.same_components(s, carried) && s.count() <= self.items.max_stack(carried),
        }
    }
    /// `getQuickCraftPlaceCount`.
    fn quick_craft_count(slots: usize, kind: i32, stack: &S, max: u32) -> u32 {
        match kind {
            0 => (stack.count() as f32 / slots as f32).floor() as u32,
            1 => 1,
            2 => max,
            _ => stack.count(),
        }
    }
    /// `Slot.isSameInventory`: the player's inventory (armour and off hand included), or the same
    /// slot type's stacks (`SlotItemHandler`: the same handler).
    fn same_inventory(&self, a: usize, b: usize) -> bool {
        match (self.kind(a), self.kind(b)) {
            (Kind::Curio(x), Kind::Curio(y)) => {
                let (p, q) = (&self.layout.slots[x], &self.layout.slots[y]);
                p.handler == q.handler && p.cosmetic == q.cosmetic
            }
            (Kind::Absent, _) | (_, Kind::Absent) | (Kind::Curio(_), _) | (_, Kind::Curio(_)) => false,
            _ => true,
        }
    }
    /// `AbstractContainerScreen.mouseReleased` on a Shift double click: every slot of the clicked
    /// slot's inventory holding `moved`'s item is quick-moved.
    pub fn quick_move_all(&mut self, slot: usize, moved: &S) {
        if slot >= self.slot_count() {
            return;
        }
        for j in 0..self.slot_count() {
            if self.may_pickup(j) && self.has(j) && self.same_inventory(j, slot) && self.quick_replace(j, moved) {
                self.click(j as i32, 0, Click::QuickMove);
                if self.move_to_page.is_some() {
                    return;
                }
            }
        }
    }
    /// `AbstractContainerMenu.clicked` / `doClick` (no bundle behaviour, see README).
    pub fn click(&mut self, slot: i32, button: i32, click: Click) {
        let n = self.slot_count() as i32;
        if slot != OUTSIDE && (slot < 0 || slot >= n) && click != Click::QuickCraft {
            return;
        }
        if click == Click::QuickCraft {
            let before = self.quick.status;
            self.quick.status = button & 3;
            if (before != 1 || self.quick.status != 2) && before != self.quick.status {
                *self.quick = QuickCraft::default();
            } else if self.player.carried.is_none() {
                *self.quick = QuickCraft::default();
            } else if self.quick.status == 0 {
                self.quick.kind = (button >> 2) & 3;
                let valid = self.quick.kind == 0 || self.quick.kind == 1 || (self.quick.kind == 2 && self.player.creative);
                if valid {
                    self.quick.status = 1;
                    self.quick.slots.clear();
                } else {
                    *self.quick = QuickCraft::default();
                }
            } else if self.quick.status == 1 {
                if slot < 0 || slot >= n {
                    return;
                }
                let i = slot as usize;
                let carried = self.player.carried.clone().expect("checked");
                if self.quick_replace(i, &carried)
                    && self.may_place(i, &carried)
                    && (self.quick.kind == 2 || carried.count() as usize > self.quick.slots.len())
                    && !self.quick.slots.contains(&i)
                {
                    self.quick.slots.push(i);
                }
            } else if self.quick.status == 2 {
                if !self.quick.slots.is_empty() {
                    if self.quick.slots.len() == 1 {
                        let only = self.quick.slots[0];
                        let kind = self.quick.kind;
                        *self.quick = QuickCraft::default();
                        self.click(only as i32, kind, Click::Pickup);
                        return;
                    }
                    let Some(template) = self.player.carried.clone() else {
                        *self.quick = QuickCraft::default();
                        return;
                    };
                    let mut left = i64::from(template.count());
                    let slots = self.quick.slots.clone();
                    let kind = self.quick.kind;
                    for i in slots.iter().copied() {
                        let carried = self.player.carried.clone().expect("present");
                        if self.quick_replace(i, &carried)
                            && self.may_place(i, &carried)
                            && (kind == 2 || carried.count() as usize >= slots.len())
                        {
                            let j = i64::from(self.get(i).map_or(0, Stack::count));
                            let k = i64::from(self.items.max_stack(&template).min(self.slot_max_for(i, &template)));
                            let place = i64::from(Self::quick_craft_count(slots.len(), kind, &template, self.items.max_stack(&template)));
                            let l = (place + j).min(k);
                            left -= l - j;
                            let mut put = template.clone();
                            put.set_count(l.max(0) as u32);
                            self.set(i, Some(put));
                        }
                    }
                    let mut rest = template;
                    rest.set_count(left.max(0) as u32);
                    *self.player.carried = (left > 0).then_some(rest);
                }
                *self.quick = QuickCraft::default();
            } else {
                *self.quick = QuickCraft::default();
            }
            return;
        }
        if self.quick.status != 0 {
            *self.quick = QuickCraft::default();
            return;
        }
        match click {
            Click::Pickup | Click::QuickMove if button == 0 || button == 1 => {
                let primary = button == 0;
                if slot == OUTSIDE {
                    if let Some(mut c) = self.player.carried.take() {
                        if primary {
                            self.player.thrown.push(c);
                        } else {
                            let mut one = c.clone();
                            one.set_count(1);
                            self.player.thrown.push(one);
                            c.set_count(c.count() - 1);
                            *self.player.carried = (c.count() > 0).then_some(c);
                        }
                    }
                    return;
                }
                let i = slot as usize;
                if click == Click::QuickMove {
                    if !self.may_pickup(i) {
                        return;
                    }
                    // Repeat while the same item keeps moving (crafting results).
                    let mut guard = 0;
                    while let Some(moved) = self.quick_move_stack(i) {
                        guard += 1;
                        if guard > 64 || self.move_to_page.is_some() {
                            break;
                        }
                        match self.get(i) {
                            Some(now) if self.items.same_item(now, &moved) => {}
                            _ => break,
                        }
                    }
                    return;
                }
                let in_slot = self.get(i).cloned();
                let carried = self.player.carried.clone();
                match (in_slot, carried) {
                    (None, None) => {}
                    (None, Some(c)) => {
                        let count = if primary { c.count() } else { 1 };
                        *self.player.carried = self.safe_insert(i, c, count);
                    }
                    (Some(s), None) => {
                        if self.may_pickup(i) {
                            let count = if primary { s.count() } else { s.count().div_ceil(2) };
                            if let Some(t) = self.try_remove(i, count, u32::MAX) {
                                *self.player.carried = Some(t);
                            }
                        }
                    }
                    (Some(s), Some(mut c)) => {
                        if !self.may_pickup(i) {
                            return;
                        }
                        if self.may_place(i, &c) {
                            if self.items.same_components(&s, &c) {
                                let count = if primary { c.count() } else { 1 };
                                *self.player.carried = self.safe_insert(i, c, count);
                            } else if c.count() <= self.slot_max_for(i, &c) {
                                self.set(i, Some(c));
                                *self.player.carried = Some(s);
                            }
                        } else if self.items.same_components(&s, &c) {
                            let room = self.items.max_stack(&c).saturating_sub(c.count());
                            if let Some(t) = self.try_remove(i, s.count(), room) {
                                c.set_count(c.count() + t.count());
                                *self.player.carried = Some(c);
                            }
                        }
                    }
                }
            }
            Click::Swap => {
                if slot < 0 {
                    return;
                }
                let i = slot as usize;
                // Inventory.getItem(button): 0..8 the hotbar, 40 the off hand.
                let from_hand = match button {
                    0..=8 => self.player.inventory.get(button as usize).cloned().flatten(),
                    40 => self.player.offhand.clone(),
                    _ => return,
                };
                let set_hand = |p: &mut Player<'p, S>, s: Option<S>| match button {
                    40 => *p.offhand = s,
                    b => {
                        if let Some(c) = p.inventory.get_mut(b as usize) {
                            *c = s;
                        }
                    }
                };
                let in_slot = self.get(i).cloned();
                match (from_hand, in_slot) {
                    (None, None) => {}
                    (None, Some(s)) => {
                        if self.may_pickup(i) {
                            set_hand(self.player, Some(s));
                            self.set(i, None);
                        }
                    }
                    (Some(mut h), None) => {
                        if self.may_place(i, &h) {
                            let max = self.slot_max_for(i, &h);
                            if h.count() > max {
                                let mut put = h.clone();
                                put.set_count(max);
                                self.set(i, Some(put));
                                h.set_count(h.count() - max);
                                set_hand(self.player, Some(h));
                            } else {
                                set_hand(self.player, None);
                                self.set(i, Some(h));
                            }
                        }
                    }
                    (Some(mut h), Some(s)) => {
                        if self.may_pickup(i) && self.may_place(i, &h) {
                            let max = self.slot_max_for(i, &h);
                            if h.count() > max {
                                let mut put = h.clone();
                                put.set_count(max);
                                self.set(i, Some(put));
                                h.set_count(h.count() - max);
                                set_hand(self.player, Some(h));
                                // Inventory.add or drop.
                                let rest = add_to_inventory(self.items, self.player.inventory, s);
                                self.player.thrown.extend(rest);
                            } else {
                                set_hand(self.player, Some(s));
                                self.set(i, Some(h));
                            }
                        }
                    }
                }
            }
            Click::Clone => {
                if self.player.creative && self.player.carried.is_none() && slot >= 0 {
                    if let Some(s) = self.get(slot as usize).cloned() {
                        let mut c = s.clone();
                        c.set_count(self.items.max_stack(&s));
                        *self.player.carried = Some(c);
                    }
                }
            }
            Click::Throw => {
                if self.player.carried.is_none() && slot >= 0 {
                    let i = slot as usize;
                    let count = if button == 0 { 1 } else { self.get(i).map_or(0, Stack::count) };
                    if let Some(t) = self.try_remove(i, count, u32::MAX) {
                        self.player.thrown.push(t);
                    }
                }
            }
            Click::PickupAll => {
                if slot < 0 {
                    return;
                }
                let i = slot as usize;
                let Some(mut c) = self.player.carried.clone() else { return };
                if self.has(i) && self.may_pickup(i) {
                    return;
                }
                let count = self.slot_count();
                let max = self.items.max_stack(&c);
                for pass in 0..2 {
                    let order: Vec<usize> = if button == 0 { (0..count).collect() } else { (0..count).rev().collect() };
                    for j in order {
                        if c.count() >= max {
                            break;
                        }
                        let Some(s) = self.get(j).cloned() else { continue };
                        if !self.quick_replace(j, &c) || !self.may_pickup(j) {
                            continue;
                        }
                        // canTakeItemForPickAll: true (no result slot in this port).
                        if pass == 0 && s.count() == self.items.max_stack(&s) {
                            continue;
                        }
                        if let Some(t) = self.try_remove(j, s.count(), max - c.count()) {
                            c.set_count(c.count() + t.count());
                        }
                    }
                }
                *self.player.carried = Some(c);
            }
            _ => {}
        }
    }
}

/// `Inventory.add`: into same stacks of the 36 slots, then the first empty; what does not fit.
pub fn add_to_inventory<S: Stack>(items: &impl Items<S>, inventory: &mut [Option<S>], mut stack: S) -> Option<S> {
    if items.stackable(&stack) {
        // Inventory.getSlotWithRemainingSpace: the selected slot, the off hand, then 0..36; here 0..36.
        for cell in inventory.iter_mut() {
            if stack.count() == 0 {
                break;
            }
            if let Some(have) = cell
                && items.same_components(have, &stack)
            {
                let room = items.max_stack(have).min(64).saturating_sub(have.count());
                let n = room.min(stack.count());
                have.set_count(have.count() + n);
                stack.set_count(stack.count() - n);
            }
        }
    }
    if stack.count() > 0
        && let Some(cell) = inventory.iter_mut().find(|c| c.is_none())
    {
        *cell = Some(stack);
        return None;
    }
    (stack.count() > 0).then_some(stack)
}

/// `CuriosEventHandler.curioRightClick`: where a used curio goes. `Ok(Some(i))`: the empty slot
/// (handler, index) it fills; `Err((h, i))`: the first slot whose stack it swaps with.
pub fn equip_from_use<S: Stack>(
    inv: &Inventory<S>,
    items: &impl Items<S>,
    entity_slots: &[SlotType],
    all_slots: &[SlotType],
    stack: &S,
    can_equip_from_use: impl Fn(&SlotCtx) -> bool,
) -> Option<Result<(usize, usize), (usize, usize)>> {
    let mut first: Option<(usize, usize)> = None;
    for (hi, h) in inv.handlers.iter().enumerate() {
        for i in 0..h.slots() {
            let ctx = inv.ctx(hi, i, false);
            if !(may_equip(items, entity_slots, all_slots, &ctx, stack) && can_equip_from_use(&ctx)) {
                continue;
            }
            match &h.stacks[i] {
                None => return Some(Ok((hi, i))),
                Some(present) if first.is_none() => {
                    if items.unequip_event(&ctx, present) == Some(false) {
                        continue;
                    }
                    // extractItem(i, maxStackSize, true).getCount() == stack.getCount()
                    let takes = if may_unequip(items, &ctx, present, false) {
                        present.count().min(items.max_stack(stack))
                    } else {
                        0
                    };
                    if takes == stack.count() {
                        first = Some((hi, i));
                    }
                }
                Some(_) => {}
            }
        }
    }
    first.map(Err)
}

// =================================================================================================
// What the client's screen needs (a section of the foundry menu), as text lines
// =================================================================================================

/// The screen of the open Curios menu, as the server sends it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub layout: Layout,
    /// Icon of each slot type on the page (`SPacketSetIcons`).
    pub icons: BTreeMap<String, String>,
}
impl View {
    pub fn encode(&self) -> Vec<u8> {
        let l = &self.layout;
        let mut s = format!(
            "v1 {} {} {} {} {} {}\n",
            l.page,
            l.total_pages,
            l.columns,
            l.panel_width,
            u8::from(l.has_cosmetics),
            u8::from(l.viewing_cosmetics)
        );
        s.push_str(&format!("g {}\n", l.grid.iter().map(i32::to_string).collect::<Vec<_>>().join(" ")));
        for p in &l.slots {
            s.push_str(&format!(
                "s {} {} {} {} {} {} {} {}\n",
                p.handler,
                p.id,
                p.index,
                u8::from(p.cosmetic),
                p.x,
                p.y,
                u8::from(p.render_toggle),
                u8::from(p.render)
            ));
        }
        for (id, icon) in &self.icons {
            s.push_str(&format!("i {id} {icon}\n"));
        }
        s.into_bytes()
    }
    pub fn decode(b: &[u8]) -> Option<View> {
        let text = std::str::from_utf8(b).ok()?;
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next()?.split(' ').collect();
        if head.first() != Some(&"v1") || head.len() != 7 {
            return None;
        }
        let n = |s: &str| s.parse::<i32>().ok();
        let mut v = View {
            layout: Layout {
                page: n(head[1])?,
                total_pages: n(head[2])?,
                columns: n(head[3])?,
                panel_width: n(head[4])?,
                has_cosmetics: head[5] == "1",
                viewing_cosmetics: head[6] == "1",
                ..Layout::default()
            },
            icons: BTreeMap::new(),
        };
        for line in lines {
            let f: Vec<&str> = line.split(' ').collect();
            match f.first().copied() {
                Some("g") => v.layout.grid = f[1..].iter().filter(|x| !x.is_empty()).map(|x| n(x)).collect::<Option<Vec<_>>>()?,
                Some("s") if f.len() == 9 => v.layout.slots.push(PageSlot {
                    handler: f[1].parse().ok()?,
                    id: f[2].to_owned(),
                    index: f[3].parse().ok()?,
                    cosmetic: f[4] == "1",
                    x: n(f[5])?,
                    y: n(f[6])?,
                    render_toggle: f[7] == "1",
                    render: f[8] == "1",
                }),
                Some("i") if f.len() == 3 => {
                    v.icons.insert(f[1].to_owned(), f[2].to_owned());
                }
                _ => return None,
            }
        }
        Some(v)
    }
}

/// Buttons of the screen (`foundry::MenuInput::Button` ids).
pub mod button {
    /// `CPacketPage(next)`: arg 1 next, 0 previous.
    pub const PAGE: u32 = 1;
    /// `CPacketToggleCosmetics`.
    pub const COSMETICS: u32 = 2;
    /// `CPacketToggleRender(id, index)`: arg = the page slot.
    pub const RENDER: u32 = 3;
    /// `CPacketOpenVanilla`: back to the inventory.
    pub const VANILLA: u32 = 4;
}

/// The V2 screen's geometry (`CuriosScreenV2`), relative to the screen's left/top.
pub mod screen {
    pub const WIDTH: f32 = 176.0;
    pub const HEIGHT: f32 = 166.0;
    pub const TEXTURE_SIZE: f32 = 256.0;
    /// `CosmeticButton`: (left - 27, top - 18), 20 x 17, u 123 / 143 (viewing), v 0 / 17 (hovered).
    pub const COSMETIC: (f32, f32, f32, f32) = (-27.0, -18.0, 20.0, 17.0);
    /// `PageButton`: next at left - 17, previous at left - 28, top + 2, 11 x 12.
    pub const PAGE_NEXT: (f32, f32, f32, f32) = (-17.0, 2.0, 11.0, 12.0);
    pub const PAGE_PREV: (f32, f32, f32, f32) = (-28.0, 2.0, 11.0, 12.0);
    /// `RenderButton`: slot.x + 11, slot.y - 3, 8 x 8, u 75 (shown) / 83 (hidden), v 0.
    pub const RENDER_DX: f32 = 11.0;
    pub const RENDER_DY: f32 = -3.0;
    /// The Curios button: 14 x 14 (inventory.png u 50, v 0, +14 hovered); creative 10 x 10 (u 64).
    pub const BUTTON: f32 = 14.0;
    pub const BUTTON_CREATIVE: f32 = 10.0;
}

/// `CuriosScreen.getButtonOffset(isCreative)` + `GuiEventHandler`'s y offset (83 / 68): the Curios
/// button's corner relative to the screen's left/top.
pub fn button_offset(creative: bool) -> (i32, i32) {
    let corner = jar_data::BUTTON_CORNERS
        .iter()
        .find(|c| c.0 == config::CLIENT_BUTTON_CORNER)
        .copied()
        .unwrap_or(jar_data::BUTTON_CORNERS[0]);
    if creative {
        (corner.3 + config::CLIENT_CREATIVE_BUTTON_XOFFSET, corner.4 + config::CLIENT_CREATIVE_BUTTON_YOFFSET + 68)
    } else {
        (corner.1 + config::CLIENT_BUTTON_XOFFSET, corner.2 + config::CLIENT_BUTTON_YOFFSET + 83)
    }
}

/// What a widget of the V2 screen is under (mx, my), relative to the screen's left/top.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Widget {
    /// The Curios button (back to the inventory).
    Vanilla,
    Cosmetics,
    PageNext,
    PagePrev,
    /// The render toggle of the page slot.
    Render(usize),
}
pub fn widget_at(view: &View, mx: f32, my: f32) -> Option<Widget> {
    let inside = |(x, y, w, h): (f32, f32, f32, f32)| mx >= x && mx < x + w && my >= y && my < y + h;
    let (bx, by) = button_offset(false);
    if config::CLIENT_ENABLE_BUTTON && inside((bx as f32, by as f32, screen::BUTTON, screen::BUTTON)) {
        return Some(Widget::Vanilla);
    }
    // The screen's children in the order `init` / `updateRenderButtons` add them.
    let l = &view.layout;
    if l.has_cosmetics && inside(screen::COSMETIC) {
        return Some(Widget::Cosmetics);
    }
    if l.total_pages > 1 {
        if inside(screen::PAGE_NEXT) {
            return Some(Widget::PageNext);
        }
        if inside(screen::PAGE_PREV) {
            return Some(Widget::PagePrev);
        }
    }
    for (j, p) in l.slots.iter().enumerate() {
        if !p.cosmetic && p.render_toggle && inside((p.x as f32 + screen::RENDER_DX, p.y as f32 + screen::RENDER_DY, 8.0, 8.0)) {
            return Some(Widget::Render(j));
        }
    }
    None
}

/// `CuriosScreenV2.mouseScrolled`: the wheel over the panel turns the page.
pub fn wheel_turns_page(view: &View, mx: f32, my: f32) -> bool {
    view.layout.total_pages > 1 && mx < 0.0 && mx > -(view.layout.panel_width as f32) && my > 0.0 && my < screen::HEIGHT
}

/// `CurioSlot.getSlotName`: `curios.identifier.<id>` or the id capitalised, "Cosmetic " first.
pub fn slot_name(id: &str, cosmetic: bool, translate: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    if cosmetic {
        out.push_str(&translate("curios.cosmetic").unwrap_or_else(|| "Cosmetic".into()));
        out.push(' ');
    }
    match translate(&format!("curios.identifier.{id}")) {
        Some(t) => out.push_str(&t),
        None => {
            let mut c = id.chars();
            if let Some(f) = c.next() {
                out.extend(f.to_uppercase());
                out.push_str(&c.as_str().to_lowercase());
            }
        }
    }
    out
}

/// `ClientEventHandler.onTooltip`'s slot list: the player's slot types the stack fits, without the
/// `curios:all` ones, and only `curio` when it is among them.
pub fn tooltip_slots<S: Stack>(items: &impl Items<S>, player_slots: &[SlotType], s: &S) -> Vec<String> {
    let mut ids: Vec<String> = stack_slots(items, player_slots, s)
        .into_iter()
        .filter(|t| !t.validators.contains("curios:all"))
        .map(|t| t.id.clone())
        .collect();
    if ids.iter().any(|i| i == "curio") {
        ids = vec!["curio".into()];
    }
    ids
}

// =================================================================================================
// /curios (CuriosCommand)
// =================================================================================================

/// A parsed `/curios` command (`target` = a player name, `@s` or `@p`).
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    List,
    Replace { slot: String, index: usize, target: String, item: String, count: u32 },
    Set { slot: String, target: String, amount: i32 },
    Add { slot: String, target: String, amount: i32 },
    Remove { slot: String, target: String, amount: i32 },
    Clear { target: String, slot: Option<String> },
    Drop { target: String, slot: Option<String> },
    Reset { target: String },
}

/// `CuriosCommand.register`'s grammar (without slot/index fields of `entity-*`/`block` sources).
pub fn parse_command(cmd: &str) -> Option<Result<Command, String>> {
    let words: Vec<&str> = cmd.split_whitespace().collect();
    if words.first() != Some(&"curios") {
        return None;
    }
    let num = |s: Option<&&str>, default: i32| -> Result<i32, String> {
        match s {
            None => Ok(default),
            Some(x) => x.parse::<i32>().map_err(|_| format!("Invalid integer '{x}'")),
        }
    };
    let r = match words.get(1).copied() {
        Some("list") => Ok(Command::List),
        Some("replace") if words.len() >= 7 && words[5] == "with" => {
            let index = words[3].parse::<usize>().map_err(|_| format!("Invalid integer '{}'", words[3]));
            match (index, num(words.get(7), 1)) {
                (Ok(index), Ok(count)) => Ok(Command::Replace {
                    slot: words[2].into(),
                    index,
                    target: words[4].into(),
                    item: full_id(words[6]),
                    count: count.clamp(1, 64) as u32,
                }),
                (Err(e), _) | (_, Err(e)) => Err(e),
            }
        }
        Some(w @ ("set" | "add" | "remove")) if words.len() >= 4 => match num(words.get(4), 1) {
            Ok(amount) => {
                let (slot, target) = (words[2].to_owned(), words[3].to_owned());
                Ok(match w {
                    "set" => Command::Set { slot, target, amount },
                    "add" => Command::Add { slot, target, amount },
                    _ => Command::Remove { slot, target, amount },
                })
            }
            Err(e) => Err(e),
        },
        Some(w @ ("clear" | "drop")) if words.len() >= 3 => {
            let (target, slot) = (words[2].to_owned(), words.get(3).map(|s| s.to_string()));
            Ok(if w == "clear" { Command::Clear { target, slot } } else { Command::Drop { target, slot } })
        }
        Some("reset") if words.len() >= 3 => Ok(Command::Reset { target: words[2].into() }),
        _ => Err("Unknown or incomplete command: /curios list | replace <slot> <index> <player> with <item> [count] | set|add|remove <slot> <player> [amount] | clear|drop <player> [slot] | reset <player>".into()),
    };
    Some(r)
}

/// `/curios set`: the legacy shift that makes the slot type `amount` slots.
pub fn set_slots<S: Stack>(h: &mut Handler<S>, amount: i32) {
    let shift = amount.max(0) - h.slots() as i32;
    if shift != 0 {
        h.legacy_change(shift);
    }
}

// Advanced Netherite's items in the content root (horizon_client::mod_content). Every
// item of the mod copies the components of a vanilla netherite item; this sets the
// numbers of the original (ModToolTiers, ModArmorTiers, ModItems): durability,
// enchantability, repair item, defence, toughness, attack damage and speed, the
// breaking speed multipliers and the worn armor's asset.
use super::J;
#[allow(dead_code)]
mod engine {
    include!("advancednetherite_engine.rs");
}

impl engine::Node for J {
    fn object() -> Self {
        J::Obj(Vec::new())
    }
    fn int(v: i64) -> Self {
        J::Num(v.into())
    }
    fn float(v: f64) -> Self {
        J::f64(v)
    }
    fn string(v: &str) -> Self {
        J::Str(v.to_owned())
    }
    fn member(&mut self, key: &str) -> Option<&mut Self> {
        self.get_mut(key)
    }
    fn put(&mut self, key: &str, value: Self) {
        self.set(key, value);
    }
    fn elements(&mut self) -> Option<&mut Vec<Self>> {
        match self {
            J::Arr(a) => Some(a),
            _ => None,
        }
    }
    fn text(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    fn flag(&self) -> Option<bool> {
        match self {
            J::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

/// `items.json` after the mods' items were copied in: their components get the
/// original's numbers.
pub(super) fn patch_items(map: &mut J, components: bool) {
    if !components {
        return;
    }
    let settings = engine::Settings::default();
    for spec in engine::items() {
        if let Some(entry) = map.get_mut(&spec.id()) {
            engine::apply(entry, &engine::ops(&spec, &settings));
        }
    }
}

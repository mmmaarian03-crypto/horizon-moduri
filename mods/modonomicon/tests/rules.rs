use horizon_modonomicon::{on_use, Action, Pos, State, World};
use std::collections::BTreeMap;

#[derive(Default)]
struct Map(BTreeMap<Pos, String>);
impl World for Map {
    fn block(&self, p: Pos) -> Option<String> {
        if p[1] < -64 || p[1] > 319 {
            return None;
        }
        Some(self.0.get(&p).cloned().unwrap_or_else(|| "minecraft:air".into()))
    }
    fn in_tag(&self, _tag: &str, _block: &str) -> bool {
        false
    }
}

#[test]
fn vanilla_blocks_pass() {
    let w = Map::default();
    assert_eq!(on_use(&w, [0, 64, 0]), Action::Pass);
}

#[test]
fn own_blocks_are_handled() {
    let mut w = Map::default();
    w.0.insert([1, 2, 3], "modonomicon:example".into());
    assert!(matches!(on_use(&w, [1, 2, 3]), Action::Notice(_)));
}

#[test]
fn state_round_trips() {
    let mut s = State::default();
    s.uses.insert([1, -2, 3], 4);
    assert_eq!(State::load(Some(s.save())), s);
}

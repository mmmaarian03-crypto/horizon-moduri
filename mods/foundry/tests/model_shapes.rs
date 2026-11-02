// Shapes from models: block states of content mods take their model's boxes as outline (and
// collision) when the template's shape is not their model. Real models of the carried mods
// (Create's seat, Macaw's Lights' ceiling fan and wall lantern, a Chipped cube) are read from
// content/mods/<id>/carry with a small JSON reader; the game does the same with its resolved models.
use horizon_foundry::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

// ---- a small JSON reader (the foundry crate has no dependencies) ----
#[allow(dead_code)]
#[derive(Clone, Debug)]
enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(BTreeMap<String, J>),
}
impl J {
    fn get(&self, k: &str) -> Option<&J> {
        match self {
            J::Obj(m) => m.get(k),
            _ => None,
        }
    }
    fn str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    fn arr(&self) -> &[J] {
        match self {
            J::Arr(a) => a,
            _ => &[],
        }
    }
    fn num(&self) -> Option<f64> {
        match self {
            J::Num(n) => Some(*n),
            _ => None,
        }
    }
}
fn parse(s: &str) -> J {
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    value(&b, &mut i)
}
fn ws(b: &[char], i: &mut usize) {
    while *i < b.len() && b[*i].is_whitespace() {
        *i += 1;
    }
}
fn value(b: &[char], i: &mut usize) -> J {
    ws(b, i);
    match b[*i] {
        '{' => {
            *i += 1;
            let mut m = BTreeMap::new();
            loop {
                ws(b, i);
                if b[*i] == '}' {
                    *i += 1;
                    break;
                }
                let J::Str(k) = value(b, i) else { panic!("key") };
                ws(b, i);
                *i += 1; // ':'
                let v = value(b, i);
                m.insert(k, v);
                ws(b, i);
                if b[*i] == ',' {
                    *i += 1;
                }
            }
            J::Obj(m)
        }
        '[' => {
            *i += 1;
            let mut a = Vec::new();
            loop {
                ws(b, i);
                if b[*i] == ']' {
                    *i += 1;
                    break;
                }
                a.push(value(b, i));
                ws(b, i);
                if b[*i] == ',' {
                    *i += 1;
                }
            }
            J::Arr(a)
        }
        '"' => {
            *i += 1;
            let mut s = String::new();
            while b[*i] != '"' {
                if b[*i] == '\\' {
                    *i += 1;
                }
                s.push(b[*i]);
                *i += 1;
            }
            *i += 1;
            J::Str(s)
        }
        't' => {
            *i += 4;
            J::Bool(true)
        }
        'f' => {
            *i += 5;
            J::Bool(false)
        }
        'n' => {
            *i += 4;
            J::Null
        }
        _ => {
            let start = *i;
            while *i < b.len() && (b[*i].is_ascii_digit() || "+-.eE".contains(b[*i])) {
                *i += 1;
            }
            J::Num(b[start..*i].iter().collect::<String>().parse().unwrap())
        }
    }
}

fn carry(ns: &str) -> PathBuf {
    PathBuf::from(r"../../content/mods").join(ns).join("carry").join("assets").join(ns)
}
/// The elements of a model, following `parent` (vanilla `block/cube*` parents are the full cube,
/// `block/block` has none); `None` when a file is missing (the test then skips).
fn elements(name: &str) -> Option<Vec<[f64; 6]>> {
    let (ns, path) = name.split_once(':').unwrap_or(("minecraft", name));
    if ns == "minecraft" {
        return Some(if path.starts_with("block/cube") { vec![[0.0, 0.0, 0.0, 16.0, 16.0, 16.0]] } else { Vec::new() });
    }
    let text = std::fs::read_to_string(carry(ns).join("models").join(format!("{path}.json"))).ok()?;
    let m = parse(&text);
    if let Some(e) = m.get("elements") {
        return Some(
            e.arr()
                .iter()
                .map(|e| {
                    let f: Vec<f64> = e.get("from").unwrap().arr().iter().map(|v| v.num().unwrap()).collect();
                    let t: Vec<f64> = e.get("to").unwrap().arr().iter().map(|v| v.num().unwrap()).collect();
                    [f[0], f[1], f[2], t[0], t[1], t[2]]
                })
                .collect(),
        );
    }
    match m.get("parent").and_then(J::str) {
        Some(p) => elements(p),
        None => Some(Vec::new()),
    }
}
/// The (model, x/y turns) of a blockstate variant.
fn variant(ns: &str, block: &str, key: &str) -> Option<(String, [u8; 3])> {
    let text = std::fs::read_to_string(carry(ns).join("blockstates").join(format!("{block}.json"))).ok()?;
    let v = parse(&text).get("variants")?.get(key)?.clone();
    let turn = |k: &str| (v.get(k).and_then(J::num).unwrap_or(0.0) as i64 / 90) as u8;
    Some((v.get("model")?.str()?.to_owned(), [turn("x"), turn("y"), 0]))
}
fn state_boxes(ns: &str, block: &str, key: &str) -> Option<Vec<[f64; 6]>> {
    let (model, turns) = variant(ns, block, key)?;
    Some(model_boxes(&[(elements(&model)?, turns)]))
}
const CUBE: [[f64; 6]; 1] = [[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]];

#[test]
fn turns_follow_vanilla_block_model_rotation() {
    // Observer: a north-facing front turned up by x = 270, east by y = 90.
    let north = [0.0, 0.0, 0.0, 16.0, 16.0, 4.0];
    assert_eq!(turn_box(north, [3, 0, 0]), [0.0, 0.75, 0.0, 1.0, 1.0, 1.0]);
    assert_eq!(turn_box(north, [1, 0, 0]), [0.0, 0.0, 0.0, 1.0, 0.25, 1.0]);
    assert_eq!(turn_box(north, [0, 1, 0]), [0.75, 0.0, 0.0, 1.0, 1.0, 1.0]);
    assert_eq!(turn_box(north, [0, 2, 0]), [0.0, 0.0, 0.75, 1.0, 1.0, 1.0]);
    assert_eq!(turn_box(north, [0, 3, 0]), [0.0, 0.0, 0.0, 0.25, 1.0, 1.0]);
    // X then Y: a bottom slab turned up-side-down then east stays a top slab.
    assert_eq!(turn_box([0.0, 0.0, 0.0, 16.0, 8.0, 16.0], [2, 1, 0]), [0.0, 0.5, 0.0, 1.0, 1.0, 1.0]);
    // Flat planes get 1/16 of thickness, boxes outside the block are cut to it.
    let flat = model_boxes(&[(vec![[0.0, 0.5, 0.0, 16.0, 0.5, 16.0], [-8.0, 0.0, 4.0, 24.0, 2.0, 12.0]], [0, 0, 0])]);
    assert_eq!(flat[0], [0.0, 0.5 / 16.0, 0.0, 1.0, 1.5 / 16.0, 1.0]);
    assert_eq!(flat[1], [0.0, 0.0, 0.25, 1.0, 0.125, 0.75]);
}

#[test]
#[ignore = "needs the generated content of other mods in content/mods"]
fn create_seat_on_a_glass_template_is_half_a_block() {
    let boxes = state_boxes("create", "black_seat", "").expect("Create carry");
    assert_eq!(boxes, vec![[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]], "SeatBlock's shape (AllShapes.SEAT)");
    // Glass: outline = collision = the cube, so both follow the model.
    let rule = model_shape_rule("create:black_seat", &boxes, &CUBE, Some(&CUBE), &["TransparentBlock", "HalfTransparentBlock"]).unwrap();
    assert!(rule.collide);
    assert_eq!(rule.outline, boxes);
}

#[test]
#[ignore = "needs the generated content of other mods in content/mods"]
fn macaws_ceiling_fan_light_follows_its_model_inside_the_block() {
    let boxes = state_boxes("mcwlights", "acacia_ceiling_fan_light", "").expect("Macaw carry");
    assert!(boxes.len() > 10, "{}", boxes.len());
    assert!(boxes.contains(&[6.0 / 16.0, 15.0 / 16.0, 6.0 / 16.0, 10.0 / 16.0, 1.0, 10.0 / 16.0]), "the ceiling mount");
    assert!(boxes.iter().all(|b| (0..3).all(|k| b[k] >= 0.0 && b[k + 3] <= 1.0 && b[k + 3] - b[k] >= MIN_THICKNESS - 1e-12)));
    let rule = model_shape_rule("mcwlights:acacia_ceiling_fan_light", &boxes, &CUBE, Some(&CUBE), &[]).unwrap();
    assert!(rule.collide);
}

#[test]
#[ignore = "needs the generated content of other mods in content/mods"]
fn macaws_wall_lantern_turns_with_its_facing() {
    let north = state_boxes("mcwlights", "bell_wall_lantern", "facing=north").expect("Macaw carry");
    let east = state_boxes("mcwlights", "bell_wall_lantern", "facing=east").expect("Macaw carry");
    assert_eq!(north.len(), east.len());
    let (model, _) = variant("mcwlights", "bell_wall_lantern", "facing=north").unwrap();
    let raw = elements(&model).unwrap();
    let turned = model_boxes(&[(raw, [0, 1, 0])]);
    assert_eq!(east, turned);
    assert_ne!(north, east);
    // The end rod template's outline is a rod: the lantern's model differs, so it takes the model.
    let rod = [[6.0 / 16.0, 6.0 / 16.0, 0.0, 10.0 / 16.0, 10.0 / 16.0, 1.0]];
    assert!(model_shape_rule("mcwlights:bell_wall_lantern", &east, &rod, Some(&rod), &["RodBlock"]).is_some());
}

#[test]
#[ignore = "needs the generated content of other mods in content/mods"]
fn templates_that_already_match_or_carry_behaviour_keep_their_shapes() {
    // A Chipped cube on a stone template: nothing to change.
    let boxes = state_boxes("chipped", "ad_astra_ochre_froglight", "").expect("Chipped carry");
    assert_eq!(boxes, CUBE.to_vec());
    assert!(model_shape_rule("chipped:ad_astra_ochre_froglight", &boxes, &CUBE, Some(&CUBE), &[]).is_none());
    let half = [[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]];
    // Vanilla blocks and builtin/entity models (no elements) keep the template.
    assert!(model_shape_rule("minecraft:stone", &half, &CUBE, Some(&CUBE), &[]).is_none());
    assert!(model_shape_rule("mymod:chest", &[], &CUBE, Some(&CUBE), &[]).is_none());
    // Doors, slabs, fences...: their shape is what they do.
    assert!(model_shape_rule("mcwdoors:door", &half, &CUBE, Some(&CUBE), &["DoorBlock"]).is_none());
    assert!(model_shape_rule("mymod:slab", &half, &half, Some(&half), &["SlabBlock"]).is_none());
    // A model that is the template's outline split in two boxes: the same shape.
    let split = [[0.0, 0.0, 0.0, 1.0, 0.5, 1.0], [0.0, 0.5, 0.0, 1.0, 1.0, 1.0]];
    assert!(same_shape(&split, &CUBE));
    assert!(model_shape_rule("mymod:pillar", &split, &CUBE, Some(&CUBE), &[]).is_none());
    // Non-colliding templates (plants: empty collision) and special collisions (a fence's 1.5,
    // soul sand's 14/16, dynamic shapes) keep their collision; the outline still follows the model.
    let plant = model_shape_rule("mymod:flower", &half, &CUBE, Some(&[]), &[]).unwrap();
    assert!(!plant.collide);
    let sand = [[0.0, 0.0, 0.0, 1.0, 14.0 / 16.0, 1.0]];
    assert!(!model_shape_rule("mymod:mud", &half, &CUBE, Some(&sand), &[]).unwrap().collide);
    assert!(!model_shape_rule("mymod:odd", &half, &CUBE, None, &[]).unwrap().collide);
}

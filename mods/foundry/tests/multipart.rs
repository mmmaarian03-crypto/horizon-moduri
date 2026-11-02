// Compound blocks (multipart): shape unions, the crosshair's part, adding and removing parts, the
// empty host, the side file round trip, no item lost. The numbers are AE2 15.4.11's
// (GlassCablePart 6..10, DenseCablePart 3..13 / 4.9..11.1, AbstractReportingPart, IOBusPart...).
mod common;
use common::*;
use horizon_foundry::*;

const HOST: &str = "test:cable_bus";

fn glass() -> PartSpec {
    PartSpec::center(
        "test:glass_cable",
        HOST,
        "test:glass_cable",
        PartShape::Cable { min: 6.0, max: 10.0, collide: None, attach_arms: true },
    )
}
fn dense() -> PartSpec {
    PartSpec::center(
        "test:dense_cable",
        HOST,
        "test:dense_cable",
        PartShape::Cable { min: 3.0, max: 13.0, collide: Some((4.9, 11.1)), attach_arms: false },
    )
    .group(2)
}
/// AbstractReportingPart.getBoxes, getCableConnectionLength 3 (AEBasePart).
fn terminal() -> PartSpec {
    PartSpec::face("test:terminal", HOST, "test:terminal", &[[2.0, 2.0, 14.0, 14.0, 14.0, 16.0], [4.0, 4.0, 13.0, 12.0, 12.0, 14.0]])
        .facing_opposite()
}
/// ImportBusPart.getBoxes, IOBusPart length 5.
fn import_bus() -> PartSpec {
    PartSpec::face(
        "test:import_bus",
        HOST,
        "test:import_bus",
        &[[6.0, 6.0, 11.0, 10.0, 10.0, 13.0], [5.0, 5.0, 13.0, 11.0, 11.0, 14.0], [4.0, 4.0, 14.0, 12.0, 12.0, 16.0]],
    )
    .cable_length(5.0)
}
/// CableAnchorPart: on cables and dense cables, length 0.
fn anchor() -> PartSpec {
    PartSpec::face("test:anchor", HOST, "test:anchor", &[[7.0, 7.0, 10.0, 9.0, 9.0, 16.0]]).cable_length(0.0).group(3)
}
fn specs(id: &str) -> Option<PartSpec> {
    [glass(), dense(), terminal(), import_bus(), anchor()].into_iter().find(|s| s.id == id)
}
fn close(a: [f64; 6], b: [f64; 6]) -> bool {
    (0..6).all(|k| (a[k] - b[k]).abs() < 1e-9)
}
fn has(boxes: &[(Slot, [f64; 6])], slot: Slot, b: [f64; 6]) -> bool {
    boxes.iter().any(|(s, x)| *s == slot && close(*x, b))
}

#[test]
fn face_boxes_turn_like_bus_collision_helper() {
    let b = [2.0, 2.0, 14.0, 14.0, 14.0, 16.0];
    let t = 2.0 / 16.0;
    assert!(close(face_box(b, Dir::South), [0.125, 0.125, 1.0 - t, 0.875, 0.875, 1.0]));
    assert!(close(face_box(b, Dir::North), [0.125, 0.125, 0.0, 0.875, 0.875, t]));
    assert!(close(face_box(b, Dir::East), [1.0 - t, 0.125, 0.125, 1.0, 0.875, 0.875]));
    assert!(close(face_box(b, Dir::West), [0.0, 0.125, 0.125, t, 0.875, 0.875]));
    assert!(close(face_box(b, Dir::Up), [0.125, 1.0 - t, 0.125, 0.875, 1.0, 0.875]));
    assert!(close(face_box(b, Dir::Down), [0.125, 0.0, 0.125, 0.875, t, 0.875]));
}

#[test]
fn the_host_shape_is_the_union_of_its_parts() {
    let mut h = Host::new(HOST);
    h.put(None, Part::new("test:glass_cable")).unwrap();
    h.put(Some(Dir::North), Part::new("test:terminal")).unwrap();
    // Linked east (a neighbouring cable) and up; north holds the terminal, so it is not linked.
    let mut links = [false; 6];
    links[Dir::East.idx()] = true;
    links[Dir::Up.idx()] = true;
    links[Dir::North.idx()] = true;
    let boxes = host_boxes(&h, &specs, links, false);
    let s = 1.0 / 16.0;
    // Core, east arm to the face, up arm, the arm into the terminal (from 3/16), the terminal's two boxes.
    assert!(has(&boxes, None, [6.0 * s, 6.0 * s, 6.0 * s, 10.0 * s, 10.0 * s, 10.0 * s]));
    assert!(has(&boxes, None, [10.0 * s, 6.0 * s, 6.0 * s, 1.0, 10.0 * s, 10.0 * s]));
    assert!(has(&boxes, None, [6.0 * s, 10.0 * s, 6.0 * s, 10.0 * s, 1.0, 10.0 * s]));
    assert!(has(&boxes, None, [6.0 * s, 6.0 * s, 3.0 * s, 10.0 * s, 10.0 * s, 6.0 * s]));
    assert!(has(&boxes, Some(Dir::North), [2.0 * s, 2.0 * s, 0.0, 14.0 * s, 14.0 * s, 2.0 * s]));
    assert!(has(&boxes, Some(Dir::North), [4.0 * s, 4.0 * s, 2.0 * s, 12.0 * s, 12.0 * s, 3.0 * s]));
    assert_eq!(boxes.len(), 6, "{boxes:?}");
    // Every box lies in the block.
    assert!(boxes.iter().all(|(_, b)| (0..3).all(|k| b[k] >= 0.0 && b[k + 3] <= 1.0 && b[k] < b[k + 3])));
}

#[test]
fn dense_cables_are_thinner_for_collisions_and_get_no_arm_into_parts() {
    let mut h = Host::new(HOST);
    h.put(None, Part::new("test:dense_cable")).unwrap();
    h.put(Some(Dir::West), Part::new("test:anchor")).unwrap();
    let mut links = [false; 6];
    links[Dir::South.idx()] = true;
    let outline = host_boxes(&h, &specs, links, false);
    let collision = host_boxes(&h, &specs, links, true);
    let s = 1.0 / 16.0;
    assert!(has(&outline, None, [3.0 * s, 3.0 * s, 3.0 * s, 13.0 * s, 13.0 * s, 13.0 * s]));
    assert!(has(&collision, None, [4.9 * s, 4.9 * s, 4.9 * s, 11.1 * s, 11.1 * s, 11.1 * s]));
    assert!(has(&collision, None, [4.9 * s, 4.9 * s, 11.1 * s, 11.1 * s, 11.1 * s, 1.0]));
    // Core + south arm + the anchor's rod: no arm towards the anchor.
    assert_eq!(outline.len(), 3);
    assert!(has(&outline, Some(Dir::West), [0.0, 7.0 * s, 7.0 * s, 6.0 * s, 9.0 * s, 9.0 * s]));
}

#[test]
fn the_crosshair_finds_the_part_it_hits() {
    let mut h = Host::new(HOST);
    h.put(None, Part::new("test:glass_cable")).unwrap();
    h.put(Some(Dir::North), Part::new("test:terminal")).unwrap();
    h.put(Some(Dir::Up), Part::new("test:import_bus")).unwrap();
    let boxes = host_boxes(&h, &specs, [false; 6], false);
    // A point on the terminal's screen, on the core, on the bus, in the air.
    assert_eq!(select(&boxes, [0.5, 0.5, 0.05]), Some(Some(Dir::North)));
    assert_eq!(select(&boxes, [0.5, 0.5, 0.5]), Some(None));
    assert_eq!(select(&boxes, [0.5, 0.95, 0.5]), Some(Some(Dir::Up)));
    assert_eq!(select(&boxes, [0.05, 0.5, 0.95]), None);
    // A ray from the north into the host stops on the terminal's face, with the north normal.
    let world: Vec<[f64; 6]> = boxes.iter().map(|(_, b)| [b[0] + 4.0, b[1] + 64.0, b[2] - 2.0, b[3] + 4.0, b[4] + 64.0, b[5] - 2.0]).collect();
    let (t, i, n) = ray_boxes([4.5, 64.5, -4.0], [0.0, 0.0, 1.0], 5.0, &world, [0, 0, -1]).unwrap();
    assert!((t - 2.0).abs() < 1e-9);
    assert_eq!(n, [0, 0, -1]);
    assert_eq!(boxes[i].0, Some(Dir::North));
    // From the south it passes the empty half and stops on the cable core's south face.
    let (t, i, n) = ray_boxes([4.5, 64.5, 1.0], [0.0, 0.0, -1.0], 5.0, &world, [0, 0, 1]).unwrap();
    // The host spans z -2..-1; the core's south face is at -2 + 10/16.
    assert!((t - (1.0 + 2.0 - 10.0 / 16.0)).abs() < 1e-9, "{t}");
    assert_eq!(n, [0, 0, 1]);
    assert_eq!(boxes[i].0, None);
    // A ray through the gaps misses everything.
    assert!(ray_boxes([4.05, 64.05, 1.0], [0.0, 0.0, -1.0], 5.0, &world, [0, 0, 1]).is_none());
    // AE2's arm test for placing on a cable segment.
    let mut links = [false; 6];
    links[Dir::East.idx()] = true;
    assert_eq!(arm_at(&h, &specs, links, [0.9, 0.5, 0.5]), Some(Dir::East));
    assert_eq!(arm_at(&h, &specs, links, [0.5, 0.5, 0.5]), None);
    assert_eq!(arm_at(&h, &specs, links, [0.5, 0.5, 0.25]), Some(Dir::North), "the arm into the terminal");
}

#[test]
fn parts_are_added_like_part_placement() {
    let spec_of = &specs;
    // On a stone face: the host goes beside, the part on the face towards the stone.
    assert_eq!(placement(&terminal(), spec_of, None, false, None, Dir::Up, None, true), Some((Target::Beside, Some(Dir::Down))));
    // A cable on stone: the centre beside.
    assert_eq!(placement(&glass(), spec_of, None, false, None, Dir::Up, None, true), Some((Target::Beside, None)));
    // Nowhere to go.
    assert_eq!(placement(&glass(), spec_of, None, false, None, Dir::Up, None, false), None);
    let mut h = Host::new(HOST);
    h.put(None, Part::new("test:glass_cable")).unwrap();
    // A terminal clicked on the cable's east arm goes on the east face; on the core's top, on the top.
    assert_eq!(placement(&terminal(), spec_of, Some(&h), false, Some(Dir::East), Dir::Up, None, true), Some((Target::Clicked, Some(Dir::East))));
    assert_eq!(placement(&terminal(), spec_of, Some(&h), false, None, Dir::Up, None, true), Some((Target::Clicked, Some(Dir::Up))));
    // A second cable cannot go in; it goes beside.
    assert_eq!(placement(&glass(), spec_of, Some(&h), false, None, Dir::Up, None, true), Some((Target::Beside, None)));
    // A taken face sends the part to the next block (its face towards this one).
    h.put(Some(Dir::Up), Part::new("test:terminal")).unwrap();
    assert_eq!(placement(&import_bus(), spec_of, Some(&h), false, None, Dir::Up, None, true), Some((Target::Beside, Some(Dir::Down))));
    // BusSupport: a dense cable takes an anchor, not a terminal; a dense cable does not go under a terminal.
    let mut d = Host::new(HOST);
    d.put(None, Part::new("test:dense_cable")).unwrap();
    assert!(can_add(&d, &anchor(), Some(Dir::North), spec_of));
    assert!(!can_add(&d, &terminal(), Some(Dir::North), spec_of));
    let mut t = Host::new(HOST);
    t.put(Some(Dir::North), Part::new("test:terminal")).unwrap();
    assert!(!can_add(&t, &dense(), None, spec_of));
    assert!(can_add(&t, &glass(), None, spec_of));
    // Another mod's host takes none of these parts.
    assert!(!can_add(&Host::new("other:bus"), &glass(), None, spec_of));
}

#[test]
fn links_stop_at_face_parts_and_looks_follow_them() {
    let mut h = Host::new(HOST);
    h.put(None, Part::new("test:glass_cable")).unwrap();
    h.put(Some(Dir::West), Part::new("test:terminal")).unwrap();
    let l = links(&h, |_| true);
    assert!(!l[Dir::West.idx()]);
    assert_eq!(l.iter().filter(|x| **x).count(), 5);
    let arms = center_arms(&h, &specs, links(&h, |d| d == Dir::East || d == Dir::West));
    let (block, props) = look(&glass(), None, arms);
    assert_eq!(block, "test:glass_cable");
    let on: Vec<&str> = props.iter().filter(|(_, v)| v == "true").map(|(k, _)| k.as_str()).collect();
    assert_eq!(on, vec!["west", "east"], "the arm into the terminal and the linked east side");
    // A face part's look: its facing (the opposite side for models drawn from the back).
    assert_eq!(look(&terminal(), Some(Dir::West), arms).1, vec![("facing".to_owned(), "east".to_owned())]);
    assert_eq!(look(&import_bus(), Some(Dir::West), arms).1, vec![("facing".to_owned(), "west".to_owned())]);
    // No centre: no links at all.
    assert_eq!(links(&Host::new(HOST), |_| true), [false; 6]);
}

#[test]
fn removing_parts_drops_them_and_an_empty_host_goes() {
    let mut h = Host::new(HOST);
    h.put(None, Part::new("test:glass_cable")).unwrap();
    h.put(Some(Dir::North), Part::new("test:terminal")).unwrap();
    assert!(h.put(Some(Dir::North), Part::new("test:import_bus")).is_err(), "a taken slot");
    let out = h.remove(Some(Dir::North));
    assert_eq!(part_drops(&out, &specs), vec!["test:terminal".to_owned()]);
    assert!(!h.is_empty());
    let out = h.remove(None);
    assert_eq!(part_drops(&out, &specs), vec!["test:glass_cable".to_owned()]);
    assert!(h.is_empty(), "the host disappears when empty");
    assert!(h.remove(None).is_empty(), "nothing twice");
    // A migration's centre (no item) goes with the last real part and gives nothing.
    let mut m = Host::new(HOST);
    let mut legacy = Part::new("test:glass_cable");
    legacy.set(NO_DROP, 1);
    m.put(None, legacy).unwrap();
    m.put(Some(Dir::East), Part::new("test:import_bus")).unwrap();
    let out = m.remove(Some(Dir::East));
    assert_eq!(out.len(), 2);
    assert_eq!(part_drops(&out, &specs), vec!["test:import_bus".to_owned()]);
    assert!(m.is_empty());
}

#[test]
fn breaking_a_whole_host_gives_back_every_part() {
    let mut h = Host::new(HOST);
    h.put(None, Part::new("test:glass_cable")).unwrap();
    for d in [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East] {
        h.put(Some(d), Part::new(if d == Dir::Up { "test:import_bus" } else { "test:terminal" })).unwrap();
    }
    let mut unknown = Host::new(HOST);
    unknown.put(None, Part::new("removedmod:pipe")).unwrap();
    let parts = h.take_all();
    assert_eq!(parts.len(), 7);
    let drops = part_drops(&parts, &specs);
    assert_eq!(drops.iter().filter(|d| *d == "test:terminal").count(), 5);
    assert_eq!(drops.iter().filter(|d| *d == "test:import_bus").count(), 1);
    assert_eq!(drops.iter().filter(|d| *d == "test:glass_cable").count(), 1);
    assert!(h.is_empty());
    // A part of a removed mod still has a box to hit and gives its id back.
    assert_eq!(host_boxes(&unknown, &specs, [false; 6], false).len(), 1);
    assert_eq!(part_drops(&unknown.take_all(), &specs), vec!["removedmod:pipe".to_owned()]);
}

#[test]
fn hosts_survive_the_side_file() {
    let mut t: Machines<St> = Machines::default();
    let mut h = Host::new(HOST);
    let mut cable = Part::new("test:glass_cable");
    cable.set("color", 14);
    h.put(None, cable).unwrap();
    h.put(Some(Dir::East), Part::new("test:import_bus")).unwrap();
    let mut legacy = Part::new("test:terminal");
    legacy.set(NO_DROP, 1);
    legacy.set("ae2:legacy", -3);
    h.put(Some(Dir::Down), legacy).unwrap();
    t.hosts.insert([-12, -60, 30_000_000], h.clone());
    t.hosts.insert([0, 64, 0], Host::new(HOST)); // empty: not written
    // A machine and a pending drop share the file.
    t.ensure([1, 2, 3], &Spec::new("test:box").slots(&[SlotRole::Both]));
    t.drops.push(([5, 5, 5], st("test:terminal", 1)));
    let text = encode(&t, &Codec);
    let back = decode(&text, &Codec);
    assert_eq!(back.hosts.len(), 1);
    assert_eq!(back.hosts[&[-12, -60, 30_000_000]], h);
    assert_eq!(back.map.len(), 1);
    assert_eq!(back.drops, t.drops);
    assert_eq!(encode(&back, &Codec), text, "stable");
    // Damaged lines are skipped, the rest is kept.
    let broken = text.replace("hp east", "hp sideways") + "hp center lonely:part\nh 1 x 3 test:cable_bus\n";
    let back = decode(&broken, &Codec);
    let h2 = &back.hosts[&[-12, -60, 30_000_000]];
    assert!(h2.get(Some(Dir::East)).is_none());
    assert_eq!(h2.get(None).map(|p| p.get("color")), Some(14));
    assert_eq!(back.hosts.len(), 1);
}

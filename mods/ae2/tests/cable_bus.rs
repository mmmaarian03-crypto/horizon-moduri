// The cable bus: cables in the centre and parts on the faces of forja compound blocks, with AE2's
// connection rules (CableBusContainer.addPart / updateConnections) and the stage-1 migration.
use horizon_ae2::*;
use std::collections::BTreeSet;

fn cable_at(p: Pos, color: u8) -> BusNode {
    BusNode { loc: Loc::block(p), kind: NodeKind::Cable { color, dense: false }, idle: 0.0 }
}
fn dense_at(p: Pos) -> BusNode {
    BusNode { loc: Loc::block(p), kind: NodeKind::Cable { color: FLUIX, dense: true }, idle: 0.0 }
}
fn part_at(p: Pos, side: Dir) -> BusNode {
    BusNode { loc: Loc::part(p, side), kind: NodeKind::Device { channel: true, front: None }, idle: 0.5 }
}
fn block_at(p: Pos) -> BusNode {
    BusNode { loc: Loc::block(p), kind: NodeKind::Device { channel: true, front: None }, idle: 2.0 }
}
fn controller_at(p: Pos) -> BusNode {
    BusNode { loc: Loc::block(p), kind: NodeKind::Controller, idle: config::CONTROLLER_IDLE }
}
fn blocked_by(nodes: &[BusNode]) -> BTreeSet<(Pos, Dir)> {
    nodes.iter().filter_map(|n| n.loc.side.map(|s| (n.loc.pos, s))).collect()
}
fn nets(nodes: &[BusNode]) -> Vec<Vec<Loc>> {
    let (_, nets) = bus_networks(nodes, &blocked_by(nodes));
    nets.iter().map(|n| n.members.iter().map(|&i| nodes[i].loc).collect()).collect()
}

#[test]
fn a_part_joins_its_own_cable_only() {
    let a = [0, 64, 0];
    let b = [1, 64, 0];
    // A terminal on the cable's north face; the next cable east: one grid of three nodes.
    let nodes = vec![cable_at(a, FLUIX), part_at(a, Dir::North), cable_at(b, FLUIX)];
    assert_eq!(nets(&nodes).len(), 1);
    // A part on the east face between them cuts the cables apart (it belongs to a's grid).
    let nodes = vec![cable_at(a, FLUIX), part_at(a, Dir::East), cable_at(b, FLUIX)];
    let n = nets(&nodes);
    assert_eq!(n.len(), 2);
    assert!(n.iter().any(|m| m.contains(&Loc::part(a, Dir::East)) && m.contains(&Loc::block(a))));
    // The same on the other host's face.
    let nodes = vec![cable_at(a, FLUIX), cable_at(b, FLUIX), part_at(b, Dir::West)];
    assert_eq!(nets(&nodes).len(), 2);
    // A part with no cable in its bus is alone, even facing a cable.
    let nodes = vec![part_at(a, Dir::East), cable_at(b, FLUIX)];
    assert_eq!(nets(&nodes).len(), 2);
    // A part never reaches the block in front of it (a drive behind a terminal's screen).
    let nodes = vec![cable_at(a, FLUIX), part_at(a, Dir::East), block_at(b)];
    assert_eq!(nets(&nodes).len(), 2);
}

#[test]
fn whole_blocks_and_colours_keep_their_rules() {
    let a = [0, 64, 0];
    // A drive beside a cable bus joins its cable when that face is free.
    let nodes = vec![cable_at(a, 14), block_at([0, 65, 0])];
    assert_eq!(nets(&nodes).len(), 1);
    // A cable anchor (no node) on that face still blocks it.
    let nodes = vec![cable_at(a, 14), block_at([0, 65, 0])];
    let mut blocked = blocked_by(&nodes);
    blocked.insert((a, Dir::Up));
    assert_eq!(bus_networks(&nodes, &blocked).1.len(), 2);
    // Red and blue cables stay apart, fluix joins both.
    let nodes = vec![cable_at(a, 14), cable_at([1, 64, 0], 11)];
    assert_eq!(nets(&nodes).len(), 2);
    let nodes = vec![cable_at(a, 14), cable_at([1, 64, 0], FLUIX), cable_at([2, 64, 0], 11)];
    assert_eq!(nets(&nodes).len(), 1);
}

#[test]
fn parts_take_channels_through_their_cable() {
    // Ad hoc: eight parts work, a ninth device stops them all (PathingService.AD_HOC_CHANNELS).
    let mut nodes = Vec::new();
    for x in 0..2 {
        let p = [x, 64, 0];
        nodes.push(cable_at(p, FLUIX));
        for side in [Dir::Down, Dir::Up, Dir::North, Dir::South] {
            nodes.push(part_at(p, side));
        }
    }
    let (specs, n) = bus_networks(&nodes, &blocked_by(&nodes));
    assert_eq!(n.len(), 1);
    let ch = assign_channels(&specs, &n[0]);
    assert_eq!(ch.state, ControllerState::NoController);
    assert_eq!(ch.granted.len(), 8);
    nodes.push(block_at([2, 64, 0]));
    let (specs, n) = bus_networks(&nodes, &blocked_by(&nodes));
    let ch = assign_channels(&specs, &n[0]);
    assert_eq!(ch.error, Some(AdHocError::TooManyChannels));
    // With a controller: a normal cable carries 8, so of 6 + 6 parts behind one cable only 8 get one.
    let mut nodes = vec![controller_at([0, 64, 0]), cable_at([1, 64, 0], FLUIX)];
    for x in 2..4 {
        let p = [x, 64, 0];
        nodes.push(cable_at(p, FLUIX));
        for side in [Dir::Down, Dir::Up, Dir::North, Dir::South] {
            nodes.push(part_at(p, side));
        }
    }
    nodes.push(part_at([3, 64, 0], Dir::East));
    let (specs, n) = bus_networks(&nodes, &blocked_by(&nodes));
    assert_eq!(n.len(), 1);
    let ch = assign_channels(&specs, &n[0]);
    assert_eq!(ch.state, ControllerState::Online);
    assert_eq!(ch.granted.len(), 8);
    // A dense cable first carries 32, but the normal cable after it is the bottleneck: still 8.
    let mut nodes = vec![controller_at([0, 64, 0]), dense_at([1, 64, 0])];
    for x in 2..4 {
        let p = [x, 64, 0];
        nodes.push(cable_at(p, FLUIX));
        for side in [Dir::Down, Dir::Up, Dir::North, Dir::South] {
            nodes.push(part_at(p, side));
        }
    }
    let (specs, n) = bus_networks(&nodes, &blocked_by(&nodes));
    assert_eq!(assign_channels(&specs, &n[0]).granted.len(), 8);
}

#[test]
fn ae2_part_numbers() {
    assert_eq!(cable_of("ae2:red_smart_dense_cable"), Some((14, "_smart_dense_cable", true)));
    assert_eq!(cable_of("ae2:fluix_glass_cable"), Some((FLUIX, "_glass_cable", false)));
    assert_eq!(cable_of("ae2:terminal"), None);
    assert_eq!(cable_size("_glass_cable"), (6.0, 10.0, None));
    assert_eq!(cable_size("_covered_cable"), (5.0, 11.0, None));
    assert_eq!(cable_size("_smart_dense_cable"), (3.0, 13.0, Some((4.9, 11.1))));
    assert_eq!(part_def("ae2:import_bus").unwrap().cable_length, 5.0);
    assert_eq!(part_def("ae2:cable_anchor").unwrap().groups, 3);
    assert!(!part_def("ae2:cable_anchor").unwrap().node);
    // Every part box is inside the block, on the SOUTH half where parts sit.
    for p in PARTS {
        for b in p.boxes {
            assert!((0..3).all(|k| b[k] >= 0.0 && b[k + 3] <= 16.0 && b[k] < b[k + 3]), "{}", p.id);
            assert!(b[5] == 16.0 || b[2] >= 10.0, "{}", p.id);
        }
    }
}

#[test]
fn ae2_parts_through_forja_turn_to_their_face() {
    use horizon_forja as forja;
    let t = part_def("ae2:terminal").unwrap();
    let spec = forja::PartSpec::face(t.id, CABLE_BUS, t.id, t.boxes).cable_length(t.cable_length).facing_opposite();
    // A terminal on the west face: its screen box hugs x = 0.
    let boxes = forja::part_boxes(&spec, Some(forja::Dir::West), [None; 6], false);
    assert_eq!(boxes[0], [0.0, 2.0 / 16.0, 2.0 / 16.0, 2.0 / 16.0, 14.0 / 16.0, 14.0 / 16.0]);
    // Its model state faces east (the stage-1 part blocks are drawn from the back).
    assert_eq!(forja::look(&spec, Some(forja::Dir::West), [None; 6]).1, vec![("facing".to_owned(), "east".to_owned())]);
}

#[test]
fn stage1_blocks_become_cable_buses_without_new_items() {
    // A cable stays the same cable.
    let m = stage1_migration("ae2:blue_covered_cable", None).unwrap();
    assert_eq!(m.center, "ae2:blue_covered_cable");
    assert!(!m.center_legacy);
    assert_eq!(m.part, None);
    // An import bus placed facing north (the player north of it) works on its south face: same target.
    let m = stage1_migration("ae2:import_bus", Some(Dir::North)).unwrap();
    assert_eq!(m.part, Some((Dir::South, "ae2:import_bus".to_owned())));
    assert_eq!(m.center, LEGACY_CABLE);
    assert!(m.center_legacy, "the stub gives no item back");
    assert!(stage1_migration("ae2:terminal", None).is_none(), "no facing: left alone");
    assert!(stage1_migration("ae2:drive", Some(Dir::North)).is_none(), "whole blocks stay");
    assert!(stage1_migration("ae2:cable_anchor", Some(Dir::North)).is_none());
}

#[test]
fn part_records_survive_the_side_file() {
    let mut s = State::default();
    let mut iface = Device::new("ae2:cable_interface", 9, 9);
    iface.slots[3] = Some(("k_iron".to_owned(), 17));
    iface.config[3] = Some(("k_iron".to_owned(), 32));
    s.devices.insert(Loc::part([4, 70, -9], Dir::Up), iface);
    s.devices.insert([4, 70, -9], Device::new("ae2:fluix_glass_cable", 0, 0));
    let mut term = Device::new("ae2:crafting_terminal", 9, 0);
    term.slots[0] = Some(("k_stick".to_owned(), 2));
    s.devices.insert(Loc::part([4, 70, -9], Dir::North), term);
    let text = s.save();
    let back = State::load(Some(text.clone()));
    assert_eq!(back, s);
    assert_eq!(back.save(), text);
    // A stage-1 file (9 words a line) loads as whole blocks.
    let old = "D 1 2 3 ae2:terminal north 0 0 0\nD 1 2 4 ae2:drive north 12.5 10 0\nS 0 k_cell 1\n";
    let back = State::load(Some(old.to_owned()));
    assert!(back.devices.contains_key(&Loc::block([1, 2, 3])));
    assert_eq!(back.devices[&Loc::block([1, 2, 4])].slots[0], Some(("k_cell".to_owned(), 1)));
    // An unknown face word drops that record only.
    let bad = "D 1 2 3 ae2:terminal - 0 0 0 sideways\nS 0 x 1\nD 5 5 5 ae2:drive - 0 10 0\n";
    let back = State::load(Some(bad.to_owned()));
    assert_eq!(back.devices.len(), 1);
}

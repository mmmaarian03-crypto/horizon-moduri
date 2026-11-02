// Rule tests for AE2 stage 1 on fake worlds (no game): cells, storage, channels, power,
// machines, terminal, world state. Expected numbers are worked out from the AE2 15.4.11 code
// quoted in src/lib.rs.
use horizon_ae2::*;
use std::collections::BTreeMap;

fn k(s: &str) -> Key {
    s.to_owned()
}

// ---------------------------------------------------------------- cells

#[test]
fn cell_types_match_aeitems() {
    let c = |i: &str| *cell_type(i).unwrap();
    assert_eq!((c("ae2:item_storage_cell_1k").bytes, c("ae2:item_storage_cell_1k").bytes_per_type), (1024, 8));
    assert_eq!((c("ae2:item_storage_cell_4k").bytes, c("ae2:item_storage_cell_4k").bytes_per_type), (4096, 32));
    assert_eq!((c("ae2:item_storage_cell_16k").bytes, c("ae2:item_storage_cell_16k").bytes_per_type), (16384, 128));
    assert_eq!((c("ae2:item_storage_cell_64k").bytes, c("ae2:item_storage_cell_64k").bytes_per_type), (65536, 512));
    assert_eq!((c("ae2:item_storage_cell_256k").bytes, c("ae2:item_storage_cell_256k").bytes_per_type), (262144, 2048));
    assert_eq!(c("ae2:item_storage_cell_256k").idle_drain, 2.5);
    assert!(CELLS.iter().all(|c| c.types == 63));
    assert!(cell_type("ae2:fluid_storage_cell_1k").is_none());
}

#[test]
fn one_type_fills_a_1k_cell_with_8128_items() {
    let t = cell_type("ae2:item_storage_cell_1k").unwrap();
    let mut c = CellInv::default();
    assert_eq!(c.status(t), CellStatus::Empty);
    assert_eq!(c.insert(t, &k("stone"), 10_000, true, false), 8128);
    assert!(c.items.is_empty(), "simulate changes nothing");
    assert_eq!(c.insert(t, &k("stone"), 10_000, false, false), 8128);
    assert_eq!(c.used_bytes(t), 1024);
    assert_eq!(c.free_bytes(t), 0);
    assert_eq!(c.status(t), CellStatus::Full);
    assert_eq!(c.insert(t, &k("stone"), 1, false, false), 0);
    assert_eq!(c.insert(t, &k("dirt"), 1, false, false), 0);
}

#[test]
fn first_type_capacity_of_every_tier() {
    for (item, max) in [
        ("ae2:item_storage_cell_4k", 32_512u64),
        ("ae2:item_storage_cell_16k", 130_048),
        ("ae2:item_storage_cell_64k", 520_192),
        ("ae2:item_storage_cell_256k", 2_080_768),
    ] {
        let t = cell_type(item).unwrap();
        let mut c = CellInv::default();
        assert_eq!(c.insert(t, &k("a"), u64::MAX / 2, false, false), max, "{item}");
    }
}

#[test]
fn partial_bytes_and_used_bytes() {
    let t = cell_type("ae2:item_storage_cell_1k").unwrap();
    let mut c = CellInv::default();
    c.insert(t, &k("a"), 1, false, false);
    assert_eq!(c.unused_item_count(), 7);
    assert_eq!(c.used_bytes(t), 9);
    assert_eq!(c.remaining_item_count(t), 8127);
    assert_eq!(c.status(t), CellStatus::NotEmpty);
}

#[test]
fn sixty_three_types_then_no_more() {
    let t = cell_type("ae2:item_storage_cell_1k").unwrap();
    let mut c = CellInv::default();
    for i in 0..63 {
        assert_eq!(c.insert(t, &format!("item{i}"), 1, false, false), 1);
    }
    assert_eq!(c.used_bytes(t), 63 * 8 + 8);
    assert_eq!(c.remaining_types(t), 0);
    assert!(!c.can_hold_new_item(t));
    assert_eq!(c.status(t), CellStatus::TypesFull);
    assert_eq!(c.insert(t, &k("item63"), 1, false, false), 0);
    // An existing type still takes more.
    assert_eq!(c.insert(t, &k("item0"), 5, false, false), 5);
}

#[test]
fn the_last_bytes_take_a_new_type_only_into_the_unused_part() {
    // free == bytesPerType with unused items: canHoldNewItem, and the new type gets exactly the
    // unused count (BasicCellInventory quirk kept 1:1).
    let t = cell_type("ae2:item_storage_cell_1k").unwrap();
    let mut c = CellInv::default();
    c.insert(t, &k("a"), 8060, false, false);
    assert_eq!(c.free_bytes(t), 8);
    assert_eq!(c.unused_item_count(), 4);
    assert!(c.can_hold_new_item(t));
    assert_eq!(c.insert(t, &k("b"), 10, false, false), 4);
    assert_eq!(c.used_bytes(t), 1024);
}

#[test]
fn non_empty_cells_do_not_go_into_cells_and_extract_is_exact() {
    let t = cell_type("ae2:item_storage_cell_4k").unwrap();
    let mut c = CellInv::default();
    assert_eq!(c.insert(t, &k("cell"), 1, false, true), 0);
    c.insert(t, &k("a"), 100, false, false);
    assert_eq!(c.extract("a", 30, true), 30);
    assert_eq!(c.amount("a"), 100);
    assert_eq!(c.extract("a", 30, false), 30);
    assert_eq!(c.extract("a", 1000, false), 70);
    assert!(c.items.is_empty());
    assert_eq!(c.extract("a", 1, false), 0);
}

#[test]
fn cell_text_round_trips_and_skips_garbage() {
    let mut c = CellInv::default();
    c.items.push((k("0a0b"), 5));
    c.items.push((k("ffee"), 123_456_789_012));
    assert_eq!(CellInv::decode(&c.encode()), c);
    let bad = "0a0b 5\nbroken\nx -3\nffee 0\n";
    assert_eq!(CellInv::decode(bad).items, vec![(k("0a0b"), 5)]);
}

// ---------------------------------------------------------------- network storage

/// A fake inventory of slots (a chest behind a storage bus).
#[derive(Clone, Debug, Default)]
struct Chest {
    slots: Vec<Option<(Key, u64)>>,
    max: u64,
}
impl Chest {
    fn new(n: usize) -> Self {
        Chest { slots: vec![None; n], max: 64 }
    }
    fn total(&self, key: &str) -> u64 {
        self.slots.iter().flatten().filter(|(x, _)| x == key).map(|e| e.1).sum()
    }
}
impl Inv for Chest {
    fn slots(&mut self) -> usize {
        self.slots.len()
    }
    fn get(&mut self, slot: usize) -> Option<(Key, u64)> {
        self.slots[slot].clone()
    }
    fn insert(&mut self, slot: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        let max = self.max;
        let s = &mut self.slots[slot];
        let room = match s {
            None => max,
            Some((x, n)) if x == key => max - *n,
            _ => 0,
        };
        let n = amount.min(room);
        if !simulate && n > 0 {
            match s {
                Some((_, c)) => *c += n,
                None => *s = Some((key.clone(), n)),
            }
        }
        n
    }
    fn extract(&mut self, slot: usize, amount: u64, simulate: bool) -> u64 {
        let s = &mut self.slots[slot];
        let Some((_, c)) = s else { return 0 };
        let n = amount.min(*c);
        if !simulate {
            *c -= n;
            if *c == 0 {
                *s = None;
            }
        }
        n
    }
}

enum Mount {
    Cell(&'static CellType, CellInv, i32),
    Bus(BusFilter, Chest),
}
struct Net(Vec<Mount>);
impl Mounts for Net {
    fn mounts(&self) -> usize {
        self.0.len()
    }
    fn priority(&self, i: usize) -> i32 {
        match &self.0[i] {
            Mount::Cell(_, _, p) => *p,
            Mount::Bus(f, _) => f.priority,
        }
    }
    fn insert(&mut self, i: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        match &mut self.0[i] {
            Mount::Cell(t, c, _) => c.insert(t, key, amount, simulate, false),
            Mount::Bus(f, inv) => f.insert(inv, key, amount, simulate),
        }
    }
    fn extract(&mut self, i: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        match &mut self.0[i] {
            Mount::Cell(_, c, _) => c.extract(key, amount, simulate),
            Mount::Bus(f, inv) => f.extract(inv, key, amount, simulate),
        }
    }
    fn listed(&self, i: usize, key: &Key) -> bool {
        matches!(&self.0[i], Mount::Bus(f, _) if f.listed(key))
    }
    fn list(&mut self, i: usize, out: &mut BTreeMap<Key, u64>) {
        match &mut self.0[i] {
            Mount::Cell(_, c, _) => {
                for (key, n) in &c.items {
                    *out.entry(key.clone()).or_insert(0) += n;
                }
            }
            Mount::Bus(_, inv) => inv_list(inv, out),
        }
    }
}
fn cell(tier: &str, prio: i32) -> Mount {
    Mount::Cell(cell_type(tier).unwrap(), CellInv::default(), prio)
}
fn cell_amount(n: &Net, i: usize, key: &str) -> u64 {
    match &n.0[i] {
        Mount::Cell(_, c, _) => c.amount(key),
        Mount::Bus(_, inv) => inv.total(key),
    }
}

#[test]
fn insert_and_extract_never_lose_or_create_items() {
    let mut n = Net(vec![cell("ae2:item_storage_cell_1k", 0), cell("ae2:item_storage_cell_1k", 0)]);
    // Two 1k cells take 2 * 8128 of one item; the rest stays with the caller.
    let wanted = 20_000u64;
    let took = net_insert(&mut n, &k("stone"), wanted, false);
    assert_eq!(took, 16_256);
    assert_eq!(net_list(&mut n).get("stone"), Some(&16_256));
    let back = net_extract(&mut n, &k("stone"), u64::MAX, false);
    assert_eq!(back, 16_256);
    assert!(net_list(&mut n).is_empty());
}

#[test]
fn higher_priority_first_and_preferred_cells_first() {
    let mut chest = Chest::new(27);
    chest.max = 64;
    let bus = Mount::Bus(BusFilter { priority: 10, partition: vec![] }, chest);
    let mut n = Net(vec![cell("ae2:item_storage_cell_4k", 0), cell("ae2:item_storage_cell_4k", 0), bus]);
    // Priority 10 (the storage bus) fills first: 27 slots * 64.
    assert_eq!(net_insert(&mut n, &k("dirt"), 100, false), 100);
    assert_eq!(cell_amount(&n, 2, "dirt"), 100);
    // A cell already holding the key is preferred inside its priority group.
    n.0[2] = Mount::Bus(BusFilter { priority: -5, partition: vec![] }, Chest::new(27));
    net_insert(&mut n, &k("iron"), 5, false); // goes to cell 0 (first, nothing preferred)
    if let Mount::Cell(t, c, _) = &mut n.0[1] {
        c.insert(t, &k("gold"), 1, false, false);
    }
    net_insert(&mut n, &k("gold"), 10, false);
    assert_eq!(cell_amount(&n, 1, "gold"), 11);
    assert_eq!(cell_amount(&n, 0, "gold"), 0);
    // Extraction takes from the lowest priority first: the bus at -5.
    net_insert(&mut n, &k("iron"), 0, false);
    if let Mount::Bus(f, inv) = &mut n.0[2] {
        f.insert(inv, &k("iron"), 7, false);
    }
    assert_eq!(net_extract(&mut n, &k("iron"), 7, false), 7);
    assert_eq!(cell_amount(&n, 2, "iron"), 0);
    assert_eq!(cell_amount(&n, 0, "iron"), 5);
}

#[test]
fn storage_bus_partition_whitelists_and_is_preferred() {
    let bus = Mount::Bus(BusFilter { priority: 0, partition: vec![k("coal")] }, Chest::new(9));
    let mut n = Net(vec![cell("ae2:item_storage_cell_1k", 0), bus]);
    // Same priority: the whitelisting bus is preferred for coal even though the cell comes first.
    net_insert(&mut n, &k("coal"), 10, false);
    assert_eq!(cell_amount(&n, 1, "coal"), 10);
    // Anything else is refused by the bus and goes into the cell.
    net_insert(&mut n, &k("sand"), 10, false);
    assert_eq!(cell_amount(&n, 1, "sand"), 0);
    assert_eq!(cell_amount(&n, 0, "sand"), 10);
    // Items in the chest that are not in the partition are listed but not extractable.
    if let Mount::Bus(_, inv) = &mut n.0[1] {
        inv.slots[5] = Some((k("apple"), 3));
    }
    assert_eq!(net_list(&mut n).get("apple"), Some(&3));
    assert_eq!(net_extract(&mut n, &k("apple"), 3, false), 0);
}

struct Juice(f64);
impl EnergySource for Juice {
    fn extract_ae(&mut self, amount: f64, simulate: bool) -> f64 {
        let n = amount.min(self.0);
        if !simulate {
            self.0 -= n;
        }
        n
    }
}

#[test]
fn powered_insert_and_extract_cost_one_ae_per_item() {
    let mut n = Net(vec![cell("ae2:item_storage_cell_4k", 0)]);
    let mut e = Juice(1000.0);
    assert_eq!(powered_insert(&mut e, &mut n, &k("a"), 64, false), 64);
    assert!((e.0 - 936.0).abs() < 1e-9);
    assert_eq!(powered_extract(&mut e, &mut n, &k("a"), 10, false), 10);
    assert!((e.0 - 926.0).abs() < 1e-9);
    // Not enough power: only what the energy pays for (+0.9 rounding).
    let mut poor = Juice(4.2);
    assert_eq!(powered_insert(&mut poor, &mut n, &k("b"), 10, false), 5);
    let mut none = Juice(0.0);
    assert_eq!(powered_extract(&mut none, &mut n, &k("a"), 10, false), 0);
    assert_eq!(cell_amount(&n, 0, "a"), 54);
}

#[test]
fn import_and_export_buses_move_one_item_per_operation() {
    let mut chest = Chest::new(5);
    chest.slots[0] = Some((k("log"), 10));
    chest.slots[1] = Some((k("sand"), 3));
    let mut n = Net(vec![cell("ae2:item_storage_cell_1k", 0)]);
    let moved = import_transfer(&mut Both { m: &mut n, i: &mut chest }, &[], IO_BUS_OPS);
    assert_eq!(moved.items, 1);
    assert_eq!(chest.total("log"), 9);
    assert_eq!(cell_amount(&n, 0, "log"), 1);
    // A filter: only sand.
    import_transfer(&mut Both { m: &mut n, i: &mut chest }, &[k("sand")], 8);
    assert_eq!(chest.total("sand"), 0);
    assert_eq!(chest.total("log"), 9);
    // Export: the configured key, powered.
    let mut target = Chest::new(1);
    let mut e = Juice(100.0);
    let out = export_transfer(&mut Both { m: &mut n, i: &mut target }, &mut e, &[None, Some(k("sand"))], IO_BUS_OPS);
    assert_eq!(out.items, 1);
    assert!(out.stray.is_empty());
    assert_eq!(target.total("sand"), 1);
    assert_eq!(cell_amount(&n, 0, "sand"), 2);
    // A full target takes nothing and nothing leaves the network.
    target.slots[0] = Some((k("dirt"), 64));
    let out = export_transfer(&mut Both { m: &mut n, i: &mut target }, &mut e, &[Some(k("sand"))], 8);
    assert_eq!(out.items, 0);
    assert_eq!(cell_amount(&n, 0, "sand"), 2);
}

// ---------------------------------------------------------------- channels

fn cable(p: Pos) -> NodeSpec {
    NodeSpec { pos: p, kind: NodeKind::Cable { color: FLUIX, dense: false }, idle: 0.0 }
}
fn dense(p: Pos) -> NodeSpec {
    NodeSpec { pos: p, kind: NodeKind::Cable { color: FLUIX, dense: true }, idle: 0.0 }
}
fn device(p: Pos) -> NodeSpec {
    NodeSpec { pos: p, kind: NodeKind::Device { channel: true, front: None }, idle: 1.0 }
}
fn controller(p: Pos) -> NodeSpec {
    NodeSpec { pos: p, kind: NodeKind::Controller, idle: config::CONTROLLER_IDLE }
}
fn one_net(nodes: &[NodeSpec]) -> Network {
    let mut nets = networks(nodes);
    assert_eq!(nets.len(), 1, "one network expected");
    nets.remove(0)
}

#[test]
fn ad_hoc_network_has_eight_channels() {
    let mut nodes = vec![];
    for x in 0..9 {
        nodes.push(cable([x, 0, 0]));
    }
    for x in 0..8 {
        nodes.push(device([x, 1 + (x % 2) * -2, 0]));
    }
    let net = one_net(&nodes);
    let ch = assign_channels(&nodes, &net);
    assert_eq!(ch.state, ControllerState::NoController);
    assert_eq!(ch.in_use, 8);
    assert_eq!(ch.granted.len(), 8);
    assert_eq!(ch.by_blocks, 17 * 8);
    // A ninth device: too many channels, nobody works.
    nodes.push(device([8, 1, 0]));
    let net = one_net(&nodes);
    let ch = assign_channels(&nodes, &net);
    assert_eq!(ch.error, Some(AdHocError::TooManyChannels));
    assert!(ch.granted.is_empty());
    assert_eq!(ch.in_use, 0);
}

#[test]
fn controller_gives_eight_channels_through_a_normal_cable() {
    let mut nodes = vec![controller([0, 0, 0])];
    for x in 1..=10 {
        nodes.push(cable([x, 0, 0]));
        // Devices alternate above and below so they never touch each other.
        nodes.push(device([x, if x % 2 == 0 { 1 } else { -1 }, 0]));
    }
    let net = one_net(&nodes);
    let ch = assign_channels(&nodes, &net);
    assert_eq!(ch.state, ControllerState::Online);
    assert_eq!(ch.in_use, 8);
    // The nearest eight devices (BFS order) get them.
    let granted_x: Vec<i32> = ch.granted.iter().map(|&i| nodes[i].pos[0]).collect();
    let mut sorted = granted_x.clone();
    sorted.sort();
    assert_eq!(sorted, (1..=8).collect::<Vec<_>>());
    // The first cable carries all eight.
    assert_eq!(ch.used.get(&1), Some(&8));
}

#[test]
fn dense_cable_carries_thirty_two() {
    let mut nodes = vec![controller([0, 0, 0])];
    for x in 1..=40 {
        nodes.push(dense([x, 0, 0]));
        nodes.push(device([x, if x % 2 == 0 { 1 } else { -1 }, 0]));
    }
    let net = one_net(&nodes);
    let ch = assign_channels(&nodes, &net);
    assert_eq!(ch.in_use, 32);
    assert_eq!(ch.used.get(&1), Some(&32));
}

#[test]
fn every_controller_face_is_its_own_source() {
    // Two normal cables on two faces of one controller: 8 + 8.
    let mut nodes = vec![controller([0, 0, 0])];
    for x in 1..=10 {
        nodes.push(cable([x, 0, 0]));
        nodes.push(device([x, 1, 5 + x * 0]));
    }
    // Make the second branch on the west side, devices far from the first ones.
    let mut west = vec![];
    for x in 1..=10 {
        west.push(cable([-x, 0, 0]));
        west.push(device([-x, if x % 2 == 0 { 1 } else { -1 }, 0]));
    }
    // Replace the first branch's devices to alternate too.
    let mut nodes2 = vec![controller([0, 0, 0])];
    for x in 1..=10 {
        nodes2.push(cable([x, 0, 0]));
        nodes2.push(device([x, if x % 2 == 0 { 1 } else { -1 }, 0]));
    }
    nodes2.extend(west);
    let _ = nodes;
    let net = one_net(&nodes2);
    let ch = assign_channels(&nodes2, &net);
    assert_eq!(ch.in_use, 16);
}

#[test]
fn controller_shape_rules() {
    // Seven in a row is fine, eight is a conflict (MAX_SIZE 7).
    let row = |n: i32| (0..n).map(|x| controller([x, 0, 0])).collect::<Vec<_>>();
    let nodes = row(7);
    assert_eq!(controller_state(&nodes, &one_net(&nodes)), ControllerState::Online);
    let nodes = row(8);
    assert_eq!(controller_state(&nodes, &one_net(&nodes)), ControllerState::Conflict);
    // A cross (lines on two axes through one block) is a conflict.
    let nodes = vec![
        controller([0, 0, 0]),
        controller([1, 0, 0]),
        controller([-1, 0, 0]),
        controller([0, 0, 1]),
        controller([0, 0, -1]),
    ];
    assert_eq!(controller_state(&nodes, &one_net(&nodes)), ControllerState::Conflict);
    // Two controllers joined only through a cable are a conflict, and no device works.
    let nodes = vec![controller([0, 0, 0]), cable([1, 0, 0]), controller([2, 0, 0]), device([1, 1, 0])];
    let net = one_net(&nodes);
    let ch = assign_channels(&nodes, &net);
    assert_eq!(ch.state, ControllerState::Conflict);
    assert!(ch.granted.is_empty());
}

#[test]
fn cable_colours_only_join_their_own_colour_or_fluix() {
    let red = NodeSpec { pos: [0, 0, 0], kind: NodeKind::Cable { color: 14, dense: false }, idle: 0.0 };
    let blue = NodeSpec { pos: [1, 0, 0], kind: NodeKind::Cable { color: 11, dense: false }, idle: 0.0 };
    assert_eq!(networks(&[red.clone(), blue.clone()]).len(), 2);
    let fluix = NodeSpec { pos: [1, 0, 0], kind: NodeKind::Cable { color: FLUIX, dense: false }, idle: 0.0 };
    assert_eq!(networks(&[red.clone(), fluix]).len(), 1);
    // A device joins any colour; a part's front does not connect.
    let term = NodeSpec { pos: [1, 0, 0], kind: NodeKind::Device { channel: true, front: Some(Dir::East) }, idle: 0.5 };
    assert_eq!(networks(&[red.clone(), term.clone()]).len(), 1);
    let term_facing = NodeSpec { kind: NodeKind::Device { channel: true, front: Some(Dir::West) }, ..term };
    assert_eq!(networks(&[red, term_facing]).len(), 2);
}

// ---------------------------------------------------------------- power

#[test]
fn idle_draw_and_thirty_tick_power_up() {
    let mut g = GridPower::new(1000.0, 8025.0);
    g.idle = 2.0;
    for i in 0..31 {
        g.end_tick();
        assert!(g.has_power);
        assert!(!g.powered, "still booting at tick {i}");
    }
    g.end_tick();
    assert!(g.powered);
    assert!((g.stored - (1000.0 - 2.0 * 32.0)).abs() < 1e-9);
}

#[test]
fn network_shuts_down_when_it_cannot_pay() {
    let mut g = GridPower::new(3.0, 8025.0);
    g.idle = 2.0;
    g.powered = true;
    g.has_power = true;
    g.end_tick();
    assert!(g.powered);
    g.end_tick(); // only 1 AE left for 2 AE/t
    assert!(!g.has_power);
    assert!(!g.powered);
    assert_eq!(g.stored, 0.0);
    // Channels cost channelsByBlocks / 128 on top of the idle draw.
    let mut g = GridPower::new(100.0, 200.0);
    g.idle = 1.0;
    g.channel_usage = 64.0 / config::CHANNEL_POWER_DIVISOR;
    g.end_tick();
    assert!((g.stored - 98.5).abs() < 1e-9);
}

#[test]
fn forge_energy_is_half_an_ae_and_limited_by_demand() {
    assert_eq!(fe_to_ae(100.0), 50.0);
    assert_eq!(ae_to_fe(25.0), 50.0);
    assert_eq!(fe_receive(10.0, 100), 20);
    assert_eq!(fe_receive(10.0, 7), 7);
    assert_eq!(fe_receive(0.4, 100), 0);
    let mut g = GridPower::new(0.0, 25.0);
    assert_eq!(g.inject(30.0, false), 5.0);
    assert_eq!(g.stored, 25.0);
}

#[test]
fn vibration_chamber_makes_8000_ae_from_coal() {
    let mut v = Vibration::default();
    assert!((v.rate - 1.0).abs() < 1e-12, "5 AE per game tick to start");
    v.eat(1600);
    let mut g = GridPower::new(0.0, 1e12);
    let mut now = 0u64;
    let mut t = Ticker::new(config::TICK_RATES_VIBRATION_CHAMBER_MIN as i32, config::TICK_RATES_VIBRATION_CHAMBER_MAX as i32, 0);
    while v.burning() {
        now += 1;
        if let Some(ticks) = t.due(now) {
            let r = v.tick(ticks, &mut g);
            t.done(now, r);
        }
    }
    assert!((g.stored - 8000.0).abs() < 1e-6, "{}", g.stored);
    assert!(v.rate <= Vibration::max_rate() + 1e-12);
}

#[test]
fn vibration_chamber_slows_down_when_power_overflows() {
    let mut v = Vibration::default();
    v.eat(1600);
    let mut g = GridPower::new(0.0, 10.0);
    let r = v.tick(10, &mut g);
    assert_eq!(r, TickRate::Slower);
    assert!(v.rate < Vibration::initial_rate());
    assert!(v.rate >= Vibration::min_rate());
}

#[test]
fn ticker_follows_ae2_rate_changes() {
    let mut t = Ticker::new(5, 40, 0);
    assert_eq!(t.current, 22);
    assert_eq!(t.due(21), None);
    assert_eq!(t.due(22), Some(22));
    t.done(22, TickRate::Faster);
    assert_eq!(t.current, 20);
    t.done(42, TickRate::Urgent);
    assert_eq!(t.current, 5);
    t.done(47, TickRate::Slower);
    assert_eq!(t.current, 6);
    t.done(53, TickRate::Sleep);
    assert_eq!(t.due(1000), None);
    t.wake(1000);
    assert_eq!(t.due(1001), Some(948));
}

// ---------------------------------------------------------------- machines

fn no_tags(_: &str, _: &str) -> bool {
    false
}
fn c_tags(t: &str, i: &str) -> bool {
    (t, i) == ("c:ingots/gold", "minecraft:gold_ingot")
}

#[test]
fn inscriber_recipes_with_mirroring() {
    let r = find_inscriber(Some("ae2:logic_processor_press"), "minecraft:gold_ingot", None, &c_tags).unwrap();
    assert_eq!(r.result, "ae2:printed_logic_processor");
    assert!(!r.press);
    // The press may sit in the bottom slot (mirrored).
    let r = find_inscriber(None, "minecraft:gold_ingot", Some("ae2:logic_processor_press"), &c_tags).unwrap();
    assert_eq!(r.result, "ae2:printed_logic_processor");
    let r = find_inscriber(Some("ae2:printed_silicon"), "minecraft:redstone", Some("ae2:printed_logic_processor"), &no_tags).unwrap();
    assert_eq!(r.result, "ae2:logic_processor");
    assert!(r.press);
    // Certus without plates is dust; with an extra plate nothing matches.
    assert_eq!(find_inscriber(None, "ae2:certus_quartz_crystal", None, &no_tags).unwrap().result, "ae2:certus_quartz_dust");
    assert!(find_inscriber(Some("ae2:silicon_press"), "minecraft:ender_pearl", None, &no_tags).is_none());
    // Copying a press needs an iron block.
    assert_eq!(find_inscriber(Some("ae2:silicon_press"), "minecraft:iron_block", None, &no_tags).unwrap().result, "ae2:silicon_press");
    assert!(inscriber_plate_ok("ae2:silicon_press", &no_tags));
    assert!(!inscriber_plate_ok("minecraft:dirt", &no_tags));
    assert!(inscriber_middle_ok("minecraft:redstone", &no_tags));
}

#[test]
fn inscriber_takes_101_steps_and_2020_ae_then_presses() {
    let mut st = Inscriber::default();
    let mut buf = Buffer::new(config::INSCRIBER_BUFFER);
    let mut grid = Juice(1e9);
    let mut ticks = 0;
    loop {
        ticks += 1;
        let step = inscriber_tick(&mut st, true, &mut buf, Some(&mut grid));
        if step == InscriberStep::Craft {
            break;
        }
        assert!(ticks < 500);
    }
    assert_eq!(ticks, 101 + 8);
    assert!((1e9 - grid.0 - 2020.0).abs() < 1e-6);
    // The animation ends 8 ticks later.
    for _ in 0..8 {
        inscriber_tick(&mut st, true, &mut buf, Some(&mut grid));
    }
    assert!(!st.smash);
    // Its own buffer is used before the grid.
    let mut st = Inscriber::default();
    let mut buf = Buffer { stored: 100.0, max: 1600.0 };
    let mut grid = Juice(0.0);
    for _ in 0..5 {
        inscriber_tick(&mut st, true, &mut buf, Some(&mut grid));
    }
    assert_eq!(st.progress, 10);
    assert!(buf.stored.abs() < 1e-9);
    inscriber_tick(&mut st, true, &mut buf, Some(&mut grid));
    assert_eq!(st.progress, 10, "no power, no progress");
}

#[test]
fn charger_needs_a_full_buffer_and_a_lucky_roll() {
    let mut buf = Buffer::new(config::CHARGER_BUFFER);
    assert_eq!(charger_tick(&mut buf, Some("ae2:certus_quartz_crystal"), 0.99, None), None);
    // The grid refills 800 per call.
    let mut grid = Juice(10_000.0);
    charger_tick(&mut buf, None, 0.0, Some(&mut grid));
    assert_eq!(buf.stored, 800.0);
    charger_tick(&mut buf, None, 0.0, Some(&mut grid));
    assert_eq!(buf.stored, 1600.0);
    assert_eq!(charger_tick(&mut buf, Some("ae2:certus_quartz_crystal"), 0.5, None), None);
    assert_eq!(charger_tick(&mut buf, Some("ae2:certus_quartz_crystal"), 0.81, None), Some("ae2:charged_certus_quartz_crystal"));
    assert_eq!(buf.stored, 0.0);
    assert!(!charger_may_take("ae2:certus_quartz_crystal"));
    assert!(charger_may_take("ae2:charged_certus_quartz_crystal"));
}

#[test]
fn crank_gives_160_ae_per_turn_after_18_ticks() {
    let mut c = Crank::default();
    assert_eq!(c.turn(true), CrankUse::Turned);
    assert_eq!(c.turn(true), CrankUse::Busy);
    let mut applied = 0;
    for _ in 0..18 {
        if c.tick() {
            applied += 1;
        }
    }
    assert_eq!(applied, 1);
    assert_eq!(c.rotation, 0);
    // Ten refused turns are fine, the eleventh breaks the crank.
    for _ in 0..10 {
        assert_eq!(c.turn(false), CrankUse::Refused);
    }
    assert_eq!(c.turn(false), CrankUse::Break);
    // 10 turns fill a charger (1600 AE).
    assert_eq!(config::CRANK_POWER_PER_TURN * 10.0, config::CHARGER_BUFFER);
}

#[test]
fn interface_plans_like_interface_logic() {
    let a = k("a");
    let b = k("b");
    assert_eq!(interface_plan(Some((&a, 16)), None), Some((a.clone(), 16)));
    assert_eq!(interface_plan(Some((&a, 16)), Some((&a, 10))), Some((a.clone(), 6)));
    assert_eq!(interface_plan(Some((&a, 16)), Some((&a, 20))), Some((a.clone(), -4)));
    assert_eq!(interface_plan(Some((&a, 16)), Some((&a, 16))), None);
    assert_eq!(interface_plan(Some((&a, 16)), Some((&b, 3))), Some((b.clone(), -3)));
    assert_eq!(interface_plan(None, Some((&b, 3))), Some((b, -3)));
    assert_eq!(interface_plan(None, None), None);
}

// ---------------------------------------------------------------- terminal

fn entry(key: &str, name: &str, amount: u64, mod_id: &str) -> Entry {
    Entry {
        key: k(key),
        amount,
        name: name.into(),
        id: format!("{mod_id}:{}", name.to_lowercase().replace(' ', "_")),
        mod_id: mod_id.into(),
        mod_name: if mod_id == "minecraft" { "Minecraft".into() } else { "Applied Energistics 2".into() },
        tooltip: format!("{name}\nSome lore"),
        tags: vec![format!("{mod_id}:things")],
    }
}

#[test]
fn terminal_sorts_like_key_sorters() {
    let es = vec![
        entry("1", "stone", 5, "minecraft"),
        entry("2", "Apple", 50, "minecraft"),
        entry("3", "Certus Quartz Crystal", 12, "ae2"),
        entry("4", "birch Log", 0, "minecraft"),
    ];
    let names = |v: Vec<Entry>| v.into_iter().map(|e| e.name).collect::<Vec<_>>();
    // Empty entries are not shown; names ignore case.
    assert_eq!(names(view(&es, "", SortBy::Name, SortDir::Ascending)), ["Apple", "Certus Quartz Crystal", "stone"]);
    assert_eq!(names(view(&es, "", SortBy::Name, SortDir::Descending)), ["stone", "Certus Quartz Crystal", "Apple"]);
    assert_eq!(names(view(&es, "", SortBy::Amount, SortDir::Descending)), ["Apple", "Certus Quartz Crystal", "stone"]);
    assert_eq!(names(view(&es, "", SortBy::Amount, SortDir::Ascending)), ["stone", "Certus Quartz Crystal", "Apple"]);
    assert_eq!(names(view(&es, "", SortBy::Mod, SortDir::Ascending)), ["Certus Quartz Crystal", "Apple", "stone"]);
    assert_eq!(SortBy::Name.next(), SortBy::Amount);
    assert_eq!(SortBy::from_code(SortBy::Mod.code()), SortBy::Mod);
}

#[test]
fn terminal_search_like_repo_search() {
    let es = vec![
        entry("1", "Stone", 5, "minecraft"),
        entry("2", "Certus Quartz Crystal", 12, "ae2"),
        entry("3", "Quartz Block", 1, "minecraft"),
    ];
    let hits = |q: &str| view(&es, q, SortBy::Name, SortDir::Ascending).into_iter().map(|e| e.key).collect::<Vec<_>>();
    assert_eq!(hits("quartz"), ["2", "3"]);
    assert_eq!(hits("QUARTZ crys"), ["2"]);
    assert_eq!(hits("@ae2"), ["2"]);
    assert_eq!(hits("@applied"), ["2"]);
    assert_eq!(hits("stone | crystal"), ["2", "1"]);
    assert_eq!(hits("#somelore"), ["2", "3", "1"]);
    assert_eq!(hits("*minecraft:quartz"), ["3"]);
    assert_eq!(hits("$ae2"), ["2"]);
    assert_eq!(hits("nothing"), Vec::<String>::new());
}

#[test]
fn slot_amounts_like_readable_number_converter() {
    assert_eq!(format_amount(0, 4), "0");
    assert_eq!(format_amount(1234, 4), "1234");
    assert_eq!(format_amount(12_345, 4), "12K");
    assert_eq!(format_amount(1_500_000, 4), "1.5M");
    assert_eq!(format_amount(999_999, 4), "999K");
    assert_eq!(format_amount(10_500, 4), "10K");
    assert_eq!(format_amount(2_500_000_000, 4), "2.5G");
    assert_eq!(format_amount(1_000_000, 4), "1M");
    assert_eq!(format_amount(12_345, 3), "12K");
}

#[test]
fn small_terminal_rows() {
    // 360 GUI px high: (360 - 50 - 17 - 97) / 18 = 10 possible rows, SMALL = 10 / 4 = 2.
    assert_eq!(terminal_rows(360, 17, 97), 2);
    assert_eq!(terminal_rows(1000, 17, 97), 11);
    assert_eq!(terminal_rows(100, 17, 97), 2);
}

// ---------------------------------------------------------------- world rules

#[test]
fn budding_quartz_grows_and_decays() {
    // Only one roll in five grows.
    assert_eq!(budding_tick("ae2:flawed_budding_quartz", 3, true, None, 0), None);
    assert_eq!(budding_tick("ae2:flawed_budding_quartz", 0, true, None, 5), Some((0, None)));
    assert_eq!(budding_tick("ae2:flawed_budding_quartz", 0, false, Some(1), 5), Some((2, None)));
    assert_eq!(budding_tick("ae2:flawed_budding_quartz", 0, false, Some(2), 0), Some((3, Some("ae2:chipped_budding_quartz"))));
    assert_eq!(budding_tick("ae2:damaged_budding_quartz", 0, true, None, 0), Some((0, Some("ae2:quartz_block"))));
    // Flawless never decays; a full cluster or another block stops growth.
    assert_eq!(budding_tick("ae2:flawless_budding_quartz", 0, true, None, 0), Some((0, None)));
    assert_eq!(budding_tick("ae2:flawless_budding_quartz", 0, false, Some(3), 0), None);
    assert_eq!(budding_tick("ae2:flawless_budding_quartz", 0, false, None, 0), None);
}

#[test]
fn fluix_forms_in_water() {
    let items = vec![
        (k("minecraft:quartz"), 1, 0),
        (k("ae2:charged_certus_quartz_crystal"), 3, 1),
        (k("minecraft:redstone"), 2, 2),
    ];
    let (used, result, n) = find_transform(&items).unwrap();
    assert_eq!((result, n), ("ae2:fluix_crystal", 2));
    assert_eq!(used, vec![1, 2, 0]);
    let items = vec![(k("ae2:charged_certus_quartz_crystal"), 1, 7), (k("ae2:certus_quartz_dust"), 1, 9)];
    assert_eq!(find_transform(&items).unwrap().1, "ae2:certus_quartz_crystal");
    assert!(find_transform(&[(k("minecraft:redstone"), 5, 0)]).is_none());
}

#[test]
fn meteorites_are_deterministic_and_hold_the_cube() {
    let m = meteor_of_region(3, -2);
    assert_eq!(m, meteor_of_region(3, -2));
    assert!((2.0..8.0).contains(&m.radius));
    assert_eq!(meteor_in_chunk(m.chunk.0, m.chunk.1), Some(m));
    assert!(m.chunk.0.rem_euclid(32) < 24 && m.chunk.1.rem_euclid(32) < 24);
    assert_eq!(meteor_block(5.0, 0, 0, 0), MeteorBlock::Cube);
    assert_eq!(meteor_block(5.0, 1, -1, 0), MeteorBlock::Quartz);
    assert_eq!(meteor_block(5.0, 1, 1, 1), MeteorBlock::Air);
    assert_eq!(meteor_block(5.0, 3, 0, 0), MeteorBlock::SkyStone);
    assert_eq!(meteor_block(5.0, 0, 30, 0), MeteorBlock::Air, "crater above");
    assert_eq!(meteor_block(5.0, 0, -7, 0), MeteorBlock::Keep);
    // One meteorite per 32x32 chunk region at most.
    let count = (0..32).flat_map(|x| (0..32).map(move |z| (x, z))).filter(|&(x, z)| meteor_in_chunk(x, z).is_some()).count();
    assert_eq!(count, 1);
}

// ---------------------------------------------------------------- save / load

#[test]
fn world_state_round_trips() {
    let mut s = State::default();
    let mut drive = Device::new("ae2:drive", 10, 0);
    drive.facing = Some(Dir::North);
    drive.energy = 12.345_678_9;
    drive.slots[3] = Some((k("0a1b"), 1));
    drive.cells.insert(3, CellInv { items: vec![(k("aa"), 8128), (k("bb"), 1)] });
    drive.set_num("priority", -4);
    s.devices.insert([1, -60, 3], drive);
    let mut iface = Device::new("ae2:interface", 9, 9);
    iface.config[0] = Some((k("cc"), 64));
    iface.slots[0] = Some((k("cc"), 12));
    iface.set_fl("buffer", 0.1 + 0.2);
    s.devices.insert([-100, 70, 2_000_000], iface);
    s.drops.push(([5, 6, 7], k("dd"), 3));
    let text = s.save();
    assert_eq!(State::load(Some(text.clone())), s);
    // A damaged file loses only its broken lines.
    let broken = format!("garbage\n{text}S 99 xx 5\nD 1 2\n");
    let back = State::load(Some(broken));
    assert_eq!(back.devices.len(), 2);
    assert_eq!(back.drops.len(), 1);
    assert_eq!(State::load(None), State::default());
}

#[test]
fn a_gone_block_gives_back_every_item() {
    let mut d = Device::new("ae2:inscriber", 4, 0);
    d.slots[0] = Some((k("press"), 1));
    d.slots[2] = Some((k("gold"), 7));
    let items = d.take_items();
    assert_eq!(items, vec![(0, k("press"), 1), (2, k("gold"), 7)]);
    assert!(d.slots.iter().all(Option::is_none));
}

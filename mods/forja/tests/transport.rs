// Moving energy, fluids and items between blocks of several "mods" through the Grid.
mod common;
use common::*;
use horizon_forja::*;

fn cell(capacity: i32, energy: i32, rate: i32) -> EnergyStorage {
    let mut s = EnergyStorage::new(capacity);
    s.max_receive = rate;
    s.max_extract = rate;
    s.energy = energy;
    s
}

#[test]
fn neighbours_of_several_mods_are_found() {
    let mut g = Fake::default();
    let gen_spec = Spec::new("powah:furnator").energy(10_000, 0, 500);
    g.machine([0, 0, 0], &gen_spec);
    g.cells.insert([1, 0, 0], cell(1000, 0, 100)); // another mod's provider
    g.machine([0, 1, 0], &Spec::new("thermal:machine_furnace").energy(4000, 100, 0));
    g.tanks.insert([0, 0, 1], FluidTank::new(1000)); // a tank only, no energy
    let mut n = energy_neighbours(&mut g, [0, 0, 0]);
    n.sort();
    assert_eq!(n, vec![Dir::Up, Dir::East]);
}

#[test]
fn transfer_conserves_and_respects_rates() {
    let mut g = Fake::default();
    g.cells.insert([0, 0, 0], cell(10_000, 10_000, 1000));
    g.machine([1, 0, 0], &Spec::new("test:sink").energy(5000, 300, 0));
    let total = g.total_energy();
    assert_eq!(transfer_energy(&mut g, ([0, 0, 0], Some(Dir::East)), ([1, 0, 0], Some(Dir::West)), 5000), 300);
    assert_eq!(g.total_energy(), total);
    // The sink cannot give back (max_extract 0).
    assert_eq!(transfer_energy(&mut g, ([1, 0, 0], Some(Dir::West)), ([0, 0, 0], Some(Dir::East)), 5000), 0);
}

#[test]
fn per_side_rules_hold_through_the_grid() {
    let mut g = Fake::default();
    let mut sides = Sides::all(Access::None);
    sides.set(Dir::Down, Access::In);
    g.machine([0, 1, 0], &Spec::new("test:top_fed").energy(1000, 1000, 1000).sides(sides, Sides::default(), Sides::default()));
    g.cells.insert([0, 0, 0], cell(1000, 1000, 1000));
    g.cells.insert([1, 1, 0], cell(1000, 1000, 1000));
    assert_eq!(transfer_energy(&mut g, ([1, 1, 0], Some(Dir::West)), ([0, 1, 0], Some(Dir::East)), 100), 0, "closed side");
    assert_eq!(transfer_energy(&mut g, ([0, 0, 0], Some(Dir::Up)), ([0, 1, 0], Some(Dir::Down)), 100), 100, "the input side");
    assert_eq!(transfer_energy(&mut g, ([0, 1, 0], Some(Dir::Down)), ([0, 0, 0], Some(Dir::Up)), 100), 0, "input only");
}

#[test]
fn a_generator_pushes_around_evenly_and_conserves() {
    let mut g = Fake::default();
    let p = [0, 0, 0];
    g.machine(p, &Spec::new("test:gen").energy(100_000, 0, 1_000_000));
    g.m(p).energy.as_mut().unwrap().generate(100_000);
    g.cells.insert([1, 0, 0], cell(100_000, 0, 1_000_000));
    g.cells.insert([-1, 0, 0], cell(100_000, 0, 1_000_000));
    g.cells.insert([0, 0, 1], cell(100, 0, 1_000_000)); // nearly full receiver
    let total = g.total_energy();
    let moved = push_energy_around(&mut g, p, 900);
    assert_eq!(moved, 900);
    assert_eq!(g.total_energy(), total);
    assert_eq!(g.cells[&[0, 0, 1]].energy, 100, "the small one takes what it can");
    let a = g.cells[&[1, 0, 0]].energy;
    let b = g.cells[&[-1, 0, 0]].energy;
    assert_eq!(a + b, 800);
    assert!((a - b).abs() <= 1, "an even split: {a} {b}");
}

#[test]
fn saturation_stops_the_flow() {
    let mut g = Fake::default();
    g.machine([0, 0, 0], &Spec::new("test:gen").energy(1000, 0, 1000));
    g.m([0, 0, 0]).energy.as_mut().unwrap().generate(1000);
    g.cells.insert([1, 0, 0], cell(250, 0, 1000));
    let mut moved = 0;
    for _ in 0..10 {
        moved += push_energy_around(&mut g, [0, 0, 0], 100);
    }
    assert_eq!(moved, 250);
    assert_eq!(g.m([0, 0, 0]).energy.unwrap().energy, 750);
}

#[test]
fn fluids_move_between_mods_without_mixing() {
    let mut g = Fake::default();
    let mut t = FluidTank::new(16_000);
    t.fill(&FluidStack::new("minecraft:water", 5000), false);
    g.tanks.insert([0, 0, 0], t);
    g.machine([1, 0, 0], &Spec::new("test:boiler").tank(4000, TankRole::Input, Some(&["minecraft:water"])));
    let before = g.total_fluid("minecraft:water");
    let moved = transfer_fluid(&mut g, ([0, 0, 0], Some(Dir::East)), ([1, 0, 0], Some(Dir::West)), None, 10_000);
    assert_eq!(moved, FluidStack::new("minecraft:water", 4000));
    assert_eq!(g.total_fluid("minecraft:water"), before);
    // The boiler is full; a lava tank cannot feed it either.
    let mut lava = FluidTank::new(1000);
    lava.fill(&FluidStack::new("minecraft:lava", 1000), false);
    g.tanks.insert([2, 0, 0], lava);
    assert!(transfer_fluid(&mut g, ([2, 0, 0], Some(Dir::West)), ([1, 0, 0], Some(Dir::East)), None, 1000).is_empty());
    assert_eq!(g.tanks[&[2, 0, 0]].amount(), 1000);
}

#[test]
fn items_move_from_a_machine_output_into_a_chest() {
    let mut g = Fake::default();
    let spec = Spec::new("test:crusher").slots(&[SlotRole::Input, SlotRole::Output]);
    g.machine([0, 0, 0], &spec);
    g.m([0, 0, 0]).items.slots[0] = Some(st("minecraft:cobblestone", 30));
    g.m([0, 0, 0]).items.slots[1] = Some(st("minecraft:gravel", 50));
    let mut chest = Inventory::new(1);
    chest.slots[0] = Some(st("minecraft:gravel", 40));
    g.chests.insert([0, 1, 0], chest);
    let (n, stray) = transfer_items(&mut g, ([0, 0, 0], Some(Dir::Up)), ([0, 1, 0], Some(Dir::Down)), 64);
    assert!(stray.is_empty());
    assert_eq!(n, 24, "only the output slot, only the room left");
    assert_eq!(g.total_items("minecraft:gravel"), 90);
    assert_eq!(g.m([0, 0, 0]).items.slots[0], Some(st("minecraft:cobblestone", 30)), "inputs stay");
    // And into the machine: only the input slot takes.
    g.chests.insert([5, 0, 0], {
        let mut c = Inventory::new(2);
        c.slots[0] = Some(st("minecraft:cobblestone", 64));
        c
    });
    let (n, _) = transfer_items(&mut g, ([5, 0, 0], None), ([0, 0, 0], Some(Dir::North)), 64);
    assert_eq!(n, 34);
    assert_eq!(g.total_items("minecraft:cobblestone"), 94);
}

#[test]
fn a_broken_machine_drops_its_items_into_the_count() {
    let mut g = Fake::default();
    g.machine([0, 0, 0], &Spec::new("test:crusher").slots(&[SlotRole::Input]));
    g.m([0, 0, 0]).items.slots[0] = Some(st("minecraft:cobblestone", 30));
    let before = g.total_items("minecraft:cobblestone");
    g.blocks.remove(&[0, 0, 0]);
    g.machines.remove([0, 0, 0]);
    assert_eq!(g.total_items("minecraft:cobblestone"), before, "waiting in the drops");
    assert_eq!(energy_info(&mut g, [0, 0, 0], None), None, "the capability is gone");
}

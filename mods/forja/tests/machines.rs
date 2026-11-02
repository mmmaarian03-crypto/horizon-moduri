// Machine state: specs, removal drops, spec changes, the side file round trip.
mod common;
use common::*;
use horizon_forja::*;

fn furnace() -> Spec {
    Spec::new("test:electric_furnace")
        .energy(20_000, 200, 0)
        .slots(&[SlotRole::Input, SlotRole::Output, SlotRole::Internal])
}

#[test]
fn a_spec_makes_an_empty_machine() {
    let m: Machine<St> = furnace().create();
    assert_eq!(m.energy, Some(EnergyStorage::with_rates(20_000, 200, 0)));
    assert_eq!(m.items.slots.len(), 3);
    assert!(m.tanks.is_empty());
    assert!(m.dirty, "a new machine is saved");
}

#[test]
fn removal_drops_every_item() {
    let mut t: Machines<St> = Machines::default();
    let p = [1, 64, 1];
    let m = t.ensure(p, &furnace());
    m.items.slots[0] = Some(st("minecraft:iron_ore", 12));
    m.items.slots[1] = Some(st("minecraft:iron_ingot", 3));
    m.energy.as_mut().unwrap().energy = 500;
    assert_eq!(t.remove(p), 2);
    assert!(t.map.is_empty());
    let drops = t.take_drops();
    assert_eq!(drops, vec![(p, st("minecraft:iron_ore", 12)), (p, st("minecraft:iron_ingot", 3))]);
    assert!(t.take_drops().is_empty());
    assert_eq!(t.remove(p), 0, "removing twice drops nothing more");
}

#[test]
fn a_replaced_block_drops_the_old_machine_items() {
    let mut t: Machines<St> = Machines::default();
    let p = [0, 0, 0];
    t.ensure(p, &furnace()).items.slots[0] = Some(st("minecraft:coal", 5));
    let other = Spec::new("test:crusher").energy(1000, 100, 0).slots(&[SlotRole::Input]);
    let m = t.ensure(p, &other);
    assert_eq!(m.kind, "test:crusher");
    assert_eq!(m.items.slots, vec![None]);
    assert_eq!(t.take_drops(), vec![(p, st("minecraft:coal", 5))]);
}

#[test]
fn a_changed_spec_keeps_what_fits_and_drops_the_rest() {
    let mut t: Machines<St> = Machines::default();
    let p = [0, 0, 0];
    {
        let m = t.ensure(p, &furnace());
        m.items.slots[2] = Some(st("test:speed_upgrade", 1));
        m.energy.as_mut().unwrap().energy = 15_000;
    }
    let smaller = Spec::new("test:electric_furnace")
        .energy(10_000, 100, 0)
        .slots(&[SlotRole::Input, SlotRole::Output]);
    let m = t.ensure(p, &smaller);
    assert_eq!(m.energy.unwrap().energy, 10_000, "clamped to the new capacity");
    assert_eq!(m.energy.unwrap().max_receive, 100);
    assert_eq!(m.items.slots.len(), 2);
    assert_eq!(t.take_drops(), vec![(p, st("test:speed_upgrade", 1))]);
}

#[test]
fn data_and_dirty_flags() {
    let mut t: Machines<St> = Machines::default();
    t.ensure([0, 0, 0], &furnace());
    assert!(t.dirty());
    t.clean();
    assert!(!t.dirty());
    let m = t.map.get_mut(&[0, 0, 0]).unwrap();
    m.set("burn", 0);
    assert!(m.dirty, "a new key is a change");
    m.dirty = false;
    m.set("burn", 0);
    assert!(!m.dirty, "the same value is not");
    assert_eq!(m.get("missing"), 0);
}

#[test]
fn save_and_load_round_trip() {
    let mut t: Machines<St> = Machines::default();
    let mut sides = Sides::all(Access::In);
    sides.set(Dir::Up, Access::None);
    sides.set(Dir::East, Access::Out);
    let spec = Spec::new("test:fluid_machine")
        .energy(50_000, 1000, 500)
        .tank(8000, TankRole::Input, Some(&["minecraft:water", "minecraft:lava"]))
        .tank(4000, TankRole::Output, None)
        .slots(&[SlotRole::Input, SlotRole::Both, SlotRole::Output, SlotRole::Internal])
        .sides(sides, Sides::all(Access::Both), Sides::default());
    {
        let m = t.ensure([10, -5, 300], &spec);
        m.energy.as_mut().unwrap().energy = 12_345;
        m.tanks[0].fill(&FluidStack::new("minecraft:lava", 7000), false);
        m.tanks[1].fill(&FluidStack::new("mekanism:steam", 99), false);
        m.items.slots[0] = Some(st("minecraft:cobblestone", 64));
        let mut named = st("minecraft:diamond_pickaxe", 1);
        named.tag = Some("{Damage:3}".into());
        m.items.slots[3] = Some(named);
        m.progress = 37;
        m.max_progress = 200;
        m.set("redstone_mode", 2);
        m.set("burn", -7);
    }
    t.ensure([0, 0, 0], &furnace());
    t.drops.push(([4, 5, 6], st("minecraft:dirt", 3)));
    let text = encode(&t, &Codec);
    let mut back = decode(&text, &Codec);
    let mut expected = t.clone();
    expected.clean();
    back.clean();
    assert_eq!(back, expected, "{text}");
    // Saving the loaded table again gives the same file.
    assert_eq!(encode(&back, &Codec), text);
}

#[test]
fn a_loaded_machine_keeps_its_contents_against_its_spec() {
    let mut t: Machines<St> = Machines::default();
    t.ensure([0, 0, 0], &furnace()).items.slots[1] = Some(st("minecraft:iron_ingot", 9));
    let mut back = decode(&encode(&t, &Codec), &Codec);
    let m = back.ensure([0, 0, 0], &furnace());
    assert_eq!(m.items.slots[1], Some(st("minecraft:iron_ingot", 9)));
    assert!(back.take_drops().is_empty());
}

#[test]
fn a_damaged_file_loses_only_its_damaged_lines() {
    let text = "forja 1\n\
                m 1 2 3 test:electric_furnace\n\
                e 500 20000 200 0\n\
                e x y\n\
                i 3 iob\n\
                s 0 minecraft:coal*5*\n\
                s 1 garbage\n\
                s 9999999 minecraft:coal*1*\n\
                t notanumber\n\
                p 1\n\
                d onlykey\n\
                m 1 2\n\
                s 0 minecraft:dirt*1*\n\
                sides bbbbbb\n\
                x 1 2 3 minecraft:stone*2*\n";
    let t: Machines<St> = decode(text, &Codec);
    assert_eq!(t.map.len(), 1);
    let m = &t.map[&[1, 2, 3]];
    assert_eq!(m.energy.unwrap().energy, 500);
    assert_eq!(m.items.slots[0], Some(st("minecraft:coal", 5)));
    assert_eq!(m.items.slots.len(), 3);
    assert_eq!(t.drops, vec![([1, 2, 3], st("minecraft:stone", 2))]);
    assert!(decode::<St>("", &Codec).map.is_empty());
}

#[test]
fn stored_energy_is_clamped_on_load() {
    let t: Machines<St> = decode("m 0 0 0 a:b\ne 999999 100 10 10\nt 50 b 80 minecraft:water\n", &Codec);
    let m = &t.map[&[0, 0, 0]];
    assert_eq!(m.energy.unwrap().energy, 100);
    assert_eq!(m.tanks[0].amount(), 50);
}

#[test]
fn an_own_capacity_survives_conform_and_save() {
    // Mekanism energy upgrades: the storage grows past the spec's and stays after a reload.
    let mut t: Machines<St> = Machines::default();
    let p = [3, 64, 3];
    let m = t.ensure(p, &furnace());
    m.set_energy_capacity(80_000);
    m.energy.as_mut().unwrap().energy = 70_000;
    let m = t.ensure(p, &furnace());
    assert_eq!(m.energy.unwrap().capacity, 80_000);
    assert_eq!(m.energy.unwrap().energy, 70_000);
    let text = encode(&t, &Codec);
    let mut back: Machines<St> = decode(&text, &Codec);
    let m = back.ensure(p, &furnace());
    assert_eq!((m.energy.unwrap().capacity, m.energy.unwrap().energy), (80_000, 70_000));
    // Upgrades out: back to the spec, the excess energy is clamped.
    m.set_energy_capacity(0);
    let m = back.ensure(p, &furnace());
    assert_eq!((m.energy.unwrap().capacity, m.energy.unwrap().energy), (20_000, 20_000));
}

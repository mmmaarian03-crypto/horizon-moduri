// Fluid rules: FluidTank, mixing, roles, transfer, buckets.
mod common;
use common::*;
use horizon_foundry::*;

fn water(n: i32) -> FluidStack {
    FluidStack::new("minecraft:water", n)
}
fn lava(n: i32) -> FluidStack {
    FluidStack::new("minecraft:lava", n)
}

#[test]
fn fill_and_drain_like_forge_fluid_tank() {
    let mut t = FluidTank::new(4000);
    assert_eq!(t.fill(&water(1500), false), 1500);
    assert_eq!(t.fill(&water(3000), false), 2500, "only the room left");
    assert_eq!(t.fill(&water(1), false), 0, "full");
    assert_eq!(t.drain_max(1000, false), water(1000));
    assert_eq!(t.drain(&water(500), false), water(500));
    assert_eq!(t.amount(), 2500);
    assert_eq!(t.drain_max(10_000, false), water(2500));
    assert!(t.fluid.is_empty(), "an emptied tank forgets its fluid");
    assert!(t.drain_max(10, false).is_empty());
}

#[test]
fn mixing_is_refused() {
    let mut t = FluidTank::new(4000);
    t.fill(&water(1000), false);
    assert_eq!(t.fill(&lava(1000), false), 0);
    assert_eq!(t.fill(&lava(1000), true), 0);
    assert!(t.drain(&lava(1000), false).is_empty(), "draining another fluid gives nothing");
    assert_eq!(t.fluid, water(1000));
    // Emptied, it takes the other fluid.
    t.drain_max(1000, false);
    assert_eq!(t.fill(&lava(300), false), 300);
}

#[test]
fn simulate_vs_execute() {
    let mut t = FluidTank::new(1000);
    assert_eq!(t.fill(&water(700), true), 700);
    assert!(t.fluid.is_empty(), "a simulated fill leaves an empty tank empty");
    t.fill(&water(700), false);
    assert_eq!(t.fill(&water(700), true), 300);
    assert_eq!(t.drain_max(5000, true), water(700));
    assert_eq!(t.amount(), 700);
}

#[test]
fn empty_stacks_and_validators() {
    let mut t = FluidTank::only(1000, &["minecraft:lava"]);
    assert_eq!(t.fill(&water(100), false), 0);
    assert_eq!(t.fill(&lava(100), false), 100);
    assert_eq!(t.fill(&FluidStack::new("minecraft:empty", 100), false), 0);
    assert_eq!(t.fill(&lava(0), false), 0);
    assert!(FluidStack::empty().is_empty());
}

#[test]
fn tank_roles_seen_from_outside_and_inside() {
    let spec = Spec::new("test:boiler")
        .tank(8000, TankRole::Input, Some(&["minecraft:water"]))
        .tank(8000, TankRole::Output, None);
    let mut m: Machine<St> = spec.create();
    {
        let mut out = m.fluid_handler(Some(Dir::North)).unwrap();
        assert_eq!(out.fill(&water(5000), false), 5000, "the input tank takes water");
        assert_eq!(out.fill(&lava(5000), false), 0, "nothing fills the output tank from outside");
        assert!(out.drain_max(1000, false).is_empty(), "the input tank cannot be drained from outside");
    }
    {
        let mut inside = m.fluid_handler(None).unwrap();
        // The machine turns water into steam in its output tank.
        assert_eq!(inside.drain(&water(1000), false), water(1000));
        assert_eq!(inside.fill(&FluidStack::new("mekanism:steam", 1000), false), 1000);
    }
    let mut out = m.fluid_handler(Some(Dir::Up)).unwrap();
    assert_eq!(out.drain_max(5000, false), FluidStack::new("mekanism:steam", 1000));
}

#[test]
fn multi_tank_fill_prefers_the_tank_holding_the_fluid() {
    let mut tanks = vec![FluidTank::new(1000), FluidTank::new(1000), FluidTank::new(1000)];
    tanks[1].fill(&water(200), false);
    let roles = [TankRole::Both; 3];
    let mut set = TankSet { tanks: &mut tanks, roles: &roles, access: Access::Both, internal: false };
    let sim = set.fill(&water(1500), true);
    assert_eq!(set.fill(&water(1500), false), sim, "simulation equals execution");
    assert_eq!(tanks[1].amount(), 1000, "the water tank first");
    assert_eq!(tanks[0].amount(), 700, "then the first empty one");
    assert!(tanks[2].fluid.is_empty());
}

#[test]
fn side_access_on_fluids() {
    let mut sides = Sides::all(Access::None);
    sides.set(Dir::Down, Access::Out);
    sides.set(Dir::Up, Access::In);
    let spec = Spec::new("test:tank")
        .tank(4000, TankRole::Both, None)
        .sides(Sides::default(), sides, Sides::default());
    let mut m: Machine<St> = spec.create();
    assert!(m.fluid_handler(Some(Dir::East)).is_none());
    assert_eq!(m.fluid_handler(Some(Dir::Down)).unwrap().fill(&water(10), false), 0);
    assert_eq!(m.fluid_handler(Some(Dir::Up)).unwrap().fill(&water(10), false), 10);
    assert!(m.fluid_handler(Some(Dir::Up)).unwrap().drain_max(10, false).is_empty());
    assert_eq!(m.fluid_handler(Some(Dir::Down)).unwrap().drain_max(10, false), water(10));
}

#[test]
fn try_fluid_transfer_conserves_and_refuses_mixing() {
    let mut a = FluidTank::new(10_000);
    a.fill(&water(3000), false);
    let mut b = FluidTank::new(2000);
    let moved = try_fluid_transfer(&mut b, &mut a, None, 5000);
    assert_eq!(moved, water(2000));
    assert_eq!(a.amount() + b.amount(), 3000);
    let mut c = FluidTank::new(2000);
    c.fill(&lava(1), false);
    assert!(try_fluid_transfer(&mut c, &mut a, None, 5000).is_empty());
    assert_eq!(a.amount(), 1000);
    assert!(try_fluid_transfer(&mut c, &mut a, Some(&lava(10)), 5000).is_empty(), "a has no lava");
}

#[test]
fn buckets_move_exactly_one_bucket() {
    let mut t = FluidTank::new(16_000);
    assert_eq!(
        bucket_interact("minecraft:water_bucket", &mut t, &[]),
        BucketUse::Emptied { sound: "minecraft:item.bucket.empty" }
    );
    assert_eq!(t.amount(), 1000);
    assert_eq!(
        bucket_interact(EMPTY_BUCKET, &mut t, &[]),
        BucketUse::Filled { bucket: "minecraft:water_bucket".into(), sound: "minecraft:item.bucket.fill" }
    );
    assert!(t.fluid.is_empty());
    t.fill(&water(999), false);
    assert_eq!(bucket_interact(EMPTY_BUCKET, &mut t, &[]), BucketUse::Nothing, "999 mB do not fill a bucket");
    assert_eq!(bucket_interact("minecraft:lava_bucket", &mut t, &[]), BucketUse::Nothing, "no mixing");
    t.fill(&water(16_000), false);
    assert_eq!(
        bucket_interact("minecraft:water_bucket", &mut t, &[]),
        BucketUse::Nothing,
        "the whole bucket must fit"
    );
    assert_eq!(t.amount(), 16_000);
}

#[test]
fn lava_milk_and_mod_buckets() {
    let mut t = FluidTank::new(4000);
    assert_eq!(
        bucket_interact("minecraft:lava_bucket", &mut t, &[]),
        BucketUse::Emptied { sound: "minecraft:item.bucket.empty_lava" }
    );
    assert_eq!(
        bucket_interact(EMPTY_BUCKET, &mut t, &[]),
        BucketUse::Filled { bucket: "minecraft:lava_bucket".into(), sound: "minecraft:item.bucket.fill_lava" }
    );
    let extra = vec![("thermal:redstone_bucket".to_string(), "thermal:redstone".to_string())];
    assert_eq!(
        bucket_interact("thermal:redstone_bucket", &mut t, &extra),
        BucketUse::Emptied { sound: "minecraft:item.bucket.empty" }
    );
    assert_eq!(bucket_interact(EMPTY_BUCKET, &mut t, &[]), BucketUse::Nothing, "no bucket known without the mod's entry");
    assert_eq!(
        bucket_interact(EMPTY_BUCKET, &mut t, &extra),
        BucketUse::Filled { bucket: "thermal:redstone_bucket".into(), sound: "minecraft:item.bucket.fill" }
    );
    assert_eq!(
        bucket_interact("minecraft:milk_bucket", &mut t, &[]),
        BucketUse::Emptied { sound: "minecraft:item.bucket.empty" }
    );
    assert_eq!(t.fluid, FluidStack::new("minecraft:milk", 1000));
}

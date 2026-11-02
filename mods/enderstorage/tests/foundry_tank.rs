// The Ender Tank reached through foundry: the frequency's `Liquid` copied into a foundry `FluidTank`
// of 16 buckets, one operation done on it, and copied back (native/horizon.rs `es_tank_fluid`).
// These tests run the same operations on both and on the round trip.
use horizon_enderstorage::{Liquid, TANK_CAPACITY};
use horizon_foundry::{FluidHandler, FluidStack, FluidTank};

fn to_foundry(l: &Liquid) -> FluidTank {
    let mut t = FluidTank::new(TANK_CAPACITY as i32);
    if let Some(f) = &l.fluid
        && l.amount > 0
    {
        t.fluid = FluidStack::new(f, l.amount as i32);
    }
    t
}
fn from_foundry(t: &FluidTank) -> Liquid {
    if t.fluid.is_empty() {
        Liquid::default()
    } else {
        Liquid { fluid: Some(t.fluid.fluid.clone()), amount: t.fluid.amount as u32 }
    }
}

#[test]
fn the_tank_through_foundry_behaves_as_the_ender_liquid_storage() {
    let fluids = ["minecraft:water", "minecraft:lava", "minecraft:milk"];
    let mut seed = 0x2545_f491u64;
    let mut next = |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    let mut liquid = Liquid::default();
    for _ in 0..4000 {
        let fluid = fluids[next(3) as usize];
        let amount = next(5000) as u32;
        let mut tank = to_foundry(&liquid);
        if next(2) == 0 {
            let a = liquid.fill(fluid, amount);
            let b = FluidHandler::fill(&mut tank, &FluidStack::new(fluid, amount as i32), false);
            assert_eq!(a as i32, b, "fill {fluid} {amount}");
        } else {
            let a = liquid.drain(amount).map_or(0, |d| d.1);
            let b = FluidHandler::drain_max(&mut tank, amount as i32, false).amount.max(0);
            assert_eq!(a as i32, b, "drain {amount}");
        }
        assert_eq!(from_foundry(&tank), liquid, "the copy back equals the storage");
        assert!(liquid.amount <= TANK_CAPACITY);
    }
}

#[test]
fn the_tank_refuses_another_fluid_through_foundry() {
    let mut liquid = Liquid::default();
    liquid.fill("minecraft:water", 3000);
    let mut tank = to_foundry(&liquid);
    assert_eq!(FluidHandler::fill(&mut tank, &FluidStack::new("minecraft:lava", 1000), false), 0);
    assert!(FluidHandler::drain(&mut tank, &FluidStack::new("minecraft:lava", 1000), false).is_empty());
    assert_eq!(from_foundry(&tank), liquid);
    assert_eq!(FluidHandler::tank_capacity(&tank, 0), 16_000);
}

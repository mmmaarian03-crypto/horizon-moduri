// Forge Energy rules: EnergyStorage, sides, push, conservation.
mod common;
use common::*;
use horizon_foundry::*;

#[test]
fn receive_and_extract_follow_forge_energy_storage() {
    let mut s = EnergyStorage::with_rates(1000, 100, 40);
    assert_eq!(s.receive_energy(250, false), 100, "max_receive limits one call");
    assert_eq!(s.energy, 100);
    assert_eq!(s.extract_energy(250, false), 40, "max_extract limits one call");
    assert_eq!(s.energy, 60);
    assert_eq!(s.extract_energy(10, false), 10);
    assert_eq!(s.energy_stored(), 50);
    assert_eq!(s.max_energy_stored(), 1000);
}

#[test]
fn simulate_changes_nothing() {
    let mut s = EnergyStorage::new(500);
    s.energy = 200;
    assert_eq!(s.receive_energy(1000, true), 300);
    assert_eq!(s.extract_energy(1000, true), 200);
    assert_eq!(s.energy, 200);
    // The simulation tells exactly what the execution does.
    let sim = s.receive_energy(123, true);
    assert_eq!(s.receive_energy(123, false), sim);
}

#[test]
fn saturation_full_and_empty() {
    let mut s = EnergyStorage::new(100);
    assert_eq!(s.receive_energy(1000, false), 100);
    assert_eq!(s.receive_energy(1, false), 0, "a full storage takes nothing");
    assert_eq!(s.extract_energy(1000, false), 100);
    assert_eq!(s.extract_energy(1, false), 0, "an empty storage gives nothing");
}

#[test]
fn zero_rates_mean_no_receive_or_no_extract() {
    let mut generator = EnergyStorage::with_rates(1000, 0, 100);
    assert!(!generator.can_receive() && generator.can_extract());
    assert_eq!(generator.receive_energy(10, false), 0);
    assert_eq!(generator.generate(60), 60, "the block itself still fills it");
    let mut consumer = EnergyStorage::with_rates(1000, 100, 0);
    assert!(consumer.can_receive() && !consumer.can_extract());
    consumer.energy = 50;
    assert_eq!(consumer.extract_energy(10, false), 0);
    assert!(consumer.consume(50));
    assert!(!consumer.consume(1), "consume is all or nothing");
}

#[test]
fn negative_requests_never_drain() {
    let mut s = EnergyStorage::new(100);
    s.energy = 50;
    assert_eq!(s.receive_energy(-20, false), 0);
    assert_eq!(s.extract_energy(-20, false), 0);
    assert_eq!(s.energy, 50);
    assert_eq!(s.generate(-5), 0);
}

#[test]
fn generate_stops_at_capacity() {
    let mut s = EnergyStorage::with_rates(100, 0, 10);
    s.energy = 90;
    assert_eq!(s.generate(40), 10);
    assert_eq!(s.energy, 100);
}

#[test]
fn per_side_access() {
    let mut s = EnergyStorage::new(1000);
    s.energy = 500;
    let mut input = SidedEnergy { storage: &mut s, access: Access::In };
    assert!(input.can_receive() && !input.can_extract());
    assert_eq!(input.extract_energy(100, false), 0);
    assert_eq!(input.receive_energy(100, false), 100);
    let mut output = SidedEnergy { storage: &mut s, access: Access::Out };
    assert!(!output.can_receive() && output.can_extract());
    assert_eq!(output.receive_energy(100, false), 0);
    assert_eq!(output.extract_energy(100, false), 100);
    assert_eq!(s.energy, 500);
}

#[test]
fn machine_sides_hide_the_capability() {
    let mut sides = Sides::all(Access::In);
    sides.set(Dir::North, Access::None);
    sides.set(Dir::Up, Access::Out);
    let spec = Spec::new("test:battery")
        .energy(1000, 100, 100)
        .sides(sides, Sides::default(), Sides::default());
    let mut m: Machine<St> = spec.create();
    assert!(m.energy_handler(Some(Dir::North)).is_none(), "no capability on a closed side");
    assert!(m.energy_handler(None).is_some(), "the block itself (null side) always sees it");
    assert!(!m.energy_handler(Some(Dir::Down)).unwrap().can_extract());
    assert!(!m.energy_handler(Some(Dir::Up)).unwrap().can_receive());
}

#[test]
fn relative_sides_follow_the_front() {
    assert_eq!(Dir::North.relative(Dir::North), Rel::Front);
    assert_eq!(Dir::South.relative(Dir::North), Rel::Back);
    assert_eq!(Dir::East.relative(Dir::North), Rel::Left);
    assert_eq!(Dir::West.relative(Dir::North), Rel::Right);
    assert_eq!(Dir::Up.relative(Dir::East), Rel::Top);
    assert_eq!(Dir::South.relative(Dir::East), Rel::Left);
}

#[test]
fn push_moves_what_both_agree_on() {
    let mut a = EnergyStorage::with_rates(1000, 0, 300);
    a.energy = 1000;
    let mut b = EnergyStorage::with_rates(500, 200, 0);
    b.energy = 400;
    assert_eq!(push_energy(&mut a, &mut b, 1000), 100, "b has room for 100 only");
    assert_eq!((a.energy, b.energy), (900, 500));
    assert_eq!(push_energy(&mut a, &mut b, 1000), 0);
}

/// A receiver that promises more than it takes.
struct Liar(EnergyStorage);
impl EnergyHandler for Liar {
    fn receive_energy(&mut self, max: i32, simulate: bool) -> i32 {
        if simulate { max } else { self.0.receive_energy(max / 2, false) }
    }
    fn extract_energy(&mut self, _max: i32, _simulate: bool) -> i32 {
        0
    }
    fn energy_stored(&self) -> i32 {
        self.0.energy
    }
    fn max_energy_stored(&self) -> i32 {
        self.0.capacity
    }
    fn can_extract(&self) -> bool {
        false
    }
    fn can_receive(&self) -> bool {
        true
    }
}

#[test]
fn a_lying_receiver_loses_nothing() {
    let mut a = EnergyStorage::new(1000);
    a.energy = 1000;
    let mut b = Liar(EnergyStorage::new(1000));
    let moved = push_energy(&mut a, &mut b, 400);
    assert_eq!(moved, 200);
    assert_eq!(a.energy + b.0.energy, 1000, "the refused half went back");
}

#[test]
fn random_pushes_conserve_energy() {
    let mut rng = Rng(0x1234_5678);
    let mut cells: Vec<EnergyStorage> = (0..6)
        .map(|i| {
            let mut s = EnergyStorage::with_rates(1000 + i * 700, 50 + i * 37, 80 + i * 11);
            s.energy = (i * 333) % s.capacity;
            s
        })
        .collect();
    let total: i32 = cells.iter().map(|c| c.energy).sum();
    for _ in 0..5000 {
        let i = rng.next(6) as usize;
        let j = rng.next(6) as usize;
        if i == j {
            continue;
        }
        let max = rng.next(3000) as i32 - 100;
        let (a, b) = if i < j {
            let (l, r) = cells.split_at_mut(j);
            (&mut l[i], &mut r[0])
        } else {
            let (l, r) = cells.split_at_mut(i);
            (&mut r[0], &mut l[j])
        };
        let n = push_energy(a, b, max);
        assert!(n >= 0 && n <= max.max(0));
        for c in &cells {
            assert!(c.energy >= 0 && c.energy <= c.capacity);
        }
    }
    assert_eq!(cells.iter().map(|c| c.energy).sum::<i32>(), total);
}

#[test]
fn gauge_scaling_and_texts() {
    let mut s = EnergyStorage::new(10_000);
    s.energy = 2_500;
    assert_eq!(s.scaled(52), 13);
    assert_eq!(scaled(5, 0, 10), 0);
    assert_eq!(scaled(20, 10, 10), 10, "never past the full bar");
    assert_eq!(fe_text(1234, 1_000_000), "1,234 / 1,000,000 FE");
    assert_eq!(mb_text(16000), "16,000 mB");
}

#[test]
fn bars_grow_the_right_way() {
    // The furnace arrow: 24x17 at (79, 34), picture at (176, 14).
    let arrow = Bar { x: 79, y: 34, u: 176, v: 14, w: 24, h: 17, grow: Grow::Right };
    assert_eq!(arrow.part(0, 200), None);
    assert_eq!(arrow.part(100, 200), Some([79, 34, 176, 14, 12, 17]));
    // An energy gauge 14x42 filling from the bottom.
    let gauge = Bar { x: 10, y: 20, u: 176, v: 31, w: 14, h: 42, grow: Grow::Up };
    assert_eq!(gauge.part(1, 2), Some([10, 41, 176, 52, 14, 21]));
    assert_eq!(gauge.part(5, 5), Some([10, 20, 176, 31, 14, 42]));
    let left = Bar { x: 0, y: 0, u: 0, v: 0, w: 10, h: 4, grow: Grow::Left };
    assert_eq!(left.part(3, 10), Some([7, 0, 7, 0, 3, 4]));
    let down = Bar { x: 0, y: 0, u: 0, v: 0, w: 4, h: 10, grow: Grow::Down };
    assert_eq!(down.part(5, 10), Some([0, 0, 0, 0, 4, 5]));
}

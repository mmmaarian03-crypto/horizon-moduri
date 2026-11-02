// Redstone output: what a mod block or part emits per side, which neighbours hear of a change, and
// what Horizon's signal code is told (its only per-position question is the TrappedChest-class
// `block_entity_signal`: weak on every side, strong only downwards).
use horizon_foundry::*;

const P: Pos = [10, 64, -3];

#[test]
fn a_signal_is_per_side_and_capped_at_15() {
    let mut s = Signal::off();
    assert!(s.is_off());
    s.set(Dir::North, 20, 3);
    assert_eq!((s.weak(Dir::North), s.strong(Dir::North)), (15, 3));
    assert_eq!((s.weak(Dir::South), s.strong(Dir::South)), (0, 0));
    assert!(!s.is_off());
    let u = Signal::uniform(7, 0);
    assert!(Dir::ALL.iter().all(|&d| u.weak(d) == 7 && u.strong(d) == 0));
    // Two parts of one host: max per side.
    let a = Signal::side(Dir::Up, 15, 15);
    let b = Signal::side(Dir::East, 4, 0);
    let m = a.max(&b);
    assert_eq!((m.weak(Dir::Up), m.weak(Dir::East), m.strong(Dir::East), m.weak(Dir::West)), (15, 4, 0, 0));
}

#[test]
fn a_change_updates_the_block_and_whatever_its_strong_sides_power_through() {
    let mut r = Redstone::new();
    // AE2's level emitter on the north face turns on: weak and strong 15 out of the north side.
    let on = Signal::side(Dir::North, 15, 15);
    let n = r.set(P, on);
    assert_eq!(n, vec![P, offset(P, Dir::North)], "the host, then the block it powers through");
    assert_eq!(r.get(P), on);
    // The same again: nothing to tell.
    assert!(r.set(P, on).is_empty());
    // Only the weak value changes: only the block's own neighbours.
    let mut weaker = on;
    weaker.weak[Dir::North.idx()] = 9;
    assert_eq!(r.set(P, weaker), vec![P]);
    // Off: both again, and the table forgets it.
    assert_eq!(r.set(P, Signal::off()), vec![P, offset(P, Dir::North)]);
    assert!(r.is_empty());
    assert!(r.clear(P).is_empty());
}

#[test]
fn horizon_hears_weak_on_every_side_and_strong_only_below() {
    // Vanilla's `direction` is from the asking block towards the source: the block north of the
    // source asks with SOUTH, the side asked is NORTH.
    let all = Signal::uniform(12, 15);
    for d in Dir::ALL {
        let asked_from = d.opposite();
        let v = horizon_signal(&all, asked_from);
        if d == Dir::Down {
            assert_eq!(v, 15, "below: the stronger of weak and strong (both queries reach the same call)");
        } else {
            assert_eq!(v, 12, "{d:?}: the weak signal");
        }
    }
    let north = Signal::side(Dir::North, 15, 15);
    assert_eq!(horizon_delivers(&north, Dir::North), (15, 0), "weak only: a lamp or piston there lights, nothing powers through");
    assert_eq!(horizon_delivers(&north, Dir::South), (0, 0));
    let down = Signal::side(Dir::Down, 15, 15);
    assert_eq!(horizon_delivers(&down, Dir::Down), (15, 15), "the block below is powered through");
    // A strong-only side (no weak) still reaches the block below.
    let s = Signal::side(Dir::Down, 0, 8);
    assert_eq!(horizon_signal(&s, Dir::Up), 8);
    assert_eq!(horizon_signal(&s, Dir::Down), 0);
}

#[test]
fn an_emitter_toggling_tells_its_neighbours_every_time() {
    let mut r = Redstone::new();
    let mut heard = 0;
    for tick in 0..20 {
        let on = (tick / 5) % 2 == 0;
        let s = if on { Signal::side(Dir::Down, 15, 15) } else { Signal::off() };
        let n = r.set(P, s);
        if !n.is_empty() {
            heard += 1;
            assert_eq!(n, vec![P, offset(P, Dir::Down)]);
        }
    }
    // on (tick 0), off (5), on (10), off (15).
    assert_eq!(heard, 4);
    assert!(r.is_empty());
    // Several emitters keep their own values.
    let q = [0, 0, 0];
    r.set(P, Signal::uniform(3, 0));
    r.set(q, Signal::side(Dir::West, 15, 0));
    assert_eq!(r.positions(), vec![q, P]);
    assert_eq!(r.get(q).weak(Dir::West), 15);
    assert_eq!(r.get(P).weak(Dir::West), 3);
}

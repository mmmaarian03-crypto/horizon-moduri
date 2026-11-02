use horizon_tlc::*;
use std::collections::BTreeMap;

/// A heightmap: a default height with some columns set, and every read logged.
struct Land {
    base: i32,
    at: BTreeMap<(i32, i32), i32>,
    reads: Vec<(i32, i32)>,
}
impl Land {
    fn flat(base: i32) -> Self {
        Self {
            base,
            at: BTreeMap::new(),
            reads: Vec::new(),
        }
    }
    fn with(mut self, x: i32, z: i32, h: i32) -> Self {
        self.at.insert((x, z), h);
        self
    }
}
impl Terrain for Land {
    fn first_occupied(&mut self, x: i32, z: i32) -> i32 {
        self.reads.push((x, z));
        *self.at.get(&(x, z)).unwrap_or(&self.base)
    }
}

// Chunk (400, 0): its corner is 6400 blocks out, past the 5000 the mod wants.
const FAR: (i32, i32) = (400, 0);

#[test]
fn distance_is_from_the_chunk_corner_truncated() {
    assert_eq!(distance_from_spawn(0, 0), 0);
    assert_eq!(distance_from_spawn(1, 0), 16);
    assert_eq!(distance_from_spawn(-1, 0), 16);
    // 3*16, 4*16 -> 48, 64 -> 80 exactly.
    assert_eq!(distance_from_spawn(3, 4), 80);
    // sqrt(16^2 + 16^2) = 22.627... -> 22.
    assert_eq!(distance_from_spawn(1, 1), 22);
    // Far out chunks do not overflow (doubles, saturating cast).
    assert_eq!(distance_from_spawn(1 << 20, 1 << 20), 23726566);
}

#[test]
fn the_boundary_is_strictly_more_than_5000() {
    // Chunk 312 has its corner at 4992, 313 at 5008.
    let mut land = Land::flat(70);
    assert!(!castle_may_start(&mut land, 312, 0));
    assert!(castle_may_start(&mut land, 313, 0));
    assert!(castle_may_start(&mut land, -313, 0));
    assert!(castle_may_start(&mut land, 0, -313));
    // (221, 221): 3536 x 3536 -> 5000.7 -> 5000, not more.
    assert!(!castle_may_start(&mut land, 221, 221));
    assert!(castle_may_start(&mut land, 222, 221));
}

#[test]
fn reads_the_corner_and_78_blocks_out_in_order() {
    let mut land = Land::flat(70);
    assert!(castle_may_start(&mut land, FAR.0, FAR.1));
    let (x, z) = (FAR.0 * 16, FAR.1 * 16);
    assert_eq!(
        land.reads,
        vec![(x, z), (x + 78, z), (x - 78, z), (x, z + 78), (x, z - 78)]
    );
}

#[test]
fn uneven_land_refuses_the_castle() {
    let (x, z) = (FAR.0 * 16, FAR.1 * 16);
    for (dx, dz) in [(78, 0), (-78, 0), (0, 78), (0, -78)] {
        // 9 blocks of difference is still fine, 10 is not, up or down.
        let mut ok = Land::flat(70).with(x + dx, z + dz, 79);
        assert!(castle_may_start(&mut ok, FAR.0, FAR.1));
        let mut ok = Land::flat(70).with(x + dx, z + dz, 61);
        assert!(castle_may_start(&mut ok, FAR.0, FAR.1));
        let mut high = Land::flat(70).with(x + dx, z + dz, 80);
        assert!(!castle_may_start(&mut high, FAR.0, FAR.1));
        let mut low = Land::flat(70).with(x + dx, z + dz, 60);
        assert!(!castle_may_start(&mut low, FAR.0, FAR.1));
    }
    // Only those four points and the corner count: a cliff between them does not.
    let mut cliff = Land::flat(70).with(x + 40, z, 120).with(x, z + 77, 20);
    assert!(castle_may_start(&mut cliff, FAR.0, FAR.1));
    // The corner itself is compared, not an average.
    let mut corner = Land::flat(70).with(x, z, 90);
    assert!(!castle_may_start(&mut corner, FAR.0, FAR.1));
}

#[test]
fn strongholds_and_other_structures() {
    let mut land = Land::flat(70);
    // the pack's tlc.toml: generate_stronghold = false.
    assert!(!GENERATE_STRONGHOLD);
    assert!(refuses_start(&mut land, "minecraft:stronghold", true, 0, 0));
    // A stronghold of another type (Better Strongholds) is not the mixin's business.
    assert!(!refuses_start(&mut land, "betterstrongholds:stronghold", false, 0, 0));
    // Other structures are never read or refused.
    let mut untouched = Land::flat(70);
    assert!(!refuses_start(&mut untouched, "minecraft:village_plains", false, 0, 0));
    assert!(untouched.reads.is_empty());
    // The castle: refused near the centre, allowed far out on even land.
    assert!(refuses_start(&mut land, STRUCTURE, false, 0, 0));
    assert!(!refuses_start(&mut land, STRUCTURE, false, FAR.0, FAR.1));
}

#[test]
fn map_price_is_15_to_25_each_as_likely() {
    let mut seen = [0u32; 26];
    for roll in 0..11_000u64 {
        let p = map_price(roll.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        assert!((MAP_PRICE_MIN..=MAP_PRICE_MAX).contains(&p), "{p}");
        seen[p as usize] += 1;
    }
    for (p, n) in seen.iter().enumerate().skip(15) {
        assert!((900..=1100).contains(n), "price {p} came {n} times");
    }
    assert_eq!(map_price(0), 15);
    assert_eq!(map_price(10), 25);
    assert_eq!(map_price(u64::MAX), 15 + (u64::MAX % 11) as i32);
}

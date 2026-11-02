// Clumps for Horizon: the rules, without any game code.
// Written from the behaviour of Clumps 12.0.0.4 for Minecraft 1.20.1 (Jaredlll08, MIT,
// https://github.com/jaredlll08/Clumps, branch 1.20.1, read for private use only); the pack has no
// config file for it. No code of the original is used.
//
// What the original does (its `MixinExperienceOrb`):
//  - every experience orb carries a `clumpedMap`: for each orb size (the value of an orb) how many
//    of those orbs it holds. A fresh orb holds `{value: 1}`; its `value` is always the sum of
//    `size * amount` over the map, so a clump of orbs is drawn as one bigger orb.
//  - `canMerge` is "the other orb is alive" (not "same value, ids a multiple of 40 apart"): any two
//    orbs that meet merge, and `ExperienceOrb.award` (`tryMergeToExisting`) adds the new orb to
//    ANY orb in the 1x1x1 box around the spot, resetting its age to 0.
//  - `scanForMerges` (every 20 ticks of an orb's life, from its first tick) merges every orb
//    within half a block of its box into it, keeping the smaller age.
//  - `playerTouch` takes the whole clump at once: no pickup delay (`takeXpDelay = 0`), the orb is
//    taken and discarded, and each orb of the map is run through the repair of the player's items
//    (`repairPlayerItems`, once per orb of that size); what is left over, summed, is given as
//    experience points in one go.
//  - the map is saved as `clumpedMap` (an orb saved without one loads as `{Value: Count}`).
use std::collections::BTreeMap;

/// `clumpedMap`: orb size -> how many orbs of that size.
pub type Clump = BTreeMap<u32, u32>;

/// What `clumps$getClumpedMap` makes of an orb that has none: `{value: 1}`.
pub fn single(value: u32) -> Clump {
    Clump::from([(value, 1)])
}

/// `clumps$resolve`: the orb's value, the sum of `size * amount`.
pub fn total(clump: &Clump) -> u32 {
    clump
        .iter()
        .fold(0u32, |sum, (&size, &amount)| sum.saturating_add(size.saturating_mul(amount)))
}

/// `Collectors.toMap(.., Integer::sum)` of two maps.
pub fn merged(a: &Clump, b: &Clump) -> Clump {
    let mut out = a.clone();
    for (&size, &amount) in b {
        let slot = out.entry(size).or_insert(0);
        *slot = slot.saturating_add(amount);
    }
    out
}

/// An axis-aligned box, as `AABB`: the entity queries use strict comparisons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box3 {
    pub min: [f64; 3],
    pub max: [f64; 3],
}
impl Box3 {
    /// `AABB.ofSize(centre, size, size, size)`.
    pub fn around(centre: [f64; 3], size: f64) -> Self {
        let h = size / 2.0;
        Self {
            min: centre.map(|c| c - h),
            max: centre.map(|c| c + h),
        }
    }
    /// `AABB.inflate(d)`.
    pub fn inflate(self, d: f64) -> Self {
        Self {
            min: self.min.map(|c| c - d),
            max: self.max.map(|c| c + d),
        }
    }
    /// `AABB.intersects(AABB)`.
    pub fn intersects(&self, other: &Self) -> bool {
        (0..3).all(|i| self.min[i] < other.max[i] && self.max[i] > other.min[i])
    }
}

/// An experience orb as the rules see it.
pub trait Orb {
    /// The id the game tracks it by (the key of its clump).
    fn id(&self) -> u32;
    /// `ExperienceOrb.value`.
    fn value(&self) -> u32;
    fn set_value(&mut self, value: u32);
    /// `ExperienceOrb.age`.
    fn age(&self) -> u32;
    fn set_age(&mut self, age: u32);
    /// `Entity.tickCount`: the ticks the orb has been through so far.
    fn ticks(&self) -> u32;
    /// `ExperienceOrb.count`: how many pickups the game still gives its value for.
    fn count(&self) -> u32;
    fn set_count(&mut self, count: u32);
    /// The orb's collision box (0.5 x 0.5 x 0.5, feet at its position).
    fn bounds(&self) -> Box3;
}

/// `scanForMerges` runs when `tickCount % 20 == 1`: on the orb's first tick and every 20th after.
pub fn scan_due(tick_count: u32) -> bool {
    tick_count % 20 == 1
}

/// An orb taken by the player, with the clump it held.
pub struct Taken<O> {
    pub orb: O,
    pub clump: Clump,
}

/// The clumps of the orbs of a game (each orb's `clumpedMap`, kept for the orbs that hold more than
/// themselves; an orb with no entry is `{value: 1}`).
#[derive(Clone, Debug, Default)]
pub struct Clumps {
    maps: BTreeMap<u32, Clump>,
}

impl Clumps {
    pub fn len(&self) -> usize {
        self.maps.len()
    }
    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }
    /// `clumps$getClumpedMap`. A map whose sum is not the orb's value (the value was changed by
    /// something else) is not trusted: the orb counts as one orb of its value.
    pub fn clump_of<O: Orb>(&self, orb: &O) -> Clump {
        match self.maps.get(&orb.id()) {
            Some(c) if total(c) == orb.value() => c.clone(),
            _ => single(orb.value()),
        }
    }
    /// `clumps$setClumpedMap`: the orb holds this clump, and its value is the sum.
    fn set<O: Orb>(&mut self, orb: &mut O, clump: Clump) {
        let sum = total(&clump);
        orb.set_value(sum);
        // `count` is the game's own count of pickups. The clump is taken in one pickup, so it is
        // always 1 here (anything else would give the value more than once).
        orb.set_count(1);
        if clump == single(sum) {
            self.maps.remove(&orb.id());
        } else {
            self.maps.insert(orb.id(), clump);
        }
    }
    /// An orb is gone: its clump with it.
    pub fn forget(&mut self, id: u32) {
        self.maps.remove(&id);
    }
    /// Keep only the clumps of the orbs `alive` says exist (for a table that has outgrown them).
    pub fn prune(&mut self, mut alive: impl FnMut(u32) -> bool) {
        self.maps.retain(|&id, _| alive(id));
    }

    /// An orb loaded with `Count` above 1 (the game's own merge, or a world saved by a game
    /// without this mod) loads as the original reads it: `{Value: Count}`.
    pub fn normalise<O: Orb>(&mut self, orbs: &mut [O]) {
        for o in orbs.iter_mut().filter(|o| o.count() > 1) {
            let clump = Clump::from([(o.value(), o.count())]);
            self.set(o, clump);
        }
    }

    /// `ExperienceOrb.tryMergeToExisting`: the new orb of `value`, awarded at `at`, joins the first
    /// orb whose box meets the 1 x 1 x 1 box around `at`, whatever its size, and that orb's age
    /// restarts. False when there is none (the game then makes a fresh orb).
    pub fn award<O: Orb>(&mut self, orbs: &mut [O], at: [f64; 3], value: u32) -> bool {
        let area = Box3::around(at, 1.0);
        let Some(i) = orbs.iter().position(|o| o.bounds().intersects(&area)) else {
            return false;
        };
        let mut clump = self.clump_of(&orbs[i]);
        let slot = clump.entry(value).or_insert(0);
        *slot = slot.saturating_add(1);
        self.set(&mut orbs[i], clump);
        orbs[i].set_age(0);
        true
    }

    /// `scanForMerges`, for every orb on the tick it is due (`ticks` are those before this tick):
    /// it takes in every other orb within half a block of its box. The age of the result is the
    /// smaller one. The orbs it took leave `orbs`.
    pub fn scan<O: Orb>(&mut self, orbs: &mut Vec<O>) {
        let mut removed = vec![false; orbs.len()];
        for i in 0..orbs.len() {
            if removed[i] || !scan_due(orbs[i].ticks().wrapping_add(1)) {
                continue;
            }
            let area = orbs[i].bounds().inflate(0.5);
            for j in 0..orbs.len() {
                if j == i || removed[j] || !orbs[j].bounds().intersects(&area) {
                    continue;
                }
                // `MixinExperienceOrb.merge`: the maps add up, the age is the smaller one.
                let clump = merged(&self.clump_of(&orbs[i]), &self.clump_of(&orbs[j]));
                let age = orbs[i].age().min(orbs[j].age());
                self.set(&mut orbs[i], clump);
                orbs[i].set_age(age);
                self.forget(orbs[j].id());
                removed[j] = true;
            }
        }
        let mut flags = removed.into_iter();
        orbs.retain(|_| !flags.next().unwrap_or(false));
    }

    /// `playerTouch` for every orb whose box meets the player's pickup box: each is taken, with the
    /// clump it held, in the order of `orbs`. There is no delay between them.
    pub fn collect<O: Orb>(&mut self, orbs: &mut Vec<O>, area: &Box3) -> Vec<Taken<O>> {
        let mut taken = Vec::new();
        let mut kept = Vec::with_capacity(orbs.len());
        for orb in orbs.drain(..) {
            if orb.bounds().intersects(area) {
                let clump = self.clump_of(&orb);
                self.forget(orb.id());
                taken.push(Taken { orb, clump });
            } else {
                kept.push(orb);
            }
        }
        *orbs = kept;
        taken
    }

    /// What a game tick does to the orbs before they move, in the original's order: orbs loaded as
    /// a count become clumps, orbs that are due take in the orbs near them, and the player (whose
    /// pickup box is `pickup`, none when dead or a spectator) takes every orb that touches them.
    pub fn before_tick<O: Orb>(&mut self, orbs: &mut Vec<O>, pickup: Option<&Box3>) -> Vec<Taken<O>> {
        self.normalise(orbs);
        self.scan(orbs);
        match pickup {
            Some(area) => self.collect(orbs, area),
            None => Vec::new(),
        }
    }
}

/// What the pickup of one clump gives: every orb of the clump runs through `repair` (the player's
/// Mending: it gets the orb's value and returns the experience left over), and what is left over,
/// summed, is the experience to give. Orbs of size 0 give nothing.
pub fn payout(clump: &Clump, mut repair: impl FnMut(u32) -> u32) -> u32 {
    let mut give = 0u32;
    for (&size, &amount) in clump {
        if size == 0 {
            continue;
        }
        for _ in 0..amount {
            give = give.saturating_add(repair(size));
        }
    }
    give
}

// --- the part of the rules the save needs; `mixins/clumps/nbt.rs` is a copy of this block ---
/// `Value` of an orb is a short: a clump above 32767 is saved as `Value` x `Count` (the product the
/// pickup gives is the same), with the largest `Value` that divides it.
pub fn split_value(total: u32) -> (u32, u32) {
    const SHORT: u32 = i16::MAX as u32;
    if total <= SHORT {
        return (total, 1);
    }
    let mut value = SHORT;
    while total % value != 0 {
        value -= 1;
    }
    (value, total / value)
}
// --- end of the save block ---

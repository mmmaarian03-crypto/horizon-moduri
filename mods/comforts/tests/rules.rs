use horizon_comforts::*;
use std::collections::BTreeMap;

// ---- time of day -------------------------------------------------------------------------

#[test]
fn check_time_follows_the_original_table() {
    // ComfortsEvents.checkTime, for the times at the edges of (500, 11500).
    for (t, day, expect) in [
        (500, TimeUse::Day, TimeCheck::Deny),
        (501, TimeUse::Day, TimeCheck::Allow),
        (11499, TimeUse::Day, TimeCheck::Allow),
        (11500, TimeUse::Day, TimeCheck::Deny),
        (13000, TimeUse::Day, TimeCheck::Deny),
        (6000, TimeUse::Night, TimeCheck::Default),
        (13000, TimeUse::Night, TimeCheck::Default),
        (6000, TimeUse::DayOrNight, TimeCheck::Allow),
        (13000, TimeUse::DayOrNight, TimeCheck::Default),
        (500, TimeUse::DayOrNight, TimeCheck::Default),
        (6000, TimeUse::None, TimeCheck::Deny),
        (13000, TimeUse::None, TimeCheck::Deny),
    ] {
        assert_eq!(check_time(t, day), expect, "time {t} use {day:?}");
    }
    // The time is taken modulo 24000, so the second day behaves like the first.
    assert_eq!(check_time(24000 * 5 + 6000, TimeUse::Day), TimeCheck::Allow);
    assert_eq!(check_time(24000 * 5 + 13000, TimeUse::Day), TimeCheck::Deny);
}

#[test]
fn hammocks_by_day_and_bags_by_night_as_in_the_players_config() {
    let s = Settings::default();
    assert_eq!(s.hammock_use, TimeUse::Day);
    assert_eq!(s.sleeping_bag_use, TimeUse::Night);
    let hammock = Some(Kind::Hammock);
    let bag = Some(Kind::SleepingBag);
    // Hammock: any weather, only in (500, 11500).
    assert!(can_sleep_now(&s, hammock, 6000, true));
    assert!(can_sleep_now(&s, hammock, 6000, false), "a day storm still allows it");
    assert!(!can_sleep_now(&s, hammock, 13000, false));
    assert!(!can_sleep_now(&s, hammock, 300, true));
    assert!(!can_sleep_now(&s, hammock, 11800, true), "dusk is too late for it");
    // Sleeping bag: like a bed, when it is not day.
    assert!(!can_sleep_now(&s, bag, 6000, true));
    assert!(can_sleep_now(&s, bag, 6000, false), "thunderstorm by day");
    assert!(can_sleep_now(&s, bag, 14000, false));
    assert!(!can_sleep_now(&s, bag, 23600, true));
    // The dry run at an empty spot is judged as a night bed.
    assert!(can_sleep_now(&s, None, 14000, false));
    assert!(!can_sleep_now(&s, None, 6000, true));
}

#[test]
fn other_time_uses() {
    let mut s = Settings::default();
    s.sleeping_bag_use = TimeUse::DayOrNight;
    assert!(can_sleep_now(&s, Some(Kind::SleepingBag), 6000, true));
    assert!(can_sleep_now(&s, Some(Kind::SleepingBag), 14000, false));
    assert!(!can_sleep_now(&s, Some(Kind::SleepingBag), 12000, true));
    s.sleeping_bag_use = TimeUse::None;
    assert!(!can_sleep_now(&s, Some(Kind::SleepingBag), 14000, false));
    assert_eq!(s.sleeping_bag_use.message(), "block.comforts.no_sleep");
    assert_eq!(TimeUse::Day.message(), "block.comforts.hammock.no_sleep");
    assert_eq!(TimeUse::Night.message(), "block.minecraft.bed.no_sleep");
    assert_eq!(TimeUse::DayOrNight.message(), "block.comforts.hammock.no_sleep.2");
}

#[test]
fn sleeping_through_a_day_skips_to_dusk_not_to_morning() {
    let s = Settings::default();
    // Slept on day 0 at noon: the world goes to tick 11999 of the same day.
    assert_eq!(wake_time(&s, 6000, 24000, true, true), 11999);
    // On day 3 (the world time keeps counting).
    assert_eq!(wake_time(&s, 24000 * 3 + 7000, 24000 * 4, true, true), 24000 * 3 + 11999);
    // Never back in time.
    assert_eq!(wake_time(&s, 11999, 24000, true, true), 11999);
    assert_eq!(wake_time(&s, 12100, 24000, true, true), 12100);
    // A night's sleep (or a day sleeper while it is not day): vanilla's morning.
    assert_eq!(wake_time(&s, 14000, 24000, false, false), 24000);
    assert_eq!(wake_time(&s, 6000, 24000, false, true), 24000);
    assert_eq!(wake_time(&s, 14000, 24000, true, false), 24000);
}

#[test]
fn wake_time_offsets_follow_the_two_options() {
    let mut s = Settings::default();
    s.night_wake_time_offset = 300;
    s.day_wake_time_offset = -1000;
    assert_eq!(wake_time(&s, 6000, 24000, true, true), 11999 + 300);
    assert_eq!(wake_time(&s, 14000, 24000, false, false), 23000);
    // The offset cannot take the clock back.
    s.day_wake_time_offset = -20000;
    assert_eq!(wake_time(&s, 14000, 24000, false, false), 14000);
}

#[test]
fn which_beds_are_slept_through_a_day() {
    let s = Settings::default();
    assert!(is_day_bed(&s, Kind::Hammock));
    assert!(!is_day_bed(&s, Kind::SleepingBag));
    let mut t = s.clone();
    t.sleeping_bag_use = TimeUse::DayOrNight;
    assert!(is_day_bed(&t, Kind::SleepingBag));
}

// ---- names -------------------------------------------------------------------------------

#[test]
fn names_of_the_sixteen_colours() {
    for c in COLORS {
        assert_eq!(kind_of(&format!("comforts:hammock_{c}")), Some(Kind::Hammock));
        assert_eq!(kind_of(&format!("comforts:sleeping_bag_{c}")), Some(Kind::SleepingBag));
    }
    assert_eq!(COLORS.len(), 16);
    assert_eq!(kind_of("comforts:hammock_plaid"), None);
    assert_eq!(kind_of("minecraft:red_bed"), None);
    assert_eq!(kind_of("comforts:rope_and_nail"), None);
    assert!(is_rope("comforts:rope_and_nail"));
    assert!(!is_rope("comforts:hammock_red"));
    assert_eq!(Kind::Hammock.name(), "hammock");
    assert_eq!(Kind::SleepingBag.name(), "sleeping_bag");
}

#[test]
fn directions() {
    for d in [Dir::North, Dir::East, Dir::South, Dir::West] {
        assert_eq!(Dir::from_name(d.name()), Some(d));
        assert_eq!(d.opposite().opposite(), d);
        assert_eq!(offset(offset([5, 6, 7], d, 3), d.opposite(), 3), [5, 6, 7]);
    }
    assert_eq!(offset([0, 0, 0], Dir::North, 1), [0, 0, -1]);
    assert_eq!(offset([0, 0, 0], Dir::East, 2), [2, 0, 0]);
    // Direction.toYRot.
    assert_eq!(Dir::South.yaw(), 0.0);
    assert_eq!(Dir::West.yaw(), 90.0);
    assert_eq!(Dir::North.yaw(), 180.0);
    assert_eq!(Dir::East.yaw(), 270.0);
}

// ---- trySleep ----------------------------------------------------------------------------

struct Player {
    time: i64,
    day: bool,
    natural: bool,
    sleeping: bool,
    alive: bool,
    creative: bool,
    at: [f64; 3],
    solid: Vec<Pos>,
    monsters: bool,
}
impl Default for Player {
    fn default() -> Self {
        Player {
            time: 14000,
            day: false,
            natural: true,
            sleeping: false,
            alive: true,
            creative: false,
            at: [10.5, 64.0, 10.5],
            solid: vec![],
            monsters: false,
        }
    }
}
impl Sleeper for Player {
    fn day_time(&self) -> i64 {
        self.time
    }
    fn is_day(&self) -> bool {
        self.day
    }
    fn natural_dimension(&self) -> bool {
        self.natural
    }
    fn sleeping(&self) -> bool {
        self.sleeping
    }
    fn alive(&self) -> bool {
        self.alive
    }
    fn creative(&self) -> bool {
        self.creative
    }
    fn in_reach(&self, bed: Pos) -> bool {
        (self.at[0] - f64::from(bed[0]) - 0.5).abs() <= 3.0
            && (self.at[1] - f64::from(bed[1])).abs() <= 2.0
            && (self.at[2] - f64::from(bed[2]) - 0.5).abs() <= 3.0
    }
    fn suffocating(&self, p: Pos) -> bool {
        self.solid.contains(&p)
    }
    fn monsters_nearby(&self, _bed: Pos) -> bool {
        self.monsters
    }
}
fn bag_at(at: Pos) -> Bed {
    Bed {
        kind: Some(Kind::SleepingBag),
        at,
        facing: Dir::North,
    }
}

#[test]
fn a_bag_at_night_beside_the_player_works() {
    let s = Settings::default();
    let mut d = SleepData::default();
    assert_eq!(try_sleep(&Player::default(), &s, &mut d, bag_at([10, 64, 10])), Ok(()));
    // The attempt is noted (`PlayerSleepInBedEvent`).
    assert_eq!(d.sleep_time, 14000);
}

#[test]
fn the_failures_come_in_the_originals_order() {
    let s = Settings::default();
    let mut d = SleepData::default();
    let bed = bag_at([10, 64, 10]);
    // Already asleep or dead: no message.
    let mut p = Player::default();
    p.sleeping = true;
    assert_eq!(try_sleep(&p, &s, &mut d, bed), Err(Problem::Other));
    p.sleeping = false;
    p.alive = false;
    assert_eq!(try_sleep(&p, &s, &mut d, bed), Err(Problem::Other));
    // Not a dimension with a day and a night.
    let mut p = Player::default();
    p.natural = false;
    assert_eq!(try_sleep(&p, &s, &mut d, bed), Err(Problem::NotPossibleHere));
    // Too far: neither the head nor the foot (one block behind the head) is within reach.
    let mut p = Player::default();
    p.at = [20.5, 64.0, 20.5];
    assert_eq!(try_sleep(&p, &s, &mut d, bed), Err(Problem::TooFarAway));
    // ... but the foot may be the near one: head 4 blocks away (north of the foot).
    let mut p = Player::default();
    p.at = [10.5, 64.0, 13.5];
    assert_eq!(try_sleep(&p, &s, &mut d, bag_at([10, 64, 9])), Ok(()), "the foot is at z=10");
    // Blocked above the head, or above the foot.
    let mut p = Player::default();
    p.solid = vec![[10, 65, 10]];
    assert_eq!(try_sleep(&p, &s, &mut d, bed), Err(Problem::Obstructed));
    p.solid = vec![[10, 65, 11]];
    assert_eq!(try_sleep(&p, &s, &mut d, bed), Err(Problem::Obstructed));
    // Time, before monsters.
    let mut p = Player::default();
    p.time = 6000;
    p.day = true;
    p.monsters = true;
    assert_eq!(try_sleep(&p, &s, &mut d, bed), Err(Problem::NotPossibleNow));
    // Monsters, unless creative.
    let mut p = Player::default();
    p.monsters = true;
    assert_eq!(try_sleep(&p, &s, &mut d, bed), Err(Problem::NotSafe));
    p.creative = true;
    assert_eq!(try_sleep(&p, &s, &mut d, bed), Ok(()));
}

#[test]
fn a_hammock_by_day_but_not_by_night() {
    let s = Settings::default();
    let mut d = SleepData::default();
    let hammock = Bed {
        kind: Some(Kind::Hammock),
        at: [10, 64, 10],
        facing: Dir::East,
    };
    let mut p = Player::default();
    p.time = 6000;
    p.day = true;
    assert_eq!(try_sleep(&p, &s, &mut d, hammock), Ok(()));
    p.time = 14000;
    p.day = false;
    assert_eq!(try_sleep(&p, &s, &mut d, hammock), Err(Problem::NotPossibleNow));
}

#[test]
fn messages_name_the_right_keys() {
    let s = Settings::default();
    let h = Some(Kind::Hammock);
    let b = Some(Kind::SleepingBag);
    assert_eq!(Problem::NotPossibleNow.message(&s, h), Some("block.comforts.hammock.no_sleep"));
    assert_eq!(Problem::NotPossibleNow.message(&s, b), Some("block.minecraft.bed.no_sleep"));
    assert_eq!(Problem::NotPossibleNow.message(&s, None), Some("block.minecraft.bed.no_sleep"));
    assert_eq!(Problem::TooFarAway.message(&s, h), Some("block.comforts.hammock.too_far_away"));
    assert_eq!(Problem::TooFarAway.message(&s, b), Some("block.comforts.sleeping_bag.too_far_away"));
    assert_eq!(Problem::TooFarAway.message(&s, None), Some("block.comforts.sleeping_bag.too_far_away"));
    assert_eq!(Problem::Obstructed.message(&s, b), Some("block.minecraft.bed.obstructed"));
    assert_eq!(Problem::NotSafe.message(&s, h), Some("block.minecraft.bed.not_safe"));
    assert_eq!(Problem::NotSleepy.message(&s, h), Some("capability.comforts.not_sleepy"));
    assert_eq!(Problem::NotPossibleHere.message(&s, h), None);
    assert_eq!(Problem::Other.message(&s, h), None);
    // Every key a message can name has a fallback text.
    for k in [
        "block.comforts.no_sleep",
        "block.comforts.hammock.no_sleep",
        "block.comforts.hammock.no_sleep.2",
        "block.minecraft.bed.no_sleep",
        "block.comforts.hammock.too_far_away",
        "block.comforts.sleeping_bag.too_far_away",
        "block.minecraft.bed.obstructed",
        "block.minecraft.bed.not_safe",
        "capability.comforts.not_sleepy",
        "block.comforts.hammock.occupied",
        "block.comforts.sleeping_bag.occupied",
        "block.comforts.hammock.no_rope",
        "block.comforts.hammock.no_space",
        "block.comforts.hammock.missing_rope",
        "block.comforts.sleeping_bag.broke",
    ] {
        assert!(!fallback_text(k).is_empty(), "{k}");
    }
}

#[test]
fn restricted_sleeping_asks_for_tiredness() {
    let mut s = Settings::default();
    s.restrict_sleeping = true;
    let bed = bag_at([10, 64, 10]);
    let mut d = SleepData {
        wake_time: 13000,
        tired_time: 20000,
        ..Default::default()
    };
    let p = Player::default(); // time 14000
    assert_eq!(try_sleep(&p, &s, &mut d, bed), Err(Problem::NotSleepy));
    // Once the tired time has passed the player may sleep again.
    let mut p = Player::default();
    p.time = 21000;
    assert_eq!(try_sleep(&p, &s, &mut d, bed), Ok(()));
    // A wake time in the future (the clock was set back) resets both.
    let mut d = SleepData {
        wake_time: 30000,
        tired_time: 40000,
        ..Default::default()
    };
    assert_eq!(try_sleep(&Player::default(), &s, &mut d, bed), Ok(()));
    assert_eq!((d.wake_time, d.tired_time), (0, 0));
    // Off by default: nothing is refused.
    let s = Settings::default();
    let mut d = SleepData {
        wake_time: 13000,
        tired_time: 20000,
        ..Default::default()
    };
    assert_eq!(try_sleep(&Player::default(), &s, &mut d, bed), Ok(()));
}

#[test]
fn the_phantom_rule_follows_the_two_options() {
    let mut s = Settings::default();
    assert!(s.stops_phantoms(Kind::Hammock) && s.stops_phantoms(Kind::SleepingBag));
    s.hammocks_stop_phantoms = false;
    assert!(!s.stops_phantoms(Kind::Hammock) && s.stops_phantoms(Kind::SleepingBag));
}

// ---- waking up ---------------------------------------------------------------------------

#[test]
fn waking_up_notes_the_times_and_forgets_the_auto_sleep() {
    let s = Settings::default();
    let mut d = SleepData {
        sleep_time: 13000,
        ..Default::default()
    };
    let out = on_wake_up(&s, &mut d, Kind::Hammock, 14000, &mut || panic!("no roll for a hammock"));
    assert_eq!(out, WakeOutcome::default());
    assert_eq!(d.wake_time, 14000);
    // tired time = wake + slept / restMultiplier (2).
    assert_eq!(d.tired_time, 14500);
}

#[test]
fn an_auto_used_bag_is_taken_up_when_the_player_gets_up() {
    let s = Settings::default();
    let mut d = SleepData {
        sleep_time: 13000,
        auto_sleep: Some([1, 2, 3]),
        ..Default::default()
    };
    // Even after a short sleep (no roll below 500 ticks).
    let out = on_wake_up(&s, &mut d, Kind::SleepingBag, 13040, &mut || panic!("no roll"));
    assert!(out.take_up && !out.broke);
    assert_eq!(d.auto_sleep, None);
    // A bag put down by hand stays.
    let mut d = SleepData {
        sleep_time: 13000,
        ..Default::default()
    };
    let out = on_wake_up(&s, &mut d, Kind::SleepingBag, 24000, &mut || 0.0);
    assert!(!out.take_up && !out.broke, "break chance 0: never");
}

#[test]
fn a_bag_that_breaks_is_not_also_taken_up() {
    let mut s = Settings::default();
    s.sleeping_bag_break_chance = 40;
    let mut d = SleepData {
        sleep_time: 13000,
        auto_sleep: Some([1, 2, 3]),
        ..Default::default()
    };
    let out = on_wake_up(&s, &mut d, Kind::SleepingBag, 24000, &mut || 0.39);
    assert!(out.broke && !out.take_up);
    // The roll is `nextDouble() < chance`.
    let mut d = SleepData {
        sleep_time: 13000,
        auto_sleep: Some([1, 2, 3]),
        ..Default::default()
    };
    let out = on_wake_up(&s, &mut d, Kind::SleepingBag, 24000, &mut || 0.40);
    assert!(!out.broke && out.take_up);
    // No roll within the first 500 ticks of sleep.
    let mut d = SleepData {
        sleep_time: 13000,
        ..Default::default()
    };
    let out = on_wake_up(&s, &mut d, Kind::SleepingBag, 13500, &mut || panic!("no roll"));
    assert!(!out.broke);
    // Hammocks never break.
    let mut d = SleepData::default();
    let out = on_wake_up(&s, &mut d, Kind::Hammock, 24000, &mut || panic!("no roll"));
    assert!(!out.broke);
}

#[test]
fn players_are_followed_from_bed_to_bed() {
    let mut p = Players::default();
    let a = [1, 2, 3, 4];
    let b = [5, 6, 7, 8];
    let bed1 = ([10, 64, 10], Kind::SleepingBag);
    let bed2 = ([20, 64, 20], Kind::Hammock);
    assert_eq!(p.track(a, None), None);
    assert_eq!(p.track(a, Some(bed1)), None);
    assert_eq!(p.track(a, Some(bed1)), None, "still asleep");
    assert_eq!(p.track(b, Some(bed2)), None);
    assert_eq!(p.track(a, None), Some(bed1), "got up");
    assert_eq!(p.track(a, None), None, "only once");
    assert_eq!(p.track(b, Some(bed1)), Some(bed2), "moved to another bed");
    assert_eq!(p.track(b, None), Some(bed1));
    // The data is per player.
    p.data(a).auto_sleep = Some([9, 9, 9]);
    assert_eq!(p.data(b).auto_sleep, None);
    assert_eq!(p.data(a).auto_sleep, Some([9, 9, 9]));
}

#[test]
fn players_who_leave_the_list_got_up() {
    let mut p = Players::default();
    let a = [1, 2, 3, 4];
    let b = [5, 6, 7, 8];
    let bed = ([10, 64, 10], Kind::SleepingBag);
    assert!(p.track_all(&[(a, Some(bed)), (b, None)]).is_empty());
    // a is still asleep, b was never in one of ours.
    assert!(p.track_all(&[(a, Some(bed)), (b, None)]).is_empty());
    // a is no longer on the list: they got up, once.
    assert_eq!(p.track_all(&[(b, None)]), vec![(a, bed)]);
    assert!(p.track_all(&[(b, None)]).is_empty());
    // Listed without a bed counts as getting up too.
    assert!(p.track_all(&[(a, Some(bed))]).is_empty());
    assert_eq!(p.track_all(&[(a, None)]), vec![(a, bed)]);
    // Nobody at all: nothing to report.
    assert!(p.track_all(&[]).is_empty());
}

// ---- ropes and hammocks ------------------------------------------------------------------

#[derive(Default)]
struct Map {
    cells: BTreeMap<Pos, Cell>,
    solid: Vec<Pos>,
}
impl Terrain for Map {
    fn cell(&self, p: Pos) -> Option<Cell> {
        if p[1] < 0 {
            return None;
        }
        Some(self.cells.get(&p).copied().unwrap_or(Cell::Other))
    }
    fn replaceable(&self, p: Pos) -> bool {
        !self.solid.contains(&p) && matches!(self.cell(p), Some(Cell::Other))
    }
}
impl Map {
    fn rope(&mut self, p: Pos, facing: Dir) {
        self.cells.insert(p, Cell::Rope { facing });
    }
    /// A pair of ropes three blocks apart along `d`, as one hangs them.
    fn pair(d: Dir) -> (Map, Pos, Pos) {
        let mut m = Map::default();
        let first = [10, 64, 10];
        let far = offset(first, d, 3);
        m.rope(first, d);
        m.rope(far, d.opposite());
        (m, first, far)
    }
    fn hang(&mut self, h: Halves) {
        self.cells.insert(
            h.foot,
            Cell::Hammock {
                part: Part::Foot,
                facing: h.facing,
            },
        );
        self.cells.insert(
            h.head,
            Cell::Hammock {
                part: Part::Head,
                facing: h.facing,
            },
        );
    }
}

#[test]
fn a_hammock_hangs_between_two_ropes_three_blocks_apart() {
    for d in [Dir::North, Dir::East, Dir::South, Dir::West] {
        let (m, first, far) = Map::pair(d);
        let plan = plan_hammock(&m, first);
        let foot = offset(first, d, 1);
        let head = offset(first, d, 2);
        assert_eq!(plan, HammockPlan::Place(Halves { foot, head, facing: d }), "{d:?}");
        // Clicking the far rope hangs it the other way round: the foot beside the clicked rope.
        let back = plan_hammock(&m, far);
        assert_eq!(
            back,
            HammockPlan::Place(Halves {
                foot: head,
                head: foot,
                facing: d.opposite()
            })
        );
        // Foot and head are the two blocks between the ropes.
        assert_eq!(offset(far, d.opposite(), 1), head);
    }
}

#[test]
fn clicking_something_else_asks_for_a_rope() {
    let (m, _, _) = Map::pair(Dir::North);
    assert_eq!(plan_hammock(&m, [0, 64, 0]), HammockPlan::NoRope);
    assert_eq!(plan_hammock(&m, [10, -5, 10]), HammockPlan::NoRope, "not loaded");
    assert_eq!(HammockPlan::NoRope.message(), Some("block.comforts.hammock.no_rope"));
}

#[test]
fn a_rope_alone_or_crooked_is_missing_its_partner() {
    let mut m = Map::default();
    m.rope([10, 64, 10], Dir::South);
    assert_eq!(plan_hammock(&m, [10, 64, 10]), HammockPlan::MissingRope);
    // The partner must face the other way.
    m.rope([10, 64, 13], Dir::South);
    assert_eq!(plan_hammock(&m, [10, 64, 10]), HammockPlan::MissingRope);
    // And be on the same line, three blocks away.
    let mut m = Map::default();
    m.rope([10, 64, 10], Dir::South);
    m.rope([11, 64, 13], Dir::North);
    assert_eq!(plan_hammock(&m, [10, 64, 10]), HammockPlan::MissingRope);
    let mut m = Map::default();
    m.rope([10, 64, 10], Dir::South);
    m.rope([10, 65, 13], Dir::North);
    assert_eq!(plan_hammock(&m, [10, 64, 10]), HammockPlan::MissingRope);
    assert_eq!(HammockPlan::MissingRope.message(), Some("block.comforts.hammock.missing_rope"));
}

#[test]
fn ropes_too_close_say_there_is_no_space() {
    for gap in [1, 2] {
        let mut m = Map::default();
        m.rope([10, 64, 10], Dir::South);
        m.rope([10, 64, 10 + gap], Dir::North);
        assert_eq!(plan_hammock(&m, [10, 64, 10]), HammockPlan::NoSpace, "gap {gap}");
    }
    assert_eq!(HammockPlan::NoSpace.message(), Some("block.comforts.hammock.no_space"));
}

#[test]
fn the_two_blocks_must_be_free() {
    let (mut m, first, _) = Map::pair(Dir::East);
    m.solid.push(offset(first, Dir::East, 1));
    assert_eq!(plan_hammock(&m, first), HammockPlan::NoSpace);
    let (mut m, first, _) = Map::pair(Dir::East);
    m.solid.push(offset(first, Dir::East, 2));
    assert_eq!(plan_hammock(&m, first), HammockPlan::NoSpace);
    // Something that is not replaceable, say another hammock block.
    let (mut m, first, _) = Map::pair(Dir::East);
    m.cells.insert(
        offset(first, Dir::East, 1),
        Cell::Hammock {
            part: Part::Foot,
            facing: Dir::North,
        },
    );
    assert_eq!(plan_hammock(&m, first), HammockPlan::NoSpace);
}

#[test]
fn ropes_that_hold_a_hammock_cannot_take_a_second_one() {
    let (mut m, first, far) = Map::pair(Dir::South);
    let HammockPlan::Place(h) = plan_hammock(&m, first) else {
        panic!("should fit")
    };
    m.hang(h);
    // Both ropes now support it; clicking either finds them busy.
    assert_eq!(plan_hammock(&m, first), HammockPlan::MissingRope);
    assert_eq!(plan_hammock(&m, far), HammockPlan::MissingRope);
    assert_eq!(rope_supports(&m, first, Dir::South), Some(h));
    assert_eq!(rope_supports(&m, far, Dir::North), Some(h));
}

#[test]
fn a_rope_supports_a_hammock_only_along_its_axis() {
    let (mut m, first, far) = Map::pair(Dir::West);
    assert_eq!(rope_supports(&m, first, Dir::West), None);
    let HammockPlan::Place(h) = plan_hammock(&m, first) else {
        panic!("should fit")
    };
    m.hang(h);
    assert_eq!(rope_supports(&m, first, Dir::West), Some(h));
    assert_eq!(rope_supports(&m, far, Dir::East), Some(h));
    // A hammock across the rope's line does not rest on it.
    let mut crooked = Map::default();
    crooked.rope(first, Dir::West);
    crooked.cells.insert(
        offset(first, Dir::West, 1),
        Cell::Hammock {
            part: Part::Foot,
            facing: Dir::North,
        },
    );
    assert_eq!(rope_supports(&crooked, first, Dir::West), None);
    // After the rope is gone the hammock is still found from where the rope was
    // (this is how a hammock is let fall).
    m.cells.remove(&first);
    assert_eq!(rope_supports(&m, first, Dir::West), Some(h));
    // Unloaded ground: nothing.
    let mut low = Map::default();
    low.rope([0, -1, 0], Dir::North);
    assert_eq!(rope_supports(&low, [0, -1, 0], Dir::South), None);
}

#[test]
fn the_head_is_always_beside_the_far_rope() {
    // Hung from the first rope: the foot is beside it and the head beside the second;
    // the head is the half that drops the item.
    let (m, first, far) = Map::pair(Dir::North);
    let HammockPlan::Place(h) = plan_hammock(&m, first) else {
        panic!("fits")
    };
    assert_eq!(offset(first, Dir::North, 1), h.foot);
    assert_eq!(offset(far, Dir::South, 1), h.head);
    assert_eq!(h.facing, Dir::North);
}

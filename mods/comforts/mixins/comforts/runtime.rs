// Comforts inside Horizon's game (nightly 142388e1c136). The rules are the engine below
// (src/lib.rs); this part reads the world and the players, and makes the game sleep, wake,
// hang hammocks and let them fall.
//
// Single player runs on an integrated server whose only players are guests, so the sleepers are
// looked up in the guest list (`host.hub.guests`) as well as in the host's own state.
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    // Comforts for Horizon: the rules, without any game code.
// Written from the original mod (Comforts 6.4.0+1.20.1 by Illusive Soulworks, LGPL-3.0-or-later,
// https://github.com/illusivesoulworks/comforts, branch 1.20.x) and the player's own
// `comforts-server.toml`. The class and method each rule comes from is named next to it.
use std::collections::BTreeMap;

pub type Pos = [i32; 3];

// ---------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------

/// `DyeColor` order, as the original registers the 16 colours.
pub const COLORS: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];
pub const SLEEPING_BAG_PREFIX: &str = "comforts:sleeping_bag_";
pub const HAMMOCK_PREFIX: &str = "comforts:hammock_";
pub const ROPE_AND_NAIL: &str = "comforts:rope_and_nail";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    SleepingBag,
    Hammock,
}
impl Kind {
    /// `BedType.name`, the middle of `block.comforts.<name>.too_far_away`.
    pub fn name(self) -> &'static str {
        match self {
            Kind::SleepingBag => "sleeping_bag",
            Kind::Hammock => "hammock",
        }
    }
}

/// The bed a block or item name stands for (`comforts:hammock_red`, ...).
pub fn kind_of(name: &str) -> Option<Kind> {
    let (rest, kind) = if let Some(rest) = name.strip_prefix(SLEEPING_BAG_PREFIX) {
        (rest, Kind::SleepingBag)
    } else if let Some(rest) = name.strip_prefix(HAMMOCK_PREFIX) {
        (rest, Kind::Hammock)
    } else {
        return None;
    };
    COLORS.contains(&rest).then_some(kind)
}
pub fn is_rope(name: &str) -> bool {
    name == ROPE_AND_NAIL
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dir {
    North,
    East,
    South,
    West,
}
impl Dir {
    pub fn from_name(name: &str) -> Option<Dir> {
        Some(match name {
            "north" => Dir::North,
            "east" => Dir::East,
            "south" => Dir::South,
            "west" => Dir::West,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Dir::North => "north",
            Dir::East => "east",
            Dir::South => "south",
            Dir::West => "west",
        }
    }
    pub fn step(self) -> [i32; 3] {
        match self {
            Dir::North => [0, 0, -1],
            Dir::East => [1, 0, 0],
            Dir::South => [0, 0, 1],
            Dir::West => [-1, 0, 0],
        }
    }
    pub fn opposite(self) -> Dir {
        match self {
            Dir::North => Dir::South,
            Dir::East => Dir::West,
            Dir::South => Dir::North,
            Dir::West => Dir::East,
        }
    }
    /// `Direction.toYRot`: the yaw whose `Direction.fromYRot` is this direction.
    pub fn yaw(self) -> f32 {
        match self {
            Dir::South => 0.0,
            Dir::West => 90.0,
            Dir::North => 180.0,
            Dir::East => 270.0,
        }
    }
}
/// `pos.relative(dir, n)`.
pub fn offset(p: Pos, d: Dir, n: i32) -> Pos {
    let s = d.step();
    [p[0] + s[0] * n, p[1] + s[1] * n, p[2] + s[2] * n]
}
pub fn above(p: Pos) -> Pos {
    [p[0], p[1] + 1, p[2]]
}

// ---------------------------------------------------------------------------
// Settings (`ComfortsConfig.Server`)
// ---------------------------------------------------------------------------

/// `ComfortsConfig.ComfortsTimeUse`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeUse {
    None,
    Day,
    Night,
    DayOrNight,
}
impl TimeUse {
    /// `ComfortsTimeUse.getMessage`: the translation key shown when the bed cannot be used now.
    pub fn message(self) -> &'static str {
        match self {
            TimeUse::None => "block.comforts.no_sleep",
            TimeUse::Day => "block.comforts.hammock.no_sleep",
            TimeUse::Night => "block.minecraft.bed.no_sleep",
            TimeUse::DayOrNight => "block.comforts.hammock.no_sleep.2",
        }
    }
    fn day(self) -> bool {
        matches!(self, TimeUse::Day | TimeUse::DayOrNight)
    }
}

/// The options of `comforts-server.toml` this port follows. `Default` is the
/// player's own file (`the 1.20.1 pack/config/comforts-server.toml`).
/// Not followed, because the file leaves them empty or off: `sleepingBagEffects`,
/// the luck term of the break chance, `daySleepingPercentage` and the multiplayer
/// "skipping day" message.
#[derive(Clone, Debug)]
pub struct Settings {
    pub auto_use: bool,
    pub restrict_sleeping: bool,
    pub rest_multiplier: f64,
    pub hammock_use: TimeUse,
    pub sleeping_bag_use: TimeUse,
    pub day_wake_time_offset: i64,
    pub night_wake_time_offset: i64,
    pub hammocks_stop_phantoms: bool,
    pub sleeping_bags_stop_phantoms: bool,
    /// Percent, 0 to 100.
    pub sleeping_bag_break_chance: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_use: true,
            restrict_sleeping: false,
            rest_multiplier: 2.0,
            hammock_use: TimeUse::Day,
            sleeping_bag_use: TimeUse::Night,
            day_wake_time_offset: 0,
            night_wake_time_offset: 0,
            hammocks_stop_phantoms: true,
            sleeping_bags_stop_phantoms: true,
            sleeping_bag_break_chance: 0,
        }
    }
}
impl Settings {
    pub fn time_use(&self, kind: Option<Kind>) -> TimeUse {
        match kind {
            Some(Kind::Hammock) => self.hammock_use,
            Some(Kind::SleepingBag) => self.sleeping_bag_use,
            // ComfortsEvents.checkTime: any other block counts as a night bed.
            None => TimeUse::Night,
        }
    }
    /// `BaseComfortsBlock.canRest`: whether sleeping here resets `TIME_SINCE_REST`
    /// (what keeps phantoms away).
    pub fn stops_phantoms(&self, kind: Kind) -> bool {
        match kind {
            Kind::SleepingBag => self.sleeping_bags_stop_phantoms,
            Kind::Hammock => self.hammocks_stop_phantoms,
        }
    }
}

// ---------------------------------------------------------------------------
// Time of day
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeCheck {
    Allow,
    Default,
    Deny,
}

/// `ComfortsEvents.checkTime`: the answer to Forge's `SleepingTimeCheckEvent`.
pub fn check_time(day_time: i64, time_use: TimeUse) -> TimeCheck {
    // Java's `%`, as in `level.getDayTime() % 24000L`.
    let time = day_time % 24000;
    if time > 500 && time < 11500 && time_use.day() {
        return TimeCheck::Allow;
    }
    if matches!(time_use, TimeUse::DayOrNight | TimeUse::Night) {
        return TimeCheck::Default;
    }
    TimeCheck::Deny
}

/// `ForgeEventFactory.fireSleepingTimeCheck`: may this bed be slept in now?
/// `is_day` is `Level.isDay()`; a `DEFAULT` answer is `!isDay()`.
pub fn can_sleep_now(settings: &Settings, kind: Option<Kind>, day_time: i64, is_day: bool) -> bool {
    match check_time(day_time, settings.time_use(kind)) {
        TimeCheck::Allow => true,
        TimeCheck::Default => !is_day,
        TimeCheck::Deny => false,
    }
}

/// `ComfortsEvents.getWakeTime` (`SleepFinishedTimeEvent`): the time the world
/// is set to when enough players have slept. `new_time` is vanilla's (the next
/// morning). `day_sleeping`: a player who has slept long enough lies in a bed
/// that is used by day. `is_day`: `Level.isDay()`.
pub fn wake_time(
    settings: &Settings,
    current_time: i64,
    new_time: i64,
    day_sleeping: bool,
    is_day: bool,
) -> i64 {
    if day_sleeping && is_day {
        let i = current_time + 24000;
        let result = (i - i % 24000) - 12001;
        return (settings.night_wake_time_offset + result).max(current_time);
    }
    (new_time + settings.day_wake_time_offset).max(current_time)
}

/// Whether sleeping in this bed counts as sleeping through a day
/// (`getWakeTime`'s `daySleeping`).
pub fn is_day_bed(settings: &Settings, kind: Kind) -> bool {
    settings.time_use(Some(kind)).day()
}

// ---------------------------------------------------------------------------
// Sleeping (`BaseComfortsBlock.trySleep`, `ComfortsEvents.onSleep / onWakeUp`)
// ---------------------------------------------------------------------------

/// `ISleepData`, the player's data (sleep time, wake time, tired time, auto-sleep position).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SleepData {
    pub sleep_time: i64,
    pub wake_time: i64,
    pub tired_time: i64,
    pub auto_sleep: Option<Pos>,
}

/// `Player.BedSleepingProblem` as Comforts raises them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// Not an overworld-like dimension: no message.
    NotPossibleHere,
    NotPossibleNow,
    TooFarAway,
    Obstructed,
    NotSafe,
    /// `restrictSleeping`: "you don't feel sleepy yet" (shown by the event itself).
    NotSleepy,
    /// Already asleep or dead: no message.
    Other,
}
impl Problem {
    /// The translation key the player sees, if any (`BaseComfortsBlock.use`,
    /// `SleepingBagItem.useOn`). `kind` is the bed type; `None` is the item's dry
    /// run at an empty spot, which words time problems for a sleeping bag.
    pub fn message(self, settings: &Settings, kind: Option<Kind>) -> Option<&'static str> {
        match self {
            Problem::NotPossibleNow => Some(settings.time_use(Some(kind.unwrap_or(Kind::SleepingBag))).message()),
            Problem::TooFarAway => Some(match kind.unwrap_or(Kind::SleepingBag) {
                Kind::Hammock => "block.comforts.hammock.too_far_away",
                Kind::SleepingBag => "block.comforts.sleeping_bag.too_far_away",
            }),
            Problem::Obstructed => Some("block.minecraft.bed.obstructed"),
            Problem::NotSafe => Some("block.minecraft.bed.not_safe"),
            Problem::NotSleepy => Some("capability.comforts.not_sleepy"),
            Problem::NotPossibleHere | Problem::Other => None,
        }
    }
}

/// English text of the keys above, for when the language file has no entry.
pub fn fallback_text(key: &str) -> &'static str {
    match key {
        "block.comforts.no_sleep" => "This is decorative, you cannot sleep here",
        "block.comforts.hammock.no_sleep" => "You can only sleep during the day",
        "block.comforts.hammock.no_sleep.2" => {
            "You can only sleep during the day, at night, or during thunderstorms"
        }
        "block.minecraft.bed.no_sleep" => "You can sleep only at night or during thunderstorms",
        "block.comforts.hammock.too_far_away" => "You may not rest now; the hammock is too far away",
        "block.comforts.sleeping_bag.too_far_away" => {
            "You may not rest now; the sleeping bag is too far away"
        }
        "block.minecraft.bed.obstructed" => "This bed is obstructed",
        "block.minecraft.bed.not_safe" => "You may not rest now; there are monsters nearby",
        "capability.comforts.not_sleepy" => "You may not rest now; you don't feel sleepy yet",
        "block.comforts.hammock.occupied" => "This hammock is occupied",
        "block.comforts.sleeping_bag.occupied" => "This sleeping bag is occupied",
        "block.comforts.hammock.no_rope" => "You must hang the hammock on a pair of rope and nail",
        "block.comforts.hammock.no_space" => "There is not enough space to fit the hammock",
        "block.comforts.hammock.missing_rope" => {
            "There must also be a rope and nail on the other end to hang the hammock"
        }
        "block.comforts.sleeping_bag.broke" => "The sleeping bag broke",
        _ => "",
    }
}

/// What `trySleep` asks of the world and the player.
pub trait Sleeper {
    fn day_time(&self) -> i64;
    /// `Level.isDay()`.
    fn is_day(&self) -> bool;
    /// `DimensionType.natural()`.
    fn natural_dimension(&self) -> bool;
    /// `Player.isSleeping()`.
    fn sleeping(&self) -> bool;
    /// `Player.isAlive()`.
    fn alive(&self) -> bool;
    fn creative(&self) -> bool;
    /// `isReachableBedBlock`: within 3 blocks sideways and 2 vertically of the block's bottom centre.
    fn in_reach(&self, bed: Pos) -> bool;
    /// `isAbnormalCube`: a block that suffocates.
    fn suffocating(&self, p: Pos) -> bool;
    /// A monster that prevents rest within 8 blocks sideways and 5 vertically of the bed.
    fn monsters_nearby(&self, bed: Pos) -> bool;
}

/// The spot a sleep attempt is made at.
#[derive(Clone, Copy, Debug)]
pub struct Bed {
    /// `None` for the sleeping-bag item's dry run, where nothing is placed yet.
    pub kind: Option<Kind>,
    /// The head of the bed (or the spot above the clicked block for the dry run).
    pub at: Pos,
    /// `BlockStateProperties.FACING` of the bed, or the player's direction.
    pub facing: Dir,
}

/// `ComfortsEvents.onSleep` (`PlayerSleepInBedEvent`): notes the time of the
/// attempt and, with `restrictSleeping`, refuses a player who is not tired.
pub fn on_sleep(settings: &Settings, data: &mut SleepData, day_time: i64) -> Result<(), Problem> {
    data.sleep_time = day_time;
    if settings.restrict_sleeping {
        if data.wake_time > day_time {
            data.wake_time = 0;
            data.tired_time = 0;
        }
        if data.tired_time > day_time {
            return Err(Problem::NotSleepy);
        }
    }
    Ok(())
}

/// `BaseComfortsBlock.trySleep` up to (not including) `startSleeping`.
pub fn try_sleep(
    world: &impl Sleeper,
    settings: &Settings,
    data: &mut SleepData,
    bed: Bed,
) -> Result<(), Problem> {
    on_sleep(settings, data, world.day_time())?;
    if world.sleeping() || !world.alive() {
        return Err(Problem::Other);
    }
    if !world.natural_dimension() {
        return Err(Problem::NotPossibleHere);
    }
    // bedInRange: the head or the foot.
    if !(world.in_reach(bed.at) || world.in_reach(offset(bed.at, bed.facing.opposite(), 1))) {
        return Err(Problem::TooFarAway);
    }
    // bedBlocked: the blocks above the head and the foot.
    let up = above(bed.at);
    if world.suffocating(up) || world.suffocating(offset(up, bed.facing.opposite(), 1)) {
        return Err(Problem::Obstructed);
    }
    if !can_sleep_now(settings, bed.kind, world.day_time(), world.is_day()) {
        return Err(Problem::NotPossibleNow);
    }
    if !world.creative() && world.monsters_nearby(bed.at) {
        return Err(Problem::NotSafe);
    }
    Ok(())
}

/// What a wake-up of a player in a sleeping bag or hammock asks the game to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct WakeOutcome {
    /// The sleeping bag broke: both halves go, nothing drops, a message and a sound.
    pub broke: bool,
    /// The bag was placed by auto-use: it is taken up again (the foot first, so the
    /// head drops the item; in creative the head first, so nothing drops).
    pub take_up: bool,
}

/// `ComfortsEvents.onWakeUp`. `roll` is `level.random.nextDouble()`, drawn only
/// when a bag was slept in for more than 500 ticks.
pub fn on_wake_up(
    settings: &Settings,
    data: &mut SleepData,
    kind: Kind,
    wake_time: i64,
    roll: &mut dyn FnMut() -> f64,
) -> WakeOutcome {
    let time_slept = wake_time - data.sleep_time;
    let mut outcome = WakeOutcome::default();
    if kind == Kind::SleepingBag {
        if time_slept > 500 {
            let break_chance = f64::from(settings.sleeping_bag_break_chance) / 100.0;
            if roll() < break_chance {
                outcome.broke = true;
            }
        }
        if !outcome.broke && data.auto_sleep.is_some() {
            outcome.take_up = true;
        }
    }
    data.wake_time = wake_time;
    // (long) (timeSlept / restMultiplier)
    data.tired_time = wake_time + (time_slept as f64 / settings.rest_multiplier) as i64;
    data.auto_sleep = None;
    outcome
}

// ---------------------------------------------------------------------------
// Ropes and hammocks
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Head,
    Foot,
}

/// What a block is, as far as hanging a hammock cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Other,
    /// `RopeAndNailBlock` with its `facing`: the side the rope hangs towards.
    Rope { facing: Dir },
    /// `HammockBlock`: `facing` runs from the foot to the head.
    Hammock { part: Part, facing: Dir },
}

pub trait Terrain {
    /// `None`: not loaded.
    fn cell(&self, p: Pos) -> Option<Cell>;
    /// `BlockState.canBeReplaced`.
    fn replaceable(&self, p: Pos) -> bool;
}

/// The two halves of a hammock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Halves {
    pub foot: Pos,
    pub head: Pos,
    pub facing: Dir,
}

/// The `SUPPORTING` property of the original, which the port reads off the
/// world: a rope supports a hammock when a hammock half lies against it along
/// its axis (the foot against the rope it was placed from, the head against the
/// far one).
pub fn rope_supports(world: &impl Terrain, rope: Pos, facing: Dir) -> Option<Halves> {
    let front = offset(rope, facing, 1);
    match world.cell(front)? {
        Cell::Hammock {
            part: Part::Foot,
            facing: f,
        } if f == facing => Some(Halves {
            foot: front,
            head: offset(front, f, 1),
            facing: f,
        }),
        Cell::Hammock {
            part: Part::Head,
            facing: f,
        } if f == facing.opposite() => Some(Halves {
            foot: offset(front, facing, 1),
            head: front,
            facing: f,
        }),
        _ => None,
    }
}

/// `HammockItem.hasPartneredRopes`.
fn partnered(world: &impl Terrain, rope: Pos, facing: Dir, other: Pos) -> bool {
    matches!(world.cell(other), Some(Cell::Rope { facing: f }) if f == facing.opposite())
        && rope_supports(world, rope, facing).is_none()
        && rope_supports(world, other, facing.opposite()).is_none()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HammockPlan {
    /// The click was not on a rope and nail ("block.comforts.hammock.no_rope").
    NoRope,
    /// No partner at three blocks, none closer either ("...missing_rope").
    MissingRope,
    /// A partner is there but the hammock does not fit ("...no_space").
    NoSpace,
    /// Hang it: the foot beside the clicked rope, the head beside the far one.
    Place(Halves),
}
impl HammockPlan {
    pub fn message(self) -> Option<&'static str> {
        match self {
            HammockPlan::NoRope => Some("block.comforts.hammock.no_rope"),
            HammockPlan::MissingRope => Some("block.comforts.hammock.missing_rope"),
            HammockPlan::NoSpace => Some("block.comforts.hammock.no_space"),
            HammockPlan::Place(_) => None,
        }
    }
}

/// `HammockItem.useOn` for a click on `clicked`.
pub fn plan_hammock(world: &impl Terrain, clicked: Pos) -> HammockPlan {
    let Some(Cell::Rope { facing }) = world.cell(clicked) else {
        return HammockPlan::NoRope;
    };
    if partnered(world, clicked, facing, offset(clicked, facing, 3)) {
        let foot = offset(clicked, facing, 1);
        let head = offset(clicked, facing, 2);
        // BlockItem.place: both spots must take the hammock.
        return if world.replaceable(foot) && world.replaceable(head) {
            HammockPlan::Place(Halves { foot, head, facing })
        } else {
            HammockPlan::NoSpace
        };
    }
    let close = partnered(world, clicked, facing, offset(clicked, facing, 1))
        || partnered(world, clicked, facing, offset(clicked, facing, 2));
    if close {
        HammockPlan::NoSpace
    } else {
        HammockPlan::MissingRope
    }
}

// ---------------------------------------------------------------------------
// Per-player data, kept by the host of the world
// ---------------------------------------------------------------------------

/// The players' `ISleepData` by profile, and which bed each was last seen in.
#[derive(Debug, Default)]
pub struct Players {
    data: BTreeMap<[i32; 4], SleepData>,
    in_bed: BTreeMap<[i32; 4], (Pos, Kind)>,
}
impl Players {
    pub fn data(&mut self, who: [i32; 4]) -> &mut SleepData {
        self.data.entry(who).or_default()
    }
    /// Notes where each of `present` lies now (`None`: in no Comforts bed) and returns the
    /// beds players got up from since the last call: those now in another bed or none, and those
    /// who are not listed any more (they left).
    pub fn track_all(
        &mut self,
        present: &[([i32; 4], Option<(Pos, Kind)>)],
    ) -> Vec<([i32; 4], (Pos, Kind))> {
        let mut woke = Vec::new();
        for &(who, now) in present {
            if let Some(bed) = self.track(who, now) {
                woke.push((who, bed));
            }
        }
        let gone: Vec<[i32; 4]> = self
            .in_bed
            .keys()
            .copied()
            .filter(|k| !present.iter().any(|(who, _)| who == k))
            .collect();
        for who in gone {
            if let Some(bed) = self.in_bed.remove(&who) {
                woke.push((who, bed));
            }
        }
        woke
    }
    /// Notes where `who` sleeps now (`None`: in no Comforts bed). Returns the bed
    /// they got up from, if they were in one a tick ago and are not any more.
    pub fn track(&mut self, who: [i32; 4], now: Option<(Pos, Kind)>) -> Option<(Pos, Kind)> {
        let before = self.in_bed.get(&who).copied();
        match now {
            Some(bed) => {
                self.in_bed.insert(who, bed);
                // A move to another Comforts bed in one tick is a wake-up too.
                before.filter(|b| b.0 != bed.0)
            }
            None => {
                self.in_bed.remove(&who);
                before
            }
        }
    }
}

}
use engine::{Bed, Cell, Dir, HammockPlan, Kind, Pos, Settings, Terrain};
use horizon_core::random::RandomSource;
use horizon_core::{geometry::Direction, geometry::Vec3, position::BlockPos};
use horizon_world::level::{LevelEvent, PlaceContext, PlaceOutcome};
use horizon_world::registry::BlockStateId;
use std::{cell::RefCell, collections::BTreeMap, path::PathBuf};

/// What the mod remembers, per world (the server thread's world in single player).
#[derive(Default)]
struct State {
    players: engine::Players,
    /// A sleeping bag being put down with auto use: the spot its dry run was made for
    /// (`SleepingBagItem.useOn`: the block above the one clicked).
    armed: Option<Pos>,
    /// Rope and nails that were destroyed (where, facing): a hammock that rested on one falls.
    falls: Vec<(Pos, Dir)>,
}
thread_local! {
    static STATES: RefCell<BTreeMap<PathBuf, State>> = const { RefCell::new(BTreeMap::new()) };
}
fn with_state<T>(game: &Game<'_>, f: impl FnOnce(&mut State) -> T) -> T {
    STATES.with(|states| f(states.borrow_mut().entry(game.world_root.clone()).or_default()))
}

fn direction(d: Dir) -> Direction {
    match d {
        Dir::North => Direction::North,
        Dir::East => Direction::East,
        Dir::South => Direction::South,
        Dir::West => Direction::West,
    }
}
/// `Direction.fromYRot`, the way the player looks.
fn dir_from_yaw(yaw: f32) -> Dir {
    match ((f64::from(yaw) / 90.0 + 0.5).floor() as i32) & 3 {
        0 => Dir::South,
        1 => Dir::West,
        2 => Dir::North,
        _ => Dir::East,
    }
}
/// `Level.isDay()` is false: the overworld's night (clock 12542 to 23460) or a thunderstorm.
fn is_night(game: &Game<'_>) -> bool {
    let time = game.world.day_time.rem_euclid(24000);
    (12542..23460).contains(&time) || game.world.weather.thundering
}

/// The world as hanging a hammock sees it.
struct Live<'a, 'g> {
    game: &'a Game<'g>,
}
impl Terrain for Live<'_, '_> {
    fn cell(&self, p: Pos) -> Option<Cell> {
        let g = self.game;
        if p[1] < g.dim.min_y() || p[1] > g.dim.max_y() || !g.world.loaded(p[0], p[2]) {
            return None;
        }
        let state = g.world.block(p[0], p[1], p[2]);
        let name = &*g.res.facts().get(state).name;
        let facing = g.comforts_facing(state);
        Some(if engine::is_rope(name) {
            match facing {
                Some(facing) => Cell::Rope { facing },
                None => Cell::Other,
            }
        } else if engine::kind_of(name) == Some(Kind::Hammock) {
            match (facing, g.comforts_prop(state, "part").as_deref()) {
                (Some(facing), Some("head")) => Cell::Hammock {
                    part: engine::Part::Head,
                    facing,
                },
                (Some(facing), Some("foot")) => Cell::Hammock {
                    part: engine::Part::Foot,
                    facing,
                },
                _ => Cell::Other,
            }
        } else {
            Cell::Other
        })
    }
    fn replaceable(&self, p: Pos) -> bool {
        let g = self.game;
        g.res
            .facts()
            .get(g.world.block(p[0], p[1], p[2]))
            .replaceable
    }
}

/// The player who acts, as `trySleep` sees them.
struct Body<'a, 'g> {
    game: &'a Game<'g>,
}
impl engine::Sleeper for Body<'_, '_> {
    fn day_time(&self) -> i64 {
        self.game.world.day_time
    }
    fn is_day(&self) -> bool {
        !is_night(self.game)
    }
    fn natural_dimension(&self) -> bool {
        matches!(self.game.dim, super::Dim::Overworld)
    }
    fn sleeping(&self) -> bool {
        self.game.sleeping.is_some()
    }
    fn alive(&self) -> bool {
        self.game
            .player
            .as_ref()
            .is_some_and(|p| !p.health.dead_or_dying())
    }
    fn creative(&self) -> bool {
        self.game.creative
    }
    /// `ServerPlayer.isReachableBedBlock`.
    fn in_reach(&self, bed: Pos) -> bool {
        let Some(p) = self.game.player.as_ref() else {
            return false;
        };
        (p.position.x - f64::from(bed[0]) - 0.5).abs() <= 3.0
            && (p.position.y - f64::from(bed[1])).abs() <= 2.0
            && (p.position.z - f64::from(bed[2]) - 0.5).abs() <= 3.0
    }
    /// `isAbnormalCube` (`BlockState.isSuffocating`), as Horizon's own bed check reads it.
    fn suffocating(&self, p: Pos) -> bool {
        let f = self
            .game
            .res
            .facts()
            .get(self.game.world.block(p[0], p[1], p[2]));
        f.full_collision && f.solid_render
    }
    /// A `Monster` that `isPreventingPlayerRest` within 8 blocks sideways and 5 vertically.
    fn monsters_nearby(&self, bed: Pos) -> bool {
        let at = [
            f64::from(bed[0]) + 0.5,
            f64::from(bed[1]),
            f64::from(bed[2]) + 0.5,
        ];
        self.game.mobs.iter().any(|m| {
            horizon_client::mobs::ai::is_monster(m.species())
                && (m.species() != horizon_client::mobs::Species::ZombifiedPiglin
                    || (m.extra.anger > 0 && m.ai.target.is_some()))
                && m.alive()
                && (m.position.x - at[0]).abs() < 8.0
                && (m.position.y - at[1]).abs() < 5.0
                && (m.position.z - at[2]).abs() < 8.0
        })
    }
}

/// Someone lying in a Comforts bed, or in some bed.
struct Sleeper {
    who: [i32; 4],
    /// The guest's index in the hub, `None` for the host's own player.
    guest: Option<usize>,
    /// The bed's head and kind, when it is one of ours.
    bed: Option<(Pos, Kind)>,
    /// `Player.sleepCounter` (100 is "slept long enough").
    timer: u32,
}

impl Game<'_> {
    /// A property of a block state (`facing`, `part`).
    fn comforts_prop(&self, state: BlockStateId, key: &str) -> Option<String> {
        self.pack
            .assets
            .registry
            .state(state)
            .and_then(|s| s.properties.get(key).cloned())
    }
    fn comforts_facing(&self, state: BlockStateId) -> Option<Dir> {
        self.comforts_prop(state, "facing")
            .and_then(|v| Dir::from_name(&v))
    }
    /// The Comforts bed whose head is at `head`.
    fn comforts_bed_at(&self, head: Pos) -> Option<Kind> {
        let state = self.world.block(head[0], head[1], head[2]);
        engine::kind_of(&self.res.facts().get(state).name)
    }
    /// A line over the hotbar of the acting player (`displayClientMessage(.., true)`).
    fn comforts_say(&mut self, key: &str) {
        self.tell_sleeper(key, engine::fallback_text(key), true);
    }

    // ---- a click on a bed (`BaseComfortsBlock.use`) -------------------------------------------

    /// Right click on a bed whose head is at (or whose foot is at) `pos`: Comforts' own rules,
    /// which never set the respawn point. `false`: not one of ours, the game goes on.
    pub(super) fn comforts_try_sleep(&mut self, pos: [i32; 3]) -> bool {
        if self.remote.is_some() {
            return false;
        }
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let Some(kind) = engine::kind_of(&self.res.facts().get(state).name) else {
            return false;
        };
        let Some(facing) = self.comforts_facing(state) else {
            return true;
        };
        let head = if self.comforts_prop(state, "part").as_deref() == Some("foot") {
            engine::offset(pos, facing, 1)
        } else {
            pos
        };
        let settings = Settings::default();
        let who = self.acting_player_uuid();
        let mut data = with_state(self, |st| st.players.data(who).clone());
        let bed = Bed {
            kind: Some(kind),
            at: head,
            facing,
        };
        let result = engine::try_sleep(&Body { game: self }, &settings, &mut data, bed);
        with_state(self, |st| *st.players.data(who) = data);
        match result {
            Err(problem) => {
                if let Some(key) = problem.message(&settings, Some(kind)) {
                    self.comforts_say(key);
                }
            }
            Ok(()) => {
                // LivingEntity.startSleeping, without setRespawnPosition.
                self.stop_mining();
                self.left = false;
                self.right = false;
                self.using = None;
                self.sleeping = Some(head);
                self.sleep_timer = 0;
                self.stats.custom("sleep_in_bed", 1);
                self.trigger_player("minecraft:slept_in_bed");
                // ServerPlayer.startSleeping resets TIME_SINCE_REST (no phantoms); Comforts puts
                // the old value back when its option for this bed is off.
                if settings.stops_phantoms(kind) {
                    self.stats
                        .reset("custom", "minecraft:time_since_rest");
                }
            }
        }
        true
    }

    // ---- items used on a block ---------------------------------------------------------------

    /// `HammockItem.useOn` and the dry run of `SleepingBagItem.useOn`. `Some`: the click ends here
    /// (`true` as `use_hand` answers it); `None`: the game places the item as usual.
    pub(super) fn comforts_use_on(&mut self, stack: &horizon_client::items::ItemStack) -> Option<Result<bool>> {
        if self.remote.is_some() {
            return None;
        }
        with_state(self, |st| st.armed = None);
        let kind = engine::kind_of(&stack.item)?;
        if !self.may_build() {
            return None;
        }
        let hit = self.pick()?;
        match kind {
            Kind::Hammock => Some(self.comforts_hang(stack, hit.pos)),
            Kind::SleepingBag => self.comforts_try_bag(hit.pos),
        }
    }

    /// `HammockItem.useOn`: only on a rope and nail with its partner three blocks away.
    fn comforts_hang(&mut self, stack: &horizon_client::items::ItemStack, clicked: Pos) -> Result<bool> {
        match engine::plan_hammock(&Live { game: self }, clicked) {
            HammockPlan::Place(h) => {
                // BlockItem.place at the foot, facing the far rope, as BlockPlaceContext.at does.
                let step = h.facing.step();
                let ctx = PlaceContext {
                    clicked_pos: BlockPos::new(h.foot[0], h.foot[1], h.foot[2]),
                    face: direction(h.facing),
                    hit: Vec3::new(
                        f64::from(h.foot[0]) + 0.5 + f64::from(step[0]) * 0.5,
                        f64::from(h.foot[1]) + 0.5,
                        f64::from(h.foot[2]) + 0.5 + f64::from(step[2]) * 0.5,
                    ),
                    inside: false,
                    yaw: h.facing.yaw(),
                    pitch: 0.0,
                    sneaking: false,
                    item: stack.item.to_string(),
                    can_use_game_master_blocks: false,
                };
                let outcome = self.with_level(|b, level, ticks| b.place_block(level, ticks, &ctx));
                self.process_level_events()?;
                match outcome {
                    PlaceOutcome::Placed { pos, .. } => {
                        self.swing_arm();
                        self.stats.add("used", &stack.item, 1);
                        self.trigger_block("minecraft:placed_block", [pos.x, pos.y, pos.z], stack);
                        self.consume_held();
                    }
                    PlaceOutcome::Failed => self.comforts_say("block.comforts.hammock.no_space"),
                }
            }
            other => {
                if let Some(key) = other.message() {
                    self.comforts_say(key);
                }
            }
        }
        Ok(true)
    }

    /// `SleepingBagItem.useOn` with `autoUse`: the bag is put down only if its owner could sleep
    /// there now (`trySleep(.., clickedPos.above(), true)`); then it is slept in at once.
    fn comforts_try_bag(&mut self, clicked: Pos) -> Option<Result<bool>> {
        let settings = Settings::default();
        if !settings.auto_use || self.keys.contains(&winit::keyboard::KeyCode::ShiftLeft) {
            return None;
        }
        let at = engine::above(clicked);
        // The block there gives the direction if it has a `facing`, else the player's.
        let state = self.world.block(at[0], at[1], at[2]);
        let facing = self
            .comforts_facing(state)
            .unwrap_or_else(|| dir_from_yaw(self.yaw));
        let who = self.acting_player_uuid();
        let mut data = with_state(self, |st| st.players.data(who).clone());
        let bed = Bed {
            kind: None,
            at,
            facing,
        };
        let result = engine::try_sleep(&Body { game: self }, &settings, &mut data, bed);
        with_state(self, |st| *st.players.data(who) = data);
        match result {
            Err(problem) => {
                if let Some(key) = problem.message(&settings, None) {
                    self.comforts_say(key);
                }
                // InteractionResult.FAIL.
                Some(Ok(true))
            }
            Ok(()) => {
                with_state(self, |st| st.armed = Some(at));
                None
            }
        }
    }

    /// A block item was placed (`BlockItem.place`): a sleeping bag put down with auto use is
    /// slept in (`ComfortsClientEvents.onTick` uses the bag the tick after it was placed).
    pub(super) fn comforts_placed(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        _stack: &horizon_client::items::ItemStack,
    ) {
        if self.remote.is_some() {
            return;
        }
        let Some(at) = with_state(self, |st| st.armed.take()) else {
            return;
        };
        if engine::kind_of(&self.res.facts().get(state).name) != Some(Kind::SleepingBag) {
            return;
        }
        let Some(facing) = self.comforts_facing(state) else {
            return;
        };
        let who = self.acting_player_uuid();
        with_state(self, |st| st.players.data(who).auto_sleep = Some(at));
        self.comforts_try_sleep(engine::offset(pos, facing, 1));
    }

    // ---- rope and nails that go -----------------------------------------------------------------

    /// A block was destroyed: a rope and nail is noted, its hammock falls at the next tick
    /// (`RopeAndNailBlock.playerWillDestroy` and `canSurvive` drop it).
    pub(super) fn comforts_destroyed(&mut self, pos: [i32; 3], state: BlockStateId) {
        if self.remote.is_some() || !engine::is_rope(&self.res.facts().get(state).name) {
            return;
        }
        let Some(facing) = self.comforts_facing(state) else {
            return;
        };
        with_state(self, |st| {
            if !st.falls.contains(&(pos, facing)) {
                st.falls.push((pos, facing));
            }
        });
    }

    /// `RopeAndNailBlock.dropHammock`: the hammock lets go; the head half drops the one item.
    fn comforts_let_fall(&mut self) {
        let falls = with_state(self, |st| std::mem::take(&mut st.falls));
        for (rope, facing) in falls {
            // Put up again in the meantime: nothing falls.
            if matches!(Live { game: self }.cell(rope), Some(Cell::Rope { facing: f }) if f == facing) {
                continue;
            }
            let Some(h) = engine::rope_supports(&Live { game: self }, rope, facing) else {
                continue;
            };
            let _ = self.destroy_block_dropping(h.head);
            if matches!(Live { game: self }.cell(h.foot), Some(Cell::Hammock { .. })) {
                let _ = self.destroy_block_dropping(h.foot);
            }
        }
    }

    // ---- sleepers, every tick ------------------------------------------------------------------

    /// Everyone lying in a bed: the host's own player and the guests.
    fn comforts_roster(&mut self) -> Vec<Sleeper> {
        let mut out = Vec::new();
        let kind_at = |game: &Game<'_>, head: Pos| game.comforts_bed_at(head).map(|k| (head, k));
        if self.player.is_some()
            && self.lan_acting.is_none()
            && let Some(head) = self.sleeping
        {
            let who = self.acting_player_uuid();
            out.push(Sleeper {
                who,
                guest: None,
                bed: kind_at(self, head),
                timer: self.sleep_timer,
            });
        }
        if let Some(host) = self.lan.as_ref() {
            for (i, g) in host.hub.guests.iter().enumerate() {
                let Some(head) = g.bed else { continue };
                out.push(Sleeper {
                    who: horizon_client::lan::playerdata::uuid_ints(g.uuid),
                    guest: Some(i),
                    bed: kind_at(self, head),
                    timer: g.sleep_timer,
                });
            }
        }
        out
    }

    /// `ServerLevel.tick`'s sleep, for Comforts' beds: waking up is noticed, a bed used outside its
    /// hours wakes its sleeper, and the day a hammock sleeps through is skipped to dusk.
    /// `true`: this tick's sleep is done here (otherwise the game's own `tick_sleep` runs).
    pub(super) fn comforts_tick_sleep(&mut self) -> bool {
        if self.remote.is_some() {
            return false;
        }
        let settings = Settings::default();
        self.comforts_let_fall();

        // Players who got up since the last tick (any way: a key, the clock, a broken bed).
        let roster = self.comforts_roster();
        let present: Vec<([i32; 4], Option<(Pos, Kind)>)> =
            roster.iter().map(|s| (s.who, s.bed)).collect();
        let woke = with_state(self, |st| st.players.track_all(&present));
        for (who, (head, kind)) in woke {
            self.comforts_woke(who, head, kind, &settings);
        }

        // Player.tick: a bed that may not be slept in at this time of day wakes its sleeper.
        let day_time = self.world.day_time;
        let day = !is_night(self);
        for s in &roster {
            let Some((_, kind)) = s.bed else { continue };
            if engine::can_sleep_now(&settings, Some(kind), day_time, day) {
                continue;
            }
            match s.guest {
                None => self.wake_up(),
                Some(i) => {
                    if let Some(host) = self.lan.as_mut() {
                        host.hub.wake(i);
                    }
                }
            }
        }

        // The sleep of the world. Only what differs from the game's is done here: the host's own
        // sleep in a Comforts bed (the game wakes a sleeper whose bed is not named `*_bed`)
        // and a day (or an offset) the time is set for.
        let roster = self.comforts_roster();
        let host_here = self.player.is_some() && self.lan_acting.is_none();
        let host_in_ours = host_here
            && self
                .sleeping
                .is_some_and(|head| self.comforts_bed_at(head).is_some());
        let enough = self.enough_slept();
        let day_sleeping = roster.iter().any(|s| {
            s.timer >= 100 && s.bed.is_some_and(|(_, kind)| engine::is_day_bed(&settings, kind))
        });
        let special = (day_sleeping && day) || settings.day_wake_time_offset != 0;
        if !host_in_ours && !(enough && special) {
            return false;
        }
        self.sleep_timer = super::tick_sleep_timer(self.sleep_timer, self.sleeping.is_some());
        if let Some(head) = self.sleeping {
            let bed_gone = self.comforts_bed_at(head).is_none();
            if (self.screen != super::Screen::Chat
                && self.keys.contains(&winit::keyboard::KeyCode::ShiftLeft))
                || bed_gone
                || self
                    .player
                    .as_ref()
                    .is_none_or(|p| p.health.dead_or_dying())
            {
                self.wake_up();
                return true;
            }
            if let Some(p) = self.player.as_mut() {
                p.position = Vec3::new(
                    f64::from(head[0]) + 0.5,
                    f64::from(head[1]) + 0.5625,
                    f64::from(head[2]) + 0.5,
                );
                p.velocity = Vec3::ZERO;
            }
        }
        if self.enough_slept() {
            if self.ui.rules.bool("advance_time") {
                let t = self.world.day_time;
                let morning = t + 24000 - t.rem_euclid(24000);
                self.world.day_time = engine::wake_time(&settings, t, morning, day_sleeping, day);
            }
            if self.ui.rules.bool("advance_weather") && self.world.weather.levels(1.0).0 > 0.2 {
                self.world.weather.raining = false;
                self.world.weather.thundering = false;
                self.world.weather.rain_time = 0;
                self.world.weather.thunder_time = 0;
            }
            if self.sleeping.is_some() {
                self.wake_up();
            }
            self.wake_guests();
            self.sleep_seen.1 = 0;
        }
        true
    }

    /// `ComfortsEvents.onWakeUp` for a player who was in the bed with its head at `head`.
    fn comforts_woke(&mut self, who: [i32; 4], head: Pos, kind: Kind, settings: &Settings) {
        let mut data = with_state(self, |st| st.players.data(who).clone());
        let wake_time = self.world.day_time;
        let outcome = {
            let mut roll = || self.world.random.next_double();
            engine::on_wake_up(settings, &mut data, kind, wake_time, &mut roll)
        };
        with_state(self, |st| *st.players.data(who) = data);
        if kind != Kind::SleepingBag || !(outcome.broke || outcome.take_up) {
            return;
        }
        // The bag may have been removed already (it is what woke them).
        let state = self.world.block(head[0], head[1], head[2]);
        if engine::kind_of(&self.res.facts().get(state).name) != Some(Kind::SleepingBag) {
            return;
        }
        let Some(facing) = self.comforts_facing(state) else {
            return;
        };
        let foot = engine::offset(head, facing, -1);
        let creative = self.comforts_creative(who);
        let air = self.world.air;
        let remove = |game: &mut Game<'_>, p: Pos| {
            if let Err(e) = game.level_set(p, air) {
                log::warn!("comforts: could not take up the sleeping bag at {p:?}: {e}");
            }
        };
        if outcome.broke {
            // removeBlock(head), removeBlock(foot): nothing drops.
            remove(self, head);
            remove(self, foot);
            self.comforts_tell_player(who, "block.comforts.sleeping_bag.broke");
            self.level_events.push(LevelEvent::Sound {
                pos: BlockPos::new(head[0], head[1], head[2]),
                sound: "minecraft:block.wool.break".into(),
                volume: 1.0,
                pitch: 1.0,
            });
            let _ = self.process_level_events();
        } else if creative {
            remove(self, head);
            remove(self, foot);
        } else {
            // The foot first: the head then loses its other half and drops the item.
            remove(self, foot);
            remove(self, head);
        }
    }

    /// The guest `who` is, as (connection, creative mode).
    fn comforts_guest(&self, who: [i32; 4]) -> Option<(u32, bool)> {
        self.lan.as_ref().and_then(|h| {
            h.hub
                .guests
                .iter()
                .find(|g| horizon_client::lan::playerdata::uuid_ints(g.uuid) == who)
                .map(|g| (g.conn, g.creative()))
        })
    }
    /// Whether `who` plays in creative mode (the host's own player, else a guest).
    fn comforts_creative(&self, who: [i32; 4]) -> bool {
        match self.comforts_guest(who) {
            Some((_, creative)) => creative,
            None => self.player.is_some() && self.lan_acting.is_none() && self.creative,
        }
    }
    /// A line over the hotbar of `who`, who may be a guest outside their turn.
    fn comforts_tell_player(&mut self, who: [i32; 4], key: &str) {
        if let Some((conn, _)) = self.comforts_guest(who)
            && let Some(host) = self.lan.as_ref()
        {
            host.hub.send_to(
                conn,
                &horizon_protocol::packets::play::Clientbound::SystemChat(
                    horizon_protocol::packets::play::SystemChat {
                        content: horizon_protocol::text::translatable(key, &[]),
                        overlay: true,
                    },
                ),
            );
            return;
        }
        self.comforts_say(key);
    }

    /// `guest_sleep_tick` for a guest in a Comforts bed: the game wakes a sleeper whose bed is not
    /// named `*_bed`. `true`: handled here.
    pub(super) fn comforts_guest_sleep_tick(&mut self, host: &mut super::lan::LanHost, gi: usize) -> bool {
        if self.remote.is_some() {
            return false;
        }
        let Some(head) = host.hub.guests[gi].bed else {
            return false;
        };
        if self.comforts_bed_at(head).is_none() {
            return false;
        }
        let g = &mut host.hub.guests[gi];
        g.sleep_timer = super::tick_sleep_timer(g.sleep_timer, true);
        if g.dead || g.health <= 0.0 {
            host.hub.wake(gi);
        }
        true
    }
}

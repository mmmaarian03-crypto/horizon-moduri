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

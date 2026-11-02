// Presence Footsteps for Horizon: the rules, without any game code.
// Written from the 1.20 branch of Sollace/Hurricaaane's Presence Footsteps (MIT; sources cloned in
// `surse\presencefootsteps`; the Forge 1.0.0 jar of the pack is that branch): `TerrestrialStepSoundGenerator`,
// `MotionTracker`, `QuadrupedModifier`, `PFSolver`, `AssociationPool`, `StateLookup`, `AbstractSubstrateLookup`,
// `AcousticsFile` and the acoustics, `DelayedSoundPlayer`, `Variator`, `SoundEngine`. The winged generator is not
// ported: the pack's locomotionmap has no FLYING entity.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

// ---- a tiny JSON reader (the config files are Gson's) ----
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(v) => v.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Json::Num(n) => Some(*n as f32),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
}

struct Reader<'a> {
    s: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn skip(&mut self) {
        loop {
            while self.at < self.s.len() && self.s[self.at].is_ascii_whitespace() {
                self.at += 1;
            }
            if self.s[self.at..].starts_with(b"//") {
                while self.at < self.s.len() && self.s[self.at] != b'\n' {
                    self.at += 1;
                }
            } else if self.s[self.at..].starts_with(b"/*") {
                self.at += 2;
                while self.at < self.s.len() && !self.s[self.at..].starts_with(b"*/") {
                    self.at += 1;
                }
                self.at = (self.at + 2).min(self.s.len());
            } else {
                return;
            }
        }
    }
    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }
    fn value(&mut self) -> Option<Json> {
        self.skip();
        match self.peek()? {
            b'{' => {
                self.at += 1;
                let mut out = Vec::new();
                loop {
                    self.skip();
                    match self.peek()? {
                        b'}' => {
                            self.at += 1;
                            return Some(Json::Obj(out));
                        }
                        b',' => self.at += 1,
                        _ => {
                            let key = self.key()?;
                            self.skip();
                            if self.peek()? != b':' {
                                return None;
                            }
                            self.at += 1;
                            out.push((key, self.value()?));
                        }
                    }
                }
            }
            b'[' => {
                self.at += 1;
                let mut out = Vec::new();
                loop {
                    self.skip();
                    match self.peek()? {
                        b']' => {
                            self.at += 1;
                            return Some(Json::Arr(out));
                        }
                        b',' => self.at += 1,
                        _ => out.push(self.value()?),
                    }
                }
            }
            b'"' | b'\'' => self.string().map(Json::Str),
            _ => {
                let start = self.at;
                while self.at < self.s.len()
                    && !matches!(self.s[self.at], b',' | b'}' | b']' | b':')
                    && !self.s[self.at].is_ascii_whitespace()
                {
                    self.at += 1;
                }
                let word = std::str::from_utf8(&self.s[start..self.at]).ok()?;
                match word {
                    "true" => Some(Json::Bool(true)),
                    "false" => Some(Json::Bool(false)),
                    "null" => Some(Json::Null),
                    _ => word.parse::<f64>().ok().map(Json::Num),
                }
            }
        }
    }
    fn key(&mut self) -> Option<String> {
        if matches!(self.peek()?, b'"' | b'\'') {
            return self.string();
        }
        let start = self.at;
        while self.at < self.s.len() && (self.s[self.at].is_ascii_alphanumeric() || self.s[self.at] == b'_') {
            self.at += 1;
        }
        (self.at > start).then(|| String::from_utf8_lossy(&self.s[start..self.at]).into_owned())
    }
    fn string(&mut self) -> Option<String> {
        let quote = self.peek()?;
        self.at += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = self.peek()?;
            self.at += 1;
            if c == quote {
                return String::from_utf8(out).ok();
            }
            if c != b'\\' {
                out.push(c);
                continue;
            }
            let e = self.peek()?;
            self.at += 1;
            match e {
                b'n' => out.push(b'\n'),
                b't' => out.push(b'\t'),
                b'r' => out.push(b'\r'),
                b'b' => out.push(8),
                b'f' => out.push(12),
                b'u' => {
                    let hex = std::str::from_utf8(self.s.get(self.at..self.at + 4)?).ok()?;
                    let mut code = u32::from_str_radix(hex, 16).ok()?;
                    self.at += 4;
                    if (0xD800..0xDC00).contains(&code) && self.s[self.at..].starts_with(b"\\u") {
                        let low = std::str::from_utf8(self.s.get(self.at + 2..self.at + 6)?).ok()?;
                        let low = u32::from_str_radix(low, 16).ok()?;
                        if (0xDC00..0xE000).contains(&low) {
                            code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                            self.at += 6;
                        }
                    }
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(char::from_u32(code).unwrap_or('\u{FFFD}').encode_utf8(&mut buf).as_bytes());
                }
                other => out.push(other),
            }
        }
    }
}

pub fn parse_json(text: &str) -> Option<Json> {
    let mut r = Reader { s: text.as_bytes(), at: 0 };
    let v = r.value()?;
    r.skip();
    (r.at >= r.s.len()).then_some(v)
}

impl Json {
    pub fn entries(&self) -> &[(String, Json)] {
        match self {
            Json::Obj(v) => v,
            _ => &[],
        }
    }
    pub fn items(&self) -> &[Json] {
        match self {
            Json::Arr(v) => v,
            _ => &[],
        }
    }
    pub fn num(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }
}

// ---- small things: the random generator, the states, the options ----

/// `java.util.Random`'s role: any generator will do (the original seeds from the clock).
#[derive(Clone, Debug)]
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    /// `nextFloat`: [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    /// `nextInt(bound)`.
    pub fn next_int(&mut self, bound: u32) -> u32 {
        (self.next() >> 33) as u32 % bound.max(1)
    }
}

/// `MathUtil.randAB` (floats).
fn rand_ab(rng: &mut Rng, a: f32, b: f32) -> f32 {
    if a >= b { a } else { a + rng.next_f32() * (b - a) }
}
/// `MathUtil.randAB` (longs: `a + nextInt(b + 1)`, as the original writes it).
fn rand_ab_long(rng: &mut Rng, a: i64, b: i64) -> i64 {
    if a >= b { a } else { a + i64::from(rng.next_int((b + 1) as u32)) }
}
/// `MathUtil.scalex`.
pub fn scalex(number: f32, min: f32, max: f32) -> f32 {
    ((number - min) / (max - min)).clamp(0.0, 1.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum State {
    Stand,
    Walk,
    Wander,
    Swim,
    Run,
    Jump,
    Land,
    Climb,
    ClimbRun,
    Down,
    DownRun,
    Up,
    UpRun,
}
impl State {
    pub const ALL: [State; 13] = [
        State::Stand,
        State::Walk,
        State::Wander,
        State::Swim,
        State::Run,
        State::Jump,
        State::Land,
        State::Climb,
        State::ClimbRun,
        State::Down,
        State::DownRun,
        State::Up,
        State::UpRun,
    ];
    /// `State.getName`: the lower-case name of the Java enum constant.
    pub fn name(self) -> &'static str {
        match self {
            State::Stand => "stand",
            State::Walk => "walk",
            State::Wander => "wander",
            State::Swim => "swim",
            State::Run => "run",
            State::Jump => "jump",
            State::Land => "land",
            State::Climb => "climb",
            State::ClimbRun => "climb_run",
            State::Down => "down",
            State::DownRun => "down_run",
            State::Up => "up",
            State::UpRun => "up_run",
        }
    }
    /// The state an acoustic without this event falls back to (`destination`); itself when none.
    fn destination(self) -> State {
        match self {
            State::Run => State::Walk,
            State::Jump => State::Wander,
            State::Land => State::Run,
            State::Climb => State::Walk,
            State::ClimbRun => State::Run,
            State::Down => State::Walk,
            State::DownRun => State::Run,
            State::Up => State::Walk,
            State::UpRun => State::Run,
            other => other,
        }
    }
    fn can_transition(self) -> bool {
        self.destination() != self
    }
    fn is_extra_loud(self) -> bool {
        self == State::Run || self == State::Jump || self.destination() == State::Run
    }
}

/// `Options`: what an acoustic hands down to the sound it plays.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    pub gliding_volume: Option<f32>,
    pub gliding_pitch: Option<f32>,
    pub volume_percentage: Option<f32>,
    pub pitch_percentage: Option<f32>,
    pub delay: Option<(i64, i64)>,
}
impl Options {
    /// `and`: `other`'s keys win.
    fn and(self, other: Options) -> Options {
        Options {
            gliding_volume: other.gliding_volume.or(self.gliding_volume),
            gliding_pitch: other.gliding_pitch.or(self.gliding_pitch),
            volume_percentage: other.volume_percentage.or(self.volume_percentage),
            pitch_percentage: other.pitch_percentage.or(self.pitch_percentage),
            delay: other.delay.or(self.delay),
        }
    }
}

/// `Range`: percentages from the files, stored as fractions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub min: f32,
    pub max: f32,
}
impl Range {
    pub const DEFAULT: Range = Range { min: 1.0, max: 1.0 };
    fn percentage(obj: &Json, name: &str, fallback: f32) -> f32 {
        obj.get(name).and_then(Json::as_f32).map_or(fallback, |v| v / 100.0)
    }
    /// `Range.read(name, json)` on top of `self` as the defaults.
    fn read(self, name: &str, json: &Json) -> Range {
        if name == "volume" && (json.get("vol").is_some() || json.get("vol_min").is_some() || json.get("vol_max").is_some()) {
            return self.read("vol", json);
        }
        if let Some(element) = json.get(name) {
            if let Json::Obj(_) = element {
                return Range {
                    min: Self::percentage(element, "min", self.min),
                    max: Self::percentage(element, "max", self.max),
                };
            }
            let v = Self::percentage(json, name, self.min);
            return Range { min: v, max: v };
        }
        Range {
            min: Self::percentage(json, &format!("{name}_min"), self.min),
            max: Self::percentage(json, &format!("{name}_max"), self.max),
        }
    }
    fn random(self, rng: &mut Rng) -> f32 {
        rand_ab(rng, self.min, self.max)
    }
    fn on(self, value: f32) -> f32 {
        self.min + (self.max - self.min) * value
    }
}

// ---- acoustics ----

#[derive(Clone, Debug)]
pub enum Acoustic {
    Empty,
    Varying { sound: String, volume: Range, pitch: Range },
    Events(Vec<(State, Acoustic)>),
    Simultaneous(Vec<Acoustic>),
    Delayed { acoustic: Box<Acoustic>, min: i64, max: i64 },
    /// Each entry's share of the total weight.
    Weighted(Vec<(f32, Acoustic)>),
    Chance { acoustic: Box<Acoustic>, probability: f32 },
}

/// `AcousticsFile`: the defaults and the sound root of a library.
struct Context {
    volume: Range,
    pitch: Range,
    root: String,
}
impl Context {
    /// `getSoundName`.
    fn sound_name(&self, name: &str) -> String {
        match name.strip_prefix('@') {
            None => format!("{}{}", self.root, name),
            Some(_) => name.replace('@', ""),
        }
    }
}

fn acoustic_type(json: &Json, default: &str) -> &'static str {
    let t = match json {
        Json::Obj(_) => json.get("type").and_then(Json::as_str).unwrap_or(default),
        Json::Arr(_) => "simultaneous",
        Json::Str(_) => "basic",
        _ => "",
    };
    match t {
        "basic" => "basic",
        "events" => "events",
        "simultaneous" => "simultaneous",
        "delayed" => "delayed",
        "probability" => "probability",
        "chance" => "chance",
        _ => "",
    }
}

fn varying(json: &Json, cx: &Context) -> Option<Acoustic> {
    if let Json::Str(s) = json {
        return Some(Acoustic::Varying { sound: cx.sound_name(s), volume: cx.volume, pitch: cx.pitch });
    }
    let name = match json.get("name").and_then(Json::as_str) {
        Some(n) if !n.is_empty() => n,
        _ => return Some(Acoustic::Empty),
    };
    Some(Acoustic::Varying {
        sound: cx.sound_name(name),
        volume: cx.volume.read("volume", json),
        pitch: cx.pitch.read("pitch", json),
    })
}

fn read_acoustic(json: &Json, cx: &Context, default: &str) -> Option<Acoustic> {
    match acoustic_type(json, default) {
        "basic" => varying(json, cx),
        "events" => {
            let mut pairs = Vec::new();
            for state in State::ALL {
                if let Some(v) = json.get(state.name()) {
                    pairs.push((state, read_acoustic(v, cx, "basic")?));
                }
            }
            Some(Acoustic::Events(pairs))
        }
        "simultaneous" => {
            let list = if let Json::Arr(a) = json { a.as_slice() } else { json.get("array")?.items() };
            Some(Acoustic::Simultaneous(
                list.iter().map(|i| read_acoustic(i, cx, "basic")).collect::<Option<Vec<_>>>()?,
            ))
        }
        "delayed" => {
            let inner = if json.get("name").is_some() { varying(json, cx)? } else { read_acoustic(json.get("acoustic")?, cx, "basic")? };
            let (min, max) = if let Some(d) = json.get("delay").and_then(Json::num) {
                (d as i64, d as i64)
            } else {
                (
                    json.get("delay_min").and_then(Json::num).map_or(0, |v| v as i64),
                    json.get("delay_max").and_then(Json::num).map_or(0, |v| v as i64),
                )
            };
            Some(Acoustic::Delayed { acoustic: Box::new(inner), min, max })
        }
        "probability" => {
            let list = json.get("array").or_else(|| json.get("entries"))?.items();
            let mut raw = Vec::new();
            let mut it = list.iter();
            while let Some(w) = it.next() {
                let weight = w.num()? as i32;
                let next = it.next()?; // an odd count is an error in the original
                if weight < 0 {
                    return None;
                }
                raw.push((weight as f32, read_acoustic(next, cx, "basic")?));
            }
            let total: f32 = raw.iter().map(|e| e.0).sum();
            Some(Acoustic::Weighted(raw.into_iter().map(|(w, a)| (w / total, a)).collect()))
        }
        "chance" => Some(Acoustic::Chance {
            acoustic: Box::new(read_acoustic(json.get("acoustic")?, cx, "basic")?),
            probability: json.get("probability")?.as_f32()?,
        }),
        _ => None,
    }
}

/// Which sort of entity makes a sound (the volume sliders and the mono sounds follow it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The player you play.
    Local,
    OtherPlayer,
    Hostile,
    Passive,
}

/// A sound ready to go to the game.
#[derive(Clone, Debug, PartialEq)]
pub struct Emission {
    pub entity: u32,
    pub pos: [f64; 3],
    /// A full id (`minecraft:block.grass.step`) or one of ours: `presencefootsteps:pf_presence.grass.grass_walk`
    /// (`presencefootstepsmono:` for the players other than the one you play, as `UncappedSoundInstance` does).
    pub sound: String,
    pub volume: f32,
    pub pitch: f32,
}

/// What the original's `DelayedSoundPlayer` holds: a sound with the moment to play it.
#[derive(Clone, Debug)]
pub struct Pending {
    ctx: EntityCtx,
    sound: String,
    volume: f32,
    pitch: f32,
    options: Options,
    time_to_play: i64,
    maximum: i64,
}
/// The little the sound player needs of the entity a sound comes from.
#[derive(Clone, Debug)]
pub struct EntityCtx {
    pub id: u32,
    pub pos: [f64; 3],
    pub kind: Kind,
    pub scale: f32,
    /// `getSpeedScalingRatio` of its generator.
    pub running: f32,
}

struct Voice<'a> {
    entity: &'a EntityCtx,
    out: &'a mut Vec<Pending>,
    rng: &'a mut Rng,
    now: i64,
}

impl Acoustic {
    fn play(&self, v: &mut Voice, state: State, options: Options) {
        match self {
            Acoustic::Empty => {}
            Acoustic::Varying { sound, volume, pitch } => {
                if sound.is_empty() {
                    return;
                }
                let vol = match options.gliding_volume {
                    Some(g) => volume.on(g),
                    None => volume.random(v.rng),
                };
                let pit = match options.gliding_pitch {
                    Some(g) => pitch.on(g),
                    None => pitch.random(v.rng),
                };
                // DelayedSoundPlayer.playSound: with no delay_min/delay_max the sound goes at once.
                let (maximum, time_to_play) = match options.delay {
                    Some((lo, hi)) => (hi, v.now + rand_ab_long(v.rng, lo, hi)),
                    None => (0, v.now),
                };
                v.out.push(Pending {
                    ctx: v.entity.clone(),
                    sound: sound.clone(),
                    volume: vol,
                    pitch: pit,
                    options,
                    time_to_play,
                    maximum,
                });
            }
            Acoustic::Events(pairs) => {
                if let Some((_, a)) = pairs.iter().find(|p| p.0 == state) {
                    a.play(v, state, options);
                } else if state.can_transition() {
                    self.play(v, state.destination(), options);
                }
            }
            Acoustic::Simultaneous(list) => {
                for a in list {
                    a.play(v, state, options);
                }
            }
            Acoustic::Delayed { acoustic, min, max } => {
                acoustic.play(v, state, options.and(Options { delay: Some((*min, *max)), ..Options::default() }));
            }
            Acoustic::Weighted(entries) => {
                let rand = v.rng.next_f32();
                // The threshold of the original is each entry's own share, not a running sum: as written.
                for (threshold, a) in entries {
                    if *threshold >= rand {
                        a.play(v, state, options);
                        return;
                    }
                }
            }
            Acoustic::Chance { acoustic, probability } => {
                if v.rng.next_f32() * 100.0 <= *probability {
                    acoustic.play(v, state, options);
                }
            }
        }
    }
}

// ---- the lookups: which acoustic a block, a sound type, a golem or an entity gets ----

/// `SoundsKey`: the names of acoustics a block (or a sound type) plays, or one of three special values.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundsKey {
    pub raw: String,
    names: Vec<String>,
    kind: KeyKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyKind {
    Unassigned,
    NonEmitter,
    MessyGround,
    Normal,
}
impl SoundsKey {
    pub fn unassigned() -> Self {
        SoundsKey { raw: "UNASSIGNED".into(), names: Vec::new(), kind: KeyKind::Unassigned }
    }
    pub fn non_emitter() -> Self {
        SoundsKey { raw: "NOT_EMITTER".into(), names: Vec::new(), kind: KeyKind::NonEmitter }
    }
    fn messy_ground() -> Self {
        SoundsKey { raw: "MESSY_GROUND".into(), names: Vec::new(), kind: KeyKind::MessyGround }
    }
    pub fn of(names: &str) -> Self {
        match names {
            "MESSY_GROUND" => Self::messy_ground(),
            "UNASSIGNED" => Self::unassigned(),
            "NOT_EMITTER" => Self::non_emitter(),
            _ => {
                let mut list: Vec<String> = Vec::new();
                for n in names.split(',').filter(|s| !s.is_empty()) {
                    if !list.iter().any(|x| x == n) {
                        list.push(n.to_owned());
                    }
                }
                SoundsKey { raw: names.to_owned(), names: list, kind: KeyKind::Normal }
            }
        }
    }
    pub fn is_result(&self) -> bool {
        self.kind != KeyKind::Unassigned
    }
    pub fn is_silent(&self) -> bool {
        self.kind == KeyKind::NonEmitter
    }
    pub fn is_emitter(&self) -> bool {
        !self.is_silent()
    }
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

pub mod substrate {
    pub const DEFAULT: &str = "";
    pub const CARPET: &str = "carpet";
    pub const WET: &str = "wet";
    pub const FENCE: &str = "bigger";
    pub const FOLIAGE: &str = "foliage";
    pub const MESSY: &str = "messy";
    pub fn is_supplementary(s: &str) -> bool {
        matches!(s, WET | FOLIAGE | MESSY)
    }
}

/// A block state as the rules read it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlockInfo {
    /// `minecraft:grass_block`.
    pub id: String,
    pub props: Vec<(String, String)>,
    pub air: bool,
    /// `BlockState.isLiquid` (water, lava, bubble column).
    pub liquid: bool,
    /// Its fluid is lava / water (waterlogged included).
    pub lava_fluid: bool,
    pub water_fluid: bool,
    /// `isSideSolidFullSquare(UP)`.
    pub full_top: bool,
    /// Its tags (`minecraft:planks`, ...), for the `#tag` keys.
    pub tags: Vec<String>,
    /// The step sound id (`minecraft:block.grass.step`), volume and pitch of its sound type.
    pub step: String,
    pub volume: f32,
    pub pitch: f32,
    /// Boxes in block space, `[minx, miny, minz, maxx, maxy, maxz]`.
    pub collision: Vec<[f64; 6]>,
    pub outline: Vec<[f64; 6]>,
}

fn max_y(boxes: &[[f64; 6]]) -> f64 {
    boxes.iter().map(|b| b[4]).fold(f64::NEG_INFINITY, f64::max)
}

#[derive(Clone, Debug, PartialEq)]
struct Attribute {
    name: String,
    value: String,
}

#[derive(Clone, Debug)]
struct Key {
    id: String,
    substrate: String,
    properties: Vec<Attribute>,
    value: SoundsKey,
    is_tag: bool,
    is_wildcard: bool,
}
impl Key {
    /// `Key.of`: `#tag`, `*`, `block[prop=value].substrate`.
    fn of(key: &str, value: SoundsKey) -> Key {
        let (is_tag, key) = match key.strip_prefix('#') {
            Some(rest) => (true, rest),
            None => (false, key),
        };
        let id_end = key.find(['.', '[']).unwrap_or(key.len());
        let id = &key[..id_end];
        let is_wildcard = id.starts_with('*');
        // A `^` (metadata, 1.12 style) is ignored.
        let identifier = if is_wildcard { "air".to_owned() } else { renamed(id.split('^').next().unwrap_or("")) };
        let rest = &key[id_end..];
        // The substrate is what is left once the first `[...]` is taken out.
        let without_props = match (rest.find('['), rest.find(']')) {
            (Some(a), Some(b)) if b > a => format!("{}{}", &rest[..a], &rest[b + 1..]),
            _ => rest.to_owned(),
        };
        let mut final_substrate = String::new();
        let mut rest = rest.to_owned();
        if without_props.contains('.') {
            final_substrate = without_props.split('.').nth(1).unwrap_or("").to_owned();
            rest = rest.replacen(&without_props, "", 1);
        }
        let properties = rest
            .replace(['[', ']'], "")
            .split(',')
            .filter(|l| l.contains('='))
            .filter_map(|l| {
                let mut it = l.split('=');
                Some(Attribute { name: it.next()?.to_owned(), value: it.next()?.to_owned() })
            })
            .collect();
        Key { id: identifier, substrate: final_substrate, properties, value, is_tag, is_wildcard }
    }
    fn empty(&self) -> bool {
        self.properties.is_empty()
    }
    fn matches(&self, state: &BlockInfo) -> bool {
        if self.empty() {
            return true;
        }
        for p in &self.properties {
            for (k, v) in &state.props {
                if *k == p.name && !v.eq_ignore_ascii_case(&p.value) {
                    return false;
                }
            }
        }
        true
    }
    /// `Key.equals`: what makes a later line replace an earlier one.
    fn same(&self, o: &Key) -> bool {
        self.is_tag == o.is_tag
            && self.is_wildcard == o.is_wildcard
            && self.id == o.id
            && self.substrate == o.substrate
            && self.properties.len() == o.properties.len()
            && self.properties.iter().all(|a| o.properties.contains(a))
    }
}

/// `KeyList`: the keys with properties first, then the ones without.
#[derive(Default)]
struct KeyList {
    priority: Vec<Key>,
    plain: Vec<Key>,
}
impl KeyList {
    fn add(&mut self, key: Key) {
        let set = if key.empty() { &mut self.plain } else { &mut self.priority };
        set.retain(|k| !k.same(&key));
        set.push(key);
    }
    fn find(&self, state: &BlockInfo) -> Option<&Key> {
        self.priority.iter().chain(self.plain.iter()).find(|k| k.matches(state))
    }
}

#[derive(Default)]
struct SubstrateBucket {
    wildcards: KeyList,
    blocks: HashMap<String, KeyList>,
    /// In file order (the first tag the block is in wins).
    tags: Vec<(String, KeyList)>,
}

/// `StateLookup`: block keys by substrate.
#[derive(Default)]
pub struct StateLookup {
    substrates: HashMap<String, SubstrateBucket>,
}
impl StateLookup {
    pub fn add(&mut self, key: &str, value: &str) {
        let sound = SoundsKey::of(value);
        if !sound.is_result() {
            return;
        }
        let k = Key::of(key, sound);
        let bucket = self.substrates.entry(k.substrate.clone()).or_default();
        if k.is_wildcard {
            bucket.wildcards.add(k);
        } else if k.is_tag {
            match bucket.tags.iter_mut().find(|(id, _)| *id == k.id) {
                Some((_, list)) => list.add(k),
                None => {
                    let id = k.id.clone();
                    let mut list = KeyList::default();
                    list.add(k);
                    bucket.tags.push((id, list));
                }
            }
        } else {
            bucket.blocks.entry(k.id.clone()).or_default().add(k);
        }
    }
    pub fn get(&self, state: &BlockInfo, substrate: &str) -> SoundsKey {
        let Some(bucket) = self.substrates.get(substrate) else {
            return SoundsKey::unassigned();
        };
        // getTile: the block's own keys, else the first tag it is in.
        let tile = bucket.blocks.get(&state.id).or_else(|| {
            bucket
                .tags
                .iter()
                .find(|(tag, _)| state.tags.iter().any(|t| t == tag))
                .map(|(_, l)| l)
        });
        let found = tile.and_then(|l| l.find(state)).or_else(|| bucket.wildcards.find(state));
        found.map_or_else(SoundsKey::unassigned, |k| k.value.clone())
    }
}

/// `AbstractSubstrateLookup`: the primitive map (step sounds) and the golem map (entity types).
#[derive(Default)]
pub struct SubstrateLookup {
    substrates: Vec<(String, HashMap<String, SoundsKey>)>,
}
impl SubstrateLookup {
    pub fn add(&mut self, key: &str, value: &str) {
        let key = key.trim();
        let mut it = key.split('@');
        let primitive = &renamed(it.next().unwrap_or(""));
        let sub = it.next().unwrap_or(substrate::DEFAULT);
        let idx = match self.substrates.iter().position(|(s, _)| s == sub) {
            Some(i) => i,
            None => {
                self.substrates.push((sub.to_owned(), HashMap::new()));
                self.substrates.len() - 1
            }
        };
        self.substrates[idx].1.insert(primitive.to_owned(), SoundsKey::of(value));
    }
    fn map(&self, id: &str, sub: &str) -> Option<&HashMap<String, SoundsKey>> {
        let find = |name: &str| self.substrates.iter().find(|(s, _)| s == name).map(|(_, m)| m);
        let path = id.split_once(':').map_or(id, |(_, p)| p);
        find(sub).or_else(|| find(&format!("break_{path}"))).or_else(|| find(substrate::DEFAULT))
    }
    pub fn get(&self, id: &str, sub: &str) -> SoundsKey {
        self.map(id, sub).and_then(|m| m.get(id)).cloned().unwrap_or_else(SoundsKey::unassigned)
    }
    pub fn contains(&self, id: &str) -> bool {
        self.substrates.iter().any(|(_, m)| m.contains_key(id))
    }
}

/// `PrimitiveLookup.getSubstrate`: `%.2f_%.2f` of the sound type's volume and pitch.
pub fn primitive_substrate(volume: f32, pitch: f32) -> String {
    format!("{volume:.2}_{pitch:.2}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locomotion {
    None,
    Biped,
    Quadruped,
    Flying,
    FlyingBiped,
}
impl Locomotion {
    /// `byName`: the name or its ordinal, else BIPED.
    pub fn by_name(name: &str) -> Locomotion {
        match name {
            "NONE" | "0" => Locomotion::None,
            "BIPED" | "1" => Locomotion::Biped,
            "QUADRUPED" | "2" => Locomotion::Quadruped,
            "FLYING" | "3" => Locomotion::Flying,
            "FLYING_BIPED" | "4" => Locomotion::FlyingBiped,
            _ => Locomotion::Biped,
        }
    }
}

/// `Variator`: the numbers of the walking model (`config/variator.json` overrides them by field name).
#[derive(Clone, Debug, PartialEq)]
pub struct Variator {
    pub immobile_duration: i64,
    pub immobile_interval_min: i64,
    pub immobile_interval_max: i64,
    pub event_on_jump: bool,
    pub land_hard_distance_min: f32,
    pub speed_to_jump_as_multifoot: f32,
    pub speed_to_run: f32,
    pub running_rampup_begin: f32,
    pub running_rampup_end: f32,
    pub distance_human: f32,
    pub distance_stair: f32,
    pub distance_ladder: f32,
    pub play_wander: bool,
}
impl Default for Variator {
    fn default() -> Self {
        Variator {
            immobile_duration: 200,
            immobile_interval_min: 500,
            immobile_interval_max: 3000,
            event_on_jump: true,
            land_hard_distance_min: 0.9,
            speed_to_jump_as_multifoot: 0.005,
            speed_to_run: 0.022,
            running_rampup_begin: 0.001,
            running_rampup_end: 0.022,
            distance_human: 0.95,
            distance_stair: 0.95 * 0.65,
            distance_ladder: 0.5,
            play_wander: true,
        }
    }
}
impl Variator {
    fn load(&mut self, json: &Json) {
        for (k, v) in json.entries() {
            let n = v.num();
            match (k.as_str(), n, v.as_bool()) {
                ("IMMOBILE_DURATION", Some(n), _) => self.immobile_duration = n as i64,
                ("IMOBILE_INTERVAL_MIN", Some(n), _) => self.immobile_interval_min = n as i64,
                ("IMOBILE_INTERVAL_MAX", Some(n), _) => self.immobile_interval_max = n as i64,
                ("EVENT_ON_JUMP", _, Some(b)) => self.event_on_jump = b,
                ("LAND_HARD_DISTANCE_MIN", Some(n), _) => self.land_hard_distance_min = n as f32,
                ("SPEED_TO_JUMP_AS_MULTIFOOT", Some(n), _) => self.speed_to_jump_as_multifoot = n as f32,
                ("SPEED_TO_RUN", Some(n), _) => self.speed_to_run = n as f32,
                ("RUNNING_RAMPUP_BEGIN", Some(n), _) => self.running_rampup_begin = n as f32,
                ("RUNNING_RAMPUP_END", Some(n), _) => self.running_rampup_end = n as f32,
                ("DISTANCE_HUMAN", Some(n), _) => self.distance_human = n as f32,
                ("DISTANCE_STAIR", Some(n), _) => self.distance_stair = n as f32,
                ("DISTANCE_LADDER", Some(n), _) => self.distance_ladder = n as f32,
                ("PLAY_WANDER", _, Some(b)) => self.play_wander = b,
                _ => {}
            }
        }
    }
}

/// The text of the carried files (`assets/presencefootsteps/config/*.json`).
#[derive(Default, Clone)]
pub struct Files {
    pub blockmap: String,
    pub golemmap: String,
    pub primitivemap: String,
    pub locomotionmap: String,
    pub acoustics: String,
    pub variator: String,
}

/// `Isolator`: everything loaded from the files.
#[derive(Default)]
pub struct Isolator {
    pub variator: Variator,
    pub blocks: StateLookup,
    pub primitives: SubstrateLookup,
    pub golems: SubstrateLookup,
    pub locomotions: HashMap<String, Locomotion>,
    pub acoustics: HashMap<String, Acoustic>,
}
impl Isolator {
    /// `Isolator.load`: whether any configuration was there. A file that does not parse is skipped.
    pub fn load(files: &Files) -> (Isolator, bool) {
        let mut iso = Isolator::default();
        let mut any = false;
        let load = |text: &str, any: &mut bool, mut add: Box<dyn FnMut(&str, &str) + '_>| {
            if let Some(Json::Obj(entries)) = parse_json(text) {
                *any = true;
                for (k, v) in &entries {
                    if let Some(s) = v.as_str() {
                        add(k, s);
                    }
                }
            }
        };
        load(&files.blockmap, &mut any, Box::new(|k, v| iso.blocks.add(k, v)));
        load(&files.golemmap, &mut any, Box::new(|k, v| iso.golems.add(k, v)));
        load(&files.primitivemap, &mut any, Box::new(|k, v| iso.primitives.add(k, v)));
        let mut loco = HashMap::new();
        load(
            &files.locomotionmap,
            &mut any,
            Box::new(|k, v| {
                loco.insert(k.to_owned(), Locomotion::by_name(&v.to_uppercase()));
            }),
        );
        iso.locomotions = loco;
        if let Some(json) = parse_json(&files.acoustics) {
            any = true;
            iso.read_acoustics(&json);
        }
        if let Some(json) = parse_json(&files.variator) {
            any = true;
            iso.variator.load(&json);
        }
        (iso, any)
    }

    /// `AcousticsFile.read`: the library's defaults, sound root and `contents`. Returns whether it was valid.
    fn read_acoustics(&mut self, json: &Json) -> bool {
        // engineversion 2, type library.
        if json.get("type").and_then(Json::as_str) != Some("library") {
            return false;
        }
        let Some(contents) = json.get("contents") else { return false };
        let root = json.get("soundroot").and_then(Json::as_str).unwrap_or("").to_owned();
        let (volume, pitch) = match json.get("defaults") {
            Some(d) => (Range::DEFAULT.read("volume", d), Range::DEFAULT.read("pitch", d)),
            None => (Range::DEFAULT, Range::DEFAULT),
        };
        let cx = Context { volume, pitch, root };
        for (name, value) in contents.entries() {
            if let Some(a) = read_acoustic(value, &cx, "events") {
                self.acoustics.insert(name.clone(), a);
            }
        }
        true
    }

    pub fn locomotion(&self, type_id: &str) -> Locomotion {
        self.locomotions.get(type_id).copied().unwrap_or(Locomotion::Biped)
    }
}

// ---- settings (`PFConfig`) ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selector {
    All,
    PlayersAndHostiles,
    PlayersOnly,
}

/// `PFConfig` with the pack's `config/presencefootsteps/userconfig.json` as the defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub volume: i32,
    pub client_player_volume: i32,
    pub other_player_volume: i32,
    pub hostile_entities_volume: i32,
    pub passive_entities_volume: i32,
    pub running_volume_increase: i32,
    pub wet_sounds_volume: i32,
    pub foliage_sounds_volume: i32,
    pub max_stepping_entities: i32,
    pub disabled: bool,
    pub multiplayer: bool,
    pub footwear: bool,
    /// The stance for players (`NONE` = automatic = biped).
    pub stance: Locomotion,
    pub target_entities: Selector,
}
impl Default for Config {
    fn default() -> Self {
        Config {
            volume: 70,
            client_player_volume: 100,
            other_player_volume: 100,
            hostile_entities_volume: 100,
            passive_entities_volume: 100,
            running_volume_increase: 0,
            wet_sounds_volume: 50,
            foliage_sounds_volume: 100,
            max_stepping_entities: 50,
            disabled: false,
            multiplayer: true,
            footwear: true,
            stance: Locomotion::None,
            target_entities: Selector::All,
        }
    }
}
impl Config {
    /// The file's keys over the defaults (a key that is not there or is not the right type is left alone).
    pub fn read(text: &str) -> Config {
        let mut c = Config::default();
        let Some(json) = parse_json(text) else { return c };
        let int = |k: &str| json.get(k).and_then(Json::num).map(|n| n as i32);
        let flag = |k: &str| json.get(k).and_then(Json::as_bool);
        macro_rules! set {
            ($f:ident, $k:expr) => {
                if let Some(v) = int($k) {
                    c.$f = v;
                }
            };
        }
        set!(volume, "volume");
        set!(client_player_volume, "clientPlayerVolume");
        set!(other_player_volume, "otherPlayerVolume");
        set!(hostile_entities_volume, "hostileEntitiesVolume");
        set!(passive_entities_volume, "passiveEntitiesVolume");
        set!(running_volume_increase, "runningVolumeIncrease");
        set!(wet_sounds_volume, "wetSoundsVolume");
        set!(foliage_sounds_volume, "foliageSoundsVolume");
        set!(max_stepping_entities, "maxSteppingEntities");
        if let Some(b) = flag("disabled") {
            c.disabled = b;
        }
        if let Some(b) = flag("multiplayer") {
            c.multiplayer = b;
        }
        if let Some(b) = flag("footwear") {
            c.footwear = b;
        }
        if let Some(s) = json.get("stance").and_then(Json::as_str) {
            c.stance = Locomotion::by_name(&s.to_uppercase());
        }
        c.target_entities = match json.get("targetEntities").and_then(Json::as_str) {
            Some("PLAYERS_AND_HOSTILES") => Selector::PlayersAndHostiles,
            Some("PLAYERS_ONLY") => Selector::PlayersOnly,
            _ => Selector::All,
        };
        c
    }
    fn pct(v: i32) -> f32 {
        v.clamp(0, 100) as f32 / 100.0
    }
    fn global_volume(&self) -> i32 {
        self.volume.clamp(0, 100)
    }
    /// `getEnabled`: not disabled and not muted.
    pub fn enabled(&self) -> bool {
        !self.disabled && self.global_volume() > 0
    }
    fn selects(&self, e: &Ent) -> bool {
        match self.target_entities {
            Selector::All => true,
            Selector::PlayersAndHostiles => matches!(e.kind, Kind::Local | Kind::OtherPlayer | Kind::Hostile),
            Selector::PlayersOnly => matches!(e.kind, Kind::Local | Kind::OtherPlayer),
        }
    }
}

// ---- the world and the entities as the rules read them ----

pub trait World {
    fn block(&self, p: [i32; 3]) -> Rc<BlockInfo>;
    /// `World.hasRain`.
    fn rain_at(&self, p: [i32; 3]) -> bool;
    /// The entity types in the box of the block, expanded 0.5 in x and z, that are not collidable or whose top
    /// is below `feet_y + 0.2` (boats, minecarts, armor stands, shulkers...), `AssociationPool.get`.
    fn golems_at(&self, p: [i32; 3], feet_y: f64) -> Vec<String>;
    fn time(&self) -> i64;
}

/// An entity as the generator reads it, once a tick.
#[derive(Clone, Debug)]
pub struct Ent {
    pub id: u32,
    pub type_id: String,
    pub kind: Kind,
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    pub yaw: f32,
    pub width: f32,
    pub height: f32,
    /// `PlayerUtil.getScale`.
    pub scale: f32,
    pub on_ground: bool,
    pub touching_water: bool,
    pub submerged_water: bool,
    pub in_lava: bool,
    pub submerged_lava: bool,
    pub climbing: bool,
    pub sprinting: bool,
    pub sneaking: bool,
    pub fall_distance: f32,
    pub jumping: bool,
    /// The boots' equip sound (`minecraft:item.armor.equip_iron`), if boots are worn.
    pub boots: Option<String>,
    /// The vanilla walked distance of the player you play (`distanceTraveled`); None for the others.
    pub move_dist: Option<f32>,
}
impl Ent {
    fn is_player(&self) -> bool {
        matches!(self.kind, Kind::Local | Kind::OtherPlayer)
    }
    fn min_y(&self) -> f64 {
        self.pos[1]
    }
}

// ---- the solver (`PFSolver`) ----

#[derive(Clone, Debug)]
pub struct Association {
    state: Rc<BlockInfo>,
    pos: [i32; 3],
    dry: SoundsKey,
    wet: SoundsKey,
    foliage: SoundsKey,
    /// `Association.NOT_EMITTER`.
    none: bool,
}
impl Association {
    fn none() -> Association {
        Association {
            state: Rc::new(BlockInfo { air: true, ..BlockInfo::default() }),
            pos: [0; 3],
            dry: SoundsKey::non_emitter(),
            wet: SoundsKey::non_emitter(),
            foliage: SoundsKey::non_emitter(),
            none: true,
        }
    }
    fn of(state: Rc<BlockInfo>, pos: [i32; 3], dry: SoundsKey, wet: SoundsKey, foliage: SoundsKey) -> Association {
        if dry.is_silent() && wet.is_silent() && foliage.is_silent() {
            return Association::none();
        }
        Association { state, pos, dry, wet, foliage, none: false }
    }
    fn is_result(&self) -> bool {
        self.dry.is_result() || self.wet.is_result() || self.foliage.is_result()
    }
    fn is_silent(&self) -> bool {
        self.none || self.state.air
    }
    fn data_equals(&self, other: &Association) -> bool {
        self.dry == other.dry
    }
}

#[derive(Clone, Copy)]
struct Box3 {
    min: [f64; 3],
    max: [f64; 3],
}
impl Box3 {
    fn expand(self, x: f64, y: f64, z: f64) -> Box3 {
        Box3 {
            min: [self.min[0] - x, self.min[1] - y, self.min[2] - z],
            max: [self.max[0] + x, self.max[1] + y, self.max[2] + z],
        }
    }
    fn intersects(self, o: Box3) -> bool {
        (0..3).all(|i| self.min[i] < o.max[i] && self.max[i] > o.min[i])
    }
}

/// What a lookup needs besides the world.
struct Cx<'a> {
    iso: &'a Isolator,
    cfg: &'a Config,
    world: &'a dyn World,
    rng: &'a mut Rng,
    pending: &'a mut Vec<Pending>,
    now: i64,
}

fn collider(e: &Ent) -> Box3 {
    // getBoundingBox, lowered to the block's floor, grown by 0.1, and by more when sprinting.
    let half = f64::from(e.width) / 2.0;
    let min_y = e.min_y();
    let mut b = Box3 {
        min: [e.pos[0] - half, min_y - (min_y - min_y.floor()), e.pos[2] - half],
        max: [e.pos[0] + half, min_y - (min_y - min_y.floor()) + f64::from(e.height), e.pos[2] + half],
    }
    .expand(0.1, 0.1, 0.1);
    if e.sprinting {
        b = b.expand(0.3, 0.5, 0.3);
    }
    b
}

fn check_collision(state: &BlockInfo, pos: [i32; 3], c: Box3) -> bool {
    let shape = if state.collision.is_empty() { &state.outline } else { &state.collision };
    if shape.is_empty() {
        return true;
    }
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for b in shape {
        for i in 0..3 {
            min[i] = min[i].min(b[i]);
            max[i] = max[i].max(b[i + 3]);
        }
    }
    let p = pos.map(f64::from);
    Box3 { min: [min[0] + p[0], min[1] + p[1], min[2] + p[2]], max: [max[0] + p[0], max[1] + p[1], max[2] + p[2]] }
        .intersects(c)
}

fn up(p: [i32; 3]) -> [i32; 3] {
    [p[0], p[1] + 1, p[2]]
}
fn down(p: [i32; 3]) -> [i32; 3] {
    [p[0], p[1] - 1, p[2]]
}

impl Cx<'_> {
    /// `AssociationPool.get`.
    fn get(&self, e: &Ent, pos: [i32; 3], state: &BlockInfo, sub: &str) -> SoundsKey {
        for golem in self.world.golems_at(pos, e.pos[1]) {
            let a = self.iso.golems.get(&golem, sub);
            if a.is_emitter() {
                return a;
            }
        }
        if state.air {
            return SoundsKey::unassigned();
        }
        let a = self.iso.blocks.get(state, sub);
        if a.is_result() {
            return a;
        }
        // getForPrimitive: the sound type's step sound (never for the supplementary substrates).
        if !substrate::is_supplementary(sub) {
            let a = self.iso.primitives.get(&state.step, &primitive_substrate(state.volume, state.pitch));
            if a.is_result() {
                return a;
            }
        }
        SoundsKey::unassigned()
    }

    /// `PFSolver.findAssociation(pool, entity, pos, MESSY_FOLIAGE_STRATEGY)`.
    fn find_messy(&self, e: &Ent, pos: [i32; 3]) -> Association {
        let pos = up(pos);
        let above = self.world.block(pos);
        let foliage = self.iso.blocks.get(&above, substrate::FOLIAGE);
        let _ = e;
        if foliage.is_emitter() && self.iso.blocks.get(&above, substrate::MESSY).kind == KeyKind::MessyGround {
            return Association::of(above, pos, SoundsKey::non_emitter(), SoundsKey::non_emitter(), foliage);
        }
        Association::none()
    }

    /// The inner `findAssociation(pool, entity, pos, collider)`; moves `pos` as the original does.
    #[allow(unused_assignments)]
    fn inner(&self, e: &Ent, pos: &mut [i32; 3], c: Box3) -> Association {
        let mut target = self.world.block(*pos);
        *pos = up(*pos);
        let has_rain = self.world.rain_at(*pos);
        let mut carpet = self.world.block(*pos);
        let valid_carpet = !carpet.outline.is_empty() && max_y(&carpet.outline) < f64::from(0.3f32);
        let mut association = SoundsKey::unassigned();
        let mut foliage = SoundsKey::unassigned();
        let mut wet = SoundsKey::unassigned();
        let carpet_key = if valid_carpet { self.get(e, *pos, &carpet, substrate::CARPET) } else { SoundsKey::unassigned() };
        if valid_carpet && carpet_key.is_emitter() {
            association = carpet_key;
            target = carpet.clone();
        } else {
            association = SoundsKey::unassigned();
            *pos = down(*pos);
            if target.air {
                *pos = down(*pos);
                let fence = self.world.block(*pos);
                if check_collision(&fence, *pos, c) {
                    association = self.get(e, *pos, &fence, substrate::FENCE);
                    if association.is_result() {
                        carpet = target.clone();
                        target = fence;
                    } else {
                        *pos = up(*pos);
                    }
                }
            }
            if !association.is_result() {
                association = self.get(e, *pos, &target, substrate::DEFAULT);
            }
            if self.cfg.foliage_sounds_volume > 0
                && (e.boots.is_none() || e.sprinting)
                && association.is_emitter()
                && carpet.collision.is_empty()
            {
                *pos = up(*pos);
                foliage = self.get(e, *pos, &carpet, substrate::FOLIAGE);
                *pos = down(*pos);
            }
        }
        if association.is_result() && !check_collision(&target, *pos, c) {
            association = SoundsKey::unassigned();
        }
        if association.is_emitter()
            && (has_rain
                || (target.water_fluid && !target.full_top)
                || (carpet.water_fluid && !carpet.full_top))
        {
            wet = self.get(e, *pos, &target, substrate::WET);
        }
        Association::of(target, *pos, association, wet, foliage)
    }

    /// The outer `findAssociation(pool, entity, collider, originalFootPos, pos)`.
    fn around(&self, e: &Ent, c: Box3, original: [i32; 3]) -> Association {
        let mut pos = original;
        let mut association = self.inner(e, &mut pos, c);
        if association.is_result() && !association.state.liquid {
            return association;
        }
        let radius = 0.4;
        let xs = [(c.min[0] - radius).floor() as i32, pos[0], (c.max[0] + radius).floor() as i32];
        let zs = [(c.min[2] - radius).floor() as i32, pos[2], (c.max[2] + radius).floor() as i32];
        for x in xs {
            for z in zs {
                if x != original[0] || z != original[2] {
                    pos = [x, original[1], z];
                    association = self.inner(e, &mut pos, c);
                    if association.is_result() && !association.state.liquid {
                        return association;
                    }
                }
            }
        }
        let state = self.world.block(original);
        if state.liquid {
            let key = if state.lava_fluid { "lavafine" } else { "waterfine" };
            return Association::of(state, down(original), SoundsKey::of(key), SoundsKey::non_emitter(), SoundsKey::non_emitter());
        }
        association
    }

    /// `PFSolver.findAssociation(pool, entity, verticalOffsetAsMinus, isRightFoot)`.
    fn find_foot(&self, e: &Ent, cache: &mut Cache, v_off: f64, right: bool) -> Association {
        let rot = f64::from(wrap_degrees(e.yaw)).to_radians();
        let side = 0.2f32 * if right { -1.0 } else { 1.0 };
        let foot = [
            (e.pos[0] + rot.cos() * f64::from(side)).floor() as i32,
            (e.min_y() - 0.1 - v_off).floor() as i32,
            (e.pos[2] + rot.sin() * f64::from(side)).floor() as i32,
        ];
        if e.kind != Kind::OtherPlayer {
            let v = e.vel;
            if v[0] * v[0] + v[1] * v[1] + v[2] * v[2] != 0.0 && v[1].abs() < 0.004 {
                return Association::none(); // no sounds on every tiny bounce
            }
        }
        let time = self.world.time();
        if cache.time != time {
            cache.time = time;
            cache.map.clear();
        }
        if let Some(a) = cache.map.get(&foot) {
            return a.clone();
        }
        let a = self.around(e, collider(e), foot);
        cache.map.insert(foot, a.clone());
        a
    }
}

#[derive(Default)]
#[derive(Debug)]
pub struct Cache {
    time: i64,
    map: HashMap<[i32; 3], Association>,
}

fn wrap_degrees(d: f32) -> f32 {
    let mut v = d % 360.0;
    if v >= 180.0 {
        v -= 360.0;
    }
    if v < -180.0 {
        v += 360.0;
    }
    v
}

// ---- the generator (`TerrestrialStepSoundGenerator`, `MotionTracker`, `QuadrupedModifier`) ----

/// `Variator`'s `RUNNING_RAMPUP_*` as `getSpeedScalingRatio` pins them.
const RAMPUP_BEGIN: f32 = 0.011;
const RAMPUP_END: f32 = 0.022;

#[derive(Debug)]
pub struct Generator {
    loco: Locomotion,
    // The last tick's position, to get the motion of an entity that does not report a velocity.
    last: Option<[f64; 3]>,
    motion: [f64; 3],
    /// The distance walked: the vanilla one for you, our own sum for the others.
    distance: f32,
    fall_distance: f32,
    dmw_base: f32,
    y_position: f64,
    is_airborne: bool,
    last_fall_distance: f32,
    last_reference: f32,
    is_immobile: bool,
    time_immobile: i64,
    immobile_playback: i64,
    immobile_interval: i64,
    is_right_foot: bool,
    x_movec: f64,
    z_movec: f64,
    scal_stat: bool,
    step_this_frame: bool,
    is_messy_foliage: bool,
    brushes_time: i64,
    // QuadrupedModifier
    hoof: i32,
    next_walk_distance_multiplier: f32,
    cache: Cache,
}

impl Generator {
    pub fn new(loco: Locomotion) -> Generator {
        Generator {
            loco,
            last: None,
            motion: [0.0; 3],
            distance: 0.0,
            fall_distance: 0.0,
            dmw_base: 0.0,
            y_position: 0.0,
            is_airborne: false,
            last_fall_distance: 0.0,
            last_reference: 0.0,
            is_immobile: false,
            time_immobile: 0,
            immobile_playback: 0,
            immobile_interval: 0,
            is_right_foot: false,
            x_movec: 0.0,
            z_movec: 0.0,
            scal_stat: false,
            step_this_frame: false,
            is_messy_foliage: false,
            brushes_time: 0,
            hoof: 0,
            next_walk_distance_multiplier: 0.05,
            cache: Cache::default(),
        }
    }

    fn horizontal_speed(&self) -> f64 {
        self.motion[0] * self.motion[0] + self.motion[2] * self.motion[2]
    }
    fn stationary(&self) -> bool {
        self.motion[0] == 0.0 && self.motion[2] == 0.0
    }
    /// `MotionTracker.getSpeedScalingRatio`.
    pub fn speed_scaling_ratio(&self) -> f32 {
        let relative = self.horizontal_speed() + self.motion[1] * self.motion[1] - f64::from(RAMPUP_BEGIN);
        let max = f64::from(RAMPUP_END - RAMPUP_BEGIN);
        (relative / max).clamp(0.0, 1.0) as f32
    }

    /// `MotionTracker.simulateMotionData` (and the walked distance the entity would have).
    fn simulate_motion(&mut self, e: &Ent) {
        let last = self.last.replace(e.pos);
        let delta = last.map_or([0.0; 3], |l| [e.pos[0] - l[0], e.pos[1] - l[1], e.pos[2] - l[2]]);
        if e.kind == Kind::Local {
            self.motion = e.vel;
        } else {
            self.motion = [delta[0], delta[1], delta[2]];
            if e.on_ground {
                self.motion[1] += 0.0784000015258789;
            }
        }
        match e.kind {
            Kind::Local => {
                self.distance = e.move_dist.unwrap_or(self.distance);
                self.fall_distance = e.fall_distance;
            }
            Kind::OtherPlayer => {
                // Another player reports no distance and no fall: the motion tracker makes them up.
                let [mx, my, mz] = self.motion;
                if mx != 0.0 || mz != 0.0 {
                    self.distance += ((mx * mx + my * my + mz * mz).sqrt() * 0.8) as f32;
                } else {
                    self.distance += ((mx * mx + mz * mz).sqrt() * 0.8) as f32;
                }
                if e.on_ground {
                    self.fall_distance = 0.0;
                } else if my < 0.0 {
                    self.fall_distance -= (my * 200.0) as f32;
                }
            }
            _ => {
                // Entity.move: the walked distance of any entity.
                let len = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
                self.distance += (len * 0.6) as f32;
                self.fall_distance = e.fall_distance;
            }
        }
    }

    /// `pickState`.
    fn pick_state(&self, e: &Ent, iso: &Isolator, walk: State, run: State) -> State {
        if e.kind == Kind::OtherPlayer {
            return if e.sprinting { run } else { walk };
        }
        if self.horizontal_speed() > f64::from(iso.variator.speed_to_run) { run } else { walk }
    }

    fn generate(&mut self, e: &Ent, cx: &mut Cx) {
        self.simulate_motion(e);
        self.simulate_footsteps(e, cx);
        self.simulate_airborne(e, cx);
        self.simulate_brushes(e, cx);
        self.simulate_stationary(e, cx);
        self.last_fall_distance = self.fall_distance;
    }

    fn ctx(&self, e: &Ent) -> EntityCtx {
        EntityCtx { id: e.id, pos: e.pos, kind: e.kind, scale: e.scale, running: self.speed_scaling_ratio() }
    }

    fn simulate_stationary(&mut self, e: &Ent, cx: &mut Cx) {
        if self.is_immobile && (e.on_ground || !e.submerged_water) && self.playback_immobile(cx) {
            let a = cx.find_foot(e, &mut self.cache, 0.0, self.is_right_foot);
            if a.is_result() && (!a.is_silent() || !self.is_immobile) {
                self.play_step(e, cx, &a, State::Stand);
            }
        }
    }

    fn playback_immobile(&mut self, cx: &mut Cx) -> bool {
        let v = &cx.iso.variator;
        if cx.now - self.immobile_playback > self.immobile_interval {
            self.immobile_playback = cx.now;
            let span = (v.immobile_interval_max - v.immobile_interval_min) as f32;
            self.immobile_interval = (cx.rng.next_f32() * span + v.immobile_interval_min as f32).floor() as i64;
            return true;
        }
        false
    }

    fn update_immobile_state(&mut self, reference: f32, cx: &Cx) -> bool {
        let diff = self.last_reference - reference;
        self.last_reference = reference;
        if !self.is_immobile && diff == 0.0 {
            self.time_immobile = cx.now;
            self.is_immobile = true;
        } else if self.is_immobile && diff != 0.0 {
            self.is_immobile = false;
            return cx.now - self.time_immobile > cx.iso.variator.immobile_duration;
        }
        false
    }

    /// QuadrupedModifier.reevaluateDistance (the others do not change it).
    fn reevaluate_distance(&self, event: State, distance: f32) -> f32 {
        if self.loco != Locomotion::Quadruped {
            return distance;
        }
        if event == State::Walk {
            let overall = 1.85f32 / 2.0;
            let ndm = 0.2f32;
            let mut pond = self.next_walk_distance_multiplier;
            pond *= pond;
            pond *= ndm;
            if self.hoof == 1 || self.hoof == 3 {
                return distance * pond * overall;
            }
            return distance * (1.0 - pond) * overall;
        }
        if event == State::Run && self.hoof == 0 {
            return distance * 0.8;
        }
        if event == State::Run {
            return distance * 0.3;
        }
        distance
    }

    fn stepped(&mut self, e: &Ent, cx: &mut Cx, event: State) {
        if self.loco != Locomotion::Quadruped {
            return;
        }
        if self.hoof == 0 || self.hoof == 2 {
            self.next_walk_distance_multiplier = cx.rng.next_f32();
        }
        self.hoof = (self.hoof + 1) % 3;
        if event == State::Walk {
            self.produce_step(e, cx, Some(event), 0.0);
        }
    }

    fn simulate_footsteps(&mut self, e: &Ent, cx: &mut Cx) {
        if !e.is_player() {
            self.distance += (self.horizontal_speed().sqrt() * 0.6) as f32;
        }
        let reference = self.distance;
        self.step_this_frame = false;
        if self.dmw_base > reference {
            self.dmw_base = 0.0;
        }
        let (mov_x, mov_z) = (self.motion[0], self.motion[2]);
        let scal = mov_x * self.x_movec + mov_z * self.z_movec;
        if self.scal_stat != (scal < f64::from(0.001f32)) {
            self.scal_stat = !self.scal_stat;
            if self.scal_stat && cx.iso.variator.play_wander && !has_stopping_conditions(e) {
                let a = cx.find_foot(e, &mut self.cache, 0.0, self.is_right_foot);
                self.play_step(e, cx, &a, State::Wander);
            }
        }
        self.x_movec = mov_x;
        self.z_movec = mov_z;
        let mut dwm = reference - self.dmw_base;
        let immobile = self.update_immobile_state(reference, cx);
        if immobile && !e.climbing {
            dwm = 0.0;
            self.dmw_base = reference;
        }
        if e.on_ground || e.submerged_water || e.climbing {
            let mut event: Option<State> = None;
            let mut distance;
            let mut vertical_offset = 0.0;
            let v = cx.iso.variator.clone();
            if e.climbing && !e.on_ground {
                distance = v.distance_ladder;
            } else if !e.submerged_water && (self.y_position - e.pos[1]).abs() > 0.4 {
                if self.y_position < e.pos[1] {
                    distance = v.distance_stair;
                    event = Some(self.pick_state(e, cx.iso, State::Up, State::UpRun));
                } else {
                    distance = 0.0;
                    if !e.sneaking {
                        distance = -1.0;
                        vertical_offset = 0.0;
                        event = Some(self.pick_state(e, cx.iso, State::Down, State::DownRun));
                    }
                }
            } else {
                distance = v.distance_human;
            }
            let event = event.unwrap_or_else(|| self.pick_state(e, cx.iso, State::Walk, State::Run));
            // Horses (AbstractHorseEntity) walk three times as far per step.
            if is_horse(&e.type_id) && self.horizontal_speed() > 0.1 {
                distance *= 3.0;
            }
            distance = self.reevaluate_distance(event, distance);
            distance *= ((e.scale - 1.0) * 0.6) + 1.0;
            if dwm > distance {
                self.produce_step(e, cx, Some(event), vertical_offset);
                self.stepped(e, cx, event);
                self.dmw_base = reference;
            }
        }
        if e.on_ground {
            self.y_position = e.pos[1];
        }
    }

    fn produce_step(&mut self, e: &Ent, cx: &mut Cx, event: Option<State>, vertical_offset: f64) {
        let event = event.unwrap_or_else(|| self.pick_state(e, cx.iso, State::Walk, State::Run));
        if has_stopping_conditions(e) {
            let speed = (e.vel[0] * e.vel[0] + e.vel[1] * e.vel[1] + e.vel[2] * e.vel[2]).sqrt() as f32;
            let volume = (speed * 0.35).min(1.0);
            let key = SoundsKey::of(if e.touching_water { "_SWIM_WATER" } else { "_SWIM_LAVA" });
            let state = if e.submerged_water || e.submerged_lava { State::Swim } else { event };
            self.play_acoustic(e, cx, &key, state, Options { gliding_volume: Some(volume), ..Options::default() });
            let block = [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32];
            let a = cx.find_messy(e, down(block));
            self.play_step(e, cx, &a, event);
        } else {
            if !e.sneaking || event.is_extra_loud() {
                let a = cx.find_foot(e, &mut self.cache, vertical_offset, self.is_right_foot);
                self.play_step(e, cx, &a, event);
            }
            self.is_right_foot = !self.is_right_foot;
        }
        self.step_this_frame = true;
    }

    fn simulate_airborne(&mut self, e: &Ent, cx: &mut Cx) {
        if (e.on_ground || e.climbing) == self.is_airborne {
            self.is_airborne = !self.is_airborne;
            self.simulate_jumping_landing(e, cx);
        }
    }

    fn offset_minus(&self, e: &Ent) -> f64 {
        if e.kind == Kind::OtherPlayer { 1.0 } else { 0.0 }
    }

    fn simulate_jumping_landing(&mut self, e: &Ent, cx: &mut Cx) {
        if has_stopping_conditions(e) {
            return;
        }
        if self.is_airborne && e.jumping {
            let v = cx.iso.variator.clone();
            if v.event_on_jump {
                if (self.horizontal_speed() as f32) < v.speed_to_jump_as_multifoot {
                    self.play_multifoot(e, cx, self.offset_minus(e) + 0.4, State::Wander);
                } else {
                    self.play_singlefoot(e, cx, self.offset_minus(e) + 0.4, State::Jump);
                }
            }
        } else if !self.is_airborne {
            let v = cx.iso.variator.clone();
            if self.last_fall_distance > 0.0 {
                if self.last_fall_distance > v.land_hard_distance_min {
                    self.play_multifoot(e, cx, self.offset_minus(e), State::Land);
                } else if !self.step_this_frame && !e.sneaking {
                    let st = self.pick_state(e, cx.iso, State::Climb, State::ClimbRun);
                    self.play_singlefoot(e, cx, self.offset_minus(e), st);
                    if !self.step_this_frame {
                        self.is_right_foot = !self.is_right_foot;
                    }
                }
            }
        }
    }

    fn simulate_brushes(&mut self, e: &Ent, cx: &mut Cx) {
        if self.brushes_time > cx.now {
            return;
        }
        self.brushes_time = cx.now + 100;
        if self.stationary() || e.sneaking || e.boots.is_some() {
            return;
        }
        let pos = [
            e.pos[0].floor() as i32,
            (e.pos[1] - 0.1 - if e.on_ground { 0.0 } else { 0.25 }).floor() as i32,
            e.pos[2].floor() as i32,
        ];
        let a = cx.find_messy(e, pos);
        if !a.is_silent() {
            if !self.is_messy_foliage {
                self.is_messy_foliage = true;
                self.play_step(e, cx, &a, State::Walk);
            }
        } else if self.is_messy_foliage {
            self.is_messy_foliage = false;
        }
    }

    fn play_singlefoot(&mut self, e: &Ent, cx: &mut Cx, v_off: f64, event: State) {
        let mut a = cx.find_foot(e, &mut self.cache, v_off, self.is_right_foot);
        if !a.is_result() {
            a = cx.find_foot(e, &mut self.cache, v_off + 1.0, self.is_right_foot);
        }
        self.play_step(e, cx, &a, event);
    }

    fn play_multifoot(&mut self, e: &Ent, cx: &mut Cx, v_off: f64, event: State) {
        let mut left = cx.find_foot(e, &mut self.cache, v_off, false);
        let mut right = cx.find_foot(e, &mut self.cache, v_off, true);
        if left.is_result() && left.data_equals(&right) {
            if self.is_right_foot {
                left = Association::none();
            } else {
                right = Association::none();
            }
        }
        self.play_step(e, cx, &left, event);
        self.play_step(e, cx, &right, event);
    }

    /// `playStep`: the boots' sound as well, when worn (and the step at half volume).
    fn play_step(&mut self, e: &Ent, cx: &mut Cx, a: &Association, event: State) {
        if cx.cfg.footwear
            && let Some(equip) = &e.boots
        {
            let boot = cx.iso.primitives.get(equip, substrate::DEFAULT);
            if boot.is_emitter() {
                self.acoustics_step(e, cx, a, event, Options { volume_percentage: Some(0.5), ..Options::default() });
                self.play_acoustic(e, cx, &boot, event, Options::default());
                return;
            }
        }
        self.acoustics_step(e, cx, a, event, Options::default());
    }

    /// `AcousticsPlayer.playStep`.
    fn acoustics_step(&mut self, e: &Ent, cx: &mut Cx, a: &Association, event: State, options: Options) {
        if a.is_silent() {
            return;
        }
        if a.dry.is_result() {
            self.play_acoustic(e, cx, &a.dry, event, options);
        } else if !a.state.liquid {
            // The sound type's own step sound, at its volume * 0.15; snow on top takes over.
            let above = cx.world.block(up(a.pos));
            let sound = if above.id == "minecraft:snow" { &above } else { &a.state };
            if !sound.step.is_empty() {
                let mut ctx = self.ctx(e);
                ctx.pos = e.pos;
                cx.pending.push(Pending {
                    ctx,
                    sound: sound.step.clone(),
                    volume: sound.volume * 0.15,
                    pitch: sound.pitch,
                    options,
                    time_to_play: cx.now,
                    maximum: 0,
                });
            }
        }
        let wet_pct = Config::pct(cx.cfg.wet_sounds_volume);
        if a.wet.is_emitter() && wet_pct > 0.1 {
            self.play_acoustic(e, cx, &a.wet, event, options.and(Options { volume_percentage: Some(wet_pct), ..Options::default() }));
        }
        let foliage_pct = Config::pct(cx.cfg.foliage_sounds_volume);
        if a.foliage.is_emitter() && foliage_pct > 0.1 {
            self.play_acoustic(e, cx, &a.foliage, event, options.and(Options { volume_percentage: Some(foliage_pct), ..Options::default() }));
        }
    }

    fn play_acoustic(&mut self, e: &Ent, cx: &mut Cx, key: &SoundsKey, event: State, options: Options) {
        let ctx = self.ctx(e);
        for name in key.names() {
            if let Some(acoustic) = cx.iso.acoustics.get(name) {
                let mut v = Voice { entity: &ctx, out: cx.pending, rng: cx.rng, now: cx.now };
                acoustic.play(&mut v, event, options);
            }
        }
    }
}

fn has_stopping_conditions(e: &Ent) -> bool {
    e.touching_water || e.in_lava
}

/// `AbstractHorseEntity`: horse, donkey, mule, skeleton and zombie horses, camel? (no: not a horse), llamas (no).
fn is_horse(type_id: &str) -> bool {
    matches!(
        type_id,
        "minecraft:horse" | "minecraft:donkey" | "minecraft:mule" | "minecraft:skeleton_horse" | "minecraft:zombie_horse"
    )
}

// ---- the engine (`SoundEngine`) ----

/// The two moments the delayed sounds are judged against: a tick is 50 ms.
const TICK_MS: i64 = 50;

#[derive(Default)]
pub struct Engine {
    pub config: Config,
    iso: Isolator,
    has_data: bool,
    generators: HashMap<u32, Generator>,
    pending: Vec<Pending>,
    rng: Option<Rng>,
}

impl Engine {
    pub fn new(config: Config, seed: u64) -> Engine {
        Engine { config, rng: Some(Rng::new(seed)), ..Engine::default() }
    }
    /// `reloadEverything`.
    pub fn load(&mut self, files: &Files) {
        let (iso, any) = Isolator::load(files);
        self.iso = iso;
        self.has_data = any;
        self.generators.clear();
        self.pending.clear();
    }
    pub fn has_data(&self) -> bool {
        self.has_data
    }
    pub fn isolator(&self) -> &Isolator {
        &self.iso
    }
    /// `isRunning` (the paused case is the host's): data, enabled, and multiplayer allowed on a server.
    pub fn running(&self, single_player: bool) -> bool {
        self.has_data && self.config.enabled() && (single_player || self.config.multiplayer)
    }
    /// `getVolumeForSource`.
    fn volume_for(&self, ctx: &EntityCtx) -> f32 {
        let mut volume = self.config.global_volume() as f32 / 100.0;
        volume *= match ctx.kind {
            Kind::Local => Config::pct(self.config.client_player_volume),
            Kind::OtherPlayer => Config::pct(self.config.other_player_volume),
            Kind::Hostile => Config::pct(self.config.hostile_entities_volume),
            Kind::Passive => Config::pct(self.config.passive_entities_volume),
        };
        let running = self.config.running_volume_increase.clamp(-100, 100) as f32 / 100.0;
        volume * (1.0 + running * ctx.running)
    }

    /// `onFrame` for one tick: the steps of every entity near the camera, then the delayed sounds.
    /// `ents` are the candidates the host selected (alive, not riding, not asleep, not a spectator, not a
    /// water creature or a flyer, within 16 blocks of `camera`).
    pub fn tick<W: World>(&mut self, world: &W, ents: &[Ent], camera: [f64; 3], now: i64, out: &mut Vec<Emission>) {
        let mut rng = self.rng.take().unwrap_or_else(|| Rng::new(0x5EED));
        let mut candidates: Vec<&Ent> = ents
            .iter()
            .filter(|e| {
                let d = (0..3).map(|i| (e.pos[i] - camera[i]).powi(2)).sum::<f64>();
                !self.iso.golems.contains(&e.type_id) && d <= 256.0 && self.config.selects(e)
            })
            .collect();
        // getTargets: when there are too many, the nearest, one per (type, block).
        let max = self.config.max_stepping_entities.max(0) as usize;
        if candidates.len() >= max {
            candidates.sort_by(|a, b| {
                let da: f64 = (0..3).map(|i| (a.pos[i] - camera[i]).powi(2)).sum();
                let db: f64 = (0..3).map(|i| (b.pos[i] - camera[i]).powi(2)).sum();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut seen: HashSet<(String, [i32; 3])> = HashSet::new();
            candidates.retain(|e| {
                e.kind == Kind::Local
                    || e.kind == Kind::OtherPlayer
                    || (seen.len() < max
                        && seen.insert((e.type_id.clone(), [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32])))
            });
        }
        let alive: HashSet<u32> = ents.iter().map(|e| e.id).collect();
        self.generators.retain(|id, _| alive.contains(id));
        let mut pending = std::mem::take(&mut self.pending);
        for e in candidates {
            let loco = if e.is_player() {
                // Locomotion.forPlayer: the stance setting, automatic = biped.
                if self.config.stance == Locomotion::None { Locomotion::Biped } else { self.config.stance }
            } else {
                self.iso.locomotion(&e.type_id)
            };
            if loco == Locomotion::None || loco == Locomotion::Flying || loco == Locomotion::FlyingBiped {
                continue;
            }
            let gen_ = self.generators.entry(e.id).or_insert_with(|| Generator::new(loco));
            let mut cx = Cx {
                iso: &self.iso,
                cfg: &self.config,
                world,
                rng: &mut rng,
                pending: &mut pending,
                now,
            };
            gen_.generate(e, &mut cx);
        }
        // DelayedSoundPlayer.think.
        let mut waiting = Vec::new();
        for p in pending {
            let late_limit = if p.maximum < 0 { i64::MAX } else { (p.maximum as f64 / 1.5) as i64 + TICK_MS };
            let early = (p.maximum.max(0) as f64).powf(0.75) as i64;
            if now >= p.time_to_play || now >= p.time_to_play - early {
                if p.maximum < 0 || now - p.time_to_play <= late_limit {
                    out.push(self.emission(&p));
                } // else: skipped, too late
            } else {
                waiting.push(p);
            }
        }
        self.pending = waiting;
        self.rng = Some(rng);
    }

    /// `ImmediateSoundPlayer.playSound`.
    fn emission(&self, p: &Pending) -> Emission {
        let volume = p.volume * p.options.volume_percentage.unwrap_or(1.0) * self.volume_for(&p.ctx);
        let pitch = p.pitch * p.options.pitch_percentage.unwrap_or(1.0) / (((p.ctx.scale - 1.0) * 0.6) + 1.0);
        let sound = if p.sound.contains(':') {
            p.sound.clone()
        } else if p.ctx.kind == Kind::Local {
            format!("presencefootsteps:{}", p.sound)
        } else {
            format!("presencefootstepsmono:{}", p.sound)
        };
        Emission { entity: p.ctx.id, pos: p.ctx.pos, sound, volume, pitch }
    }
}

/// Block ids that changed between 1.20.1 (the files) and Horizon's game.
fn renamed(id: &str) -> String {
    // `Identifier`'s default namespace.
    let id = if id.contains(':') || id.is_empty() || id.starts_with('*') { id.to_owned() } else { format!("minecraft:{id}") };
    match id.as_str() {
        "minecraft:grass" => "minecraft:short_grass".to_owned(),
        _ => id,
    }
}

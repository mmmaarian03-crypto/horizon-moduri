// A Good Place inside Horizon's game (nightly 142388e1c136). The rules are the engine below (the animation pack,
// the pose, the particles in flight); this part starts an animation when the player places a block, ticks them,
// draws the animated block and hides the placed block from the chunk meshes meanwhile (the `a_good_place_hidden`
// module of the mesher, see mixins/a_good_place/mesher.rs).
use super::{Game, ItemStack};
use horizon_client::mesher::Vertex;
use horizon_core::geometry::Axis;
use horizon_world::registry::BlockStateId;
use std::cell::RefCell;

#[allow(dead_code)]
mod engine {
    // A Good Place for Horizon: the rules of the placement animation, without any game code.
// Written from the behaviour of A Good Place 1.20-1.2.8 for Forge 1.20.1 (MehVahdJukaar and
// Enjarai, MIT), read from its sources (branch master of enjarai/a-good-place, version 1.2.7, same
// classes as the 1.2.8 jar) and from the default pack `all_blocks.json` inside the jar:
// `ConfiguredPlacingParticle`, `PlacingBlockParticle`, `BlocksParticlesManager`,
// `AnimationParameters`, `AnimationsManager` and the `state_tests` predicates. The mod has no config
// file (nothing in `config/`), so the only numbers are the pack's and the code's own.

/// A block position.
pub type Pos = [i32; 3];

// ---------------------------------------------------------------------------------------------
// Vanilla maths the original leans on.

const SIN_SCALE: f32 = 10430.378;

fn sin_table() -> &'static [f32] {
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        (0..65536)
            .map(|i| (f64::from(i) * std::f64::consts::PI * 2.0 / 65536.0).sin() as f32)
            .collect()
    })
}
/// `Mth.sin` of 1.20.1 (the quantised table).
pub fn mth_sin(x: f32) -> f32 {
    sin_table()[((x * SIN_SCALE) as i32 & 65535) as usize]
}
/// `Mth.cos` of 1.20.1.
pub fn mth_cos(x: f32) -> f32 {
    sin_table()[((x * SIN_SCALE + 16384.0) as i32 & 65535) as usize]
}

/// The six directions in `Direction.values()` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Down,
    Up,
    North,
    South,
    West,
    East,
}
impl Dir {
    pub const ALL: [Dir; 6] = [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East];
    pub fn index(self) -> usize {
        self as usize
    }
    /// `Direction.step()`.
    pub fn step(self) -> [f32; 3] {
        match self {
            Dir::Down => [0.0, -1.0, 0.0],
            Dir::Up => [0.0, 1.0, 0.0],
            Dir::North => [0.0, 0.0, -1.0],
            Dir::South => [0.0, 0.0, 1.0],
            Dir::West => [-1.0, 0.0, 0.0],
            Dir::East => [1.0, 0.0, 0.0],
        }
    }
    /// The block offset of the neighbour in this direction.
    pub fn offset(self) -> [i32; 3] {
        self.step().map(|v| v as i32)
    }
    /// 0 for x, 1 for y, 2 for z.
    pub fn axis(self) -> usize {
        match self {
            Dir::West | Dir::East => 0,
            Dir::Down | Dir::Up => 1,
            Dir::North | Dir::South => 2,
        }
    }
    pub fn positive(self) -> bool {
        matches!(self, Dir::Up | Dir::South | Dir::East)
    }
    pub fn opposite(self) -> Dir {
        match self {
            Dir::Down => Dir::Up,
            Dir::Up => Dir::Down,
            Dir::North => Dir::South,
            Dir::South => Dir::North,
            Dir::West => Dir::East,
            Dir::East => Dir::West,
        }
    }
}

/// `Direction.orderedByNearest(entity)` for a player looking along `yaw`, `pitch` (degrees).
pub fn ordered_by_nearest(yaw: f32, pitch: f32) -> [Dir; 6] {
    let f = pitch * (std::f32::consts::PI / 180.0);
    let f1 = -yaw * (std::f32::consts::PI / 180.0);
    let (f2, f3, f4, f5) = (mth_sin(f), mth_cos(f), mth_sin(f1), mth_cos(f1));
    let (flag, flag1, flag2) = (f4 > 0.0, f2 < 0.0, f5 > 0.0);
    let f6 = if flag { f4 } else { -f4 };
    let f7 = if flag1 { -f2 } else { f2 };
    let f8 = if flag2 { f5 } else { -f5 };
    let (f9, f10) = (f6 * f3, f8 * f3);
    let d = if flag { Dir::East } else { Dir::West };
    let d1 = if flag1 { Dir::Up } else { Dir::Down };
    let d2 = if flag2 { Dir::South } else { Dir::North };
    let make = |a: Dir, b: Dir, c: Dir| [a, b, c, c.opposite(), b.opposite(), a.opposite()];
    if f6 > f8 {
        if f7 > f9 {
            make(d1, d, d2)
        } else if f10 > f7 {
            make(d, d2, d1)
        } else {
            make(d, d1, d2)
        }
    } else if f7 > f10 {
        make(d1, d2, d)
    } else if f6 > f7 {
        make(d2, d, d1)
    } else {
        make(d2, d1, d)
    }
}

/// `Entity.calculateViewVector(xRot, yRot)` (as floats, `Player.getLookAngle`).
pub fn look_vector(yaw: f32, pitch: f32) -> [f32; 3] {
    let f = pitch * 0.017453292;
    let f1 = -yaw * 0.017453292;
    let (f2, f3, f4, f5) = (mth_cos(f1), mth_sin(f1), mth_cos(f), mth_sin(f));
    [f3 * f4, -f5, f2 * f4]
}

fn length(v: [f32; 3]) -> f32 {
    let (x, y, z) = (f64::from(v[0]), f64::from(v[1]), f64::from(v[2]));
    (x * x + y * y + z * z).sqrt() as f32
}
/// JOML `Vector3f.normalize()` (a zero vector gives NaNs there, as it does here).
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let (x, y, z) = (f64::from(v[0]), f64::from(v[1]), f64::from(v[2]));
    let s = (1.0 / (x * x + y * y + z * z).sqrt()) as f32;
    [v[0] * s, v[1] * s, v[2] * s]
}

// ---------------------------------------------------------------------------------------------
// 4x4 matrices, column major, post-multiplied like `PoseStack`.

pub type Mat = [f32; 16];
pub const IDENTITY: Mat = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];
fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut r = [0.0; 16];
    for c in 0..4 {
        for row in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[k * 4 + row] * b[c * 4 + k];
            }
            r[c * 4 + row] = s;
        }
    }
    r
}
/// `PoseStack.translate`.
fn translate(m: &Mat, x: f32, y: f32, z: f32) -> Mat {
    let mut t = IDENTITY;
    t[12] = x;
    t[13] = y;
    t[14] = z;
    mul(m, &t)
}
/// `PoseStack.scale`.
fn scale(m: &Mat, x: f32, y: f32, z: f32) -> Mat {
    let mut t = IDENTITY;
    t[0] = x;
    t[5] = y;
    t[10] = z;
    mul(m, &t)
}
/// `PoseStack.mulPose(Axis.YP.rotation(a))`.
fn rotate_y(m: &Mat, a: f32) -> Mat {
    let (s, c) = (f64::from(a).sin() as f32, f64::from(a).cos() as f32);
    let mut t = IDENTITY;
    t[0] = c;
    t[2] = -s;
    t[8] = s;
    t[10] = c;
    mul(m, &t)
}
/// `Axis.ZP.rotation(a)`.
fn rotate_z(m: &Mat, a: f32) -> Mat {
    let (s, c) = (f64::from(a).sin() as f32, f64::from(a).cos() as f32);
    let mut t = IDENTITY;
    t[0] = c;
    t[1] = s;
    t[4] = -s;
    t[5] = c;
    mul(m, &t)
}
/// `Axis.XP.rotation(a)`.
fn rotate_x(m: &Mat, a: f32) -> Mat {
    let (s, c) = (f64::from(a).sin() as f32, f64::from(a).cos() as f32);
    let mut t = IDENTITY;
    t[5] = c;
    t[6] = s;
    t[9] = -s;
    t[10] = c;
    mul(m, &t)
}
/// The image of a point.
pub fn transform(m: &Mat, p: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r])
}

// ---------------------------------------------------------------------------------------------
// The animation pack.

/// What a predicate asks of a block state (`BlockStatePredicate.test(state, pos, level)`).
pub trait StateView {
    /// `IsDoubleBlock`: a bed, a `DoublePlantBlock`, a `DoorBlock` or a chest that is not single.
    fn is_double_block(&self) -> bool;
    /// `HasCollision`: the collision shape of the block at the position is not empty.
    fn has_collision(&self) -> bool;
    /// `MatchingBlocks` with a block id.
    fn is_block(&self, id: &str) -> bool;
    /// `MatchingBlocks` with a tag (`#minecraft:flowers` without the `#`).
    fn in_tag(&self, tag: &str) -> bool;
}

/// The `state_tests` of the original that the default pack and its readme use.
#[derive(Clone, Debug, PartialEq)]
pub enum Predicate {
    Not(Box<Predicate>),
    AnyOf(Vec<Predicate>),
    IsDoubleBlock,
    HasCollision,
    /// Ids, or `#tag`.
    MatchingBlocks(Vec<String>),
}
impl Predicate {
    pub fn test(&self, view: &impl StateView) -> bool {
        match self {
            Predicate::Not(p) => !p.test(view),
            Predicate::AnyOf(list) => list.iter().any(|p| p.test(view)),
            Predicate::IsDoubleBlock => view.is_double_block(),
            Predicate::HasCollision => view.has_collision(),
            Predicate::MatchingBlocks(list) => list.iter().any(|b| match b.strip_prefix('#') {
                Some(tag) => view.in_tag(tag),
                None => view.is_block(b),
            }),
        }
    }
}

/// `AnimationParameters`: angles are already in radians (`DEG_TO_RAD_CODEC`).
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    pub predicates: Vec<Predicate>,
    pub priority: i32,
    /// Ticks, 0 to 300.
    pub duration: i32,
    pub scale_start: f32,
    pub scale_curve: f32,
    pub translation: [f64; 3],
    pub translation_curve: f32,
    pub rotation: [f64; 3],
    pub pivot: [f64; 3],
    pub rotation_curve: f32,
    pub height_start: f32,
    pub height_curve: f32,
    pub restrict_direction: bool,
}
impl Default for Params {
    /// The codec's defaults (`StrOpt.of(..., default)`), for a pack that leaves a field out.
    fn default() -> Self {
        Self {
            predicates: Vec::new(),
            priority: 0,
            duration: 4,
            scale_start: 1.0,
            scale_curve: 0.5,
            translation: [0.0; 3],
            translation_curve: 0.5,
            rotation: [0.0; 3],
            pivot: [0.0; 3],
            rotation_curve: 0.5,
            height_start: 1.0,
            height_curve: 0.5,
            restrict_direction: true,
        }
    }
}
fn radians(degrees: f32) -> f64 {
    f64::from((f64::from(degrees)).to_radians() as f32)
}

/// `resourcepacks/default_animations/assets/a_good_place/placement_animations/all_blocks.json`
/// of the 1.20.1 jar, the one pack the original enables by default (`firstInstall`).
pub fn default_pack() -> Vec<Params> {
    vec![Params {
        predicates: vec![
            Predicate::Not(Box::new(Predicate::IsDoubleBlock)),
            Predicate::HasCollision,
        ],
        priority: 0,
        duration: 3,
        scale_start: 1.0,
        scale_curve: -0.7,
        translation: [0.1, 0.2, 0.2],
        translation_curve: 0.9,
        rotation: [radians(0.0), radians(-5.7), radians(5.7)],
        pivot: [0.5, -0.5, 0.5],
        rotation_curve: -0.08,
        height_start: 1.0,
        height_curve: 0.0,
        restrict_direction: true,
    }]
}

/// `AnimationsManager.getAnimation`: the packs sorted by priority (high first, stable), the first
/// whose predicates all hold (no predicates: always).
pub fn select<'a>(pack: &'a [Params], view: &impl StateView) -> Option<&'a Params> {
    let mut order: Vec<&Params> = pack.iter().collect();
    order.sort_by(|a, b| b.priority.cmp(&a.priority));
    order
        .into_iter()
        .find(|p| p.predicates.iter().all(|pred| pred.test(view)))
}

// ---------------------------------------------------------------------------------------------
// One animation.

/// `PlacingBlockParticle.lifetime` extra ticks of `ConfiguredPlacingParticle`.
pub const EXTRA_LIFE_TICKS: i32 = 2;
/// The scale an animation ends at, "slightly below 1 so we don't have potential clipping".
pub const END_SCALE: f32 = 0.999;

/// The placer, as `ConfiguredPlacingParticle`'s constructor reads them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placer {
    pub yaw: f32,
    pub pitch: f32,
    /// `getMainArm() == RIGHT`.
    pub right_arm: bool,
    /// The hand that placed is the main hand.
    pub main_hand: bool,
}

/// What `adjustDirectionBasedOnNeighbors` reads of the world: the collision shape of the
/// neighbour on the `dir` side along the axis of `dir`, as `(min, max)`, or `None` when it has no
/// collision (`collision.isEmpty()`).
pub trait Around {
    fn extent(&self, dir: Dir) -> Option<[f64; 2]>;
}

/// `getAffectedDirections`.
pub fn affected_directions(v: [f32; 3]) -> Vec<Dir> {
    let mut list = Vec::new();
    if v[2] > 0.0 {
        list.push(Dir::South);
    }
    if v[2] < 0.0 {
        list.push(Dir::North);
    }
    if v[0] > 0.0 {
        list.push(Dir::East);
    }
    if v[0] < 0.0 {
        list.push(Dir::West);
    }
    if v[1] > 0.0 {
        list.push(Dir::Up);
    }
    if v[1] < 0.0 {
        list.push(Dir::Down);
    }
    list
}

/// `adjustDirectionBasedOnNeighbors`: the slide direction without the components that point into
/// a neighbour with collision; if nothing is left, the free direction nearest to where the placer
/// looks.
pub fn adjust_direction(placer: &Placer, around: &impl Around, mut slide: [f32; 3]) -> [f32; 3] {
    let affected = affected_directions(slide);
    let mut empty: Vec<Dir> = Vec::new();
    for d in Dir::ALL {
        match around.extent(d) {
            None => empty.push(d),
            Some([min, max]) => {
                if d.positive() {
                    if min > f64::from(0.25_f32) {
                        empty.push(d);
                    }
                } else if max < f64::from(0.75_f32) {
                    empty.push(d);
                }
            }
        }
    }
    for d in affected {
        if !empty.contains(&d) {
            // `slideDir.sub(d.step().mul(d.step()).mul(slideDir))`
            let s = d.step();
            for k in 0..3 {
                slide[k] -= s[k] * s[k] * slide[k];
            }
        }
    }
    if length(slide) == 0.0 && !empty.is_empty() {
        let nearest = ordered_by_nearest(placer.yaw, placer.pitch);
        empty.sort_by_key(|d| nearest.iter().position(|n| n == d));
        slide = empty[0].step();
    }
    slide
}

/// One configured animation, started at a placement (`ConfiguredPlacingParticle`).
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub params: Params,
    /// Where the slide comes from, as an angle around Y.
    pub y_angle: f32,
    /// The slide start, (z, y) relative to `y_angle`.
    pub slide_start: [f32; 2],
    pub rot_start: [f64; 3],
}

/// `fancyExponent`: an exponential between 0 and 1 whose bend `curve` (-1 to 1, exclusive) sets;
/// 0 is a line.
pub fn fancy_exponent(t: f32, curve: f32) -> f32 {
    if curve == 0.0 {
        return t;
    }
    let base: f32 = if curve > 0.0 {
        (-f64::from(curve).ln()) as f32
    } else {
        (f64::from(-curve).ln() - 1.0) as f32
    };
    // `base * Math.pow(1 / base + 1, t) - base`, the division and sum in float
    let inner = 1.0_f32 / base + 1.0;
    (f64::from(base) * f64::from(inner).powf(f64::from(t)) - f64::from(base)) as f32
}

impl Pose {
    /// The constructor: slide direction from the way the player looks and which hand placed.
    pub fn new(params: &Params, placer: &Placer, around: &impl Around) -> Pose {
        let look = look_vector(placer.yaw, placer.pitch);
        // `.mul(-1, 0, -1).normalize()`; looking straight up or down leaves nothing to normalise
        // (NaNs in the original, which hides the block for its animation): the facing is used.
        let mut flat = [-look[0], 0.0, -look[2]];
        if length(flat) == 0.0 {
            let y = placer.yaw * 0.017453292;
            flat = [mth_sin(y), 0.0, -mth_cos(y)];
        }
        let flat = normalize(flat);
        let mut right: f32 = if placer.right_arm { 1.0 } else { -1.0 };
        if !placer.main_hand {
            right *= -1.0;
        }
        let x_vec = [flat[2] * right, 0.0 * right, -flat[0] * right];
        let t = params.translation.map(|v| v as f32);
        let slide_pow = length(t);
        let tr = if placer.pitch > 0.0 {
            t
        } else {
            [t[0], -t[1], t[2]]
        };
        // `tr.mul(changeOfBasis)`: columns x_vec, up, flat
        let mut slide = [0, 1, 2].map(|r| x_vec[r] * tr[0] + [0.0, 1.0, 0.0][r] * tr[1] + flat[r] * tr[2]);
        if params.restrict_direction {
            slide = adjust_direction(placer, around, slide);
        }
        let slide = normalize(slide);
        if slide.iter().any(|v| v.is_nan()) {
            // Closed in on every side: nowhere to slide from (NaNs in the original).
            return Pose {
                params: params.clone(),
                y_angle: 0.0,
                slide_start: [0.0, 0.0],
                rot_start: params.rotation,
            };
        }
        let y_angle = f64::from(slide[0]).atan2(f64::from(slide[2])) as f32;
        // `animationDirection.yRot(-yAngle)`
        let (c, s) = (mth_cos(-y_angle), mth_sin(-y_angle));
        let dir = slide.map(f64::from);
        let temp_z = dir[2] * f64::from(c) - dir[0] * f64::from(s);
        let temp_y = dir[1];
        Pose {
            params: params.clone(),
            y_angle,
            slide_start: [temp_z as f32 * slide_pow, temp_y as f32 * slide_pow],
            rot_start: params.rotation,
        }
    }

    /// `applyAnimation(poseStack, time, partialTicks)` followed by the block's own offset
    /// (`tesselateBlock` translates by `getOffset`): the matrix from model space (0 to 1) to the
    /// block's cell, for the animation time `t` (0 to 1).
    pub fn matrix(&self, t: f32, offset: [f64; 3]) -> Mat {
        let p = &self.params;
        let off = offset.map(|v| v as f32);
        let mut m = IDENTITY;
        m = translate(&m, 0.5, 0.5, 0.5);
        m = translate(&m, off[0], off[1], off[2]);
        // all animations are relative to the move direction
        m = rotate_y(&m, self.y_angle);
        // slide
        {
            let progress = fancy_exponent(t, p.translation_curve);
            let k = 1.0 - progress;
            m = translate(&m, 0.0, self.slide_start[1] * k, self.slide_start[0] * k);
        }
        // rotate
        {
            let progress = fancy_exponent(t, p.rotation_curve);
            let k = f64::from(1.0 - progress);
            let mut rotation = self.rot_start.map(|v| v * k);
            let mut pivot = p.pivot;
            if self.slide_start[1] < 0.0 {
                pivot[1] = -pivot[1];
                rotation = [-rotation[0], rotation[1], -rotation[2]];
            }
            m = translate(&m, pivot[0] as f32, pivot[1] as f32, pivot[2] as f32);
            m = rotate_y(&m, rotation[1] as f32);
            m = rotate_z(&m, rotation[2] as f32);
            m = rotate_x(&m, rotation[0] as f32);
            m = translate(&m, -pivot[0] as f32, -pivot[1] as f32, -pivot[2] as f32);
        }
        // scale
        {
            let progress = fancy_exponent(t, p.scale_curve);
            let start = p.scale_start;
            let s = start + (END_SCALE - start) * progress;
            m = scale(&m, s, s, s);
        }
        // height scale (the original translates back by +0.5, not by what it took off)
        {
            let progress = fancy_exponent(t, p.height_curve);
            let start = p.height_start;
            let h = start + (1.0 - start) * progress;
            m = translate(&m, 0.0, -0.5, 0.0);
            m = scale(&m, 1.0, h, 1.0);
            m = translate(&m, 0.0, 0.5, 0.0);
        }
        // reset so the block renders straight
        m = rotate_y(&m, -self.y_angle);
        m = translate(&m, -off[0], -off[1], -off[2]);
        m = translate(&m, -0.5, -0.5, -0.5);
        // the model's own offset
        translate(&m, off[0], off[1], off[2])
    }
}

// ---------------------------------------------------------------------------------------------
// The particles in flight (`BlocksParticlesManager`, `PlacingBlockParticle`).

/// `distanceToSqr(pos.getCenter()) <= 1024.0`: placements farther from the camera are not animated.
pub const MAX_DISTANCE_SQ: f64 = 1024.0;

/// `PlacingBlockParticle`: one placement being animated.
#[derive(Clone, Debug, PartialEq)]
pub struct Placing {
    pub pos: Pos,
    /// The block state at the position when the particle began (any id the game uses).
    pub state: u32,
    pub pose: Pose,
    pub age: i32,
    pub lifetime: i32,
    pub removed: bool,
    /// Drawn by the particle (blocks that are just a model): the block is hidden meanwhile.
    pub can_render: bool,
    /// Added after this client tick's particle pass: it is not ticked until the next one.
    pub fresh: bool,
}
impl Placing {
    /// `Math.min(1, (age + partialTicks) / (lifetime + 1))`.
    pub fn time(&self, partial: f32) -> f32 {
        ((self.age as f32 + partial) / (self.lifetime + 1) as f32).min(1.0)
    }
    /// `finishedAnimation`.
    pub fn finished(&self) -> bool {
        self.age >= self.lifetime
    }
}

/// What the manager asks the game to do with the block meshes.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// `hideBlock`: the block at the position is left out of the chunk meshes.
    Hide(Pos),
    /// `unHideBlock`; `was_hidden`: the block had been hidden, so its chunk is rebuilt
    /// (`markBlockForRender`).
    Unhide { pos: Pos, was_hidden: bool },
}

/// `BlocksParticlesManager` with `RENDER_AS_VANILLA_PARTICLES` on (as the mod ships).
#[derive(Debug, Default)]
pub struct Manager {
    list: Vec<Placing>,
    hidden: std::collections::BTreeSet<Pos>,
}
impl Manager {
    pub fn particles(&self) -> &[Placing] {
        &self.list
    }
    pub fn is_hidden(&self, pos: Pos) -> bool {
        self.hidden.contains(&pos)
    }
    pub fn hidden_count(&self) -> usize {
        self.hidden.len()
    }
    /// `addParticle` once the animation was found and the camera is near enough: a particle for
    /// `pos`, the one before it there removed, the block hidden when it is only a model.
    /// `fresh`: the placement came before this client tick's particle pass.
    pub fn add(
        &mut self,
        pos: Pos,
        state: u32,
        pose: Pose,
        just_model: bool,
        fresh: bool,
    ) -> Vec<Effect> {
        for old in self.list.iter_mut().filter(|p| p.pos == pos) {
            old.removed = true;
        }
        let lifetime = pose.params.duration;
        self.list.push(Placing {
            pos,
            state,
            pose,
            age: 0,
            lifetime,
            removed: false,
            can_render: just_model,
            fresh,
        });
        let mut effects = Vec::new();
        if just_model {
            self.hidden.insert(pos);
            effects.push(Effect::Hide(pos));
        }
        effects
    }

    /// `unHideBlock`.
    fn unhide(&mut self, pos: Pos, effects: &mut Vec<Effect>) {
        let was_hidden = self.hidden.remove(&pos);
        effects.push(Effect::Unhide { pos, was_hidden });
    }

    /// One client tick of the particle engine: every particle ticks, then the dead ones go.
    /// `state_at` is the block state in the level now.
    pub fn tick(&mut self, state_at: impl Fn(Pos) -> u32) -> Vec<Effect> {
        let mut effects = Vec::new();
        let mut i = 0;
        while i < self.list.len() {
            if self.list[i].fresh {
                self.list[i].fresh = false;
                i += 1;
                continue;
            }
            // `PlacingBlockParticle.tick`
            if !self.list[i].removed {
                self.list[i].age += 1;
                if self.list[i].age >= self.list[i].lifetime + EXTRA_LIFE_TICKS {
                    self.list[i].removed = true;
                }
                let pos = self.list[i].pos;
                if self.list[i].finished() {
                    self.unhide(pos, &mut effects);
                }
                if state_at(pos) != self.list[i].state {
                    self.list[i].removed = true;
                    self.unhide(pos, &mut effects);
                }
            }
            i += 1;
        }
        // `ParticleEngine.tickParticleList` takes out the particles no longer alive.
        self.list.retain(|p| !p.removed);
        effects
    }

    /// `BlocksParticlesManager.clear` (the level unloads).
    pub fn clear(&mut self) {
        self.list.clear();
        self.hidden.clear();
    }
}

}

pub const ID: &str = "a_good_place";
pub const TITLE: &str = "A Good Place 1.2.8";
pub const DESCRIPTION: &str = "Blocks you place slide, tilt and scale into position (the original's default animation pack).";

struct State {
    pack: Vec<engine::Params>,
    manager: engine::Manager,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        pack: engine::default_pack(),
        manager: engine::Manager::default(),
    });
}

/// `RenderShape` that is not MODEL in 1.20.1 and gets its own renderer: the original neither hides nor draws these
/// itself (it only moves their block entity renderer, which Horizon cannot do), so they are left alone here.
fn special_render(name: &str) -> bool {
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    n.ends_with("chest")
        || n.ends_with("_bed")
        || n.ends_with("_banner")
        || n.ends_with("_sign")
        || n.ends_with("_skull")
        || n.ends_with("_head")
        || n.ends_with("shulker_box")
        || matches!(
            n,
            "conduit" | "bell" | "enchanting_table" | "end_portal" | "end_gateway" | "decorated_pot" | "lectern"
                | "player_head" | "player_wall_head" | "dragon_head" | "dragon_wall_head" | "piglin_head"
                | "piglin_wall_head" | "skeleton_skull" | "wither_skeleton_skull" | "zombie_head"
                | "creeper_head" | "sculk_sensor" | "sculk_shrieker" | "sculk_catalyst"
        )
}

/// What the predicates and the direction fix ask of the level at one placement.
struct View<'g, 'a> {
    game: &'g Game<'a>,
    pos: [i32; 3],
    state: BlockStateId,
}
impl View<'_, '_> {
    fn shape(&self, pos: [i32; 3]) -> Option<horizon_core::geometry::VoxelShape> {
        let state = self.game.world.block(pos[0], pos[1], pos[2]);
        let mut scene = horizon_client::collisions::SceneCollisions {
            world: &self.game.world,
            binding: self.game.collision,
            subject: None,
        };
        self.game.collision.collision_shape(&mut scene, state, pos).ok()
    }
    fn name(&self) -> std::sync::Arc<str> {
        self.game.res.facts().get(self.state).name.clone()
    }
    fn property(&self, key: &str) -> Option<String> {
        self.game
            .pack
            .assets
            .registry
            .state(self.state)?
            .properties
            .get(key)
            .cloned()
    }
}
impl engine::StateView for View<'_, '_> {
    fn is_double_block(&self) -> bool {
        let name = self.name();
        let n = name.strip_prefix("minecraft:").unwrap_or(&name);
        // BedBlock, DoorBlock, DoublePlantBlock (the lower/upper `half`) and a chest that is not single.
        if n.ends_with("_bed") || n.ends_with("_door") {
            return true;
        }
        if matches!(self.property("half").as_deref(), Some("upper" | "lower")) {
            return true;
        }
        if n == "chest" || n == "trapped_chest" {
            return self.property("type").is_some_and(|t| t != "single");
        }
        false
    }
    fn has_collision(&self) -> bool {
        self.shape(self.pos).is_some_and(|s| !s.is_empty())
    }
    fn is_block(&self, id: &str) -> bool {
        *self.name() == *id
    }
    fn in_tag(&self, tag: &str) -> bool {
        let tag = tag.strip_prefix("minecraft:").map_or(tag.to_owned(), str::to_owned);
        let name = self.name();
        let tags = &self.game.res.items().block_tags;
        [format!("minecraft:{tag}"), tag].iter().any(|t| tags.get(t).is_some_and(|s| s.contains(&*name)))
    }
}
impl engine::Around for View<'_, '_> {
    fn extent(&self, dir: engine::Dir) -> Option<[f64; 2]> {
        let o = dir.offset();
        let shape = self.shape([self.pos[0] + o[0], self.pos[1] + o[1], self.pos[2] + o[2]])?;
        let bounds = shape.bounds()?;
        let axis = [Axis::X, Axis::Y, Axis::Z][dir.axis()];
        Some([bounds.min(axis), bounds.max(axis)])
    }
}

impl Game<'_> {
    /// `BlockItem.place` of the player, after the block is in the level: the animation starts
    /// (`BlocksParticlesManager.addParticle`).
    pub(super) fn a_good_place_placed(&mut self, pos: [i32; 3], _state: BlockStateId, _stack: &ItemStack) {
        if !self.native_on(ID) || !self.native_window() {
            return;
        }
        let Some(p) = self.player.as_ref() else { return };
        let state = self.world.block(pos[0], pos[1], pos[2]);
        if self.world.is_air(state) {
            return;
        }
        let name = self.res.facts().get(state).name.clone();
        // Blocks with their own renderer are not animated (see `special_render`).
        if special_render(&name) {
            return;
        }
        // `camera.getPosition().distanceToSqr(pos.getCenter()) <= 1024`.
        let eye = [p.position.x, p.position.y + f64::from(p.dimensions.eye_height), p.position.z];
        let d: f64 = (0..3).map(|i| (eye[i] - (f64::from(pos[i]) + 0.5)).powi(2)).sum();
        if d > engine::MAX_DISTANCE_SQ {
            return;
        }
        let view = View { game: self, pos, state };
        let (yaw, pitch) = (self.yaw, self.pitch);
        let right_arm = !self.settings.main_hand_left;
        let pose = STATE.with(|s| {
            let s = s.borrow();
            engine::select(&s.pack, &view).map(|params| {
                let placer = engine::Placer { yaw, pitch, right_arm, main_hand: true };
                engine::Pose::new(params, &placer, &view)
            })
        });
        let Some(pose) = pose else { return };
        let effects = STATE.with(|s| s.borrow_mut().manager.add(pos, state.0, pose, true, true));
        self.a_good_place_apply(effects);
    }

    /// The meshes of the positions the manager hid or showed again are rebuilt.
    fn a_good_place_apply(&mut self, effects: Vec<engine::Effect>) {
        for effect in effects {
            let pos = match effect {
                engine::Effect::Hide(pos) => {
                    horizon_client::mesher::a_good_place_hidden::hide(pos);
                    pos
                }
                engine::Effect::Unhide { pos, was_hidden } => {
                    if !was_hidden {
                        continue;
                    }
                    horizon_client::mesher::a_good_place_hidden::unhide(pos);
                    pos
                }
            };
            for dz in -1..=1 {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let (x, y, z) = (pos[0] + dx, pos[1] + dy, pos[2] + dz);
                        let s = (y - horizon_client::chunk::MIN_Y) >> 4;
                        if (0..horizon_client::chunk::SECTIONS as i32).contains(&s) {
                            self.world.dirty_sections.insert((x >> 4, s, z >> 4));
                        }
                    }
                }
            }
        }
    }

    /// One client tick of the particle engine.
    pub(super) fn a_good_place_tick(&mut self) {
        if self.player.is_none() || !self.native_on(ID) {
            let any = STATE.with(|s| !s.borrow().manager.particles().is_empty());
            if any {
                STATE.with(|s| s.borrow_mut().manager.clear());
                horizon_client::mesher::a_good_place_hidden::clear();
            }
            return;
        }
        let world = &self.world;
        let effects = STATE.with(|s| {
            s.borrow_mut().manager.tick(|p| world.block(p[0], p[1], p[2]).0)
        });
        self.a_good_place_apply(effects);
    }

    /// `PlacingBlockParticle.render`: the animated blocks as entity geometry (camera-relative vertices).
    pub(super) fn a_good_place_geometry(&self, camera: glam::DVec3, alpha: f64, out: &mut Vec<Vertex>) {
        if !self.native_on(ID) {
            return;
        }
        let drawn: Vec<(engine::Pos, u32, glam::Mat4)> = STATE.with(|s| {
            let s = s.borrow();
            s.manager
                .particles()
                .iter()
                .filter(|p| p.can_render && !p.removed)
                .map(|p| {
                    let t = p.time(alpha as f32);
                    let offset = self
                        .pack
                        .context
                        .tables
                        .offset(BlockStateId(p.state), p.pos[0], p.pos[2])
                        .unwrap_or([0.0; 3]);
                    (p.pos, p.state, glam::Mat4::from_cols_array(&p.pose.matrix(t, offset)))
                })
                .collect()
        });
        for (pos, state, m) in drawn {
            let state = BlockStateId(state);
            let at = glam::Vec3::new(
                (f64::from(pos[0]) - camera.x) as f32,
                (f64::from(pos[1]) - camera.y) as f32,
                (f64::from(pos[2]) - camera.z) as f32,
            );
            // The light around the block (its own cell is solid).
            let light = [[0, 0, 0], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]
                .iter()
                .map(|o| self.world.light(pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]))
                .fold((0u8, 0u8), |(sky, block), l| (sky.max(l >> 4), block.max(l & 15)));
            let packed = (u32::from(light.0) * 16) | ((u32::from(light.1) * 16) << 8);
            let tint = self.particle_sprite(state, pos).map(|(_, tint)| tint);
            let start = out.len();
            self.block_model_vertices(
                state,
                horizon_core::random::position_seed(horizon_core::position::BlockPos::new(pos[0], pos[1], pos[2])),
                glam::Mat4::from_translation(at) * m,
                packed,
                false,
                out,
            );
            if let Some(tint) = tint {
                let tint = horizon_client::mobs::render::abgr(tint);
                for vertex in &mut out[start..] {
                    vertex.color = horizon_client::hud_layout::multiply(vertex.color, tint);
                }
            }
        }
    }
}

// Smooth Swapping inside Horizon's game (nightly 142388e1c136). The rules are the engine below
// (src/lib.rs); this part reads the open menu once a frame, hears the clicks, and draws the items
// that are on their way.
//
// Where the original replaces `DrawContext.drawItem` and `drawItemInSlot` for a slot, this does the
// same through `draw_item_model` (the hook draws what the plan says and keeps the game from drawing
// the slot's item). The count and bar the game draws after an item cannot be hooked (the function
// has a macro), so that overlay is taken out of the batch afterwards, when it can be shown to be
// exactly what the game's own call produced, and the one the original draws is put in its place.
// A slot is recognised by where its item is drawn: the screen lists its slots and their places at
// the start of the frame (`draw_inventory`).
use super::{Game, Screen, stations};
use horizon_client::containers::BlockEntity;
use horizon_client::hud::{HudBatch, HudVertex};
use horizon_client::items::ItemStack;
use horizon_client::menu::Input;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::ops::{ControlFlow, Range};

#[allow(dead_code)]
mod engine {
    // Smooth Swapping for Horizon: the rules of the sliding, without any game code.
// Written from the behaviour of Smooth Swapping 0.9.3.2 for 1.20.1 (Schauweg and Riflusso, LGPL-3.0;
// the pack has `smoothswapping-0.9.3.2-1.20.1-forge.jar`), read from the 1.20.1 branch of its public
// source and checked against the jar, and from `smoothswapping.json` of the pack. The algorithms
// (which stacks lose items and which gain them, how the movers are paired, the speed and the
// easing curve) are followed step by step, including their oddities, so the items move as they do
// there; the code is written fresh for Horizon's menus.
use std::collections::BTreeMap;
use std::f64::consts::PI;

/// An item stack as the engine needs to know it. `is_empty` is `ItemStack.isEmpty` (air).
pub trait Stack: Clone {
    fn is_empty(&self) -> bool;
    /// `getCount()`, 0 for an empty stack.
    fn count(&self) -> i32;
    /// `getItem() == other.getItem()`: the same kind of item (two empty stacks are the same).
    fn same_item(&self, other: &Self) -> bool;
    /// `ItemStack.areEqual`: the same item, count and data.
    fn same(&self, other: &Self) -> bool;
    /// `isItemBarVisible()`: a damaged item or a bundle with something in it.
    fn bar_visible(&self) -> bool;
}

/// `Config`: the options of `smoothswapping.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// `toggle_mod`.
    pub enabled: bool,
    /// `animation_speed`, in percent: at 100 an item covers a tenth of its way per tick (ten ticks,
    /// half a second); 200 is twice as fast.
    pub animation_speed: i32,
    /// `curve_points`: the points between the fixed ends (0,0) and (1,1) of the easing curve.
    pub curve_points: Vec<[f32; 2]>,
}
impl Default for Config {
    /// the pack's `config/smoothswapping.json`.
    fn default() -> Self {
        Self {
            enabled: true,
            animation_speed: 100,
            curve_points: Vec::new(),
        }
    }
}
impl Config {
    /// `getAnimationSpeedFormatted()`: a float, `animationSpeed / 100F`.
    pub fn speed(&self) -> f64 {
        f64::from(self.animation_speed as f32 / 100.0)
    }
}

// ---- the easing curve (`CatmullRomWidget`) ------------------------------------------------------

type Vec2 = [f64; 2];
fn add(a: Vec2, b: Vec2) -> Vec2 {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub(a: Vec2, b: Vec2) -> Vec2 {
    [a[0] - b[0], a[1] - b[1]]
}
fn mul(a: Vec2, s: f64) -> Vec2 {
    [a[0] * s, a[1] * s]
}
fn div(a: Vec2, s: f64) -> Vec2 {
    [a[0] / s, a[1] / s]
}
fn dist(a: Vec2, b: Vec2) -> f64 {
    sub(a, b)[0].hypot(sub(a, b)[1])
}

/// One piece of the curve between two of its points: a cubic.
#[derive(Clone, Debug, PartialEq)]
struct Spline {
    /// The x of its end and of its start (floats in the original).
    x: f64,
    old_x: f64,
    a: Vec2,
    b: Vec2,
    c: Vec2,
    d: Vec2,
}
impl Spline {
    /// `CatmullRomSpline(p0, p1, p2, p3)` with the original's `tension` 0 and `alpha` 0.
    fn new(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2) -> Self {
        let (tension, alpha) = (0.0f64, 0.0f64);
        let t01 = dist(p0, p1).powf(alpha);
        let t12 = dist(p1, p2).powf(alpha);
        let t23 = dist(p2, p3).powf(alpha);
        let m1 = mul(
            add(
                sub(p2, p1),
                mul(
                    sub(div(sub(p1, p0), t01), div(sub(p2, p0), t01 + t12)),
                    t12,
                ),
            ),
            1.0 - tension,
        );
        let m2 = mul(
            add(
                sub(p2, p1),
                mul(
                    sub(div(sub(p3, p2), t23), div(sub(p3, p1), t12 + t23)),
                    t12,
                ),
            ),
            1.0 - tension,
        );
        Self {
            x: f64::from(p2[0] as f32),
            old_x: f64::from(p1[0] as f32),
            a: add(mul(sub(p1, p2), 2.0), add(m1, m2)),
            b: sub(sub(sub(mul(sub(p1, p2), -3.0), m1), m1), m2),
            c: m1,
            d: p1,
        }
    }
    /// `Segment.getPoint(t)`.
    fn point(&self, t: f64) -> Vec2 {
        add(
            mul(self.a, t * t * t),
            add(mul(self.b, t * t), add(mul(self.c, t), self.d)),
        )
    }
}

/// The easing curve: where an item is along its way (0 to 1) for the share of the time that has
/// passed.
#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    splines: Vec<Spline>,
}
impl Curve {
    /// `Config.getSplines()`: the points are (0,0) twice, the options' points, (1,1) twice.
    pub fn from_points(points: &[[f32; 2]]) -> Self {
        let mut all: Vec<Vec2> = vec![[0.0, 0.0], [0.0, 0.0]];
        all.extend(points.iter().map(|p| [f64::from(p[0]), f64::from(p[1])]));
        all.extend([[1.0, 1.0], [1.0, 1.0]]);
        let splines = (1..all.len() - 2)
            .map(|i| Spline::new(all[i - 1], all[i], all[i + 1], all[i + 2]))
            .collect();
        Self { splines }
    }
    /// `CatmullRomWidget.getProgress`: the segment `t` falls in (the first one when none does),
    /// then the curve's y there.
    pub fn progress(&self, t: f64) -> f64 {
        let segment = self
            .splines
            .iter()
            .find(|s| t >= s.old_x && t < s.x)
            .unwrap_or(&self.splines[0]);
        let u = (t - segment.old_x) / (segment.x - segment.old_x);
        segment.point(u)[1]
    }
}

// ---- Java's `HashMap` order -----------------------------------------------------------------------

/// The order a `java.util.HashMap<Integer, _>` iterates keys that were put in ascending order: by
/// bucket (the key's low bits, the table being at least 16 wide and growing past three quarters
/// full), then by insertion.
pub fn java_hash_order(keys: &[usize]) -> Vec<usize> {
    let mut width = 16usize;
    while keys.len() > width * 3 / 4 {
        width *= 2;
    }
    let mut out = keys.to_vec();
    out.sort_by_key(|&k| (k & (width - 1), k));
    out
}

// ---- swaps ----------------------------------------------------------------------------------------

/// The key of the swaps that end at the cursor (`ASSUME_CURSOR_STACK_SLOT_INDEX`).
pub const CURSOR: i32 = -2;

/// What only a swap to the cursor has (`ItemToCursorInventorySwap`).
#[derive(Clone, Debug, PartialEq)]
struct ToCursor {
    started_render: bool,
    /// The identity of the cursor stack when the swap first drew, `None` until then.
    target: Option<u64>,
    arrived: bool,
}

/// One item on its way (`InventorySwap`).
#[derive(Clone, Debug, PartialEq)]
pub struct Swap<S> {
    x: f64,
    y: f64,
    distance: f64,
    start_x: f64,
    start_y: f64,
    angle: f64,
    render_destination: bool,
    checked: bool,
    amount: i32,
    item: S,
    cursor: Option<ToCursor>,
}
impl<S> Swap<S> {
    /// `new InventorySwap(from, to, item, checked, amount)`.
    fn new(from: Vec2, to: Vec2, item: S, checked: bool, amount: i32, cursor: bool) -> Self {
        let (x, y) = (to[0] - from[0], to[1] - from[1]);
        Self {
            x,
            y,
            distance: x.hypot(y),
            start_x: x,
            start_y: y,
            // `(float) (atan2(y, x) + PI)`.
            angle: f64::from((y.atan2(x) + PI) as f32),
            render_destination: false,
            checked,
            amount,
            item,
            cursor: cursor.then_some(ToCursor {
                started_render: false,
                target: None,
                arrived: false,
            }),
        }
    }
    pub fn amount(&self) -> i32 {
        self.amount
    }
    pub fn item(&self) -> &S {
        &self.item
    }
    pub fn is_checked(&self) -> bool {
        self.checked
    }
    pub fn distance(&self) -> f64 {
        self.distance
    }
    pub fn angle(&self) -> f64 {
        self.angle
    }
    pub fn position(&self) -> (f64, f64) {
        (self.x, self.y)
    }
    /// `SwapUtil.hasArrived`: the item has passed its destination, by the sign of where it is
    /// left to go in the quadrant it is heading to.
    pub fn has_arrived(&self) -> bool {
        let quadrant = (((2.0 * self.angle / PI).floor() % 4.0 + 4.0) as i32) % 4;
        let (x, y) = (self.x, self.y);
        match quadrant {
            0 => x > 0.0 && y > 0.0,
            1 => x < 0.0 && y > 0.0,
            2 => x < 0.0 && y < 0.0,
            _ => x > 0.0 && y < 0.0,
        }
    }
}

/// `SwapStacks`: a slot that changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Change<S> {
    pub slot: usize,
    pub old: S,
    pub new: S,
    /// `itemCountToChange`: old minus new count; the items still to place or to take.
    pub count: i32,
}

/// A slot of the open menu as the engine sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotView<S> {
    /// The slot's top left in the screen's own coordinates (`Slot.x`, `Slot.y`).
    pub x: f64,
    pub y: f64,
    pub stack: S,
    /// A slot that cannot be put into, such as a crafting result (`!canTakePartial`).
    pub output: bool,
}

/// What the click that just happened was (`SlotActionType`), when it matters here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickKind {
    QuickMove,
    Swap,
    Other,
}

/// Where to draw an item that is on its way: the offset from the place it would be drawn at.
#[derive(Clone, Debug, PartialEq)]
pub struct Flying<S> {
    pub item: S,
    pub dx: f64,
    pub dy: f64,
}

/// What `renderSlot` draws for a slot with swaps going to it.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelPlan<S> {
    /// The movers, drawn over the slot's place with their offsets.
    pub flying: Vec<Flying<S>>,
    /// Whether the slot's own item is drawn too (not while an item that replaces it is on its way).
    pub draw_destination: bool,
}

/// What the count and bar of a slot with swaps going to it are drawn as.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlayPlan<S> {
    /// The movers' counts (or bars): the stack to take the bar from, the text to show in place of
    /// the count (`None`: the stack's own), and the offset.
    pub flying: Vec<FlyingOverlay<S>>,
    /// The slot's own: the count it shows (what is left once the movers' are taken off), when it is
    /// shown at all.
    pub destination: Option<i32>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct FlyingOverlay<S> {
    pub stack: S,
    pub text: Option<i32>,
    pub dx: f64,
    pub dy: f64,
}

/// A click still to be looked at when the next frame begins.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Click {
    slot: usize,
    kind: ClickKind,
}

/// What the mod remembers.
#[derive(Clone, Debug)]
pub struct Smooth<S: Stack> {
    pub config: Config,
    curve: Curve,
    /// `SmoothSwapping.swaps`: by the slot they go to, or [`CURSOR`].
    swaps: BTreeMap<i32, Vec<Swap<S>>>,
    old_stacks: Vec<S>,
    current_cursor: Option<S>,
    old_cursor: Option<S>,
    click_swap: bool,
    click_swap_stack: Option<usize>,
    screen: Option<u64>,
    /// Which stack the cursor holds, as an identity: it changes when the cursor is emptied or
    /// filled, or holds another item. A count changing keeps it.
    cursor_id: u64,
    pending: Vec<Click>,
}

impl<S: Stack> Smooth<S> {
    pub fn new(config: Config) -> Self {
        let curve = Curve::from_points(&config.curve_points);
        Self {
            config,
            curve,
            swaps: BTreeMap::new(),
            old_stacks: Vec::new(),
            current_cursor: None,
            old_cursor: None,
            click_swap: false,
            click_swap_stack: None,
            screen: None,
            // 0 is the identity of an empty cursor.
            cursor_id: 1,
            pending: Vec::new(),
        }
    }

    /// `SwapUtil.reset`.
    pub fn reset(&mut self) {
        self.click_swap = false;
        self.click_swap_stack = None;
        self.swaps.clear();
        self.old_stacks.clear();
        self.old_cursor = None;
        self.pending.clear();
    }

    /// The swaps going to `slot` (or to the cursor), for tests.
    pub fn swaps_to(&self, slot: i32) -> &[Swap<S>] {
        self.swaps.get(&slot).map_or(&[], Vec::as_slice)
    }
    /// Whether any swap is pending.
    pub fn busy(&self) -> bool {
        !self.swaps.is_empty()
    }

    // ---- the click (`ClickSlotC2SPacketMixin`) --------------------------------------------------

    /// A click on `slot` (a menu index; `None` for outside the window) is sent: an item that was
    /// still on its way to that slot stops. A quick move or a swap is looked at when the next frame
    /// begins, with the menu already changed by it.
    pub fn clicked(&mut self, slot: Option<usize>, kind: ClickKind) {
        if !self.config.enabled {
            return;
        }
        let Some(slot) = slot else {
            return;
        };
        self.swaps.remove(&(slot as i32));
        if kind != ClickKind::Other {
            self.pending.push(Click { slot, kind });
        }
    }

    fn click_logic(&mut self, click: Click, slots: &[SlotView<S>], current: &[S]) {
        if self.old_stacks.len() != current.len() || click.slot >= slots.len() {
            return;
        }
        let modified: Vec<usize> = (0..current.len())
            .filter(|&i| !self.old_stacks[i].same(&current[i]))
            .collect();
        if modified.len() <= 1 {
            return;
        }
        let hover = &slots[click.slot];
        match click.kind {
            ClickKind::QuickMove if hover.output => {
                // A result that gives its stack away: it is the one the items leave from.
                if modified.contains(&click.slot) {
                    let new = &current[click.slot];
                    let old = &self.old_stacks[click.slot];
                    if new.count() - old.count() <= 0 {
                        self.click_swap_stack = Some(click.slot);
                    }
                }
            }
            ClickKind::Swap => {
                self.click_swap = true;
                for &dest in &modified {
                    if dest == click.slot {
                        continue;
                    }
                    let destination = &slots[dest];
                    let dest_old_empty = self.old_stacks[dest].is_empty();
                    if hover.output && !destination.output {
                        if dest_old_empty {
                            self.add_i2i(dest, click.slot, dest, false, destination.stack.count(), slots);
                        }
                    } else if !hover.output && !destination.output {
                        if !destination.stack.is_empty() {
                            self.add_i2i(dest, click.slot, dest, false, destination.stack.count(), slots);
                        }
                        if !hover.stack.is_empty() {
                            self.add_i2i(click.slot, dest, click.slot, false, hover.stack.count(), slots);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // ---- the frame (`HandledScreenMixin.doRender`) ---------------------------------------------

    /// A frame of a container screen begins. `slots` are the menu's slots in menu order; `cursor`
    /// is the stack on the cursor (empty when none); `mouse` is the pointer relative to the screen's
    /// top left; `screen` names the screen shown, so that a new one starts afresh.
    pub fn frame(
        &mut self,
        slots: &[SlotView<S>],
        cursor: S,
        mouse: (f64, f64),
        screen: u64,
        creative: bool,
    ) {
        if !self.config.enabled || creative {
            return;
        }
        let current: Vec<S> = slots.iter().map(|s| s.stack.clone()).collect();

        // The cursor stack as of this frame.
        let replace = match &self.current_cursor {
            None => true,
            Some(prev) => prev.count() != cursor.count() || !prev.same_item(&cursor),
        };
        if replace {
            if let Some(prev) = &self.current_cursor
                && (prev.is_empty() != cursor.is_empty() || !prev.same_item(&cursor))
            {
                self.cursor_id += 1;
            }
            self.current_cursor = Some(cursor.clone());
        }

        // The clicks of the frame before.
        for click in std::mem::take(&mut self.pending) {
            self.click_logic(click, slots, &current);
        }
        if self.click_swap {
            self.click_swap = false;
            self.old_stacks = current;
            return;
        }
        if self.screen != Some(screen) {
            self.swaps.clear();
            self.old_stacks = current;
            self.screen = Some(screen);
            return;
        }

        // What changed, in the order a `HashMap` of them is walked.
        let mut changed_slots = Vec::new();
        for (i, old) in self.old_stacks.iter().enumerate() {
            let Some(new) = current.get(i) else {
                self.reset();
                return;
            };
            if !old.same(new) {
                changed_slots.push(i);
            }
        }
        let changed: Vec<usize> = java_hash_order(&changed_slots);

        if changed.len() > 1 {
            let mut more: Vec<Change<S>> = Vec::new();
            let mut less: Vec<Change<S>> = Vec::new();
            let mut total = 0;
            for &i in &changed {
                let (old, new) = (self.old_stacks[i].clone(), current[i].clone());
                let (old_count, new_count) = (old.count(), new.count());
                if new_count > old_count && !slots[i].output {
                    more.push(Change {
                        slot: i,
                        old,
                        new,
                        count: old_count - new_count,
                    });
                    total += new_count - old_count;
                } else if new_count < old_count && !slots[i].output && self.click_swap_stack.is_none() {
                    less.push(Change {
                        slot: i,
                        old,
                        new,
                        count: old_count - new_count,
                    });
                }
            }
            if let Some(source) = self.click_swap_stack.take() {
                less.clear();
                less.push(Change {
                    slot: source,
                    old: self.old_stacks[source].clone(),
                    new: current[source].clone(),
                    count: total,
                });
            }
            if more.is_empty() {
                self.assign_i2c(&less, mouse, slots, &cursor);
            } else {
                self.assign_i2i(&mut more, &mut less, slots);
            }
        } else if changed.len() == 1 {
            // One slot changed: if it gave its items to a stack of the same item on the cursor.
            if let (Some(now), Some(before)) = (&self.current_cursor, &self.old_cursor)
                && now.same_item(before)
                && now.count() != before.count()
            {
                let i = changed[0];
                let (old, new) = (&self.old_stacks[i], &current[i]);
                let diff = now.count() - before.count();
                if (old.same_item(new) && old.count() - new.count() == diff) || new.is_empty() {
                    let change = Change {
                        slot: i,
                        old: old.clone(),
                        new: new.clone(),
                        count: old.count() - new.count(),
                    };
                    self.assign_i2c(&[change], mouse, slots, &cursor);
                }
            }
        }

        if self.old_stacks.len() != current.len()
            || self.old_stacks.iter().zip(&current).any(|(a, b)| !a.same(b))
        {
            self.old_stacks = current;
            self.old_cursor = self.current_cursor.clone();
        }
    }

    /// `SwapUtil.addI2IInventorySwap`.
    fn add_i2i(
        &mut self,
        index: usize,
        from: usize,
        to: usize,
        checked: bool,
        amount: i32,
        slots: &[SlotView<S>],
    ) {
        if slots[to].stack.is_empty() {
            return;
        }
        let swap = Swap::new(
            [slots[from].x, slots[from].y],
            [slots[to].x, slots[to].y],
            slots[to].stack.clone(),
            checked,
            amount,
            false,
        );
        self.swaps.entry(index as i32).or_default().push(swap);
    }

    /// `SwapUtil.assignI2ISwaps`: the slots that gained pair off with the slots that lost, in the
    /// order of the lists, item by item. Removing from a list while walking it by index skips the
    /// element after, as the original does.
    fn assign_i2i(&mut self, more: &mut Vec<Change<S>>, less: &mut Vec<Change<S>>, slots: &[SlotView<S>]) {
        let mut i = 0;
        while i < more.len() {
            let mut gain = more[i].clone();
            if gain.count == 0 {
                more.remove(i);
            }
            let mut rounds = 0;
            while gain.count < 0 && rounds < 64 {
                rounds += 1;
                let mut j = 0;
                while j < less.len() {
                    let mut amount = 0;
                    while less[j].count != 0 && gain.count != 0 {
                        less[j].count -= 1;
                        gain.count += 1;
                        amount += 1;
                    }
                    let from = less[j].slot;
                    let checked = gain.old.same_item(&gain.new);
                    self.add_i2i(gain.slot, from, gain.slot, checked, amount, slots);
                    if less[j].count == 0 {
                        less.remove(j);
                    }
                    if gain.count == 0 {
                        break;
                    }
                    j += 1;
                }
            }
            i += 1;
        }
    }

    /// `SwapUtil.assignI2CSwaps`: every slot that lost sends an item to the cursor, if the cursor
    /// holds something.
    fn assign_i2c(&mut self, less: &[Change<S>], mouse: (f64, f64), slots: &[SlotView<S>], cursor: &S) {
        for change in less {
            if cursor.is_empty() {
                return;
            }
            let from = &slots[change.slot];
            let swap = Swap::new(
                [from.x + 8.0, from.y + 8.0],
                [mouse.0, mouse.1],
                change.old.clone(),
                false,
                change.count,
                true,
            );
            self.swaps.entry(CURSOR).or_default().push(swap);
        }
    }

    // ---- drawing --------------------------------------------------------------------------------

    /// `renderSwap`: where the mover is drawn (relative to the place the item is drawn at), and
    /// one step of its flight: it covers a tenth of its distance per tick at speed 100.
    fn render(config: &Config, curve: &Curve, swap: &mut Swap<S>, frame_ticks: f32) -> (f64, f64) {
        let progress = 1.0 - (swap.x.hypot(swap.y) / swap.distance);
        let ease = curve.progress(progress);
        let render_x = -swap.start_x - swap.angle.cos() * swap.distance * ease;
        let render_y = swap.start_y + swap.angle.sin() * swap.distance * ease;
        let speed = swap.distance / 10.0 * config.speed();
        let step = f64::from(frame_ticks) * speed;
        swap.x += step * swap.angle.cos();
        swap.y += step * swap.angle.sin();
        (render_x, -render_y)
    }

    /// The offset a mover has at its current place (no step), for the counts drawn after it.
    fn offset(curve: &Curve, swap: &Swap<S>) -> (f64, f64) {
        let progress = 1.0 - (swap.x.hypot(swap.y) / swap.distance);
        let ease = curve.progress(progress);
        let render_x = -swap.start_x - swap.angle.cos() * swap.distance * ease;
        let render_y = swap.start_y + swap.angle.sin() * swap.distance * ease;
        (render_x, -render_y)
    }

    /// The model of the item in menu slot `index` is about to be drawn (`DrawContext.drawItem`).
    /// `Some`: the slot has items going to it; draw what the plan says instead. `None`: draw as the
    /// game does (and, while items are on their way to the cursor, this draw counts as one that
    /// lets them start).
    pub fn slot_model(&mut self, index: usize, frame_ticks: f32) -> Option<ModelPlan<S>> {
        if !self.config.enabled {
            return None;
        }
        let key = index as i32;
        if self.swaps.contains_key(&key) {
            let Self {
                config,
                curve,
                swaps,
                ..
            } = self;
            let list = swaps.get_mut(&key)?;
            let mut draw_destination = true;
            let mut flying = Vec::new();
            let mut i = 0;
            while i < list.len() {
                list[i].render_destination = list[i].checked;
                if !list[i].render_destination {
                    draw_destination = false;
                }
                let (dx, dy) = Self::render(config, curve, &mut list[i], frame_ticks);
                flying.push(Flying {
                    item: list[i].item.clone(),
                    dx,
                    dy,
                });
                if list[i].has_arrived() {
                    for s in list.iter_mut() {
                        s.render_destination = true;
                    }
                    list.remove(i);
                }
                i += 1;
            }
            if list.is_empty() {
                swaps.remove(&key);
            }
            return Some(ModelPlan {
                flying,
                draw_destination,
            });
        }
        // Not a slot with swaps: the other branch, for a draw that is not the cursor's.
        self.cursor_branch(false, self.cursor_id, frame_ticks);
        None
    }

    /// The carried stack is about to be drawn: the items on their way to the cursor are drawn as
    /// well, at offsets from where the cursor item is drawn.
    pub fn cursor_model(&mut self, frame_ticks: f32) -> Vec<Flying<S>> {
        if !self.config.enabled {
            return Vec::new();
        }
        self.cursor_branch(true, self.cursor_id, frame_ticks)
    }

    /// The branch of `doSwap` for the swaps to the cursor, run for every item drawn that has no swaps
    /// of its own. `is_cursor` is true for the carried stack (the one not among the slots).
    fn cursor_branch(&mut self, is_cursor: bool, cursor_id: u64, frame_ticks: f32) -> Vec<Flying<S>> {
        let Self {
            config,
            curve,
            swaps,
            current_cursor,
            ..
        } = self;
        let mut flying = Vec::new();
        let Some(list) = swaps.get_mut(&CURSOR) else {
            return flying;
        };
        if list.iter().any(|s| s.cursor.as_ref().is_some_and(|c| !c.arrived)) {
            let cursor_empty = current_cursor.as_ref().is_none_or(S::is_empty);
            let live = if cursor_empty { 0 } else { cursor_id };
            for swap in list.iter_mut() {
                let Some(state) = swap.cursor.as_mut() else {
                    continue;
                };
                if state.arrived {
                    continue;
                }
                if !state.started_render {
                    state.started_render = true;
                } else if state.target.is_none() || state.target == Some(live) {
                    if is_cursor {
                        if state.target.is_none() {
                            state.target = Some(live);
                        }
                        let mut moving = swap.clone();
                        let (dx, dy) = Self::render(config, curve, &mut moving, frame_ticks);
                        flying.push(Flying {
                            item: moving.item.clone(),
                            dx,
                            dy,
                        });
                        let arrived = moving.has_arrived();
                        swap.x = moving.x;
                        swap.y = moving.y;
                        if arrived && let Some(state) = swap.cursor.as_mut() {
                            state.arrived = true;
                        }
                    }
                } else {
                    state.arrived = true;
                }
            }
        }
        if list
            .iter()
            .all(|s| s.cursor.as_ref().is_none_or(|c| c.arrived))
        {
            swaps.remove(&CURSOR);
        }
        flying
    }

    /// The count and bar of the item in slot `index` (holding `stack`) are about to be drawn
    /// (`DrawContext.drawItemInSlot`). `None`: draw them as the game does.
    pub fn slot_overlay(&mut self, index: usize, stack: &S) -> Option<OverlayPlan<S>> {
        if !self.config.enabled {
            return None;
        }
        let key = index as i32;
        let list = self.swaps.get(&key)?;
        if list.iter().any(|swap| !stack.same_item(&swap.item)) {
            // The stack is no longer what the movers carry: forget them.
            self.swaps.remove(&key);
            return None;
        }
        let mut count = stack.count();
        let mut render_to_slot = true;
        let mut flying = Vec::new();
        for swap in list {
            count -= swap.amount;
            if !swap.render_destination {
                render_to_slot = false;
            }
            if swap.amount > 1 || stack.bar_visible() {
                let (dx, dy) = Self::offset(&self.curve, swap);
                flying.push(FlyingOverlay {
                    stack: stack.clone(),
                    text: (!stack.bar_visible()).then_some(swap.amount),
                    dx,
                    dy,
                });
            }
        }
        Some(OverlayPlan {
            flying,
            destination: (render_to_slot && count > 1).then_some(count),
        })
    }
}

}

/// A stack as the engine reads it: the game's stack, and whether its bar shows.
#[derive(Clone)]
struct Item {
    stack: Option<ItemStack>,
    bar: bool,
}
impl engine::Stack for Item {
    fn is_empty(&self) -> bool {
        self.stack.is_none()
    }
    fn count(&self) -> i32 {
        self.stack.as_ref().map_or(0, |s| s.count as i32)
    }
    fn same_item(&self, other: &Self) -> bool {
        match (&self.stack, &other.stack) {
            (None, None) => true,
            (Some(a), Some(b)) => a.item == b.item,
            _ => false,
        }
    }
    fn same(&self, other: &Self) -> bool {
        self.stack == other.stack
    }
    fn bar_visible(&self) -> bool {
        self.bar
    }
}

/// One slot of the screen being drawn: its index in the menu's order, its id in the game, and where
/// its item is drawn.
struct Placed {
    index: usize,
    id: usize,
    x: f32,
    y: f32,
}
/// Something to draw over the screen once its slots are done.
enum Later {
    Model(ItemStack, f32, f32),
    Overlay(ItemStack, f32, f32, Option<String>),
}
/// A slot's own count and bar, to put in place of what the game draws after its item.
struct Cleanup {
    frame: u64,
    /// Where the game's overlay begins in the batch.
    start: usize,
    stack: ItemStack,
    x: f32,
    y: f32,
    /// The count to show in its place, if any is shown.
    count: Option<i32>,
}
struct State {
    smooth: engine::Smooth<Item>,
    token: u64,
    last_frame: u64,
    ids: Vec<usize>,
    frame: u64,
    slots: Vec<Placed>,
    /// Where the carried stack is drawn this frame.
    carried_at: Option<(f32, f32)>,
    later: Vec<Later>,
    cleanup: Option<Cleanup>,
}
thread_local! {
    static SS: RefCell<State> = RefCell::new(State {
        smooth: engine::Smooth::new(engine::Config::default()),
        token: 0,
        last_frame: 0,
        ids: Vec::new(),
        frame: 0,
        slots: Vec::new(),
        carried_at: None,
        later: Vec::new(),
        cleanup: None,
    });
    /// Set while the hook draws an item itself, so that the nested call is left alone.
    static INSIDE: Cell<bool> = const { Cell::new(false) };
}

/// Replace `range` of the batch's vertices by `with`, keeping the blend runs that come after in
/// place.
fn splice(batch: &mut HudBatch, range: Range<usize>, with: Vec<HudVertex>) {
    let delta = with.len() as i64 - (range.end - range.start) as i64;
    drop(batch.vertices.splice(range.clone(), with));
    let shift = |v: u32| (i64::from(v) + delta) as u32;
    for (run, _) in &mut batch.blends {
        if run.start as usize >= range.end {
            run.start = shift(run.start);
            run.end = shift(run.end);
        } else if run.end as usize > range.start {
            run.end = shift(run.end);
        }
    }
    if batch.blur_at as usize >= range.end {
        batch.blur_at = shift(batch.blur_at);
    }
}
fn same_vertex(a: &HudVertex, b: &HudVertex) -> bool {
    a.pos == b.pos
        && a.uv == b.uv
        && a.color == b.color
        && a.depth == b.depth
        && a.glint_uv == b.glint_uv
}

impl Game<'_> {
    /// A stack as the engine reads it.
    fn smoothswapping_item(&self, stack: Option<ItemStack>) -> Item {
        let bar = stack.as_ref().is_some_and(|s| {
            let items = self.res.items();
            let max_damage = items.get(&s.item).map_or(0, |info| info.max_damage as i32);
            horizon_client::bundle::Contents::of(items, s)
                .map(|c| c.bar())
                .unwrap_or_else(|| {
                    horizon_client::hud_overlays::damage_bar(s.damage as i32, max_damage)
                })
                .is_some()
        });
        Item { stack, bar }
    }

    /// The slots of the open screen in the order of the vanilla menu (`AbstractContainerMenu.slots`),
    /// which the original numbers its slots by.
    fn smoothswapping_menu_order(&self) -> Vec<usize> {
        let stationed = self.station_slots();
        let valid: HashSet<usize> = stationed
            .clone()
            .unwrap_or_else(|| self.slots())
            .into_iter()
            .collect();
        let mut order: Vec<usize> = Vec::new();
        if stationed.is_none() && self.container.is_none() {
            // InventoryMenu / CraftingMenu: result, grid, armour, inventory, hotbar, off hand.
            let n = self.craft_size * self.craft_size;
            order.push(200);
            order.extend(100..100 + n);
            if self.craft_size == 2 {
                order.extend(500..504);
            }
            order.extend(9..36);
            order.extend(0..9);
            if self.craft_size == 2 {
                order.push(504);
            }
        } else {
            // The block's or station's own slots first (inputs, then the result), then the player's.
            let mut own: Vec<usize> = valid
                .iter()
                .copied()
                .filter(|&i| i >= 36 && i != stations::RESULT)
                .collect();
            own.sort_unstable();
            order.extend(own);
            if valid.contains(&stations::RESULT) {
                order.push(stations::RESULT);
            }
            order.extend(9..36);
            order.extend(0..9);
        }
        order.retain(|i| valid.contains(i));
        order
    }
    /// Slots nothing can be put into (`!canTakePartial`): results.
    fn smoothswapping_output(&self, id: usize) -> bool {
        id == 200
            || id == stations::RESULT
            || (id == 302 && matches!(self.container_kind(), Some(BlockEntity::Furnace(_))))
            || (id == 309 && matches!(self.container_kind(), Some(BlockEntity::Crafter(_))))
    }

    /// `ClickSlotC2SPacketMixin`: the clicks of the menu engine, before they are run.
    pub(super) fn smoothswapping_clicks(&mut self, ops: &[(Option<usize>, i32, Input)]) {
        SS.with(|s| {
            let mut s = s.borrow_mut();
            let State { smooth, slots, .. } = &mut *s;
            for &(slot, _, input) in ops {
                let index = slot.and_then(|g| slots.iter().find(|p| p.id == g).map(|p| p.index));
                let kind = match input {
                    Input::QuickMove => engine::ClickKind::QuickMove,
                    Input::Swap => engine::ClickKind::Swap,
                    _ => engine::ClickKind::Other,
                };
                smooth.clicked(index, kind);
            }
        });
    }

    /// Every tick: a closed screen leaves no frame behind.
    pub(super) fn smoothswapping_tick(&mut self) {
        if self.screen != Screen::Inventory {
            SS.with(|s| {
                let mut s = s.borrow_mut();
                s.frame = 0;
                s.later.clear();
                s.cleanup = None;
            });
        }
    }

    /// `HandledScreenMixin.render` at its head: the inventory screen begins a frame.
    pub(super) fn smoothswapping_frame(&mut self, batch: &HudBatch) {
        let (ox, oy) = self.inventory_origin(batch);
        let ids = self.smoothswapping_menu_order();
        let mut views = Vec::with_capacity(ids.len());
        let mut placed = Vec::with_capacity(ids.len());
        for (index, &id) in ids.iter().enumerate() {
            let (sx, sy) = self.slot_position(id);
            let stack = self.slot_view(id);
            views.push(engine::SlotView {
                x: f64::from(sx),
                y: f64::from(sy),
                stack: self.smoothswapping_item(stack),
                output: self.smoothswapping_output(id),
            });
            placed.push(Placed {
                index,
                id,
                x: ox + sx,
                y: oy + sy,
            });
        }
        let cursor = self.smoothswapping_item(self.carried.clone());
        let mouse = (
            f64::from(self.cursor.0 - ox),
            f64::from(self.cursor.1 - oy),
        );
        let frame = self.frames;
        SS.with(|s| {
            let mut s = s.borrow_mut();
            // A screen opened anew, or another menu in it, is another screen.
            if s.last_frame + 1 != frame || s.ids != ids {
                s.token += 1;
            }
            s.last_frame = frame;
            s.ids = ids;
            s.frame = frame;
            s.slots = placed;
            s.carried_at = None;
            s.later.clear();
            s.cleanup = None;
            let token = s.token;
            s.smooth.frame(&views, cursor, mouse, token, false);
        });
    }

    /// The game's overlay of the slot drawn last is replaced by the original's, if it is there.
    fn smoothswapping_cleanup(&self, batch: &mut HudBatch) {
        let Some(c) = SS.with(|s| s.borrow_mut().cleanup.take()) else {
            return;
        };
        if c.frame != self.frames {
            return;
        }
        let size = [
            (batch.width * batch.scale).round() as u32,
            (batch.height * batch.scale).round() as u32,
        ];
        // What the game's call drew, again, to know what to take out and to be sure of it.
        let mut game_overlay = HudBatch::new(size);
        self.draw_item_decorations(&mut game_overlay, &c.stack, c.x, c.y, None);
        let n = game_overlay.vertices.len();
        let end = c.start + n;
        if end > batch.vertices.len()
            || !batch.vertices[c.start..end]
                .iter()
                .zip(&game_overlay.vertices)
                .all(|(a, b)| same_vertex(a, b))
        {
            return;
        }
        let mut ours = HudBatch::new(size);
        if let Some(count) = c.count {
            self.draw_item_decorations(&mut ours, &c.stack, c.x, c.y, Some(count.to_string()));
        }
        batch.patches.append(&mut ours.patches);
        splice(batch, c.start..end, std::mem::take(&mut ours.vertices));
    }

    /// The things to draw over the screen: the items on their way, and their counts.
    fn smoothswapping_draw_later(&self, batch: &mut HudBatch) {
        let later = SS.with(|s| std::mem::take(&mut s.borrow_mut().later));
        for item in later {
            INSIDE.set(true);
            match item {
                Later::Model(stack, x, y) => self.draw_item_model(batch, &stack, x, y),
                Later::Overlay(stack, x, y, text) => {
                    self.draw_item_decorations(batch, &stack, x, y, text)
                }
            }
            INSIDE.set(false);
        }
    }

    /// An item's model is about to be drawn (`DrawContext.drawItem`). A slot with items on their
    /// way is drawn as the plan says: the movers are drawn later, over everything, and the slot's
    /// own item only when it is to be seen.
    pub(super) fn smoothswapping_model(
        &self,
        batch: &mut HudBatch,
        stack: &ItemStack,
        x: f32,
        y: f32,
    ) -> ControlFlow<()> {
        if INSIDE.get() {
            return ControlFlow::Continue(());
        }
        self.smoothswapping_cleanup(batch);
        let found = SS.with(|s| {
            let s = s.borrow();
            if s.frame != self.frames {
                return None;
            }
            if s.carried_at == Some((x, y)) {
                return None;
            }
            s.slots.iter().find(|p| p.x == x && p.y == y).map(|p| p.index)
        });
        let Some(index) = found else {
            return ControlFlow::Continue(());
        };
        let item = self.smoothswapping_item(Some(stack.clone()));
        let ticks = self.frame_ticks;
        let planned = SS.with(|s| {
            let mut s = s.borrow_mut();
            let plan = s.smooth.slot_model(index, ticks)?;
            let overlay = s.smooth.slot_overlay(index, &item);
            for f in &plan.flying {
                // A mover with no distance to go has no place (NaN): nothing to draw.
                if let Some(stack) = f.item.stack.clone()
                    && f.dx.is_finite()
                    && f.dy.is_finite()
                {
                    s.later
                        .push(Later::Model(stack, x + f.dx as f32, y + f.dy as f32));
                }
            }
            if let Some(overlay) = &overlay {
                for f in &overlay.flying {
                    if let Some(stack) = f.stack.stack.clone()
                        && f.dx.is_finite()
                        && f.dy.is_finite()
                    {
                        s.later.push(Later::Overlay(
                            stack,
                            x + f.dx as f32,
                            y + f.dy as f32,
                            f.text.map(|n| n.to_string()),
                        ));
                    }
                }
            }
            Some((plan.draw_destination, overlay.map(|o| o.destination)))
        });
        let Some((draw_destination, overlay)) = planned else {
            return ControlFlow::Continue(());
        };
        if draw_destination {
            INSIDE.set(true);
            self.draw_item_model(batch, stack, x, y);
            INSIDE.set(false);
        }
        if let Some(count) = overlay {
            // The game draws the slot's count and bar next; they are the original's to decide.
            let start = batch.vertices.len();
            let frame = self.frames;
            SS.with(|s| {
                s.borrow_mut().cleanup = Some(Cleanup {
                    frame,
                    start,
                    stack: stack.clone(),
                    x,
                    y,
                    count,
                })
            });
        }
        ControlFlow::Break(())
    }

    /// The carried stack is about to be drawn (`renderFloatingItem`): the items on their way to the
    /// cursor are drawn with it, and everything that was waiting for the slots to be done.
    pub(super) fn smoothswapping_carried(&self, batch: &mut HudBatch) {
        self.smoothswapping_cleanup(batch);
        if self.carried.is_some() {
            let (cx, cy) = (self.cursor.0 - 8.0, self.cursor.1 - 8.0);
            let ticks = self.frame_ticks;
            let flying = SS.with(|s| {
                let mut s = s.borrow_mut();
                s.carried_at = Some((cx, cy));
                s.smooth.cursor_model(ticks)
            });
            for f in flying {
                if let Some(stack) = f.item.stack
                    && f.dx.is_finite()
                    && f.dy.is_finite()
                {
                    INSIDE.set(true);
                    self.draw_item_model(batch, &stack, cx + f.dx as f32, cy + f.dy as f32);
                    INSIDE.set(false);
                }
            }
        }
        self.smoothswapping_draw_later(batch);
    }

    /// The end of the screen's drawing: what is still waiting is drawn (screens that draw no
    /// carried stack, such as the stations when nothing is carried).
    pub(super) fn smoothswapping_end(&self, batch: &mut HudBatch) {
        self.smoothswapping_cleanup(batch);
        self.smoothswapping_draw_later(batch);
    }
}

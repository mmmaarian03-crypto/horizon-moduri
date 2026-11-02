// Visual Workbench for Horizon: the rules, without any game code.
// Written from Visual Workbench 8.0.1 for Forge 1.20.1 (Fuzs, MPL-2.0, the jar the pack plays; sources
// github.com/Fuzss/visualworkbench branch 1.20.1) and the pack's config/visual_workbench.json and
// config/visualworkbench-client.toml. The Java class each rule follows is named next to it:
// CraftingTableBlockEntity (the 3x3 inventory, the last result, the turning animation),
// WorkbenchBlockEntityRenderer (where the items are drawn), MathHelper, ModCraftingMenu.
use std::collections::BTreeMap;
use std::f64::consts::PI;

pub type Pos = [i32; 3];

/// `ClientConfig` with the pack's `visualworkbench-client.toml`.
pub const ROTATE_INGREDIENTS: bool = true;
pub const FLAT_RENDERING: bool = false;
pub const RENDER_RESULT: bool = true;

/// `CraftingTableBlockEntity.inventory`: nine slots.
pub const SLOTS: usize = 9;

/// A crafting table's contents (`inventory` and `lastResult`).
#[derive(Clone, Debug, PartialEq)]
pub struct Table<S> {
    pub slots: Vec<Option<S>>,
    /// `lastResult`: what the grid made when the menu last changed.
    pub last: Option<S>,
}
impl<S> Default for Table<S> {
    fn default() -> Self {
        Self { slots: (0..SLOTS).map(|_| None).collect(), last: None }
    }
}
impl<S> Table<S> {
    /// `isEmpty` of the inventory (the result alone does not count).
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }
    /// Nothing to show and nothing to keep.
    pub fn is_blank(&self) -> bool {
        self.is_empty() && self.last.is_none()
    }
}

/// `MathHelper.easeOutQuad`.
pub fn ease_out_quad(t: f64, b: f32, c: f32, d: f64) -> f32 {
    let z = t as f32 / d as f32;
    -c * z * (z - 2.0) + b
}

/// The client-side turning of one table (`ticks`, `currentAngle`, `nextAngle`, ... of
/// `CraftingTableBlockEntity`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Anim {
    pub ticks: i64,
    pub current: f32,
    pub next: f32,
    sector: i32,
    animating: bool,
    start: f32,
    end: f32,
    start_ticks: f64,
    player_angle: f64,
}
impl Anim {
    /// `CraftingTableBlockEntity.clientTick`. `player` is the offset from the table's centre to the
    /// nearest player within 3 blocks (`getNearestPlayer(.., 3.0, false)`), `x` then `z`.
    pub fn tick(&mut self, empty: bool, player: Option<(f64, f64)>) {
        self.ticks += 1;
        if empty || !ROTATE_INGREDIENTS {
            return;
        }
        if let Some((d0, d1)) = player {
            self.player_angle = ((-d0).atan2(-d1) + 3.926_990_816_987_241_4) % (2.0 * PI);
        }
        let sector = (self.player_angle * 2.0 / PI) as i32;
        if self.sector != sector {
            self.animating = true;
            self.start = self.current;
            let delta1 = sector as f32 * 90.0 - self.current;
            let abs1 = delta1.abs();
            let delta2 = delta1 + 360.0;
            let shift = delta2.abs();
            let delta3 = delta1 - 360.0;
            let abs3 = delta3.abs();
            self.end = if abs3 < abs1 && abs3 < shift {
                delta3 + self.current
            } else if shift < abs1 && shift < abs3 {
                delta2 + self.current
            } else {
                delta1 + self.current
            };
            self.start_ticks = self.ticks as f64;
            self.sector = sector;
        }
        if self.animating {
            if self.ticks as f64 >= self.start_ticks + 20.0 {
                self.animating = false;
                self.current = (self.end + 360.0) % 360.0;
                self.next = self.current;
            } else {
                let span = self.end - self.start;
                self.current =
                    (ease_out_quad(self.ticks as f64 - self.start_ticks, self.start, span, 20.0)
                        + 360.0)
                        % 360.0;
                self.next = (ease_out_quad(
                    (self.ticks as f64 + 1.0 - self.start_ticks).min(20.0),
                    self.start,
                    span,
                    20.0,
                ) + 360.0)
                    % 360.0;
                if self.current != 0.0 || self.next != 0.0 {
                    if self.current == 0.0 && self.next >= 180.0 {
                        self.current = 360.0;
                    }
                    if self.next == 0.0 && self.current >= 180.0 {
                        self.next = 360.0;
                    }
                }
            }
        }
    }

    /// `Mth.lerp(partial, currentAngle, nextAngle)`.
    pub fn angle(&self, partial: f32) -> f32 {
        self.current + partial * (self.next - self.current)
    }
}

/// How an item is placed: the render calls `translate(a) rotateY(yaw) translate(b) [rotateX(90)
/// rotateZ(180)] scale(s)` in this order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub first: [f32; 3],
    pub yaw: f32,
    pub second: [f32; 3],
    /// The laying renderer turns the item flat (`rotateX(90)`, `rotateZ(180)`).
    pub flat: bool,
    pub scale: f32,
}

/// `setupFloatingRenderer` (`flat_rendering = false`); `block_item` is the model's `isGui3d`.
pub fn floating_pose(anim: &Anim, partial: f32, index: usize, block_item: bool) -> Pose {
    // -0.0125 to 0.0125
    let shift = ((f64::from(anim.ticks as f32 + partial) * 50.0 + (index as i64 * 1000) as f64)
        % 5000.0
        - 2500.0)
        .abs() as f32
        / 200_000.0;
    Pose {
        first: [0.5, shift, 0.5],
        yaw: anim.angle(partial),
        second: [
            (index % 3) as f32 * 3.0 / 16.0 + 0.3125 - 0.5,
            1.093_75,
            (index / 3) as f32 * 3.0 / 16.0 + 0.3125 - 0.5,
        ],
        flat: false,
        scale: if block_item { 0.24 } else { 0.18 },
    }
}

/// `setupLayingRenderer` (`flat_rendering = true`).
pub fn laying_pose(anim: &Anim, partial: f32, index: usize, block_item: bool) -> Pose {
    Pose {
        first: [0.5, 0.0, 0.5],
        yaw: anim.angle(partial),
        second: [
            (index % 3) as f32 * 3.0 / 16.0 + 0.3125 - 0.5,
            if block_item { 1.0625 } else { 1.005 },
            (index / 3) as f32 * 3.0 / 16.0 + 0.3125 - 0.5,
        ],
        flat: true,
        scale: if block_item { 0.25 } else { 0.175 },
    }
}

/// `renderIngredientItem`'s pose by the config.
pub fn ingredient_pose(anim: &Anim, partial: f32, index: usize, block_item: bool) -> Pose {
    if FLAT_RENDERING {
        laying_pose(anim, partial, index, block_item)
    } else {
        floating_pose(anim, partial, index, block_item)
    }
}

/// `renderResultItem`: where the result hovers, its turn in radians and its scale. `ground_y` is the
/// model's `GROUND` display scale on y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResultPose {
    pub at: [f32; 3],
    pub yaw: f32,
    pub scale: f32,
}
pub fn result_pose(time: f32, block_item: bool, ground_y: f32) -> ResultPose {
    let hover = (time / 10.0).sin() * 0.04 + 0.1;
    ResultPose {
        at: [0.5, 1.15 + hover + 0.25 * ground_y, 0.5],
        yaw: time / 20.0,
        scale: if block_item { 1.0 } else { 0.75 },
    }
}

/// The tables of one dimension of a world, by block position.
#[derive(Clone, Debug)]
pub struct Store<S> {
    pub tables: BTreeMap<Pos, Table<S>>,
}
impl<S> Default for Store<S> {
    fn default() -> Self {
        Self { tables: BTreeMap::new() }
    }
}

/// The file text of a store: `T x y z`, then `S slot stack` lines and an `R stack` line.
pub fn save<S>(store: &Store<S>, encode: impl Fn(&S) -> String) -> String {
    let mut out = String::new();
    for (p, t) in &store.tables {
        if t.is_blank() {
            continue;
        }
        out.push_str(&format!("T\t{}\t{}\t{}\n", p[0], p[1], p[2]));
        for (i, s) in t.slots.iter().enumerate() {
            if let Some(s) = s {
                out.push_str(&format!("S\t{i}\t{}\n", encode(s)));
            }
        }
        if let Some(r) = &t.last {
            out.push_str(&format!("R\t{}\n", encode(r)));
        }
    }
    out
}

/// The store of a file text; lines that do not parse are skipped (an item that is gone never
/// loses the others).
pub fn load<S>(text: &str, decode: impl Fn(&str) -> Option<S>) -> Store<S> {
    let mut store = Store::default();
    let mut at: Option<Pos> = None;
    for line in text.lines() {
        let mut f = line.splitn(3, '\t');
        match (f.next(), f.next(), f.next()) {
            (Some("T"), Some(x), Some(rest)) => {
                let mut r = rest.split('\t');
                at = match (x.parse(), r.next().and_then(|v| v.parse().ok()), r.next().and_then(|v| v.parse().ok())) {
                    (Ok(x), Some(y), Some(z)) => Some([x, y, z]),
                    _ => None,
                };
                if let Some(p) = at {
                    store.tables.entry(p).or_default();
                }
            }
            (Some("S"), Some(i), Some(s)) => {
                if let (Some(p), Ok(i)) = (at, i.parse::<usize>())
                    && i < SLOTS
                    && let Some(stack) = decode(s)
                {
                    store.tables.entry(p).or_default().slots[i] = Some(stack);
                }
            }
            (Some("R"), Some(s), rest) => {
                // the result's snbt may hold tabs only in the unlikely split form: rejoin
                let whole = match rest {
                    Some(r) => format!("{s}\t{r}"),
                    None => s.to_owned(),
                };
                if let (Some(p), Some(stack)) = (at, decode(&whole)) {
                    store.tables.entry(p).or_default().last = Some(stack);
                }
            }
            _ => {}
        }
    }
    store
}

/// The menu of a table opens: the 3x3 grid starts with the table's items (`ModCraftingMenu` is
/// built over the block entity); a table seen for the first time is created empty.
pub fn open_table<S: Clone>(store: &mut Store<S>, pos: Pos) -> Vec<Option<S>> {
    store.tables.entry(pos).or_default().slots.clone()
}

/// The grid and the result of an open table are copied to the table. True when it changed.
pub fn mirror<S: Clone + PartialEq>(
    store: &mut Store<S>,
    pos: Pos,
    grid: &[Option<S>],
    result: Option<S>,
) -> bool {
    let t = store.tables.entry(pos).or_default();
    let mut grid = grid.to_vec();
    grid.resize(SLOTS, None);
    if t.slots == grid && t.last == result {
        return false;
    }
    t.slots = grid;
    t.last = result;
    true
}

/// `CraftingTableBlock.onRemove`: the tables whose block is no longer a crafting table
/// (`is_table` answers None for an unloaded spot) are taken out of the store with their contents.
pub fn take_gone<S>(store: &mut Store<S>, is_table: impl Fn(Pos) -> Option<bool>) -> Vec<(Pos, Table<S>)> {
    let gone: Vec<Pos> = store
        .tables
        .keys()
        .copied()
        .filter(|p| is_table(*p) == Some(false))
        .collect();
    gone.into_iter()
        .filter_map(|p| store.tables.remove(&p).map(|t| (p, t)))
        .collect()
}

/// Whether a player whose menu of the table at `pos` opens takes the table's grid. The original's menu is
/// built over the one block entity, so everyone at a table shares one grid; Horizon gives each player a grid
/// of his own (the in-frame host's `craft_grid`, a server guest's menu), so a table's grid goes to one
/// player at a time: nobody else may hold it (`holders`: who holds which table of this level), and the
/// player's grid must be empty (a grid with his own items in it stays his, as at a plain crafting table).
/// A player who does not take it crafts as at a vanilla table: his items go back to him when he closes.
pub fn may_bind<W: PartialEq>(holders: &[(Pos, W)], pos: Pos, who: &W, grid_empty: bool) -> bool {
    grid_empty && !holders.iter().any(|(p, w)| *p == pos && w != who)
}

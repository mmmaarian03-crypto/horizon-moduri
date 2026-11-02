// Polymorph for Horizon: the rules, without any game code.
// Written from Polymorph 0.49.10 for Minecraft 1.20.1 (TheIllusiveC4 / illusivesoulworks, LGPL-3.0-or-later;
// behaviour read from its public description and jar assets; no code of it is used) and the pack's config.
// This file is pasted into the game as `crate::game::polymorph::engine` (tools/hnmod.py bundle) and is also the
// `horizon_polymorph` crate the tests use: plain Rust, no game types, no inner attributes (`#![...]`).
//
// What Polymorph does: when the crafting grid matches several recipes (different results), vanilla
// always makes the first one. Polymorph lists every match, lets the player pick one and remembers the
// pick: the remembered recipe is used whenever it still matches, otherwise the first match.

/// the pack's `polymorph-integrations.toml` (the only Polymorph config he has). They switch on
/// compatibility with the FastFurnace / FastSuite / FastBench mods, which are 1.20.1 Forge speed-ups
/// with no Horizon counterpart: nothing to port.
#[allow(dead_code)]
pub mod config {
    pub const FASTFURNACE: bool = true; // polymorph-integrations.toml:3 [] fastfurnace
    pub const FASTSUITE: bool = true; // polymorph-integrations.toml:4 [] fastsuite
    pub const FASTBENCH: bool = true; // polymorph-integrations.toml:5 [] fastbench
}

/// At most this many alternatives are shown (the popup has 6 columns of 3).
pub const MAX_ALTS: usize = 18;
/// Output button size, from the original's 25x25 `output_button` sprite.
pub const BUTTON: f32 = 25.0;
/// Selector (toggle) button size, from the original's 16x16 `selector_button` sprite.
pub const TOGGLE: f32 = 16.0;
const ROWS: usize = 3;

/// One recipe that matches the grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alt {
    /// Recipe id (file name without namespace), as the game's recipe list names it.
    pub id: String,
    pub item: String,
    pub count: u32,
}

/// The matches in the order Polymorph offers them: the game's own first match first (what vanilla
/// crafts), then the others in recipe order, no recipe twice, at most `MAX_ALTS`.
pub fn order(first: Option<Alt>, rest: Vec<Alt>) -> Vec<Alt> {
    let mut out: Vec<Alt> = Vec::new();
    for a in first.into_iter().chain(rest) {
        if out.len() < MAX_ALTS && !out.iter().any(|o| o.id == a.id) {
            out.push(a);
        }
    }
    out
}

/// Index of the recipe that is crafted: the remembered one while it still matches, else the first.
pub fn pick(alts: &[Alt], remembered: Option<&str>) -> usize {
    remembered
        .and_then(|r| alts.iter().position(|a| a.id == r))
        .unwrap_or(0)
}

/// Which crafting screen (its result slot decides where the buttons go).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The 2x2 grid of the inventory screen (result slot at 154,28).
    Inventory,
    /// The crafting table (result slot at 124,35).
    Table,
}
pub type Rect = (f32, f32, f32, f32);

/// The selector button, relative to the screen's top-left corner: just above the result slot.
pub fn toggle(kind: Kind) -> Rect {
    match kind {
        Kind::Table => (124.0, 15.0, TOGGLE, TOGGLE),
        Kind::Inventory => (154.0, 8.0, TOGGLE, TOGGLE),
    }
}
/// The popup's output buttons (`n` of them): columns of three, read top to bottom then left to
/// right, the last column touching the selector button's right edge.
pub fn buttons(kind: Kind, n: usize) -> Vec<Rect> {
    let n = n.min(MAX_ALTS);
    let t = toggle(kind);
    let cols = n.div_ceil(ROWS);
    let left = (t.0 + TOGGLE - cols as f32 * BUTTON).max(0.0);
    (0..n)
        .map(|i| {
            (
                left + (i / ROWS) as f32 * BUTTON,
                t.1 + TOGGLE + 2.0 + (i % ROWS) as f32 * BUTTON,
                BUTTON,
                BUTTON,
            )
        })
        .collect()
}
fn inside(r: Rect, x: f32, y: f32) -> bool {
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}
/// The popup's whole area.
fn popup(kind: Kind, n: usize) -> Option<Rect> {
    let b = buttons(kind, n);
    let first = b.first()?;
    let right = b.iter().map(|r| r.0 + r.2).fold(f32::MIN, f32::max);
    let rows = n.min(ROWS) as f32;
    Some((first.0, first.1, right - first.0, rows * BUTTON))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    /// Not ours: the game handles the click.
    Pass,
    /// The selector button: open or close the popup.
    Toggle,
    /// An output button: craft alternative `i` from now on.
    Choose(usize),
    /// Inside the popup but on no button: eaten (it must not click the slot under it).
    Swallow,
}
/// A left click at (`x`,`y`) relative to the screen's corner, with `n` matching recipes.
/// Nothing is shown (and nothing eaten) while fewer than two recipes match.
pub fn click(kind: Kind, open: bool, n: usize, x: f32, y: f32) -> Click {
    if n < 2 {
        return Click::Pass;
    }
    if inside(toggle(kind), x, y) {
        return Click::Toggle;
    }
    if !open {
        return Click::Pass;
    }
    if let Some(i) = buttons(kind, n).iter().position(|r| inside(*r, x, y)) {
        return Click::Choose(i);
    }
    match popup(kind, n) {
        Some(p) if inside(p, x, y) => Click::Swallow,
        _ => Click::Pass,
    }
}

/// The remembered recipe, saved in `<world>/data/polymorph/<dimension>.txt` (kit_state): one line, the
/// recipe id (the original keeps it in the player's data).
pub fn load(text: Option<String>) -> Option<String> {
    text.and_then(|t| t.lines().next().map(|l| l.trim().to_owned()))
        .filter(|l| !l.is_empty())
}
pub fn save(remembered: Option<&str>) -> String {
    remembered.map(|r| format!("{r}\n")).unwrap_or_default()
}

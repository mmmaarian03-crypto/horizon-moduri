// Ecran Incarcare inside Horizon's game (nightly 142388e1c). The layout, text and logo are the engine
// below (src/lib.rs); the progress counters live in horizon_mods::content::ecran_incarcare (mixins/shared.rs).
// Before the UI bundle exists (`Loading.gui` is None) the loading renderer's atlas is one black texel,
// so every quad would come out black: one patch makes it white, then solid rectangles take their colour
// from the vertex colour.
use super::Loading;
use horizon_client::hud::{HudBatch, rgba};
#[allow(dead_code)]
mod engine {
    // Ecran Incarcare for Horizon: layout, bitmap text and logo of the loading screen, without any game code.
// Pasted into the game as `crate::game::ecran_incarcare::engine` (tools/hnmod.py bundle) and also the
// `horizon_ecran_incarcare` crate the tests use: plain Rust, no game types, no inner attributes.
// Colours are Horizon's dark theme, `crates/horizon-theme/src/lib.rs` (the dark `Blocks`, lines 385-399):
// page 0x0f0d17, well 0x0c0b12, bevel 0x342d52, ink_soft 0xc5c7d1, muted 0xa29cb8, accent 0x9a86ff.

pub mod config {
    pub const PAGE: u32 = 0x0f0d17;
    pub const WELL: u32 = 0x0c0b12;
    pub const BORDER: u32 = 0x342d52;
    pub const ACCENT: u32 = 0x9a86ff;
    /// A lighter violet for the stripe that sweeps the filled part (liveness while the number waits).
    pub const SHEEN: u32 = 0xc9beff;
    pub const TEXT: u32 = 0xc5c7d1;
    pub const NOTE: u32 = 0xa29cb8;
    /// Seconds a sweep of the stripe / one way of the indeterminate segment takes.
    pub const SWEEP_SECONDS: f32 = 1.6;
}

/// One solid rectangle in GUI pixels; `color` is `0xRRGGBB`, always opaque.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fill {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: u32,
}

/// What the screen says.
pub struct View<'a> {
    pub line: &'a str,
    pub note: Option<&'a str>,
    /// `None`: no measurement yet, the bar animates.
    pub fraction: Option<f32>,
    /// Memory line (`RAM: 1.4 GB OF 31.8 GB`); `None` = not available, nothing is drawn.
    pub memory: Option<&'a str>,
}

// ---------------------------------------------------------------- memory line

const GIB: u64 = 1 << 30;

/// The screen's texts are English; `true` gives the Romanian ones (set it here and in
/// `mixins/ecran_incarcare/shared.rs`). The font draws Romanian's accented letters too.
pub const ROMANIAN: bool = false;

/// `RAM: 1.4 GB OF 31.8 GB` (GB = 2^30 bytes, one decimal); with [`ROMANIAN`], `RAM: 1,4 GB DIN 31,8 GB`.
/// `None` when the total is 0.
pub fn format_memory(used: u64, total: u64) -> Option<String> {
    format_memory_in(used, total, ROMANIAN)
}

/// [`format_memory`] in English, or in Romanian (decimal comma) when `romanian` is set.
pub fn format_memory_in(used: u64, total: u64, romanian: bool) -> Option<String> {
    if total == 0 {
        return None;
    }
    let (point, of) = if romanian { (',', "DIN") } else { ('.', "OF") };
    let gb = |b: u64| {
        let tenths = ((b as u128 * 10 + GIB as u128 / 2) / GIB as u128) as u64;
        format!("{}{point}{}", tenths / 10, tenths % 10)
    };
    Some(format!("RAM: {} GB {of} {} GB", gb(used), gb(total)))
}

/// Seconds between two memory queries.
pub const MEMORY_INTERVAL: f32 = 0.5;

/// Decides when the memory is queried again: about twice a second.
#[derive(Clone, Copy, Debug, Default)]
pub struct MemoryGate {
    last: Option<f32>,
}

impl MemoryGate {
    pub const fn new() -> Self {
        Self { last: None }
    }
    /// `true` the first time and then once every [`MEMORY_INTERVAL`] of `time` (seconds); also when
    /// the clock went backwards (a new screen).
    pub fn due(&mut self, time: f32) -> bool {
        let go = match self.last {
            None => true,
            Some(t) => time < t || time - t >= MEMORY_INTERVAL,
        };
        if go {
            self.last = Some(time);
        }
        go
    }
}

// ---------------------------------------------------------------- bitmap font (5x7, capitals)

/// Rows top to bottom, `#` = lit. Capitals, digits and the few signs the screen uses; accents are
/// drawn over/under a base letter ([`split_accent`]).
const FONT: &[(char, [&str; 7])] = &[
    ('A', [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('B', ["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."]),
    ('C', [".####", "#....", "#....", "#....", "#....", "#....", ".####"]),
    ('D', ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."]),
    ('E', ["#####", "#....", "#....", "####.", "#....", "#....", "#####"]),
    ('F', ["#####", "#....", "#....", "####.", "#....", "#....", "#...."]),
    ('G', [".####", "#....", "#....", "#.###", "#...#", "#...#", ".####"]),
    ('H', ["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('I', [".###.", "..#..", "..#..", "..#..", "..#..", "..#..", ".###."]),
    ('J', ["..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##.."]),
    ('K', ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"]),
    ('L', ["#....", "#....", "#....", "#....", "#....", "#....", "#####"]),
    ('M', ["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"]),
    ('N', ["#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#"]),
    ('O', [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('P', ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."]),
    ('Q', [".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"]),
    ('R', ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"]),
    ('S', [".####", "#....", "#....", ".###.", "....#", "....#", "####."]),
    ('T', ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."]),
    ('U', ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('V', ["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."]),
    ('W', ["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "##.##", "#...#"]),
    ('X', ["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"]),
    ('Y', ["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."]),
    ('Z', ["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"]),
    ('0', [".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."]),
    ('1', ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."]),
    ('2', [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"]),
    ('3', ["####.", "....#", "....#", ".###.", "....#", "....#", "####."]),
    ('4', ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."]),
    ('5', ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."]),
    ('6', [".###.", "#....", "#....", "####.", "#...#", "#...#", ".###."]),
    ('7', ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."]),
    ('8', [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."]),
    ('9', [".###.", "#...#", "#...#", ".####", "....#", "....#", ".###."]),
    (':', [".....", "..#..", "..#..", ".....", "..#..", "..#..", "....."]),
    ('/', ["....#", "....#", "...#.", "..#..", ".#...", "#....", "#...."]),
    ('%', ["##..#", "##..#", "...#.", "..#..", ".#...", "#..##", "#..##"]),
    ('.', [".....", ".....", ".....", ".....", ".....", ".....", "..#.."]),
    (',', [".....", ".....", ".....", ".....", ".....", "..#..", ".#..."]),
    ('-', [".....", ".....", ".....", "#####", ".....", ".....", "....."]),
    ('(', ["...#.", "..#..", ".#...", ".#...", ".#...", "..#..", "...#."]),
    (')', [".#...", "..#..", "...#.", "...#.", "...#.", "..#..", ".#..."]),
    (' ', [".....", ".....", ".....", ".....", ".....", ".....", "....."]),
];

pub const GLYPH_W: i32 = 5;
pub const GLYPH_H: i32 = 7;
/// Glyph width plus one column of space, in font pixels.
pub const ADVANCE: i32 = 6;

/// The marks Romanian puts on letters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    None,
    /// ă: a breve above.
    Breve,
    /// â, î: a circumflex above.
    Circumflex,
    /// ș, ț: a comma below.
    Comma,
}

/// A character as a capital base letter and its mark (`'ă'` -> `('A', Breve)`); lower case is
/// folded to capitals because the font only has those.
pub fn split_accent(c: char) -> (char, Mark) {
    match c {
        'ă' | 'Ă' => ('A', Mark::Breve),
        'â' | 'Â' | 'î' | 'Î' => (
            if matches!(c, 'â' | 'Â') { 'A' } else { 'I' },
            Mark::Circumflex,
        ),
        'ș' | 'Ș' | 'ş' | 'Ş' => ('S', Mark::Comma),
        'ț' | 'Ț' | 'ţ' | 'Ţ' => ('T', Mark::Comma),
        _ => (c.to_uppercase().next().unwrap_or(c), Mark::None),
    }
}

/// The rows of a glyph (bit 4 = leftmost column), `None` for a character the font lacks.
pub fn glyph(c: char) -> Option<[u8; 7]> {
    let rows = FONT.iter().find(|(g, _)| *g == c)?.1;
    let mut out = [0u8; 7];
    for (i, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch == '#' {
                out[i] |= 1 << (4 - x);
            }
        }
    }
    Some(out)
}

/// Width in GUI pixels of `text` at `scale` GUI pixels per font pixel.
pub fn text_width(text: &str, scale: i32) -> i32 {
    let n = text.chars().count() as i32;
    if n == 0 { 0 } else { (n * ADVANCE - 1) * scale }
}

/// Appends the rectangles of `text` with its top-left glyph corner at (`x`, `y`); marks use the two
/// rows above and below the 7-row cell.
pub fn text_fills(text: &str, x: i32, y: i32, scale: i32, color: u32, out: &mut Vec<Fill>) {
    let mut at = x;
    let dot = |px: i32, py: i32, out: &mut Vec<Fill>, at: i32| {
        out.push(Fill {
            x: (at + px * scale) as f32,
            y: (y + py * scale) as f32,
            w: scale as f32,
            h: scale as f32,
            color,
        });
    };
    for c in text.chars() {
        let (base, mark) = split_accent(c);
        if let Some(rows) = glyph(base) {
            for (row, bits) in rows.iter().enumerate() {
                // Horizontal runs in one rectangle each.
                let mut col = 0;
                while col < GLYPH_W {
                    if bits & (1 << (4 - col)) == 0 {
                        col += 1;
                        continue;
                    }
                    let start = col;
                    while col < GLYPH_W && bits & (1 << (4 - col)) != 0 {
                        col += 1;
                    }
                    out.push(Fill {
                        x: (at + start * scale) as f32,
                        y: (y + row as i32 * scale) as f32,
                        w: ((col - start) * scale) as f32,
                        h: scale as f32,
                        color,
                    });
                }
            }
        }
        match mark {
            Mark::None => {}
            Mark::Breve => {
                dot(1, -2, out, at);
                dot(3, -2, out, at);
                dot(2, -1, out, at);
            }
            Mark::Circumflex => {
                dot(2, -2, out, at);
                dot(1, -1, out, at);
                dot(3, -1, out, at);
            }
            Mark::Comma => {
                dot(2, GLYPH_H, out, at);
                dot(1, GLYPH_H + 1, out, at);
            }
        }
        at += ADVANCE * scale;
    }
}

// ---------------------------------------------------------------- the bar

/// The coloured spans inside the bar's inner width `inner` (pixels): `(x, w, color)`, left to right,
/// all inside `0..=inner`. Determinate: the accent fill with a lighter stripe sweeping over it.
/// Indeterminate: an accent segment going back and forth.
pub fn bar_spans(inner: i32, fraction: Option<f32>, time: f32) -> Vec<(i32, i32, u32)> {
    use config::*;
    let mut out = Vec::new();
    if inner <= 0 {
        return out;
    }
    let phase = (time / SWEEP_SECONDS).max(0.0);
    match fraction {
        Some(f) => {
            let fill = ((inner as f32) * f.clamp(0.0, 1.0)).round() as i32;
            if fill <= 0 {
                return out;
            }
            let stripe = (inner / 6).max(8).min(fill);
            // The stripe enters at the left edge and leaves at the fill's right edge.
            let travel = fill + stripe;
            let head = ((phase.fract() * travel as f32) as i32) - stripe;
            let (a, b) = (head.clamp(0, fill), (head + stripe).clamp(0, fill));
            if a > 0 {
                out.push((0, a, ACCENT));
            }
            if b > a {
                out.push((a, b - a, SHEEN));
            }
            if b < fill {
                out.push((b, fill - b, ACCENT));
            }
        }
        None => {
            let seg = (inner * 3 / 10).max(1);
            // Ping-pong 0..1..0.
            let p = phase % 2.0;
            let p = if p > 1.0 { 2.0 - p } else { p };
            let x = (p * (inner - seg) as f32).round() as i32;
            out.push((x, seg, ACCENT));
        }
    }
    out
}

// ---------------------------------------------------------------- layout

/// Where everything sits on a GUI of `w` x `h`, integers in GUI pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// GUI pixels per logo cell.
    pub cell: i32,
    pub logo: (i32, i32),
    /// Outer rectangle of the bar (x, y, w, h); the border is 1 pixel.
    pub bar: (i32, i32, i32, i32),
    pub text_scale: i32,
    pub line: (i32, i32),
    pub note_scale: i32,
    pub note: (i32, i32),
    pub memory_scale: i32,
    pub memory: (i32, i32),
}

/// The widest `scale` (at most `max`, at least 1) at which `text` fits in `room` pixels.
pub fn fit_scale(text: &str, max: i32, room: i32) -> i32 {
    let mut s = max.max(1);
    while s > 1 && text_width(text, s) > room {
        s -= 1;
    }
    s
}

pub fn layout(w: i32, h: i32, line: &str, note: Option<&str>) -> Layout {
    layout_with(w, h, line, note, None)
}

pub fn layout_with(w: i32, h: i32, line: &str, note: Option<&str>, memory: Option<&str>) -> Layout {
    let grid = LOGO_GRID as i32;
    // The logo takes about 30% of the height, but never more than half of it.
    let cell = ((h * 3 / 10) / grid).min((h / 2) / grid).max(1);
    let logo_size = grid * cell;
    let room = (w - 16).max(1);
    let text_scale = fit_scale(line, cell.max(2), room);
    let note_scale = note.map_or(1, |n| fit_scale(n, (text_scale / 2).max(1), room));
    let bar_w = (logo_size * 2).max(120).min(room);
    let bar_h = (4 * cell).max(4);
    let gap_logo = 6 * cell + 2;
    let gap_text = 3 * text_scale + 2;
    let gap_note = 4 * note_scale;
    // Cell of text: 2 rows of marks above, 7 of letter, 2 below.
    let line_h = (GLYPH_H + 4) * text_scale;
    let note_h = note.map_or(0, |_| gap_note + (GLYPH_H + 4) * note_scale);
    let memory_scale = memory.map_or(1, |m| fit_scale(m, note_scale, room));
    let gap_memory = 3 * memory_scale;
    let memory_h = memory.map_or(0, |_| gap_memory + (GLYPH_H + 4) * memory_scale);
    let total = logo_size + gap_logo + bar_h + gap_text + line_h + note_h + memory_h;
    let top = ((h - total) / 2).max(0);
    let logo = ((w - logo_size) / 2, top);
    let bar_y = top + logo_size + gap_logo;
    let bar = ((w - bar_w) / 2, bar_y, bar_w, bar_h);
    let line_top = bar_y + bar_h + gap_text + 2 * text_scale;
    let line_x = (w - text_width(line, text_scale)) / 2;
    let note_top = bar_y + bar_h + gap_text + line_h + gap_note + 2 * note_scale;
    let note_x = note.map_or(0, |n| (w - text_width(n, note_scale)) / 2);
    let memory_top = bar_y + bar_h + gap_text + line_h + note_h + gap_memory + 2 * memory_scale;
    let memory_x = memory.map_or(0, |m| (w - text_width(m, memory_scale)) / 2);
    Layout {
        cell,
        logo,
        bar,
        text_scale,
        line: (line_x.max(0), line_top),
        note_scale,
        note: (note_x.max(0), note_top),
        memory_scale,
        memory: (memory_x.max(0), memory_top),
    }
}

/// Every rectangle of the screen, back to front: page, logo, bar, text. `time` is seconds since
/// the window opened.
pub fn scene(w: f32, h: f32, time: f32, view: &View<'_>) -> Vec<Fill> {
    use config::*;
    let (wi, hi) = (w.floor() as i32, h.floor() as i32);
    let l = layout_with(wi, hi, view.line, view.note, view.memory);
    let mut out = vec![Fill { x: 0.0, y: 0.0, w, h, color: PAGE }];
    for &(x, y, n, color) in LOGO.iter() {
        out.push(Fill {
            x: (l.logo.0 + x as i32 * l.cell) as f32,
            y: (l.logo.1 + y as i32 * l.cell) as f32,
            w: (n as i32 * l.cell) as f32,
            h: l.cell as f32,
            color,
        });
    }
    let (bx, by, bw, bh) = l.bar;
    let rect = |x: i32, y: i32, w: i32, h: i32, color: u32| Fill {
        x: x as f32,
        y: y as f32,
        w: w as f32,
        h: h as f32,
        color,
    };
    out.push(rect(bx, by, bw, bh, BORDER));
    out.push(rect(bx + 1, by + 1, bw - 2, bh - 2, WELL));
    for (x, sw, color) in bar_spans(bw - 2, view.fraction, time) {
        out.push(rect(bx + 1 + x, by + 1, sw, bh - 2, color));
    }
    text_fills(view.line, l.line.0, l.line.1, l.text_scale, TEXT, &mut out);
    if let Some(note) = view.note {
        text_fills(note, l.note.0, l.note.1, l.note_scale, NOTE, &mut out);
    }
    if let Some(memory) = view.memory {
        text_fills(memory, l.memory.0, l.memory.1, l.memory_scale, NOTE, &mut out);
    }
    out
}

// LOGO-BEGIN (tools/gen_logo.py: horizon.png 256x256, grila 48, pas 24, 144 runde)
pub const LOGO_GRID: u32 = 48;
/// (x, y, lungime, 0xRRGGBB) pe grila LOGO_GRID, runde orizontale.
pub const LOGO: [(u8, u8, u8, u32); 144] = [
    (7, 0, 34, 0x181818),
    (5, 1, 38, 0x181818),
    (4, 2, 40, 0x181818),
    (3, 3, 42, 0x181818),
    (2, 4, 44, 0x181818),
    (1, 5, 46, 0x181818),
    (1, 6, 46, 0x181818),
    (0, 7, 48, 0x181818),
    (0, 8, 48, 0x181818),
    (0, 9, 12, 0x181818),
    (12, 9, 6, 0xf0f0f0),
    (18, 9, 12, 0x181818),
    (30, 9, 6, 0xf0f0f0),
    (36, 9, 12, 0x181818),
    (0, 10, 12, 0x181818),
    (12, 10, 6, 0xf0f0f0),
    (18, 10, 12, 0x181818),
    (30, 10, 6, 0xf0f0f0),
    (36, 10, 12, 0x181818),
    (0, 11, 12, 0x181818),
    (12, 11, 6, 0xf0f0f0),
    (18, 11, 12, 0x181818),
    (30, 11, 6, 0xf0f0f0),
    (36, 11, 12, 0x181818),
    (0, 12, 12, 0x181818),
    (12, 12, 6, 0xf0f0f0),
    (18, 12, 12, 0x181818),
    (30, 12, 6, 0xf0f0f0),
    (36, 12, 12, 0x181818),
    (0, 13, 12, 0x181818),
    (12, 13, 6, 0xf0f0f0),
    (18, 13, 12, 0x181818),
    (30, 13, 6, 0xf0f0f0),
    (36, 13, 12, 0x181818),
    (0, 14, 12, 0x181818),
    (12, 14, 6, 0xf0f0f0),
    (18, 14, 12, 0x181818),
    (30, 14, 6, 0xf0f0f0),
    (36, 14, 12, 0x181818),
    (0, 15, 12, 0x181818),
    (12, 15, 6, 0xf0f0f0),
    (18, 15, 12, 0x181818),
    (30, 15, 6, 0xf0f0f0),
    (36, 15, 12, 0x181818),
    (0, 16, 12, 0x181818),
    (12, 16, 6, 0xf0f0f0),
    (18, 16, 12, 0x181818),
    (30, 16, 6, 0xf0f0f0),
    (36, 16, 12, 0x181818),
    (0, 17, 12, 0x181818),
    (12, 17, 6, 0xf0f0f0),
    (18, 17, 12, 0x181818),
    (30, 17, 6, 0xf0f0f0),
    (36, 17, 12, 0x181818),
    (0, 18, 12, 0x181818),
    (12, 18, 6, 0xf0f0f0),
    (18, 18, 12, 0x181818),
    (30, 18, 6, 0xf0f0f0),
    (36, 18, 12, 0x181818),
    (0, 19, 12, 0x181818),
    (12, 19, 6, 0xf0f0f0),
    (18, 19, 12, 0x181818),
    (30, 19, 6, 0xf0f0f0),
    (36, 19, 12, 0x181818),
    (0, 20, 12, 0x181818),
    (12, 20, 6, 0xf0f0f0),
    (18, 20, 12, 0x181818),
    (30, 20, 6, 0xf0f0f0),
    (36, 20, 12, 0x181818),
    (0, 21, 48, 0x9090ff),
    (0, 22, 48, 0x9090ff),
    (0, 23, 48, 0x9090ff),
    (0, 24, 12, 0x181818),
    (12, 24, 6, 0x606060),
    (18, 24, 12, 0x181818),
    (30, 24, 6, 0x606060),
    (36, 24, 12, 0x181818),
    (0, 25, 12, 0x181818),
    (12, 25, 6, 0x606060),
    (18, 25, 12, 0x181818),
    (30, 25, 6, 0x606060),
    (36, 25, 12, 0x181818),
    (0, 26, 12, 0x181818),
    (12, 26, 6, 0x606060),
    (18, 26, 12, 0x181818),
    (30, 26, 6, 0x606060),
    (36, 26, 12, 0x181818),
    (0, 27, 12, 0x181818),
    (12, 27, 6, 0x606060),
    (18, 27, 12, 0x181818),
    (30, 27, 6, 0x606060),
    (36, 27, 12, 0x181818),
    (0, 28, 12, 0x181818),
    (12, 28, 6, 0x606060),
    (18, 28, 12, 0x181818),
    (30, 28, 6, 0x606060),
    (36, 28, 12, 0x181818),
    (0, 29, 12, 0x181818),
    (12, 29, 6, 0x606060),
    (18, 29, 12, 0x181818),
    (30, 29, 6, 0x606060),
    (36, 29, 12, 0x181818),
    (0, 30, 12, 0x181818),
    (12, 30, 6, 0x606060),
    (18, 30, 12, 0x181818),
    (30, 30, 6, 0x606060),
    (36, 30, 12, 0x181818),
    (0, 31, 12, 0x181818),
    (12, 31, 6, 0x606060),
    (18, 31, 12, 0x181818),
    (30, 31, 6, 0x606060),
    (36, 31, 12, 0x181818),
    (0, 32, 12, 0x181818),
    (12, 32, 6, 0x606060),
    (18, 32, 12, 0x181818),
    (30, 32, 6, 0x606060),
    (36, 32, 12, 0x181818),
    (0, 33, 12, 0x181818),
    (12, 33, 6, 0x606060),
    (18, 33, 12, 0x181818),
    (30, 33, 6, 0x606060),
    (36, 33, 12, 0x181818),
    (0, 34, 12, 0x181818),
    (12, 34, 6, 0x606060),
    (18, 34, 12, 0x181818),
    (30, 34, 6, 0x606060),
    (36, 34, 12, 0x181818),
    (0, 35, 12, 0x181818),
    (12, 35, 6, 0x606060),
    (18, 35, 12, 0x181818),
    (30, 35, 6, 0x606060),
    (36, 35, 12, 0x181818),
    (0, 36, 48, 0x181818),
    (0, 37, 48, 0x181818),
    (0, 38, 48, 0x181818),
    (0, 39, 48, 0x181818),
    (0, 40, 48, 0x181818),
    (1, 41, 46, 0x181818),
    (1, 42, 46, 0x181818),
    (2, 43, 44, 0x181818),
    (3, 44, 42, 0x181818),
    (4, 45, 40, 0x181818),
    (5, 46, 38, 0x181818),
    (7, 47, 34, 0x181818),
];
// LOGO-END

}

thread_local! {
    static MEMORY: std::cell::RefCell<(engine::MemoryGate, Option<String>)> =
        const { std::cell::RefCell::new((engine::MemoryGate::new(), None)) };
}

/// The memory line, re-queried about twice a second; a failed query keeps the last text (or none).
fn ecran_incarcare_memory(time: f32) -> Option<String> {
    MEMORY.with(|m| {
        let mut m = m.borrow_mut();
        if m.0.due(time) {
            if let Some((used, total)) = horizon_mods::content::ecran_incarcare::memory_usage() {
                m.1 = engine::format_memory(used, total).or(m.1.take());
            }
        }
        m.1.clone()
    })
}

impl Loading {
    /// HEAD of `startup_menu::Loading::draw_menu`. `true` = the screen was drawn (skip the original,
    /// which draws nothing without the atlas); `false` = the atlas exists, the game draws as before.
    pub(super) fn ecran_incarcare_draw(&mut self, batch: &mut HudBatch) -> bool {
        if self.gui.is_some() {
            return false;
        }
        batch.patches.push(horizon_client::font::Patch {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            rgba: vec![255; 4],
        });
        let snapshot = horizon_mods::content::ecran_incarcare::snapshot();
        let line = snapshot.line();
        let time = self.started.elapsed().as_secs_f32();
        let memory = ecran_incarcare_memory(time);
        let view = engine::View {
            memory: memory.as_deref(),
            line: &line,
            note: snapshot.note(),
            fraction: horizon_mods::content::ecran_incarcare::fraction(),
        };
        for f in engine::scene(batch.width, batch.height, time, &view) {
            let color = rgba((f.color >> 16) as u8, (f.color >> 8) as u8, f.color as u8, 255);
            batch.quad(
                [[f.x, f.y], [f.x + f.w, f.y], [f.x + f.w, f.y + f.h], [f.x, f.y + f.h]],
                [[0.5, 0.5]; 4],
                color,
            );
        }
        true
    }
}

// Layout, text and logo of the loading screen (src/lib.rs).
use horizon_ecran_incarcare::*;

fn view<'a>(line: &'a str, note: Option<&'a str>, fraction: Option<f32>) -> View<'a> {
    View { line, note, fraction, memory: None }
}

const NOTE: &str = "Nu închide jocul: prima pornire durează mai mult";
const LINE: &str = "Se pregătesc modurile: 23/60 (41%)";

#[test]
fn every_glyph_is_five_wide_and_the_alphabet_is_complete() {
    for c in ('A'..='Z').chain('0'..='9').chain(": /%.,-()".chars()) {
        assert!(glyph(c).is_some(), "lipseste {c:?}");
    }
    for c in ('A'..='Z').chain('0'..='9') {
        assert!(glyph(c).unwrap().iter().all(|r| r & 0xe0 == 0), "{c}");
        assert!(glyph(c).unwrap().iter().any(|r| *r != 0), "{c} gol");
    }
}

#[test]
fn every_character_of_the_texts_can_be_drawn() {
    let texts = [
        LINE,
        "Se pregătesc modurile: tabele 7/20",
        "Se pregătesc modurile (60)...",
        "Se pornește Horizon...",
        NOTE,
    ];
    for t in texts {
        for c in t.chars() {
            let (base, _) = split_accent(c);
            assert!(glyph(base).is_some(), "{c:?} in {t:?}");
        }
    }
}

#[test]
fn accents_split_into_base_and_mark() {
    assert_eq!(split_accent('ă'), ('A', Mark::Breve));
    assert_eq!(split_accent('â'), ('A', Mark::Circumflex));
    assert_eq!(split_accent('î'), ('I', Mark::Circumflex));
    assert_eq!(split_accent('Î'), ('I', Mark::Circumflex));
    assert_eq!(split_accent('ș'), ('S', Mark::Comma));
    assert_eq!(split_accent('ț'), ('T', Mark::Comma));
    assert_eq!(split_accent('ş'), ('S', Mark::Comma));
    assert_eq!(split_accent('m'), ('M', Mark::None));
}

#[test]
fn text_width_counts_advance_without_the_trailing_gap() {
    assert_eq!(text_width("", 3), 0);
    assert_eq!(text_width("A", 1), 5);
    assert_eq!(text_width("AB", 2), 22);
    let mut out = Vec::new();
    text_fills("AB", 10, 4, 2, 0xffffff, &mut out);
    let right = out.iter().map(|f| f.x + f.w).fold(0.0, f32::max);
    assert!(right <= (10 + text_width("AB", 2)) as f32);
    assert!(out.iter().all(|f| f.w > 0.0 && f.h > 0.0));
}

#[test]
fn marks_sit_above_or_below_the_letter_cell() {
    let mut a = Vec::new();
    text_fills("Ă", 0, 20, 1, 1, &mut a);
    assert!(a.iter().any(|f| f.y < 20.0), "breve deasupra");
    let mut s = Vec::new();
    text_fills("Ș", 0, 20, 1, 1, &mut s);
    assert!(s.iter().any(|f| f.y >= 27.0), "virgula dedesubt");
    let mut plain = Vec::new();
    text_fills("S", 0, 20, 1, 1, &mut plain);
    assert!(plain.iter().all(|f| f.y >= 20.0 && f.y < 27.0));
}

#[test]
fn fit_scale_shrinks_until_it_fits_but_not_below_one() {
    assert_eq!(fit_scale(LINE, 2, 10_000), 2);
    let w2 = text_width(LINE, 2);
    assert_eq!(fit_scale(LINE, 2, w2 - 1), 1);
    assert_eq!(fit_scale(LINE, 5, 1), 1);
}

#[test]
fn layout_is_centred_and_inside_the_screen_for_common_sizes() {
    for (w, h) in [(320, 240), (427, 240), (480, 270), (640, 360), (960, 540), (1280, 720), (1920, 1080), (2560, 1440)] {
        let l = layout(w, h, LINE, Some(NOTE));
        let logo = LOGO_GRID as i32 * l.cell;
        assert!(l.cell >= 1);
        assert_eq!(l.logo.0, (w - logo) / 2, "{w}x{h}");
        assert!(l.logo.0 >= 0 && l.logo.1 >= 0 && l.logo.0 + logo <= w && l.logo.1 + logo <= h, "{w}x{h} logo");
        let (bx, by, bw, bh) = l.bar;
        assert!(bx >= 0 && bx + bw <= w && by + bh <= h, "{w}x{h} bar");
        assert_eq!(bx, (w - bw) / 2);
        assert!(by >= l.logo.1 + logo, "bara sub logo");
        assert!(l.line.1 > by + bh, "textul sub bara");
        let lw = text_width(LINE, l.text_scale);
        assert!(l.line.0 >= 0 && l.line.0 + lw <= w, "{w}x{h} text {lw}");
        assert!(l.note.1 > l.line.1 + GLYPH_H * l.text_scale, "nota sub linie");
        let nw = text_width(NOTE, l.note_scale);
        assert!(l.note.0 >= 0 && l.note.0 + nw <= w, "{w}x{h} nota {nw}");
        assert!(l.note.1 + (GLYPH_H + 2) * l.note_scale <= h, "{w}x{h} nota jos");
    }
}

#[test]
fn logo_grows_with_the_screen_but_stays_under_half_of_it() {
    let mut last = 0;
    for h in [240, 480, 720, 1080, 1440] {
        let l = layout(h * 16 / 9, h, "X", None);
        let size = LOGO_GRID as i32 * l.cell;
        assert!(size >= last);
        assert!(size * 2 <= h || l.cell == 1);
        last = size;
    }
}

#[test]
fn bar_determinate_fill_matches_fraction_and_stays_inside() {
    let inner = 200;
    for f in [0.0f32, 0.01, 0.25, 0.5, 0.99, 1.0] {
        for t in [0.0f32, 0.3, 0.8, 1.59, 1.6, 7.7] {
            let spans = bar_spans(inner, Some(f), t);
            let total: i32 = spans.iter().map(|s| s.1).sum();
            let want = (inner as f32 * f).round() as i32;
            assert_eq!(total, want, "f={f} t={t}");
            let mut at = 0;
            for (x, w, _) in &spans {
                assert_eq!(*x, at, "contigue");
                assert!(*w > 0);
                at += w;
            }
            assert!(at <= inner);
        }
    }
}

#[test]
fn bar_stripe_moves_and_wraps() {
    let a = bar_spans(300, Some(1.0), 0.2);
    let b = bar_spans(300, Some(1.0), 0.9);
    assert_ne!(a, b);
    let start = bar_spans(300, Some(1.0), 0.0);
    let wrapped = bar_spans(300, Some(1.0), config::SWEEP_SECONDS);
    assert_eq!(start, wrapped);
}

#[test]
fn bar_indeterminate_ping_pongs_inside() {
    let inner = 100;
    let mut xs = Vec::new();
    for i in 0..40 {
        let t = i as f32 * 0.1;
        let spans = bar_spans(inner, None, t);
        assert_eq!(spans.len(), 1);
        let (x, w, c) = spans[0];
        assert_eq!(c, config::ACCENT);
        assert_eq!(w, 30);
        assert!(x >= 0 && x + w <= inner, "t={t} x={x}");
        xs.push(x);
    }
    assert!(xs.iter().any(|x| *x == 0) && xs.iter().any(|x| *x >= 69));
    assert!(bar_spans(0, None, 1.0).is_empty());
}

#[test]
fn scene_draws_page_first_and_stays_in_the_screen() {
    for (w, h) in [(427.0f32, 240.0f32), (960.0, 540.0), (1920.0, 1080.0)] {
        let s = scene(w, h, 3.1, &view(LINE, Some("Nu închide jocul"), Some(0.4)));
        assert_eq!(s[0], Fill { x: 0.0, y: 0.0, w, h, color: config::PAGE });
        for f in &s {
            assert!(f.w > 0.0 && f.h > 0.0);
            assert!(f.x >= 0.0 && f.y >= 0.0 && f.x + f.w <= w && f.y + f.h <= h, "{f:?} in {w}x{h}");
            assert!(f.color <= 0xffffff);
        }
        assert!(s.iter().any(|f| f.color == config::ACCENT));
    }
}

#[test]
fn logo_runs_are_inside_the_grid() {
    let g = LOGO_GRID;
    assert!(!LOGO.is_empty());
    for &(x, y, n, _) in LOGO.iter() {
        assert!(n >= 1 && (x as u32 + n as u32) <= g && (y as u32) < g);
    }
}

#[test]
fn without_a_note_nothing_is_drawn_below_the_line() {
    let line = "Se pornește Horizon...";
    let a = scene(960.0, 540.0, 0.0, &view(line, None, None));
    let l = layout(960, 540, line, None);
    let lowest = a.iter().skip(1).map(|f| f.y + f.h).fold(0.0, f32::max);
    assert!(lowest <= (l.line.1 + (GLYPH_H + 2) * l.text_scale) as f32);
}

#[test]
fn memory_text_is_romanian_decimal_comma() {
    const G: u64 = 1 << 30;
    assert_eq!(format_memory(G * 14 / 10, G * 318 / 10).unwrap(), "RAM: 1,4 GB DIN 31,8 GB");
    assert_eq!(format_memory(0, 16 * G).unwrap(), "RAM: 0,0 GB DIN 16,0 GB");
    assert_eq!(format_memory(G / 2, 8 * G).unwrap(), "RAM: 0,5 GB DIN 8,0 GB");
    assert_eq!(format_memory(1, 0), None);
    let t = format_memory(123456789, 34359738368).unwrap();
    assert!(t.chars().all(|c| glyph(c).is_some() || c == ' '), "{t}");
}

#[test]
fn memory_is_queried_about_twice_a_second() {
    let mut g = MemoryGate::new();
    assert!(g.due(0.0));
    assert!(!g.due(0.1) && !g.due(0.49));
    assert!(g.due(0.5));
    assert!(!g.due(0.9));
    assert!(g.due(1.0));
    assert!(g.due(0.0), "clock went backwards");
    let asked = (0..600).filter(|i| g.due(1.0 + *i as f32 / 60.0)).count();
    assert!((19..=21).contains(&asked), "{asked}");
}

#[test]
fn memory_line_is_drawn_below_and_only_when_given() {
    let mut v = view(LINE, Some(NOTE), Some(0.4));
    let without = scene(800.0, 450.0, 1.0, &v).len();
    v.memory = Some("RAM: 1,4 GB DIN 31,8 GB");
    assert!(scene(800.0, 450.0, 1.0, &v).len() > without);
    let l = layout_with(800, 450, LINE, Some(NOTE), v.memory);
    assert!(l.memory.1 > l.note.1 && l.memory.0 >= 0);
    assert_eq!(layout(800, 450, LINE, Some(NOTE)).memory_scale, 1);
}

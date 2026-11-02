// Progress math of the loading screen (mixins/ecran_incarcare/shared.rs, compiled as is).
#[path = "../mixins/ecran_incarcare/shared.rs"]
mod shared;
use shared::*;

fn stamp(c: &Counters, mods: &[usize]) {
    for &files in mods {
        c.source_seen(files);
    }
}

#[test]
fn the_texts_are_english_unless_romanian_is_chosen() {
    assert!(!ROMANIAN, "English is the default");
    let c = Counters::new();
    assert_eq!(c.snapshot().line_in(true), "Se pornește Horizon...");
    stamp(&c, &[10, 20, 30]);
    let s = c.snapshot();
    assert_eq!(s.line_in(false), "Preparing mods (3)...");
    assert_eq!(s.line_in(true), "Se pregătesc modurile (3)...");
    assert_eq!(s.note_in(false), Some("Do not close the game: the first start takes longer"));
    assert_eq!(s.note_in(true), Some("Nu închide jocul: prima pornire durează mai mult"));
    c.tables_known(4, 1000);
    c.table_written(750);
    assert_eq!(c.snapshot().line_in(true), "Se pregătesc modurile: tabele 1/4");
}

#[test]
fn nothing_known_is_indeterminate_with_a_plain_line() {
    let c = Counters::new();
    assert_eq!(c.fraction(), None);
    assert_eq!(c.snapshot().line(), "Starting Horizon...");
    assert_eq!(c.snapshot().note(), None);
}

#[test]
fn stamp_alone_names_the_mod_count_but_stays_indeterminate() {
    let c = Counters::new();
    stamp(&c, &[10, 20, 30]);
    let s = c.snapshot();
    assert_eq!((s.mods_total, s.files_total), (3, 60));
    assert_eq!(c.fraction(), None);
    assert_eq!(s.line(), "Preparing mods (3)...");
    assert!(s.note().is_some());
}

#[test]
fn tables_fill_the_first_half_by_bytes() {
    let c = Counters::new();
    stamp(&c, &[10]);
    c.tables_known(4, 1000);
    c.table_written(750);
    let s = c.snapshot();
    assert_eq!(s.line(), "Preparing mods: tables 1/4");
    let f = c.fraction().unwrap();
    assert!((f - 0.375).abs() < 0.002, "{f}");
    c.table_written(250);
    c.table_written(0);
    c.table_written(0);
    let f = c.fraction().unwrap();
    assert!((f - TABLES_WEIGHT).abs() < 0.002, "{f}");
}

#[test]
fn mods_fill_the_second_half_by_files() {
    let c = Counters::new();
    stamp(&c, &[10, 30, 60]);
    c.tables_known(2, 100);
    c.table_written(60);
    c.table_written(40);
    // First mod just started: half of its 10 files out of 100.
    c.reader_started(10);
    let s = c.snapshot();
    assert!(s.line().starts_with("Preparing mods: 1/3 (5"), "{}", s.line());
    let f = c.fraction().unwrap();
    assert!((f - (0.5 + 0.5 * 0.05)).abs() < 0.002, "{f}");
    // Second mod: the first is done (10) and half of 30.
    c.reader_started(30);
    let f = c.fraction().unwrap();
    assert!((f - (0.5 + 0.5 * 0.25)).abs() < 0.002, "{f}");
    assert!(c.snapshot().line().starts_with("Preparing mods: 2/3 (6"));
}

#[test]
fn the_bar_never_reaches_one() {
    let c = Counters::new();
    stamp(&c, &[5, 5]);
    c.tables_known(1, 10);
    c.table_written(10);
    c.reader_started(5);
    c.reader_started(5);
    for _ in 0..5 {
        c.reader_started(5);
    }
    assert!(c.fraction().unwrap() <= CEILING);
}

#[test]
fn the_displayed_value_never_goes_back() {
    let c = Counters::new();
    c.tables_known(2, 10);
    c.table_written(10);
    let before = c.fraction().unwrap();
    // A late change of the totals shrinks the raw value (10 of 1000): the peak holds.
    c.tables_known(2, 1000);
    assert!(c.snapshot().fraction().unwrap() < before);
    assert_eq!(c.fraction().unwrap(), before);
}

#[test]
fn reset_forgets_the_peak() {
    let c = Counters::new();
    stamp(&c, &[100]);
    c.tables_known(2, 100);
    c.table_written(90);
    assert!(c.fraction().unwrap() > 0.4);
    c.reset();
    assert_eq!(c.fraction(), None);
}

#[test]
fn more_started_than_stamped_is_clamped() {
    let c = Counters::new();
    stamp(&c, &[2]);
    c.reader_started(2);
    c.reader_started(2);
    let s = c.snapshot();
    assert!(s.line().starts_with("Preparing mods: 2/2 ("));
    assert!(c.fraction().unwrap() <= CEILING);
}

#[test]
fn a_new_stamp_after_a_build_starts_over() {
    let c = Counters::new();
    stamp(&c, &[10, 10]);
    c.reader_started(10);
    c.reader_started(10);
    stamp(&c, &[7]);
    let s = c.snapshot();
    assert_eq!((s.mods_total, s.files_total, s.mods_started), (1, 7, 0));
    assert_eq!(c.fraction(), None);
}

#[test]
fn files_total_zero_falls_back_to_mod_counts() {
    let s = Snapshot { mods_total: 4, mods_started: 3, files_total: 0, ..Default::default() };
    // tables part 1 (mods started), mods part 2/4
    let f = s.fraction().unwrap();
    assert!((f - 0.75).abs() < 1e-6, "{f}");
}

#[test]
fn snapshot_fraction_is_within_bounds_for_odd_inputs() {
    let s = Snapshot {
        tables_count: 3,
        tables_done: 9,
        tables_bytes: 5,
        tables_bytes_done: 50,
        mods_total: 1,
        files_total: 1,
        mods_started: 1,
        files_before: 99,
        files_current: 9,
    };
    let f = s.fraction().unwrap();
    assert!((0.0..=CEILING).contains(&f));
    assert!(s.line().starts_with("Preparing mods: 1/1"));
}

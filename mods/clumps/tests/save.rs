use horizon_clumps::split_value;

#[test]
fn a_value_that_fits_a_short_is_saved_as_it_is() {
    for total in [0, 1, 3, 17, 2477, 32767] {
        assert_eq!(split_value(total), (total, 1));
    }
}

#[test]
fn a_big_clump_is_saved_as_value_times_count() {
    // 32768 = 2^15: the largest divisor that fits a short is 16384.
    assert_eq!(split_value(32768), (16384, 2));
    assert_eq!(split_value(40000), (20000, 2));
    // A prime has only 1.
    assert_eq!(split_value(65537), (1, 65537));
}

#[test]
fn the_product_is_always_the_clump_and_the_value_fits_a_short() {
    let mut x = 32768u32;
    while x < 3_000_000 {
        let (value, count) = split_value(x);
        assert_eq!(value * count, x, "{x}");
        assert!(value <= i16::MAX as u32 && value >= 1, "{x}");
        // No larger divisor that fits.
        assert!(((value + 1)..=i16::MAX as u32).all(|d| x % d != 0), "{x}");
        x += 7919;
    }
}

#[test]
fn the_save_module_is_the_engine_block() {
    // `mixins/clumps/nbt.rs` is what the game gets; `tools/sync_nbt.py` writes it from `src/lib.rs`.
    let lib = include_str!("../src/lib.rs");
    let begin = lib.find("// --- the part of the rules the save needs").unwrap();
    let begin = begin + lib[begin..].find('\n').unwrap() + 1;
    let end = lib.find("// --- end of the save block ---").unwrap();
    let nbt = include_str!("../mixins/clumps/nbt.rs");
    assert_eq!(&lib[begin..end], nbt, "run tools/sync_nbt.py");
}

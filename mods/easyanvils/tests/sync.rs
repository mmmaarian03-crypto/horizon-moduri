// The library copy of the rules (mixins/easyanvils/shared.rs) must be src/lib.rs + tools/glue.rs
// (run `py -I -X utf8 tools/sync.py` after editing either).
#[test]
fn shared_rs_is_up_to_date() {
    let root = env!("CARGO_MANIFEST_DIR");
    let read = |p: &str| std::fs::read_to_string(format!("{root}/{p}")).unwrap();
    let want = read("src/lib.rs") + &read("tools/glue.rs");
    assert_eq!(read("mixins/easyanvils/shared.rs"), want);
}

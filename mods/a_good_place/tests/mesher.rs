// The set the chunk mesher reads (mixins/a_good_place/mesher.rs), tested as a plain module.
#[allow(dead_code)]
#[path = "../mixins/a_good_place/mesher.rs"]
mod hidden;

#[test]
fn hide_near_inside_blank_and_guard() {
    hidden::clear();
    assert!(!hidden::is_hidden([5, 64, 5]));
    assert!(!hidden::near([0, 64, 0], 2));
    hidden::hide([5, 64, 5]);
    assert!(hidden::is_hidden([5, 64, 5]));
    assert!(hidden::near([0, 64, 0], 2));
    // Two cells of margin: a section 3 blocks away on x does not reach it.
    assert!(!hidden::near([32, 64, 0], 2));
    assert_eq!(hidden::inside(0, 0), vec![[5, 64, 5]]);
    assert!(hidden::inside(1, 0).is_empty());
    // Padded cube of side 20 (border 2): the cell (5, 64, 5) of the section at (0, 64, 0) is index ((0+2)*20+(5+2))*20+(5+2).
    let mut blocks = vec![9u32; 20 * 20 * 20];
    hidden::blank(&mut blocks, [0, 64, 0], 2, 20);
    let at = ((0 + 2) * 20 + (5 + 2)) * 20 + (5 + 2);
    assert_eq!(blocks[at], 0);
    assert_eq!(blocks.iter().filter(|&&b| b == 0).count(), 1);
    // The guard lets the inner call through once.
    assert!(hidden::enter());
    assert!(!hidden::enter());
    hidden::leave();
    assert!(hidden::enter());
    hidden::leave();
    assert!(hidden::unhide([5, 64, 5]));
    assert!(!hidden::unhide([5, 64, 5]));
    assert!(!hidden::near([0, 64, 0], 2));
}

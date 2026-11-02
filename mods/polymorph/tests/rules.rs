use horizon_polymorph::*;

fn alt(id: &str, item: &str) -> Alt {
    Alt { id: id.into(), item: item.into(), count: 1 }
}

#[test]
fn order_puts_vanilla_first_and_drops_duplicates() {
    let o = order(
        Some(alt("a", "x")),
        vec![alt("b", "y"), alt("a", "x"), alt("c", "z")],
    );
    let ids: Vec<_> = o.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "c"]);
}

#[test]
fn order_without_a_first_match_keeps_recipe_order() {
    let o = order(None, vec![alt("b", "y"), alt("c", "z")]);
    assert_eq!(o.len(), 2);
    assert_eq!(o[0].id, "b");
}

#[test]
fn order_is_capped() {
    let many: Vec<Alt> = (0..40).map(|i| alt(&format!("r{i}"), "x")).collect();
    assert_eq!(order(None, many).len(), MAX_ALTS);
}

#[test]
fn pick_uses_the_remembered_recipe_while_it_matches() {
    let alts = vec![alt("a", "x"), alt("b", "y"), alt("c", "z")];
    assert_eq!(pick(&alts, None), 0);
    assert_eq!(pick(&alts, Some("c")), 2);
    // No longer matches: the first one (vanilla's) is crafted.
    assert_eq!(pick(&alts, Some("gone")), 0);
    assert_eq!(pick(&[], Some("a")), 0);
}

#[test]
fn nothing_is_shown_or_eaten_with_a_single_match() {
    for n in [0, 1] {
        let t = toggle(Kind::Table);
        assert_eq!(click(Kind::Table, true, n, t.0 + 1.0, t.1 + 1.0), Click::Pass);
    }
}

#[test]
fn toggle_opens_and_buttons_choose() {
    for kind in [Kind::Table, Kind::Inventory] {
        let t = toggle(kind);
        assert_eq!(click(kind, false, 3, t.0 + 2.0, t.1 + 2.0), Click::Toggle);
        // Closed: the popup area is not ours.
        let b = buttons(kind, 3);
        assert_eq!(click(kind, false, 3, b[0].0 + 2.0, b[0].1 + 2.0), Click::Pass);
        for (i, r) in b.iter().enumerate() {
            assert_eq!(click(kind, true, 3, r.0 + 12.0, r.1 + 12.0), Click::Choose(i));
        }
    }
}

#[test]
fn popup_grid_is_three_rows_and_stays_inside_the_screen() {
    for kind in [Kind::Table, Kind::Inventory] {
        for n in 1..=MAX_ALTS {
            let b = buttons(kind, n);
            assert_eq!(b.len(), n);
            for r in &b {
                assert!(r.0 >= 0.0 && r.0 + r.2 <= 176.0, "{kind:?} {n}: {r:?}");
            }
            // No two buttons overlap.
            for (i, a) in b.iter().enumerate() {
                for c in &b[i + 1..] {
                    assert!(a.0 + a.2 <= c.0 || c.0 + c.2 <= a.0 || a.1 + a.3 <= c.1 || c.1 + c.3 <= a.1);
                }
            }
        }
    }
    let b = buttons(Kind::Table, 4);
    // Fourth button starts the second column, top row.
    assert_eq!(b[3].1, b[0].1);
    assert_eq!(b[3].0, b[0].0 + BUTTON);
}

#[test]
fn the_popup_eats_clicks_between_buttons_only_inside_itself() {
    let kind = Kind::Table;
    // 4 buttons: the second column has one button, the area under it is empty but inside the popup.
    let b = buttons(kind, 4);
    assert_eq!(click(kind, true, 4, b[3].0 + 2.0, b[3].1 + BUTTON + 5.0), Click::Swallow);
    assert_eq!(click(kind, true, 4, 1.0, 1.0), Click::Pass);
}

#[test]
fn remembered_recipe_round_trips() {
    assert_eq!(load(Some(save(Some("oak_planks_alt")))), Some("oak_planks_alt".into()));
    assert_eq!(load(Some(save(None))), None);
    assert_eq!(load(None), None);
}

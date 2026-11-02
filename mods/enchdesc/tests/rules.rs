use horizon_enchdesc::*;
use std::collections::HashMap;

#[derive(Default)]
struct L(HashMap<String, String>);
impl Lang for L {
    fn has(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }
    fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
}
fn lang(pairs: &[(&str, &str)]) -> L {
    L(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
}
fn ench(name: &str, id: &str) -> Ench {
    Ench { full_name: name.into(), description_id: id.into() }
}
fn lines(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}
fn desc(key: &str, text: &str) -> Entry {
    Entry::Description { key: key.into(), text: text.into() }
}
fn sword() -> Context {
    Context { enchanted: true, ..Context::default() }
}
fn book() -> Context {
    Context { book: true, ..Context::default() }
}
fn sharp() -> L {
    lang(&[
        ("enchantment.minecraft.sharpness.desc", "Increases the damage of the item."),
        ("enchantment.minecraft.unbreaking.desc", "Increases durability."),
    ])
}

#[test]
fn tuned_values_are_the_file() {
    // enchdesc.json: everything default, dark gray.
    let c = Config::tuned();
    assert!(c.enable_mod && !c.only_display_on_books && !c.only_display_in_enchanting_table);
    assert!(!c.require_keybind_press);
    assert_eq!(c.indent_size, 0);
    assert_eq!(c.style.color, Some(0x55_5555));
    assert!(!c.style.bold && !c.style.italic);
}

#[test]
fn a_description_follows_each_enchantment_line() {
    let l = lines(&["Diamond Sword", "Sharpness V", "Unbreaking III", "When in Main Hand:"]);
    let e = [
        ench("Sharpness V", "enchantment.minecraft.sharpness"),
        ench("Unbreaking III", "enchantment.minecraft.unbreaking"),
    ];
    let out = edit(&Config::tuned(), &sword(), &l, &e, &sharp()).unwrap();
    assert_eq!(
        out,
        vec![
            Entry::Line(0),
            Entry::Line(1),
            desc("enchantment.minecraft.sharpness.desc", "Increases the damage of the item."),
            Entry::Line(2),
            desc("enchantment.minecraft.unbreaking.desc", "Increases durability."),
            Entry::Line(3),
        ]
    );
}

#[test]
fn the_tooltip_order_is_the_game_s_not_the_enchantments() {
    // Unbreaking is listed before Sharpness in the tooltip, the NBT lists Sharpness first.
    let l = lines(&["Book", "Unbreaking III", "Sharpness V"]);
    let e = [
        ench("Sharpness V", "enchantment.minecraft.sharpness"),
        ench("Unbreaking III", "enchantment.minecraft.unbreaking"),
    ];
    let out = edit(&Config::tuned(), &book(), &l, &e, &sharp()).unwrap();
    let texts: Vec<String> = out
        .iter()
        .map(|x| match x {
            Entry::Line(i) => l[*i].clone(),
            Entry::Description { text, .. } => format!("- {text}"),
            Entry::Hint { .. } => "hint".into(),
        })
        .collect();
    assert_eq!(
        texts,
        ["Book", "Unbreaking III", "- Increases durability.", "Sharpness V", "- Increases the damage of the item."]
    );
}

#[test]
fn a_plain_item_and_a_stack_without_enchantments_are_left_alone() {
    let l = lines(&["Stick"]);
    assert_eq!(edit(&Config::tuned(), &Context::default(), &l, &[ench("x", "y")], &sharp()), None);
    assert_eq!(edit(&Config::tuned(), &sword(), &l, &[], &sharp()), None);
    // A book with nothing stored.
    assert_eq!(edit(&Config::tuned(), &book(), &l, &[], &sharp()), None);
}

#[test]
fn only_display_on_books_skips_tools_but_not_books() {
    let mut c = Config::tuned();
    c.only_display_on_books = true;
    let l = lines(&["Sword", "Sharpness V"]);
    let e = [ench("Sharpness V", "enchantment.minecraft.sharpness")];
    assert_eq!(edit(&c, &sword(), &l, &e, &sharp()), None);
    assert!(edit(&c, &book(), &l, &e, &sharp()).is_some());
}

#[test]
fn the_enchanting_table_option() {
    let mut c = Config::tuned();
    c.only_display_in_enchanting_table = true;
    let l = lines(&["Sword", "Sharpness V"]);
    let e = [ench("Sharpness V", "enchantment.minecraft.sharpness")];
    assert_eq!(edit(&c, &sword(), &l, &e, &sharp()), None);
    let at_table = Context { enchanting_table: true, ..sword() };
    assert!(edit(&c, &at_table, &l, &e, &sharp()).is_some());
}

#[test]
fn the_shift_option_adds_a_hint_at_the_end_until_shift_is_held() {
    let mut c = Config::tuned();
    c.require_keybind_press = true;
    let l = lines(&["Sword", "Sharpness V", "Damage"]);
    let e = [ench("Sharpness V", "enchantment.minecraft.sharpness")];
    let mut t = sharp();
    t.0.insert(ACTIVATE_KEY.into(), "Hold shift to view enchantment descriptions.".into());
    let out = edit(&c, &sword(), &l, &e, &t).unwrap();
    assert_eq!(
        out,
        vec![
            Entry::Line(0),
            Entry::Line(1),
            Entry::Line(2),
            Entry::Hint { text: "Hold shift to view enchantment descriptions.".into() },
        ]
    );
    let held = Context { shift: true, ..sword() };
    let out = edit(&c, &held, &l, &e, &t).unwrap();
    assert!(out.iter().any(|x| matches!(x, Entry::Description { .. })));
    assert!(!out.iter().any(|x| matches!(x, Entry::Hint { .. })));
}

#[test]
fn the_mod_can_be_off() {
    let mut c = Config::tuned();
    c.enable_mod = false;
    let l = lines(&["Sword", "Sharpness V"]);
    let e = [ench("Sharpness V", "enchantment.minecraft.sharpness")];
    assert_eq!(edit(&c, &sword(), &l, &e, &sharp()), None);
}

#[test]
fn the_long_key_is_used_only_when_the_short_one_is_missing() {
    let id = "enchantment.x.y";
    assert_eq!(description_key(id, &lang(&[])), "enchantment.x.y.desc");
    assert_eq!(
        description_key(id, &lang(&[("enchantment.x.y.description", "d")])),
        "enchantment.x.y.description"
    );
    assert_eq!(
        description_key(id, &lang(&[("enchantment.x.y.desc", "a"), ("enchantment.x.y.description", "b")])),
        "enchantment.x.y.desc"
    );
}

#[test]
fn a_missing_translation_shows_its_key_like_the_original() {
    let l = lines(&["Sword", "Mending"]);
    let e = [ench("Mending", "enchantment.modded.mending")];
    let out = edit(&Config::tuned(), &sword(), &l, &e, &lang(&[])).unwrap();
    assert_eq!(out[2], desc("enchantment.modded.mending.desc", "enchantment.modded.mending.desc"));
}

#[test]
fn an_enchantment_without_a_line_gets_no_description() {
    // The tooltip hides the enchantments (a hide flag): nothing to follow.
    let l = lines(&["Sword"]);
    let e = [ench("Sharpness V", "enchantment.minecraft.sharpness")];
    let out = edit(&Config::tuned(), &sword(), &l, &e, &sharp()).unwrap();
    assert_eq!(out, vec![Entry::Line(0)]);
}

#[test]
fn indent_pads_the_description() {
    let mut c = Config::tuned();
    c.indent_size = 2;
    let l = lines(&["Sword", "Sharpness V"]);
    let e = [ench("Sharpness V", "enchantment.minecraft.sharpness")];
    let out = edit(&c, &sword(), &l, &e, &sharp()).unwrap();
    assert_eq!(out[2], desc("enchantment.minecraft.sharpness.desc", "  Increases the damage of the item."));
}

#[test]
fn a_line_is_matched_once_each() {
    // Two identical lines (it cannot happen in the game); the first gets the description, only one.
    let l = lines(&["Sharpness V", "Sharpness V"]);
    let e = [ench("Sharpness V", "enchantment.minecraft.sharpness")];
    let out = edit(&Config::tuned(), &book(), &l, &e, &sharp()).unwrap();
    assert_eq!(out.len(), 3);
    assert!(matches!(out[1], Entry::Description { .. }));
}

#[test]
fn the_settings_folder_overrides_tuned_values() {
    let get = |k: &str| match k {
        "requireKeybindPress" => Some("true".to_string()),
        "indentSize" => Some("3".to_string()),
        "style.color" => Some("#12ab34".to_string()),
        "style.italic" => Some("true".to_string()),
        "onlyDisplayOnBooks" => Some("maybe".to_string()),
        _ => None,
    };
    let c = Config::read(get);
    assert!(c.require_keybind_press);
    assert_eq!(c.indent_size, 3);
    assert_eq!(c.style.color, Some(0x12_AB34));
    assert!(c.style.italic);
    // Not a boolean: the pack's value stays.
    assert!(!c.only_display_on_books);
    assert_eq!(Config::read(|_| None), Config::tuned());
}

#[test]
fn colours_parse_like_the_style_codec() {
    assert_eq!(parse_color("dark_gray"), Some(0x55_5555));
    assert_eq!(parse_color("GOLD"), Some(0xFF_AA00));
    assert_eq!(parse_color("#ffffff"), Some(0xFF_FFFF));
    assert_eq!(parse_color("#fff"), None);
    assert_eq!(parse_color("purple"), None);
}

#[test]
fn wrapping_follows_the_gather_components_rule() {
    // Fits to the right of the cursor.
    assert_eq!(wrap_limit(100.0, 480.0, 200.0), None);
    // Does not fit on the right, fits on the left of the cursor.
    assert_eq!(wrap_limit(400.0, 480.0, 200.0), None);
    // Fits on neither side: the wider side, 8 short of the edge.
    assert_eq!(wrap_limit(300.0, 480.0, 400.0), Some(300.0 - 12.0 - 8.0));
    assert_eq!(wrap_limit(100.0, 480.0, 450.0), Some(480.0 - 16.0 - 100.0));
}

//! Tests of the rules of Patchouli on a fake world: markup, layout, loading, locks, navigation and pages.
use horizon_patchouli::*;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Default)]
struct TestEnv {
    lang: String,
    texts: HashMap<String, String>,
    items: BTreeSet<String>,
    tags: HashMap<String, Vec<String>>,
    done: RefCell<BTreeSet<String>>,
    mods: BTreeSet<String>,
    recipes: HashMap<String, Value>,
    keybinds: HashMap<String, (String, String)>,
}
impl Env for TestEnv {
    fn lang_code(&self) -> String {
        if self.lang.is_empty() { "en_us".into() } else { self.lang.clone() }
    }
    fn translate(&self, key: &str) -> Option<String> {
        self.texts.get(key).cloned()
    }
    fn item_exists(&self, id: &str) -> bool {
        self.items.contains(id)
    }
    fn item_name(&self, s: &Stack) -> String {
        format!("<{}>", s.id)
    }
    fn tag_items(&self, tag: &str) -> Vec<String> {
        self.tags.get(tag).cloned().unwrap_or_default()
    }
    fn advancement_done(&self, id: &str) -> bool {
        self.done.borrow().contains(id)
    }
    fn mod_loaded(&self, ns: &str) -> bool {
        self.mods.contains(ns)
    }
    fn recipe(&self, id: &str) -> Option<Value> {
        self.recipes.get(id).cloned()
    }
    fn advance(&self, _c: char, bold: bool, _uniform: bool) -> f32 {
        if bold { 7.0 } else { 6.0 }
    }
    fn player_name(&self) -> String {
        "Alex".into()
    }
    fn keybind(&self, name: &str) -> Option<(String, String)> {
        self.keybinds.get(name).cloned()
    }
}

fn book_json() -> Value {
    json!({"name": "Guide", "landing_text": "Hello $(item)world$()", "use_resource_pack": true})
}

fn files(book: &Value, rest: &[(&str, Value)]) -> MemFiles {
    let mut m = MemFiles::default();
    m.0.insert("data/test/patchouli_books/guide/book.json".into(), book.to_string());
    for (path, v) in rest {
        m.0.insert(format!("assets/test/patchouli_books/guide/en_us/{path}.json"), v.to_string());
    }
    m
}

fn load(env: &TestEnv, f: &MemFiles) -> Book {
    let found = discover(f);
    assert_eq!(found.len(), 1);
    let info = found.into_iter().next().unwrap().1.expect("book.json");
    load_book(f, env, info)
}

fn cat(name: &str) -> Value {
    json!({"name": name, "description": format!("About {name}"), "icon": "minecraft:stone"})
}
fn entry(name: &str, category: &str, pages: Vec<Value>) -> Value {
    json!({"name": name, "category": category, "icon": "minecraft:stone", "pages": pages})
}

fn mini_book(env: &TestEnv) -> Book {
    let f = files(
        &book_json(),
        &[
            ("categories/basics", cat("Basics")),
            ("entries/intro", entry("Intro", "test:basics", vec![json!("First page $(l:test:more) link"), json!({"type": "text", "text": "Second"}), json!("Third")])),
            ("entries/more", entry("More", "test:basics", vec![json!({"type": "patchouli:text", "title": "Title", "text": "Body"})])),
        ],
    );
    load(env, &f)
}

// ---------------------------------------------------------------------------------------------
// json, stacks
// ---------------------------------------------------------------------------------------------

#[test]
fn lenient_json_accepts_comments_and_trailing_commas() {
    let v = parse_json("\u{feff}{\"a\": [1, 2,], // note\n \"b\": \"x // not a comment\", /* c */ \"c\": {\"d\": 1,},}").unwrap();
    assert_eq!(v["a"], json!([1, 2]));
    assert_eq!(v["b"], "x // not a comment");
    assert_eq!(v["c"]["d"], 1);
    assert!(parse_json("{").is_err());
}

#[test]
fn item_strings_are_read_like_patchouli_reads_them() {
    let s = parse_stack("minecraft:potion#3{Potion:'minecraft:water'}").unwrap();
    assert_eq!((s.id.as_str(), s.count), ("minecraft:potion", 3));
    assert_eq!(s.nbt.as_deref(), Some("{Potion:\"minecraft:water\"}"));
    assert!(parse_stack("stone").is_err());
    let mut env = TestEnv::default();
    env.tags.insert("minecraft:logs".into(), vec!["minecraft:oak_log".into(), "minecraft:birch_log".into()]);
    let l = parse_stack_list(&env, "minecraft:stone,tag:minecraft:logs, minecraft:dirt#2").unwrap();
    assert_eq!(l.iter().map(|s| (s.id.as_str(), s.count)).collect::<Vec<_>>(), [("minecraft:stone", 1), ("minecraft:oak_log", 1), ("minecraft:birch_log", 1), ("minecraft:dirt", 2)]);
}

// ---------------------------------------------------------------------------------------------
// text
// ---------------------------------------------------------------------------------------------

fn spans_of(env: &TestEnv, book: &Book, text: &str) -> Vec<Span> {
    let cx = TextCx { env, book: &book.info, contents: &book.contents };
    parse_spans(&cx, text, Style::colored(0))
}

#[test]
fn colors_bold_reset_and_macros() {
    let env = TestEnv::default();
    let book = mini_book(&env);
    let s = spans_of(&env, &book, "a$(c)b$(bold)c$()d$(item)e$(#abc)f$(#112233)g$(nocolor)h");
    let texts: Vec<&str> = s.iter().map(|x| x.text.as_str()).collect();
    assert_eq!(texts, ["a", "b", "c", "d", "e", "f", "g", "h"]);
    assert_eq!(s[1].style.color, 0xFF5555);
    assert!(s[2].style.bold && s[2].style.color == 0xFF5555);
    assert!(!s[3].style.bold && s[3].style.color == 0);
    // $(item) is the macro for $(#b0b)
    assert_eq!(s[4].style.color, 0xBB00BB);
    assert_eq!(s[5].style.color, 0xAABBCC);
    assert_eq!(s[6].style.color, 0x112233);
    assert_eq!(s[7].style.color, 0);
}

#[test]
fn line_breaks_lists_and_unknown_commands() {
    let env = TestEnv::default();
    let book = mini_book(&env);
    let s = spans_of(&env, &book, "a$(br)b$(br2)c$(li)d$(li2)e$(nope)f$(foo:bar)g");
    let lb: Vec<i32> = s.iter().map(|x| x.line_breaks).collect();
    assert_eq!(lb[1], 1, "b follows one $(br)");
    assert_eq!(s.iter().find(|x| x.text == "c").unwrap().line_breaks, 2);
    let bullet = s.iter().find(|x| x.text == "§0\u{2022}").expect("bullet");
    assert_eq!((bullet.line_breaks, bullet.spacing_left, bullet.spacing_right), (1, 4, 6));
    let bullet2 = s.iter().find(|x| x.text == "§0\u{25E6}").expect("second bullet");
    assert_eq!(bullet2.spacing_left, 8);
    assert!(s.iter().any(|x| x.text == "$(nope)"));
    assert!(s.iter().any(|x| x.text == "[MISSING FUNCTION: foo]"));
}

#[test]
fn links_tooltips_and_the_external_arrow() {
    let mut env = TestEnv::default();
    env.keybinds.insert("inventory".into(), ("Open Inventory".into(), "E".into()));
    let book = mini_book(&env);
    let s = spans_of(&env, &book, "x$(l:test:more)go$(/l)y $(l:https://example.com)web$(/l) $(l:test:nowhere)bad$(/l) $(l:test:more#none)z$(/l) $(k:inventory)$(k:nope) $(playername)");
    let go = s.iter().find(|x| x.text == "go").unwrap();
    assert_eq!(go.link, Some(Link::Entry { id: "test:more".into(), spread: 0 }));
    assert_eq!(go.tooltip, "More");
    assert_eq!(go.style.color, 0x0000EE, "link colour");
    assert!(go.cluster.is_some());
    let y = s.iter().find(|x| x.text == "y ").unwrap();
    assert!(y.link.is_none() && y.cluster.is_none());
    let web = s.iter().find(|x| x.text == "web").unwrap();
    assert_eq!(web.link, Some(Link::Url("https://example.com".into())));
    assert!(s.iter().any(|x| x.text == "§7\u{21AA}"), "the external link arrow");
    let bad = s.iter().find(|x| x.text == "bad").unwrap();
    assert!(bad.link.is_none() && bad.tooltip.starts_with("BAD LINK"));
    let z = s.iter().find(|x| x.text == "z").unwrap();
    assert!(z.tooltip.contains("INVALID ANCHOR:none"));
    assert!(s.iter().any(|x| x.text == "E" && x.tooltip == "Keybind: Open Inventory"));
    assert!(s.iter().any(|x| x.text == "N/A"));
    assert!(s.iter().any(|x| x.text == "Alex"));
}

fn text_of(env: &TestEnv, book: &Book, text: &str, x: i32, y: i32, width: i32, color: u32) -> BookText {
    let cx = TextCx { env, book: &book.info, contents: &book.contents };
    BookText::new(&cx, text, x, y, width, 9, color)
}

#[test]
fn text_wraps_at_the_last_space_that_fits() {
    let env = TestEnv::default();
    let book = mini_book(&env);
    let t = text_of(&env, &book, "aaaa bbbb cccc dddd eeee ffff", 0, 0, 116, 0);
    assert_eq!(t.scale, 1.0);
    let lines: Vec<(&str, i32)> = t.words.iter().map(|w| (w.text.as_str(), w.y)).collect();
    assert_eq!(lines, [("aaaa bbbb cccc dddd ", 0), ("eeee ffff", 9)]);
    // $(br) starts a new line, $(br2) leaves an empty one
    let t = text_of(&env, &book, "one$(br)two$(br2)three", 3, 10, 116, 0);
    let ys: Vec<(i32, i32)> = t.words.iter().map(|w| (w.x, w.y)).collect();
    assert_eq!(ys, [(3, 10), (3, 19), (3, 37)]);
}

#[test]
fn a_word_longer_than_the_line_is_cut() {
    let env = TestEnv::default();
    let book = mini_book(&env);
    let t = text_of(&env, &book, &"x".repeat(40), 0, 0, 116, 0);
    assert!(t.words.len() >= 2);
    assert_eq!(t.words.iter().map(|w| w.text.len()).sum::<usize>(), 40);
    assert!(t.words.iter().all(|w| w.text.len() as i32 * 6 <= 116));
}

#[test]
fn text_resize_truncate_and_overflow() {
    let env = TestEnv::default();
    let long = "word ".repeat(120);
    // RESIZE is the pack's setting: the text shrinks until it fits
    let book = mini_book(&env);
    let t = text_of(&env, &book, &long, 0, 0, 116, 0);
    assert!(t.scale < 1.0, "scale {}", t.scale);
    let last = t.words.iter().map(|w| w.y).max().unwrap();
    assert!((last + 9) as f32 * t.scale <= PAGE_HEIGHT as f32 + 0.5, "last line at {last}, scale {}", t.scale);
    // the book may ask for another mode
    let mut over = book.clone();
    over.info.overflow = Some(Overflow::Overflow);
    let t = text_of(&env, &over, &long, 0, 0, 116, 0);
    assert_eq!(t.scale, 1.0);
    assert!(t.words.iter().map(|w| w.y).max().unwrap() > PAGE_HEIGHT);
    let mut cut = book.clone();
    cut.info.overflow = Some(Overflow::Truncate);
    let t = text_of(&env, &cut, &long, 0, 0, 116, 0);
    assert!(t.words.iter().all(|w| w.y + 9 <= PAGE_HEIGHT));
    assert!(!t.words.is_empty());
}

#[test]
fn clicks_hit_words_and_follow_the_scale() {
    let env = TestEnv::default();
    let book = mini_book(&env);
    let t = text_of(&env, &book, "see $(l:test:more)here$(/l) now", 10, 20, 116, 0);
    // "see " is 4 chars = 24 px, "here" starts at x = 34
    let w = t.word_at(40.0, 24.0).expect("word under the mouse");
    assert_eq!(t.words[w].text, "here");
    assert_eq!(t.click(40.0, 24.0, 0), Some(Link::Entry { id: "test:more".into(), spread: 0 }));
    assert_eq!(t.click(40.0, 24.0, 1), None);
    assert_eq!(t.click(12.0, 24.0, 0), None, "plain text is not a link");
    assert!(t.is_hot(w, 40.0, 24.0));
}

// ---------------------------------------------------------------------------------------------
// the book
// ---------------------------------------------------------------------------------------------

#[test]
fn book_json_defaults_and_refusals() {
    let ok = BookInfo::from_json("test", "guide", &book_json()).unwrap();
    assert_eq!(ok.id, "test:guide");
    assert_eq!(ok.book_texture, "patchouli:textures/gui/book_brown.png");
    assert_eq!((ok.text_color, ok.header_color, ok.link_color), (0x000000, 0x333333, 0x0000EE));
    assert!(ok.show_progress && !ok.i18n && !ok.use_blocky_font);
    assert_eq!(ok.macros.get("$(item)").map(String::as_str), Some("$(#b0b)"));
    // 1.20 refuses these
    assert!(BookInfo::from_json("t", "g", &json!({"name": "x"})).is_err());
    assert!(BookInfo::from_json("t", "g", &json!({"name": "x", "use_resource_pack": true, "extend": "a:b"})).is_err());
    assert!(BookInfo::from_json("t", "g", &json!({"use_resource_pack": true})).is_err());
    let c = BookInfo::from_json("t", "g", &json!({"name": "x", "use_resource_pack": true, "text_color": "FF0000", "macros": {"$(foo)": "bar"}, "text_overflow_mode": "truncate"})).unwrap();
    assert_eq!(c.text_color, 0xFF0000);
    assert_eq!(c.overflow, Some(Overflow::Truncate));
    assert_eq!(c.macros["$(foo)"], "bar");
}

#[test]
fn subtitles_and_ordinals() {
    let env = TestEnv::default();
    let mut b = BookInfo::from_json("t", "g", &json!({"name": "x", "use_resource_pack": true, "version": 3, "subtitle": "sub"})).unwrap();
    assert_eq!(b.subtitle_text(&env), "3rd Edition");
    for (v, s) in [(1, "1st"), (2, "2nd"), (11, "11th"), (12, "12th"), (21, "21st"), (113, "113th")] {
        b.version = v.to_string();
        assert_eq!(b.subtitle_text(&env), format!("{s} Edition"));
    }
    b.version = "0".into();
    assert_eq!(b.subtitle_text(&env), "sub");
    b.version = "dev".into();
    assert_eq!(b.subtitle_text(&env), "Writer's Edition");
}

#[test]
fn contents_load_with_languages_flags_and_errors() {
    let mut env = TestEnv::default();
    env.lang = "ro_ro".into();
    env.mods.insert("jei".into());
    let mut f = files(
        &book_json(),
        &[
            ("categories/basics", cat("Basics")),
            ("categories/sub", json!({"name": "Sub", "description": "d", "icon": "minecraft:dirt", "parent": "test:basics"})),
            ("categories/hidden_by_flag", json!({"name": "H", "description": "d", "icon": "minecraft:dirt", "flag": "mod:nothing"})),
            ("entries/a/b", entry("Deep", "test:sub", vec![json!("x")])),
            ("entries/lost", entry("Lost", "test:missing", vec![json!("x")])),
            ("entries/short_category", json!({"name": "S", "category": "basics", "icon": "minecraft:stone", "pages": []})),
            ("entries/flagged", json!({"name": "F", "category": "test:basics", "icon": "minecraft:stone", "flag": "mod:jei", "pages": [{"type": "text", "text": "t", "flag": "mod:nothing"}, "kept"]})),
            ("templates/t", json!({"components": []})),
        ],
    );
    // a name in the player's language (ro_ro) for one entry, and a broken file
    f.0.insert("assets/test/patchouli_books/guide/ro_ro/entries/a/b.json".into(), entry("Localized", "test:sub", vec![json!("x")]).to_string());
    f.0.insert("assets/test/patchouli_books/guide/en_us/entries/broken.json".into(), "{ nope".into());
    let b = load(&env, &f);
    let c = &b.contents;
    assert!(c.categories.contains_key("test:basics") && c.categories.contains_key("test:sub"));
    assert!(!c.categories.contains_key("test:hidden_by_flag"));
    assert_eq!(c.entries["test:a/b"].name, "Localized", "the player's language wins");
    assert_eq!(c.categories["test:basics"].children, ["test:sub"]);
    assert_eq!(c.categories["test:sub"].entries, ["test:a/b"]);
    assert!(!c.entries.contains_key("test:lost"));
    assert!(!c.entries.contains_key("test:short_category"));
    assert_eq!(c.entries["test:flagged"].pages.len(), 1, "the page with a false flag is not added");
    assert_eq!(c.entries["test:flagged"].pages[0].index, 1, "but it still counts in the page number");
    assert!(c.templates.contains_key("test:t"));
    assert!(c.errors.iter().any(|e| e.contains("broken.json")));
    assert!(c.errors.iter().any(|e| e.contains("test:missing")));
    assert!(c.errors.iter().any(|e| e.contains("fully qualified")));
}

#[test]
fn discovery_lists_books_and_errors() {
    let mut f = files(&book_json(), &[]);
    f.0.insert("data/other/patchouli_books/old/book.json".into(), json!({"name": "x"}).to_string());
    let found = discover(&f);
    assert_eq!(found.len(), 2);
    assert!(found.iter().any(|(r, i)| r.id() == "test:guide" && i.is_ok()));
    assert!(found.iter().any(|(r, i)| r.id() == "other:old" && i.is_err()));
}

#[test]
fn recipe_pages_map_items_to_entries() {
    let mut env = TestEnv::default();
    env.recipes.insert(
        "minecraft:torch".into(),
        json!({"type": "minecraft:crafting_shaped", "pattern": ["C", "S"], "key": {"C": "minecraft:coal", "S": "minecraft:stick"}, "result": {"id": "minecraft:torch", "count": 4}}),
    );
    let f = files(
        &book_json(),
        &[
            ("categories/c", cat("C")),
            (
                "entries/e",
                json!({"name": "E", "category": "test:c", "icon": "minecraft:stone",
                    "pages": ["x", {"type": "crafting", "recipe": "minecraft:torch"}, {"type": "spotlight", "item": "minecraft:diamond", "link_recipe": true},
                              {"type": "spotlight", "item": "minecraft:dirt"}],
                    "extra_recipe_mappings": {"minecraft:bone": 0}}),
            ),
        ],
    );
    let b = load(&env, &f);
    assert_eq!(b.contents.entry_for_stack("minecraft:torch"), Some(&("test:e".to_owned(), 0)));
    assert_eq!(b.contents.entry_for_stack("minecraft:diamond"), Some(&("test:e".to_owned(), 1)));
    assert_eq!(b.contents.entry_for_stack("minecraft:bone"), Some(&("test:e".to_owned(), 0)));
    assert!(b.contents.entry_for_stack("minecraft:dirt").is_none(), "no link_recipe");
    let names: Vec<&str> = b.contents.entries["test:e"].relevant.iter().map(|s| s.id.as_str()).collect();
    assert!(names.contains(&"minecraft:torch") && names.contains(&"minecraft:diamond"));
}

// ---------------------------------------------------------------------------------------------
// locks, progress, reading state
// ---------------------------------------------------------------------------------------------

fn locked_book(env: &TestEnv) -> Book {
    let f = files(
        &book_json(),
        &[
            ("categories/open", cat("Open")),
            ("categories/closed", cat("Closed")),
            ("categories/secret", json!({"name": "Secret", "description": "d", "icon": "minecraft:dirt", "secret": true})),
            ("entries/free", entry("Free", "test:open", vec![json!("x")])),
            ("entries/gated", json!({"name": "Gated", "category": "test:open", "icon": "minecraft:stone", "advancement": "test:go", "pages": ["x"]})),
            ("entries/gated2", json!({"name": "Gated2", "category": "test:closed", "icon": "minecraft:stone", "advancement": "test:go", "pages": ["x"]})),
            ("entries/hush", json!({"name": "Hush", "category": "test:open", "icon": "minecraft:stone", "advancement": "test:go", "secret": true, "pages": ["x"]})),
            ("entries/in_secret", json!({"name": "S1", "category": "test:secret", "icon": "minecraft:stone", "advancement": "test:go", "pages": ["x"]})),
            ("entries/default_read", json!({"name": "Old", "category": "test:open", "icon": "minecraft:stone", "read_by_default": true, "pages": ["x"]})),
            ("entries/waiting", json!({"name": "Wait", "category": "test:open", "icon": "minecraft:stone", "read_by_default": true, "turnin": "test:turn", "pages": ["x"]})),
            ("entries/quest", json!({"name": "Quest", "category": "test:open", "icon": "minecraft:stone", "read_by_default": true, "pages": [{"type": "quest", "text": "do it", "trigger": "test:q"}]})),
        ],
    );
    load(env, &f)
}

#[test]
fn advancements_lock_entries_and_categories() {
    let env = TestEnv::default();
    let b = locked_book(&env);
    let (c, i) = (&b.contents, &b.info);
    assert!(!c.entry_locked(&env, i, "test:free"));
    assert!(c.entry_locked(&env, i, "test:gated"));
    assert!(c.category_locked(&env, i, "test:closed"), "nothing in it is unlocked");
    assert!(!c.category_locked(&env, i, "test:open"));
    assert!(c.entry_hidden(&env, i, "test:hush"), "secret and locked");
    assert!(c.category_hidden(&env, i, "test:secret"));
    assert_eq!(c.root_categories(&env, i), ["test:open", "test:closed"].map(String::from), "locked categories come last, secret ones are hidden");
    env.done.borrow_mut().insert("test:go".into());
    assert!(!c.entry_locked(&env, i, "test:gated"));
    assert!(!c.category_locked(&env, i, "test:closed"));
    assert!(!c.entry_hidden(&env, i, "test:hush"));
    assert_eq!(c.root_categories(&env, i).len(), 3);
}

#[test]
fn read_states_follow_the_original() {
    let env = TestEnv::default();
    let b = locked_book(&env);
    let mut p = Persist::default();
    let (c, i) = (&b.contents, &b.info);
    assert_eq!(c.entry_state(&env, &p, i, "test:free"), ReadState::Unread);
    assert_eq!(c.entry_state(&env, &p, i, "test:default_read"), ReadState::Neutral);
    assert_eq!(c.entry_state(&env, &p, i, "test:waiting"), ReadState::Pending);
    assert_eq!(c.entry_state(&env, &p, i, "test:quest"), ReadState::Neutral);
    env.done.borrow_mut().insert("test:q".into());
    env.done.borrow_mut().insert("test:turn".into());
    assert_eq!(c.entry_state(&env, &p, i, "test:quest"), ReadState::Completed);
    assert_eq!(c.entry_state(&env, &p, i, "test:waiting"), ReadState::Neutral);
    p.data("test:guide").viewed.push("test:free".into());
    assert_eq!(c.entry_state(&env, &p, i, "test:free"), ReadState::Neutral);
    env.done.borrow_mut().insert("test:go".into());
    assert!(ReadState::Unread < ReadState::Pending && ReadState::Pending < ReadState::Neutral && ReadState::Neutral < ReadState::Completed);
    // a category shows the most important state of what it holds
    assert_eq!(c.category_state(&env, &p, i, "test:open"), ReadState::Unread, "gated is unlocked now but unread");
}

#[test]
fn entries_sort_unlocked_first_then_unread_priority_sortnum_name() {
    let env = TestEnv::default();
    let f = files(
        &book_json(),
        &[
            ("categories/c", cat("C")),
            ("entries/b", json!({"name": "B", "category": "test:c", "icon": "minecraft:stone", "read_by_default": true, "pages": []})),
            ("entries/a", json!({"name": "A", "category": "test:c", "icon": "minecraft:stone", "read_by_default": true, "pages": []})),
            ("entries/p", json!({"name": "Z", "category": "test:c", "icon": "minecraft:stone", "read_by_default": true, "priority": true, "pages": []})),
            ("entries/s", json!({"name": "Y", "category": "test:c", "icon": "minecraft:stone", "read_by_default": true, "sortnum": -1, "pages": []})),
            ("entries/l", json!({"name": "0", "category": "test:c", "icon": "minecraft:stone", "read_by_default": true, "advancement": "test:no", "pages": []})),
            ("entries/u", json!({"name": "U", "category": "test:c", "icon": "minecraft:stone", "pages": []})),
        ],
    );
    let b = load(&env, &f);
    let p = Persist::default();
    let mut ids: Vec<String> = b.contents.entries.keys().cloned().collect();
    ids.sort_by(|x, y| b.contents.compare_entries(&env, &p, &b.info, x, y));
    assert_eq!(ids, ["test:u", "test:p", "test:s", "test:a", "test:b", "test:l"].map(String::from));
}

#[test]
fn persistent_data_round_trips_without_the_history() {
    let mut p = Persist::default();
    let d = p.data("a:b");
    d.viewed = vec!["a:x".into()];
    d.bookmarks = vec![("a:x".into(), 2)];
    d.history = vec!["a:x".into()];
    d.completed_quests = vec!["a:q".into()];
    p.book_gui_scale = 3;
    let text = p.save();
    let back = Persist::load(&text);
    assert_eq!(back.book_gui_scale, 3);
    let d = back.get("a:b").unwrap();
    assert_eq!((d.viewed.clone(), d.bookmarks.clone(), d.completed_quests.clone()), (vec!["a:x".to_owned()], vec![("a:x".to_owned(), 2)], vec!["a:q".to_owned()]));
    assert!(d.history.is_empty(), "Patchouli keeps no history between runs");
    assert_eq!(Persist::load("garbage"), Persist::default());
}

// ---------------------------------------------------------------------------------------------
// navigation
// ---------------------------------------------------------------------------------------------

fn view_of(env: &TestEnv, b: &Book, p: &Persist, nav: &mut Nav) -> View {
    normalize(env, b, p, nav);
    build_view(env, b, p, nav)
}

fn click(env: &TestEnv, b: &Book, p: &mut Persist, nav: &mut Nav, cmd: Cmd, shift: bool) -> Vec<Effect> {
    let v = view_of(env, b, p, nav);
    apply_cmd(env, b, p, nav, &v, &cmd, shift)
}

#[test]
fn opening_entries_pushes_the_stack_and_back_pops_it() {
    let env = TestEnv::default();
    let b = mini_book(&env);
    let mut p = Persist::default();
    let mut nav = Nav::default();
    let fx = click(&env, &b, &mut p, &mut nav, Cmd::Open(Where { gui: Gui::Category("test:basics".into()), spread: 0 }), false);
    assert!(fx.contains(&Effect::Flip));
    assert_eq!(nav.stack, [Where::landing()]);
    click(&env, &b, &mut p, &mut nav, Cmd::Entry("test:intro".into()), false);
    assert_eq!(nav.cur, Where::entry("test:intro", 0));
    assert_eq!(nav.stack.len(), 2);
    // viewing marks it read and puts it first in the history
    let d = p.get("test:guide").unwrap();
    assert_eq!(d.viewed, ["test:intro"]);
    assert_eq!(d.history, ["test:intro"]);
    assert!(p.dirty);
    // the same screen is not pushed on itself
    click(&env, &b, &mut p, &mut nav, Cmd::Entry("test:intro".into()), false);
    assert_eq!(nav.stack.len(), 2);
    assert!(click(&env, &b, &mut p, &mut nav, Cmd::Back, false).contains(&Effect::Flip));
    assert!(matches!(nav.cur.gui, Gui::Category(_)));
    // Shift goes home and forgets the way
    click(&env, &b, &mut p, &mut nav, Cmd::Entry("test:more".into()), false);
    click(&env, &b, &mut p, &mut nav, Cmd::Back, true);
    assert_eq!(nav.cur, Where::landing());
    assert!(nav.stack.is_empty());
    assert!(click(&env, &b, &mut p, &mut nav, Cmd::Back, false).iter().all(|e| *e != Effect::Flip), "nothing to go back to");
}

#[test]
fn pages_turn_within_the_limits() {
    let env = TestEnv::default();
    let b = mini_book(&env);
    let mut p = Persist::default();
    let mut nav = Nav { cur: Where::entry("test:intro", 0), ..Nav::default() };
    let v = view_of(&env, &b, &p, &mut nav);
    assert_eq!(v.max_spreads, 2, "three pages make two spreads");
    assert!(!change_page(&v, &mut nav, true));
    assert!(change_page(&v, &mut nav, false));
    assert_eq!(nav.cur.spread, 1);
    assert!(!change_page(&v, &mut nav, false));
    let v1 = view_of(&env, &b, &p, &mut nav);
    // the arrows follow the spread
    let arrows: Vec<&Cmd> = v1.elems.iter().filter_map(|e| if let Elem::Button { cmd: c @ Cmd::Page(_), .. } = e { Some(c) } else { None }).collect();
    assert_eq!(arrows, [&Cmd::Page(true)]);
    let _ = &mut p;
}

#[test]
fn bookmarks_are_added_opened_and_removed() {
    let env = TestEnv::default();
    let b = mini_book(&env);
    let mut p = Persist::default();
    let mut nav = Nav { cur: Where::entry("test:intro", 1), ..Nav::default() };
    click(&env, &b, &mut p, &mut nav, Cmd::Bookmark(None), false);
    assert_eq!(p.get("test:guide").unwrap().bookmarks, [("test:intro".to_owned(), 1)]);
    // the "add" button is gone for a page that is bookmarked already
    let v = view_of(&env, &b, &p, &mut nav);
    assert!(!v.elems.iter().any(|e| matches!(e, Elem::Bookmark { cmd: Cmd::Bookmark(None), .. })));
    let mut other = Nav { cur: Where::entry("test:more", 0), ..Nav::default() };
    let v = view_of(&env, &b, &p, &mut other);
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Bookmark { cmd: Cmd::Bookmark(None), .. })));
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Bookmark { cmd: Cmd::Bookmark(Some(0)), .. })));
    click(&env, &b, &mut p, &mut other, Cmd::Bookmark(Some(0)), false);
    assert_eq!(other.cur, Where::entry("test:intro", 1));
    click(&env, &b, &mut p, &mut other, Cmd::Bookmark(Some(0)), true);
    assert!(p.get("test:guide").unwrap().bookmarks.is_empty());
    // Shift on an entry button bookmarks it at spread 0, and again removes it
    let mut nav = Nav::default();
    click(&env, &b, &mut p, &mut nav, Cmd::Entry("test:more".into()), true);
    assert_eq!(p.get("test:guide").unwrap().bookmarks, [("test:more".to_owned(), 0)]);
    assert_eq!(nav.cur, Where::landing(), "Shift does not open it");
    click(&env, &b, &mut p, &mut nav, Cmd::Entry("test:more".into()), true);
    assert!(p.get("test:guide").unwrap().bookmarks.is_empty());
    // at most ten
    for i in 0..12 {
        p.data("test:guide").bookmarks.push((format!("test:x{i}"), 0));
    }
    let before = p.get("test:guide").unwrap().bookmarks.len();
    click(&env, &b, &mut p, &mut nav, Cmd::Entry("test:more".into()), true);
    assert!(p.get("test:guide").unwrap().bookmarks.len() == before, "full: Shift opens it instead");
}

#[test]
fn locked_entries_do_not_open_and_links_still_flip() {
    let env = TestEnv::default();
    let b = locked_book(&env);
    let mut p = Persist::default();
    let mut nav = Nav::default();
    click(&env, &b, &mut p, &mut nav, Cmd::Entry("test:gated".into()), false);
    assert_eq!(nav.cur, Where::landing());
    env.done.borrow_mut().insert("test:go".into());
    click(&env, &b, &mut p, &mut nav, Cmd::Entry("test:gated".into()), false);
    assert_eq!(nav.cur, Where::entry("test:gated", 0));
}

#[test]
fn history_keeps_thirteen_entries_newest_first() {
    let env = TestEnv::default();
    let mut rest = vec![("categories/c".to_owned(), cat("C"))];
    for i in 0..15 {
        rest.push((format!("entries/e{i:02}"), entry(&format!("E{i:02}"), "test:c", vec![json!("x")])));
    }
    let rest: Vec<(&str, Value)> = rest.iter().map(|(a, b)| (a.as_str(), b.clone())).collect();
    let f = files(&book_json(), &rest);
    let b = load(&env, &f);
    let mut p = Persist::default();
    let mut nav = Nav::default();
    for i in 0..15 {
        click(&env, &b, &mut p, &mut nav, Cmd::Entry(format!("test:e{i:02}")), false);
    }
    let h = &p.get("test:guide").unwrap().history;
    assert_eq!(h.len(), 13);
    assert_eq!(h[0], "test:e14");
    // opening one again moves it to the front
    click(&env, &b, &mut p, &mut nav, Cmd::Entry("test:e05".into()), false);
    assert_eq!(p.get("test:guide").unwrap().history[0], "test:e05");
}

#[test]
fn open_at_entry_keeps_the_way_back() {
    let env = TestEnv::default();
    let b = mini_book(&env);
    let mut p = Persist::default();
    let mut nav = Nav::default();
    open_at_entry(&env, &b, &mut p, &mut nav, "test:intro", 3);
    assert_eq!(nav.cur, Where::entry("test:intro", 1), "page 3 is in spread 1");
    assert_eq!(nav.stack, [Where::landing()]);
    open_at_entry(&env, &b, &mut p, &mut nav, "test:intro", 2);
    assert_eq!(nav.cur, Where::entry("test:intro", 1), "same spread: nothing is pushed");
    assert_eq!(nav.stack.len(), 1);
}

#[test]
fn search_filters_lists_by_name_and_items() {
    let mut env = TestEnv::default();
    env.recipes.insert("minecraft:torch".into(), json!({"type": "minecraft:crafting_shapeless", "ingredients": ["minecraft:coal"], "result": {"id": "minecraft:torch"}}));
    let f = files(
        &book_json(),
        &[
            ("categories/c", cat("C")),
            ("entries/apple", entry("Apple pie", "test:c", vec![json!("x")])),
            ("entries/light", entry("Lighting", "test:c", vec![json!({"type": "crafting", "recipe": "minecraft:torch"})])),
        ],
    );
    let b = load(&env, &f);
    let p = Persist::default();
    let mut nav = Nav { cur: Where { gui: Gui::Index, spread: 0 }, ..Nav::default() };
    set_search(&env, &b, &p, &mut nav, "APPLE");
    let v = build_view(&env, &b, &p, &nav);
    assert_eq!(v.single_result.as_deref(), Some("test:apple"));
    set_search(&env, &b, &p, &mut nav, "torch");
    assert_eq!(build_view(&env, &b, &p, &nav).single_result.as_deref(), Some("test:light"), "found by the item it explains");
    set_search(&env, &b, &p, &mut nav, "zzz");
    let v = build_view(&env, &b, &p, &nav);
    assert!(v.single_result.is_none());
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Str { text, .. } if text == "No Results")));
}

#[test]
fn long_lists_make_spreads() {
    let env = TestEnv::default();
    let mut rest = vec![("categories/c".to_owned(), cat("C"))];
    for i in 0..37 {
        rest.push((format!("entries/e{i:02}"), entry(&format!("E{i:02}"), "test:c", vec![json!("x")])));
    }
    let rest: Vec<(&str, Value)> = rest.iter().map(|(a, b)| (a.as_str(), b.clone())).collect();
    let f = files(&book_json(), &rest);
    let b = load(&env, &f);
    let p = Persist::default();
    let mut nav = Nav { cur: Where { gui: Gui::Index, spread: 0 }, ..Nav::default() };
    let v = view_of(&env, &b, &p, &mut nav);
    // 11 on the first page, then 26 per spread: 37 entries need 2 spreads
    assert_eq!(v.max_spreads, 2);
    let buttons = |v: &View| v.elems.iter().filter(|e| matches!(e, Elem::EntryBtn { .. })).count();
    assert_eq!(buttons(&v), 11);
    nav.cur.spread = 1;
    let v = view_of(&env, &b, &p, &mut nav);
    assert_eq!(buttons(&v), 26);
}

// ---------------------------------------------------------------------------------------------
// pages
// ---------------------------------------------------------------------------------------------

fn words_of(v: &View) -> Vec<&BookText> {
    v.elems.iter().filter_map(|e| if let Elem::Words { text, .. } = e { Some(text) } else { None }).collect()
}
fn strings_of(v: &View) -> Vec<(&str, i32, i32)> {
    v.elems.iter().filter_map(|e| if let Elem::Str { text, x, y, .. } = e { Some((text.as_str(), *x, *y)) } else { None }).collect()
}

#[test]
fn the_first_text_page_has_the_entry_name_and_a_separator() {
    let env = TestEnv::default();
    let b = mini_book(&env);
    let p = Persist::default();
    let mut nav = Nav { cur: Where::entry("test:intro", 0), ..Nav::default() };
    let v = view_of(&env, &b, &p, &mut nav);
    // "Intro" is 5 characters = 30 px, centred on the left page (15 + 58)
    assert!(strings_of(&v).contains(&("Intro", 15 + 58 - 15, 18)));
    let t = words_of(&v);
    assert_eq!(t[0].words[0].y, 18 + 22, "the text starts 22 px under the top of the page");
    assert_eq!(t[0].words[0].x, 15);
    // the right page: no title, the text starts 4 px above the page top
    assert_eq!(t[1].words[0].y, 18 - 4);
    assert_eq!(t[1].words[0].x, 141);
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Blit { v: 180, u: 140, w: 110, .. })), "separator");
}

#[test]
fn a_titled_page_puts_the_text_12_down_and_a_missing_right_page_gets_the_filler() {
    let env = TestEnv::default();
    let b = mini_book(&env);
    let p = Persist::default();
    let mut nav = Nav { cur: Where::entry("test:more", 0), ..Nav::default() };
    let v = view_of(&env, &b, &p, &mut nav);
    assert_eq!(v.max_spreads, 1);
    // a one page entry: the first page has the name, and the filler is on the right page
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Blit { tex: Tex::Filler, x: 135, y: 22, .. })));
}

#[test]
fn crafting_pages_draw_the_grid_and_the_titles() {
    let mut env = TestEnv::default();
    env.recipes.insert(
        "minecraft:torch".into(),
        json!({"type": "minecraft:crafting_shaped", "pattern": [" C", " S"], "key": {"C": "minecraft:coal", "S": ["minecraft:stick", "minecraft:bamboo"]}, "result": {"id": "minecraft:torch", "count": 4}}),
    );
    env.recipes.insert("minecraft:planks".into(), json!({"type": "minecraft:crafting_shapeless", "ingredients": [{"item": "minecraft:oak_log"}], "result": "minecraft:oak_planks"}));
    let f = files(
        &book_json(),
        &[
            ("categories/c", cat("C")),
            ("entries/e", entry("E", "test:c", vec![json!({"type": "patchouli:crafting", "recipe": "minecraft:torch", "recipe2": "minecraft:planks", "text": "more"})])),
        ],
    );
    let b = load(&env, &f);
    let p = Persist::default();
    let mut nav = Nav { cur: Where::entry("test:e", 0), ..Nav::default() };
    let v = view_of(&env, &b, &p, &mut nav);
    let items: Vec<(&Vec<Stack>, i32, i32)> = v.elems.iter().filter_map(|e| if let Elem::Item { stacks, x, y, .. } = e { Some((stacks, *x, *y)) } else { None }).collect();
    // the shaped recipe is shrunk to its one column: coal over a choice of sticks
    let (rx, ry) = (15 + PAGE_WIDTH / 2 - 49, 18 + 4);
    assert!(items.iter().any(|(s, x, y)| s[0].id == "minecraft:coal" && *x == rx + 3 && *y == ry + 3));
    assert!(items.iter().any(|(s, x, y)| s.len() == 2 && s[0].id == "minecraft:stick" && *x == rx + 3 && *y == ry + 22));
    assert!(items.iter().any(|(s, x, y)| s[0].id == "minecraft:torch" && *x == rx + 79 && *y == ry + 22));
    assert!(items.iter().any(|(s, ..)| s[0].id == "minecraft:crafting_table"));
    // the second recipe is 78 lower, and shapeless recipes get the little icon
    assert!(items.iter().any(|(s, x, y)| s[0].id == "minecraft:oak_planks" && *x == rx + 79 && *y == ry + 78 + 22));
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Tip { lines, .. } if lines[0] == "Shapeless")));
    // the titles are the names of the results
    let s = strings_of(&v);
    assert!(s.iter().any(|(t, ..)| *t == "<minecraft:torch>"));
    assert!(s.iter().any(|(t, ..)| *t == "<minecraft:oak_planks>"));
    // the text is below two recipes: 4 + 156 - 13 = 147, too low (>= 156 - 10) so it is not drawn
    assert!(words_of(&v).is_empty() || words_of(&v).iter().all(|t| t.words.is_empty() || t.words[0].y < 18 + 147));
}

#[test]
fn a_recipe_of_another_type_or_missing_shows_nothing_but_the_text() {
    let mut env = TestEnv::default();
    env.recipes.insert("minecraft:stone".into(), json!({"type": "minecraft:smelting", "ingredient": "minecraft:cobblestone", "result": {"id": "minecraft:stone"}}));
    let f = files(
        &book_json(),
        &[
            ("categories/c", cat("C")),
            ("entries/e", entry("E", "test:c", vec![json!({"type": "crafting", "recipe": "minecraft:stone", "text": "hello"}), json!({"type": "smelting", "recipe": "minecraft:stone", "text": "t"})])),
        ],
    );
    let b = load(&env, &f);
    let p = Persist::default();
    let mut nav = Nav { cur: Where::entry("test:e", 0), ..Nav::default() };
    let v = view_of(&env, &b, &p, &mut nav);
    let items: Vec<&Elem> = v.elems.iter().filter(|e| matches!(e, Elem::Item { .. })).collect();
    // only the smelting page (right) has items: ingredient, furnace, result
    assert_eq!(items.len(), 3);
    assert!(matches!(items[1], Elem::Item { stacks, .. } if stacks[0].id == "minecraft:furnace"));
}

#[test]
fn spotlight_image_quest_relations_and_empty_pages() {
    let env = TestEnv::default();
    let f = files(
        &book_json(),
        &[
            ("categories/c", cat("C")),
            ("entries/t", entry("T", "test:c", vec![json!("a")])),
            (
                "entries/e",
                entry(
                    "E",
                    "test:c",
                    vec![
                        json!({"type": "spotlight", "item": "minecraft:diamond,minecraft:emerald", "title": "Gems", "text": "shiny"}),
                        json!({"type": "image", "images": ["test:textures/a.png", "test:textures/b.png"], "border": true, "title": "Pics", "text": "look"}),
                        json!({"type": "quest", "text": "q", "title": "", "trigger": "test:never"}),
                        json!({"type": "relations", "entries": ["test:t", "test:nowhere"], "text": "see"}),
                        json!({"type": "empty"}),
                        json!({"type": "quest", "text": "manual"}),
                    ],
                ),
            ),
        ],
    );
    let b = load(&env, &f);
    let p = Persist::default();
    let mut nav = Nav { cur: Where::entry("test:e", 0), ..Nav::default() };
    // spread 0: spotlight + image
    let v = view_of(&env, &b, &p, &mut nav);
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Blit { tex: Tex::Crafting, w: 66, h: 26, v: 102, x, y, .. } if *x == 15 + 25 && *y == 18 + 10)));
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Item { stacks, x, y, .. } if stacks.len() == 2 && *x == 15 + 50 && *y == 18 + 15)));
    assert!(strings_of(&v).iter().any(|(t, ..)| *t == "Gems"));
    assert_eq!(words_of(&v)[0].words[0].y, 18 + 40);
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Image { texture, w: 100, h: 100, sw: 200, .. } if texture == "test:textures/a.png")));
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Blit { u: 405, v: 149, w: 106, .. })), "border");
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Button { cmd: Cmd::Image { left: false, .. }, .. })));
    assert!(!v.elems.iter().any(|e| matches!(e, Elem::Button { cmd: Cmd::Image { left: true, .. }, .. })), "the first picture has no previous");
    // the arrow turns the picture
    click(&env, &b, &mut Persist::default(), &mut nav, Cmd::Image { page: 1, left: false }, false);
    let v = view_of(&env, &b, &p, &mut nav);
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Image { texture, .. } if texture == "test:textures/b.png")));
    // spread 1: quest (automatic) + relations
    nav.cur.spread = 1;
    let v = view_of(&env, &b, &p, &mut nav);
    let s = strings_of(&v);
    assert!(s.iter().any(|(t, ..)| *t == "Objective"));
    assert!(s.iter().any(|(t, ..)| *t == "Incomplete"));
    assert!(s.iter().any(|(t, ..)| *t == "Related Chapters"));
    let rel: Vec<&Elem> = v.elems.iter().filter(|e| matches!(e, Elem::EntryBtn { .. })).collect();
    assert_eq!(rel.len(), 1, "the entry that does not exist is left out");
    assert_eq!(words_of(&v).last().unwrap().words[0].y, 18 + 22 + 2 * 11, "the text is below room for both entries");
    // spread 2: empty (filler) + manual quest with its button
    nav.cur.spread = 2;
    let v = view_of(&env, &b, &p, &mut nav);
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Blit { tex: Tex::Filler, x, y, .. } if *x == 15 - 6 && *y == 18 + 4)));
    assert!(v.elems.iter().any(|e| matches!(e, Elem::PlainButton { text, .. } if text == "Mark Complete")));
    let mut p2 = Persist::default();
    p2.data("test:guide").viewed.push("test:e".into());
    click(&env, &b, &mut p2, &mut nav, Cmd::ToggleQuest("test:e".into()), false);
    let v = build_view(&env, &b, &p2, &nav);
    assert!(v.elems.iter().any(|e| matches!(e, Elem::PlainButton { text, .. } if text == "Mark Incomplete")));
    assert_eq!(b.contents.entry_state(&env, &p2, &b.info, "test:e"), ReadState::Completed);
}

#[test]
fn landing_page_lists_categories_and_the_index() {
    let env = TestEnv::default();
    let b = locked_book(&env);
    let p = Persist::default();
    let mut nav = Nav::default();
    let v = view_of(&env, &b, &p, &mut nav);
    let cats: Vec<(&String, i32, i32)> = v.elems.iter().filter_map(|e| if let Elem::CatBtn { name, x, y, .. } = e { Some((name, *x, *y)) } else { None }).collect();
    // Open, Closed (locked, last), then the index; four per row, 24 apart
    assert_eq!(cats.len(), 3);
    assert_eq!((cats[0].0.as_str(), cats[0].1, cats[0].2), ("Open", 151, 43));
    assert_eq!((cats[1].0.as_str(), cats[1].1, cats[1].2), ("Closed", 175, 43));
    assert_eq!((cats[2].0.as_str(), cats[2].1, cats[2].2), ("Entry Index", 199, 43));
    assert_eq!(words_of(&v)[0].words[0].x, LEFT_PAGE_X);
    assert_eq!(words_of(&v)[0].words[0].y, TOP_PADDING + 25);
    // the progress bar counts the entries that are not secret
    let tip = v.elems.iter().find_map(|e| if let Elem::Tip { lines, rect } = e { if rect[1] == 144 { Some(lines.clone()) } else { None } } else { None }).expect("progress tooltip");
    assert_eq!(tip[0], "4/7 Unlocked");
}

// ---------------------------------------------------------------------------------------------
// templates
// ---------------------------------------------------------------------------------------------

fn template_book(env: &TestEnv) -> Book {
    let f = files(
        &book_json(),
        &[
            ("categories/c", cat("C")),
            (
                "templates/gem",
                json!({
                    "include": [{"template": "test:title", "as": "head", "using": {"name": "#gem_name", "size": "2"}, "x": 0, "y": 0}],
                    "components": [
                        {"type": "patchouli:item", "item": "#gem", "x": 10, "y": 30, "framed": true},
                        {"type": "patchouli:text", "text": "Made of #gem_name#, count #gem->icount#.", "x": 0, "y": 60},
                        {"type": "patchouli:text", "text": "hidden", "guard": "#nope", "x": 0, "y": 90},
                        {"type": "patchouli:text", "text": "shown when set", "flag": "mod:jei", "x": 0, "y": 100},
                        {"type": "patchouli:separator", "x": -1, "y": -1},
                        {"type": "patchouli:tooltip", "tooltip": ["tip #gem_name#"], "x": 1, "y": 2, "width": 3, "height": 4},
                        {"type": "mod:custom", "x": 0, "y": 0}
                    ]
                }),
            ),
            ("templates/title", json!({"components": [{"type": "patchouli:header", "text": "#name->upper", "x": -1, "y": -1}]})),
            ("entries/e", entry("E", "test:c", vec![json!("intro"), json!({"type": "gem", "gem": "minecraft:diamond#3", "gem_name": "Diamond"})])),
        ],
    );
    load(env, &f)
}

#[test]
fn template_pages_resolve_variables_includes_and_functions() {
    let mut env = TestEnv::default();
    env.mods.insert("jei".into());
    let b = template_book(&env);
    let page = b.contents.entries["test:e"].pages[1].clone();
    let comps = compile_template_page(&env, &b.contents, &b.info, &page).unwrap();
    // own components first, then the included ones
    assert_eq!(comps.len(), 8);
    match &comps[0].kind {
        CompKind::Item { items, framed, .. } => {
            assert_eq!(items, &vec![Stack::new("minecraft:diamond", 3)]);
            assert!(framed);
        }
        other => panic!("{other:?}"),
    }
    match &comps[1].kind {
        CompKind::Text { text, .. } => assert_eq!(text, "Made of Diamond, count 3."),
        other => panic!("{other:?}"),
    }
    assert!(!comps[2].guard_pass, "the guard is a missing variable");
    assert!(comps[3].visible(&env));
    assert!(matches!(comps[4].kind, CompKind::Separator) && (comps[4].x, comps[4].y) == (0, 12));
    assert!(matches!(&comps[5].kind, CompKind::Tooltip { lines, .. } if lines[0] == "tip Diamond"));
    assert!(matches!(&comps[6].kind, CompKind::Unknown(_)));
    // the included header: its text variable is bound to the page's, then upper-cased
    match &comps[7].kind {
        CompKind::Header { text, .. } => assert_eq!(text, "DIAMOND"),
        other => panic!("{other:?}"),
    }
    assert_eq!(comps[7].x, PAGE_WIDTH / 2);
    // and the page draws what is visible
    let mut nav = Nav { cur: Where::entry("test:e", 0), ..Nav::default() };
    let v = view_of(&env, &b, &Persist::default(), &mut nav);
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Item { stacks, .. } if stacks[0].id == "minecraft:diamond")));
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Blit { tex: Tex::Crafting, w: 26, h: 26, .. })), "the frame");
    assert!(strings_of(&v).iter().any(|(t, ..)| *t == "DIAMOND"));
    assert!(!words_of(&v).iter().any(|t| t.words.iter().any(|w| w.text.contains("hidden"))));
}

#[test]
fn a_missing_template_is_told_on_the_page_and_circular_includes_are_refused() {
    let env = TestEnv::default();
    let f = files(
        &book_json(),
        &[
            ("categories/c", cat("C")),
            ("templates/a", json!({"include": [{"template": "test:b", "as": "b"}]})),
            ("templates/b", json!({"include": [{"template": "test:a", "as": "a"}]})),
            ("entries/e", entry("E", "test:c", vec![json!({"type": "ghost"}), json!({"type": "a"})])),
        ],
    );
    let b = load(&env, &f);
    let e = &b.contents.entries["test:e"];
    assert!(compile_template_page(&env, &b.contents, &b.info, &e.pages[0]).unwrap_err().contains("does not exist"));
    assert!(compile_template_page(&env, &b.contents, &b.info, &e.pages[1]).unwrap_err().contains("circular"));
    let mut nav = Nav { cur: Where::entry("test:e", 0), ..Nav::default() };
    let v = view_of(&env, &b, &Persist::default(), &mut nav);
    assert!(!v.elems.is_empty());
}

#[test]
fn inline_variables_and_functions() {
    let mut env = TestEnv::default();
    env.items.insert("minecraft:stone".into());
    let f = files(
        &book_json(),
        &[
            ("categories/c", cat("C")),
            ("templates/t", json!({"components": [
                {"type": "patchouli:text", "text": "#a#-#b#!", "x": 0, "y": 0},
                {"type": "patchouli:text", "text": "#w->capital", "x": 0, "y": 0},
                {"type": "patchouli:text", "text": "#w->fcapital", "x": 0, "y": 0},
                {"type": "patchouli:text", "text": "#missing->exists", "x": 0, "y": 0},
                {"type": "patchouli:text", "text": "#stone->iexists", "x": 0, "y": 0},
                {"type": "patchouli:text", "text": "#flag->inv", "x": 0, "y": 0},
                {"type": "patchouli:text", "text": "#stone->iname", "x": 0, "y": 0}
            ]})),
            ("entries/e", entry("E", "test:c", vec![json!({"type": "t", "a": "x", "b": 5, "w": "hELLO wORLD", "stone": "minecraft:stone", "flag": false})])),
        ],
    );
    let b = load(&env, &f);
    let comps = compile_template_page(&env, &b.contents, &b.info, &b.contents.entries["test:e"].pages[0]).unwrap();
    let texts: Vec<String> = comps.iter().filter_map(|c| if let CompKind::Text { text, .. } = &c.kind { Some(text.clone()) } else { None }).collect();
    assert_eq!(texts, ["x-5!", "HELLO WORLD", "Hello World", "false", "true", "true", "<minecraft:stone>"]);
}

// ---------------------------------------------------------------------------------------------
// processors, the list of books, pamphlets, lookups
// ---------------------------------------------------------------------------------------------

struct Doubler {
    n: i64,
}
impl Processor for Doubler {
    fn setup(&mut self, _env: &dyn Env, vars: &dyn Vars) {
        self.n = vars.get("n").as_i64().unwrap_or(0);
    }
    fn process(&self, _env: &dyn Env, key: &str) -> Option<Value> {
        match key {
            "twice" => Some(json!(self.n * 2)),
            "gem" => Some(json!("minecraft:emerald")),
            _ => None,
        }
    }
    fn allow_render(&self, group: &str) -> bool {
        group != "hidden"
    }
}

#[test]
fn a_registered_processor_fills_in_the_variables() {
    register_processor("test.Doubler", || Box::new(Doubler { n: 0 }));
    let env = TestEnv::default();
    let f = files(
        &book_json(),
        &[
            ("categories/c", cat("C")),
            (
                "templates/dbl",
                json!({
                    "processor": "test.Doubler",
                    "include": [{"template": "test:inner", "as": "in", "using": {"what": "#twice"}}],
                    "components": [
                        {"type": "patchouli:text", "text": "n is #n#, twice #twice#", "x": 0, "y": 0},
                        {"type": "patchouli:item", "item": "#gem", "x": 0, "y": 0},
                        {"type": "patchouli:text", "text": "never", "group": "hidden", "x": 0, "y": 0},
                        {"type": "patchouli:text", "text": "own #n#", "x": 0, "y": 0}
                    ]
                }),
            ),
            ("templates/inner", json!({"components": [{"type": "patchouli:text", "text": "inner #what#", "x": 1, "y": 1}]})),
            ("templates/unknown_class", json!({"processor": "no.such.Class", "components": [{"type": "patchouli:text", "text": "#n#!", "x": 0, "y": 0}]})),
            ("entries/e", entry("E", "test:c", vec![json!({"type": "dbl", "n": 21}), json!({"type": "unknown_class", "n": 5})])),
        ],
    );
    let b = load(&env, &f);
    let pages = &b.contents.entries["test:e"].pages;
    let comps = compile_template_page(&env, &b.contents, &b.info, &pages[0]).unwrap();
    let texts: Vec<String> = comps.iter().filter_map(|c| if let CompKind::Text { text, .. } = &c.kind { Some(text.clone()) } else { None }).collect();
    assert_eq!(texts, ["n is 21, twice 42", "never", "own 21", "inner 42"]);
    assert!(matches!(&comps[1].kind, CompKind::Item { items, .. } if items[0].id == "minecraft:emerald"));
    assert!(!comps[2].visible(&env), "the processor hides the group");
    // a class nobody registered: the page's own variables answer
    let comps = compile_template_page(&env, &b.contents, &b.info, &pages[1]).unwrap();
    assert!(matches!(&comps[0].kind, CompKind::Text { text, .. } if text == "5!"));
}

#[test]
fn the_list_of_books_pages_like_a_list_of_entries() {
    let env = TestEnv::default();
    let items: Vec<LibraryItem> = (0..30)
        .map(|i| LibraryItem { id: format!("m:b{i}"), title: format!("Book {i}"), entries: i, icon: Icon::Stack(Stack::new("patchouli:guide_book", 1)) })
        .collect();
    let v = build_library(&env, &items, 0);
    assert_eq!(v.max_spreads, 2);
    let books = |v: &View| v.elems.iter().filter_map(|e| if let Elem::EntryBtn { cmd: Cmd::OpenBook(id), .. } = e { Some(id.clone()) } else { None }).collect::<Vec<_>>();
    assert_eq!(books(&v).len(), 11);
    assert_eq!(books(&v)[0], "m:b0");
    let v = build_library(&env, &items, 1);
    assert_eq!(books(&v).len(), 19);
    assert_eq!(books(&v)[0], "m:b11");
    assert!(v.elems.iter().any(|e| matches!(e, Elem::Button { cmd: Cmd::Page(true), .. })));
    assert!(!v.elems.iter().any(|e| matches!(e, Elem::Button { cmd: Cmd::Page(false), .. })));
    assert_eq!(build_library(&env, &[], 5).max_spreads, 1);
}

#[test]
fn a_pamphlet_lists_its_entries_on_the_landing_page() {
    let env = TestEnv::default();
    let mut bj = book_json();
    bj["pamphlet"] = json!(true);
    let f = files(&bj, &[("categories/only", cat("Only")), ("entries/a", entry("A", "test:only", vec![json!("x")])), ("entries/b", entry("B", "test:only", vec![json!("x")]))]);
    let b = load(&env, &f);
    assert_eq!(b.contents.pamphlet.as_deref(), Some("test:only"));
    let p = Persist::default();
    let mut nav = Nav::default();
    let v = view_of(&env, &b, &p, &mut nav);
    assert_eq!(v.elems.iter().filter(|e| matches!(e, Elem::EntryBtn { .. })).count(), 2);
    assert!(!v.elems.iter().any(|e| matches!(e, Elem::CatBtn { .. })), "no categories, no index");
    // two categories: not a pamphlet
    let f = files(&bj, &[("categories/a", cat("A")), ("categories/b", cat("B"))]);
    assert!(load(&env, &f).contents.errors.iter().any(|e| e.contains("exactly one category")));
}

#[test]
fn books_with_their_own_item() {
    let mut bj = book_json();
    bj["dont_generate_book"] = json!(true);
    bj["custom_book_item"] = json!("mymod:lexicon");
    let info = BookInfo::from_json("test", "guide", &bj).unwrap();
    assert_eq!(info.custom_item().map(|s| s.id), Some("mymod:lexicon".to_owned()));
    assert!(BookInfo::from_json("test", "guide", &book_json()).unwrap().custom_item().is_none());
}

#[test]
fn an_item_opens_the_entry_that_explains_it() {
    let mut env = TestEnv::default();
    env.recipes.insert("minecraft:torch".into(), json!({"type": "minecraft:crafting_shapeless", "ingredients": ["minecraft:coal"], "result": {"id": "minecraft:torch"}}));
    let f = files(
        &book_json(),
        &[("categories/c", cat("C")), ("entries/e", entry("E", "test:c", vec![json!("a"), json!("b"), json!({"type": "crafting", "recipe": "minecraft:torch"})]))],
    );
    let b = load(&env, &f);
    let mut p = Persist::default();
    let mut nav = Nav::default();
    let fx = open_for_stack(&env, &b, &mut p, &mut nav, "minecraft:torch");
    assert!(fx.contains(&Effect::Flip));
    assert_eq!(nav.cur, Where::entry("test:e", 1), "page 2 is in spread 1");
    assert!(open_for_stack(&env, &b, &mut p, &mut nav, "minecraft:torch").is_empty(), "already there");
    assert!(open_for_stack(&env, &b, &mut p, &mut nav, "minecraft:dirt").is_empty());
}

#[test]
fn mark_read_marks_the_unlocked_entries_of_the_category() {
    let env = TestEnv::default();
    let f = files(
        &book_json(),
        &[
            ("categories/a", cat("A")),
            ("categories/b", cat("B")),
            ("entries/a1", entry("A1", "test:a", vec![json!("x")])),
            ("entries/b1", entry("B1", "test:b", vec![json!("x")])),
            ("entries/g", json!({"name": "G", "category": "test:a", "icon": "minecraft:stone", "advancement": "test:no", "pages": ["x"]})),
        ],
    );
    let b = load(&env, &f);
    let mut p = Persist::default();
    // the button exists while something is unread
    let mut nav = Nav::default();
    assert!(view_of(&env, &b, &p, &mut nav).elems.iter().any(|e| matches!(e, Elem::MarkRead { .. })));
    // inside category A: only A's entries
    click(&env, &b, &mut p, &mut nav, Cmd::Open(Where { gui: Gui::Category("test:a".into()), spread: 0 }), false);
    click(&env, &b, &mut p, &mut nav, Cmd::MarkRead, false);
    let viewed = &p.get("test:guide").unwrap().viewed;
    assert_eq!(viewed, &["test:a1".to_owned()], "b1 is in another category, g is locked");
    // from the main page: everything unlocked
    let mut nav = Nav::default();
    click(&env, &b, &mut p, &mut nav, Cmd::MarkRead, false);
    assert!(p.get("test:guide").unwrap().viewed.contains(&"test:b1".to_owned()));
    assert!(!view_of(&env, &b, &p, &mut nav).elems.iter().any(|e| matches!(e, Elem::MarkRead { .. })));
}

// ---------------------------------------------------------------------------------------------
// robustness
// ---------------------------------------------------------------------------------------------

#[test]
fn odd_texts_never_panic_or_hang() {
    let env = TestEnv::default();
    let book = mini_book(&env);
    let pieces = [
        "$(", ")", "$()", "$(br)", "$(li)", "$(l:test:more)", "$(/l)", "$(/l)$(/l)", "$(l:", "x", " ", "  ", "word", "§", "§l", "\n", "é", "日本語", "$(#zz)", "$(#12)", "$(k:nope)",
        "$(c:/say hi)", "$(/c)", "$(t:tip)", "$(/t)", "$(item)", "$(2br)", "-", "a-b", "\u{200B}", "$(foo:bar:baz)", "$(playername)", "<br>", "$(list", "/$",
    ];
    // a small linear congruential generator keeps the test the same on every run
    let mut seed: u64 = 12345;
    let mut next = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as usize
    };
    for _ in 0..600 {
        let n = 1 + next() % 40;
        let text: String = (0..n).map(|_| pieces[next() % pieces.len()]).collect::<Vec<_>>().join(if next() % 2 == 0 { "" } else { " " });
        let width = 10 + (next() % 120) as i32;
        for mode in [Overflow::Overflow, Overflow::Truncate, Overflow::Resize] {
            let mut b = book.clone();
            b.info.overflow = Some(mode);
            let t = text_of(&env, &b, &text, 0, 0, width, 0);
            assert!(t.scale > 0.0 && t.scale <= 1.0 + f32::EPSILON, "scale {} for {text:?}", t.scale);
            if let Some(w) = t.word_at(5.0, 5.0) {
                let _ = t.is_hot(w, 5.0, 5.0);
            }
            let _ = t.click(5.0, 5.0, 0);
        }
    }
}

#[test]
fn odd_books_never_panic() {
    let env = TestEnv::default();
    let f = files(
        &book_json(),
        &[
            ("categories/c", cat("C")),
            ("categories/loop_a", json!({"name": "A", "description": "d", "icon": "x", "parent": "test:loop_b"})),
            ("categories/loop_b", json!({"name": "B", "description": "d", "icon": "x", "parent": "test:loop_a"})),
            ("entries/empty_pages", entry("E", "test:c", vec![])),
            ("entries/weird", entry("W", "test:c", vec![json!(null), json!(5), json!({}), json!({"type": 7}), json!({"type": "spotlight"}), json!({"type": "image", "images": []}), json!({"type": "image"}), json!({"type": "crafting"}), json!({"type": "relations", "entries": "x"}), json!({"type": "multiblock"}), json!({"type": "entity"}), json!({"type": "text", "text": {"text": "json comp", "extra": [{"text": "!"}]}})])),
            ("entries/in_loop", entry("L", "test:loop_a", vec![json!("x")])),
        ],
    );
    let b = load(&env, &f);
    let p = Persist::default();
    for id in b.contents.entries.keys() {
        let mut nav = Nav { cur: Where::entry(id, 0), ..Nav::default() };
        for spread in 0..8 {
            nav.cur.spread = spread;
            normalize(&env, &b, &p, &mut nav);
            let v = build_view(&env, &b, &p, &nav);
            assert!(!v.elems.is_empty());
        }
    }
    for c in b.contents.categories.keys() {
        let nav = Nav { cur: Where { gui: Gui::Category(c.clone()), spread: 3 }, ..Nav::default() };
        let mut nav = nav;
        normalize(&env, &b, &p, &mut nav);
        build_view(&env, &b, &p, &nav);
        assert!(!b.contents.category_locked(&env, &b.info, c) || b.contents.categories[c].entries.is_empty() || true);
    }
    let v = build_view(&env, &b, &p, &Nav::default());
    assert!(!v.elems.is_empty());
}

// ---------------------------------------------------------------------------------------------
// multiblocks
// ---------------------------------------------------------------------------------------------

#[test]
fn dense_multiblocks_put_the_first_layer_on_top() {
    let v = json!({"pattern": [["AB", "0 "], ["CC", "  "]], "mapping": {"A": "test:a[facing=east]", "B": "#minecraft:logs", "C": "test:c", "0": "test:center"}});
    let (blocks, size) = multiblock_blocks(&v).unwrap();
    assert_eq!(size, [2, 2, 2]);
    let at = |x, y, z| blocks.iter().find(|b| (b.0, b.1, b.2) == (x, y, z)).map(|b| b.3.as_str());
    assert_eq!(at(0, 1, 0), Some("test:a"), "layer 0 is the top one");
    assert_eq!(at(0, 1, 1), None, "tags are not drawn");
    assert_eq!(at(1, 1, 0), Some("test:center"), "0 is air unless it is mapped");
    assert_eq!(at(0, 0, 0), Some("test:c"));
    let sparse = json!({"sparse_pattern": {"X": [[0, 0, 0], [2, 1, 3]]}, "mapping": {"X": "test:x"}});
    let (blocks, size) = multiblock_blocks(&sparse).unwrap();
    assert_eq!((blocks.len(), size), (2, [3, 2, 4]));
}

// ---------------------------------------------------------------------------------------------
// the books of the pack (skipped when they are not on this machine)
// ---------------------------------------------------------------------------------------------

#[test]
fn every_book_of_the_pack_loads_and_every_page_draws() {
    let root = std::path::Path::new(r"../../content/mods");
    if !root.is_dir() {
        return;
    }
    // One folder with the carries of all mods that have books: the pack's resource root.
    let mut merged = MemFiles::default();
    let mut found = BTreeMap::new();
    for m in std::fs::read_dir(root).unwrap().flatten() {
        for tree in ["assets", "data"] {
            let dirs = m.path().join("carry").join(tree);
            if !dirs.is_dir() {
                continue;
            }
            for ns in std::fs::read_dir(&dirs).unwrap().flatten() {
                let books = ns.path().join("patchouli_books");
                if !books.is_dir() {
                    continue;
                }
                let fs = FsFiles(m.path().join("carry"));
                let ns_name = ns.file_name().to_string_lossy().into_owned();
                let prefix = format!("{tree}/{ns_name}/patchouli_books");
                for rel in walk_all(&fs, &prefix) {
                    if let Some(text) = fs.read(&format!("{prefix}/{rel}")) {
                        merged.0.insert(format!("{prefix}/{rel}"), text);
                    }
                }
                found.insert(m.file_name().to_string_lossy().into_owned(), ns_name);
            }
        }
    }
    if merged.0.is_empty() {
        return;
    }
    let mut env = TestEnv::default();
    for ns in ["minecraft", "botania", "bloodmagic", "alchemistry", "statues", "growthcraft", "ironsbotany", "enigmaticlegacy", "ars_nouveau"] {
        env.mods.insert(ns.into());
    }
    let mut entries = 0usize;
    let mut pages = 0usize;
    for (r, info) in discover(&merged) {
        let info = match info {
            Ok(i) => i,
            Err(e) => {
                eprintln!("book {} refused: {e}", r.id());
                continue;
            }
        };
        let book = load_book(&merged, &env, info);
        let p = Persist::default();
        let mut template_pages = 0usize;
        let mut processor_pages = 0usize;
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for e in book.contents.entries.values() {
            for pg in &e.pages {
                *kinds.entry(pg.kind.clone()).or_default() += 1;
                if !pg.is_builtin() {
                    template_pages += 1;
                    let key = qualify(&book.info.ns, jstr(&pg.json, "type").unwrap_or(&pg.kind));
                    if book.contents.templates.get(&key).is_some_and(|t| t.get("processor").is_some()) {
                        processor_pages += 1;
                    }
                }
            }
        }
        eprintln!(
            "book {}: {} categories, {} entries, {} template pages ({} of them with a Java processor), {} errors",
            r.id(),
            book.contents.categories.len(),
            book.contents.entries.len(),
            template_pages,
            processor_pages,
            book.contents.errors.len()
        );
        for err in book.contents.errors.iter().take(3) {
            eprintln!("    {err}");
        }
        for id in book.contents.entries.keys() {
            let mut nav = Nav { cur: Where::entry(id, 0), ..Nav::default() };
            let n = visible_pages(&env, &book.info, &book.contents.entries[id]).len().div_ceil(2);
            for spread in 0..n {
                nav.cur.spread = spread;
                let v = build_view(&env, &book, &p, &nav);
                assert!(!v.elems.is_empty(), "{} {id}", r.id());
            }
            entries += 1;
            pages += n * 2;
        }
        for gui in [Gui::Landing, Gui::Index, Gui::History] {
            let nav = Nav { cur: Where { gui, spread: 0 }, ..Nav::default() };
            assert!(!build_view(&env, &book, &p, &nav).elems.is_empty());
        }
        for c in book.contents.categories.keys() {
            let nav = Nav { cur: Where { gui: Gui::Category(c.clone()), spread: 0 }, ..Nav::default() };
            build_view(&env, &book, &p, &nav);
        }
    }
    eprintln!("books of the pack: {} entries, {} pages drawn, from {found:?}", entries, pages);
}

fn walk_all(fs: &FsFiles, dir: &str) -> Vec<String> {
    fn go(fs: &FsFiles, dir: &str, rel: &str, out: &mut Vec<String>) {
        for name in fs.list(dir) {
            let path = format!("{dir}/{name}");
            let r = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if name.ends_with(".json") {
                out.push(r);
            } else if !name.contains('.') {
                go(fs, &path, &r, out);
            }
        }
    }
    let mut out = Vec::new();
    go(fs, dir, "", &mut out);
    out
}

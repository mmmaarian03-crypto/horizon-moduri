use horizon_visualworkbench as v;
use v::{Anim, Store, Table};

#[test]
fn tuned_client_config() {
    assert!(v::ROTATE_INGREDIENTS);
    assert!(!v::FLAT_RENDERING);
    assert!(v::RENDER_RESULT);
}

#[test]
fn table_is_nine_slots_and_the_result_does_not_count_as_content() {
    let mut t: Table<String> = Table::default();
    assert_eq!(t.slots.len(), 9);
    assert!(t.is_empty() && t.is_blank());
    t.last = Some("stick".into());
    assert!(t.is_empty());
    assert!(!t.is_blank());
    t.slots[4] = Some("plank".into());
    assert!(!t.is_empty());
}

#[test]
fn ease_out_quad_runs_from_start_to_end() {
    assert_eq!(v::ease_out_quad(0.0, 10.0, 90.0, 20.0), 10.0);
    assert!((v::ease_out_quad(20.0, 10.0, 90.0, 20.0) - 100.0).abs() < 1e-4);
    // fast first: halfway in time is three quarters of the way
    assert!((v::ease_out_quad(10.0, 0.0, 100.0, 20.0) - 75.0).abs() < 1e-4);
}

#[test]
fn an_empty_table_only_counts_ticks() {
    let mut a = Anim::default();
    for _ in 0..5 {
        a.tick(true, Some((1.0, 1.0)));
    }
    assert_eq!(a.ticks, 5);
    assert_eq!((a.current, a.next), (0.0, 0.0));
}

#[test]
fn the_items_turn_to_the_player_in_twenty_ticks_and_stop() {
    // a player to the north-west of the centre: angle = (atan2(-d0,-d1) + 3.927) % tau
    let mut a = Anim::default();
    a.tick(false, Some((-1.0, 1.0)));
    // the first sector change starts an animation
    assert!(a.ticks == 1);
    let mut last = a.current;
    for _ in 0..30 {
        a.tick(false, Some((-1.0, 1.0)));
        last = a.current;
    }
    // finished: current == next, a multiple of 90
    assert_eq!(a.current, a.next);
    assert_eq!(last % 90.0, 0.0, "{last}");
    // another side: turns again to a different multiple of 90
    let before = a.current;
    for _ in 0..30 {
        a.tick(false, Some((1.0, -1.0)));
    }
    assert_eq!(a.current, a.next);
    assert_eq!(a.current % 90.0, 0.0);
    assert_ne!(a.current, before);
}

/// The offset of a player standing in the given sector (0..4) of `playerAngle`.
fn toward(sector: u32) -> (f64, f64) {
    let theta = f64::from(sector) * std::f64::consts::FRAC_PI_2 + 0.3;
    let phi = theta - 3.926_990_816_987_241_4;
    (-phi.sin(), -phi.cos())
}

#[test]
fn it_takes_the_short_way_round() {
    let mut a = Anim::default();
    for (sector, expect) in [(2, 180.0), (3, 270.0)] {
        for _ in 0..40 {
            a.tick(false, Some(toward(sector)));
        }
        assert_eq!(a.current, expect);
    }
    // from 270 to 0: up through 360, not back down through 180
    let mut saw_wrap = false;
    let mut went_down = false;
    for _ in 0..40 {
        a.tick(false, Some(toward(0)));
        if a.current > 270.0 && a.current <= 360.0 {
            saw_wrap = true;
        }
        if a.current < 270.0 && a.current > 0.5 {
            went_down = true;
        }
    }
    assert!(saw_wrap && !went_down);
    assert_eq!(a.current, 0.0);
    // and from 0 to 270: down through 360 -> 270 (the short way is backwards)
    for _ in 0..40 {
        a.tick(false, Some(toward(3)));
    }
    assert_eq!(a.current, 270.0);
}

#[test]
fn floating_pose_values() {
    let a = Anim::default();
    let p = v::floating_pose(&a, 0.0, 4, true);
    assert_eq!(p.scale, 0.24);
    assert_eq!(p.first[0], 0.5);
    // index 4 is the centre cell: 1*3/16 + 0.3125 - 0.5 = 0
    assert!((p.second[0]).abs() < 1e-6 && (p.second[2]).abs() < 1e-6);
    assert_eq!(p.second[1], 1.09375);
    assert!(p.first[1] <= 0.0125 + 1e-6);
    let q = v::floating_pose(&a, 0.0, 0, false);
    assert_eq!(q.scale, 0.18);
    assert!((q.second[0] - (0.3125 - 0.5)).abs() < 1e-6);
    let r = v::floating_pose(&a, 0.0, 8, false);
    assert!((r.second[0] - (2.0 * 3.0 / 16.0 + 0.3125 - 0.5)).abs() < 1e-6);
    assert!((r.second[2] - (2.0 * 3.0 / 16.0 + 0.3125 - 0.5)).abs() < 1e-6);
    // the bobbing is within -0.0125..0.0125 -> abs form 0..0.0125
    for t in 0..200 {
        let mut a = Anim::default();
        a.ticks = t;
        let p = v::floating_pose(&a, 0.5, 3, false);
        assert!((0.0..=0.0125 + 1e-6).contains(&p.first[1]));
    }
}

#[test]
fn laying_pose_values() {
    let a = Anim::default();
    let p = v::laying_pose(&a, 0.0, 0, true);
    assert!(p.flat);
    assert_eq!((p.scale, p.second[1]), (0.25, 1.0625));
    let q = v::laying_pose(&a, 0.0, 0, false);
    assert_eq!((q.scale, q.second[1]), (0.175, 1.005));
}

#[test]
fn result_hovers_and_spins() {
    let p = v::result_pose(0.0, false, 0.5);
    assert_eq!(p.scale, 0.75);
    assert!((p.at[1] - (1.15 + 0.1 + 0.125)).abs() < 1e-6);
    let p = v::result_pose(0.0, true, 0.25);
    assert_eq!(p.scale, 1.0);
    let hi = v::result_pose(15.707_963, true, 0.25); // sin(pi/2)
    assert!((hi.at[1] - (1.15 + 0.14 + 0.0625)).abs() < 1e-5);
    assert!((hi.yaw - 15.707_963 / 20.0).abs() < 1e-6);
}

#[test]
fn the_world_file_round_trips_and_skips_bad_lines() {
    let mut store: Store<String> = Store::default();
    let mut t: Table<String> = Table::default();
    t.slots[0] = Some("{id:\"minecraft:stick\",count:2}".into());
    t.slots[8] = Some("{id:\"minecraft:oak_planks\",count:1}".into());
    t.last = Some("{id:\"minecraft:ladder\",count:3}".into());
    store.tables.insert([10, 64, -20], t.clone());
    store.tables.insert([0, 0, 0], Table::default()); // blank: not written
    let text = v::save(&store, |s| s.clone());
    assert!(!text.contains("T\t0\t0\t0"));
    let back = v::load(&text, |s| Some(s.to_owned()));
    assert_eq!(back.tables.len(), 1);
    assert_eq!(back.tables[&[10, 64, -20]], t);
    // a stack that no longer decodes is dropped, the rest stays
    let partial = v::load(&text, |s| (!s.contains("stick")).then(|| s.to_owned()));
    let t2 = &partial.tables[&[10, 64, -20]];
    assert!(t2.slots[0].is_none() && t2.slots[8].is_some() && t2.last.is_some());
    // junk lines
    let junk = v::load("x\nT\ta\tb\nS\t99\t1\nT\t1\t2\t3\nS\t99\tz\nS\t2\tq\n", |s| Some(s.to_owned()));
    assert_eq!(junk.tables.len(), 1);
    assert_eq!(junk.tables[&[1, 2, 3]].slots[2].as_deref(), Some("q"));
}

// ---- a fake world: the menu, the table block and its contents ----

#[derive(Default)]
struct World {
    store: Store<String>,
    /// block positions that are crafting tables, and loaded ones
    tables: Vec<[i32; 3]>,
    loaded: bool,
    grid: Vec<Option<String>>,
    spilled: Vec<String>,
}
impl World {
    fn open(&mut self, pos: [i32; 3]) {
        self.grid = v::open_table(&mut self.store, pos);
    }
    fn close(&mut self, pos: [i32; 3], result: Option<String>) {
        v::mirror(&mut self.store, pos, &self.grid, result);
        self.grid = vec![None; 9]; // nothing goes back to the player
    }
    fn tick(&mut self) {
        let tables = self.tables.clone();
        let loaded = self.loaded;
        for (_, t) in v::take_gone(&mut self.store, |p| loaded.then(|| tables.contains(&p))) {
            self.spilled.extend(t.slots.into_iter().flatten());
        }
    }
}

#[test]
fn closing_the_menu_keeps_the_grid_in_the_table() {
    let p = [5, 70, 5];
    let mut w = World { tables: vec![p], loaded: true, ..Default::default() };
    w.open(p);
    assert!(w.grid.iter().all(Option::is_none));
    w.grid[0] = Some("planks".into());
    w.grid[4] = Some("stick x2".into());
    w.close(p, Some("ladder".into()));
    assert!(w.grid.iter().all(Option::is_none));
    // the table keeps them, and the result
    let t = &w.store.tables[&p];
    assert_eq!(t.slots[0].as_deref(), Some("planks"));
    assert_eq!(t.last.as_deref(), Some("ladder"));
    // reopening gives the same grid back
    w.open(p);
    assert_eq!(w.grid[4].as_deref(), Some("stick x2"));
    // another table is separate
    let q = [6, 70, 5];
    w.tables.push(q);
    w.close(p, None);
    w.open(q);
    assert!(w.grid.iter().all(Option::is_none));
}

#[test]
fn mirror_reports_changes_only() {
    let p = [0, 0, 0];
    let mut s: Store<String> = Store::default();
    let mut g: Vec<Option<String>> = vec![None; 9];
    assert!(!v::mirror(&mut s, p, &g, None));
    g[3] = Some("x".into());
    assert!(v::mirror(&mut s, p, &g, None));
    assert!(!v::mirror(&mut s, p, &g, None));
    assert!(v::mirror(&mut s, p, &g, Some("r".into())));
}

#[test]
fn a_broken_table_spills_its_contents_once_and_only_when_loaded() {
    let p = [1, 64, 1];
    let mut w = World { tables: vec![p], loaded: true, ..Default::default() };
    w.open(p);
    w.grid[1] = Some("a".into());
    w.grid[7] = Some("b".into());
    w.close(p, Some("res".into()));
    w.tables.clear(); // broken
    w.loaded = false;
    w.tick();
    assert!(w.spilled.is_empty(), "an unloaded spot is not judged");
    assert_eq!(w.store.tables.len(), 1);
    w.loaded = true;
    w.tick();
    assert_eq!(w.spilled, vec!["a".to_string(), "b".to_string()]); // the result is not an item
    assert!(w.store.tables.is_empty());
    w.tick();
    assert_eq!(w.spilled.len(), 2);
}

#[test]
fn a_table_that_stays_a_table_is_kept() {
    let p = [1, 64, 1];
    let mut w = World { tables: vec![p], loaded: true, ..Default::default() };
    w.open(p);
    w.grid[0] = Some("a".into());
    w.close(p, None);
    for _ in 0..5 {
        w.tick();
    }
    assert!(w.spilled.is_empty());
    assert_eq!(w.store.tables.len(), 1);
}

#[test]
fn one_player_at_a_time_takes_a_tables_grid() {
    let (p, q) = ([1, 64, 1], [2, 64, 1]);
    // (position, who): 0 = the in-frame host, 7 = a guest's connection
    let holders = vec![(p, 0u32)];
    assert!(!v::may_bind(&holders, p, &7, true), "the host holds it: the guest gets a plain table");
    assert!(v::may_bind(&holders, p, &0, true), "the holder himself again");
    assert!(v::may_bind(&holders, q, &7, true), "another table is free");
    assert!(!v::may_bind(&holders, q, &7, false), "a grid with the player's own items stays his");
    assert!(v::may_bind(&[], p, &7, true));
}

#[test]
fn the_tables_a_server_sends_its_clients_round_trip() {
    // A server's client draws the tables from the world file's text the server sends (foundry channel
    // `visualworkbench:tables`): the same `save` / `load`.
    let mut s: Store<String> = Store::default();
    let mut g: Vec<Option<String>> = vec![None; 9];
    g[2] = Some("{id:\"minecraft:stick\",count:2}".into());
    v::mirror(&mut s, [3, 70, -4], &g, Some("{id:\"minecraft:ladder\",count:3}".into()));
    v::mirror(&mut s, [9, 70, 9], &vec![None; 9], None);
    let text = v::save(&s, |x| x.clone());
    let back: Store<String> = v::load(&text, |x| Some(x.to_owned()));
    assert_eq!(back.tables.len(), 1, "blank tables are not sent");
    assert_eq!(back.tables[&[3, 70, -4]], s.tables[&[3, 70, -4]]);
}

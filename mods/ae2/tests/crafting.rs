// Stage 2 (autocrafting) rules: patterns, the crafting calculation, CPUs, providers, assemblers,
// cards, the level emitter, cell partitions. Keys are plain strings here; `item_of` takes the
// part before '#' (a key with components is "item#data").
use horizon_ae2::crafting::*;
use horizon_ae2::*;
use std::collections::{BTreeMap, BTreeSet};

fn item_of(k: &Key) -> String {
    k.split('#').next().unwrap_or("").to_owned()
}
fn k(s: &str) -> Key {
    s.to_owned()
}
fn net(items: &[(&str, u64)]) -> BTreeMap<Key, u64> {
    items.iter().map(|(a, n)| (k(a), *n)).collect()
}
/// A processing pattern: inputs -> outputs.
fn proc_pattern(def: &str, inputs: &[(&str, u64)], outputs: &[(&str, u64)]) -> Details {
    let e = encode_processing(
        &inputs.iter().map(|(a, n)| Some((k(a), *n))).collect::<Vec<_>>(),
        &outputs.iter().map(|(a, n)| Some((k(a), *n))).collect::<Vec<_>>(),
    )
    .unwrap();
    processing_details(&k(def), &e).unwrap()
}

/// One network storage with unlimited room (a creative-like cell).
#[derive(Default)]
struct Net(BTreeMap<Key, u64>);
impl Mounts for Net {
    fn mounts(&self) -> usize {
        1
    }
    fn priority(&self, _: usize) -> i32 {
        0
    }
    fn insert(&mut self, _: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        if !simulate {
            *self.0.entry(key.clone()).or_insert(0) += amount;
        }
        amount
    }
    fn extract(&mut self, _: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        let have = self.0.get(key).copied().unwrap_or(0);
        let n = have.min(amount);
        if !simulate && n > 0 {
            *self.0.get_mut(key).unwrap() -= n;
            if self.0[key] == 0 {
                self.0.remove(key);
            }
        }
        n
    }
    fn list(&mut self, _: usize, out: &mut BTreeMap<Key, u64>) {
        for (k, n) in &self.0 {
            *out.entry(k.clone()).or_insert(0) += n;
        }
    }
}
struct Power(f64);
impl EnergySource for Power {
    fn extract_ae(&mut self, amount: f64, simulate: bool) -> f64 {
        let n = amount.min(self.0);
        if !simulate {
            self.0 -= n;
        }
        n
    }
}
/// Pattern providers that accept every push and keep what they got.
#[derive(Default)]
struct Machines_ {
    pushed: Vec<(Key, Vec<(Key, u64)>)>,
    busy: bool,
}
impl Machines for Machines_ {
    fn providers(&mut self, _: &Key) -> usize {
        1
    }
    fn busy(&mut self, _: usize) -> bool {
        self.busy
    }
    fn push(&mut self, _: usize, d: &Details, table: &[Vec<(Key, u64)>]) -> bool {
        self.pushed.push((d.definition.clone(), table.iter().flatten().cloned().collect()));
        true
    }
}

// ---- patterns ----

#[test]
fn patterns_encode_and_decode() {
    let e = encode_processing(&[Some((k("iron_ore"), 2)), None, Some((k("coal"), 1))], &[Some((k("iron_ingot"), 2)), Some((k("slag"), 1))]).unwrap();
    assert_eq!(e.inputs.len(), MAX_PROCESSING_INPUTS);
    assert_eq!(e.outputs.len(), MAX_PROCESSING_OUTPUTS);
    assert_eq!(Encoded::decode(&e.encode()), Some(e.clone()));
    // The first output slot is required, and one input.
    assert!(encode_processing(&[Some((k("a"), 1))], &[None, Some((k("b"), 1))]).is_none());
    assert!(encode_processing(&[None], &[Some((k("b"), 1))]).is_none());
    // Crafting: a recipe with a result; smithing: all three inputs.
    let grid = [Some(k("planks")), None, None, Some(k("planks")), None, None, None, None, None];
    let c = encode_recipe(PatternKind::Crafting, &grid, "stick", Some((k("stick"), 4)), true, false).unwrap();
    assert_eq!(Encoded::decode(&c.encode()), Some(c.clone()));
    assert!(encode_recipe(PatternKind::Crafting, &grid, "", Some((k("stick"), 4)), false, false).is_none());
    assert!(encode_recipe(PatternKind::Smithing, &[Some(k("t")), None, Some(k("a"))], "x", Some((k("r"), 1)), false, false).is_none());
    assert!(Encoded::decode("garbage").is_none());
    // The encode button: a blank is used only when the encoded slot is empty.
    assert_eq!(encode_slots(3, None), Some(2));
    assert_eq!(encode_slots(0, None), None);
    assert_eq!(encode_slots(0, Some("ae2:crafting_pattern")), Some(0));
    assert_eq!(encode_slots(1, Some("ae2:blank_pattern")), Some(1));
    assert_eq!(encode_slots(1, Some("minecraft:stone")), None);
}

#[test]
fn processing_inputs_condense_and_push_in_sparse_order() {
    let d = proc_pattern("p", &[("a", 1), ("b", 2), ("a", 3)], &[("c", 1), ("c", 1)]);
    assert_eq!(d.inputs.len(), 2);
    assert_eq!((d.inputs[0].primary().clone(), d.inputs[0].multiplier), (k("a"), 4));
    assert_eq!(d.outputs, vec![(k("c"), 2)]);
    let order = d.push_order(&[vec![(k("a"), 4)], vec![(k("b"), 2)]]);
    assert_eq!(order, vec![(k("a"), 1), (k("b"), 2), (k("a"), 3)]);
}

#[test]
fn crafting_pattern_condenses_the_grid_and_keeps_remainders() {
    let grid = [Some(k("water_bucket")), Some(k("sand")), Some(k("sand")), None, None, None, None, None, None];
    let e = encode_recipe(PatternKind::Crafting, &grid, "clay", Some((k("clay"), 1)), false, false).unwrap();
    let rem = vec![Some(k("bucket")), None, None, None, None, None, None, None, None];
    let d = recipe_details(&k("pat"), &e, (k("clay"), 1), &[], &rem).unwrap();
    assert_eq!(d.inputs.len(), 2);
    assert_eq!(d.inputs[0].remainder, Some(k("bucket")));
    assert_eq!(d.inputs[1].multiplier, 2);
    assert_eq!(d.grid[..3], [Some(0), Some(1), Some(1)]);
    // Substitutions accept the ingredient's other items.
    let planks = [Some(k("oak_planks")), None, None, Some(k("oak_planks")), None, None, None, None, None];
    let s = encode_recipe(PatternKind::Crafting, &planks, "stick", Some((k("stick"), 4)), true, false).unwrap();
    let alts = vec![vec![(k("oak_planks"), "oak_planks".into()), (k("birch_planks"), "birch_planks".into())]; 9];
    let d = recipe_details(&k("sticks"), &s, (k("stick"), 4), &alts, &[]).unwrap();
    assert!(d.inputs[0].is_valid(&k("birch_planks"), &item_of));
    assert!(!d.inputs[0].is_valid(&k("stone"), &item_of));
}

// ---- the crafting calculation ----

#[test]
fn plan_counts_bytes_and_used_items() {
    let book = Book::new(vec![proc_pattern("sticks", &[("planks", 2)], &[("stick", 4)])], BTreeSet::new());
    let p = plan(&book, &net(&[("planks", 2)]), &k("stick"), 4, Strategy::ReportMissing, &item_of);
    assert!(!p.simulation);
    // 1*4 (request) + 1*2 (planks) + 1 (one craft) + 2 nodes * 8.
    assert_eq!(p.bytes, 23);
    assert_eq!(p.used, net(&[("planks", 2)]));
    assert_eq!(p.crafts, vec![(k("sticks"), 1)]);
    assert!(p.missing.is_empty());
}

#[test]
fn plan_with_sub_crafts_and_missing_items() {
    let book = Book::new(
        vec![proc_pattern("sticks", &[("planks", 2)], &[("stick", 4)]), proc_pattern("planks", &[("log", 1)], &[("planks", 4)])],
        BTreeSet::new(),
    );
    let inv = net(&[("planks", 2), ("log", 1)]);
    // 12 sticks: 3 crafts, 6 planks = 2 stored + one log.
    let ok = plan(&book, &inv, &k("stick"), 12, Strategy::ReportMissing, &item_of);
    assert!(!ok.simulation);
    assert_eq!(ok.used, net(&[("planks", 2), ("log", 1)]));
    assert_eq!(ok.crafts, vec![(k("sticks"), 3), (k("planks"), 1)]);
    // 14 sticks: one log missing; the plan is a simulation.
    let miss = plan(&book, &inv, &k("stick"), 14, Strategy::ReportMissing, &item_of);
    assert!(miss.simulation);
    assert_eq!(miss.missing, net(&[("log", 1)]));
    let summary = miss.summary(&book, &inv);
    assert_eq!(summary[0].key, k("log"));
    assert_eq!((summary[0].missing, summary[0].stored), (1, 1));
    // CRAFT_LESS finds the largest amount that works.
    let less = plan(&book, &inv, &k("stick"), 14, Strategy::CraftLess, &item_of);
    assert!(!less.simulation);
    assert_eq!(less.output, (k("stick"), 12));
}

#[test]
fn plan_does_not_loop_on_cyclic_recipes() {
    // A from B and B from A: B's only pattern takes A, an ancestor (notRecursive).
    let book = Book::new(vec![proc_pattern("a", &[("b", 1)], &[("a", 1)]), proc_pattern("b", &[("a", 1)], &[("b", 1)])], BTreeSet::new());
    let p = plan(&book, &BTreeMap::new(), &k("a"), 3, Strategy::ReportMissing, &item_of);
    assert!(p.simulation);
    assert_eq!(p.missing, net(&[("b", 3)]));
    // A pattern that gives back its own input (limitQty): runs one at a time and reuses it.
    let book = Book::new(vec![proc_pattern("dup", &[("seed", 1), ("dirt", 1)], &[("seed", 2)])], BTreeSet::new());
    let p = plan(&book, &net(&[("dirt", 5)]), &k("seed"), 2, Strategy::ReportMissing, &item_of);
    assert!(p.simulation, "no seed to start with");
    assert_eq!(p.missing, net(&[("seed", 1)]));
    // CraftingCalculation ignores the requested key in the network (`ignore(output)`), and the
    // pattern cannot feed itself (notRecursive): a stored seed does not help either.
    let still = plan(&book, &net(&[("dirt", 5), ("seed", 1)]), &k("seed"), 2, Strategy::ReportMissing, &item_of);
    assert!(still.simulation);
    assert_eq!(still.missing, net(&[("seed", 1)]));
    // Feeding it from a sub-craft works: seed from wheat, then duplicated one at a time.
    let book = Book::new(
        vec![proc_pattern("grow", &[("seed", 1), ("dirt", 1)], &[("wheat", 1), ("seed", 1)]), proc_pattern("seed", &[("wheat", 1)], &[("seed", 1)])],
        BTreeSet::new(),
    );
    let ok = plan(&book, &net(&[("dirt", 5), ("wheat", 1)]), &k("seed"), 1, Strategy::ReportMissing, &item_of);
    assert!(!ok.simulation, "{ok:?}");
    assert_eq!(ok.used, net(&[("wheat", 1)]));
}

#[test]
fn plan_reuses_container_items() {
    // clay = water bucket + dirt (the bucket comes back); water bucket = bucket.
    let grid = [Some(k("water_bucket")), Some(k("dirt")), None, None, None, None, None, None, None];
    let e = encode_recipe(PatternKind::Crafting, &grid, "clay", Some((k("clay"), 1)), false, false).unwrap();
    let mut rem = vec![None; 9];
    rem[0] = Some(k("bucket"));
    let clay = recipe_details(&k("clay"), &e, (k("clay"), 1), &[], &rem).unwrap();
    let book = Book::new(vec![clay, proc_pattern("fill", &[("bucket", 1)], &[("water_bucket", 1)])], BTreeSet::new());
    let p = plan(&book, &net(&[("bucket", 1), ("dirt", 2)]), &k("clay"), 2, Strategy::ReportMissing, &item_of);
    assert!(!p.simulation, "{p:?}");
    assert_eq!(p.used, net(&[("bucket", 1), ("dirt", 2)]));
    assert_eq!(p.crafts, vec![(k("clay"), 2), (k("fill"), 2)]);
}

#[test]
fn plan_tries_the_next_pattern_and_uses_emitters() {
    // Two patterns for gears: the first needs diamonds (none), the second iron.
    let book = Book::new(
        vec![proc_pattern("dia", &[("diamond", 1)], &[("gear", 1)]), proc_pattern("iron", &[("iron", 2)], &[("gear", 1)])],
        BTreeSet::new(),
    );
    let p = plan(&book, &net(&[("iron", 4)]), &k("gear"), 2, Strategy::ReportMissing, &item_of);
    assert!(!p.simulation);
    assert!(p.multiple_paths);
    assert_eq!(p.crafts, vec![(k("iron"), 2)]);
    // A level emitter that can emit for redstone (CRAFT_VIA_REDSTONE): emitted, not missing.
    let book = Book::new(vec![proc_pattern("lamp", &[("redstone", 4)], &[("lamp", 1)])], BTreeSet::from([k("redstone")]));
    let p = plan(&book, &BTreeMap::new(), &k("lamp"), 1, Strategy::ReportMissing, &item_of);
    assert!(!p.simulation);
    assert_eq!(p.emitted, net(&[("redstone", 4)]));
}

#[test]
fn plan_takes_fuzzy_templates_when_substituting() {
    // A damaged sword (another key of the same item) is valid for a substituting recipe.
    let grid = [Some(k("sword")), None, None, None, None, None, None, None, None];
    let e = encode_recipe(PatternKind::Crafting, &grid, "salvage", Some((k("iron"), 1)), true, false).unwrap();
    let alts = vec![vec![(k("sword"), "sword".into())]];
    let d = recipe_details(&k("salvage"), &e, (k("iron"), 1), &alts, &[]).unwrap();
    let book = Book::new(vec![d], BTreeSet::new());
    let p = plan(&book, &net(&[("sword#damage=5", 1)]), &k("iron"), 1, Strategy::ReportMissing, &item_of);
    assert!(!p.simulation);
    assert_eq!(p.used, net(&[("sword#damage=5", 1)]));
}

// ---- crafting CPUs ----

fn units(list: &[(Pos, Unit)]) -> BTreeMap<Pos, Unit> {
    list.iter().copied().collect()
}

#[test]
fn cpu_multiblock_validity() {
    // A 2x1x2 box with a 4k storage and an accelerator.
    let ok = units(&[([0, 0, 0], Unit::Storage(4)), ([1, 0, 0], Unit::Accelerator), ([0, 0, 1], Unit::Unit), ([1, 0, 1], Unit::Monitor)]);
    let c = clusters(&ok);
    assert_eq!(c.len(), 1);
    assert_eq!((c[0].min, c[0].max, c[0].storage, c[0].coprocessors), ([0, 0, 0], [1, 0, 1], 4096, 1));
    // No storage: not formed.
    assert!(clusters(&units(&[([0, 0, 0], Unit::Unit), ([1, 0, 0], Unit::Accelerator)])).is_empty());
    // An L shape: the box has a hole.
    assert!(clusters(&units(&[([0, 0, 0], Unit::Storage(1)), ([1, 0, 0], Unit::Unit), ([0, 0, 1], Unit::Unit)])).is_empty());
    // A unit touching the box from outside (verifyUnownedRegion).
    let mut touch = ok.clone();
    touch.insert([2, 0, 0], Unit::Unit);
    touch.insert([2, 0, 1], Unit::Unit);
    touch.insert([0, 1, 0], Unit::Unit);
    assert!(clusters(&touch).iter().all(|c| !c.members.contains(&[0, 0, 0])));
    // 17 blocks long is the largest.
    let line: BTreeMap<Pos, Unit> = (0..17).map(|x| ([x, 0, 0], Unit::Storage(1))).collect();
    assert_eq!(clusters(&line)[0].storage, 17 * 1024);
    let long: BTreeMap<Pos, Unit> = (0..18).map(|x| ([x, 0, 0], Unit::Storage(1))).collect();
    assert!(clusters(&long).is_empty());
    assert_eq!(Unit::of("ae2:256k_crafting_storage").map(Unit::bytes), Some(256 * 1024));
}

#[test]
fn a_formed_cpu_takes_one_channel() {
    // Without a controller: 8 ad-hoc channels; a CPU of 9 blocks + 7 devices fits.
    let mut nodes = Vec::new();
    let mut groups = BTreeMap::new();
    for x in 0..9 {
        groups.insert(nodes.len(), 0);
        nodes.push(NodeSpec { pos: [x, 0, 0], kind: NodeKind::Device { channel: true, front: None }, idle: 1.0 });
    }
    for x in 0..7 {
        nodes.push(NodeSpec { pos: [x, 1, 0], kind: NodeKind::Device { channel: true, front: None }, idle: 1.0 });
    }
    let nets = networks(&nodes);
    assert_eq!(nets.len(), 1);
    let ch = assign_channels_grouped(&nodes, &nets[0], &groups);
    assert_eq!(ch.error, None);
    assert_eq!(ch.in_use, 8);
    assert_eq!(ch.granted.len(), 16);
    // The same blocks unformed need 16 channels: too many.
    assert_eq!(assign_channels(&nodes, &nets[0]).error, Some(AdHocError::TooManyChannels));
}

#[test]
fn cpu_selection_and_storage_limits() {
    let cpus = [
        CpuInfo { active: true, busy: false, storage: 1024, coprocessors: 0 },
        CpuInfo { active: true, busy: false, storage: 65536, coprocessors: 4 },
        CpuInfo { active: true, busy: false, storage: 4096, coprocessors: 4 },
        CpuInfo { active: false, busy: false, storage: 1 << 20, coprocessors: 9 },
    ];
    // Players: most co-processors, then the smallest storage; machines: fewest co-processors.
    assert_eq!(select_cpu(&cpus, 1000, true), Ok(2));
    assert_eq!(select_cpu(&cpus, 1000, false), Ok(0));
    assert_eq!(select_cpu(&cpus, 5000, false), Ok(1));
    assert_eq!(select_cpu(&cpus, 1 << 21, true), Err(SubmitError::NoSuitableCpu { offline: 1, busy: 0, too_small: 3 }));
    assert_eq!(select_cpu(&[], 1, true), Err(SubmitError::NoCpuFound));
    // A CPU too small for the plan, offline, busy.
    let book = Book::new(vec![proc_pattern("sticks", &[("planks", 2)], &[("stick", 4)])], BTreeSet::new());
    let p = plan(&book, &net(&[("planks", 2)]), &k("stick"), 4, Strategy::ReportMissing, &item_of);
    let mut store = Net(net(&[("planks", 2)]));
    let mut cpu = Cpu::default();
    assert_eq!(cpu.submit(&p, 22, true, &mut store, 1, None, true), Err(SubmitError::CpuTooSmall));
    assert_eq!(cpu.submit(&p, 1024, false, &mut store, 1, None, true), Err(SubmitError::CpuOffline));
    assert_eq!(cpu.submit(&p, 1024, true, &mut store, 1, None, true), Ok(()));
    assert_eq!(cpu.inventory, net(&[("planks", 2)]));
    assert!(store.0.is_empty());
    assert_eq!(cpu.submit(&p, 1024, true, &mut store, 2, None, true), Err(SubmitError::CpuBusy));
}

#[test]
fn a_missing_ingredient_goes_back_to_the_network() {
    let book = Book::new(vec![proc_pattern("sticks", &[("planks", 2)], &[("stick", 4)])], BTreeSet::new());
    let p = plan(&book, &net(&[("planks", 2)]), &k("stick"), 4, Strategy::ReportMissing, &item_of);
    // Someone took a plank between the plan and the start.
    let mut store = Net(net(&[("planks", 1)]));
    let mut cpu = Cpu::default();
    assert_eq!(cpu.submit(&p, 1024, true, &mut store, 1, None, true), Err(SubmitError::MissingIngredient(k("planks"), 1)));
    assert!(cpu.inventory.is_empty());
    assert_eq!(store.0, net(&[("planks", 1)]));
}

#[test]
fn co_processors_set_the_crafting_speed() {
    let book = Book::new(vec![proc_pattern("p", &[("a", 1)], &[("b", 1)])], BTreeSet::new());
    for (copro, expected) in [(0u32, 2usize), (3, 8)] {
        let p = plan(&book, &net(&[("a", 100)]), &k("b"), 20, Strategy::ReportMissing, &item_of);
        let mut store = Net(net(&[("a", 100)]));
        let mut cpu = Cpu::default();
        cpu.submit(&p, 1 << 20, true, &mut store, 1, None, true).unwrap();
        let mut m = Machines_::default();
        let mut power = Power(1e9);
        for _ in 0..8 {
            cpu.tick(copro, &book, &mut m, &mut power, &mut store, &item_of);
        }
        // coprocessors + 1 pushes in every 4 ticks (usedOps over the last 3).
        assert_eq!(m.pushed.len(), expected, "co-processors {copro}");
        assert_eq!(cpu.job.as_ref().unwrap().waiting.get("b").copied(), Some(expected as u64));
    }
}

#[test]
fn pushes_need_energy_and_a_free_provider() {
    let book = Book::new(vec![proc_pattern("p", &[("a", 3)], &[("b", 1)])], BTreeSet::new());
    let p = plan(&book, &net(&[("a", 30)]), &k("b"), 2, Strategy::ReportMissing, &item_of);
    let mut store = Net(net(&[("a", 30)]));
    let mut cpu = Cpu::default();
    cpu.submit(&p, 1 << 20, true, &mut store, 1, None, true).unwrap();
    let mut m = Machines_ { busy: true, ..Default::default() };
    cpu.tick(0, &book, &mut m, &mut Power(1e9), &mut store, &item_of);
    assert!(m.pushed.is_empty());
    m.busy = false;
    // 3 AE per push (one per input item).
    cpu.tick(0, &book, &mut m, &mut Power(2.0), &mut store, &item_of);
    assert!(m.pushed.is_empty());
    let mut power = Power(10.0);
    cpu.tick(0, &book, &mut m, &mut power, &mut store, &item_of);
    assert_eq!(m.pushed.len(), 1);
    assert!((power.0 - 7.0).abs() < 1e-9);
    // The inputs went out of the CPU, nothing else.
    assert_eq!(cpu.inventory, net(&[("a", 3)]));
}

#[test]
fn results_finish_the_job_and_go_to_the_network() {
    let book = Book::new(
        vec![proc_pattern("sticks", &[("planks", 2)], &[("stick", 4)]), proc_pattern("planks", &[("log", 1)], &[("planks", 4)])],
        BTreeSet::new(),
    );
    let mut store = Net(net(&[("log", 1)]));
    let p = plan(&book, &store.0, &k("stick"), 4, Strategy::ReportMissing, &item_of);
    let mut cpu = Cpu::default();
    cpu.submit(&p, 1 << 20, true, &mut store, 7, None, true).unwrap();
    let mut m = Machines_::default();
    let mut power = Power(1e9);
    for _ in 0..20 {
        cpu.tick(0, &book, &mut m, &mut power, &mut store, &item_of);
        // The machine answers every push at once (the provider returns into the network).
        while let Some((def, _)) = m.pushed.pop() {
            let out = book.get(&def).unwrap().outputs.clone();
            for (key, n) in out {
                let a = cpu.insert(&key, n, false);
                // A standalone job lets the final output continue into storage.
                let left = n - a.stored;
                if left > 0 {
                    net_insert(&mut store, &key, left, false);
                }
            }
        }
    }
    assert!(cpu.job.is_none());
    assert_eq!(cpu.finished, vec![Finished { output: (k("stick"), 4), success: true, player: true }]);
    // 4 planks made, 2 used: the other 2 are stored when the CPU is idle.
    cpu.tick(0, &book, &mut m, &mut power, &mut store, &item_of);
    assert!(cpu.inventory.is_empty());
    assert_eq!(store.0, net(&[("planks", 2), ("stick", 4)]));
}

#[test]
fn cancelling_a_job_loses_nothing() {
    let book = Book::new(vec![proc_pattern("p", &[("a", 2)], &[("b", 1)])], BTreeSet::new());
    let mut store = Net(net(&[("a", 10)]));
    let p = plan(&book, &store.0, &k("b"), 5, Strategy::ReportMissing, &item_of);
    let mut cpu = Cpu::default();
    cpu.submit(&p, 1 << 20, true, &mut store, 1, None, true).unwrap();
    let mut m = Machines_::default();
    cpu.tick(3, &book, &mut m, &mut Power(1e9), &mut store, &item_of);
    assert_eq!(m.pushed.len(), 4);
    cpu.cancel();
    assert!(cpu.job.is_none());
    // The machine's results come back later: the CPU no longer takes them.
    assert_eq!(cpu.insert(&k("b"), 4, false), Accept::default());
    net_insert(&mut store, &k("b"), 4, false);
    cpu.tick(3, &book, &mut m, &mut Power(1e9), &mut store, &item_of);
    // 2 a still in the CPU went back; 8 a became 4 b.
    assert_eq!(store.0, net(&[("a", 2), ("b", 4)]));
    assert!(cpu.inventory.is_empty());
    assert_eq!(cpu.finished.last().map(|f| f.success), Some(false));
}

#[test]
fn a_running_job_survives_save_and_load() {
    let book = Book::new(vec![proc_pattern("p", &[("a", 2)], &[("b", 1)])], BTreeSet::new());
    let mut store = Net(net(&[("a", 10)]));
    let p = plan(&book, &store.0, &k("b"), 5, Strategy::ReportMissing, &item_of);
    let mut cpu = Cpu::default();
    cpu.submit(&p, 1 << 20, true, &mut store, 42, Some(([1, 2, 3], Some(Dir::East))), false).unwrap();
    cpu.set_total(&book);
    let mut m = Machines_::default();
    cpu.tick(1, &book, &mut m, &mut Power(1e9), &mut store, &item_of);
    let mut s = State::default();
    let mut d = Device::new("ae2:1k_crafting_storage", 0, 0);
    cpu.save_to(&mut d);
    s.devices.insert([0, 64, 0], d);
    let back = State::load(Some(s.save()));
    assert_eq!(back, s);
    let loaded = Cpu::load_from(back.devices.at([0, 64, 0]).unwrap());
    assert_eq!(loaded, cpu);
    assert_eq!(loaded.job.as_ref().map(|j| j.tasks.clone()), Some(vec![(k("p"), 3)]));
    // A record whose block is gone spills the CPU's items.
    let mut d = back.devices.at([0, 64, 0]).unwrap().clone();
    assert_eq!(d.take_list_items(), vec![(k("a"), 6)]);
}

// ---- pattern providers ----

#[derive(Default)]
struct Sides {
    room: BTreeMap<Dir, u64>,
    held: BTreeMap<Dir, BTreeMap<Key, u64>>,
}
impl Targets for Sides {
    fn sides(&mut self) -> Vec<Dir> {
        self.room.keys().copied().collect()
    }
    fn insert(&mut self, side: Dir, key: &Key, amount: u64, simulate: bool) -> u64 {
        let room = self.room.get(&side).copied().unwrap_or(0);
        let n = room.min(amount);
        if !simulate {
            *self.room.get_mut(&side).unwrap() -= n;
            *self.held.entry(side).or_default().entry(key.clone()).or_insert(0) += n;
        }
        n
    }
    fn contains_any(&mut self, side: Dir, keys: &BTreeSet<Key>) -> bool {
        self.held.get(&side).is_some_and(|h| h.iter().any(|(k, n)| *n > 0 && keys.contains(k)))
    }
}

#[test]
fn provider_round_robin_blocking_and_send_list() {
    let d = proc_pattern("p", &[("ore", 2)], &[("ingot", 2)]);
    let table = vec![vec![(k("ore"), 2)]];
    let inputs = d.input_keys();
    let mut t = Sides { room: BTreeMap::from([(Dir::North, 100), (Dir::South, 100)]), ..Default::default() };
    let mut pr = Provider::default();
    assert!(pr.push(&d, &table, &mut t, &inputs));
    assert!(pr.push(&d, &table, &mut t, &inputs));
    // Round robin: one push each side.
    assert_eq!(t.held[&Dir::North][&k("ore")], 2);
    assert_eq!(t.held[&Dir::South][&k("ore")], 2);
    // Blocking mode: both sides still hold ore, nothing is pushed.
    pr.blocking = true;
    assert!(!pr.push(&d, &table, &mut t, &inputs));
    // A target with room for part of the push: the rest waits, the provider is busy.
    let mut small = Sides { room: BTreeMap::from([(Dir::Up, 1)]), ..Default::default() };
    let mut pr = Provider::default();
    assert!(pr.push(&d, &table, &mut small, &inputs));
    assert!(pr.busy());
    assert_eq!(pr.send, vec![(k("ore"), 1)]);
    assert!(!pr.push(&d, &table, &mut small, &inputs));
    *small.room.get_mut(&Dir::Up).unwrap() = 5;
    assert!(pr.flush(&mut small));
    assert!(!pr.busy());
    assert_eq!(small.held[&Dir::Up][&k("ore")], 2);
    // Saved with its send list (real items).
    let mut dev = Device::new("ae2:pattern_provider", 18, 0);
    pr.send = vec![(k("ore"), 3)];
    pr.send_dir = Some(Dir::Up);
    pr.save_to(&mut dev);
    assert_eq!(Provider::load_from(&dev), pr);
}

// ---- the molecular assembler ----

#[test]
fn assembler_speed_cards_and_crafts() {
    for (cards, ticks) in [(0u32, 10), (1, 8), (2, 6), (3, 5), (4, 4), (5, 2)] {
        let mut power = Power(1e9);
        let mut progress = 0;
        let mut n = 0;
        while progress < ASSEMBLER_PROGRESS {
            progress += assembler_progress(&mut power, 1, cards);
            n += 1;
        }
        assert_eq!(n, ticks, "{cards} speed cards");
    }
    // 0 cards: 10 AE per tick; without energy no progress.
    let mut power = Power(10.0);
    assert_eq!(assembler_progress(&mut power, 1, 0), 10);
    assert_eq!(assembler_progress(&mut power, 1, 0), 0);
    let grid = [Some(k("water_bucket")), Some(k("dirt")), None, None, None, None, None, None, None];
    let e = encode_recipe(PatternKind::Crafting, &grid, "clay", Some((k("clay"), 1)), false, false).unwrap();
    let mut rem = vec![None; 9];
    rem[0] = Some(k("bucket"));
    let d = recipe_details(&k("clay"), &e, (k("clay"), 1), &[], &rem).unwrap();
    let filled = d.fill_grid(&[vec![(k("water_bucket"), 1)], vec![(k("dirt"), 1)]]);
    assert_eq!(filled[..2], [Some(k("water_bucket")), Some(k("dirt"))]);
    let (out, after) = assembler_craft(&d, &filled, &item_of).unwrap();
    assert_eq!(out, (k("clay"), 1));
    assert_eq!(after[0], Some(k("bucket")));
    assert_eq!(after[1], None);
    // The bucket left in the grid is not valid for the plan: it is ejected.
    assert_eq!(assembler_eject(Some(&d), &after, &item_of), Some(0));
    assert!(assembler_craft(&d, &after, &item_of).is_none());
}

// ---- cards, emitter, cells ----

#[test]
fn upgrade_cards_rules() {
    let none: Vec<String> = Vec::new();
    assert!(may_install(Host::ExportBus, &none, CRAFTING_CARD));
    assert!(!may_install(Host::ImportBus, &none, CRAFTING_CARD));
    let four = vec![SPEED_CARD.to_owned(); 4];
    assert!(!may_install(Host::ImportBus, &four, SPEED_CARD));
    assert!(may_install(Host::Assembler, &four, SPEED_CARD));
    assert_eq!(upgrade_slots(Host::Interface), 1);
    assert_eq!([io_ops(0), io_ops(1), io_ops(2), io_ops(3), io_ops(4)], [1, 8, 32, 64, 96]);
    assert_eq!([io_slots(0), io_slots(1), io_slots(2), io_slots(5)], [1, 5, 9, 9]);
    assert_eq!([storage_bus_slots(0), storage_bus_slots(1), storage_bus_slots(5)], [18, 27, 63]);
    // Fuzzy and inverter filters.
    let f = Filter { keys: vec![k("sword")], inverted: false, fuzzy: true };
    assert!(f.passes(&k("sword#dmg"), &item_of));
    let inv = Filter { keys: vec![k("stone")], inverted: true, fuzzy: false };
    assert!(!inv.passes(&k("stone"), &item_of));
    assert!(inv.passes(&k("dirt"), &item_of));
    assert!(!Filter { keys: vec![], inverted: true, fuzzy: false }.imports(&k("dirt"), &item_of));
    assert!(Filter::default().imports(&k("dirt"), &item_of));
}

#[test]
fn level_emitter_rules() {
    let n = net(&[("iron", 10), ("iron#nbt", 5), ("gold", 1)]);
    assert_eq!(emitter_reported(&n, Some(&k("iron")), false, &item_of, 100), 10);
    assert_eq!(emitter_reported(&n, Some(&k("iron")), true, &item_of, 100), 15);
    assert_eq!(emitter_reported(&n, None, false, &item_of, 100), 16);
    // HIGH_SIGNAL: on at the level; LOW_SIGNAL: on below it.
    assert!(emitter_on(true, false, false, false, 10, 10));
    assert!(!emitter_on(true, false, false, false, 11, 10));
    assert!(emitter_on(true, false, false, true, 11, 10));
    assert!(!emitter_on(false, false, false, false, 0, 10));
    // Crafting card: while a CPU waits for it.
    assert!(emitter_on(true, true, true, false, 0, 0));
    assert!(!emitter_on(true, true, false, false, 0, 99));
}

#[test]
fn cell_partitions_and_cards() {
    let mut c = CellConfig::decode("");
    assert!(c.is_empty());
    c.partition[0] = Some(k("cobblestone"));
    c.upgrades.push(INVERTER_CARD.to_owned());
    let back = CellConfig::decode(&c.encode());
    assert_eq!(back, c);
    let f = back.filter();
    assert!(!f.passes(&k("cobblestone"), &item_of));
    assert!(f.passes(&k("dirt"), &item_of));
    let p = CellConfig::partition_from(&[(k("a"), 5), (k("b"), 1)]);
    assert_eq!(p.iter().flatten().count(), 2);
    assert_eq!(p.len(), CELL_PARTITION);
}

/// The network as the adapter builds it: a CPU with a job mounted first (priority i32::MAX,
/// preferred for everything), then the storage.
struct WithCpu<'a> {
    cpu: &'a mut Cpu,
    store: &'a mut Net,
}
impl Mounts for WithCpu<'_> {
    fn mounts(&self) -> usize {
        2
    }
    fn priority(&self, i: usize) -> i32 {
        if i == 0 { i32::MAX } else { 0 }
    }
    fn insert(&mut self, i: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        if i == 0 {
            // A standalone job: the final output goes on into storage.
            self.cpu.insert(key, amount, simulate).stored
        } else {
            self.store.insert(0, key, amount, simulate)
        }
    }
    fn extract(&mut self, i: usize, key: &Key, amount: u64, simulate: bool) -> u64 {
        if i == 0 { 0 } else { self.store.extract(0, key, amount, simulate) }
    }
    fn listed(&self, i: usize, _: &Key) -> bool {
        i == 0
    }
    fn list(&mut self, i: usize, out: &mut BTreeMap<Key, u64>) {
        if i == 1 {
            self.store.list(0, out);
        }
    }
}

#[test]
fn returned_items_reach_the_waiting_cpu_first() {
    let book = Book::new(
        vec![proc_pattern("sticks", &[("planks", 2)], &[("stick", 4)]), proc_pattern("planks", &[("log", 1)], &[("planks", 4)])],
        BTreeSet::new(),
    );
    let mut store = Net(net(&[("log", 1), ("planks", 50)]));
    // 50 planks stored, yet the plan asks for crafting sticks from... the stored planks.
    let p = plan(&book, &store.0, &k("stick"), 4, Strategy::ReportMissing, &item_of);
    assert_eq!(p.crafts, vec![(k("sticks"), 1)]);
    let mut cpu = Cpu::default();
    cpu.submit(&p, 1 << 20, true, &mut store, 1, None, true).unwrap();
    let mut m = Machines_::default();
    cpu.tick(0, &book, &mut m, &mut Power(1e9), &mut store, &item_of);
    assert_eq!(m.pushed.len(), 1);
    // A provider returns 4 sticks and 3 stray planks into the network: the CPU takes nothing
    // but what it waits for, the final output and the rest go to storage.
    let mut w = WithCpu { cpu: &mut cpu, store: &mut store };
    assert_eq!(net_insert(&mut w, &k("stick"), 4, false), 4);
    assert_eq!(net_insert(&mut w, &k("planks"), 3, false), 3);
    assert!(cpu.job.is_none());
    assert_eq!(store.0, net(&[("log", 1), ("planks", 51), ("stick", 4)]));
    // A sub-craft's result is taken by the CPU (it waits for it).
    let p = plan(&book, &net(&[("log", 1)]), &k("stick"), 4, Strategy::ReportMissing, &item_of);
    let mut store = Net(net(&[("log", 1)]));
    let mut cpu = Cpu::default();
    cpu.submit(&p, 1 << 20, true, &mut store, 2, None, true).unwrap();
    cpu.tick(0, &book, &mut m, &mut Power(1e9), &mut store, &item_of);
    let mut w = WithCpu { cpu: &mut cpu, store: &mut store };
    assert_eq!(net_insert(&mut w, &k("planks"), 4, false), 4);
    assert_eq!(cpu.inventory, net(&[("planks", 4)]));
    assert!(store.0.is_empty());
}

#[test]
fn a_cpu_behind_a_controller_takes_one_channel() {
    // Controller, a line of 4 cables, a 3x1x4 CPU (12 blocks) and 7 devices on the line.
    let mut nodes = vec![NodeSpec { pos: [0, 0, 0], kind: NodeKind::Controller, idle: 3.0 }];
    for x in 1..5 {
        nodes.push(NodeSpec { pos: [x, 0, 0], kind: NodeKind::Cable { color: FLUIX, dense: false }, idle: 0.0 });
    }
    let mut groups = BTreeMap::new();
    for x in 5..8 {
        for z in 0..4 {
            groups.insert(nodes.len(), 0);
            nodes.push(NodeSpec { pos: [x, 0, z], kind: NodeKind::Device { channel: true, front: None }, idle: 1.0 });
        }
    }
    for x in 1..5 {
        nodes.push(NodeSpec { pos: [x, 1, 0], kind: NodeKind::Device { channel: true, front: None }, idle: 1.0 });
    }
    for x in 1..4 {
        nodes.push(NodeSpec { pos: [x, -1, 0], kind: NodeKind::Device { channel: true, front: None }, idle: 1.0 });
    }
    let nets = networks(&nodes);
    assert_eq!(nets.len(), 1);
    let ch = assign_channels_grouped(&nodes, &nets[0], &groups);
    assert_eq!(ch.state, ControllerState::Online);
    // 7 devices + 1 for the CPU = 8 channels through the cable; all 19 devices work.
    assert_eq!(ch.in_use, 8);
    assert_eq!(ch.granted.len(), 19);
    let without = assign_channels(&nodes, &nets[0]);
    assert!(without.granted.len() < 19);
}

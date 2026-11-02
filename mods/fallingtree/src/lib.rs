// Falling Tree for Horizon: the tree-finding rules, without any game code.
// Written from the original mod's documented behaviour and its config file
// (Falling Tree by Rakambda, LGPL-3.0); no code of the original is used.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub type Pos = [i32; 3];

/// What a block is, as far as felling a tree cares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A block of `#minecraft:logs` (or mangrove roots), by its block name.
    Log(String),
    /// A block of `#minecraft:leaves`.
    Leaves { persistent: bool },
    /// Nether wart blocks and shroomlights: the "leaves" of huge fungi.
    Wart,
    Other,
}

/// The options of `fallingtree.json` this port follows, with its defaults.
#[derive(Clone, Debug)]
pub struct Settings {
    pub max_scan_size: usize,
    pub max_size: usize,
    pub max_leaf_distance_from_log: usize,
    pub minimum_leaves_around_required: usize,
    pub include_persistent_leaves_in_required_count: bool,
    pub leaves_breaking: bool,
    pub allow_mixed_logs: bool,
    pub break_nether_tree_warts: bool,
    pub break_mangrove_roots: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            max_scan_size: 500,
            max_size: 100,
            max_leaf_distance_from_log: 15,
            minimum_leaves_around_required: 1,
            include_persistent_leaves_in_required_count: true,
            leaves_breaking: true,
            allow_mixed_logs: false,
            break_nether_tree_warts: true,
            break_mangrove_roots: true,
        }
    }
}

/// Reads blocks. `None` means not loaded or outside the world: never part of a tree.
pub trait World {
    fn kind(&self, p: Pos) -> Option<Kind>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum Plan {
    /// Not a log: the game breaks the block as usual.
    NotALog,
    /// Logs with too few leaves around them (a house, a log pile): one block only.
    NotATree,
    /// More logs than `max_size` (or the scan ran past `max_scan_size`): one block only.
    TooBig { found: usize },
    /// The other logs of the tree, furthest from the cut first, then the cut itself.
    Fell { logs: Vec<Pos> },
}

/// Leaves (vanilla's `LeavesBlock` distance) stay only within 6 steps of a log.
const LEAF_REACH: usize = 6;

fn neighbours26(p: Pos) -> impl Iterator<Item = Pos> {
    (-1..=1).flat_map(move |dx| {
        (-1..=1).flat_map(move |dy| {
            (-1..=1).filter_map(move |dz| {
                (dx != 0 || dy != 0 || dz != 0).then(|| {
                    [
                        p[0].saturating_add(dx),
                        p[1].saturating_add(dy),
                        p[2].saturating_add(dz),
                    ]
                })
            })
        })
    })
}
fn neighbours6(p: Pos) -> [Pos; 6] {
    [
        [p[0] - 1, p[1], p[2]],
        [p[0] + 1, p[1], p[2]],
        [p[0], p[1] - 1, p[2]],
        [p[0], p[1] + 1, p[2]],
        [p[0], p[1], p[2] - 1],
        [p[0], p[1], p[2] + 1],
    ]
}
fn dist2(a: Pos, b: Pos) -> i64 {
    (0..3)
        .map(|i| {
            let d = i64::from(a[i]) - i64::from(b[i]);
            d * d
        })
        .sum()
}

fn mangrove(name: &str) -> bool {
    name == "minecraft:mangrove_log" || name == "minecraft:mangrove_roots"
}
/// Whether a log of `other` belongs to the tree cut at a log of `origin`.
fn same_tree(s: &Settings, origin: &str, other: &str) -> bool {
    origin == other
        || s.allow_mixed_logs
        || (s.break_mangrove_roots && mangrove(origin) && mangrove(other))
}
fn leaf_like(s: &Settings, k: &Kind) -> bool {
    match k {
        Kind::Leaves { .. } => true,
        Kind::Wart => s.break_nether_tree_warts,
        _ => false,
    }
}

/// What breaking the block at `origin` does to the tree it belongs to.
pub fn plan(world: &impl World, s: &Settings, origin: Pos) -> Plan {
    let Some(Kind::Log(name)) = world.kind(origin) else {
        return Plan::NotALog;
    };
    // The whole tree: every connected log of the same kind, diagonals included.
    let mut seen = BTreeSet::from([origin]);
    let mut queue = VecDeque::from([origin]);
    let mut logs = vec![origin];
    while let Some(p) = queue.pop_front() {
        for n in neighbours26(p) {
            if !seen.insert(n) {
                continue;
            }
            if let Some(Kind::Log(other)) = world.kind(n)
                && same_tree(s, &name, &other)
            {
                logs.push(n);
                if logs.len() > s.max_scan_size {
                    return Plan::TooBig { found: logs.len() };
                }
                queue.push_back(n);
            }
        }
    }
    if logs.len() > s.max_size {
        return Plan::TooBig { found: logs.len() };
    }
    // A tree has leaves: count the leaf blocks touching its logs.
    let mut leaves = BTreeSet::new();
    for &l in &logs {
        for n in neighbours26(l) {
            match world.kind(n) {
                Some(Kind::Leaves { persistent })
                    if !persistent || s.include_persistent_leaves_in_required_count =>
                {
                    leaves.insert(n);
                }
                Some(Kind::Wart) if s.break_nether_tree_warts => {
                    leaves.insert(n);
                }
                _ => {}
            }
        }
        if leaves.len() >= s.minimum_leaves_around_required {
            break;
        }
    }
    if leaves.len() < s.minimum_leaves_around_required {
        return Plan::NotATree;
    }
    // FURTHEST_FIRST, the cut last; ties in a fixed order.
    let mut rest: Vec<Pos> = logs[1..].to_vec();
    rest.sort_by(|a, b| dist2(*b, origin).cmp(&dist2(*a, origin)).then(a.cmp(b)));
    rest.push(origin);
    Plan::Fell { logs: rest }
}

/// After the tree's logs are gone: the leaves (and warts) that no remaining log
/// holds any more, as vanilla decay would take them, but at once.
/// Only leaves within `max_leaf_distance_from_log` of a removed log are taken.
pub fn loose_leaves(world: &impl World, s: &Settings, removed: &[Pos]) -> Vec<Pos> {
    if !s.leaves_breaking || removed.is_empty() {
        return Vec::new();
    }
    // The leaves around the felled tree, with their steps from it.
    let reach = s.max_leaf_distance_from_log + LEAF_REACH;
    let mut depth: BTreeMap<Pos, usize> = BTreeMap::new();
    let mut queue = VecDeque::new();
    for &r in removed {
        for n in neighbours6(r) {
            if !depth.contains_key(&n) && world.kind(n).is_some_and(|k| leaf_like(s, &k)) {
                depth.insert(n, 1);
                queue.push_back(n);
            }
        }
    }
    while let Some(p) = queue.pop_front() {
        let d = depth[&p];
        if d >= reach {
            continue;
        }
        for n in neighbours6(p) {
            if !depth.contains_key(&n) && world.kind(n).is_some_and(|k| leaf_like(s, &k)) {
                depth.insert(n, d + 1);
                queue.push_back(n);
            }
        }
    }
    // How far each of them is from a log still standing (through leaves).
    let removed: BTreeSet<Pos> = removed.iter().copied().collect();
    let mut held: BTreeMap<Pos, usize> = BTreeMap::new();
    let mut queue = VecDeque::new();
    for &p in depth.keys() {
        let by_log = neighbours6(p).into_iter().any(|n| {
            !removed.contains(&n) && matches!(world.kind(n), Some(Kind::Log(_)))
        });
        if by_log {
            held.insert(p, 1);
            queue.push_back(p);
        }
    }
    while let Some(p) = queue.pop_front() {
        let d = held[&p];
        if d >= LEAF_REACH {
            continue;
        }
        for n in neighbours6(p) {
            if depth.contains_key(&n) && !held.contains_key(&n) {
                held.insert(n, d + 1);
                queue.push_back(n);
            }
        }
    }
    let mut out: Vec<Pos> = depth
        .iter()
        .filter(|&(p, &d)| {
            d <= s.max_leaf_distance_from_log
                && !held.contains_key(p)
                && !matches!(world.kind(*p), Some(Kind::Leaves { persistent: true }))
        })
        .map(|(p, _)| *p)
        .collect();
    out.sort();
    out
}

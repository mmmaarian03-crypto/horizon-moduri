// The fake world of the tests: stacks, a stack codec, and a grid where several "mods" expose
// capabilities (forja's machine table, another mod's energy cells and tanks, vanilla-like chests).
#![allow(dead_code)]
use horizon_forja::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct St {
    pub item: String,
    pub count: u32,
    /// Stands for components (a named or enchanted item does not stack with a plain one).
    pub tag: Option<String>,
}
pub fn st(item: &str, count: u32) -> St {
    St { item: item.into(), count, tag: None }
}
impl Stack for St {
    fn item(&self) -> &str {
        &self.item
    }
    fn count(&self) -> u32 {
        self.count
    }
    fn set_count(&mut self, count: u32) {
        self.count = count;
    }
    fn same_kind(&self, other: &Self) -> bool {
        self.item == other.item && self.tag == other.tag
    }
}
/// Ender pearls stack to 16, tools to 1, the rest to 64.
pub fn max_stack(s: &St) -> u32 {
    match s.item.as_str() {
        "minecraft:ender_pearl" => 16,
        "minecraft:iron_pickaxe" => 1,
        _ => 64,
    }
}
pub struct Codec;
impl StackCodec<St> for Codec {
    fn encode(&self, s: &St) -> String {
        format!("{}*{}*{}", s.item, s.count, s.tag.clone().unwrap_or_default())
    }
    fn decode(&self, t: &str) -> Option<St> {
        let mut it = t.split('*');
        let item = it.next()?.to_owned();
        let count = it.next()?.parse().ok()?;
        let tag = it.next().filter(|s| !s.is_empty()).map(str::to_owned);
        Some(St { item, count, tag })
    }
}

/// A grid of blocks: forja machines (by spec), another mod's energy cells and tanks (as a
/// registered provider would expose them, same access on every side), plain chests.
#[derive(Default)]
pub struct Fake {
    pub blocks: BTreeMap<Pos, String>,
    pub specs: BTreeMap<String, Spec>,
    pub machines: Machines<St>,
    pub cells: BTreeMap<Pos, EnergyStorage>,
    pub tanks: BTreeMap<Pos, FluidTank>,
    pub chests: BTreeMap<Pos, Inventory<St>>,
}
impl Fake {
    pub fn machine(&mut self, p: Pos, spec: &Spec) {
        self.blocks.insert(p, spec.block.clone());
        self.specs.insert(spec.block.clone(), spec.clone());
        self.machines.ensure(p, spec);
    }
    pub fn m(&mut self, p: Pos) -> &mut Machine<St> {
        self.machines.map.get_mut(&p).unwrap()
    }
    pub fn total_energy(&self) -> i64 {
        self.machines.map.values().filter_map(|m| m.energy).map(|e| e.energy as i64).sum::<i64>()
            + self.cells.values().map(|c| c.energy as i64).sum::<i64>()
    }
    pub fn total_fluid(&self, fluid: &str) -> i64 {
        let mut n = 0i64;
        for m in self.machines.map.values() {
            for t in &m.tanks {
                if t.fluid.fluid == fluid {
                    n += t.amount() as i64;
                }
            }
        }
        for t in self.tanks.values() {
            if t.fluid.fluid == fluid {
                n += t.amount() as i64;
            }
        }
        n
    }
    pub fn total_items(&self, item: &str) -> u32 {
        let mut n = 0;
        for m in self.machines.map.values() {
            n += m.items.slots.iter().flatten().filter(|s| s.item == item).map(|s| s.count).sum::<u32>();
        }
        for c in self.chests.values() {
            n += c.slots.iter().flatten().filter(|s| s.item == item).map(|s| s.count).sum::<u32>();
        }
        n + self.machines.drops.iter().filter(|d| d.1.item == item).map(|d| d.1.count).sum::<u32>()
    }
    fn spec_at(&self, p: Pos) -> Option<Spec> {
        self.blocks.get(&p).and_then(|b| self.specs.get(b)).cloned()
    }
}
impl Grid for Fake {
    type Stack = St;
    fn energy(&mut self, p: Pos, face: Option<Dir>, f: &mut dyn FnMut(&mut dyn EnergyHandler)) -> bool {
        if let Some(spec) = self.spec_at(p) {
            let m = self.machines.ensure(p, &spec);
            return match m.energy_handler(face) {
                Some(mut h) => {
                    f(&mut h);
                    true
                }
                None => false,
            };
        }
        match self.cells.get_mut(&p) {
            Some(c) => {
                f(c);
                true
            }
            None => false,
        }
    }
    fn fluid(&mut self, p: Pos, face: Option<Dir>, f: &mut dyn FnMut(&mut dyn FluidHandler)) -> bool {
        if let Some(spec) = self.spec_at(p) {
            let m = self.machines.ensure(p, &spec);
            return match m.fluid_handler(face) {
                Some(mut h) => {
                    f(&mut h);
                    true
                }
                None => false,
            };
        }
        match self.tanks.get_mut(&p) {
            Some(t) => {
                f(t);
                true
            }
            None => false,
        }
    }
    fn items(&mut self, p: Pos, face: Option<Dir>, f: &mut dyn FnMut(&mut dyn ItemHandler<St>)) -> bool {
        let ms: &dyn Fn(&St) -> u32 = &max_stack;
        if let Some(spec) = self.spec_at(p) {
            let m = self.machines.ensure(p, &spec);
            return match m.item_handler(face, ms) {
                Some(mut h) => {
                    f(&mut h);
                    true
                }
                None => false,
            };
        }
        match self.chests.get_mut(&p) {
            Some(inv) => {
                let mut h = InvView { inv, roles: &[], access: Access::Both, internal: false, max_stack: ms, valid: None };
                f(&mut h);
                true
            }
            None => false,
        }
    }
}

/// A small deterministic random sequence (xorshift) for the property loops.
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self, n: u64) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x % n.max(1)
    }
}

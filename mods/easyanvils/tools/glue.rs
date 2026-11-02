
// ---- the game library's side (inside `horizon_client::stations`) ----

struct Live<'a> {
    e: &'a crate::enchant::Enchantments,
    items: &'a crate::items::Items,
}
impl Rules for Live<'_> {
    fn can_enchant(&self, id: &str, item: &str) -> bool {
        self.e.can_enchant(self.items, id, item)
    }
    fn compatible(&self, a: &str, b: &str) -> bool {
        self.e.compatible(a, b)
    }
    fn max_level(&self, id: &str) -> i32 {
        self.e.get(id).map_or(0, |d| d.max_level as i32)
    }
    fn anvil_cost(&self, id: &str) -> i32 {
        self.e.get(id).map_or(1, |d| d.anvil_cost as i32)
    }
    fn valid_repair(&self, stack: &Stack, material: &Stack) -> bool {
        self.e.props(&stack.item).is_some_and(|p| {
            crate::enchant::Enchantments::item_in(self.items, &p.repairable, &material.item)
        })
    }
}

fn view(items: &crate::items::Items, s: &crate::items::ItemStack) -> Stack {
    Stack {
        item: s.item.to_string(),
        count: s.count,
        damage: s.damage as i32,
        max_damage: if super::is_damageable(items, s) {
            items.get(&s.item).map_or(0, |i| i.max_damage as i32)
        } else {
            0
        },
        repair_cost: s.repair_cost() as i32,
        custom_name: s.custom_name().map(str::to_owned),
        enchants: s
            .crafting_enchantments()
            .iter()
            .map(|(id, l)| (id.to_string(), *l as i32))
            .collect(),
    }
}

/// The result stack: the left item with what the rules changed.
fn apply(base: &crate::items::ItemStack, out: &Stack) -> crate::items::ItemStack {
    let mut r = base.clone();
    r.damage = out.damage.max(0) as u32;
    r.data_mut().custom_name = out.custom_name.clone();
    r.data_mut().repair_cost = out.repair_cost.max(0) as u32;
    r.set_crafting_enchantments(
        out.enchants
            .iter()
            .map(|(id, l)| (std::sync::Arc::from(id.as_str()), (*l).max(0) as u32))
            .collect(),
    );
    r.tidy();
    r
}

/// `stations::anvil_result` with Easy Anvils' rules (`ModAnvilMenu.createResult`).
pub fn result(
    e: &crate::enchant::Enchantments,
    items: &crate::items::Items,
    input: Option<&crate::items::ItemStack>,
    addition: Option<&crate::items::ItemStack>,
    name: Option<&str>,
    hover_name: &str,
    creative: bool,
) -> super::AnvilResult {
    let live = Live { e, items };
    let left = input.map(|s| view(items, s));
    let right = addition.map(|s| view(items, s));
    let out = anvil_result(&live, &Config::MARIAN, left.as_ref(), right.as_ref(), name, hover_name, creative);
    super::AnvilResult {
        result: match (&out.result, input) {
            (Some(r), Some(base)) => Some(apply(base, r)),
            _ => None,
        },
        cost: out.cost,
        repair_item_count_cost: out.repair_item_count_cost,
        only_renaming: out.only_renaming,
    }
}

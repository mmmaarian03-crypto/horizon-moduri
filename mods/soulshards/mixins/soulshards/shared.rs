// What the game library's hooks need from Soul Shards (horizon_client::soulshards).

/// The id of the dying mob that must drop no experience (a cage-born mob, the pack's `dropExperience = false`);
/// `u32::MAX` = none. Written by the death hook, read by `mobs::drops::drops_experience`.
pub static NO_XP: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(u32::MAX);

/// `EventHandler.onAnvil` (allowShardCombination = true): two Soul Shards bound to the same entity give one shard
/// with the left binding (its owner) and both kill counts (capped at `Tier.maxKills` = 1024, the pack's
/// tiers.json); cost = the left shard's tier index * 6; one item of the right slot is used (Forge's material cost
/// 1). The same rule is `engine::combine` (tested in tests/rules.rs); the tier thresholds are tiers.json's.
pub fn anvil(
    input: Option<&crate::items::ItemStack>,
    addition: Option<&crate::items::ItemStack>,
) -> Option<(crate::items::ItemStack, i32)> {
    use horizon_world::nbt::{JavaString, Tag};
    const SHARD: &str = "soulshards:soul_shard";
    const TIERS: [i32; 6] = [0, 64, 128, 256, 512, 1024];
    let (l, r) = (input?, addition?);
    if &*l.item != SHARD || &*r.item != SHARD {
        return None;
    }
    let binding = |s: &crate::items::ItemStack| s.data()?.other("minecraft:custom_data")?.get("binding").cloned();
    let (lb, rb) = (binding(l)?, binding(r)?);
    let bound = |b: &Tag| match b.get("bound") {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    };
    let kills = |b: &Tag| match b.get("kills") {
        Some(Tag::Int(n)) => *n,
        _ => 0,
    };
    let entity = bound(&lb)?;
    if bound(&rb).as_deref() != Some(entity.as_str()) {
        return None;
    }
    let tier = |k: i32| TIERS.iter().rposition(|t| *t <= k).unwrap_or(0) as i32;
    let total = (kills(&lb).saturating_add(kills(&rb))).min(1024);
    let mut nb = lb.clone();
    if let Tag::Compound(c) = &mut nb {
        c.insert(JavaString::from("kills"), Tag::Int(total));
    }
    let mut out = crate::items::ItemStack::new(SHARD, 1);
    let d = out.data_mut();
    d.set_other("minecraft:custom_data", Some(Tag::compound([("binding", nb)])));
    let model = format!("tier_{}", tier(total));
    d.set_other(
        "minecraft:custom_model_data",
        Some(Tag::compound([("strings", Tag::List(vec![Tag::String(model.as_str().into())]))])),
    );
    if total >= 1024 {
        d.set_other("minecraft:enchantment_glint_override", Some(Tag::Byte(1)));
    }
    Some((out, tier(kills(&lb)) * 6))
}

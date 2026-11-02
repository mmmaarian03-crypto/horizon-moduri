// RETURN on `Plan::items`: the jetpacks' and goggles' armour numbers (JetpackArmorMaterial).
fn simplyjetpacks_items(value: (), map: &mut J, components: bool) {
    let () = value;
    simplyjetpacks::patch_items(map, components);
}

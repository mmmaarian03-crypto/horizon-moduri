// RETURN on `Plan::items`: each pattern item's `provides_banner_patterns` is its own tag.
fn additionalbanners_items(value: (), map: &mut J, components: bool) {
    let () = value;
    additionalbanners::patch_items(map, components);
}

fn storagedrawers_signal(containers: &Containers<'_>, view: &View) -> ::core::ops::ControlFlow<i32> {
    // A drawer gives a comparator nothing (its redstone upgrade is not ported; stacks of
    // thousands would otherwise read as more than 15).
    let pos = view.parts[0].0;
    let state = containers.world.block(pos.0, pos.1, pos.2);
    if crate::game::storagedrawers::storagedrawers_size(containers.name(state)).is_some() {
        ::core::ops::ControlFlow::Break(0)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}

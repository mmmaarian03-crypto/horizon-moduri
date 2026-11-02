fn storagedrawers_pre_remove(name: &str) -> ::core::ops::ControlFlow<PreRemove> {
    // Containers.dropContents over every slot of a drawer: its drawers and its 7 upgrade slots.
    // The game library cannot be reached from here, so the sizes are written out; the tests
    // check this rule against the rules crate.
    let Some(path) = name.strip_prefix("storagedrawers:") else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let Some(at) = path.rfind("_drawers_") else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let Ok(n) = path[at + 9..].parse::<usize>() else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let compacting = path.contains("compacting_");
    if !(compacting && (n == 2 || n == 3)) && !(!compacting && (n == 1 || n == 2 || n == 4)) {
        return ::core::ops::ControlFlow::Continue(());
    }
    ::core::ops::ControlFlow::Break(PreRemove::Contents {
        slots: n + 7,
        furnace: false,
    })
}

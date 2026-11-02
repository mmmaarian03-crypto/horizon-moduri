// A vanilla block step at a player's feet is not played while Presence Footsteps steps for the players.
fn presencefootsteps_gate(event: &str, at: Option<[f64; 3]>) -> ::core::ops::ControlFlow<()> {
    if crate::audio::presencefootsteps_gate::blocks(event, at) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}

// A press over a container screen: Mouse Tweaks notes the slot and what is on the mouse (it never takes the click).
fn mousetweaks_press(game: &mut crate::game::Game<'_>, button: winit::event::MouseButton) {
    game.mousetweaks_pressed(button);
}

// Hang Glider (PlayerMixin): whether the player whose movement tick runs next on this thread glides.
// The game sets it before the tick (the window's client, a host in the frame) and before each guest's
// body tick on a server; `MotionWorld::update_player_pose` reads it.
use std::cell::Cell;

thread_local! {
    static GLIDING: Cell<bool> = const { Cell::new(false) };
}

pub fn set(gliding: bool) {
    GLIDING.with(|g| g.set(gliding));
}

pub fn get() -> bool {
    GLIDING.with(|g| g.get())
}

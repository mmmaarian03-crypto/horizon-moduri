// Presence Footsteps, the part in the sound library (`horizon_client::audio`): the vanilla block step sounds of
// the players give way to the engine's while it runs (`IEntity.playStepSounds` is cancelled in the original for
// a player with a step generator; the other mobs keep theirs, as the original does without exclusive mode).
// Plain Rust without game types.
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

static ACTIVE: AtomicBool = AtomicBool::new(false);
/// The engine's own sounds pass (its fallback is a vanilla step sound).
static PASS: AtomicBool = AtomicBool::new(false);
/// Where the players the engine steps for are (feet).
static PLAYERS: Mutex<Vec<[f64; 3]>> = Mutex::new(Vec::new());

pub fn set_active(on: bool) {
    ACTIVE.store(on, Ordering::Relaxed);
    if !on && let Ok(mut p) = PLAYERS.lock() {
        p.clear();
    }
}
pub fn pass(on: bool) {
    PASS.store(on, Ordering::Relaxed);
}
pub fn set_players(players: Vec<[f64; 3]>) {
    if let Ok(mut p) = PLAYERS.lock() {
        *p = players;
    }
}

/// A block step sound is `block.<name>.step` (with or without `minecraft:`).
pub fn is_block_step(event: &str) -> bool {
    let name = event.strip_prefix("minecraft:").unwrap_or(event);
    name.starts_with("block.") && name.ends_with(".step")
}

/// Whether `event` at `at` is a vanilla step of one of the players (within a block around their feet).
pub fn blocks(event: &str, at: Option<[f64; 3]>) -> bool {
    if !ACTIVE.load(Ordering::Relaxed) || PASS.load(Ordering::Relaxed) || !is_block_step(event) {
        return false;
    }
    let Some(at) = at else { return false };
    PLAYERS.lock().is_ok_and(|players| {
        players
            .iter()
            .any(|p| (p[0] - at[0]).abs() < 1.0 && (p[2] - at[2]).abs() < 1.0 && (p[1] - at[1]).abs() < 1.5)
    })
}

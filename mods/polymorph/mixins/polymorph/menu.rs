// Polymorph inside the lib crate's menu engine (`horizon_client::menu`): after `Menu::recompute` made the
// crafting result from the first matching recipe, the recipe the player chose (the game side publishes its
// id in a file in the temp dir: the game is a bin over this lib and forbids unsafe, so no env var) replaces it while it still matches the
// grid. Taking the result then gives the chosen recipe's stack. Kept tiny on purpose: the rules (what is
// offered, which one is current) live in the game-side engine.
//
// The file is per process and thread: single player's window (a client predicting its clicks) and its
// integrated server (running the guests' menus) are two threads of one process, each with its own player's
// choice; the server writes the acting guest's choice before each of his clicks.
use super::{Env, Menu, GRID, RESULT};
use crate::crafting::Recipe;

const PICK_FILE: &str = "horizon-polymorph-pick";

/// This thread's file (the game's `polymorph::pick_path` makes the same name).
fn pick_path() -> std::path::PathBuf {
    let thread: String = format!("{:?}", std::thread::current().id()).chars().filter(char::is_ascii_digit).collect();
    std::env::temp_dir().join(format!("{PICK_FILE}-{}-{thread}.txt", std::process::id()))
}

/// True when the chosen recipe's result was set (the original `recompute` must then be skipped).
pub(crate) fn apply(menu: &mut Menu, env: &Env<'_>) -> bool {
    if menu.grid_size == 0 {
        return false;
    }
    let Ok(id) = std::fs::read_to_string(pick_path()) else {
        return false;
    };
    let id = id.trim().to_owned();
    if !env.recipes.recipe_matches(&id, &menu.containers[GRID], menu.grid_size) {
        return false;
    }
    let Some((_, recipe)) = env.recipes.crafting.iter().find(|(n, _)| *n == id) else {
        return false;
    };
    if let Recipe::Shaped { result, .. } | Recipe::Shapeless { result, .. } = recipe {
        menu.containers[RESULT][0] = Some(result.create());
        return true;
    }
    false
}

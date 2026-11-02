// The page's buttons.
fn presencefootsteps_settings(game: &crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    game.pf_settings(mod_id, buttons);
}

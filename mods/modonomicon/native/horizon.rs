// Modonomicon inside Horizon's game (nightly 142388e1c). The rules are the engine below
// (src/lib.rs); this part reads the world through the kit (tools/kit, pasted by tools/hnmod.py bundle).
#[allow(unused_imports)]
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    include!("modonomicon_engine.rs");
}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
#[allow(dead_code)]
const MOD_ID: &str = "modonomicon";

include!("kit_blocks.rs");
include!("kit_notice.rs");
include!("kit_sound.rs");
include!("kit_state.rs");

thread_local! {
    static STATE: std::cell::RefCell<kit_state::PerWorld<engine::State>> =
        const { std::cell::RefCell::new(kit_state::PerWorld::new()) };
}

/// The world as the engine sees it.
struct Live<'a, 'g> {
    game: &'a Game<'g>,
}
impl engine::World for Live<'_, '_> {
    fn block(&self, p: engine::Pos) -> Option<String> {
        kit_blocks::name_at(self.game, p).map(|n| n.to_string())
    }
    fn in_tag(&self, tag: &str, block: &str) -> bool {
        kit_blocks::block_in_tag(self.game, tag, block)
    }
}

impl Game<'_> {
    /// HEAD on `copper_golems::Game::use_golem_statue` (a block's use in `use_hand`, before
    /// placement; not reached when sneaking with an item). `true` = handled, the hand swings.
    pub(super) fn modonomicon_use(&mut self, pos: [i32; 3]) -> bool {
        // The original runs on the server: a remote server's world is its own.
        if self.remote.is_some() {
            return false;
        }
        match engine::on_use(&Live { game: self }, pos) {
            engine::Action::Pass => false,
            engine::Action::Notice(text) => {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            *s.level(self, engine::State::load).uses.entry(pos).or_insert(0) += 1;
            s.dirty(self);
        });
                kit_sound::at_block(self, "minecraft:ui.button.click", pos, 0.3, 1.0);
                kit_notice::action_bar(self, text);
                true
            }
        }
    }

    /// HEAD `save_game_metadata`: the world is saved, so are this mod's side files.
    pub(super) fn modonomicon_save(&mut self) {
        let game: &Game<'_> = self;
        STATE.with(|s| s.borrow_mut().save(game, engine::State::save));
    }
    /// HEAD `load_world_state` (no bindings): a world opens; its state is read again on first use.
    pub(super) fn modonomicon_world_opened() {
        STATE.with(|s| s.borrow_mut().clear());
    }
}

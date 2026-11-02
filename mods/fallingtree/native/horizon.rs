// Falling Tree inside Horizon's game (nightly 83f15fa276c4). The rules are the
// engine below; this part reads the world and breaks blocks the game's way.
use super::{Game, Result};
mod engine {
    include!("fallingtree_engine.rs");
}
use engine::{Kind, Plan, Pos, Settings};

/// The world as the engine sees it.
struct Live<'a, 'g> {
    game: &'a Game<'g>,
}
impl engine::World for Live<'_, '_> {
    fn kind(&self, p: Pos) -> Option<Kind> {
        let g = self.game;
        if p[1] < g.dim.min_y() || p[1] > g.dim.max_y() || !g.world.loaded(p[0], p[2]) {
            return None;
        }
        let state = g.world.block(p[0], p[1], p[2]);
        let name: &str = &g.res.facts().get(state).name;
        let items = g.res.items();
        Some(
            if items.block_in_tag("minecraft:logs", name) || name == "minecraft:mangrove_roots" {
                Kind::Log(name.to_owned())
            } else if items.block_in_tag("minecraft:leaves", name) {
                Kind::Leaves {
                    persistent: g.with_props(state, &[("persistent", "true")]) == Some(state),
                }
            } else if matches!(
                name,
                "minecraft:nether_wart_block" | "minecraft:warped_wart_block" | "minecraft:shroomlight"
            ) {
                Kind::Wart
            } else {
                Kind::Other
            },
        )
    }
}

impl Game<'_> {
    /// Breaking a block: `None` leaves it to the game (not a tree, no axe,
    /// sneaking, creative, a remote server's world); `Some` felled the tree.
    pub(super) fn fallingtree_break(&mut self, pos: Pos) -> Option<Result<()>> {
        if self.remote.is_some() || self.player.is_none() || !self.may_build() || self.creative {
            return None;
        }
        // sneakMode SNEAK_DISABLE: sneaking cuts a single block.
        if self.player.as_ref().is_some_and(|p| p.shift_key_down) {
            return None;
        }
        // ignoreTools false: only axes fell trees.
        let axes = self.res.items().item_tags.get("minecraft:axes");
        let axe = self
            .held()
            .is_some_and(|s| axes.is_some_and(|t| t.contains(&*s.item)));
        if !axe {
            return None;
        }
        let settings = Settings::default();
        match engine::plan(&Live { game: self }, &settings, pos) {
            Plan::NotALog | Plan::NotATree => None,
            Plan::TooBig { found } => {
                // maxSizeAction ABORT: only this block breaks.
                self.fallingtree_notice(format!(
                    "Tree too big ({found}/{} logs)",
                    settings.max_size
                ));
                None
            }
            Plan::Fell { logs } => {
                Some(self.with_player_actor(|game| game.fallingtree_fell(&settings, logs)))
            }
        }
    }

    /// Breaks each log as the player would (drops, tool wear, statistics, hunger),
    /// furthest first, then lets the crown go as decay would.
    fn fallingtree_fell(&mut self, settings: &Settings, logs: Vec<Pos>) -> Result<()> {
        let mut removed = Vec::with_capacity(logs.len());
        for p in logs {
            if !matches!(engine::World::kind(&Live { game: self }, p), Some(Kind::Log(_))) {
                continue;
            }
            let before = self.world.block(p[0], p[1], p[2]);
            self.break_block_as_player(p)?;
            if self.world.block(p[0], p[1], p[2]) != before {
                removed.push(p);
            }
        }
        for p in engine::loose_leaves(&Live { game: self }, settings, &removed) {
            // Level.destroyBlock(pos, true): the leaves' own drops, no tool.
            let _ = self.destroy_block_dropping(p);
        }
        Ok(())
    }

    /// notificationMode ACTION_BAR: a line over the hotbar of the acting player.
    fn fallingtree_notice(&mut self, text: String) {
        if self.lan_acting.is_some() {
            self.guest_packets
                .push(horizon_protocol::packets::play::Clientbound::SystemChat(
                    horizon_protocol::packets::play::SystemChat {
                        content: horizon_protocol::text::literal(&text),
                        overlay: true,
                    },
                ));
        } else {
            self.message = Some((text, std::time::Instant::now()));
        }
    }
}

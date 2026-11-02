// Ecran Incarcare inside Horizon's game (nightly 142388e1c). The layout, text and logo are the engine
// below (src/lib.rs); the progress counters live in horizon_mods::content::ecran_incarcare (mixins/shared.rs).
// Before the UI bundle exists (`Loading.gui` is None) the loading renderer's atlas is one black texel,
// so every quad would come out black: one patch makes it white, then solid rectangles take their colour
// from the vertex colour.
use super::Loading;
use horizon_client::hud::{HudBatch, rgba};
#[allow(dead_code)]
mod engine {
    include!("ecran_incarcare_engine.rs");
}

thread_local! {
    static MEMORY: std::cell::RefCell<(engine::MemoryGate, Option<String>)> =
        const { std::cell::RefCell::new((engine::MemoryGate::new(), None)) };
}

/// The memory line, re-queried about twice a second; a failed query keeps the last text (or none).
fn ecran_incarcare_memory(time: f32) -> Option<String> {
    MEMORY.with(|m| {
        let mut m = m.borrow_mut();
        if m.0.due(time) {
            if let Some((used, total)) = horizon_mods::content::ecran_incarcare::memory_usage() {
                m.1 = engine::format_memory(used, total).or(m.1.take());
            }
        }
        m.1.clone()
    })
}

impl Loading {
    /// HEAD of `startup_menu::Loading::draw_menu`. `true` = the screen was drawn (skip the original,
    /// which draws nothing without the atlas); `false` = the atlas exists, the game draws as before.
    pub(super) fn ecran_incarcare_draw(&mut self, batch: &mut HudBatch) -> bool {
        if self.gui.is_some() {
            return false;
        }
        batch.patches.push(horizon_client::font::Patch {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            rgba: vec![255; 4],
        });
        let snapshot = horizon_mods::content::ecran_incarcare::snapshot();
        let line = snapshot.line();
        let time = self.started.elapsed().as_secs_f32();
        let memory = ecran_incarcare_memory(time);
        let view = engine::View {
            memory: memory.as_deref(),
            line: &line,
            note: snapshot.note(),
            fraction: horizon_mods::content::ecran_incarcare::fraction(),
        };
        for f in engine::scene(batch.width, batch.height, time, &view) {
            let color = rgba((f.color >> 16) as u8, (f.color >> 8) as u8, f.color as u8, 255);
            batch.quad(
                [[f.x, f.y], [f.x + f.w, f.y], [f.x + f.w, f.y + f.h], [f.x, f.y + f.h]],
                [[0.5, 0.5]; 4],
                color,
            );
        }
        true
    }
}

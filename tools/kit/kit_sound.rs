// kit_sound (tools/kit): sounds, level events and particles at a block. Pasted by
// `tools/hnmod.py bundle` in place of `include!("kit_sound.rs");`.
// - `audio.play` is captured and sent to LAN guests too (`Audio::play` vs `play_unshared`).
// - `level_event_at` queues `Level.levelEvent` (2001 = block break particles + sound, 1000s =
//   sounds); everyone around sees/hears it.
// - `particles.add` is this window only (guests do not see it).
#[allow(dead_code)]
mod kit_sound {
    use crate::game::Game;

    fn center(p: [i32; 3]) -> [f64; 3] {
        [
            f64::from(p[0]) + 0.5,
            f64::from(p[1]) + 0.5,
            f64::from(p[2]) + 0.5,
        ]
    }
    /// `level.playSound(null, pos, SoundEvents.X, SoundSource.BLOCKS, volume, pitch)`.
    pub(super) fn at_block(g: &mut Game<'_>, sound: &str, pos: [i32; 3], volume: f32, pitch: f32) {
        g.audio.play(sound, Some(center(pos)), volume, pitch);
    }
    /// A sound at an exact point (entities, items).
    pub(super) fn at(g: &mut Game<'_>, sound: &str, at: [f64; 3], volume: f32, pitch: f32) {
        g.audio.play(sound, Some(at), volume, pitch);
    }
    /// `level.levelEvent(id, pos, data)`; `2001` with the block state id = break effect.
    pub(super) fn level_event(g: &mut Game<'_>, id: i32, pos: [i32; 3], data: i32) {
        g.level_event_at(id, pos, data);
    }
    /// `level.addParticle(type, x, y, z, dx, dy, dz)` for this window only.
    pub(super) fn particle(
        g: &mut Game<'_>,
        ty: horizon_client::particles::ParticleType,
        at: [f64; 3],
        velocity: [f64; 3],
    ) {
        let options = horizon_client::particles::ParticleOptions::simple(ty);
        g.particles.add(&options, at, velocity);
    }
}

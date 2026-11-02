// Drip Sounds: the particle engine's side (module `waterdripsound_particles` of particles.rs). A child of the
// particle module, so it sees the private `Behaviour` of each particle.

/// The marker events the engine pushes into `ParticleEngine.sounds` when a marked drip lands; the game's side
/// (`waterdripsound_after_particles`) takes them out and plays the mod's sound in their place.
pub const WATER: &str = "waterdripsound:drip_water";
pub const LAVA: &str = "waterdripsound:drip_lava";

/// `MixinFallAndLandParticle`: the non-dripstone `FallAndLandParticle`s of water and lava (`FALLING_WATER`,
/// `FALLING_LAVA`) get a landing sound. Their `postMoveUpdate` then pushes the marker when `onGround` (the
/// mixin's `remove()` call); the dripstone drips keep their own sound and are not touched.
pub fn mark(engine: &mut super::ParticleEngine) {
    for p in engine.particles.iter_mut().chain(engine.pending.iter_mut()) {
        let marker = match p.ty {
            super::ParticleType::FallingWater => WATER,
            super::ParticleType::FallingLava => LAVA,
            _ => continue,
        };
        if let super::Behaviour::Drip {
            stage: super::DripStage::FallAndLand,
            sound,
            ..
        } = &mut p.behaviour
            && sound.is_none()
        {
            *sound = Some(marker);
        }
    }
}

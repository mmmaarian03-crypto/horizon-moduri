// Clumps inside Horizon's game (nightly 142388e1c). The rules are the engine below; this part
// hands them the game's experience orbs before they move and does what the game does for a pickup
// (the flying orb, the sound, Mending, the experience points).
use super::{ItemStack, Game, pickups};
use horizon_client::entities::ExperienceOrb;
use horizon_core::random::RandomSource;
use std::cell::RefCell;
#[allow(dead_code)]
mod engine {
    include!("clumps_engine.rs");
}

thread_local! {
    /// The clumps of the orbs of the world this thread plays.
    static CLUMPS: RefCell<engine::Clumps> = RefCell::new(engine::Clumps::default());
}

/// More clumps than this are checked against the orbs that exist (one for every orb that grew
/// past a single orb and then despawned unpicked would otherwise stay).
const PRUNE_ABOVE: usize = 4096;

impl engine::Orb for ExperienceOrb {
    fn id(&self) -> u32 {
        self.id
    }
    fn value(&self) -> u32 {
        self.value
    }
    fn set_value(&mut self, value: u32) {
        self.value = value;
    }
    fn age(&self) -> u32 {
        self.age
    }
    fn set_age(&mut self, age: u32) {
        self.age = age;
    }
    fn ticks(&self) -> u32 {
        self.ticks
    }
    fn count(&self) -> u32 {
        self.count
    }
    fn set_count(&mut self, count: u32) {
        self.count = count;
    }
    fn bounds(&self) -> engine::Box3 {
        let b = ExperienceOrb::bounds(self);
        engine::Box3 {
            min: b.min_position().components(),
            max: b.max_position().components(),
        }
    }
}

/// `repairPlayerItems` finds something to do only with a damaged item that has Mending.
fn damaged_mending(slot: &Option<ItemStack>) -> bool {
    slot.as_ref()
        .is_some_and(|s| s.damage > 0 && s.enchantment_level("minecraft:mending") > 0)
}

impl Game<'_> {
    /// Before `tick_experience` moves the orbs: the original's merges and its pickup. The game's own
    /// merge (`can_merge` is off) and pickup (`xp_delay` is kept above 0) never run on the server's
    /// orbs, so this is the only one.
    pub(super) fn clumps_before_tick(&mut self) {
        // The original runs on the server, where the orbs are: a remote server's are its own.
        if self.remote.is_some() {
            return;
        }
        // Player.aiStep: the pickup box is the player's box inflated by (1, 0.5, 1) (the same box
        // `tick_experience` uses), for a player who is alive and not a spectator.
        let spectator = self.spectator();
        let pickup = self
            .player
            .as_ref()
            .filter(|p| !p.health.dead_or_dying() && !spectator)
            .map(|p| {
                let w = f64::from(p.dimensions.width) / 2.0;
                engine::Box3 {
                    min: [p.position.x - w - 1.0, p.position.y - 0.5, p.position.z - w - 1.0],
                    max: [
                        p.position.x + w + 1.0,
                        p.position.y + f64::from(p.dimensions.height) + 0.5,
                        p.position.z + w + 1.0,
                    ],
                }
            });
        let mut orbs = std::mem::take(&mut self.orbs);
        let taken = CLUMPS.with(|c| {
            let mut c = c.borrow_mut();
            let taken = c.before_tick(&mut orbs, pickup.as_ref());
            if c.len() > PRUNE_ABOVE {
                let alive: std::collections::HashSet<u32> = orbs.iter().map(|o| o.id).collect();
                c.prune(|id| alive.contains(&id));
            }
            taken
        });
        self.orbs = orbs;
        // `tick_experience` counts the delay down first and takes an orb only at 0: it never does.
        // (`player.takeXpDelay = 0` in the original's `playerTouch` means no delay at all.)
        self.xp_delay = 2;
        for t in taken {
            self.clumps_pickup(t);
        }
    }

    /// `MixinExperienceOrb.playerTouch`: the orb is taken (the flying orb and its sound, which the
    /// client plays for every take packet) and each orb it holds goes through Mending; what is left
    /// is given as experience, once.
    fn clumps_pickup(&mut self, taken: engine::Taken<ExperienceOrb>) {
        let engine::Taken { orb, clump } = taken;
        let at = [orb.position.x, orb.position.y, orb.position.z];
        // ClientPacketListener.handleTakeItemEntity.
        let pitch = (self.world.random.next_float() - self.world.random.next_float()) * 0.35 + 0.9;
        self.audio
            .play("minecraft:entity.experience_orb.pickup", Some(at), 0.1, pitch);
        self.add_pickup(pickups::Taken::Orb(Box::new(orb)), pickups::Collector::Player);
        let mending = damaged_mending(&self.inventory[self.selected])
            || damaged_mending(&self.offhand)
            || self.armor.iter().any(damaged_mending);
        // Without a damaged Mending item every `repairPlayerItems` returns what it got.
        let give = if mending {
            engine::payout(&clump, |size| self.mending(size))
        } else {
            engine::payout(&clump, |size| size)
        };
        if give > 0 {
            self.give_orb_points(give);
        }
    }

    /// `ExperienceOrb.tryMergeToExisting`, which `award` calls for every orb it splits an amount
    /// into: true when the amount joined an orb that is already there.
    pub(super) fn clumps_award(&mut self, at: [f64; 3], value: u32) -> bool {
        // The orbs of a remote server are its own; a guest only shows them.
        if self.remote.is_some() {
            return false;
        }
        CLUMPS.with(|c| {
            let mut c = c.borrow_mut();
            c.normalise(&mut self.orbs);
            c.award(&mut self.orbs, at, value)
        })
    }
}

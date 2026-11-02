// Tiny Item Animations for Horizon: the animation rules, without any game code.
// Written from the behaviour of Tiny Item Animations 1.1 for Forge 1.20 (Trivaxy; the build in
// the pack is `tia-1.20-1.1-forge.jar`), read from the mod's mixin classes and `tia.toml` of the
// pack; no code of the original is used. The source's licence (LPUL) only restricts hosting it on
// public platforms; nothing of it is published here.
use std::collections::BTreeMap;

/// The options of `tia.toml`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// `animation_speed` (0.01 to 10): how much of an animation one frame plays, per unit of the
    /// frame's partial tick.
    pub animation_speed: f32,
    /// `pickup_scale` (0.01 to 10): the size an item has when it is carried, or as it is put
    /// down.
    pub pickup_scale: f32,
}
impl Default for Settings {
    /// the pack's `config/tia.toml`.
    fn default() -> Self {
        Self {
            animation_speed: 0.5,
            pickup_scale: 1.4,
        }
    }
}

/// `1 + (pickup_scale - 1) * (1 - (1 - progress)^5)`: how large an item is drawn at `progress`
/// (0 to 1). It leaps toward the full size at first and settles slowly: at 0.5 it has gone 97% of
/// the way. Zero is the normal size, one is `pickup_scale`.
pub fn eased_scale(pickup_scale: f32, progress: f32) -> f32 {
    let left = f64::from(1.0 - progress);
    1.0 + (pickup_scale - 1.0) * (1.0 - left.powf(5.0) as f32)
}

/// One slot's animation: what it has left, and the first frame whose step it has not had yet.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Running {
    progress: f32,
    next_step: u64,
}

/// What the mod remembers: the animation of every slot of the open menu, and of the carried item.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tia {
    pub settings: Settings,
    /// `Animated.animationProgress` of the slots that have some left, 1 down to 0.
    slots: BTreeMap<usize, Running>,
    /// `TiaMod.carriedAnimationProgress`, 0 up to 1.
    carried: f32,
    /// The partial tick of the frame before the one that began last.
    last_partial: f32,
}
impl Tia {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            ..Self::default()
        }
    }

    /// `AbstractContainerMenu.doClick` after `tryItemClickBehaviourOverride`: any plain
    /// left or right click (`PICKUP`, button 0 or 1) on a slot (not outside the window) starts the
    /// slot's animation from the top and the carried item's from the start. Quick moves, swaps,
    /// throws, drags and double clicks do not. `frame` is the next frame to be drawn.
    pub fn click(&mut self, slot: Option<usize>, button: i32, pickup: bool, frame: u64) {
        if pickup && (button == 0 || button == 1) && let Some(slot) = slot {
            self.slots.insert(
                slot,
                Running {
                    progress: 1.0,
                    next_step: frame,
                },
            );
            self.carried = 0.0;
        }
    }

    /// A frame of a menu begins. The original steps every slot after drawing it, empty slots
    /// included; a slot nothing was drawn for (it holds nothing) takes the step of the frame before
    /// this one now, with that frame's partial tick. `partial` is this frame's.
    pub fn begin_frame(&mut self, frame: u64, partial: f32) {
        let step = self.last_partial * self.settings.animation_speed;
        self.slots.retain(|_, s| {
            if s.next_step < frame {
                s.progress = (s.progress - step).max(0.0);
                s.next_step = frame;
            }
            s.progress > 0.0
        });
        self.last_partial = partial;
    }

    /// A slot's item is drawn (`renderSlot`): the size of the item now, from the progress the slot
    /// has, and then one step of its animation (`postRenderSlot`), by `partial` (the frame's
    /// partial tick) times the speed. Slots with no animation draw at the normal size.
    pub fn draw_slot(&mut self, slot: usize, partial: f32, frame: u64) -> f32 {
        let Some(running) = self.slots.get_mut(&slot) else {
            return 1.0;
        };
        let scale = eased_scale(self.settings.pickup_scale, running.progress);
        if running.next_step <= frame {
            running.progress = (running.progress - partial * self.settings.animation_speed).max(0.0);
            running.next_step = frame + 1;
        }
        if running.progress <= 0.0 {
            self.slots.remove(&slot);
        }
        scale
    }

    /// The carried item is drawn (`preRenderFloatingItem`): one step of its animation first, up to
    /// one, then the size it is drawn at.
    pub fn draw_carried(&mut self, partial: f32) -> f32 {
        self.carried = (self.carried + partial * self.settings.animation_speed).min(1.0);
        eased_scale(self.settings.pickup_scale, self.carried)
    }

    /// The screen closed: the next menu has new slots.
    pub fn closed(&mut self) {
        self.slots.clear();
        self.last_partial = 0.0;
    }

    /// The progress a slot has left, for tests.
    pub fn slot_progress(&self, slot: usize) -> f32 {
        self.slots.get(&slot).map_or(0.0, |s| s.progress)
    }
    /// The carried item's progress, for tests.
    pub fn carried_progress(&self) -> f32 {
        self.carried
    }
}

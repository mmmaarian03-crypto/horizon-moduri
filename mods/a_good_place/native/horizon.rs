// A Good Place inside Horizon's game (nightly 142388e1c136). The rules are the engine below (the animation pack,
// the pose, the particles in flight); this part starts an animation when the player places a block, ticks them,
// draws the animated block and hides the placed block from the chunk meshes meanwhile (the `a_good_place_hidden`
// module of the mesher, see mixins/a_good_place/mesher.rs).
use super::{Game, ItemStack};
use horizon_client::mesher::Vertex;
use horizon_core::geometry::Axis;
use horizon_world::registry::BlockStateId;
use std::cell::RefCell;

#[allow(dead_code)]
mod engine {
    include!("a_good_place_engine.rs");
}

pub const ID: &str = "a_good_place";
pub const TITLE: &str = "A Good Place 1.2.8";
pub const DESCRIPTION: &str = "Blocks you place slide, tilt and scale into position (the original's default animation pack).";

struct State {
    pack: Vec<engine::Params>,
    manager: engine::Manager,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        pack: engine::default_pack(),
        manager: engine::Manager::default(),
    });
}

/// `RenderShape` that is not MODEL in 1.20.1 and gets its own renderer: the original neither hides nor draws these
/// itself (it only moves their block entity renderer, which Horizon cannot do), so they are left alone here.
fn special_render(name: &str) -> bool {
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    n.ends_with("chest")
        || n.ends_with("_bed")
        || n.ends_with("_banner")
        || n.ends_with("_sign")
        || n.ends_with("_skull")
        || n.ends_with("_head")
        || n.ends_with("shulker_box")
        || matches!(
            n,
            "conduit" | "bell" | "enchanting_table" | "end_portal" | "end_gateway" | "decorated_pot" | "lectern"
                | "player_head" | "player_wall_head" | "dragon_head" | "dragon_wall_head" | "piglin_head"
                | "piglin_wall_head" | "skeleton_skull" | "wither_skeleton_skull" | "zombie_head"
                | "creeper_head" | "sculk_sensor" | "sculk_shrieker" | "sculk_catalyst"
        )
}

/// What the predicates and the direction fix ask of the level at one placement.
struct View<'g, 'a> {
    game: &'g Game<'a>,
    pos: [i32; 3],
    state: BlockStateId,
}
impl View<'_, '_> {
    fn shape(&self, pos: [i32; 3]) -> Option<horizon_core::geometry::VoxelShape> {
        let state = self.game.world.block(pos[0], pos[1], pos[2]);
        let mut scene = horizon_client::collisions::SceneCollisions {
            world: &self.game.world,
            binding: self.game.collision,
            subject: None,
        };
        self.game.collision.collision_shape(&mut scene, state, pos).ok()
    }
    fn name(&self) -> std::sync::Arc<str> {
        self.game.res.facts().get(self.state).name.clone()
    }
    fn property(&self, key: &str) -> Option<String> {
        self.game
            .pack
            .assets
            .registry
            .state(self.state)?
            .properties
            .get(key)
            .cloned()
    }
}
impl engine::StateView for View<'_, '_> {
    fn is_double_block(&self) -> bool {
        let name = self.name();
        let n = name.strip_prefix("minecraft:").unwrap_or(&name);
        // BedBlock, DoorBlock, DoublePlantBlock (the lower/upper `half`) and a chest that is not single.
        if n.ends_with("_bed") || n.ends_with("_door") {
            return true;
        }
        if matches!(self.property("half").as_deref(), Some("upper" | "lower")) {
            return true;
        }
        if n == "chest" || n == "trapped_chest" {
            return self.property("type").is_some_and(|t| t != "single");
        }
        false
    }
    fn has_collision(&self) -> bool {
        self.shape(self.pos).is_some_and(|s| !s.is_empty())
    }
    fn is_block(&self, id: &str) -> bool {
        *self.name() == *id
    }
    fn in_tag(&self, tag: &str) -> bool {
        let tag = tag.strip_prefix("minecraft:").map_or(tag.to_owned(), str::to_owned);
        let name = self.name();
        let tags = &self.game.res.items().block_tags;
        [format!("minecraft:{tag}"), tag].iter().any(|t| tags.get(t).is_some_and(|s| s.contains(&*name)))
    }
}
impl engine::Around for View<'_, '_> {
    fn extent(&self, dir: engine::Dir) -> Option<[f64; 2]> {
        let o = dir.offset();
        let shape = self.shape([self.pos[0] + o[0], self.pos[1] + o[1], self.pos[2] + o[2]])?;
        let bounds = shape.bounds()?;
        let axis = [Axis::X, Axis::Y, Axis::Z][dir.axis()];
        Some([bounds.min(axis), bounds.max(axis)])
    }
}

impl Game<'_> {
    /// `BlockItem.place` of the player, after the block is in the level: the animation starts
    /// (`BlocksParticlesManager.addParticle`).
    pub(super) fn a_good_place_placed(&mut self, pos: [i32; 3], _state: BlockStateId, _stack: &ItemStack) {
        if !self.native_on(ID) || !self.native_window() {
            return;
        }
        let Some(p) = self.player.as_ref() else { return };
        let state = self.world.block(pos[0], pos[1], pos[2]);
        if self.world.is_air(state) {
            return;
        }
        let name = self.res.facts().get(state).name.clone();
        // Blocks with their own renderer are not animated (see `special_render`).
        if special_render(&name) {
            return;
        }
        // `camera.getPosition().distanceToSqr(pos.getCenter()) <= 1024`.
        let eye = [p.position.x, p.position.y + f64::from(p.dimensions.eye_height), p.position.z];
        let d: f64 = (0..3).map(|i| (eye[i] - (f64::from(pos[i]) + 0.5)).powi(2)).sum();
        if d > engine::MAX_DISTANCE_SQ {
            return;
        }
        let view = View { game: self, pos, state };
        let (yaw, pitch) = (self.yaw, self.pitch);
        let right_arm = !self.settings.main_hand_left;
        let pose = STATE.with(|s| {
            let s = s.borrow();
            engine::select(&s.pack, &view).map(|params| {
                let placer = engine::Placer { yaw, pitch, right_arm, main_hand: true };
                engine::Pose::new(params, &placer, &view)
            })
        });
        let Some(pose) = pose else { return };
        let effects = STATE.with(|s| s.borrow_mut().manager.add(pos, state.0, pose, true, true));
        self.a_good_place_apply(effects);
    }

    /// The meshes of the positions the manager hid or showed again are rebuilt.
    fn a_good_place_apply(&mut self, effects: Vec<engine::Effect>) {
        for effect in effects {
            let pos = match effect {
                engine::Effect::Hide(pos) => {
                    horizon_client::mesher::a_good_place_hidden::hide(pos);
                    pos
                }
                engine::Effect::Unhide { pos, was_hidden } => {
                    if !was_hidden {
                        continue;
                    }
                    horizon_client::mesher::a_good_place_hidden::unhide(pos);
                    pos
                }
            };
            for dz in -1..=1 {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let (x, y, z) = (pos[0] + dx, pos[1] + dy, pos[2] + dz);
                        let s = (y - horizon_client::chunk::MIN_Y) >> 4;
                        if (0..horizon_client::chunk::SECTIONS as i32).contains(&s) {
                            self.world.dirty_sections.insert((x >> 4, s, z >> 4));
                        }
                    }
                }
            }
        }
    }

    /// One client tick of the particle engine.
    pub(super) fn a_good_place_tick(&mut self) {
        if self.player.is_none() || !self.native_on(ID) {
            let any = STATE.with(|s| !s.borrow().manager.particles().is_empty());
            if any {
                STATE.with(|s| s.borrow_mut().manager.clear());
                horizon_client::mesher::a_good_place_hidden::clear();
            }
            return;
        }
        let world = &self.world;
        let effects = STATE.with(|s| {
            s.borrow_mut().manager.tick(|p| world.block(p[0], p[1], p[2]).0)
        });
        self.a_good_place_apply(effects);
    }

    /// `PlacingBlockParticle.render`: the animated blocks as entity geometry (camera-relative vertices).
    pub(super) fn a_good_place_geometry(&self, camera: glam::DVec3, alpha: f64, out: &mut Vec<Vertex>) {
        if !self.native_on(ID) {
            return;
        }
        let drawn: Vec<(engine::Pos, u32, glam::Mat4)> = STATE.with(|s| {
            let s = s.borrow();
            s.manager
                .particles()
                .iter()
                .filter(|p| p.can_render && !p.removed)
                .map(|p| {
                    let t = p.time(alpha as f32);
                    let offset = self
                        .pack
                        .context
                        .tables
                        .offset(BlockStateId(p.state), p.pos[0], p.pos[2])
                        .unwrap_or([0.0; 3]);
                    (p.pos, p.state, glam::Mat4::from_cols_array(&p.pose.matrix(t, offset)))
                })
                .collect()
        });
        for (pos, state, m) in drawn {
            let state = BlockStateId(state);
            let at = glam::Vec3::new(
                (f64::from(pos[0]) - camera.x) as f32,
                (f64::from(pos[1]) - camera.y) as f32,
                (f64::from(pos[2]) - camera.z) as f32,
            );
            // The light around the block (its own cell is solid).
            let light = [[0, 0, 0], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]
                .iter()
                .map(|o| self.world.light(pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]))
                .fold((0u8, 0u8), |(sky, block), l| (sky.max(l >> 4), block.max(l & 15)));
            let packed = (u32::from(light.0) * 16) | ((u32::from(light.1) * 16) << 8);
            let tint = self.particle_sprite(state, pos).map(|(_, tint)| tint);
            let start = out.len();
            self.block_model_vertices(
                state,
                horizon_core::random::position_seed(horizon_core::position::BlockPos::new(pos[0], pos[1], pos[2])),
                glam::Mat4::from_translation(at) * m,
                packed,
                false,
                out,
            );
            if let Some(tint) = tint {
                let tint = horizon_client::mobs::render::abgr(tint);
                for vertex in &mut out[start..] {
                    vertex.color = horizon_client::hud_layout::multiply(vertex.color, tint);
                }
            }
        }
    }
}

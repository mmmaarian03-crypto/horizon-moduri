// Neat inside Horizon's game (nightly 142388e1c136). The rules and the plan of a plate are the engine
// below; this part lists the living entities near the player, answers the questions the rules ask about
// them, and puts the plate's quads, texts and icons into the world facing the camera, as Horizon puts
// its own name tags: into the entity vertices, depth tested.
use super::{Game, ItemStack, view_vector};
use horizon_assets::item_model::DisplayContext;
use horizon_client::{
    block_entity_render::vertex_light, mesher::Vertex, scoreboard as sb,
};
use std::cell::RefCell;

#[allow(dead_code)]
mod engine {
    include!("neat_engine.rs");
}

pub const ID: &str = "neat";
pub const TITLE: &str = "Neat 1.20.1-41";
pub const DESCRIPTION: &str = "A health plate above each mob: its name, a bar coloured by its health, the numbers, an icon for its kind and its armour. The original's defaults and the pack's file.";

struct State {
    config: Option<engine::Config>,
    /// `NeatConfig.draw`: the toggle key.
    drawing: bool,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State { config: None, drawing: true });
}

/// A plate vertex's light: `0xF000F0`, full brightness.
const LIGHT: u8 = 0xFF;

impl Game<'_> {
    /// `neat-client.toml`, read once (the pack's values unless a file in the mod's folder says otherwise).
    fn neat_config(&self) -> engine::Config {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.config
                .get_or_insert_with(|| engine::Config::read(|key| self.native_setting(ID, key)))
                .clone()
        })
    }

    /// The toggle key (`neat.keybind.toggle`, unbound by default).
    pub(super) fn neat_key(&self, key: &str, pressed: bool) {
        if !pressed || !self.native_on(ID) || !self.native_playing() {
            return;
        }
        if key == self.native_mod_key(ID, "toggle", "key.keyboard.unknown") {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.drawing = !s.drawing;
            });
        }
    }

    /// The mob under the crosshair, up to 32 blocks and in front of the first block (`getEntityLookedAt`).
    fn neat_target(&self) -> Option<usize> {
        let eye = self.eye_position()?;
        let dir = view_vector(self.yaw, self.pitch);
        let mut best: Option<(usize, f64)> = None;
        for (i, m) in self.mobs.iter().enumerate() {
            if !m.alive() || Some(m.id) == self.riding {
                continue;
            }
            if let Some(t) = super::ray_box(eye, dir, 32.0, m.bounds())
                && best.is_none_or(|b| t < b.1)
            {
                best = Some((i, t));
            }
        }
        let (i, distance) = best?;
        self.before_block(distance).then_some(i)
    }

    /// The plates of the living entities near the player, into the entity vertices of the frame.
    pub(super) fn neat_plates(&self, camera: glam::DVec3, alpha: f64, out: &mut Vec<Vertex>) {
        if !self.native_on(ID) || !self.native_window() {
            return;
        }
        let Some(player) = self.player.as_ref() else {
            return;
        };
        let Some(eye) = self.eye_position() else {
            return;
        };
        let cfg = self.neat_config();
        let env = engine::Env {
            render_names: !self.ui.hide_gui,
            drawing: STATE.with(|s| s.borrow().drawing),
            debug: self.debug,
        };
        if !env.drawing || (!cfg.render_without_gui && !env.render_names) {
            return;
        }
        let atlas = &self.gui.atlas;
        let text_width = |text: &str| atlas.font.width(text);
        let viewer_team = self.player_team();
        let player_names: std::collections::HashMap<u32, String> =
            self.remote_player_names().into_iter().collect();
        let target = if cfg.only_health_bar_for_target {
            self.neat_target()
        } else {
            None
        };
        // The world the line of sight is asked in: built when the first mob needs it.
        let mut world = None;
        let basis = self.neat_basis();
        for (index, m) in self.mobs.iter().enumerate() {
            if !m.alive() || m.removed {
                continue;
            }
            // living == cameraEntity never happens (the player is not in the list), and a mount that
            // carries the camera has no plate (hasPassenger).
            let carries_camera = Some(m.id) == self.riding;
            let distance = {
                let (dx, dy, dz) = (
                    m.position.x - player.position.x,
                    m.position.y - player.position.y,
                    m.position.z - player.position.z,
                );
                ((dx * dx + dy * dy + dz * dz) as f32).sqrt()
            };
            if distance > cfg.max_distance as f32 {
                continue;
            }
            let id = m.kind.id.clone();
            let is_player = id == "minecraft:player";
            let own_team = if is_player {
                player_names
                    .get(&m.id)
                    .and_then(|name| self.score.board.players_team(name))
            } else {
                self.mob_team(m.id)
            };
            let invisible = sb::invisible_to(
                own_team,
                viewer_team,
                self.spectator(),
                m.invisible || m.has_effect("minecraft:invisibility"),
            );
            // An invisible mob still shows what it wears or holds.
            let wearing = [
                &m.extra.main_hand,
                &m.extra.off_hand,
                &m.extra.head,
                &m.extra.chest,
                &m.extra.legs,
                &m.extra.feet,
            ]
            .iter()
            .any(|slot| slot.is_some());
            let visible = !(invisible && !wearing);
            // The team's name tag rules decide for a mob on a team.
            let visible = sb::team_shows_name(own_team, viewer_team, visible).unwrap_or(visible);
            let line_of_sight = distance <= cfg.max_distance_without_line_of_sight as f32 || {
                let world = world.get_or_insert_with(|| self.mob_world_base(&[]));
                let from = horizon_core::geometry::Vec3::new(m.position.x, m.eye_y(), m.position.z);
                let to = horizon_core::geometry::Vec3::new(eye[0], eye[1], eye[2]);
                world.line_of_sight(from, to)
            };
            let name = self.neat_name(m, &id, player_names.get(&m.id));
            let armor = (m.kind.armor + m.worn_armor().0).floor() as i32;
            let subject = engine::Subject {
                player: is_player,
                friendly: engine::is_friendly(&m.kind.category),
                monster: m.kind.category == "monster",
                health: m.health,
                max_health: m.attrs.max_health,
                armor,
                custom_name: m.custom_name.is_some(),
                name,
                distance,
                line_of_sight,
                looked_at: target == Some(index),
                carries_camera,
                visible,
                id,
            };
            if !engine::should_show(&cfg, env, &subject) {
                continue;
            }
            let plate = engine::plate(&cfg, env, &subject, &text_width);
            // The plate hangs over the mob's head, at its height and 0.6 above (`height_above_mob`).
            let at = glam::DVec3::new(
                m.previous.x + (m.position.x - m.previous.x) * alpha,
                m.previous.y + (m.position.y - m.previous.y) * alpha,
                m.previous.z + (m.position.z - m.previous.z) * alpha,
            ) - camera
                + glam::DVec3::new(0.0, f64::from(m.height()) + cfg.height_above_mob, 0.0);
            self.mark_entity(out.len(), "");
            self.neat_emit(&plate, at, distance, &basis, out);
        }
    }

    /// `Camera.rotation()` as the billboard's axes: right, up and forward (turned round in the third
    /// person front view, as Horizon's name tags do).
    fn neat_basis(&self) -> (glam::DVec3, glam::DVec3, glam::DVec3) {
        let (yaw, pitch) = if self.camera_type == super::perspective::CameraType::ThirdPersonFront {
            (self.yaw + 180.0, -self.pitch)
        } else {
            (self.yaw, self.pitch)
        };
        let forward = glam::DVec3::from_array(view_vector(yaw, pitch));
        let right = forward.cross(glam::DVec3::Y).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        (right, up, forward)
    }

    /// `living.hasCustomName() ? customName : getDisplayName()`.
    fn neat_name(&self, m: &horizon_client::mobs::Mob, id: &str, player: Option<&String>) -> String {
        if let Some(name) = &m.custom_name {
            return name.clone();
        }
        if let Some(name) = player {
            return name.clone();
        }
        let path = id.trim_start_matches("minecraft:");
        let fallback = path
            .split('_')
            .map(|w| {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().chain(c).collect::<String>())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ");
        self.translate(&format!("entity.minecraft.{path}"), &fallback)
    }

    /// One plate's quads, texts and icons as world vertices. `at` is the plate's origin from the camera.
    fn neat_emit(
        &self,
        plate: &engine::Plate,
        at: glam::DVec3,
        distance: f32,
        basis: &(glam::DVec3, glam::DVec3, glam::DVec3),
        out: &mut Vec<Vertex>,
    ) {
        let (right, up, forward) = *basis;
        let scale = f64::from(engine::GLOBAL_SCALE);
        // Each layer lies this much further from the camera than the one in front of it, so that the
        // depth test keeps the order the original's quads have.
        let step = 0.002 * (1.0 + f64::from(distance) / 16.0);
        let atlas = &self.gui.atlas;
        let white = atlas.font.white();
        let light = vertex_light(LIGHT);
        let color = |rgb: u32, a: u8| {
            u32::from(a) << 24 | ((rgb & 0xFF) << 16) | (rgb & 0xFF00) | ((rgb >> 16) & 0xFF)
        };
        // Four corners in plate units (bottom left, bottom right, top right, top left), a texture
        // rectangle and a layer.
        let mut quad = |c: [(f32, f32); 4], uv: [f32; 4], rgba: u32, depth: u8| {
            let uvs = [[uv[0], uv[3]], [uv[2], uv[3]], [uv[2], uv[1]], [uv[0], uv[1]]];
            let back = forward * (step * f64::from(depth));
            for ((x, y), uv) in c.into_iter().zip(uvs) {
                let p = at + right * (f64::from(x) * scale) - up * (f64::from(y) * scale) + back;
                out.push(Vertex {
                    normal: [0; 4],
                    pos: p.as_vec3().to_array(),
                    uv,
                    layer: 0,
                    color: rgba,
                    light,
                    glint_uv: [0.0, 0.0],
                });
            }
        };
        let solid = [white.u0, white.v0, white.u1, white.v1];
        for q in &plate.quads {
            let c = [(q.x0, q.y1), (q.x1, q.y1), (q.x1, q.y0), (q.x0, q.y0)];
            let rgba = color(
                u32::from(q.rgba[0]) << 16 | u32::from(q.rgba[1]) << 8 | u32::from(q.rgba[2]),
                q.rgba[3],
            );
            quad(c, solid, rgba, q.depth);
        }
        for t in &plate.texts {
            let rgba = color(t.rgb, 0xFF);
            let mut cursor = 0.0_f32;
            for ch in t.text.chars() {
                let Some((rect, advance)) = atlas.glyph(ch) else {
                    cursor += 6.0 * t.scale;
                    continue;
                };
                // A bold glyph is drawn twice, one font pixel apart, and is one pixel wider.
                let passes: &[f32] = if t.bold { &[0.0, 1.0] } else { &[0.0] };
                if ch != ' ' {
                    for extra in passes {
                        let x0 = t.x + cursor + extra * t.scale;
                        let (x1, y0, y1) = (x0 + 8.0 * t.scale, t.y, t.y + 8.0 * t.scale);
                        // Italic: the top leans right by a pixel, the bottom left by a pixel.
                        let (top, bottom) = if t.italic { (t.scale, -t.scale) } else { (0.0, 0.0) };
                        let c = [
                            (x0 + bottom, y1),
                            (x1 + bottom, y1),
                            (x1 + top, y0),
                            (x0 + top, y0),
                        ];
                        quad(c, [rect.u0, rect.v0, rect.u1, rect.v1], rgba, 0);
                    }
                }
                cursor += (advance + if t.bold { 1.0 } else { 0.0 }) * t.scale;
            }
        }
        // The icons: item models, flat, not scaled with the plate. `Camera.rotation()` takes the local x to
        // the camera's left, y up and z forward (which is why the original negates x and the plate's own
        // scale is `(-s, -s, s)`), so the icons' offsets are along those.
        let left = -right;
        for icon in &plate.icons {
            let stack = ItemStack::new(icon.item, 1);
            let state = self.resolve_item(&stack, DisplayContext::None, None, 0);
            if state.is_empty() {
                continue;
            }
            let origin = at
                + left * f64::from(icon.offset[0])
                + up * f64::from(icon.offset[1])
                + forward * f64::from(icon.offset[2]);
            let basis = glam::Mat4::from_cols(
                left.as_vec3().extend(0.0),
                up.as_vec3().extend(0.0),
                forward.as_vec3().extend(0.0),
                origin.as_vec3().extend(1.0),
            );
            // translate, scale 0.12, then Axis.YP.rotationDegrees(180).
            let pose = basis
                * glam::Mat4::from_scale(glam::Vec3::splat(engine::ICON_SCALE))
                * glam::Mat4::from_rotation_y(std::f32::consts::PI);
            self.submit_item_world(&state, pose, light, out);
        }
    }
}

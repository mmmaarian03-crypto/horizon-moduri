// Forja's shapes, the part the library crate reads (module `horizon_client::mesher::forja_shapes`):
// - the chunk mesher (worker threads) draws the parts of compound blocks (multipart hosts) at
//   their host's cell with the parts' own block models and the game's lighting and culling;
// - the collision scan (`SceneCollisions::context_shape`) gets a host's union of part boxes and
//   the model shapes of content-mod blocks (shapes from models);
// - the outline renderer (`gfx::outline_vertices`) draws the targeted part's boxes, or the model
//   boxes of a content-mod block, instead of the template's.
// The game (forja's runtime, `crate::game::forja`) writes these tables; they are read-mostly.
use horizon_core::geometry::{Aabb, Direction, ShapeOp, Vec3, VoxelShape};
use horizon_core::position::BlockPos;
use horizon_world::registry::BlockStateId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};

/// A slot as the library keeps it: 0..6 = the faces in `Direction` order, 6 = the centre.
pub type SlotCode = u8;
pub const CENTER: SlotCode = 6;

struct Host {
    /// The block states drawing its parts.
    states: Vec<u32>,
    /// Outline boxes (block units, relative to the host) with their part's slot.
    outline: Vec<(SlotCode, [f64; 6])>,
    collision: VoxelShape,
}
#[derive(Default)]
struct Tables {
    hosts: HashMap<[i32; 3], Host>,
    /// Hosts by section (chunk x, section index from the bottom, chunk z), for the mesher.
    sections: HashMap<(i32, i32, i32), Vec<[i32; 3]>>,
    /// By block state id: the state of a host block.
    host_states: Vec<bool>,
    /// By block state id: a content-mod block's model shape (outline, collision if it follows).
    models: Vec<Option<(Vec<[f64; 6]>, Option<VoxelShape>)>>,
}
static TABLES: RwLock<Option<Tables>> = RwLock::new(None);
/// Something is in the tables: everything else costs one atomic read.
static ANY: AtomicBool = AtomicBool::new(false);
/// The outline to draw this frame: the targeted cell and its boxes (world space).
static PICKED: Mutex<Option<([i32; 3], Vec<[f64; 6]>)>> = Mutex::new(None);

fn write(f: impl FnOnce(&mut Tables)) {
    let mut g = TABLES.write().unwrap_or_else(|e| e.into_inner());
    let t = g.get_or_insert_with(Tables::default);
    f(t);
    let any = !t.hosts.is_empty() || t.models.iter().any(Option::is_some);
    ANY.store(any, Ordering::Release);
}
fn read<R>(f: impl FnOnce(&Tables) -> R) -> Option<R> {
    if !ANY.load(Ordering::Acquire) {
        return None;
    }
    let g = TABLES.read().unwrap_or_else(|e| e.into_inner());
    g.as_ref().map(f)
}
fn section(p: [i32; 3]) -> (i32, i32, i32) {
    (p[0] >> 4, (p[1] - super::MIN_Y) >> 4, p[2] >> 4)
}

/// `Shapes.or` of block-unit boxes (empty boxes skipped).
pub fn shape_of(boxes: &[[f64; 6]]) -> VoxelShape {
    let mut out = VoxelShape::empty();
    for b in boxes {
        if (0..3).any(|k| b[k + 3] - b[k] <= 1e-7) {
            continue;
        }
        let aabb = Aabb::new(Vec3::new(b[0], b[1], b[2]), Vec3::new(b[3], b[4], b[5]));
        if let Ok(s) = VoxelShape::from_box(aabb) {
            out = if out.is_empty() { s } else { out.join(&s, ShapeOp::OR).unwrap_or(out) };
        }
    }
    out
}

/// A host's parts as the library draws and collides them (replaces what was there).
pub fn set_host(pos: [i32; 3], states: Vec<u32>, outline: Vec<(SlotCode, [f64; 6])>, collision: &[[f64; 6]]) {
    let host = Host { states, outline, collision: shape_of(collision) };
    write(|t| {
        if t.hosts.insert(pos, host).is_none() {
            t.sections.entry(section(pos)).or_default().push(pos);
        }
    });
}
pub fn remove_host(pos: [i32; 3]) {
    write(|t| {
        if t.hosts.remove(&pos).is_some()
            && let Some(v) = t.sections.get_mut(&section(pos))
        {
            v.retain(|p| *p != pos);
        }
    });
}
/// Another world or dimension: no hosts until the game tells again.
pub fn clear_hosts() {
    write(|t| {
        t.hosts.clear();
        t.sections.clear();
    });
}
/// The states of the host blocks (registry length `len`).
pub fn set_host_states(ids: &[u32], len: usize) {
    write(|t| {
        t.host_states = vec![false; len];
        for &i in ids {
            if let Some(s) = t.host_states.get_mut(i as usize) {
                *s = true;
            }
        }
    });
}
/// The model shapes of content-mod block states: (state, outline boxes, collision follows).
pub fn set_models(list: Vec<(u32, Vec<[f64; 6]>, bool)>, len: usize) {
    let mut models: Vec<Option<(Vec<[f64; 6]>, Option<VoxelShape>)>> = Vec::new();
    models.resize_with(len, || None);
    for (id, boxes, collide) in list {
        if let Some(m) = models.get_mut(id as usize) {
            let shape = collide.then(|| shape_of(&boxes));
            *m = Some((boxes, shape));
        }
    }
    write(|t| t.models = models);
}
/// A host's outline boxes with their slots (block units, relative), if `state` is a host block's
/// and the position holds a host.
pub fn host_outline(state: u32, pos: [i32; 3]) -> Option<Vec<(SlotCode, [f64; 6])>> {
    read(|t| {
        if !t.host_states.get(state as usize).copied().unwrap_or(false) {
            return None;
        }
        t.hosts.get(&pos).map(|h| h.outline.clone())
    })
    .flatten()
}
/// The model outline of a content-mod block state (shapes from models).
pub fn model_outline(state: u32) -> Option<Vec<[f64; 6]>> {
    read(|t| t.models.get(state as usize).and_then(|m| m.as_ref()).map(|m| m.0.clone())).flatten()
}
/// What entities collide with at `p` (block units, relative): a host's parts, or a content-mod
/// block's model; `None` = the template's own rule.
pub fn collision(state: u32, p: [i32; 3]) -> Option<VoxelShape> {
    read(|t| {
        if t.host_states.get(state as usize).copied().unwrap_or(false) {
            if let Some(h) = t.hosts.get(&p) {
                return Some(h.collision.clone());
            }
        }
        t.models.get(state as usize)?.as_ref()?.1.clone()
    })
    .flatten()
}
pub fn any() -> bool {
    ANY.load(Ordering::Acquire)
}
/// The game's crosshair: the cell and boxes (world space) to outline, or `None` for the
/// template's outline.
pub fn set_picked(picked: Option<([i32; 3], Vec<[f64; 6]>)>) {
    let mut g = PICKED.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_some() || picked.is_some() {
        *g = picked;
    }
}
/// `outline_vertices` for the frame's outline: when it is the template's outline of the cell the
/// crosshair holds (the edges lie in that cell), the edges of the picked boxes instead.
pub fn outline(edges: &[[f64; 6]], camera: glam::DVec3) -> Option<Vec<[f32; 6]>> {
    let picked = PICKED.lock().unwrap_or_else(|e| e.into_inner()).clone()?;
    let (pos, boxes) = picked;
    let inside = edges.iter().all(|e| {
        (0..6).all(|i| {
            let v = e[i] - f64::from(pos[i % 3]);
            (-0.5..=1.5).contains(&v)
        })
    });
    if !inside || boxes.is_empty() {
        return None;
    }
    let c = camera.to_array();
    let mut out = Vec::with_capacity(boxes.len() * 12);
    for b in &boxes {
        for e in crate::blocks::cube_edges() {
            out.push(std::array::from_fn(|i| {
                let k = i % 3;
                (b[k] + e[i] * (b[k + 3] - b[k]) - c[k]) as f32
            }));
        }
    }
    // The renderer draws at most 256 edges (gfx MAX_OUTLINE_EDGES).
    out.truncate(256);
    Some(out)
}

thread_local! {
    /// Inside this hook's own call of `Mesher::mesh`.
    static INNER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/// The `Mesher::mesh` hook: a section holding hosts is meshed as usual, then their parts are
/// added (`None` = nothing to add, the original runs).
pub fn mesh(
    this: &mut super::Mesher<'_>,
    region: &[[Option<&super::Chunk>; 3]; 3],
    sy: i32,
) -> Option<super::SectionMesh> {
    let c = region[1][1]?;
    let parts: Vec<([i32; 3], Vec<u32>)> = read(|t| {
        t.sections
            .get(&(c.x, sy, c.z))
            .map(|v| v.iter().filter_map(|p| t.hosts.get(p).map(|h| (*p, h.states.clone()))).collect())
            .unwrap_or_default()
    })?;
    if parts.is_empty() || INNER.with(|i| i.replace(true)) {
        return None;
    }
    let mut out = this.mesh(region, sy);
    INNER.with(|i| i.set(false));
    draw(this, region, sy, &parts, &mut out);
    Some(out)
}

/// The parts' models at their hosts' cells, like the mesher's own block loop: neighbour culling,
/// smooth light and ambient occlusion, tints.
fn draw(
    this: &mut super::Mesher<'_>,
    region: &[[Option<&super::Chunk>; 3]; 3],
    sy: i32,
    parts: &[([i32; 3], Vec<u32>)],
    out: &mut super::SectionMesh,
) {
    let Some(center) = region[1][1] else { return };
    let pad = super::Padded::new(region, sy);
    let base_y = super::MIN_Y + sy * 16;
    let mut tints = super::Tints::new(region, this.biomes, this.zoom, this.blend_radius, [center.x, center.z, base_y]);
    let tables = this.tables;
    let render = this.render;
    let catalog = &render.model_catalog;
    let mut models = Vec::new();
    for (p, states) in parts {
        let (x, y, z) = (p[0] - center.x * 16, p[1] - base_y, p[2] - center.z * 16);
        if !(0..16).contains(&x) || !(0..16).contains(&y) || !(0..16).contains(&z) {
            continue;
        }
        for &id in states {
            let Some(state) = tables.info.get(id as usize) else { continue };
            if !state.model {
                continue;
            }
            let world = BlockPos::new(p[0], p[1], p[2]);
            if catalog.models_at(BlockStateId(id), world, &mut models).is_err() {
                continue;
            }
            let Some(first) = models.first() else { continue };
            let ao = this.smooth_lighting && state.emission == 0 && first.ambient_occlusion;
            let origin = [x as f32, y as f32, z as f32];
            let mut visible = [true; 6];
            for d in Direction::ALL {
                let [dx, dy, dz] = d.normal();
                let neighbor = pad.blocks[super::pi(x + dx, y + dy, z + dz)];
                visible[d as usize] = this
                    .visibility
                    .should_render_face(BlockStateId(id), BlockStateId(neighbor), d, true)
                    .unwrap_or(true);
            }
            for model in &models {
                for face in &model.faces {
                    if face.quad.cull.is_some_and(|d| !visible[d as usize]) {
                        continue;
                    }
                    this.face(&pad, &mut tints, out, [x, y, z], origin, face, ao, state);
                }
            }
        }
    }
}

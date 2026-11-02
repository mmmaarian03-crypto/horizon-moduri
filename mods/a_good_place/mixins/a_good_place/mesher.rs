// A Good Place, the part the chunk mesher reads (the mesher runs on worker threads, in another
// crate than the game): the blocks whose placement animation is running. They are left out of the
// section meshes and count as air to their neighbours, as the original's `BlockMixin`
// (`shouldRenderFace`), `BlockBehaviorMixin` (`isSolidRender`) and `RebuildTaskMixin`
// (`getRenderShape` invisible) do through `BlocksParticlesManager.isBlockHidden`.
use std::collections::BTreeSet;
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

static HIDDEN: RwLock<BTreeSet<[i32; 3]>> = RwLock::new(BTreeSet::new());
/// How many are hidden, so a mesh job with none costs one atomic read.
static COUNT: AtomicUsize = AtomicUsize::new(0);

/// `BlocksParticlesManager.hideBlock`.
pub fn hide(pos: [i32; 3]) {
    if let Ok(mut set) = HIDDEN.write() {
        set.insert(pos);
        COUNT.store(set.len(), Ordering::Release);
    }
}
/// `unHideBlock`'s removal: whether it was hidden.
pub fn unhide(pos: [i32; 3]) -> bool {
    match HIDDEN.write() {
        Ok(mut set) => {
            let was = set.remove(&pos);
            COUNT.store(set.len(), Ordering::Release);
            was
        }
        Err(_) => false,
    }
}
/// `BlocksParticlesManager.clear`.
pub fn clear() {
    if let Ok(mut set) = HIDDEN.write() {
        set.clear();
        COUNT.store(0, Ordering::Release);
    }
}
/// `isBlockHidden`.
pub fn is_hidden(pos: [i32; 3]) -> bool {
    COUNT.load(Ordering::Acquire) != 0 && HIDDEN.read().is_ok_and(|set| set.contains(&pos))
}

/// Blank the hidden blocks out of a mesher's padded block cube: `blocks` holds `side`^3 ids, the
/// cell (x, y, z) of the section at `base` (the corner of the section) with `border` cells of
/// margin, at `((y + border) * side + (z + border)) * side + (x + border)`.
pub fn blank(blocks: &mut [u32], base: [i32; 3], border: i32, side: i32) {
    if COUNT.load(Ordering::Acquire) == 0 {
        return;
    }
    let Ok(set) = HIDDEN.read() else {
        return;
    };
    for p in set.iter() {
        let l = [p[0] - base[0], p[1] - base[1], p[2] - base[2]];
        if l.iter().all(|&v| (-border..16 + border).contains(&v)) {
            let i = ((l[1] + border) * side + (l[2] + border)) * side + (l[0] + border);
            if let Some(cell) = blocks.get_mut(i as usize) {
                *cell = 0;
            }
        }
    }
}

thread_local! {
    /// Inside the mesher hook's own call of `Padded::new`.
    static INNER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/// Takes the guard: false when this thread already holds it (the call is the hook's own).
pub fn enter() -> bool {
    INNER.with(|i| !i.replace(true))
}
pub fn leave() {
    INNER.with(|i| i.set(false));
}
/// Whether a hidden block is inside the section at `base` with `border` cells of margin.
pub fn near(base: [i32; 3], border: i32) -> bool {
    if COUNT.load(Ordering::Acquire) == 0 {
        return false;
    }
    HIDDEN.read().is_ok_and(|set| {
        set.iter().any(|p| (0..3).all(|k| (-border..16 + border).contains(&(p[k] - base[k]))))
    })
}

/// The hidden positions inside the chunk column `(cx, cz)`.
pub fn inside(cx: i32, cz: i32) -> Vec<[i32; 3]> {
    if COUNT.load(Ordering::Acquire) == 0 {
        return Vec::new();
    }
    HIDDEN.read().map_or_else(
        |_| Vec::new(),
        |set| set.iter().filter(|p| p[0] >> 4 == cx && p[2] >> 4 == cz).copied().collect(),
    )
}

// Pornire Rapida in the content root builder (crates/horizon-client/src/mod_content.rs, library crate, nightly
// 142388e1c). The rules are the engine below (src/lib.rs, pasted here by tools/hnmod.py bundle); this part is the one function
// the hook calls. It is the `cfg(not(unix))` body of `mod_content::link`, which the hook cannot reach (two definitions:
// E_TARGET_AMBIGUOUS), reached from its only caller, `Overlay::link_children`, whose own body is kept line by line.
#[allow(unused_imports)]
use super::{Overlay, Result};
use std::path::Path;
#[allow(dead_code)]
mod engine {
    include!("pornire_rapida_engine.rs");
}
#[allow(unused_imports)]
use std::fs;

/// `Overlay::link_children` for Windows: the base's children appear in the overlay as hard links (a copy when
/// linking fails), where the game copied them. On Unix the original runs (symlinks).
///
/// The original, for comparison:
/// ```text
/// fn link_children(&self, rel: &Path) -> Result<()> {
///     let Ok(read) = fs::read_dir(self.base.join(rel)) else { return Ok(()); };
///     for entry in read.flatten() {
///         let rel = rel.join(entry.file_name());
///         link(&self.base.join(&rel), &self.out.join(&rel))?;
///     }
///     Ok(())
/// }
/// ```
pub(super) fn link_children(overlay: &Overlay<'_>, rel: &Path) -> core::ops::ControlFlow<Result<()>> {
    #[cfg(unix)]
    {
        let _ = (overlay, rel);
        core::ops::ControlFlow::Continue(())
    }
    #[cfg(not(unix))]
    {
        let started = std::time::Instant::now();
        let Ok(read) = fs::read_dir(overlay.base.join(rel)) else {
            return core::ops::ControlFlow::Break(Ok(()));
        };
        let linker = engine::Linker::new();
        for entry in read.flatten() {
            let child = rel.join(entry.file_name());
            if let Err(e) = linker.link(&overlay.base.join(&child), &overlay.out.join(&child)) {
                return core::ops::ControlFlow::Break(Err(e.into()));
            }
        }
        // The whole base is linked once, from the root; later calls make one folder of a mod.
        if rel.as_os_str().is_empty() {
            let c = linker.counts();
            log::info!(
                "Horizon: pornire_rapida: {} files hard-linked, {} copied, {} folders, in {:.1} s",
                c.linked,
                c.copied,
                c.dirs,
                started.elapsed().as_secs_f32()
            );
            if let Some(why) = c.first_failure {
                log::warn!(
                    "Horizon: pornire_rapida: {} files were copied because a hard link failed (the first: {why})",
                    c.copied
                );
            }
        }
        core::ops::ControlFlow::Break(Ok(()))
    }
}

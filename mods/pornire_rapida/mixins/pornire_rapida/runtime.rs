// Pornire Rapida in the content root builder (crates/horizon-client/src/mod_content.rs, library crate, nightly
// 142388e1c). The rules are the engine below (src/lib.rs, pasted here by tools/hnmod.py bundle); this part is the one function
// the hook calls. It is the `cfg(not(unix))` body of `mod_content::link`, which the hook cannot reach (two definitions:
// E_TARGET_AMBIGUOUS), reached from its only caller, `Overlay::link_children`, whose own body is kept line by line.
#[allow(unused_imports)]
use super::{Overlay, Result};
use std::path::Path;
#[allow(dead_code)]
mod engine {
    // Pornire Rapida for Horizon: the rules, without any game code.
//
// Not a port of a Forge mod: it fixes the first start of the game on Windows. `mod_content::link` (nightly
// 142388e1c, crates/horizon-client/src/mod_content.rs:846, the `cfg(not(unix))` twin) COPIES the vanilla resource
// root into `.horizon-cache/mod-content` of the game home (`fs::copy` per file) where Unix makes a symlink. This file
// holds the replacement: a directory becomes a real directory, a file becomes a HARD LINK to the base's file, and only
// when linking fails (another volume, a filesystem without links, the NTFS limit of 1023 links per file) it is copied,
// as before. Hard links need no administrator rights on NTFS (symlinks do).
//
// The rest of `link`'s behaviour is kept as it was, line by line (see `Linker::link`): `is_dir()` follows links, a
// directory is made with `create_dir_all`, its children come from `read_dir(..)?.flatten()`, and a file that is a
// symlink is still copied through it.
//
// Why a hard link cannot corrupt the base: the root is only ever REPLACED file by file, never edited in place.
// `Overlay::write` removes the name before `fs::write` (`remove_file` drops the link, not the base's data) and the
// stamp is written to a file the base does not have. The tests check the first of these on a real folder.
//
// This file is pasted into the game into `crates/horizon-client/src/mod_content.rs` (library crate: the adapter
// native/horizon.rs includes it, tools/hnmod.py bundle writes mixins/pornire_rapida/runtime.rs) and it is also the
// `horizon_pornire_rapida` crate the tests use: plain Rust, std only, no inner attributes (`#![...]`).
use std::cell::{Cell, RefCell};
use std::fs;
use std::io;
use std::path::Path;

/// Makes `at` a hard link to `target`. A function pointer so the tests can make linking fail.
pub type LinkFn = fn(&Path, &Path) -> io::Result<()>;

/// The real thing: `CreateHardLinkW` on Windows, `link(2)` elsewhere.
pub fn hard_link(target: &Path, at: &Path) -> io::Result<()> {
    fs::hard_link(target, at)
}

/// What a [`Linker`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Files made as hard links.
    pub linked: u64,
    /// Files copied because linking failed (or because the file is a symlink, which is followed).
    pub copied: u64,
    /// Directories made.
    pub dirs: u64,
    /// Why the first copy was needed, when linking failed (not for a symlink).
    pub first_failure: Option<String>,
}

/// Links a tree: the Windows body of `mod_content::link` with hard links in place of copies.
pub struct Linker {
    hard: LinkFn,
    linked: Cell<u64>,
    copied: Cell<u64>,
    dirs: Cell<u64>,
    first_failure: RefCell<Option<String>>,
}

impl Default for Linker {
    fn default() -> Self {
        Self::new()
    }
}

impl Linker {
    /// Hard links, copies when they fail.
    pub fn new() -> Self {
        Self::with(hard_link)
    }

    /// Like [`Linker::new`] with another way to link (the tests pass one that fails).
    pub fn with(hard: LinkFn) -> Self {
        Linker {
            hard,
            linked: Cell::new(0),
            copied: Cell::new(0),
            dirs: Cell::new(0),
            first_failure: RefCell::new(None),
        }
    }

    pub fn counts(&self) -> Counts {
        Counts {
            linked: self.linked.get(),
            copied: self.copied.get(),
            dirs: self.dirs.get(),
            first_failure: self.first_failure.borrow().clone(),
        }
    }

    /// `at` as `target` is: a directory is made (`create_dir_all`) and holds what each child of `target` becomes; a
    /// file is a hard link, else a copy. The errors are the original `link`'s (a directory that cannot be made or
    /// read, a copy that fails); a link that fails is not one, the file is copied.
    pub fn link(&self, target: &Path, at: &Path) -> io::Result<()> {
        if target.is_dir() {
            fs::create_dir_all(at)?;
            self.dirs.set(self.dirs.get() + 1);
            for entry in fs::read_dir(target)?.flatten() {
                self.link(&entry.path(), &at.join(entry.file_name()))?;
            }
            Ok(())
        } else {
            self.file(target, at)
        }
    }

    /// One file. A symlink is copied (`fs::copy` follows it, as the original did): a hard link to a symlink would be a
    /// symlink, perhaps a relative one that points nowhere from its new folder.
    fn file(&self, target: &Path, at: &Path) -> io::Result<()> {
        let symlink = fs::symlink_metadata(target).is_ok_and(|m| m.file_type().is_symlink());
        if !symlink {
            match (self.hard)(target, at) {
                Ok(()) => {
                    self.linked.set(self.linked.get() + 1);
                    return Ok(());
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    // The original copied over what was there. Take the name away first (this drops a link, never
                    // the base's data) and link again: a copy onto a link to the same file would fail, or worse.
                    if fs::remove_file(at).is_ok() && (self.hard)(target, at).is_ok() {
                        self.linked.set(self.linked.get() + 1);
                        return Ok(());
                    }
                }
                Err(e) => {
                    let mut first = self.first_failure.borrow_mut();
                    if first.is_none() {
                        *first = Some(format!("{}: {e}", target.display()));
                    }
                }
            }
        }
        fs::copy(target, at).map(|_| ())?;
        self.copied.set(self.copied.get() + 1);
        Ok(())
    }
}

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

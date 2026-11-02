// Tests of the rules on a temp folder: links are made, the copy fallback works, the result is byte-identical to what the
// game's copy version builds, a second run changes nothing, and replacing a file of the overlay never touches the base.
use horizon_pornire_rapida::{Counts, Linker};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// A folder in the temp dir, removed when the test ends (also when it fails).
struct Scratch(PathBuf);
impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("pornire_rapida-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
    fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A small copy of what the resource root looks like: nested folders, JSON, binary files, an empty file and an empty
/// folder. Returns the number of files.
fn make_base(base: &Path) -> usize {
    let mut n = 0;
    let mut put = |rel: &str, bytes: Vec<u8>| {
        let p = base.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
        n += 1;
    };
    put("horizon-resources.json", b"{\"version\":\"26.3\"}".to_vec());
    put("items.json", b"{\"minecraft:stone\":{}}".to_vec());
    put("empty.log", Vec::new());
    put("assets/minecraft/lang/en_us.json", "{\"a\":\"ă î ș\"}".as_bytes().to_vec());
    put("assets/minecraft/textures/block/stone.png", (0..=255u8).cycle().take(300_000).collect());
    put("assets/minecraft/models/block/stone.json", b"{}".to_vec());
    put("data/minecraft/tags/block/logs.json", b"{\"values\":[]}".to_vec());
    put("data/minecraft/worldgen/a/b/c/deep.json", b"deep".to_vec());
    fs::create_dir_all(base.join("worldgen/no-files-here")).unwrap();
    n
}

/// Every folder (`None`) and file (`Some(bytes)`) under `root`, by relative path.
fn tree(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Option<Vec<u8>>>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            if p.is_dir() {
                out.insert(rel, None);
                walk(root, &p, out);
            } else {
                out.insert(rel, Some(fs::read(&p).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// Links `base` into `out` the way `Overlay::link_children` does: each child of the root, with the linker.
fn link_all(linker: &Linker, base: &Path, out: &Path) -> io::Result<()> {
    fs::create_dir_all(out)?;
    for e in fs::read_dir(base)?.flatten() {
        linker.link(&e.path(), &out.join(e.file_name()))?;
    }
    Ok(())
}

/// `mod_content::link`, the `cfg(not(unix))` twin of nightly 142388e1c, as it was (the copy version).
fn link_original(target: &Path, at: &Path) -> io::Result<()> {
    if target.is_dir() {
        fs::create_dir_all(at)?;
        for entry in fs::read_dir(target)?.flatten() {
            link_original(&entry.path(), &at.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(target, at).map(|_| ())
    }
}

/// Edits a file where it is (same inode), not by replacing it: a hard link sees the edit, a copy does not.
fn edit_in_place(path: &Path, text: &[u8]) {
    use std::io::Write;
    let mut f = fs::OpenOptions::new().write(true).truncate(true).open(path).unwrap();
    f.write_all(text).unwrap();
}

fn no_links(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "no links on this volume"))
}

#[test]
fn files_become_hard_links_and_folders_become_folders() {
    let s = Scratch::new("links");
    let (base, out) = (s.path("base"), s.path("out"));
    let files = make_base(&base);
    let linker = Linker::new();
    link_all(&linker, &base, &out).unwrap();
    let c = linker.counts();
    assert_eq!(c.linked, files as u64, "every file is a link: {c:?}");
    assert_eq!((c.copied, c.first_failure.as_deref()), (0, None));
    assert_eq!(c.dirs, 17, "every folder of the base is made once: {c:?}");
    // Same names, same bytes, the empty folder too.
    assert_eq!(tree(&out), tree(&base));
    assert!(out.join("worldgen/no-files-here").is_dir());
    // Linked, not copied: an edit of the base's file in place shows in the overlay.
    edit_in_place(&base.join("items.json"), b"edited");
    assert_eq!(fs::read(out.join("items.json")).unwrap(), b"edited");
    edit_in_place(&base.join("assets/minecraft/lang/en_us.json"), b"deep edit");
    assert_eq!(fs::read(out.join("assets/minecraft/lang/en_us.json")).unwrap(), b"deep edit");
}

#[test]
fn a_failing_link_falls_back_to_a_copy() {
    let s = Scratch::new("copy");
    let (base, out) = (s.path("base"), s.path("out"));
    let files = make_base(&base);
    let linker = Linker::with(no_links);
    link_all(&linker, &base, &out).unwrap();
    let c = linker.counts();
    assert_eq!((c.linked, c.copied), (0, files as u64), "{c:?}");
    assert!(c.first_failure.unwrap().contains("no links on this volume"));
    assert_eq!(tree(&out), tree(&base));
    // Copies are independent: the base can change without the overlay noticing, as before.
    edit_in_place(&base.join("items.json"), b"edited");
    assert_eq!(fs::read(out.join("items.json")).unwrap(), b"{\"minecraft:stone\":{}}");
}

#[test]
fn some_links_fail_and_the_rest_still_link() {
    // A link that works only for small files (stands for a file that cannot be linked): the others are copied.
    fn small_only(target: &Path, at: &Path) -> io::Result<()> {
        if fs::metadata(target)?.len() > 100_000 {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        fs::hard_link(target, at)
    }
    let s = Scratch::new("mixed");
    let (base, out) = (s.path("base"), s.path("out"));
    let files = make_base(&base);
    let linker = Linker::with(small_only);
    link_all(&linker, &base, &out).unwrap();
    let c = linker.counts();
    assert_eq!((c.linked, c.copied), (files as u64 - 1, 1), "{c:?}");
    assert_eq!(tree(&out), tree(&base));
}

#[test]
fn the_result_is_byte_identical_to_the_copy_version() {
    let s = Scratch::new("same");
    let (base, linked, copied) = (s.path("base"), s.path("linked"), s.path("copied"));
    make_base(&base);
    link_all(&Linker::new(), &base, &linked).unwrap();
    fs::create_dir_all(&copied).unwrap();
    for e in fs::read_dir(&base).unwrap().flatten() {
        link_original(&e.path(), &copied.join(e.file_name())).unwrap();
    }
    let (a, b) = (tree(&linked), tree(&copied));
    assert_eq!(a.len(), b.len());
    assert_eq!(a, b);
    assert!(a.values().any(|v| v.as_ref().is_some_and(|b| b.len() == 300_000)));
}

#[test]
fn running_again_changes_nothing() {
    let s = Scratch::new("again");
    let (base, out) = (s.path("base"), s.path("out"));
    let files = make_base(&base);
    link_all(&Linker::new(), &base, &out).unwrap();
    let first = tree(&out);
    // Again onto the same overlay: every name exists already (and is a link to the very same file).
    let linker = Linker::new();
    link_all(&linker, &base, &out).unwrap();
    let c = linker.counts();
    assert_eq!((c.linked, c.copied), (files as u64, 0), "{c:?}");
    assert_eq!(tree(&out), first);
    assert_eq!(tree(&base), first);
    // Still links to the base, not copies made the second time.
    edit_in_place(&base.join("data/minecraft/worldgen/a/b/c/deep.json"), b"edited again");
    assert_eq!(fs::read(out.join("data/minecraft/worldgen/a/b/c/deep.json")).unwrap(), b"edited again");
}

#[test]
fn a_name_already_taken_by_another_file_is_replaced() {
    let s = Scratch::new("taken");
    let (base, out) = (s.path("base"), s.path("out"));
    make_base(&base);
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("items.json"), b"stale, not a link").unwrap();
    link_all(&Linker::new(), &base, &out).unwrap();
    assert_eq!(fs::read(out.join("items.json")).unwrap(), b"{\"minecraft:stone\":{}}");
    // The stale file was a file of its own: the base has not changed.
    assert_eq!(fs::read(base.join("items.json")).unwrap(), b"{\"minecraft:stone\":{}}");
}

/// `Overlay::write`: the name is taken away, then the new bytes are written. On a hard link the first step drops the
/// link and not the base's data, so the base keeps its content and the overlay's other links are untouched.
#[test]
fn replacing_a_file_of_the_overlay_leaves_the_base_alone() {
    let s = Scratch::new("write");
    let (base, out) = (s.path("base"), s.path("out"));
    make_base(&base);
    link_all(&Linker::new(), &base, &out).unwrap();
    let before = tree(&base);
    let path = out.join("data/minecraft/tags/block/logs.json");
    assert!(fs::symlink_metadata(&path).is_ok());
    fs::remove_file(&path).unwrap();
    fs::write(&path, b"{\"values\":[\"mod:log\"]}").unwrap();
    assert_eq!(tree(&base), before, "the base must not change");
    assert_eq!(fs::read(&path).unwrap(), b"{\"values\":[\"mod:log\"]}");
    // The rest of the overlay still follows the base.
    edit_in_place(&base.join("items.json"), b"edited");
    assert_eq!(fs::read(out.join("items.json")).unwrap(), b"edited");
    // Removing the whole overlay (a rebuild starts that way) leaves the base whole.
    let expected = {
        let mut t = before.clone();
        t.insert("items.json".into(), Some(b"edited".to_vec()));
        t
    };
    fs::remove_dir_all(&out).unwrap();
    assert_eq!(tree(&base), expected);
}

#[test]
fn a_missing_target_is_an_error_like_before() {
    let s = Scratch::new("missing");
    fs::create_dir_all(s.path("out")).unwrap();
    let e = Linker::new().link(&s.path("nope.json"), &s.path("out/nope.json")).unwrap_err();
    let original = link_original(&s.path("nope.json"), &s.path("out/nope2.json")).unwrap_err();
    assert_eq!(e.kind(), original.kind());
    assert!(!s.path("out/nope.json").exists());
}

#[test]
fn counts_start_at_zero() {
    assert_eq!(Linker::new().counts(), Counts::default());
}

/// A file that is a symlink is copied through it (what `fs::copy` did), not linked as a symlink.
#[cfg(unix)]
#[test]
fn a_symlink_is_copied_through() {
    let s = Scratch::new("symlink");
    let (base, out) = (s.path("base"), s.path("out"));
    fs::create_dir_all(&base).unwrap();
    fs::write(base.join("real.json"), b"real").unwrap();
    std::os::unix::fs::symlink("real.json", base.join("alias.json")).unwrap();
    let linker = Linker::new();
    link_all(&linker, &base, &out).unwrap();
    let c = linker.counts();
    assert_eq!((c.linked, c.copied), (1, 1), "{c:?}");
    assert!(!fs::symlink_metadata(out.join("alias.json")).unwrap().file_type().is_symlink());
    assert_eq!(fs::read(out.join("alias.json")).unwrap(), b"real");
}

// Loading screen, its part in the game's libraries (`horizon_mods::content::ecran_incarcare`): the counters
// of the content build and the progress math. Plain Rust, no game types: the mod's tests
// compile this very file (tests/progress.rs). The hooks feed it:
//   Content::source  (HEAD, once per mod, in `stamp`, before the build)  -> source_seen
//   Content::reader  (HEAD, once per mod, in the mod loop of `build`)    -> reader_started
//   Overlay::write   (HEAD, mod_content.rs, for every rewritten table)   -> tables_known / table_written
// Read by the loading screen (the game, `Loading::draw_menu`) through `snapshot()` / `fraction()`.
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

/// The screen's texts are English; `true` gives the Romanian ones (set it here and in `src/lib.rs`).
pub const ROMANIAN: bool = false;

/// How much of the bar is the game's tables (written first, on parallel threads); the rest is the mods' files.
/// An assumption: the real time ratio was not measured, so the bar promises no time, only the order.
pub const TABLES_WEIGHT: f32 = 0.5;
/// The bar does not reach 100% before the build ends (the tags come after the last mod).
pub const CEILING: f32 = 0.99;

/// What is known now about the build.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// The tables to rewrite (count and bytes) and how many are done; 0 = none written yet.
    pub tables_count: u32,
    pub tables_done: u32,
    pub tables_bytes: u64,
    pub tables_bytes_done: u64,
    /// The content mods and their files, from `stamp`.
    pub mods_total: u32,
    pub files_total: u64,
    /// How many mods have started being read, the files of the finished ones and of the current one.
    pub mods_started: u32,
    pub files_before: u64,
    pub files_current: u64,
}

impl Snapshot {
    /// The tables' part (0..1); 1 once the first mod has started (the tables are written before the mods).
    fn tables_part(&self) -> f32 {
        if self.mods_started > 0 {
            return 1.0;
        }
        if self.tables_bytes > 0 {
            return (self.tables_bytes_done as f64 / self.tables_bytes as f64).clamp(0.0, 1.0) as f32;
        }
        if self.tables_count > 0 {
            return (self.tables_done as f32 / self.tables_count as f32).clamp(0.0, 1.0);
        }
        0.0
    }
    /// The mods' part (0..1): the files of the finished ones plus half of the current mod's.
    fn mods_part(&self) -> f32 {
        if self.mods_started == 0 {
            return 0.0;
        }
        if self.files_total > 0 {
            let done = self.files_before as f64 + self.files_current as f64 / 2.0;
            return (done / self.files_total as f64).clamp(0.0, 1.0) as f32;
        }
        let total = self.mods_total.max(self.mods_started) as f32;
        ((self.mods_started - 1) as f32 / total).clamp(0.0, 1.0)
    }
    /// `None` while nothing is measured yet (the indeterminate animated bar).
    pub fn fraction(&self) -> Option<f32> {
        if self.mods_started == 0 && self.tables_done == 0 {
            return None;
        }
        let f = TABLES_WEIGHT * self.tables_part() + (1.0 - TABLES_WEIGHT) * self.mods_part();
        Some(f.clamp(0.0, CEILING))
    }
    /// The status line, in English (in Romanian with [`ROMANIAN`]).
    pub fn line(&self) -> String {
        self.line_in(ROMANIAN)
    }
    /// The status line in English, or in Romanian when `romanian` is set.
    pub fn line_in(&self, romanian: bool) -> String {
        let preparing = if romanian { "Se pregătesc modurile" } else { "Preparing mods" };
        if self.mods_started > 0 {
            let total = self.mods_total.max(self.mods_started);
            let mut s = format!("{preparing}: {}/{}", self.mods_started, total);
            if let Some(f) = self.fraction() {
                s += &format!(" ({}%)", (f * 100.0).round() as u32);
            }
            s
        } else if self.tables_done > 0 {
            format!(
                "{preparing}: {} {}/{}",
                if romanian { "tabele" } else { "tables" },
                self.tables_done,
                self.tables_count.max(self.tables_done)
            )
        } else if self.mods_total > 0 {
            format!("{preparing} ({})...", self.mods_total)
        } else if romanian {
            "Se pornește Horizon...".to_owned()
        } else {
            "Starting Horizon...".to_owned()
        }
    }
    /// A second row, only while the content is being built.
    pub fn note(&self) -> Option<&'static str> {
        self.note_in(ROMANIAN)
    }
    /// [`Snapshot::note`] in English, or in Romanian when `romanian` is set.
    pub fn note_in(&self, romanian: bool) -> Option<&'static str> {
        let note = if romanian {
            "Nu închide jocul: prima pornire durează mai mult"
        } else {
            "Do not close the game: the first start takes longer"
        };
        (self.mods_total > 0).then_some(note)
    }
}

/// The live counters: atomic, so the threads writing the tables never wait.
#[derive(Default)]
pub struct Counters {
    tables_count: AtomicU32,
    tables_done: AtomicU32,
    tables_bytes: AtomicU64,
    tables_bytes_done: AtomicU64,
    mods_total: AtomicU32,
    files_total: AtomicU64,
    mods_started: AtomicU32,
    files_before: AtomicU64,
    files_current: AtomicU64,
    /// The largest fraction shown (tenths of a percent): the bar never goes back.
    peak: AtomicU32,
}

impl Counters {
    pub const fn new() -> Self {
        Self {
            tables_count: AtomicU32::new(0),
            tables_done: AtomicU32::new(0),
            tables_bytes: AtomicU64::new(0),
            tables_bytes_done: AtomicU64::new(0),
            mods_total: AtomicU32::new(0),
            files_total: AtomicU64::new(0),
            mods_started: AtomicU32::new(0),
            files_before: AtomicU64::new(0),
            files_current: AtomicU64::new(0),
            peak: AtomicU32::new(0),
        }
    }
    /// `stamp` passes over every mod: the totals. Once a build has started (`reader_started`),
    /// a new `source_seen` starts another one (a pack reload): the counters start from zero.
    pub fn source_seen(&self, files: usize) {
        if self.mods_started.load(Relaxed) > 0 {
            self.reset();
        }
        self.mods_total.fetch_add(1, Relaxed);
        self.files_total.fetch_add(files as u64, Relaxed);
    }
    /// `build` starts reading a mod (in order): the one before is done.
    pub fn reader_started(&self, files: usize) {
        let current = self.files_current.swap(files as u64, Relaxed);
        self.files_before.fetch_add(current, Relaxed);
        self.mods_started.fetch_add(1, Relaxed);
    }
    /// How many tables `tables` rewrites and their bytes; idempotent (both threads may get here).
    pub fn tables_known(&self, count: u32, bytes: u64) {
        self.tables_count.store(count, Relaxed);
        self.tables_bytes.store(bytes, Relaxed);
    }
    /// A table written (its bytes in the base).
    pub fn table_written(&self, bytes: u64) {
        self.tables_bytes_done.fetch_add(bytes, Relaxed);
        self.tables_done.fetch_add(1, Relaxed);
    }
    pub fn reset(&self) {
        for a in [&self.tables_count, &self.tables_done, &self.mods_total, &self.mods_started, &self.peak] {
            a.store(0, Relaxed);
        }
        for a in [
            &self.tables_bytes,
            &self.tables_bytes_done,
            &self.files_total,
            &self.files_before,
            &self.files_current,
        ] {
            a.store(0, Relaxed);
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            tables_count: self.tables_count.load(Relaxed),
            tables_done: self.tables_done.load(Relaxed),
            tables_bytes: self.tables_bytes.load(Relaxed),
            tables_bytes_done: self.tables_bytes_done.load(Relaxed),
            mods_total: self.mods_total.load(Relaxed),
            files_total: self.files_total.load(Relaxed),
            mods_started: self.mods_started.load(Relaxed),
            files_before: self.files_before.load(Relaxed),
            files_current: self.files_current.load(Relaxed),
        }
    }
    /// The fraction to show: it never goes down.
    pub fn fraction(&self) -> Option<f32> {
        let f = self.snapshot().fraction()?;
        let now = (f * 1000.0).round() as u32;
        let peak = self.peak.fetch_max(now, Relaxed).max(now);
        Some(peak as f32 / 1000.0)
    }
}

static STATE: Counters = Counters::new();

pub fn source_seen(files: usize) {
    STATE.source_seen(files);
}
pub fn reader_started(files: usize) {
    STATE.reader_started(files);
}
pub fn tables_known(count: u32, bytes: u64) {
    STATE.tables_known(count, bytes);
}
pub fn table_written(bytes: u64) {
    STATE.table_written(bytes);
}
pub fn snapshot() -> Snapshot {
    STATE.snapshot()
}
pub fn fraction() -> Option<f32> {
    STATE.fraction()
}

// Memory of this process and of the machine (the game crate forbids unsafe code, this library does not).
#[cfg(windows)]
mod ram {
    #[repr(C)]
    struct Counters {
        cb: u32,
        page_faults: u32,
        peak_working_set: usize,
        working_set: usize,
        rest: [usize; 6],
    }
    #[repr(C)]
    struct Status {
        length: u32,
        load: u32,
        total_phys: u64,
        rest: [u64; 6],
    }
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
        fn GlobalMemoryStatusEx(status: *mut Status) -> i32;
    }
    /// (working set of this process, total physical memory) in bytes.
    pub fn query() -> Option<(u64, u64)> {
        unsafe {
            let mut c: Counters = std::mem::zeroed();
            c.cb = std::mem::size_of::<Counters>() as u32;
            if K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) == 0 {
                return None;
            }
            let mut s: Status = std::mem::zeroed();
            s.length = std::mem::size_of::<Status>() as u32;
            if GlobalMemoryStatusEx(&mut s) == 0 {
                return None;
            }
            Some((c.working_set as u64, s.total_phys))
        }
    }
}
#[cfg(target_os = "linux")]
mod ram {
    pub fn query() -> Option<(u64, u64)> {
        let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
        let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
        let info = std::fs::read_to_string("/proc/meminfo").ok()?;
        let kb: u64 = info.lines().find(|l| l.starts_with("MemTotal:"))?.split_whitespace().nth(1)?.parse().ok()?;
        Some((pages * 4096, kb * 1024))
    }
}
#[cfg(not(any(windows, target_os = "linux")))]
mod ram {
    pub fn query() -> Option<(u64, u64)> {
        None
    }
}

/// (working set of this process, total physical memory) in bytes; `None` where unknown or on failure.
pub fn memory_usage() -> Option<(u64, u64)> {
    ram::query()
}

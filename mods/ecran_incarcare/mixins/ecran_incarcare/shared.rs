// Ecran Incarcare, partea din bibliotecile jocului (`horizon_mods::content::ecran_incarcare`): contoarele
// constructiei de continut si matematica progresului. Rust simplu, fara tipuri de joc: testele modului
// compileaza chiar acest fisier (tests/progress.rs). Il alimenteaza hook-urile:
//   Content::source  (HEAD, o data per mod, in `stamp`, inainte de constructie)  -> source_seen
//   Content::reader  (HEAD, o data per mod, in bucla modurilor din `build`)       -> reader_started
//   Overlay::write   (HEAD, mod_content.rs, pentru fiecare tabel rescris)         -> tables_known / table_written
// Citit de ecranul de incarcare (jocul, `Loading::draw_menu`) prin `snapshot()` / `fraction()`.
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

/// Cat din bara e tabelele jocului (scrise prima data, pe fire paralele); restul sunt fisierele modurilor.
/// Presupunere: nu am masurat raportul de timp real, de aceea bara nu promite un timp, doar ordinea.
pub const TABLES_WEIGHT: f32 = 0.5;
/// Bara nu ajunge la 100% cat timp constructia nu s-a terminat (dupa ultimul mod mai vin etichetele).
pub const CEILING: f32 = 0.99;

/// Ce se stie acum despre constructie.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// Tabelele de rescris (numar si octeti) si cate sunt gata; 0 = inca nu s-a scris niciunul.
    pub tables_count: u32,
    pub tables_done: u32,
    pub tables_bytes: u64,
    pub tables_bytes_done: u64,
    /// Modurile cu continut si fisierele lor, din `stamp`.
    pub mods_total: u32,
    pub files_total: u64,
    /// Cate moduri au inceput sa fie citite, fisierele celor terminate si ale celui curent.
    pub mods_started: u32,
    pub files_before: u64,
    pub files_current: u64,
}

impl Snapshot {
    /// Partea tabelelor (0..1); 1 de cand a inceput primul mod (tabelele se scriu inaintea lor).
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
    /// Partea modurilor (0..1): fisierele celor terminate plus jumatate din cele ale modului curent.
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
    /// `None` cat timp nu avem nicio masuratoare (bara animata nedeterminata).
    pub fn fraction(&self) -> Option<f32> {
        if self.mods_started == 0 && self.tables_done == 0 {
            return None;
        }
        let f = TABLES_WEIGHT * self.tables_part() + (1.0 - TABLES_WEIGHT) * self.mods_part();
        Some(f.clamp(0.0, CEILING))
    }
    /// Linia de stare, in romana.
    pub fn line(&self) -> String {
        if self.mods_started > 0 {
            let total = self.mods_total.max(self.mods_started);
            let mut s = format!("Se pregătesc modurile: {}/{}", self.mods_started, total);
            if let Some(f) = self.fraction() {
                s += &format!(" ({}%)", (f * 100.0).round() as u32);
            }
            s
        } else if self.tables_done > 0 {
            format!(
                "Se pregătesc modurile: tabele {}/{}",
                self.tables_done,
                self.tables_count.max(self.tables_done)
            )
        } else if self.mods_total > 0 {
            format!("Se pregătesc modurile ({})...", self.mods_total)
        } else {
            "Se pornește Horizon...".to_owned()
        }
    }
    /// Un al doilea rand, doar cat se construieste continutul.
    pub fn note(&self) -> Option<&'static str> {
        (self.mods_total > 0).then_some("Nu închide jocul: prima pornire durează mai mult")
    }
}

/// Contoarele vii: atomice, ca firele care scriu tabelele sa nu astepte.
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
    /// Cea mai mare fractiune afisata (zecimi de procent): bara nu se da inapoi.
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
    /// `stamp` trece pe fiecare mod: totalurile. Dupa ce o constructie a inceput (`reader_started`),
    /// un `source_seen` nou e inceputul alteia (reincarcarea pachetelor): contoarele pornesc de la zero.
    pub fn source_seen(&self, files: usize) {
        if self.mods_started.load(Relaxed) > 0 {
            self.reset();
        }
        self.mods_total.fetch_add(1, Relaxed);
        self.files_total.fetch_add(files as u64, Relaxed);
    }
    /// `build` incepe sa citeasca un mod (in ordine): cel dinainte e gata.
    pub fn reader_started(&self, files: usize) {
        let current = self.files_current.swap(files as u64, Relaxed);
        self.files_before.fetch_add(current, Relaxed);
        self.mods_started.fetch_add(1, Relaxed);
    }
    /// Cate tabele rescrie `tables` si cati octeti au; idempotent (firele pot ajunge amandoua aici).
    pub fn tables_known(&self, count: u32, bytes: u64) {
        self.tables_count.store(count, Relaxed);
        self.tables_bytes.store(bytes, Relaxed);
    }
    /// Un tabel scris (octetii lui in baza).
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
    /// Fractiunea de afisat: nu scade niciodata.
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

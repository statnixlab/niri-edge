//! Persistent per-application window sizes: the last floating window size and the last tiled
//! column width (as a proportion of the working area). Only app-ids, optional rule-provided
//! group names and sizes are stored; no window titles, contents or PIDs.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use smithay::utils::{Logical, Size};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::{SurfaceCachedState, ToplevelSurface};

use crate::utils::with_toplevel_role;
use crate::window::ResolvedWindowRules;
use crate::window_layout_memory::{valid_app_id, write_atomic};

const MAX_APPLICATIONS: usize = 4096;
const MAX_DIMENSION: i32 = 32768;
/// Changes are written once they have been stable for this long, even with windows open.
const SAVE_DELAY: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FloatingSize {
    pub width: i32,
    pub height: i32,
}

impl FloatingSize {
    fn is_valid(&self) -> bool {
        (1..=MAX_DIMENSION).contains(&self.width) && (1..=MAX_DIMENSION).contains(&self.height)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floating: Option<FloatingSize>,
    /// Column width as a proportion of the working area (restored when leaving full width).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiled_width: Option<f64>,
    /// Whether the column was full-width (maximize-column, title bar double-click).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiled_full_width: Option<bool>,
}

impl Record {
    fn is_valid(&self) -> bool {
        self.floating.is_none_or(|size| size.is_valid())
            && self.tiled_width.is_none_or(valid_proportion)
    }
}

fn valid_proportion(proportion: f64) -> bool {
    proportion.is_finite() && (0.01..=2.).contains(&proportion)
}

/// What a window currently looks like in its normal (non-maximized, non-fullscreen) state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Observed {
    Floating(Size<i32, Logical>),
    Tiled { proportion: f64, full_width: bool },
}

impl Observed {
    fn same_as(&self, other: &Observed) -> bool {
        match (self, other) {
            (Observed::Floating(a), Observed::Floating(b)) => a == b,
            (
                Observed::Tiled {
                    proportion: a,
                    full_width: full_a,
                },
                Observed::Tiled {
                    proportion: b,
                    full_width: full_b,
                },
            ) => full_a == full_b && (a - b).abs() < 1e-4,
            _ => false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedState {
    version: u32,
    applications: BTreeMap<String, Record>,
}

struct Writer {
    tx: Option<Sender<SavedState>>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub struct WindowSizeMemory {
    path: Option<PathBuf>,
    saved: SavedState,
    /// Last normal observation of each live window, to tell user changes from new windows.
    observed: HashMap<u64, (String, Observed)>,
    /// When unsaved changes were last made.
    dirty_since: Option<Instant>,
    writer: Option<Writer>,
}

impl WindowSizeMemory {
    pub fn new(mut path: Option<PathBuf>) -> Self {
        let mut saved = SavedState {
            version: 1,
            applications: BTreeMap::new(),
        };
        if let Some(file) = path.as_ref() {
            match fs::read(file) {
                Ok(bytes) => {
                    let loaded = (bytes.len() <= 1_048_576)
                        .then(|| serde_json::from_slice::<SavedState>(&bytes).ok())
                        .flatten()
                        .filter(|s| {
                            s.version == 1
                                && s.applications.len() <= MAX_APPLICATIONS
                                && s.applications
                                    .iter()
                                    .all(|(key, record)| valid_app_id(key) && record.is_valid())
                        });
                    if let Some(state) = loaded {
                        saved = state;
                    } else {
                        warn!(path = %file.display(), "invalid window size memory; preserving file");
                        path = None;
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => {
                    warn!(?err, path = %file.display(), "cannot load window size memory");
                    path = None;
                }
            }
        }
        Self {
            path,
            saved,
            observed: HashMap::new(),
            dirty_since: None,
            writer: None,
        }
    }

    pub fn default_path() -> Option<PathBuf> {
        let base = directories::BaseDirs::new()?;
        Some(base.state_dir()?.join("niri/window-sizes.json"))
    }

    pub fn record(&self, key: &str) -> Option<Record> {
        self.saved.applications.get(key).copied()
    }

    /// The remembered sizes for a window that is about to open.
    pub fn open_record(
        &self,
        rules: &ResolvedWindowRules,
        toplevel: &ToplevelSurface,
        enabled: bool,
    ) -> Option<Record> {
        self.record(&memory_key(toplevel, rules, enabled)?)
    }

    /// Takes every managed window; `None` means the window is alive but currently not in a
    /// rememberable normal state (maximized, fullscreen, excluded).
    ///
    /// The most recently *changed* window of an application wins: a window that merely opens
    /// with a different size does not replace an existing record.
    pub fn observe(&mut self, windows: impl IntoIterator<Item = (u64, Option<(String, Observed)>)>) {
        self.observe_at(windows, Instant::now());
    }

    fn observe_at(
        &mut self,
        windows: impl IntoIterator<Item = (u64, Option<(String, Observed)>)>,
        now: Instant,
    ) {
        let mut alive = HashSet::new();
        let mut changed = false;
        for (id, observation) in windows {
            alive.insert(id);
            let Some((key, value)) = observation else {
                continue;
            };
            if !valid_app_id(&key) {
                continue;
            }
            let valid = match value {
                Observed::Floating(size) => FloatingSize {
                    width: size.w,
                    height: size.h,
                }
                .is_valid(),
                Observed::Tiled { proportion, .. } => valid_proportion(proportion),
            };
            if !valid {
                continue;
            }

            let previous = self.observed.insert(id, (key.clone(), value));
            let adjusted = previous.is_some_and(|(k, v)| k == key && !v.same_as(&value));

            if !self.saved.applications.contains_key(&key)
                && self.saved.applications.len() >= MAX_APPLICATIONS
            {
                continue;
            }
            let record = self.saved.applications.entry(key).or_default();
            let slot_missing = match value {
                Observed::Floating(_) => record.floating.is_none(),
                Observed::Tiled { .. } => record.tiled_width.is_none(),
            };
            if !(adjusted || slot_missing) {
                continue;
            }
            let before = *record;
            match value {
                Observed::Floating(size) => {
                    record.floating = Some(FloatingSize {
                        width: size.w,
                        height: size.h,
                    })
                }
                Observed::Tiled {
                    proportion,
                    full_width,
                } => {
                    record.tiled_width = Some(proportion);
                    record.tiled_full_width = Some(full_width);
                }
            }
            changed |= *record != before;
        }

        let closed = self.observed.keys().any(|id| !alive.contains(id));
        self.observed.retain(|id, _| alive.contains(id));

        if changed {
            self.dirty_since = Some(now);
        }
        if let Some(since) = self.dirty_since {
            if closed || now.saturating_duration_since(since) >= SAVE_DELAY {
                self.persist();
            }
        }
    }

    pub fn reset_observations(&mut self) {
        self.observed.clear();
    }

    fn persist(&mut self) {
        self.dirty_since = None;
        let Some(path) = self.path.clone() else {
            return;
        };
        if self.writer.is_none() {
            let (tx, rx) = mpsc::channel::<SavedState>();
            let worker = thread::spawn(move || {
                while let Ok(mut latest) = rx.recv() {
                    while let Ok(next) = rx.try_recv() {
                        latest = next;
                    }
                    if let Err(err) = write_atomic(&path, &latest) {
                        warn!(?err, path = %path.display(), "cannot save window size memory");
                    }
                }
            });
            self.writer = Some(Writer {
                tx: Some(tx),
                thread: Some(worker),
            });
        }
        if let Some(tx) = self.writer.as_ref().and_then(|writer| writer.tx.as_ref()) {
            let _ = tx.send(self.saved.clone());
        }
    }
}

impl Drop for WindowSizeMemory {
    fn drop(&mut self) {
        // Save pending changes at orderly shutdown; the writer drains afterwards.
        if self.dirty_since.is_some() {
            self.persist();
        }
    }
}

/// The record key for a window, or `None` if its size should not be remembered.
///
/// With `remember-window-size` enabled, unparented resizable windows are remembered per app-id.
/// The `remember-window-size` window rule forces inclusion (true) or exclusion (false), and
/// `window-memory-group` keeps an auxiliary window's sizes separate from the main windows.
pub fn memory_key(
    toplevel: &ToplevelSurface,
    rules: &ResolvedWindowRules,
    enabled: bool,
) -> Option<String> {
    match rules.remember_window_size {
        Some(false) => return None,
        Some(true) => (),
        None => {
            if !enabled || toplevel.parent().is_some() || rules.open_fullscreen == Some(true) {
                return None;
            }
            let (min, max) = with_states(toplevel.wl_surface(), |states| {
                let mut guard = states.cached_state.get::<SurfaceCachedState>();
                let state = guard.current();
                rules.apply_min_max_size(state.min_size, state.max_size)
            });
            if (min.w > 0 && min.w == max.w) || (min.h > 0 && min.h == max.h) {
                return None;
            }
        }
    }
    let app_id = with_toplevel_role(toplevel, |role| role.app_id.clone())?;
    let key = match rules.window_memory_group.as_deref() {
        Some(group) if !group.is_empty() => format!("{app_id}#{group}"),
        _ => app_id,
    };
    valid_app_id(&key).then_some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floating(id: u64, key: &str, w: i32, h: i32) -> (u64, Option<(String, Observed)>) {
        (
            id,
            Some((key.to_owned(), Observed::Floating(Size::from((w, h))))),
        )
    }

    fn tiled(id: u64, key: &str, p: f64) -> (u64, Option<(String, Observed)>) {
        tiled_full(id, key, p, false)
    }

    fn tiled_full(id: u64, key: &str, p: f64, full: bool) -> (u64, Option<(String, Observed)>) {
        let observed = Observed::Tiled {
            proportion: p,
            full_width: full,
        };
        (id, Some((key.to_owned(), observed)))
    }

    fn floating_size(memory: &WindowSizeMemory, key: &str) -> Option<(i32, i32)> {
        memory
            .record(key)
            .and_then(|r| r.floating)
            .map(|s| (s.width, s.height))
    }

    #[test]
    fn window_size_memory_last_adjusted_window_wins() {
        let mut memory = WindowSizeMemory::new(None);
        memory.observe([floating(1, "app", 800, 500)]);
        // A second window opening at another size does not replace the record.
        memory.observe([floating(1, "app", 800, 500), floating(2, "app", 300, 200)]);
        assert_eq!(floating_size(&memory, "app"), Some((800, 500)));
        // Resizing the second window does.
        memory.observe([floating(1, "app", 800, 500), floating(2, "app", 640, 480)]);
        assert_eq!(floating_size(&memory, "app"), Some((640, 480)));
        // Then resizing the first one again.
        memory.observe([floating(1, "app", 900, 600), floating(2, "app", 640, 480)]);
        assert_eq!(floating_size(&memory, "app"), Some((900, 600)));
    }

    #[test]
    fn window_size_memory_keeps_floating_and_tiled_separately() {
        let mut memory = WindowSizeMemory::new(None);
        memory.observe([floating(1, "app", 800, 500)]);
        memory.observe([tiled(1, "app", 0.5)]);
        memory.observe([tiled(1, "app", 0.75)]);
        // Temporarily maximized/fullscreen windows are not learned.
        memory.observe([(1, None)]);
        memory.observe([tiled(1, "app", 0.75)]);
        let record = memory.record("app").unwrap();
        assert_eq!(floating_size(&memory, "app"), Some((800, 500)));
        assert_eq!(record.tiled_width, Some(0.75));
        assert_eq!(record.tiled_full_width, Some(false));
        // Going full width keeps the width it restores to.
        memory.observe([tiled_full(1, "app", 0.75, true)]);
        let record = memory.record("app").unwrap();
        assert_eq!(record.tiled_width, Some(0.75));
        assert_eq!(record.tiled_full_width, Some(true));
        memory.observe([tiled(1, "app", 0.75)]);
        // Invalid values and identities are ignored.
        memory.observe([tiled(1, "app", f64::NAN), floating(2, "", 10, 10)]);
        memory.observe([floating(3, "app", 0, 10)]);
        assert_eq!(memory.record("app").unwrap().tiled_width, Some(0.75));
        assert_eq!(memory.saved.applications.len(), 1);
    }

    #[test]
    fn window_size_memory_saves_on_close_delay_and_shutdown() {
        let shell = xshell::Shell::new().unwrap();
        let dir = shell.create_temp_dir().unwrap();
        let path = dir.path().join("niri/window-sizes.json");
        let start = Instant::now();

        // Changes while the window stays open are written after the delay.
        let mut memory = WindowSizeMemory::new(Some(path.clone()));
        for w in 500..600 {
            memory.observe_at([floating(1, "app", w, 400)], start);
        }
        drop(memory.writer.take());
        assert!(!path.exists());
        memory.observe_at([floating(1, "app", 599, 400)], start + SAVE_DELAY);
        drop(memory.writer.take());
        assert_eq!(
            floating_size(&WindowSizeMemory::new(Some(path.clone())), "app"),
            Some((599, 400))
        );

        // Closing a window writes immediately.
        memory.observe_at([tiled(1, "app", 0.5)], start + SAVE_DELAY);
        memory.observe_at([], start + SAVE_DELAY);
        drop(memory);
        let mut memory = WindowSizeMemory::new(Some(path.clone()));
        assert_eq!(memory.record("app").unwrap().tiled_width, Some(0.5));

        // Shutdown writes pending changes.
        memory.observe([floating(1, "app", 599, 400)]);
        memory.observe([floating(1, "app", 700, 450)]);
        drop(memory);
        let memory = WindowSizeMemory::new(Some(path.clone()));
        assert_eq!(floating_size(&memory, "app"), Some((700, 450)));
        assert_eq!(memory.record("app").unwrap().tiled_width, Some(0.5));
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn window_size_memory_preserves_corrupt_or_future_state_files() {
        let shell = xshell::Shell::new().unwrap();
        let dir = shell.create_temp_dir().unwrap();
        for contents in [
            "{broken",
            r#"{"version":2,"applications":{}}"#,
            r#"{"version":1,"applications":{"a":{"floating":{"width":0,"height":5}}}}"#,
            r#"{"version":1,"applications":{"a":{"tiled_width":9.0}}}"#,
        ] {
            let path = dir.path().join("window-sizes.json");
            fs::write(&path, contents).unwrap();
            let mut memory = WindowSizeMemory::new(Some(path.clone()));
            memory.observe([floating(1, "app", 300, 200)]);
            memory.observe([]);
            drop(memory);
            assert_eq!(fs::read_to_string(path).unwrap(), contents);
        }
    }
}

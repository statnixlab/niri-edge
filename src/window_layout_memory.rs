//! Persistent application layout choices. No window titles, contents or PIDs are stored.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use serde::{Deserialize, Serialize};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::{SurfaceCachedState, ToplevelSurface};

use crate::utils::with_toplevel_role;
use crate::window::ResolvedWindowRules;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowLayout {
    Tiled,
    Floating,
}

impl From<bool> for WindowLayout {
    fn from(floating: bool) -> Self {
        if floating {
            Self::Floating
        } else {
            Self::Tiled
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedState {
    version: u32,
    applications: BTreeMap<String, WindowLayout>,
}

struct Writer {
    tx: Option<Sender<SavedState>>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Writer {
    fn drop(&mut self) {
        // Drain the last queued state on orderly shutdown, after closing the producer.
        self.tx.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub struct WindowLayoutMemory {
    path: Option<PathBuf>,
    saved: SavedState,
    observed: HashMap<u64, (String, WindowLayout)>,
    writer: Option<Writer>,
}

impl WindowLayoutMemory {
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
                                && s.applications.len() <= 4096
                                && s.applications.keys().all(|id| valid_app_id(id))
                        });
                    if let Some(state) = loaded {
                        saved = state;
                    } else {
                        warn!(path = %file.display(), "invalid window layout memory; preserving file");
                        path = None;
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => {
                    warn!(?err, path = %file.display(), "cannot load window layout memory");
                    path = None;
                }
            }
        }
        Self {
            path,
            saved,
            observed: HashMap::new(),
            writer: None,
        }
    }

    pub fn default_path() -> Option<PathBuf> {
        let base = directories::BaseDirs::new()?;
        Some(base.state_dir()?.join("niri/window-layouts.json"))
    }

    pub fn preference(&self, app_id: &str) -> Option<bool> {
        self.saved
            .applications
            .get(app_id)
            .map(|mode| *mode == WindowLayout::Floating)
    }

    pub fn open_floating(
        &self,
        rules: &ResolvedWindowRules,
        toplevel: &ToplevelSurface,
        enabled: bool,
    ) -> bool {
        if enabled {
            if let Some(id) = rememberable_app_id(toplevel, rules) {
                if let Some(floating) = self.preference(&id) {
                    return floating;
                }
            }
        }
        rules.compute_open_floating(toplevel)
    }

    /// A missing observation means the window is temporarily fullscreen/maximized/ineligible.
    /// Keep its last normal mode while it is alive, without learning that temporary placement.
    pub fn observe(&mut self, windows: impl IntoIterator<Item = (u64, Option<(String, bool)>)>) {
        let mut alive = HashSet::new();
        let mut changed = false;
        for (id, normal) in windows {
            alive.insert(id);
            let Some((app_id, floating)) = normal else {
                continue;
            };
            if !valid_app_id(&app_id) {
                continue;
            }
            let mode = WindowLayout::from(floating);
            let previous = self.observed.insert(id, (app_id.clone(), mode));
            let selected = previous.is_some_and(|(app, old)| app == app_id && old != mode);
            // A newly opened second window must not overwrite an existing choice.
            if selected || !self.saved.applications.contains_key(&app_id) {
                if self.saved.applications.len() >= 4096
                    && !self.saved.applications.contains_key(&app_id)
                {
                    continue;
                }
                changed |= self.saved.applications.insert(app_id, mode) != Some(mode);
            }
        }
        self.observed.retain(|id, _| alive.contains(id));
        if changed {
            self.persist();
        }
    }

    pub fn reset_observations(&mut self) {
        self.observed.clear();
    }

    fn persist(&mut self) {
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
                        warn!(?err, path = %path.display(), "cannot save window layout memory");
                    }
                }
            });
            self.writer = Some(Writer {
                tx: Some(tx),
                thread: Some(worker),
            });
        }
        let _ = self
            .writer
            .as_ref()
            .unwrap()
            .tx
            .as_ref()
            .unwrap()
            .send(self.saved.clone());
    }
}

pub(crate) fn valid_app_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 512 && !id.chars().any(char::is_control)
}

pub fn rememberable_app_id(
    toplevel: &ToplevelSurface,
    rules: &ResolvedWindowRules,
) -> Option<String> {
    if toplevel.parent().is_some()
        || rules.remember_window_layout == Some(false)
        || rules.open_fullscreen == Some(true)
    {
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
    with_toplevel_role(toplevel, |role| role.app_id.clone()).filter(|id| valid_app_id(id))
}

pub(crate) fn write_atomic<T: Serialize>(path: &Path, state: &T) -> io::Result<()> {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing state directory"))?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)?;
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("state");
    let temp = parent.join(format!(
        ".{name}-{}-{}.tmp",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        serde_json::to_writer_pretty(&mut file, state)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        fs::File::open(parent)?.sync_all()
    })();
    let _ = fs::remove_file(temp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(id: u64, app: &str, floating: bool) -> (u64, Option<(String, bool)>) {
        (id, Some((app.to_owned(), floating)))
    }

    #[test]
    fn window_layout_memory_last_changed_window_wins_not_last_opened_or_closed() {
        let mut memory = WindowLayoutMemory::new(None);
        memory.observe([observation(1, "editor", false)]);
        memory.observe([observation(1, "editor", true)]);
        assert_eq!(memory.preference("editor"), Some(true));
        memory.observe([
            observation(1, "editor", true),
            observation(2, "editor", false),
        ]);
        assert_eq!(memory.preference("editor"), Some(true));
        memory.observe([
            observation(1, "editor", true),
            observation(2, "editor", true),
        ]);
        memory.observe([
            observation(1, "editor", false),
            observation(2, "editor", true),
        ]);
        assert_eq!(memory.preference("editor"), Some(false));
        memory.observe([observation(2, "editor", true)]);
        assert_eq!(memory.preference("editor"), Some(false));
        memory.observe([]);
        memory.observe([observation(3, "editor", true)]);
        assert_eq!(memory.preference("editor"), Some(false));
    }

    #[test]
    fn window_layout_memory_ignores_temporary_modes_and_invalid_identity() {
        let mut memory = WindowLayoutMemory::new(None);
        memory.observe([observation(1, "editor", true), observation(2, "", true)]);
        memory.observe([(1, None), observation(2, "bad\nidentity", true)]);
        assert_eq!(memory.preference("editor"), Some(true));
        assert_eq!(memory.saved.applications.len(), 1);
        memory.observe([observation(1, "editor", false)]);
        assert_eq!(memory.preference("editor"), Some(false));
    }

    #[test]
    fn window_layout_memory_flushes_latest_choice_and_reloads_after_shutdown() {
        let shell = xshell::Shell::new().unwrap();
        let dir = shell.create_temp_dir().unwrap();
        let path = dir.path().join("niri/window-layouts.json");
        let mut memory = WindowLayoutMemory::new(Some(path.clone()));
        for i in 0..101 {
            memory.observe([observation(1, "editor", i % 2 == 0)]);
        }
        drop(memory);
        let memory = WindowLayoutMemory::new(Some(path.clone()));
        assert_eq!(memory.preference("editor"), Some(true));
        let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(json["applications"]["editor"], "floating");
        assert_eq!(json.as_object().unwrap().len(), 2);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn window_layout_memory_preserves_corrupt_or_future_state_files() {
        let shell = xshell::Shell::new().unwrap();
        let dir = shell.create_temp_dir().unwrap();
        for contents in ["{broken", r#"{"version":2,"applications":{}}"#] {
            let path = dir.path().join("window-layouts.json");
            fs::write(&path, contents).unwrap();
            let mut memory = WindowLayoutMemory::new(Some(path.clone()));
            memory.observe([observation(1, "editor", true)]);
            drop(memory);
            assert_eq!(fs::read_to_string(path).unwrap(), contents);
        }
    }
}

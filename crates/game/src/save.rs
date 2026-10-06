//! Progress that survives a reload: best stars per lesson, chosen car and
//! gearbox. Browser `localStorage` on the web, a JSON file on desktop.

use bevy::prelude::*;
use drivetrain::{LessonId, Outcome, TransmissionMode};
use serde::{Deserialize, Serialize};

use crate::driving::Drive;
use crate::input::Bindings;

const KEY: &str = "miata-progress";

#[derive(Resource, Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Progress {
    /// Best stars per lesson, indexed like `LessonId::ALL`; 0 = not passed.
    pub best: Vec<u8>,
    pub car: usize,
    pub mode: Mode,
    pub bindings: Bindings,
}

/// Serializable mirror of `TransmissionMode` (the sim crate stays serde-free).
#[derive(Serialize, Deserialize, Default, Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    #[default]
    Manual,
    AutoClutch,
    Paddles,
}

impl From<TransmissionMode> for Mode {
    fn from(m: TransmissionMode) -> Self {
        match m {
            TransmissionMode::Manual => Mode::Manual,
            TransmissionMode::AutoClutch => Mode::AutoClutch,
            TransmissionMode::Paddles => Mode::Paddles,
        }
    }
}

impl From<Mode> for TransmissionMode {
    fn from(m: Mode) -> Self {
        match m {
            Mode::Manual => TransmissionMode::Manual,
            Mode::AutoClutch => TransmissionMode::AutoClutch,
            Mode::Paddles => TransmissionMode::Paddles,
        }
    }
}

impl Progress {
    pub fn stars(&self, id: LessonId) -> u8 {
        self.best.get(index(id)).copied().unwrap_or(0)
    }

    /// Keep the better of the old and new result. Returns whether it improved.
    pub fn record(&mut self, id: LessonId, stars: u8) -> bool {
        let i = index(id);
        if self.best.len() <= i {
            self.best.resize(i + 1, 0);
        }
        let improved = stars > self.best[i];
        if improved {
            self.best[i] = stars;
        }
        improved
    }
}

fn index(id: LessonId) -> usize {
    LessonId::ALL
        .iter()
        .position(|&l| l == id)
        .expect("every lesson is in ALL")
}

pub fn plugin(app: &mut App) {
    app.insert_resource(load())
        .add_systems(Startup, restore_choices)
        .add_systems(Update, (track, persist).chain());
}

fn restore_choices(
    progress: Res<Progress>,
    mut drive: ResMut<Drive>,
    mut bindings: ResMut<Bindings>,
) {
    *bindings = progress.bindings.clone();
    if progress.car < drive.cars.len() {
        drive.car = progress.car;
    }
    drive.mode = progress.mode.into();
}

fn track(drive: Res<Drive>, bindings: Res<Bindings>, mut progress: ResMut<Progress>) {
    if progress.bindings != *bindings {
        progress.bindings = bindings.clone();
    }
    if let (Some(run), Outcome::Passed { stars }) = (&drive.lesson, drive.outcome)
        && progress.stars(run.id) < stars
    {
        progress.record(run.id, stars);
    }
    let mode = Mode::from(drive.mode);
    if progress.car != drive.car || progress.mode != mode {
        progress.car = drive.car;
        progress.mode = mode;
    }
}

fn persist(progress: Res<Progress>) {
    if progress.is_changed() && !progress.is_added() {
        store(&progress);
    }
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
fn load() -> Progress {
    storage()
        .and_then(|s| s.get_item(KEY).ok()?)
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn store(p: &Progress) {
    if let (Some(s), Ok(json)) = (storage(), serde_json::to_string(p)) {
        let _ = s.set_item(KEY, &json);
    }
}

/// `~/Library/Application Support/miata` on macOS, `$XDG_CONFIG_HOME/miata`
/// or `~/.config/miata` elsewhere.
#[cfg(not(target_arch = "wasm32"))]
fn path() -> Option<std::path::PathBuf> {
    let home = std::path::PathBuf::from(std::env::var_os("HOME")?);
    let dir = if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map_or_else(|| home.join(".config"), Into::into)
    };
    Some(dir.join("miata").join(format!("{KEY}.json")))
}

#[cfg(not(target_arch = "wasm32"))]
fn load() -> Progress {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn store(p: &Progress) {
    let Some(path) = path() else { return };
    let write = || -> std::io::Result<()> {
        std::fs::create_dir_all(path.parent().expect("path has a parent"))?;
        std::fs::write(&path, serde_json::to_string_pretty(p)?)
    };
    if let Err(e) = write() {
        warn!("couldn't save progress to {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_best_result_and_round_trips() {
        let mut p = Progress::default();
        assert!(p.record(LessonId::PullAway, 2));
        assert!(!p.record(LessonId::PullAway, 1));
        assert!(p.record(LessonId::PullAway, 3));
        assert_eq!(p.stars(LessonId::PullAway), 3);
        assert_eq!(p.stars(LessonId::BitePoint), 0);
        p.mode = Mode::Paddles;
        let back: Progress = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn tolerates_old_or_partial_saves() {
        let p: Progress = serde_json::from_str(r#"{"best":[1]}"#).unwrap();
        assert_eq!(p.stars(LessonId::BitePoint), 1);
        assert_eq!(p.mode, Mode::Manual);
    }
}

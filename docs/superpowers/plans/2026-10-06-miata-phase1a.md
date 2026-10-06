# miata Phase 1a (vertical slice) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A playable browser build of a stick-shift trainer: NA Miata, manual gearbox, keyboard + gamepad, cockpit view, lessons 1–3, coffee cup, engine sound, deployed to GitHub Pages by CI.

**Architecture:** `crates/drivetrain` is pure Rust (serde + toml only): car data, a 1 ms-substep drivetrain sim, the coffee cup, and lesson judging, all unit-tested locally in seconds. `crates/game` is a Bevy 0.19 app that turns leafwing input into `drivetrain::Controls`, steps the sim each frame, and renders a primitive-built cockpit, egui HUD and a procedural engine synth. Every Bevy compile happens on GitHub Actions.

**Tech Stack:** Rust 1.95 (edition 2024), bevy 0.19.1, bevy_egui 0.42, leafwing-input-manager 0.21, serde 1, toml 1, wasm-bindgen (CLI matched to lockfile), GitHub Actions + Pages.

**Spec:** `docs/superpowers/specs/2026-10-06-miata-design.md`

## Global Constraints

- **No local Bevy compiles.** Locally only `cargo test -p drivetrain`. `cargo check/clippy/test/build` of anything touching `crates/game` runs on GitHub Actions (user's machine; CPU spikes are not acceptable).
- **The agent never runs `git commit`/`push`/`tag`.** Each task ends with a checkpoint the user runs. Personal repo: `lubabs770/miata`, commit email `246544701+lubabs770@users.noreply.github.com`; confirm `gh auth status` shows `lubabs770` active before any push.
- Pinned versions: `bevy = "0.19.1"`, `bevy_egui = "0.42"`, `leafwing-input-manager = "0.21"`. Bevy 0.20 is RC only — do not upgrade.
- `drivetrain` must not depend on Bevy. The game reads physics only through `Sim::step`, `Sim::state()`, `Sim::bite_point()`, `Sim::set_moving()`.
- Phase 1a scope only: one car (Miata), Manual mode, cockpit view, lessons 1–3. No rebinding UI, other views, other cars, persistence, release builds (all Phase 1b).
- CI must pass `cargo fmt --check` and `cargo clippy -- -D warnings`.
- In-game honesty note (spec): pad/keyboard trains sequence and timing; pedal feel needs real pedals.

## File map

| File | Responsibility |
|---|---|
| `Cargo.toml` | Workspace; dev-profile dep optimisation for Bevy |
| `cars/miata.toml` | NA Miata data (published ratios + tuned feel values) |
| `crates/drivetrain/src/car.rs` | `CarSpec`: TOML load, torque curve, ratios |
| `crates/drivetrain/src/sim.rs` | `Sim`, `Controls`, `Env`, `Event`, `SimState`: engine, clutch, gearbox, vehicle |
| `crates/drivetrain/src/score.rs` | `Cup` (coffee spill from jerk), `stars()` |
| `crates/drivetrain/src/lesson.rs` | `LessonId`, `LessonRun`, `Outcome`: setup, judging, hints |
| `crates/game/src/main.rs` | App, `AppState` (ClickToStart → Driving), plugin wiring |
| `crates/game/src/input.rs` | leafwing `Action`s, default bindings, `Pedals` resource |
| `crates/game/src/driving.rs` | `Drive` resource; per-frame sim + lesson step |
| `crates/game/src/cockpit.rs` | World + cockpit meshes, gauge/wheel/knob/cup animation |
| `crates/game/src/hud.rs` | egui start screen, instruments, hints, lesson panel |
| `crates/game/src/audio.rs` | Procedural engine `Decodable` fed via atomics |
| `web/index.html` | wasm loader + AudioContext resume-on-gesture shim |
| `.github/workflows/ci.yml` | fmt/clippy/test, wasm bundle, Pages deploy on `main` |

---

### Task 1: Workspace and car data

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `.cargo/config.toml`, `cars/miata.toml`
- Create: `crates/drivetrain/Cargo.toml`, `crates/drivetrain/src/lib.rs`, `crates/drivetrain/src/car.rs`
- Test: `crates/drivetrain/tests/car.rs`

**Interfaces:**
- Produces: `CarSpec { name, mass_kg, wheelbase_m, tire_radius_m, idle_rpm, stall_rpm, redline_rpm, limiter_rpm, torque_curve: Vec<[f32;2]>, engine_inertia, friction_nm, friction_per_rad_s, idle_base_throttle, idle_gain, idle_max_throttle, gear_ratios: Vec<f32>, reverse_ratio, final_drive, clutch_capacity_nm, bite_point, bite_width, bite_variance, drag_cd_a, rolling_c, brake_force_n, handbrake_force_n, max_steer_rad }` (all `f32` except noted); `CarSpec::from_toml(&str) -> Result<CarSpec, toml::de::Error>`, `CarSpec::miata() -> CarSpec`, `torque_at(rpm: f32) -> f32`, `ratio(gear: i8) -> Option<f32>` (gearbox × final drive, reverse negative, neutral/missing `None`), `top_gear() -> i8`.

- [ ] **Step 0: User initialises the repo** (personal identity, before any commit):

```bash
mkdir -p ~/code/miata && cd ~/code/miata
git init -b main
git config user.email 246544701+lubabs770@users.noreply.github.com
```

- [ ] **Step 1: Workspace files**

`Cargo.toml` (game crate is added in Task 5):

```toml
[workspace]
resolver = "3"
members = ["crates/drivetrain"]

[workspace.package]
edition = "2024"
rust-version = "1.95"

# Bevy is slow unoptimised; optimise dependencies even in dev builds.
[profile.dev.package."*"]
opt-level = 2
```

`.gitignore`:

```
/target
/dist
```

`.cargo/config.toml`:

```toml
# getrandom 0.3 needs this cfg to use the browser's crypto API on wasm.
[target.wasm32-unknown-unknown]
rustflags = ['--cfg', 'getrandom_backend="wasm_js"']
```

`crates/drivetrain/Cargo.toml`:

```toml
[package]
name = "drivetrain"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }
toml = "1"
```

`cars/miata.toml` (ratios/final drive/idle/redline/mass are published NA 1.6 figures; inertia, friction, idle-controller and clutch values were tuned so the physics tests in Task 2 pass — change them only together with those tests):

```toml
# 1990 Mazda MX-5 (NA) 1.6, 5-speed. Published figures where known; feel values tuned.
name = "NA Miata 1.6"
mass_kg = 960.0
wheelbase_m = 2.265
tire_radius_m = 0.29
idle_rpm = 850.0
stall_rpm = 400.0
redline_rpm = 7000.0
limiter_rpm = 7200.0
# [rpm, Nm] at full throttle
torque_curve = [[500.0, 70.0], [1000.0, 100.0], [2500.0, 118.0], [4000.0, 128.0], [5500.0, 136.0], [6500.0, 128.0], [7200.0, 115.0]]
engine_inertia = 0.09
friction_nm = 8.0
friction_per_rad_s = 0.04
idle_base_throttle = 0.06
idle_gain = 2.0
idle_max_throttle = 0.2
gear_ratios = [3.136, 1.888, 1.330, 1.000, 0.814]
reverse_ratio = 3.758
final_drive = 4.30
clutch_capacity_nm = 220.0
# pedal travel, 0 = released, 1 = floored. Clutch starts to grab at bite_point
# and is fully engaged bite_width below it.
bite_point = 0.55
bite_width = 0.18
bite_variance = 0.08
drag_cd_a = 0.62
rolling_c = 0.012
brake_force_n = 9000.0
handbrake_force_n = 4000.0
max_steer_rad = 0.6
```

- [ ] **Step 2: Write the failing test** — `crates/drivetrain/tests/car.rs`:

```rust
use drivetrain::CarSpec;

#[test]
fn miata_loads_with_published_ratios() {
    let c = CarSpec::miata();
    assert_eq!(c.top_gear(), 5);
    assert!((c.ratio(1).unwrap() - 3.136 * 4.30).abs() < 1e-4);
    assert!(c.ratio(-1).unwrap() < 0.0, "reverse spins the wheels backwards");
    assert_eq!(c.ratio(0), None);
    assert_eq!(c.ratio(6), None);
}

#[test]
fn torque_curve_interpolates_and_clamps() {
    let c = CarSpec::miata();
    assert_eq!(c.torque_at(0.0), 70.0);
    assert_eq!(c.torque_at(9000.0), 115.0);
    assert!((c.torque_at(4750.0) - 132.0).abs() < 1e-3);
}

#[test]
fn every_bundled_car_file_parses() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../cars");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        CarSpec::from_toml(&text).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    }
}
```

- [ ] **Step 3: Run to confirm it fails**

Run: `cargo test -p drivetrain`
Expected: compile error, `CarSpec` not found.

- [ ] **Step 4: Implement** — `crates/drivetrain/src/lib.rs`:

```rust
//! Pure-Rust manual-transmission car model. No Bevy; the game drives it via
//! `Sim::step` and reads `SimState`.
mod car;

pub use car::CarSpec;
```

`crates/drivetrain/src/car.rs`:

```rust
use serde::Deserialize;

/// One car, loaded from `cars/*.toml`. Units are SI; pedal values are 0..1.
#[derive(Debug, Clone, Deserialize)]
pub struct CarSpec {
    pub name: String,
    pub mass_kg: f32,
    pub wheelbase_m: f32,
    pub tire_radius_m: f32,
    pub idle_rpm: f32,
    pub stall_rpm: f32,
    pub redline_rpm: f32,
    pub limiter_rpm: f32,
    /// `[rpm, Nm]` pairs at full throttle, ascending rpm.
    pub torque_curve: Vec<[f32; 2]>,
    pub engine_inertia: f32,
    pub friction_nm: f32,
    pub friction_per_rad_s: f32,
    pub idle_base_throttle: f32,
    pub idle_gain: f32,
    pub idle_max_throttle: f32,
    pub gear_ratios: Vec<f32>,
    pub reverse_ratio: f32,
    pub final_drive: f32,
    pub clutch_capacity_nm: f32,
    pub bite_point: f32,
    pub bite_width: f32,
    pub bite_variance: f32,
    pub drag_cd_a: f32,
    pub rolling_c: f32,
    pub brake_force_n: f32,
    pub handbrake_force_n: f32,
    pub max_steer_rad: f32,
}

impl CarSpec {
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    pub fn miata() -> Self {
        Self::from_toml(include_str!("../../../cars/miata.toml"))
            .expect("bundled miata.toml is valid")
    }

    /// Full-throttle torque at `rpm`, linearly interpolated, clamped at the curve ends.
    pub fn torque_at(&self, rpm: f32) -> f32 {
        let c = &self.torque_curve;
        if rpm <= c[0][0] {
            return c[0][1];
        }
        for w in c.windows(2) {
            let ([r0, t0], [r1, t1]) = (w[0], w[1]);
            if rpm <= r1 {
                return t0 + (t1 - t0) * (rpm - r0) / (r1 - r0);
            }
        }
        c[c.len() - 1][1]
    }

    /// Overall ratio (gearbox × final drive). Reverse is negative; neutral and
    /// gears the car doesn't have are `None`.
    pub fn ratio(&self, gear: i8) -> Option<f32> {
        match gear {
            -1 => Some(-self.reverse_ratio * self.final_drive),
            g if g >= 1 => self
                .gear_ratios
                .get(g as usize - 1)
                .map(|r| r * self.final_drive),
            _ => None,
        }
    }

    pub fn top_gear(&self) -> i8 {
        self.gear_ratios.len() as i8
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p drivetrain`
Expected: 3 passed.

- [ ] **Step 6:** **Checkpoint — the user commits** (the agent does not run git):

```bash
git add Cargo.toml Cargo.lock .gitignore .cargo cars crates/drivetrain
git commit -m "feat(drivetrain): car spec and NA Miata data"
```

---

### Task 2: Drivetrain simulation

**Files:**
- Create: `crates/drivetrain/src/sim.rs`
- Modify: `crates/drivetrain/src/lib.rs`
- Test: `crates/drivetrain/tests/physics.rs`

**Interfaces:**
- Consumes: `CarSpec` (Task 1).
- Produces: `Controls { clutch, throttle, brake, handbrake, steer: f32, shift: Option<i8>, ignition: bool }` (pedals 0..1, 1 = floored; steer +left; `shift` = gear the lever moved to *this frame*, −1 R / 0 N / 1.. forward; `ignition` = key turned this frame); `Env { grade: f32 }`; `Event::{Started, Stalled, Grind, OverRev, Shifted(i8)}`; `SimState { engine_rpm, speed_mps, gear: i8, engine_running: bool, clutch_engagement, clutch_locked: bool, clutch_heat_j, accel, jerk, x, y, heading, rollback_m, distance_m }`; `Sim::new(CarSpec, seed: u64)`, `Sim::step(&mut self, &Controls, Env, dt: f32) -> Vec<Event>`, `Sim::state(&self) -> &SimState`, `Sim::bite_point(&self) -> f32`, `Sim::set_moving(&mut self, gear: i8, speed_mps: f32)`, public field `Sim::car: CarSpec`; consts `RPM_PER_RAD_S`, `SHIFT_CLUTCH = 0.85`.

Model notes (so the reviewer can check intent): engine torque = torque curve × progressive pedal map `1-(1-p)²`, with an idle controller of limited authority (`idle_max_throttle`) — enough to creep away very gently, not enough to survive a dumped clutch. Clutch capacity = `engagement² × clutch_capacity_nm`, engagement linear over `bite_width` below this session's bite point. Locked clutch = engine and car as one inertia; unlocks when needed torque exceeds capacity; relocks when slip crosses zero. Brakes and rolling resistance hold a stopped car and never push it backwards. Spec deviation: rollback is a running total `SimState::rollback_m` rather than an event (lessons read it directly).

- [ ] **Step 1: Write the failing tests** — `crates/drivetrain/tests/physics.rs`:

```rust
use drivetrain::*;

const DT: f32 = 1.0 / 60.0;

/// Run `secs` of frames, building each frame's controls from elapsed time.
fn drive(sim: &mut Sim, secs: f32, env: Env, mut f: impl FnMut(f32) -> Controls) -> Vec<Event> {
    let mut all = Vec::new();
    let mut t = 0.0;
    while t < secs {
        all.extend(sim.step(&f(t), env, DT));
        t += DT;
    }
    all
}

fn in_first(seed: u64) -> Sim {
    let mut sim = Sim::new(CarSpec::miata(), seed);
    let ev = sim.step(
        &Controls {
            clutch: 1.0,
            shift: Some(1),
            ..Default::default()
        },
        Env::default(),
        DT,
    );
    assert!(ev.contains(&Event::Shifted(1)));
    sim
}

#[test]
fn idles_in_neutral() {
    let mut sim = Sim::new(CarSpec::miata(), 1);
    drive(&mut sim, 3.0, Env::default(), |_| Controls::default());
    let rpm = sim.state().engine_rpm;
    assert!((800.0..900.0).contains(&rpm), "idle rpm {rpm}");
}

#[test]
fn very_slow_release_at_idle_pulls_away() {
    for seed in 0..5 {
        let mut sim = in_first(seed);
        let top = sim.bite_point() + 0.02;
        // Quickly up to the bite point, then ~6 s through the engagement band, no gas.
        let ev = drive(&mut sim, 9.0, Env::default(), |t| Controls {
            clutch: if t < 0.5 {
                1.0 - (1.0 - top) * t / 0.5
            } else {
                (top - (t - 0.5) / 8.0 * 0.25).max(0.0)
            },
            ..Default::default()
        });
        assert!(!ev.contains(&Event::Stalled), "seed {seed} stalled");
        assert!(
            sim.state().speed_mps > 1.5,
            "seed {seed} speed {}",
            sim.state().speed_mps
        );
        assert!(sim.state().clutch_locked);
    }
}

#[test]
fn normal_release_with_some_gas_pulls_away() {
    for seed in 0..5 {
        let mut sim = in_first(seed);
        let top = sim.bite_point() + 0.02;
        // 1.5 s through the band holding ~2000 rpm worth of throttle.
        let ev = drive(&mut sim, 4.0, Env::default(), |t| Controls {
            clutch: (top - t / 1.5 * 0.22).max(0.0),
            throttle: 0.25,
            ..Default::default()
        });
        assert!(!ev.contains(&Event::Stalled), "seed {seed} stalled");
        assert!(
            sim.state().speed_mps > 3.0,
            "seed {seed} speed {}",
            sim.state().speed_mps
        );
    }
}

#[test]
fn clutch_dump_at_idle_stalls() {
    let mut sim = in_first(0);
    let ev = drive(&mut sim, 1.0, Env::default(), |_| Controls::default());
    assert!(ev.contains(&Event::Stalled));
    assert!(!sim.state().engine_running);
}

#[test]
fn handbrake_release_on_hill_rolls_back() {
    let hill = Env { grade: 0.10 };
    let mut sim = Sim::new(CarSpec::miata(), 0);
    drive(&mut sim, 1.0, hill, |_| Controls {
        handbrake: 1.0,
        ..Default::default()
    });
    assert_eq!(sim.state().rollback_m, 0.0, "handbrake holds");
    drive(&mut sim, 2.0, hill, |_| Controls::default());
    assert!(
        sim.state().rollback_m > 1.0,
        "rolled {}",
        sim.state().rollback_m
    );
}

#[test]
fn shift_without_clutch_grinds() {
    let mut sim = Sim::new(CarSpec::miata(), 0);
    let ev = sim.step(
        &Controls {
            shift: Some(1),
            ..Default::default()
        },
        Env::default(),
        DT,
    );
    assert_eq!(ev, vec![Event::Grind]);
    assert_eq!(sim.state().gear, 0);
}

#[test]
fn money_shift_flags_over_rev() {
    let mut sim = Sim::new(CarSpec::miata(), 0);
    sim.set_moving(5, 100.0 / 3.6);
    let ev = sim.step(
        &Controls {
            clutch: 1.0,
            shift: Some(2),
            ..Default::default()
        },
        Env::default(),
        DT,
    );
    assert!(ev.contains(&Event::OverRev));
    assert_eq!(sim.state().gear, 2);
}

#[test]
fn braking_to_stop_in_gear_stalls_but_clutch_in_does_not() {
    let mut sim = Sim::new(CarSpec::miata(), 0);
    sim.set_moving(2, 30.0 / 3.6);
    let ev = drive(&mut sim, 5.0, Env::default(), |_| Controls {
        brake: 0.6,
        ..Default::default()
    });
    assert!(ev.contains(&Event::Stalled));

    let mut sim = Sim::new(CarSpec::miata(), 0);
    sim.set_moving(2, 30.0 / 3.6);
    let ev = drive(&mut sim, 5.0, Env::default(), |_| Controls {
        brake: 0.6,
        clutch: 1.0,
        ..Default::default()
    });
    assert!(!ev.contains(&Event::Stalled));
    assert_eq!(sim.state().speed_mps, 0.0);
}

#[test]
fn full_throttle_pulls_to_limiter_in_first() {
    let mut sim = in_first(0);
    drive(&mut sim, 6.0, Env::default(), |t| Controls {
        throttle: 1.0,
        clutch: (1.0 - t).max(0.0),
        ..Default::default()
    });
    let s = sim.state();
    assert!(s.engine_running);
    assert!(
        s.engine_rpm > 6500.0 && s.engine_rpm < 7300.0,
        "rpm {}",
        s.engine_rpm
    );
}

#[test]
fn restart_needs_clutch_in_when_in_gear() {
    let mut sim = in_first(0);
    drive(&mut sim, 1.0, Env::default(), |_| Controls::default());
    let ev = sim.step(
        &Controls {
            ignition: true,
            ..Default::default()
        },
        Env::default(),
        DT,
    );
    assert!(!ev.contains(&Event::Started));
    let ev = sim.step(
        &Controls {
            ignition: true,
            clutch: 1.0,
            ..Default::default()
        },
        Env::default(),
        DT,
    );
    assert!(ev.contains(&Event::Started));
}

#[test]
fn zero_dt_is_a_no_op() {
    let mut sim = Sim::new(CarSpec::miata(), 0);
    assert!(
        sim.step(&Controls::default(), Env::default(), 0.0)
            .is_empty()
    );
    assert!(sim.state().accel.is_finite());
}
```

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p drivetrain --test physics`
Expected: compile error, `Sim` not found.

- [ ] **Step 3: Implement** — `crates/drivetrain/src/lib.rs`:

```rust
//! Pure-Rust manual-transmission car model. No Bevy; the game drives it via
//! `Sim::step` and reads `SimState`.
mod car;
mod sim;

pub use car::CarSpec;
pub use sim::{Controls, Env, Event, RPM_PER_RAD_S, SHIFT_CLUTCH, Sim, SimState};
```

`crates/drivetrain/src/sim.rs`:

```rust
use crate::car::CarSpec;

pub const RPM_PER_RAD_S: f32 = 60.0 / std::f32::consts::TAU;
const SUBSTEP_S: f32 = 0.001;
const G: f32 = 9.81;
const AIR_DENSITY: f32 = 1.2;
/// Clutch pedal travel needed before the gearbox will change gear.
pub const SHIFT_CLUTCH: f32 = 0.85;
/// Reverse only engages below this speed (~2 km/h).
const REVERSE_MAX_MPS: f32 = 0.56;
const JERK_SMOOTHING_S: f32 = 0.05;

/// One frame of driver input. Pedals 0..1 (1 = floored), steer -1..1.
#[derive(Debug, Clone, Copy, Default)]
pub struct Controls {
    pub clutch: f32,
    pub throttle: f32,
    pub brake: f32,
    pub handbrake: f32,
    pub steer: f32,
    /// Gear the driver moved the lever to this frame: -1 R, 0 N, 1.. forward.
    pub shift: Option<i8>,
    /// Key turned this frame.
    pub ignition: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Env {
    /// Road slope as rise/run; positive = uphill in the car's forward direction.
    pub grade: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Started,
    Stalled,
    Grind,
    OverRev,
    Shifted(i8),
}

/// Read-only snapshot for the game, lessons and scoring.
#[derive(Debug, Clone, Copy, Default)]
pub struct SimState {
    pub engine_rpm: f32,
    /// Signed; negative = rolling backwards.
    pub speed_mps: f32,
    pub gear: i8,
    pub engine_running: bool,
    /// 0 = clutch fully disengaged, 1 = fully engaged.
    pub clutch_engagement: f32,
    pub clutch_locked: bool,
    pub clutch_heat_j: f32,
    pub accel: f32,
    /// Low-passed rate of change of `accel`, m/s³.
    pub jerk: f32,
    pub x: f32,
    pub y: f32,
    pub heading: f32,
    /// Distance rolled backwards while not in reverse.
    pub rollback_m: f32,
    pub distance_m: f32,
}

pub struct Sim {
    pub car: CarSpec,
    bite: f32,
    we: f32,
    v: f32,
    locked: bool,
    state: SimState,
}

impl Sim {
    /// Engine running at idle, in neutral, stopped. `seed` picks this
    /// session's bite point within the car's variance.
    pub fn new(car: CarSpec, seed: u64) -> Self {
        let bite = car.bite_point + car.bite_variance * unit_noise(seed);
        let we = car.idle_rpm / RPM_PER_RAD_S;
        let state = SimState {
            engine_running: true,
            engine_rpm: car.idle_rpm,
            ..SimState::default()
        };
        Self {
            car,
            bite,
            we,
            v: 0.0,
            locked: false,
            state,
        }
    }

    pub fn state(&self) -> &SimState {
        &self.state
    }

    pub fn bite_point(&self) -> f32 {
        self.bite
    }

    /// Lesson setup: put the car in `gear` at `speed_mps`. When moving, the
    /// clutch starts locked; when stopped, the engine keeps idling.
    pub fn set_moving(&mut self, gear: i8, speed_mps: f32) {
        self.state.gear = gear;
        self.v = speed_mps;
        if let Some(k) = self.k(gear).filter(|_| speed_mps != 0.0) {
            self.we = k * speed_mps;
            self.locked = true;
        }
        self.sync_state();
    }

    /// Advance by one frame. `dt` is capped at 0.1 s so a backgrounded tab
    /// doesn't run a huge catch-up step.
    pub fn step(&mut self, c: &Controls, env: Env, dt: f32) -> Vec<Event> {
        let mut ev = Vec::new();
        let dt = dt.min(0.1);
        if dt <= 0.0 {
            return ev;
        }
        if c.ignition
            && !self.state.engine_running
            && (self.state.gear == 0 || c.clutch >= SHIFT_CLUTCH)
        {
            self.state.engine_running = true;
            self.we = self.car.idle_rpm / RPM_PER_RAD_S;
            self.locked = false;
            ev.push(Event::Started);
        }
        if let Some(g) = c.shift.filter(|&g| g != self.state.gear) {
            self.try_shift(g, c.clutch, &mut ev);
        }

        let (v0, a0) = (self.v, self.state.accel);
        let n = ((dt / SUBSTEP_S).round() as usize).max(1);
        let h = dt / n as f32;
        for _ in 0..n {
            self.substep(c, env, h, &mut ev);
        }

        let accel = (self.v - v0) / dt;
        let raw_jerk = (accel - a0) / dt;
        let alpha = dt / (JERK_SMOOTHING_S + dt);
        self.state.jerk += alpha * (raw_jerk - self.state.jerk);
        self.state.accel = accel;
        self.sync_state();
        ev
    }

    fn k(&self, gear: i8) -> Option<f32> {
        self.car.ratio(gear).map(|r| r / self.car.tire_radius_m)
    }

    fn try_shift(&mut self, g: i8, clutch: f32, ev: &mut Vec<Event>) {
        // Pulling into neutral works without the clutch, like a real box.
        if g == 0 {
            self.state.gear = 0;
            self.locked = false;
            ev.push(Event::Shifted(0));
            return;
        }
        let Some(k) = self.k(g) else { return };
        if clutch < SHIFT_CLUTCH || (g == -1 && self.v.abs() > REVERSE_MAX_MPS) {
            ev.push(Event::Grind);
            return;
        }
        if (k * self.v * RPM_PER_RAD_S).abs() > self.car.limiter_rpm {
            ev.push(Event::OverRev);
        }
        self.state.gear = g;
        self.locked = false;
        ev.push(Event::Shifted(g));
    }

    fn substep(&mut self, c: &Controls, env: Env, h: f32, ev: &mut Vec<Event>) {
        let car = &self.car;
        let rpm = self.we * RPM_PER_RAD_S;
        let running = self.state.engine_running;

        let combustion = if running && rpm < car.limiter_rpm {
            let idle = (car.idle_base_throttle
                + car.idle_gain * (car.idle_rpm - rpm) / car.idle_rpm)
                .clamp(0.0, car.idle_max_throttle);
            // Part throttle makes more than proportional torque, as a real intake does.
            let pedal = c.throttle.max(idle);
            (1.0 - (1.0 - pedal) * (1.0 - pedal)) * car.torque_at(rpm)
        } else {
            0.0
        };
        let friction = if self.we.abs() < 1e-3 {
            0.0
        } else {
            car.friction_nm * self.we.signum() + car.friction_per_rad_s * self.we
        };
        let te = combustion - friction;
        let je = car.engine_inertia;
        let m = car.mass_kg;

        let theta = env.grade.atan();
        let f_ext =
            -m * G * theta.sin() - 0.5 * AIR_DENSITY * car.drag_cd_a * self.v * self.v.abs();
        // Rolling resistance behaves like a weak brake: it never pushes the car.
        let f_brake = c.brake * car.brake_force_n
            + c.handbrake * car.handbrake_force_n
            + car.rolling_c * m * G * theta.cos();

        let engagement = ((self.bite - c.clutch) / car.bite_width).clamp(0.0, 1.0);
        // Progressive: the first part of the band grabs gently, like a real clutch.
        let cap = engagement * engagement * car.clutch_capacity_nm;

        match self.k(self.state.gear) {
            Some(k) if cap > 0.0 => {
                if self.locked {
                    let a = brake_accel(self.v, te * k + f_ext, f_brake, m + je * k * k, h);
                    let tc = te - je * k * a;
                    if tc.abs() <= cap {
                        self.v += a * h;
                        self.we = k * self.v;
                    } else {
                        self.locked = false;
                    }
                }
                if !self.locked {
                    let slip = self.we - k * self.v;
                    let tc = if slip == 0.0 {
                        0.0
                    } else {
                        cap * slip.signum()
                    };
                    self.we += (te - tc) / je * h;
                    self.v += brake_accel(self.v, tc * k + f_ext, f_brake, m, h) * h;
                    self.state.clutch_heat_j += (slip * tc).abs() * h;
                    let new_slip = self.we - k * self.v;
                    if new_slip.signum() != slip.signum() || new_slip.abs() < 0.5 {
                        let a = brake_accel(self.v, te * k + f_ext, f_brake, m + je * k * k, h);
                        if (te - je * k * a).abs() <= cap {
                            self.locked = true;
                            self.we = k * self.v;
                        }
                    }
                }
            }
            _ => {
                self.locked = false;
                self.we += te / je * h;
                self.v += brake_accel(self.v, f_ext, f_brake, m, h) * h;
            }
        }

        if !self.locked && self.we < 0.0 {
            self.we = 0.0;
        }
        if running && self.we * RPM_PER_RAD_S < car.stall_rpm {
            self.state.engine_running = false;
            ev.push(Event::Stalled);
        }
        if !self.state.engine_running && !self.locked && self.we < 1.0 {
            self.we = 0.0;
        }

        let s = &mut self.state;
        s.heading += self.v / car.wheelbase_m * (c.steer * car.max_steer_rad).tan() * h;
        s.x += self.v * s.heading.cos() * h;
        s.y += self.v * s.heading.sin() * h;
        s.distance_m += self.v.abs() * h;
        if self.v < 0.0 && s.gear != -1 {
            s.rollback_m -= self.v * h;
        }
        s.clutch_engagement = engagement;
    }

    fn sync_state(&mut self) {
        self.state.engine_rpm = self.we * RPM_PER_RAD_S;
        self.state.speed_mps = self.v;
        self.state.clutch_locked = self.locked;
    }
}

/// Acceleration from net force `f` with up to `fb` of brake/rolling force
/// opposing motion. Brakes hold a stopped car and never reverse it.
fn brake_accel(v: f32, f: f32, fb: f32, m: f32, h: f32) -> f32 {
    if v.abs() < 1e-3 {
        return if f.abs() <= fb {
            -v / h
        } else {
            (f - fb * f.signum()) / m
        };
    }
    let a = (f - fb * v.signum()) / m;
    if (v + a * h).signum() != v.signum() && fb * v.signum() * a < 0.0 {
        return -v / h;
    }
    a
}

/// Deterministic value in -1..1 from a seed (splitmix64).
fn unit_noise(seed: u64) -> f32 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 23) as f32 - 1.0
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p drivetrain`
Expected: 14 passed (3 car + 11 physics). If a launch test fails after touching `cars/miata.toml`, tune the car values, not the test thresholds — the thresholds encode real-car behaviour.

- [ ] **Step 5:** **Checkpoint — the user commits** (the agent does not run git):

```bash
git add crates/drivetrain
git commit -m "feat(drivetrain): engine, clutch, gearbox and vehicle sim"
```

---

### Task 3: Coffee cup and lessons 1–3

**Files:**
- Create: `crates/drivetrain/src/score.rs`, `crates/drivetrain/src/lesson.rs`
- Modify: `crates/drivetrain/src/lib.rs`
- Test: `crates/drivetrain/tests/lessons.rs`

**Interfaces:**
- Consumes: `Sim`, `SimState`, `Controls`, `Env`, `Event` (Task 2).
- Produces: `Cup { level: f32 }` with `Cup::default()` (full) and `Cup::update(&mut self, jerk: f32, dt: f32) -> f32`; `stars(grinds: u32, &Cup, clutch_heat_j: f32) -> u8`; `LessonId::{BitePoint, PullAway, Stop}` with `ALL`, `title()`, `brief()`; `Outcome::{Running, Passed { stars: u8 }, Failed(&'static str)}`; `LessonRun { pub id, pub cup, pub grinds }` with `new(LessonId)`, `setup(&self, &mut Sim) -> bool` (returns handbrake-starts-on), `env()`, `update(&mut self, &SimState, &[Event], dt) -> Outcome`, `hint(&self, &SimState, &Controls) -> Option<&'static str>`.

- [ ] **Step 1: Write the failing tests** — `crates/drivetrain/tests/lessons.rs` (scripted input traces; the spec's "recorded traces" are these scripts):

```rust
use drivetrain::*;

const DT: f32 = 1.0 / 60.0;

/// Play a lesson with scripted inputs until it ends or `max_s` passes.
fn play(id: LessonId, max_s: f32, mut f: impl FnMut(f32, &Sim) -> Controls) -> Outcome {
    let mut sim = Sim::new(CarSpec::miata(), 3);
    let mut run = LessonRun::new(id);
    run.setup(&mut sim);
    let mut t = 0.0;
    while t < max_s {
        let c = f(t, &sim);
        let ev = sim.step(&c, run.env(), DT);
        match run.update(sim.state(), &ev, DT) {
            Outcome::Running => {}
            done => return done,
        }
        t += DT;
    }
    Outcome::Running
}

#[test]
fn bite_point_held_passes() {
    let out = play(LessonId::BitePoint, 6.0, |_, sim| Controls {
        clutch: sim.bite_point() - 0.3 * sim.car.bite_width,
        handbrake: 1.0,
        ..Default::default()
    });
    assert!(matches!(out, Outcome::Passed { .. }), "{out:?}");
}

#[test]
fn bite_point_released_fully_fails() {
    let out = play(LessonId::BitePoint, 6.0, |_, _| Controls {
        handbrake: 1.0,
        ..Default::default()
    });
    assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
}

#[test]
fn pull_away_clean_passes_with_three_stars() {
    let out = play(LessonId::PullAway, 8.0, |t, sim| {
        let top = sim.bite_point() + 0.02;
        Controls {
            clutch: if t < 0.5 {
                1.0
            } else {
                (top - (t - 0.5) / 1.5 * 0.22).max(0.0)
            },
            throttle: if t < 0.5 { 0.0 } else { 0.25 },
            shift: (t < DT).then_some(1),
            ..Default::default()
        }
    });
    assert_eq!(out, Outcome::Passed { stars: 3 });
}

#[test]
fn pull_away_clutch_dump_fails() {
    let out = play(LessonId::PullAway, 4.0, |t, _| Controls {
        clutch: if t < 0.5 { 1.0 } else { 0.0 },
        shift: (t < DT).then_some(1),
        ..Default::default()
    });
    assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
}

#[test]
fn stop_with_clutch_in_passes() {
    let out = play(LessonId::Stop, 10.0, |t, _| Controls {
        brake: 0.3,
        clutch: if t > 1.0 { 1.0 } else { 0.0 },
        ..Default::default()
    });
    assert!(matches!(out, Outcome::Passed { .. }), "{out:?}");
}

#[test]
fn stop_without_clutch_fails() {
    let out = play(LessonId::Stop, 10.0, |_, _| Controls {
        brake: 0.3,
        ..Default::default()
    });
    assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
}

#[test]
fn cup_survives_gentle_launch_but_not_a_slam() {
    let mut gentle = Cup::default();
    let mut slam = Cup::default();
    for _ in 0..60 {
        gentle.update(2.0, DT);
        slam.update(40.0, DT);
    }
    assert_eq!(gentle.level, 1.0);
    assert!(slam.level < 0.5, "{}", slam.level);
}
```

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p drivetrain --test lessons`
Expected: compile error, `LessonId` not found.

- [ ] **Step 3: Implement** — `crates/drivetrain/src/lib.rs`:

```rust
//! Pure-Rust manual-transmission car model. No Bevy; the game drives it via
//! `Sim::step` and reads `SimState`.
mod car;
mod lesson;
mod score;
mod sim;

pub use car::CarSpec;
pub use lesson::{LessonId, LessonRun, Outcome};
pub use score::{Cup, stars};
pub use sim::{Controls, Env, Event, RPM_PER_RAD_S, SHIFT_CLUTCH, Sim, SimState};
```

`crates/drivetrain/src/score.rs`:

```rust
/// Jerk (m/s³) a full coffee cup tolerates before it starts to spill.
const SPILL_JERK: f32 = 6.0;
/// Fraction of the cup lost per second per m/s³ above the threshold.
const SPILL_RATE: f32 = 0.02;

/// The coffee cup on the dash. Smooth driving keeps it full.
#[derive(Debug, Clone, Copy)]
pub struct Cup {
    /// 1 = full, 0 = empty.
    pub level: f32,
}

impl Default for Cup {
    fn default() -> Self {
        Self { level: 1.0 }
    }
}

impl Cup {
    /// Spill for one frame of `jerk`; returns how much spilled this frame.
    pub fn update(&mut self, jerk: f32, dt: f32) -> f32 {
        let spill = ((jerk.abs() - SPILL_JERK).max(0.0) * SPILL_RATE * dt).min(self.level);
        self.level -= spill;
        spill
    }
}

/// 1–3 stars for a passed lesson. Each mistake class costs one star.
pub fn stars(grinds: u32, cup: &Cup, clutch_heat_j: f32) -> u8 {
    let mut s = 3u8;
    if grinds > 0 {
        s -= 1;
    }
    if cup.level < 0.9 {
        s -= 1;
    }
    if clutch_heat_j > 15_000.0 {
        s -= 1;
    }
    s.max(1)
}
```

`crates/drivetrain/src/lesson.rs`:

```rust
use crate::score::{Cup, stars};
use crate::sim::{Controls, Env, Event, Sim, SimState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LessonId {
    BitePoint,
    PullAway,
    Stop,
}

impl LessonId {
    pub const ALL: [LessonId; 3] = [LessonId::BitePoint, LessonId::PullAway, LessonId::Stop];

    pub fn title(self) -> &'static str {
        match self {
            LessonId::BitePoint => "1. Find the bite point",
            LessonId::PullAway => "2. Pull away",
            LessonId::Stop => "3. Stop without stalling",
        }
    }

    pub fn brief(self) -> &'static str {
        match self {
            LessonId::BitePoint => {
                "You're in 1st with the handbrake on. Slowly lift the clutch until the revs dip \
                 and the engine note drops. Hold it there for 3 seconds."
            }
            LessonId::PullAway => {
                "Clutch in, select 1st, release the handbrake. Add a little gas, lift the clutch \
                 slowly through the bite point, then all the way up. Reach 10 km/h."
            }
            LessonId::Stop => {
                "You're cruising in 2nd. Brake gently, and press the clutch in before the revs \
                 drop below about 1000. Come to a stop and wait 2 seconds."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    Running,
    Passed { stars: u8 },
    Failed(&'static str),
}

/// One attempt at a lesson: sets the car up, then judges each frame.
pub struct LessonRun {
    pub id: LessonId,
    pub cup: Cup,
    pub grinds: u32,
    timer_s: f32,
}

impl LessonRun {
    pub fn new(id: LessonId) -> Self {
        Self {
            id,
            cup: Cup::default(),
            grinds: 0,
            timer_s: 0.0,
        }
    }

    /// Put the car in the lesson's starting state. Returns whether the
    /// handbrake starts on.
    pub fn setup(&self, sim: &mut Sim) -> bool {
        match self.id {
            LessonId::BitePoint => {
                sim.set_moving(1, 0.0);
                true
            }
            LessonId::PullAway => true,
            LessonId::Stop => {
                sim.set_moving(2, 30.0 / 3.6);
                false
            }
        }
    }

    pub fn env(&self) -> Env {
        Env::default()
    }

    pub fn update(&mut self, s: &SimState, events: &[Event], dt: f32) -> Outcome {
        self.cup.update(s.jerk, dt);
        self.grinds += events.iter().filter(|e| **e == Event::Grind).count() as u32;
        if events.contains(&Event::Stalled) {
            return Outcome::Failed("Stalled. Clutch in, turn the key, try again.");
        }
        let goal_met = match self.id {
            LessonId::BitePoint => {
                s.gear == 1 && (0.05..0.6).contains(&s.clutch_engagement) && s.engine_running
            }
            LessonId::PullAway => {
                s.gear >= 1 && s.speed_mps * 3.6 >= 10.0 && s.clutch_engagement >= 1.0
            }
            LessonId::Stop => s.speed_mps.abs() < 0.05 && s.engine_running,
        };
        let hold_s = match self.id {
            LessonId::BitePoint => 3.0,
            LessonId::PullAway => 0.0,
            LessonId::Stop => 2.0,
        };
        self.timer_s = if goal_met { self.timer_s + dt } else { 0.0 };
        if goal_met && self.timer_s >= hold_s {
            Outcome::Passed {
                stars: stars(self.grinds, &self.cup, s.clutch_heat_j),
            }
        } else {
            Outcome::Running
        }
    }

    /// Live coaching line for this frame, if any.
    pub fn hint(&self, s: &SimState, c: &Controls) -> Option<&'static str> {
        let coupled = s.gear != 0 && s.clutch_engagement > 0.0 && s.engine_running;
        if coupled && s.engine_rpm < 650.0 {
            return Some("RPM dropping — clutch in!");
        }
        match self.id {
            LessonId::BitePoint if s.gear == 1 && s.clutch_engagement > 0.05 => {
                Some("That's the bite point. Hold it.")
            }
            LessonId::PullAway if s.gear == 0 && c.clutch < 0.85 => {
                Some("Press the clutch all the way in, then select 1st.")
            }
            LessonId::PullAway if s.gear >= 1 && c.handbrake > 0.0 => {
                Some("Release the handbrake.")
            }
            LessonId::Stop if coupled && s.engine_rpm < 1100.0 => Some("Clutch in now."),
            _ => None,
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p drivetrain`
Expected: 21 passed (3 + 11 + 7).

- [ ] **Step 5:** **Checkpoint — the user commits** (the agent does not run git):

```bash
git add crates/drivetrain
git commit -m "feat(drivetrain): coffee cup scoring and lessons 1-3"
```

---

### Task 4: GitHub repo and CI

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Produces: CI jobs `check` (fmt, clippy `-D warnings`, tests), `web` (wasm32 release build → wasm-bindgen → `dist/` Pages artifact), `deploy` (Pages, `main` pushes only). Later tasks rely on `check` + `web` as their compile gate.

- [ ] **Step 1: Write the workflow** — `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
  pull_request:
  workflow_dispatch:

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Bevy Linux deps
        run: |
          sudo apt-get update
          sudo apt-get install -y --no-install-recommends g++ pkg-config libx11-dev \
            libasound2-dev libudev-dev libxkbcommon-x11-0 libwayland-dev libxkbcommon-dev
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace

  web:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - uses: Swatinem/rust-cache@v2
        with:
          key: wasm
      - run: cargo build --release -p game --target wasm32-unknown-unknown
      - name: Match wasm-bindgen CLI to the locked crate
        id: wbg
        run: echo "version=$(cargo metadata --format-version 1 | jq -r '.packages[] | select(.name=="wasm-bindgen") | .version')" >> "$GITHUB_OUTPUT"
      - uses: taiki-e/install-action@v2
        with:
          tool: wasm-bindgen@${{ steps.wbg.outputs.version }}
      - name: Bundle
        run: |
          wasm-bindgen --no-typescript --target web --out-dir dist --out-name miata \
            target/wasm32-unknown-unknown/release/miata.wasm
          cp web/index.html dist/
      - uses: actions/upload-pages-artifact@v3
        with:
          path: dist

  deploy:
    if: github.ref == 'refs/heads/main' && github.event_name == 'push'
    needs: [check, web]
    runs-on: ubuntu-latest
    permissions:
      pages: write
      id-token: write
    environment:
      name: github-pages
      url: ${{ steps.deploy.outputs.page_url }}
    steps:
      - id: deploy
        uses: actions/deploy-pages@v4
```

Until Task 5 adds the game crate, the `web` job fails at `cargo build -p game` — expected; only `check` must be green in this task.

- [ ] **Step 2: User creates the repo and enables Pages** (agent hands these over; user runs them):

```bash
gh auth switch -u lubabs770 && gh auth status
# after committing this task's checkpoint:
gh repo create lubabs770/miata --public --source ~/code/miata --push
gh api -X POST repos/lubabs770/miata/pages -f build_type=workflow
```

- [ ] **Step 3: Verify** — `gh run watch --exit-status $(gh run list --limit 1 --json databaseId -q '.[0].databaseId')`; expected: `check` green.

- [ ] **Step 4:** **Checkpoint — the user commits** (the agent does not run git):

```bash
git add .github
git commit -m "ci: fmt, clippy, tests, wasm bundle and Pages deploy"
```

---

### Task 5: Game crate — input and driving loop

**Files:**
- Create: `crates/game/Cargo.toml`, `crates/game/src/main.rs`, `crates/game/src/input.rs`, `crates/game/src/driving.rs`
- Modify: `Cargo.toml` (add `crates/game` to members)

**Interfaces:**
- Consumes: `drivetrain::{CarSpec, Controls, Cup, Event, LessonId, LessonRun, Outcome, Sim}`.
- Produces: `AppState::{ClickToStart, Driving}` (in `main.rs`, `pub`); `input::Action` (leafwing `Actionlike`), `input::Pedals { pub controls: Controls, pub handbrake_on: bool, .. }` resource updated in `PreUpdate`; `input::gate(Vec2) -> Option<i8>`; `driving::Drive { pub sim, pub lesson: Option<LessonRun>, pub outcome: Outcome, pub hint: Option<&'static str>, pub free_cup: Cup, pub last_events: Vec<Event> }` with `Drive::new(seed)`, `Drive::cup()`, `Drive::start(Option<LessonId>, seed, &mut Pedals)`; `driving::step` system. Each module exposes `pub fn plugin(app: &mut App)`.

- [ ] **Step 1:** In root `Cargo.toml` set `members = ["crates/drivetrain", "crates/game"]`.

- [ ] **Step 2:** `crates/game/Cargo.toml`:

```toml
[package]
name = "game"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true

[[bin]]
name = "miata"
path = "src/main.rs"

[dependencies]
drivetrain = { path = "../drivetrain" }
bevy = "0.19.1"
bevy_egui = "0.42"
leafwing-input-manager = "0.21"

[target.'cfg(target_arch = "wasm32")'.dependencies]
bevy = { version = "0.19.1", features = ["web"] }
```

- [ ] **Step 3:** `crates/game/src/input.rs` (includes its tests: H-pattern gates, slow clutch-key release):

```rust
//! Keyboard + gamepad → `drivetrain::Controls`, via leafwing actions.

use bevy::prelude::*;
use drivetrain::Controls;
use leafwing_input_manager::plugin::InputManagerSystem;
use leafwing_input_manager::prelude::*;

/// Keyboard pedal ramp rates, pedal travel per second.
const KEY_PRESS_RATE: f32 = 4.0;
const KEY_RELEASE_RATE: f32 = 4.0;
/// Releasing the clutch key lets the pedal up slowly so keyboard players can
/// feather it through the bite point.
const CLUTCH_KEY_RELEASE_RATE: f32 = 0.35;
/// How far the right stick must move to select a gate on the H-pattern.
const GATE_THRESHOLD: f32 = 0.7;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum Action {
    // Analog (gamepad triggers / stick).
    Throttle,
    Clutch,
    #[actionlike(Axis)]
    BrakeStick,
    // Digital keys, ramped in software.
    ThrottleKey,
    BrakeKey,
    ClutchKey,
    #[actionlike(Axis)]
    Steer,
    /// Right stick as an H-pattern shifter.
    #[actionlike(DualAxis)]
    Shifter,
    Gear1,
    Gear2,
    Gear3,
    Gear4,
    Gear5,
    GearR,
    Neutral,
    Handbrake,
    Ignition,
}

impl Action {
    pub fn default_map() -> InputMap<Self> {
        use Action::*;
        let mut m = InputMap::default();
        m.insert(Throttle, GamepadButton::RightTrigger2);
        m.insert(Clutch, GamepadButton::LeftTrigger2);
        m.insert_axis(BrakeStick, GamepadControlAxis::LEFT_Y);
        m.insert_axis(Steer, GamepadControlAxis::LEFT_X);
        m.insert_dual_axis(Shifter, GamepadStick::RIGHT);
        m.insert(Neutral, GamepadButton::RightThumb);
        m.insert(Handbrake, GamepadButton::West);
        m.insert(Ignition, GamepadButton::North);

        m.insert(ThrottleKey, KeyCode::KeyW);
        m.insert(BrakeKey, KeyCode::KeyS);
        m.insert(ClutchKey, KeyCode::ShiftLeft);
        m.insert_axis(Steer, VirtualAxis::ad());
        for (a, k) in [
            (Gear1, KeyCode::Digit1),
            (Gear2, KeyCode::Digit2),
            (Gear3, KeyCode::Digit3),
            (Gear4, KeyCode::Digit4),
            (Gear5, KeyCode::Digit5),
            (GearR, KeyCode::KeyR),
            (Neutral, KeyCode::KeyN),
            (Handbrake, KeyCode::Space),
            (Ignition, KeyCode::KeyI),
        ] {
            m.insert(a, k);
        }
        m
    }
}

/// The player's current pedal/lever state, rebuilt each frame.
#[derive(Resource, Default)]
pub struct Pedals {
    pub controls: Controls,
    pub handbrake_on: bool,
    kb_throttle: f32,
    kb_brake: f32,
    kb_clutch: f32,
    last_gate: Option<i8>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Pedals>()
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Action::default_map());
        })
        .add_systems(PreUpdate, read_input.after(InputManagerSystem::Update));
}

fn ramp(current: f32, held: bool, up: f32, down: f32, dt: f32) -> f32 {
    if held {
        current + up * dt
    } else {
        current - down * dt
    }
    .clamp(0.0, 1.0)
}

/// Map a right-stick position to an H-pattern gate:
/// left column 1/2, middle 3/4, right 5/R.
pub fn gate(stick: Vec2) -> Option<i8> {
    let row = if stick.y > GATE_THRESHOLD {
        0
    } else if stick.y < -GATE_THRESHOLD {
        1
    } else {
        return None;
    };
    let col = if stick.x < -0.5 {
        0
    } else if stick.x > 0.5 {
        2
    } else {
        1
    };
    Some([[1, 2], [3, 4], [5, -1]][col][row])
}

fn read_input(a: Single<&ActionState<Action>>, time: Res<Time>, mut p: ResMut<Pedals>) {
    let dt = time.delta_secs();
    p.kb_throttle = ramp(
        p.kb_throttle,
        a.pressed(&Action::ThrottleKey),
        KEY_PRESS_RATE,
        KEY_RELEASE_RATE,
        dt,
    );
    p.kb_brake = ramp(
        p.kb_brake,
        a.pressed(&Action::BrakeKey),
        KEY_PRESS_RATE,
        KEY_RELEASE_RATE,
        dt,
    );
    p.kb_clutch = ramp(
        p.kb_clutch,
        a.pressed(&Action::ClutchKey),
        KEY_PRESS_RATE,
        CLUTCH_KEY_RELEASE_RATE,
        dt,
    );
    if a.just_pressed(&Action::Handbrake) {
        p.handbrake_on = !p.handbrake_on;
    }

    let gate_now = gate(a.axis_pair(&Action::Shifter));
    let stick_shift = gate_now.filter(|g| Some(*g) != p.last_gate);
    p.last_gate = gate_now;
    let key_shift = [
        (Action::Gear1, 1),
        (Action::Gear2, 2),
        (Action::Gear3, 3),
        (Action::Gear4, 4),
        (Action::Gear5, 5),
        (Action::GearR, -1),
        (Action::Neutral, 0),
    ]
    .into_iter()
    .find(|(act, _)| a.just_pressed(act))
    .map(|(_, g)| g);

    p.controls = Controls {
        clutch: a.button_value(&Action::Clutch).max(p.kb_clutch),
        throttle: a.button_value(&Action::Throttle).max(p.kb_throttle),
        brake: (-a.value(&Action::BrakeStick)).max(0.0).max(p.kb_brake),
        handbrake: if p.handbrake_on { 1.0 } else { 0.0 },
        // Sim steer is +left; the A/D axis and stick are +right.
        steer: -a.value(&Action::Steer),
        shift: key_shift.or(stick_shift),
        ignition: a.just_pressed(&Action::Ignition),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stick_gates_follow_h_pattern() {
        assert_eq!(gate(Vec2::new(-1.0, 1.0)), Some(1));
        assert_eq!(gate(Vec2::new(-1.0, -1.0)), Some(2));
        assert_eq!(gate(Vec2::new(0.0, 1.0)), Some(3));
        assert_eq!(gate(Vec2::new(0.0, -1.0)), Some(4));
        assert_eq!(gate(Vec2::new(1.0, 1.0)), Some(5));
        assert_eq!(gate(Vec2::new(1.0, -1.0)), Some(-1));
        assert_eq!(gate(Vec2::new(0.9, 0.1)), None);
    }

    #[test]
    fn clutch_key_releases_slowly() {
        let down = ramp(0.0, true, KEY_PRESS_RATE, CLUTCH_KEY_RELEASE_RATE, 1.0);
        assert_eq!(down, 1.0);
        let after_1s = ramp(down, false, KEY_PRESS_RATE, CLUTCH_KEY_RELEASE_RATE, 1.0);
        assert!((after_1s - 0.65).abs() < 1e-6);
    }
}
```

- [ ] **Step 4:** `crates/game/src/driving.rs` (includes a headless `MinimalPlugins` test that the sim advances):

```rust
//! Owns the simulation and the active lesson; steps them once per frame.

use bevy::prelude::*;
use drivetrain::{CarSpec, Cup, Event, LessonId, LessonRun, Outcome, Sim};

use crate::AppState;
use crate::input::Pedals;

#[derive(Resource)]
pub struct Drive {
    pub sim: Sim,
    /// `None` = free drive.
    pub lesson: Option<LessonRun>,
    pub outcome: Outcome,
    pub hint: Option<&'static str>,
    /// Cup used in free drive (lessons keep their own).
    pub free_cup: Cup,
    pub last_events: Vec<Event>,
}

impl Drive {
    pub fn new(seed: u64) -> Self {
        Self {
            sim: Sim::new(CarSpec::miata(), seed),
            lesson: None,
            outcome: Outcome::Running,
            hint: None,
            free_cup: Cup::default(),
            last_events: Vec::new(),
        }
    }

    pub fn cup(&self) -> &Cup {
        self.lesson.as_ref().map_or(&self.free_cup, |l| &l.cup)
    }

    /// Fresh car and attempt. `seed` varies the bite point.
    pub fn start(&mut self, lesson: Option<LessonId>, seed: u64, pedals: &mut Pedals) {
        *self = Self::new(seed);
        if let Some(id) = lesson {
            let run = LessonRun::new(id);
            pedals.handbrake_on = run.setup(&mut self.sim);
            self.lesson = Some(run);
        }
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Drive::new(0))
        .add_systems(OnEnter(AppState::Driving), start_free_drive)
        .add_systems(Update, step.run_if(in_state(AppState::Driving)));
}

fn start_free_drive(mut drive: ResMut<Drive>, mut pedals: ResMut<Pedals>, time: Res<Time>) {
    drive.start(None, time.elapsed().as_nanos() as u64, &mut pedals);
}

pub fn step(mut drive: ResMut<Drive>, pedals: Res<Pedals>, time: Res<Time>) {
    let dt = time.delta_secs();
    let Drive {
        sim,
        lesson,
        outcome,
        hint,
        free_cup,
        last_events,
    } = &mut *drive;
    let env = lesson.as_ref().map(|l| l.env()).unwrap_or_default();
    *last_events = sim.step(&pedals.controls, env, dt);
    let s = *sim.state();
    match lesson {
        Some(run) if *outcome == Outcome::Running => {
            *outcome = run.update(&s, last_events, dt);
            *hint = run.hint(&s, &pedals.controls);
        }
        Some(_) => *hint = None,
        None => {
            free_cup.update(s.jerk, dt);
            *hint =
                (!s.engine_running).then_some("Engine off — clutch in, then I (pad Y) to start.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    #[test]
    fn step_advances_sim_headless() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Pedals>()
            .insert_resource(Drive::new(1))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                16,
            )))
            .add_systems(Update, step);
        app.world_mut().resource_mut::<Pedals>().controls.throttle = 1.0;
        for _ in 0..30 {
            app.update();
        }
        let rpm = app.world().resource::<Drive>().sim.state().engine_rpm;
        assert!(rpm > 1500.0, "free-revving in neutral, rpm {rpm}");
    }
}
```

- [ ] **Step 5:** `crates/game/src/main.rs` (later tasks replace this file as modules are added):

```rust
// Some `Drive` fields are only read once the HUD exists; Task 7 removes this.
#![allow(dead_code)]

mod driving;
mod input;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use leafwing_input_manager::prelude::*;

#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AppState {
    /// Browsers only allow audio after a user gesture, so we wait for one.
    #[default]
    ClickToStart,
    Driving,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "miata — learn to drive stick".into(),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(InputManagerPlugin::<input::Action>::default())
        .init_state::<AppState>()
        .add_plugins((input::plugin, driving::plugin))
        .run();
}
```

- [ ] **Step 6: Verify on CI** — Push and wait for CI (the game crate is never compiled locally):

```bash
git push && gh run watch --exit-status $(gh run list --branch "$(git branch --show-current)" --limit 1 --json databaseId -q '.[0].databaseId')
```

Expected: `check` and `web` jobs green. On failure: `gh run view --log-failed`, fix, push again.

If `web` fails on a `getrandom` error, add to `crates/game/Cargo.toml` under the wasm target section: `getrandom = { version = "0.3", features = ["wasm_js"] }` (and a `getrandom_04 = { package = "getrandom", version = "0.4", features = ["wasm_js"] }` line if 0.4 is the one complaining), then push again.

- [ ] **Step 7:** **Checkpoint — the user commits** (the agent does not run git):

```bash
git add Cargo.toml Cargo.lock crates/game
git commit -m "feat(game): leafwing input and per-frame driving loop"
```

---

### Task 6: World and cockpit

**Files:**
- Create: `crates/game/src/cockpit.rs`
- Modify: `crates/game/src/main.rs`

**Interfaces:**
- Consumes: `driving::Drive`, `input::Pedals`.
- Produces: camera (child of the car rig, driver's eye), world meshes, animated tach/speedo needles, steering wheel, shifter knob, coffee cup. Coordinate mapping: sim `(x, y, heading)` → world `(-y, 0, -x)`, rotation `Y(heading)`.

- [ ] **Step 1:** `crates/game/src/cockpit.rs`:

```rust
//! Low-poly world and the driver's-seat view, built from Bevy primitives.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;
use drivetrain::SimState;

use crate::driving::Drive;
use crate::input::Pedals;

const ROAD_LEN: f32 = 4000.0;
const TACH_MAX_RPM: f32 = 8000.0;
const SPEEDO_MAX_KMH: f32 = 220.0;
/// Needle sweep: 135° either side of straight up.
const SWEEP: f32 = 0.75 * PI;
const STEER_WHEEL_TURNS: f32 = 1.5 * PI;

#[derive(Component)]
struct CarRig;
#[derive(Component)]
struct TachNeedle;
#[derive(Component)]
struct SpeedNeedle;
#[derive(Component)]
struct SteeringWheel(Quat);
#[derive(Component)]
struct ShifterKnob(Vec3);
#[derive(Component)]
struct Coffee;

pub fn plugin(app: &mut App) {
    app.insert_resource(ClearColor(Color::srgb(0.62, 0.78, 0.93)))
        .add_systems(Startup, (spawn_world, spawn_car))
        .add_systems(Update, (follow_sim, animate_cockpit));
}

fn mat(materials: &mut Assets<StandardMaterial>, c: Color) -> MeshMaterial3d<StandardMaterial> {
    MeshMaterial3d(materials.add(StandardMaterial {
        base_color: c,
        perceptual_roughness: 0.9,
        ..default()
    }))
}

fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::OVERCAST_DAY,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(30.0, 60.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(ROAD_LEN, ROAD_LEN))),
        mat(&mut materials, Color::srgb(0.42, 0.55, 0.33)),
        Transform::from_xyz(0.0, -0.01, -ROAD_LEN / 2.0 + 50.0),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(8.0, 0.02, ROAD_LEN))),
        mat(&mut materials, Color::srgb(0.25, 0.25, 0.27)),
        Transform::from_xyz(0.0, 0.0, -ROAD_LEN / 2.0 + 50.0),
    ));
    // Centre dashes and roadside posts give a sense of speed.
    let dash = meshes.add(Cuboid::new(0.15, 0.03, 3.0));
    let dash_mat = mat(&mut materials, Color::srgb(0.95, 0.9, 0.6));
    let post = meshes.add(Cuboid::new(0.15, 1.0, 0.15));
    let post_mat = mat(&mut materials, Color::WHITE);
    for i in 0..(ROAD_LEN as i32 / 10) {
        let z = 50.0 - i as f32 * 10.0;
        commands.spawn((
            Mesh3d(dash.clone()),
            dash_mat.clone(),
            Transform::from_xyz(0.0, 0.0, z),
        ));
        if i % 2 == 0 {
            for x in [-5.0, 5.0] {
                commands.spawn((
                    Mesh3d(post.clone()),
                    post_mat.clone(),
                    Transform::from_xyz(x, 0.5, z),
                ));
            }
        }
    }
    // A few buildings either side, placed deterministically.
    let block = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    for i in 0..120 {
        let h = 4.0 + (i * 7 % 13) as f32;
        let w = 6.0 + (i * 5 % 7) as f32;
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let tint = 0.55 + (i * 3 % 5) as f32 * 0.08;
        commands.spawn((
            Mesh3d(block.clone()),
            mat(&mut materials, Color::srgb(tint, tint * 0.9, tint * 0.8)),
            Transform::from_xyz(
                side * (14.0 + (i % 3) as f32 * 4.0),
                h / 2.0,
                20.0 - i as f32 * 30.0,
            )
            .with_scale(Vec3::new(w, h, w)),
        ));
    }
}

fn spawn_car(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let red = mat(&mut materials, Color::srgb(0.75, 0.05, 0.05));
    let dark = mat(&mut materials, Color::srgb(0.08, 0.08, 0.09));
    let face = mat(&mut materials, Color::srgb(0.02, 0.02, 0.02));
    let needle = mat(&mut materials, Color::srgb(1.0, 0.45, 0.1));
    let white = mat(&mut materials, Color::srgb(0.95, 0.95, 0.92));
    let coffee = mat(&mut materials, Color::srgb(0.3, 0.17, 0.07));

    let gauge = meshes.add(Cylinder::new(0.075, 0.01));
    let needle_mesh = meshes.add(Cuboid::new(0.006, 0.065, 0.004));
    // Gauge faces point at the driver: cylinder axis (Y) turned to +Z.
    let face_rot = Quat::from_rotation_x(FRAC_PI_2);
    let wheel_rot = Quat::from_rotation_x(FRAC_PI_2 - 0.35);

    commands
        .spawn((CarRig, Transform::default(), Visibility::default()))
        .with_children(|car| {
            // US car: driver sits left of centre.
            car.spawn((
                Camera3d::default(),
                Transform::from_xyz(-0.35, 1.05, 0.0).with_rotation(Quat::from_rotation_x(-0.12)),
            ));
            car.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.65, 0.12, 1.8))),
                red.clone(),
                Transform::from_xyz(0.0, 0.62, -1.7),
            ));
            car.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.5, 0.25, 0.45))),
                dark.clone(),
                Transform::from_xyz(0.0, 0.75, -0.65),
            ));
            for x in [-0.78, 0.78] {
                car.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.05, 0.6, 0.05))),
                    dark.clone(),
                    Transform::from_xyz(x, 1.05, -0.75).with_rotation(Quat::from_rotation_x(-0.5)),
                ));
            }
            for (x, marker) in [(-0.45, 0), (-0.27, 1)] {
                car.spawn((
                    Mesh3d(gauge.clone()),
                    face.clone(),
                    Transform::from_xyz(x, 0.9, -0.52).with_rotation(face_rot),
                ))
                .with_children(|g| {
                    // Pivot at the gauge centre; the needle sits above it.
                    let mut pivot =
                        g.spawn((Transform::from_xyz(0.0, 0.008, 0.0), Visibility::default()));
                    if marker == 0 {
                        pivot.insert(TachNeedle);
                    } else {
                        pivot.insert(SpeedNeedle);
                    }
                    pivot.with_children(|p| {
                        p.spawn((
                            Mesh3d(needle_mesh.clone()),
                            needle.clone(),
                            Transform::from_xyz(0.0, 0.0, -0.03)
                                .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
                        ));
                    });
                });
            }
            car.spawn((
                SteeringWheel(wheel_rot),
                Mesh3d(meshes.add(Torus::new(0.16, 0.185))),
                dark.clone(),
                Transform::from_xyz(-0.35, 0.82, -0.38).with_rotation(wheel_rot),
            ));
            let knob_home = Vec3::new(0.0, 0.62, -0.3);
            car.spawn((
                ShifterKnob(knob_home),
                Mesh3d(meshes.add(Sphere::new(0.035))),
                white.clone(),
                Transform::from_translation(knob_home),
            ));
            car.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.04, 0.1))),
                white.clone(),
                Transform::from_xyz(0.15, 0.93, -0.55),
            ))
            .with_children(|cup| {
                cup.spawn((
                    Coffee,
                    Mesh3d(meshes.add(Cylinder::new(0.037, 0.01))),
                    coffee.clone(),
                    Transform::from_xyz(0.0, 0.045, 0.0),
                ));
            });
        });
}

/// Sim x/y are a ground plane with heading 0 = sim +x. In Bevy the car
/// faces -Z, so sim +x → world -Z and sim +y (left) → world -X.
fn follow_sim(drive: Res<Drive>, mut rig: Single<&mut Transform, With<CarRig>>) {
    let s = drive.sim.state();
    rig.translation = Vec3::new(-s.y, 0.0, -s.x);
    rig.rotation = Quat::from_rotation_y(s.heading);
}

fn needle_angle(value: f32, max: f32) -> Quat {
    // Gauge faces were rotated so local Y is "toward the driver"; spin about it.
    Quat::from_rotation_y(SWEEP - (value / max).clamp(0.0, 1.0) * 2.0 * SWEEP)
}

fn knob_offset(gear: i8) -> Vec3 {
    let (col, row) = match gear {
        1 => (-1.0, -1.0),
        2 => (-1.0, 1.0),
        3 => (0.0, -1.0),
        4 => (0.0, 1.0),
        5 => (1.0, -1.0),
        -1 => (1.0, 1.0),
        _ => (0.0, 0.0),
    };
    Vec3::new(col * 0.04, 0.0, row * 0.04)
}

#[allow(clippy::type_complexity)]
fn animate_cockpit(
    drive: Res<Drive>,
    pedals: Res<Pedals>,
    mut q: ParamSet<(
        Single<&mut Transform, With<TachNeedle>>,
        Single<&mut Transform, With<SpeedNeedle>>,
        Single<(&mut Transform, &SteeringWheel)>,
        Single<(&mut Transform, &ShifterKnob)>,
        Single<&mut Transform, With<Coffee>>,
    )>,
) {
    let s: &SimState = drive.sim.state();
    q.p0().rotation = needle_angle(s.engine_rpm, TACH_MAX_RPM);
    q.p1().rotation = needle_angle(s.speed_mps.abs() * 3.6, SPEEDO_MAX_KMH);
    {
        let mut w = q.p2();
        let (t, base) = &mut *w;
        t.rotation =
            base.0 * Quat::from_rotation_y(pedals.controls.steer * STEER_WHEEL_TURNS / 2.0);
    }
    {
        let mut k = q.p3();
        let (t, home) = &mut *k;
        t.translation = home.0 + knob_offset(s.gear);
    }
    let level = drive.cup().level;
    let mut c = q.p4();
    c.translation.y = -0.045 + 0.09 * level;
    // Coffee surface tilts with acceleration.
    c.rotation = Quat::from_rotation_x((s.accel * 0.04).clamp(-0.4, 0.4));
    c.scale = if level <= 0.01 { Vec3::ZERO } else { Vec3::ONE };
}
```

- [ ] **Step 2:** Replace `crates/game/src/main.rs`:

```rust
// Some `Drive` fields are only read once the HUD exists; Task 7 removes this.
#![allow(dead_code)]

mod cockpit;
mod driving;
mod input;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use leafwing_input_manager::prelude::*;

#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AppState {
    /// Browsers only allow audio after a user gesture, so we wait for one.
    #[default]
    ClickToStart,
    Driving,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "miata — learn to drive stick".into(),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(InputManagerPlugin::<input::Action>::default())
        .init_state::<AppState>()
        .add_plugins((input::plugin, driving::plugin, cockpit::plugin))
        .run();
}
```

- [ ] **Step 3: Verify on CI** — Push and wait for CI (the game crate is never compiled locally):

```bash
git push && gh run watch --exit-status $(gh run list --branch "$(git branch --show-current)" --limit 1 --json databaseId -q '.[0].databaseId')
```

Expected: `check` and `web` jobs green. On failure: `gh run view --log-failed`, fix, push again.

- [ ] **Step 4:** **Checkpoint — the user commits** (the agent does not run git):

```bash
git add crates/game
git commit -m "feat(game): low-poly world and Miata cockpit"
```

---

### Task 7: HUD, click-to-start and lesson panel

**Files:**
- Create: `crates/game/src/hud.rs`
- Modify: `crates/game/src/main.rs`

**Interfaces:**
- Consumes: `AppState`, `Drive` (incl. `start`, `cup`, `hint`, `outcome`), `Pedals`, `LessonId::ALL/title/brief`.
- Produces: `ClickToStart → Driving` transition on any key/mouse/pad press; instruments (gear, km/h, rpm, pedal bars, handbrake, engine-off, coffee %); centred coaching hint; lessons window (pick, brief, result stars, Retry/Enter, Free drive, key help); the spec's honesty note on the start screen.

- [ ] **Step 1:** `crates/game/src/hud.rs`:

```rust
//! egui overlays: start screen, instruments, coaching, lesson picker.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use drivetrain::{LessonId, Outcome};

use crate::AppState;
use crate::driving::Drive;
use crate::input::Pedals;

const KEYS: &str = "Keyboard: W gas · S brake · Left Shift clutch (hold; releases slowly) · A/D steer · \
1–5 / R / N gears · Space handbrake · I ignition\n\
Gamepad: RT gas · LT clutch · left stick steer, down = brake · right stick H-shifter · \
R3 neutral · X handbrake · Y ignition";

pub fn plugin(app: &mut App) {
    app.add_systems(
        EguiPrimaryContextPass,
        (
            start_screen.run_if(in_state(AppState::ClickToStart)),
            (instruments, lesson_panel).run_if(in_state(AppState::Driving)),
        ),
    )
    .add_systems(
        Update,
        any_input_starts.run_if(in_state(AppState::ClickToStart)),
    );
}

fn any_input_starts(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    mut next: ResMut<NextState<AppState>>,
) {
    let pad = pads.iter().any(|p| p.get_just_pressed().next().is_some());
    if keys.get_just_pressed().next().is_some() || mouse.get_just_pressed().next().is_some() || pad
    {
        next.set(AppState::Driving);
    }
}

fn start_screen(mut contexts: EguiContexts) -> Result {
    egui::Window::new("start")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(contexts.ctx_mut()?, |ui| {
        ui.vertical_centered(|ui| {
            ui.heading(egui::RichText::new("miata").size(48.0));
            ui.label("Learn to drive a manual transmission.");
            ui.add_space(24.0);
            ui.label(egui::RichText::new("Click or press any key to start").size(22.0).strong());
            ui.add_space(24.0);
            ui.label(KEYS);
            ui.add_space(16.0);
            ui.label(
                egui::RichText::new(
                    "With a keyboard or gamepad this trains the sequence, the timing and reading the \
                     revs and sound. The physical feel of a clutch pedal only carries over with real pedals.",
                )
                .italics(),
            );
        });
    });
    Ok(())
}

fn bar(ui: &mut egui::Ui, label: &str, v: f32, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(format!("{label:>8}"));
        ui.add(egui::ProgressBar::new(v).desired_width(140.0).fill(color));
    });
}

fn instruments(mut contexts: EguiContexts, drive: Res<Drive>, pedals: Res<Pedals>) -> Result {
    let ctx = contexts.ctx_mut()?;
    let s = drive.sim.state();
    let c = &pedals.controls;
    egui::Area::new("instruments".into())
        .anchor(egui::Align2::LEFT_BOTTOM, [12.0, -12.0])
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                let gear = match s.gear {
                    -1 => "R".to_string(),
                    0 => "N".to_string(),
                    g => g.to_string(),
                };
                ui.label(
                    egui::RichText::new(format!(
                        "{gear}   {:>3.0} km/h   {:>4.0} rpm",
                        s.speed_mps.abs() * 3.6,
                        s.engine_rpm
                    ))
                    .size(24.0)
                    .monospace(),
                );
                bar(
                    ui,
                    "clutch",
                    c.clutch,
                    egui::Color32::from_rgb(90, 140, 230),
                );
                bar(ui, "brake", c.brake, egui::Color32::from_rgb(220, 70, 60));
                bar(ui, "gas", c.throttle, egui::Color32::from_rgb(80, 190, 90));
                ui.horizontal(|ui| {
                    if pedals.handbrake_on {
                        ui.colored_label(egui::Color32::YELLOW, "(P) handbrake");
                    }
                    if !s.engine_running {
                        ui.colored_label(egui::Color32::RED, "engine off");
                    }
                    ui.label(format!("coffee {:.0}%", drive.cup().level * 100.0));
                });
            });
        });
    if let Some(h) = drive.hint {
        egui::Area::new("hint".into())
            .anchor(egui::Align2::CENTER_TOP, [0.0, 24.0])
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(egui::RichText::new(h).size(26.0).strong());
                });
            });
    }
    Ok(())
}

fn lesson_panel(
    mut contexts: EguiContexts,
    mut drive: ResMut<Drive>,
    mut pedals: ResMut<Pedals>,
    time: Res<Time>,
) -> Result {
    let seed = time.elapsed().as_nanos() as u64;
    let mut start: Option<Option<LessonId>> = None;
    egui::Window::new("Lessons")
        .anchor(egui::Align2::RIGHT_TOP, [-12.0, 12.0])
        .resizable(false)
        .show(contexts.ctx_mut()?, |ui| {
            for id in LessonId::ALL {
                if ui
                    .selectable_label(
                        drive.lesson.as_ref().is_some_and(|l| l.id == id),
                        id.title(),
                    )
                    .clicked()
                {
                    start = Some(Some(id));
                }
            }
            if ui
                .selectable_label(drive.lesson.is_none(), "Free drive")
                .clicked()
            {
                start = Some(None);
            }
            if let Some(run) = &drive.lesson {
                ui.separator();
                ui.label(run.id.brief());
                match drive.outcome {
                    Outcome::Running => {}
                    Outcome::Passed { stars } => {
                        ui.heading(format!(
                            "Passed {}",
                            "★".repeat(stars as usize) + &"☆".repeat(3 - stars as usize)
                        ));
                    }
                    Outcome::Failed(why) => {
                        ui.colored_label(egui::Color32::RED, why);
                    }
                }
                if ui.button("Retry (Enter)").clicked() {
                    start = Some(Some(run.id));
                }
            }
            ui.separator();
            ui.small(KEYS);
        });
    if start.is_none() && drive.lesson.is_some() && drive.outcome != Outcome::Running {
        // Enter retries a finished lesson without reaching for the mouse.
        if contexts
            .ctx_mut()?
            .input(|i| i.key_pressed(egui::Key::Enter))
        {
            start = drive.lesson.as_ref().map(|l| Some(l.id));
        }
    }
    if let Some(choice) = start {
        drive.start(choice, seed, &mut pedals);
    }
    Ok(())
}
```

- [ ] **Step 2:** Replace `crates/game/src/main.rs` (drops the temporary `allow(dead_code)`):

```rust
mod cockpit;
mod driving;
mod hud;
mod input;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use leafwing_input_manager::prelude::*;

#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AppState {
    /// Browsers only allow audio after a user gesture, so we wait for one.
    #[default]
    ClickToStart,
    Driving,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "miata — learn to drive stick".into(),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(InputManagerPlugin::<input::Action>::default())
        .init_state::<AppState>()
        .add_plugins((input::plugin, driving::plugin, cockpit::plugin, hud::plugin))
        .run();
}
```

- [ ] **Step 3: Verify on CI** — Push and wait for CI (the game crate is never compiled locally):

```bash
git push && gh run watch --exit-status $(gh run list --branch "$(git branch --show-current)" --limit 1 --json databaseId -q '.[0].databaseId')
```

Expected: `check` and `web` jobs green. On failure: `gh run view --log-failed`, fix, push again.

- [ ] **Step 4:** **Checkpoint — the user commits** (the agent does not run git):

```bash
git add crates/game
git commit -m "feat(game): egui HUD, start screen and lesson panel"
```

---

### Task 8: Engine sound

**Files:**
- Create: `crates/game/src/audio.rs`
- Modify: `crates/game/src/main.rs`

**Interfaces:**
- Consumes: `AppState`, `Drive`, `Pedals`.
- Produces: `EngineSound` (`Decodable` asset) whose decoder reads rpm/load/on atomics; spawned on `OnEnter(Driving)` (after the user gesture); fed every frame.

- [ ] **Step 1:** `crates/game/src/audio.rs` (includes a test: silent when off, audible when running):

```rust
//! Procedural engine sound. The decoder runs on the audio thread and reads
//! rpm/load/running from atomics the game updates each frame.

use std::f32::consts::TAU;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::time::Duration;

use bevy::audio::{AddAudioSource, ChannelCount, SampleRate, Source};
use bevy::prelude::*;

use crate::AppState;
use crate::driving::Drive;
use crate::input::Pedals;

const SAMPLE_RATE: u32 = 44_100;
/// 4-cylinder 4-stroke: two firing pulses per crank revolution.
const PULSES_PER_REV: f32 = 2.0;
/// Re-read the atomics every this many samples.
const CONTROL_EVERY: u32 = 64;

#[derive(Default)]
pub struct EngineParams {
    rpm: AtomicU32,
    load: AtomicU32,
    /// 1.0 running, 0.0 off (as f32 bits).
    on: AtomicU32,
}

impl EngineParams {
    fn set(&self, rpm: f32, load: f32, on: bool) {
        self.rpm.store(rpm.to_bits(), Relaxed);
        self.load.store(load.to_bits(), Relaxed);
        self.on
            .store(if on { 1.0f32 } else { 0.0 }.to_bits(), Relaxed);
    }
    fn get(&self) -> (f32, f32, f32) {
        let f = |a: &AtomicU32| f32::from_bits(a.load(Relaxed));
        (f(&self.rpm), f(&self.load), f(&self.on))
    }
}

#[derive(Asset, TypePath)]
pub struct EngineSound(Arc<EngineParams>);

#[derive(Resource)]
struct Engine(Arc<EngineParams>);

pub struct EngineDecoder {
    params: Arc<EngineParams>,
    phase: f32,
    rpm: f32,
    load: f32,
    amp: f32,
    target: (f32, f32, f32),
    n: u32,
    noise: u32,
}

impl EngineDecoder {
    fn new(params: Arc<EngineParams>) -> Self {
        Self {
            params,
            phase: 0.0,
            rpm: 0.0,
            load: 0.0,
            amp: 0.0,
            target: (0.0, 0.0, 0.0),
            n: 0,
            noise: 0x1234_5678,
        }
    }
}

impl Iterator for EngineDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.n.is_multiple_of(CONTROL_EVERY) {
            self.target = self.params.get();
        }
        self.n = self.n.wrapping_add(1);
        let (rpm, load, on) = self.target;
        // One-pole smoothing so parameter steps don't click.
        self.rpm += (rpm - self.rpm) * 0.002;
        self.load += (load - self.load) * 0.002;
        self.amp += (on - self.amp) * 0.0005;

        let freq = self.rpm / 60.0 * PULSES_PER_REV;
        self.phase = (self.phase + freq / SAMPLE_RATE as f32).fract();
        let p = self.phase * TAU;
        let tone =
            p.sin() + 0.5 * (2.0 * p).sin() + 0.25 * (3.0 * p).sin() + 0.12 * (0.5 * p).sin();
        // xorshift noise, louder under load: the "working" rasp.
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        let noise = (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0;
        Some((tone * (0.6 + 0.4 * self.load) + noise * 0.15 * self.load) * self.amp * 0.15)
    }
}

impl Source for EngineDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(SAMPLE_RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Decodable for EngineSound {
    type Decoder = EngineDecoder;
    fn decoder(&self) -> Self::Decoder {
        EngineDecoder::new(self.0.clone())
    }
}

pub fn plugin(app: &mut App) {
    app.add_audio_source::<EngineSound>()
        .insert_resource(Engine(Arc::default()))
        // Start only after the click-to-start gesture so browsers allow audio.
        .add_systems(OnEnter(AppState::Driving), start_engine_sound)
        .add_systems(
            Update,
            feed_engine_sound.run_if(in_state(AppState::Driving)),
        );
}

fn start_engine_sound(
    mut commands: Commands,
    mut sounds: ResMut<Assets<EngineSound>>,
    engine: Res<Engine>,
) {
    commands.spawn(AudioPlayer(sounds.add(EngineSound(engine.0.clone()))));
}

fn feed_engine_sound(engine: Res<Engine>, drive: Res<Drive>, pedals: Res<Pedals>) {
    let s = drive.sim.state();
    engine
        .0
        .set(s.engine_rpm, pedals.controls.throttle, s.engine_running);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(d: &mut EngineDecoder, n: usize) -> f32 {
        (d.take(n).map(|x| x * x).sum::<f32>() / n as f32).sqrt()
    }

    #[test]
    fn silent_when_off_audible_when_running() {
        let params = Arc::new(EngineParams::default());
        let mut d = EngineDecoder::new(params.clone());
        params.set(3000.0, 0.0, false);
        assert!(rms(&mut d, 44_100) < 1e-4);
        params.set(3000.0, 0.5, true);
        let _ = rms(&mut d, 44_100); // let the fade-in settle
        assert!(rms(&mut d, 44_100) > 0.02);
    }
}
```

- [ ] **Step 2:** Replace `crates/game/src/main.rs` with the final version:

```rust
mod audio;
mod cockpit;
mod driving;
mod hud;
mod input;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use leafwing_input_manager::prelude::*;

#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AppState {
    /// Browsers only allow audio after a user gesture, so we wait for one.
    #[default]
    ClickToStart,
    Driving,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "miata — learn to drive stick".into(),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(InputManagerPlugin::<input::Action>::default())
        .init_state::<AppState>()
        .add_plugins((
            input::plugin,
            driving::plugin,
            cockpit::plugin,
            hud::plugin,
            audio::plugin,
        ))
        .run();
}
```

- [ ] **Step 3: Verify on CI** — Push and wait for CI (the game crate is never compiled locally):

```bash
git push && gh run watch --exit-status $(gh run list --branch "$(git branch --show-current)" --limit 1 --json databaseId -q '.[0].databaseId')
```

Expected: `check` and `web` jobs green. On failure: `gh run view --log-failed`, fix, push again.

- [ ] **Step 4:** **Checkpoint — the user commits** (the agent does not run git):

```bash
git add crates/game
git commit -m "feat(game): procedural engine sound"
```

---

### Task 9: Web page and Pages deploy

**Files:**
- Create: `web/index.html`

**Interfaces:**
- Consumes: `dist/miata.js` + `miata_bg.wasm` from the `web` CI job.
- Produces: `https://lubabs770.github.io/miata/`.

- [ ] **Step 1:** `web/index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>miata — learn to drive stick</title>
    <style>
      html, body { margin: 0; height: 100%; background: #9ec7ed; overflow: hidden; }
      canvas { display: block; outline: none; }
    </style>
  </head>
  <body>
    <script>
      // Browsers create AudioContexts suspended until a user gesture. Bevy makes
      // its context at startup, before our click-to-start screen, so track every
      // context and resume them on the first click/key/touch.
      (() => {
        const Ctx = window.AudioContext || window.webkitAudioContext;
        if (!Ctx) return;
        const made = [];
        window.AudioContext = new Proxy(Ctx, {
          construct(target, args) {
            const c = new target(...args);
            made.push(c);
            return c;
          },
        });
        const resume = () => made.forEach((c) => c.state !== "running" && c.resume());
        for (const e of ["pointerdown", "keydown", "touchstart"]) document.addEventListener(e, resume);
      })();
    </script>
    <script type="module">
      import init from "./miata.js";
      init();
    </script>
  </body>
</html>
```

- [ ] **Step 2: Verify on CI** — Push and wait for CI (the game crate is never compiled locally):

```bash
git push && gh run watch --exit-status $(gh run list --branch "$(git branch --show-current)" --limit 1 --json databaseId -q '.[0].databaseId')
```

Expected: `check` and `web` jobs green. On failure: `gh run view --log-failed`, fix, push again.

- [ ] **Step 3:** **Checkpoint — the user commits** (the agent does not run git):

```bash
git add web
git commit -m "feat(web): wasm loader with audio-resume shim"
```

- [ ] **Step 4: User merges to `main` and pushes**; `deploy` job publishes. Expected: `gh run watch` shows `deploy` green; `bopen -p https://lubabs770.github.io/miata/` loads the start screen.

---

### Task 10: Playtest and tune (user + agent)

**Files:**
- Modify (only if tuning is needed): `cars/miata.toml`, constants in `crates/game/src/input.rs`, `crates/drivetrain/src/score.rs`

- [ ] **Step 1: User plays the Pages build** with keyboard, then a gamepad, and reports against this checklist:
  1. Start screen shows; first click starts engine sound.
  2. Lesson 1 passes holding the clutch at the bite point; releasing fully stalls.
  3. Lesson 2 passes with gas + slow release; dumping the clutch stalls.
  4. Lesson 3 passes with clutch in before ~1000 rpm; braking in gear without clutch stalls.
  5. A jerky launch visibly spills the coffee and costs a star.
- [ ] **Step 2: Tune** from the reports: bite feel → `bite_width`, `clutch_capacity_nm`; stalls too easy/hard → `idle_max_throttle`; keyboard clutch too fast/slow → `CLUTCH_KEY_RELEASE_RATE`; cup too sensitive → `SPILL_JERK`. After any `cars/miata.toml` or `score.rs` change run `cargo test -p drivetrain` locally (seconds) before pushing.
- [ ] **Step 3: Verify on CI**, then checkpoint `tune: playtest adjustments`.

Phase 1a is done when the spec's Verification section holds: drivetrain tests green locally, CI green, Pages build loads, audio starts after click, lessons 1–3 completable with keyboard and gamepad, clutch dump stalls, coffee spills on a jerky launch.

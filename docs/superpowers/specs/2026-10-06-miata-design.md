# miata — stick-shift driving trainer (design spec)

Date: 2026-10-06
Status: approved 2026-10-06

## Goal

A Rust game (web, macOS, Linux) that teaches someone to drive a manual
transmission well enough to get into a real car and do it. Also teaches
paddle/auto-clutch shifting for modern cars. Success = a player who completes
the Core + Hills tracks can pull away, shift, stop, and hill-start a real
manual car without stalling on the first few tries.

Honesty note shown in-game: with pad/keyboard the game trains sequence,
timing, and reading rpm/sound; physical pedal feel only transfers with real
pedals.

## Scope and phasing

- **Phase 1a — vertical slice (first plan, ~2–3 sessions):** `drivetrain`
  crate + tests, NA Miata only, Manual mode only, keyboard + gamepad via
  leafwing default bindings (no rebind UI yet), cockpit view only, lessons
  1–3, coffee cup, basic engine synth, "click to start" screen (web audio),
  wasm → GitHub Pages CI. Goal: play in a browser and tune clutch feel.
- **Phase 1b — full trainer (~6–9 sessions):** hot hatch + pickup,
  AutoClutch/Paddles modes, Hood/Chase/Bird's-eye views, rebinding screen,
  wheel/pedal calibration, lessons 4–15, ghost pedals, replay graph, free
  drive town, audio polish, persistence, macOS/Linux release CI.
- **Phase 2 (separate plan, after Phase 1 plays well):** OpenStreetMap
  real-street free drive, real elevation, "drive anywhere".

## Key dependencies

- `bevy` (pin latest stable at plan time; verify via Context7).
- `leafwing-input-manager` — actions, multi-binding, axes, runtime rebinding.
- `bevy_egui` — settings/controls screens, replay graph (`egui_plot`), lesson
  menus. In-world cockpit gauges stay Bevy meshes.
- `serde`, `toml`, `serde_json`, `rand` in `drivetrain`.

## Architecture

```
miata/
  Cargo.toml                 workspace
  crates/drivetrain/         pure Rust, no Bevy. Physics + scoring core.
  crates/game/               Bevy app (wasm + native).
  cars/*.toml                car definitions (data, not code)
  .github/workflows/         CI
  docs/superpowers/specs/    this file
```

Boundary: `drivetrain` exposes
`Sim::new(car: CarSpec, mode: TransmissionMode)` and
`Sim::step(&mut self, controls: Controls, env: Env, dt: f32) -> Events`
plus read-only `SimState`. The game never touches physics internals.

- `Controls { clutch, throttle, brake, handbrake: f32 (0..1), steer: f32 (-1..1), gear_request: GearRequest, ignition: bool }`
- `Env { grade: f32 (rise/run), surface_mu: f32 }`
- `SimState { engine_rpm, wheel_speed, vehicle_speed, gear, clutch_slip, clutch_wear, engine_running, heading, position_2d, longitudinal_accel, jerk }`
- `Events`: `Stalled`, `Grind`, `OverRev`, `Rollback(m)`, `ShiftRefused`, `Shifted(gear)`.

## Physics (`crates/drivetrain`)

Fixed internal substep of 1 ms regardless of frame dt.

- **Engine:** torque curve (rpm → Nm) × throttle, rotational inertia,
  friction/engine-braking torque, idle controller holding idle rpm (lets the
  car creep in 1st without throttle, like a real car). Stall when rpm < 400
  while drivetrain is coupled; restart via ignition. Rev limiter cuts fuel at
  limiter rpm.
- **Clutch:** pedal → clamp curve: dead travel, narrow bite band, full lock.
  Bite point randomized ±N% per session (N from car file). Slipping clutch
  transmits torque ∝ clamp × sign(slip); locks when slip ≈ 0 and demanded
  torque < capacity. Heat/wear accumulates from slip × torque.
- **Gearbox:** H-pattern gears per car + R. Gear change requires clutch
  > 85% depressed, else `Grind`, gear unchanged. Reverse only below 2 km/h.
  Downshift whose resulting rpm > limiter → `OverRev` (manual mode: allowed,
  flagged; auto modes: refused).
- **Vehicle:** longitudinal force = drive − aero drag − rolling resistance −
  grade·m·g − brake − handbrake; kinematic bicycle model for steering. No tire
  slip simulation. Handbrake holds car on grade; releasing without enough
  clutch/throttle → rollback.

### Transmission modes (any car)

| Mode | Clutch | Shifting |
|---|---|---|
| Manual | player | H-pattern |
| AutoClutch | computer | H-pattern or sequential +/- |
| Paddles | computer | +/- paddles / tip-shift |

Computer clutch: automatic launch, stop, and shift; cannot stall; refuses
over-revving downshifts.

## Cars (`cars/*.toml`)

Fields: `name`, `mass_kg`, `tire_radius_m`, `idle_rpm`, `redline_rpm`,
`limiter_rpm`, `torque_curve = [[rpm, nm], ...]`, `engine_inertia`,
`gear_ratios = [...]`, `reverse_ratio`, `final_drive`, `clutch_capacity_nm`,
`bite_point`, `bite_variance`, `drag_cd_a`, `rolling_c`,
`default_mode`, `dash` (`round` | `digital` | `truck`).

Shipped:
1. **NA Miata 1.6 5MT** (default): 960 kg, idle 850, redline 7000, limiter
   7200, ratios 3.136/1.888/1.330/1.000/0.814, R 3.758, FD 4.30, tire 0.29 m.
2. **Hot hatch 6MT**: more torque, heavier clutch, higher bite point.
3. **Old pickup 4MT**: heavy, low-revving, stall-prone. Hard mode.

Generic names except the Miata (repo namesake). Adding a car = adding a file.

## Input and key bindings

- All devices (keyboard, gamepad, wheel+pedals via Bevy gamepad) map through
  `leafwing-input-manager` actions, then normalize to `Controls`. Keyboard axes ramp at a configurable rate.
- **Controls screen:** pick action → press key/button/axis to bind; multiple
  bindings per action active simultaneously; presets (Keyboard, Xbox/PS pad,
  Wheel+pedals); reset to defaults; conflict warning with swap.
- **Per axis:** deadzone, invert, sensitivity curve (linear / progressive),
  range calibration ("press clutch fully").
- Rumble at bite point and pre-stall (native only).
- Bindings persist with settings.

## Views and cockpit

- Views (cycle with `V` / pad button): Cockpit (default), Hood, Chase,
  Bird's-eye. Non-cockpit views show a HUD overlay: tach, gear, pedal bars,
  coffee cup.
- Low-poly flat-shaded, built from Bevy primitives; glTF swap-in later.
- Cockpit: live tach + speedo, gear readout, coffee cup, animated pedals and
  H-pattern shifter knob; dash style per car.

## Audio

Procedural engine synth: firing frequency = rpm/60 × cylinders/2, harmonics,
load-dependent roughness. Effects: grind, stall shudder, handbrake ratchet,
coffee splash.

## Lessons and scoring

Lessons are Rust structs: spawn point, initial state, car/mode, goal check,
fail check, coaching hints.

| Track | Lessons |
|---|---|
| Core (manual) | 1 Find bite point (hold 3 s) · 2 Pull away · 3 Stop without stalling · 4 Upshift 1→2→3 · 5 Slow + downshift 3→2 · 6 Free drive loop |
| Hills | 7 Handbrake hill start (8%) · 8 No-handbrake hill start (rollback < 0.5 m) |
| Real-world | 9 Parking-lot creep into box · 10 Stop-and-go behind AI car · 11 Stop sign + turn · 12 Stop sign on a hill |
| Paddles/auto-clutch | 13 Shift points · 14 Engine braking downhill · 15 Gear choice before a corner |

- **Coaching:** live prompts ("RPM dropping — clutch in!"), ghost pedals on
  first attempt, post-attempt replay graph (clutch/throttle/rpm vs time, stall
  marked).
- **Scoring:** 1–3 stars from stalls, grinds, rollback distance, clutch wear,
  jerk. **Coffee cup** on dash spills with jerk.
- **Free drive:** no stars; session log (stalls, spills, distance).
- **World:** one low-poly town: flat streets, hill road, intersections with
  stop signs, parking lot. Lessons spawn within it.
- **Persistence:** JSON in platform config dir (native), `localStorage` (web).

## Phase 2: real streets (separate plan)

- Place search via Nominatim → ~1 km² fetch via Overpass → build roads (width
  from `highway`/`lanes`), extruded buildings (`height`/`building:levels`),
  `highway=stop` / traffic signals, `maxspeed` on HUD. Cache fetched areas.
- 2–3 bundled areas for offline play.
- Elevation from AWS Terrain Tiles (terrarium).
- Collision with building footprints and curbs (stop + penalty, no damage).
- On-screen "© OpenStreetMap contributors" (ODbL). Respect 1 req/s and set a
  User-Agent.

## Build and CI (GitHub Actions)

| Job | Trigger | Output |
|---|---|---|
| fmt + clippy + test | every push / PR | pass/fail |
| wasm build → GitHub Pages | push to `main` | `lubabs770.github.io/miata` |
| macOS universal `.app` (unsigned) | tag `v*` | GitHub Release |
| Linux x86_64 `.tar.gz` | tag `v*` | GitHub Release |

Locally: only `cargo test -p drivetrain` (pure Rust, seconds). Anything that
compiles Bevy — check, clippy, test, build — runs on GitHub Actions only. Repo is personal
(`lubabs770/miata`, noreply commit email); the user runs commit/push.

## Testing

- `drivetrain` unit tests: slow release at idle pulls away without stall;
  clutch dump at idle stalls; 10% grade handbrake release with no throttle
  rolls back; shift without clutch grinds; 5→2 at 100 km/h flags OverRev
  (manual) / refused (auto modes); auto-clutch full throttle from stop never
  stalls; every `cars/*.toml` loads and pulls away.
- Lesson goal checks replayed against recorded input traces (clean run
  passes, stalled run fails).
- Bindings: defaults have no conflicts; settings round-trip.
- `game`: headless Bevy smoke test (app builds, car spawns, no window).

## Verification

- Phase 1a done when: `cargo test -p drivetrain` green locally; CI green;
  GitHub Pages build loads, audio starts after click, lessons 1–3 completable
  with keyboard and with a gamepad, a clutch dump stalls, coffee spills on a
  jerky launch.

## Out of scope (Phase 1)

Advanced technique (rev-match/heel-toe lessons), tire slip/drifting, damage,
multiplayer, code signing/notarization, mobile.

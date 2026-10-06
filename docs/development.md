# Development

## Layout

```
cars/                 car definitions (TOML)
crates/drivetrain/    pure-Rust sim, scoring and lessons; no Bevy
crates/game/          Bevy 0.19 app: input, driving loop, cockpit, HUD, audio
web/index.html        wasm loader and audio-resume shim
docs/                 these docs, plus the design spec and plans
```

| Module | Responsibility |
|---|---|
| `game/src/input.rs` | Turns leafwing-input-manager actions into `drivetrain::Controls` (keyboard pedals ramp; the right stick is an H-pattern shifter) |
| `game/src/driving.rs` | The `Drive` resource; steps the sim and the active lesson each frame |
| `game/src/cockpit.rs` | The world and the cockpit, built from Bevy primitives |
| `game/src/hud.rs` | egui screens: start screen, instruments, hints, lesson panel |
| `game/src/audio.rs` | Engine sound generated in code, fed through atomics |

## Tests

The physics crate is small and fast, so run it locally:

```bash
cargo test -p drivetrain
```

**Don't compile the Bevy crate locally.** Bevy's dependency tree is heavy, so
check, clippy, test and build for `crates/game` all run on GitHub Actions.

## CI and deploys

`.github/workflows/ci.yml` runs on every push:

- **check:** `cargo fmt --check`, `cargo clippy -D warnings` and
  `cargo test --workspace`.
- **web:** a release build for `wasm32-unknown-unknown`, then `wasm-bindgen`
  (the CLI version matches `Cargo.lock`), bundled as a Pages artifact.
- **deploy:** on pushes to `main` only, publishes to
  https://lubabs770.github.io/miata/.

To watch the latest run:

```bash
gh run watch --exit-status $(gh run list --limit 1 --json databaseId -q '.[0].databaseId')
```

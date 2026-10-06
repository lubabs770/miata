# How the physics works

All of it lives in `crates/drivetrain` (pure Rust, no Bevy). The game calls
`Sim::step(&controls, env, dt)` once per frame. Internally the sim runs
1 ms substeps, because a clutch locking and unlocking is too stiff to
simulate at frame rate.

## Engine

- **Torque:** the full-throttle torque curve from the car file, scaled by a
  progressive pedal map `1 − (1 − pedal)²`. Part throttle gives more than
  proportional torque, as a real intake does.
- **Friction and engine braking:** `friction_nm + friction_per_rad_s × ω`.
- **Idle controller:** adds throttle as rpm sags below idle. Its authority is
  capped at `idle_max_throttle`. That is enough to creep away very gently with
  no gas, but not enough to survive a dumped clutch.
- **Stall:** if rpm drops below `stall_rpm` the engine stops. You restart it
  with the ignition, with the clutch in or in neutral.
- **Rev limiter:** fuel is cut at `limiter_rpm`.

## Clutch

- **Pedal travel:** `clutch = 0` is the pedal released and `1` is floored.
- **Engagement:** rises linearly over `bite_width` of travel below this
  session's bite point. The bite point is `bite_point ± bite_variance`, picked
  by seed.
- **Torque capacity:** `engagement² × clutch_capacity_nm`. The squared term
  makes the first part of the band grab gently.
- **Slipping:** the clutch passes its capacity in the direction of slip and
  heats up. The total heat goes into `clutch_heat_j` and costs a star when
  high.
- **Locked:** the engine and car turn as one inertia. The clutch unlocks when
  the torque needed to stay locked exceeds its capacity, and relocks when slip
  crosses zero.

## Gearbox

- **Clutch required:** a gear change needs the clutch more than 85% down
  (`SHIFT_CLUTCH`). Otherwise you get `Event::Grind` and the gear stays put.
- **Neutral:** you can always pull into neutral, even without the clutch.
- **Reverse:** only engages below about 2 km/h.
- **Over-rev:** a downshift that would put the engine past the limiter emits
  `Event::OverRev`. In manual mode the shift still happens; that's the "money
  shift".

## Car

- **Forces:** the car moves under drive force minus grade × weight minus
  aerodynamic drag.
- **Brakes:** the brakes, handbrake and rolling resistance oppose motion, hold
  a stopped car, and never push it backwards.
- **Steering:** a kinematic bicycle model. There's no tyre slip yet.
- **Rollback:** `rollback_m` totals the distance the car has rolled backwards
  while not in reverse.

## Coffee cup

`Cup::update(jerk, dt)` spills when the smoothed jerk (the rate of change of
acceleration) goes above `SPILL_JERK` (6 m/s³). Smooth driving keeps it full;
dumping the clutch or snapping the brakes doesn't.

## Tuning

The launch tests in `crates/drivetrain/tests/physics.rs` encode real-car
behaviour:
- A very slow release with no gas pulls away.
- A 1.5 s release with a little gas pulls away.
- Dumping the clutch at idle stalls.
- Braking to a stop in gear stalls unless the clutch is in.

If a change to a car file breaks one of them, tune the car, not the test.

| Feels wrong | Knob |
|---|---|
| Bite point too narrow or too wide | `bite_width` |
| Grabs too hard | `clutch_capacity_nm` |
| Stalls too easily, or too hard to stall | `idle_max_throttle` |
| Engine revs too slowly or too quickly | `engine_inertia` |
| Coffee too sensitive | `SPILL_JERK` in `score.rs` |

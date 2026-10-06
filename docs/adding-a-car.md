# Adding a car

Cars are plain TOML files in `cars/`. `CarSpec::from_toml` loads them, and the
test `every_bundled_car_file_parses` checks that every file in the folder is
valid.

Start by copying `cars/miata.toml`, then change these fields:

| Field | Meaning |
|---|---|
| `mass_kg`, `wheelbase_m`, `tire_radius_m` | Body and tyres |
| `idle_rpm`, `stall_rpm`, `redline_rpm`, `limiter_rpm` | Engine speeds |
| `torque_curve` | `[rpm, Nm]` pairs at full throttle, ascending rpm |
| `gear_ratios`, `reverse_ratio`, `final_drive` | Gearbox; the number of entries sets the number of gears |
| `engine_inertia`, `friction_nm`, `friction_per_rad_s` | How quickly the engine revs and how hard it engine-brakes |
| `idle_base_throttle`, `idle_gain`, `idle_max_throttle` | Idle controller (see [physics](physics.md)) |
| `clutch_capacity_nm`, `bite_point`, `bite_width`, `bite_variance` | Clutch feel |
| `drag_cd_a`, `rolling_c` | Drag coefficient × frontal area, and rolling resistance coefficient |
| `brake_force_n`, `handbrake_force_n`, `max_steer_rad` | Brakes and steering |

Use published figures where you have them: ratios, final drive, mass, idle and
redline. The feel values (inertia, friction, idle controller and clutch) need
playtesting.

Phase 1a only drives the Miata. Choosing a car in the game is part of
Phase 1b.

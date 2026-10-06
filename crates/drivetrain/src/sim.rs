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

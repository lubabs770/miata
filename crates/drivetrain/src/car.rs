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

/// Car files compiled into the binary (the browser build can't read the disk).
/// Miata first: it's the default.
const BUNDLED: [&str; 3] = [
    include_str!("../../../cars/miata.toml"),
    include_str!("../../../cars/hot_hatch.toml"),
    include_str!("../../../cars/pickup.toml"),
];

impl CarSpec {
    pub fn bundled() -> Vec<CarSpec> {
        BUNDLED
            .iter()
            .map(|s| Self::from_toml(s).expect("bundled car files are valid"))
            .collect()
    }

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

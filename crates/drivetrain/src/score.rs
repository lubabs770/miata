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

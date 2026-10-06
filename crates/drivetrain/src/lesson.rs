use crate::score::{Cup, stars};
use crate::sim::{Controls, Env, Event, Sim, SimState};

/// Slope used by the hill lessons (8%: a steep-ish town street).
pub const HILL_GRADE: f32 = 0.08;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LessonId {
    BitePoint,
    PullAway,
    Stop,
    Upshift,
    Downshift,
    FreeLoop,
    HillHandbrake,
    HillNoHandbrake,
}

impl LessonId {
    pub const ALL: [LessonId; 8] = [
        LessonId::BitePoint,
        LessonId::PullAway,
        LessonId::Stop,
        LessonId::Upshift,
        LessonId::Downshift,
        LessonId::FreeLoop,
        LessonId::HillHandbrake,
        LessonId::HillNoHandbrake,
    ];

    pub fn title(self) -> &'static str {
        match self {
            LessonId::BitePoint => "1. Find the bite point",
            LessonId::PullAway => "2. Pull away",
            LessonId::Stop => "3. Stop without stalling",
            LessonId::Upshift => "4. Upshift 1→2→3",
            LessonId::Downshift => "5. Slow down and downshift",
            LessonId::FreeLoop => "6. Drive 500 m",
            LessonId::HillHandbrake => "7. Hill start (handbrake)",
            LessonId::HillNoHandbrake => "8. Hill start (foot brake)",
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
            LessonId::Upshift => {
                "Pull away, then shift up at around 2500–3000 rpm: off the gas, clutch in, next \
                 gear, clutch out, back on the gas. Reach 3rd gear and 25 km/h."
            }
            LessonId::Downshift => {
                "You're doing 50 km/h in 3rd. Brake to about 20–30 km/h, then clutch in, select \
                 2nd, and let the clutch out smoothly. Hold 2nd for 2 seconds."
            }
            LessonId::FreeLoop => {
                "Put it all together: start off, shift through the gears and cover 500 m \
                 without stalling. Keep the coffee in the cup."
            }
            LessonId::HillHandbrake => {
                "You're on an 8% hill with the handbrake on. Clutch in, 1st, then add gas \
                 (about 2000 rpm) and lift the clutch to the bite point until the car squats. \
                 Release the handbrake and drive 20 m without rolling back."
            }
            LessonId::HillNoHandbrake => {
                "Same hill, no handbrake help. Hold the foot brake, release the handbrake, then \
                 move your foot from brake to gas quickly while lifting to the bite point. \
                 Drive 20 m rolling back less than half a metre."
            }
        }
    }

    pub fn is_hill(self) -> bool {
        matches!(self, LessonId::HillHandbrake | LessonId::HillNoHandbrake)
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
            LessonId::PullAway | LessonId::Upshift | LessonId::FreeLoop => true,
            LessonId::Stop => {
                sim.set_moving(2, 30.0 / 3.6);
                false
            }
            LessonId::Downshift => {
                sim.set_moving(3, 50.0 / 3.6);
                false
            }
            LessonId::HillHandbrake | LessonId::HillNoHandbrake => true,
        }
    }

    pub fn env(&self) -> Env {
        Env {
            grade: if self.id.is_hill() { HILL_GRADE } else { 0.0 },
        }
    }

    pub fn update(&mut self, s: &SimState, c: &Controls, events: &[Event], dt: f32) -> Outcome {
        self.cup.update(s.jerk, dt);
        self.grinds += events.iter().filter(|e| **e == Event::Grind).count() as u32;
        if events.contains(&Event::Stalled) {
            return Outcome::Failed("Stalled. Clutch in, turn the key, try again.");
        }
        let rollback_limit = match self.id {
            LessonId::HillHandbrake => Some(1.0),
            LessonId::HillNoHandbrake => Some(0.5),
            _ => None,
        };
        if rollback_limit.is_some_and(|max| s.rollback_m > max) {
            return Outcome::Failed("Rolled back too far. More gas and find the bite sooner.");
        }
        // The foot-brake technique means the handbrake is off before the clutch bites.
        if self.id == LessonId::HillNoHandbrake && c.handbrake > 0.0 && s.clutch_engagement > 0.05 {
            return Outcome::Failed(
                "That used the handbrake. Release it first, on the foot brake.",
            );
        }
        let locked_in = |g: i8| s.gear == g && s.clutch_engagement >= 1.0;
        let goal_met = match self.id {
            LessonId::BitePoint => {
                s.gear == 1 && (0.05..0.6).contains(&s.clutch_engagement) && s.engine_running
            }
            LessonId::PullAway => {
                s.gear >= 1 && s.speed_mps * 3.6 >= 10.0 && s.clutch_engagement >= 1.0
            }
            LessonId::Stop => s.speed_mps.abs() < 0.05 && s.engine_running,
            LessonId::Upshift => locked_in(3) && s.speed_mps * 3.6 >= 25.0,
            LessonId::Downshift => locked_in(2) && (15.0..=32.0).contains(&(s.speed_mps * 3.6)),
            LessonId::FreeLoop => s.distance_m >= 500.0,
            LessonId::HillHandbrake | LessonId::HillNoHandbrake => {
                s.x >= 20.0 && s.clutch_engagement >= 1.0
            }
        };
        let hold_s = match self.id {
            LessonId::BitePoint => 3.0,
            LessonId::Stop | LessonId::Downshift => 2.0,
            _ => 0.0,
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
        if self.id.is_hill() && s.speed_mps < -0.05 {
            return Some("Rolling back! More gas, clutch up to the bite.");
        }
        match self.id {
            LessonId::BitePoint if s.gear == 1 && s.clutch_engagement > 0.05 => {
                Some("That's the bite point. Hold it.")
            }
            LessonId::PullAway | LessonId::Upshift | LessonId::FreeLoop
                if s.gear == 0 && c.clutch < 0.85 =>
            {
                Some("Press the clutch all the way in, then select 1st.")
            }
            LessonId::PullAway | LessonId::Upshift | LessonId::FreeLoop
                if s.gear >= 1 && c.handbrake > 0.0 =>
            {
                Some("Release the handbrake.")
            }
            LessonId::Stop if coupled && s.engine_rpm < 1100.0 => Some("Clutch in now."),
            LessonId::Upshift if coupled && s.gear < 3 && s.engine_rpm > 3000.0 => {
                Some("Shift up: off the gas, clutch in, next gear.")
            }
            LessonId::Downshift if s.gear == 3 && s.speed_mps * 3.6 < 30.0 => {
                Some("Clutch in and select 2nd.")
            }
            LessonId::HillHandbrake
                if s.gear == 1 && c.handbrake > 0.0 && s.clutch_engagement > 0.3 =>
            {
                Some("Car's squatting — release the handbrake now.")
            }
            LessonId::HillNoHandbrake if c.handbrake > 0.0 => {
                Some("Hold the foot brake, then release the handbrake.")
            }
            _ => None,
        }
    }
}

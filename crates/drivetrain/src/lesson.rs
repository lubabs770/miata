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

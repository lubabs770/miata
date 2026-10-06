use crate::score::{Cup, stars};
use crate::sim::{Controls, Env, Event, Sim, SimState};

/// Slope used by the hill lessons (8%: a steep-ish town street).
pub const HILL_GRADE: f32 = 0.08;
/// Parking lesson: stop with the car's reference point inside this x range.
pub const PARK_BOX: (f32, f32) = (18.0, 20.5);
/// Stop-sign lessons: the line's distance ahead of the start.
pub const STOP_LINE: f32 = 40.0;
pub const HILL_STOP_LINE: f32 = 25.0;
/// Bumper-to-reference distance used for following gaps.
pub const CAR_LEN: f32 = 4.0;
/// Stop-and-go: how many times the car in front starts and stops.
const TRAFFIC_CYCLES: u8 = 4;

/// The car in front in the stop-and-go lesson: waits, pulls away to
/// 20 km/h, cruises, brakes to a stop, repeats.
#[derive(Debug, Clone, Copy)]
pub struct Lead {
    pub x: f32,
    v: f32,
    phase: LeadPhase,
    phase_t: f32,
    cycles: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LeadPhase {
    Accelerate,
    Cruise,
    Brake,
    Wait,
}

impl Lead {
    fn new() -> Self {
        Self {
            x: 12.0,
            v: 0.0,
            phase: LeadPhase::Wait,
            phase_t: 4.0,
            cycles: 0,
        }
    }

    fn step(&mut self, dt: f32) {
        match self.phase {
            LeadPhase::Accelerate => {
                self.v += 1.5 * dt;
                if self.v >= 5.5 {
                    self.v = 5.5;
                    self.phase = LeadPhase::Cruise;
                    self.phase_t = 3.0;
                }
            }
            LeadPhase::Cruise => {
                self.phase_t -= dt;
                if self.phase_t <= 0.0 {
                    self.phase = LeadPhase::Brake;
                }
            }
            LeadPhase::Brake => {
                self.v -= 2.0 * dt;
                if self.v <= 0.0 {
                    self.v = 0.0;
                    self.cycles += 1;
                    self.phase = LeadPhase::Wait;
                    self.phase_t = 3.0;
                }
            }
            LeadPhase::Wait => {
                self.phase_t -= dt;
                if self.phase_t <= 0.0 && self.cycles < TRAFFIC_CYCLES {
                    self.phase = LeadPhase::Accelerate;
                }
            }
        }
        self.x += self.v * dt;
    }

    pub fn done(&self) -> bool {
        self.cycles >= TRAFFIC_CYCLES
    }
}

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
    Parking,
    Traffic,
    StopSign,
    HillStopSign,
}

impl LessonId {
    pub const ALL: [LessonId; 12] = [
        LessonId::BitePoint,
        LessonId::PullAway,
        LessonId::Stop,
        LessonId::Upshift,
        LessonId::Downshift,
        LessonId::FreeLoop,
        LessonId::HillHandbrake,
        LessonId::HillNoHandbrake,
        LessonId::Parking,
        LessonId::Traffic,
        LessonId::StopSign,
        LessonId::HillStopSign,
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
            LessonId::Parking => "9. Car-park creep",
            LessonId::Traffic => "10. Stop-and-go traffic",
            LessonId::StopSign => "11. Stop sign and turn",
            LessonId::HillStopSign => "12. Stop sign on a hill",
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
            LessonId::Parking => {
                "Creep into the marked space about 19 m ahead and stop inside it. Stay under \
                 10 km/h: slip the clutch at the bite point and cover the brake."
            }
            LessonId::Traffic => {
                "Follow the car in front as it stops and starts four times. Don't get closer \
                 than a metre, don't fall more than 40 m behind, and don't stall."
            }
            LessonId::StopSign => {
                "Stop sign ahead. Brake, clutch in before the revs drop, and come to a full \
                 stop at the line. Then select 1st, pull away and turn left."
            }
            LessonId::HillStopSign => {
                "Stop sign on an uphill. Stop fully at the line, then do a hill start and \
                 drive 20 m on, rolling back less than half a metre."
            }
        }
    }

    pub fn is_hill(self) -> bool {
        matches!(
            self,
            LessonId::HillHandbrake | LessonId::HillNoHandbrake | LessonId::HillStopSign
        )
    }

    /// Where the stop line is, for lessons that have one.
    pub fn stop_line(self) -> Option<f32> {
        match self {
            LessonId::StopSign => Some(STOP_LINE),
            LessonId::HillStopSign => Some(HILL_STOP_LINE),
            _ => None,
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
    /// Stop-and-go only: the car in front.
    pub lead: Option<Lead>,
    stopped_at_line: bool,
    timer_s: f32,
}

impl LessonRun {
    pub fn new(id: LessonId) -> Self {
        Self {
            id,
            cup: Cup::default(),
            grinds: 0,
            lead: (id == LessonId::Traffic).then(Lead::new),
            stopped_at_line: false,
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
            LessonId::Parking | LessonId::Traffic => true,
            LessonId::StopSign => {
                sim.set_moving(2, 30.0 / 3.6);
                false
            }
            LessonId::HillStopSign => {
                sim.set_moving(2, 20.0 / 3.6);
                false
            }
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
        if let Some(lead) = &mut self.lead {
            lead.step(dt);
            let gap = lead.x - s.x - CAR_LEN;
            if gap < 1.0 {
                return Outcome::Failed("Too close — you hit the car in front.");
            }
            if gap > 40.0 && !lead.done() {
                return Outcome::Failed("Fell too far behind. Keep up with the traffic.");
            }
        }
        let stopped = s.speed_mps.abs() < 0.05 && s.engine_running;
        if let Some(line) = self.id.stop_line() {
            if stopped && (line - 5.0..=line).contains(&s.x) {
                self.stopped_at_line = true;
            }
            if !self.stopped_at_line && s.x > line {
                return Outcome::Failed("Rolled through the stop sign. Stop fully at the line.");
            }
        }
        if self.id == LessonId::Parking {
            if s.speed_mps * 3.6 > 10.0 {
                return Outcome::Failed(
                    "Too fast for a car park. Slip the clutch, cover the brake.",
                );
            }
            if s.x > PARK_BOX.1 + 2.0 {
                return Outcome::Failed("Overshot the space.");
            }
        }
        let rollback_limit = match self.id {
            LessonId::HillHandbrake => Some(1.0),
            LessonId::HillNoHandbrake | LessonId::HillStopSign => Some(0.5),
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
            LessonId::Parking => stopped && (PARK_BOX.0..=PARK_BOX.1).contains(&s.x),
            LessonId::Traffic => self.lead.is_some_and(|l| l.done()) && stopped,
            LessonId::StopSign => self.stopped_at_line && s.heading >= 1.3,
            LessonId::HillStopSign => {
                self.stopped_at_line && s.x >= HILL_STOP_LINE + 20.0 && s.clutch_engagement >= 1.0
            }
        };
        let hold_s = match self.id {
            LessonId::BitePoint => 3.0,
            LessonId::Parking | LessonId::Traffic => 1.0,
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
            LessonId::Parking | LessonId::Traffic | LessonId::StopSign | LessonId::HillStopSign
                if s.gear >= 1 && c.handbrake > 0.0 =>
            {
                Some("Release the handbrake.")
            }
            LessonId::Parking if s.speed_mps * 3.6 > 7.0 => {
                Some("Slower: slip the clutch, cover the brake.")
            }
            LessonId::Traffic
                if self.lead.is_some_and(|l| l.x - s.x - CAR_LEN < 4.0) && s.speed_mps > 0.5 =>
            {
                Some("Too close — clutch in and brake.")
            }
            LessonId::StopSign | LessonId::HillStopSign if self.stopped_at_line => {
                Some(if self.id == LessonId::StopSign {
                    "Good stop. Now pull away and turn left."
                } else {
                    "Good stop. Hill start: gas, bite, off the brake."
                })
            }
            LessonId::StopSign | LessonId::HillStopSign
                if self.id.stop_line().is_some_and(|l| s.x > l - 20.0) =>
            {
                Some("Stop sign: brake, clutch in, stop at the line.")
            }
            _ => None,
        }
    }
}

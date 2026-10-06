//! Owns the simulation and the active lesson; steps them once per frame.

use bevy::prelude::*;
use drivetrain::{
    CarSpec, Controls, Cup, Event, LessonId, LessonRun, Outcome, Sim, TransmissionMode,
};

use crate::AppState;
use crate::input::Pedals;

/// One frame of a lesson attempt, for the replay graph.
#[derive(Clone, Copy)]
pub struct Sample {
    pub t: f32,
    pub clutch: f32,
    pub throttle: f32,
    pub brake: f32,
    /// rpm / redline.
    pub rpm: f32,
}

/// Pedals and revs over one lesson attempt, plus when it stalled.
#[derive(Default)]
pub struct Trace {
    pub samples: Vec<Sample>,
    pub stalls: Vec<f32>,
    t: f32,
}

impl Trace {
    fn record(&mut self, c: &Controls, rpm_frac: f32, stalled: bool, dt: f32) {
        self.t += dt;
        self.samples.push(Sample {
            t: self.t,
            clutch: c.clutch,
            throttle: c.throttle,
            brake: c.brake,
            rpm: rpm_frac,
        });
        if stalled {
            self.stalls.push(self.t);
        }
    }
}

#[derive(Resource)]
pub struct Drive {
    pub cars: Vec<CarSpec>,
    /// Index into `cars` of the car being driven.
    pub car: usize,
    pub sim: Sim,
    /// `None` = free drive.
    pub lesson: Option<LessonRun>,
    pub outcome: Outcome,
    pub hint: Option<&'static str>,
    /// Cup used in free drive (lessons keep their own).
    pub free_cup: Cup,
    pub last_events: Vec<Event>,
    /// Free-drive transmission mode. Each lesson sets its own.
    pub mode: TransmissionMode,
    pub trace: Trace,
}

impl Drive {
    pub fn new(cars: Vec<CarSpec>, car: usize, seed: u64) -> Self {
        Self {
            sim: Sim::new(cars[car].clone(), seed),
            cars,
            car,
            lesson: None,
            outcome: Outcome::Running,
            hint: None,
            free_cup: Cup::default(),
            last_events: Vec::new(),
            mode: TransmissionMode::Manual,
            trace: Trace::default(),
        }
    }

    pub fn cup(&self) -> &Cup {
        self.lesson.as_ref().map_or(&self.free_cup, |l| &l.cup)
    }

    /// Fresh car and attempt. `seed` varies the bite point.
    pub fn start(&mut self, lesson: Option<LessonId>, seed: u64, pedals: &mut Pedals) {
        let mode = self.mode;
        *self = Self::new(std::mem::take(&mut self.cars), self.car, seed);
        self.mode = mode;
        if lesson.is_none() {
            self.sim.set_mode(mode);
        }
        if let Some(id) = lesson {
            let mut run = LessonRun::new(id);
            pedals.handbrake_on = run.setup(&mut self.sim);
            self.lesson = Some(run);
        }
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Drive::new(CarSpec::bundled(), 0, 0))
        .add_systems(OnEnter(AppState::Driving), start_free_drive)
        .add_systems(Update, step.run_if(in_state(AppState::Driving)));
}

fn start_free_drive(mut drive: ResMut<Drive>, mut pedals: ResMut<Pedals>, time: Res<Time>) {
    drive.start(None, time.elapsed().as_nanos() as u64, &mut pedals);
}

pub fn step(mut drive: ResMut<Drive>, pedals: Res<Pedals>, time: Res<Time>) {
    let dt = time.delta_secs();
    let Drive {
        sim,
        lesson,
        outcome,
        hint,
        free_cup,
        last_events,
        trace,
        ..
    } = &mut *drive;
    let env = lesson.as_ref().map(|l| l.env()).unwrap_or_default();
    *last_events = sim.step(&pedals.controls, env, dt);
    let s = *sim.state();
    match lesson {
        Some(run) if *outcome == Outcome::Running => {
            *outcome = run.update(&s, &pedals.controls, last_events, dt);
            let stalled = last_events.contains(&Event::Stalled);
            trace.record(
                &pedals.controls,
                s.engine_rpm / sim.car.redline_rpm,
                stalled,
                dt,
            );
            *hint = run.hint(&s, &pedals.controls);
        }
        Some(_) => *hint = None,
        None => {
            free_cup.update(s.jerk, dt);
            *hint =
                (!s.engine_running).then_some("Engine off — clutch in, then I (pad Y) to start.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    #[test]
    fn step_advances_sim_headless() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Pedals>()
            .insert_resource(Drive::new(CarSpec::bundled(), 0, 1))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                16,
            )))
            .add_systems(Update, step);
        app.world_mut().resource_mut::<Pedals>().controls.throttle = 1.0;
        for _ in 0..30 {
            app.update();
        }
        let rpm = app.world().resource::<Drive>().sim.state().engine_rpm;
        assert!(rpm > 1500.0, "free-revving in neutral, rpm {rpm}");
    }
}

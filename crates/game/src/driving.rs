//! Owns the simulation and the active lesson; steps them once per frame.

use bevy::prelude::*;
use drivetrain::{CarSpec, Cup, Event, LessonId, LessonRun, Outcome, Sim};

use crate::AppState;
use crate::input::Pedals;

#[derive(Resource)]
pub struct Drive {
    pub sim: Sim,
    /// `None` = free drive.
    pub lesson: Option<LessonRun>,
    pub outcome: Outcome,
    pub hint: Option<&'static str>,
    /// Cup used in free drive (lessons keep their own).
    pub free_cup: Cup,
    pub last_events: Vec<Event>,
}

impl Drive {
    pub fn new(seed: u64) -> Self {
        Self {
            sim: Sim::new(CarSpec::miata(), seed),
            lesson: None,
            outcome: Outcome::Running,
            hint: None,
            free_cup: Cup::default(),
            last_events: Vec::new(),
        }
    }

    pub fn cup(&self) -> &Cup {
        self.lesson.as_ref().map_or(&self.free_cup, |l| &l.cup)
    }

    /// Fresh car and attempt. `seed` varies the bite point.
    pub fn start(&mut self, lesson: Option<LessonId>, seed: u64, pedals: &mut Pedals) {
        *self = Self::new(seed);
        if let Some(id) = lesson {
            let run = LessonRun::new(id);
            pedals.handbrake_on = run.setup(&mut self.sim);
            self.lesson = Some(run);
        }
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Drive::new(0))
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
    } = &mut *drive;
    let env = lesson.as_ref().map(|l| l.env()).unwrap_or_default();
    *last_events = sim.step(&pedals.controls, env, dt);
    let s = *sim.state();
    match lesson {
        Some(run) if *outcome == Outcome::Running => {
            *outcome = run.update(&s, last_events, dt);
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
            .insert_resource(Drive::new(1))
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

use drivetrain::*;

const DT: f32 = 1.0 / 60.0;

/// Play a lesson with scripted inputs until it ends or `max_s` passes.
fn play(id: LessonId, max_s: f32, mut f: impl FnMut(f32, &Sim) -> Controls) -> Outcome {
    let mut sim = Sim::new(CarSpec::miata(), 3);
    let mut run = LessonRun::new(id);
    run.setup(&mut sim);
    let mut t = 0.0;
    while t < max_s {
        let c = f(t, &sim);
        let ev = sim.step(&c, run.env(), DT);
        match run.update(sim.state(), &c, &ev, DT) {
            Outcome::Running => {}
            done => return done,
        }
        t += DT;
    }
    Outcome::Running
}

#[test]
fn bite_point_held_passes() {
    let out = play(LessonId::BitePoint, 6.0, |_, sim| Controls {
        clutch: sim.bite_point() - 0.3 * sim.car.bite_width,
        handbrake: 1.0,
        ..Default::default()
    });
    assert!(matches!(out, Outcome::Passed { .. }), "{out:?}");
}

#[test]
fn bite_point_released_fully_fails() {
    let out = play(LessonId::BitePoint, 6.0, |_, _| Controls {
        handbrake: 1.0,
        ..Default::default()
    });
    assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
}

#[test]
fn pull_away_clean_passes_with_three_stars() {
    let out = play(LessonId::PullAway, 8.0, |t, sim| {
        let top = sim.bite_point() + 0.02;
        Controls {
            clutch: if t < 0.5 {
                1.0
            } else {
                (top - (t - 0.5) / 1.5 * 0.22).max(0.0)
            },
            throttle: if t < 0.5 { 0.0 } else { 0.25 },
            shift: (t < DT).then_some(1),
            ..Default::default()
        }
    });
    assert_eq!(out, Outcome::Passed { stars: 3 });
}

#[test]
fn pull_away_clutch_dump_fails() {
    let out = play(LessonId::PullAway, 4.0, |t, _| Controls {
        clutch: if t < 0.5 { 1.0 } else { 0.0 },
        shift: (t < DT).then_some(1),
        ..Default::default()
    });
    assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
}

#[test]
fn stop_with_clutch_in_passes() {
    let out = play(LessonId::Stop, 10.0, |t, _| Controls {
        brake: 0.3,
        clutch: if t > 1.0 { 1.0 } else { 0.0 },
        ..Default::default()
    });
    assert!(matches!(out, Outcome::Passed { .. }), "{out:?}");
}

#[test]
fn stop_without_clutch_fails() {
    let out = play(LessonId::Stop, 10.0, |_, _| Controls {
        brake: 0.3,
        ..Default::default()
    });
    assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
}

#[test]
fn cup_survives_gentle_launch_but_not_a_slam() {
    let mut gentle = Cup::default();
    let mut slam = Cup::default();
    for _ in 0..60 {
        gentle.update(2.0, DT);
        slam.update(40.0, DT);
    }
    assert_eq!(gentle.level, 1.0);
    assert!(slam.level < 0.5, "{}", slam.level);
}

/// A scripted driver: pulls away in 1st, then shifts up at 2800 rpm until
/// `top`, holding `cruise_throttle`. Good enough to pass the driving lessons.
struct Driver {
    top: i8,
    cruise_throttle: f32,
    /// Time the current shift (or launch) began.
    since: f32,
    shifting: bool,
}

impl Driver {
    fn new(top: i8, cruise_throttle: f32) -> Self {
        Self {
            top,
            cruise_throttle,
            since: 0.3,
            shifting: false,
        }
    }

    fn controls(&mut self, t: f32, sim: &Sim) -> Controls {
        let s = sim.state();
        if t < 0.3 {
            return Controls {
                clutch: 1.0,
                shift: (t < DT).then_some(1),
                ..Default::default()
            };
        }
        let release = |dt: f32| (sim.bite_point() + 0.02 - dt / 1.2 * 0.22).max(0.0);
        if !self.shifting && s.clutch_locked && s.gear < self.top && s.engine_rpm > 2800.0 {
            self.shifting = true;
            self.since = t;
        }
        if self.shifting {
            let dt = t - self.since;
            if dt < 0.25 {
                return Controls {
                    clutch: 1.0,
                    ..Default::default()
                };
            }
            if dt < 0.25 + DT {
                return Controls {
                    clutch: 1.0,
                    shift: Some(s.gear + 1),
                    ..Default::default()
                };
            }
            self.shifting = false;
            self.since = t;
        }
        Controls {
            clutch: release(t - self.since),
            throttle: self.cruise_throttle,
            ..Default::default()
        }
    }
}

#[test]
fn upshift_to_third_passes() {
    let mut d = Driver::new(3, 0.4);
    let out = play(LessonId::Upshift, 20.0, |t, sim| d.controls(t, sim));
    assert!(matches!(out, Outcome::Passed { .. }), "{out:?}");
}

#[test]
fn downshift_to_second_passes() {
    let mut braking_done_at = None;
    let out = play(LessonId::Downshift, 15.0, |t, sim| {
        let kmh = sim.state().speed_mps * 3.6;
        let done = *braking_done_at.get_or_insert_with(|| if kmh < 27.0 { Some(t) } else { None });
        match done {
            None => {
                braking_done_at = None;
                Controls {
                    brake: 0.3,
                    ..Default::default()
                }
            }
            Some(t0) if t - t0 < 0.3 => Controls {
                clutch: 1.0,
                shift: Some(2),
                ..Default::default()
            },
            Some(t0) => Controls {
                clutch: (0.7 - (t - t0 - 0.3) / 0.8 * 0.7).max(0.0),
                ..Default::default()
            },
        }
    });
    assert!(matches!(out, Outcome::Passed { .. }), "{out:?}");
}

#[test]
fn free_loop_of_500m_passes() {
    let mut d = Driver::new(3, 0.45);
    let out = play(LessonId::FreeLoop, 90.0, |t, sim| d.controls(t, sim));
    assert!(matches!(out, Outcome::Passed { .. }), "{out:?}");
}

fn hill_start(foot_brake: bool) -> impl FnMut(f32, &Sim) -> Controls {
    move |t, sim| {
        let bite = sim.bite_point();
        if t < 0.3 {
            return Controls {
                clutch: 1.0,
                handbrake: 1.0,
                brake: 1.0,
                shift: (t < DT).then_some(1),
                ..Default::default()
            };
        }
        if foot_brake {
            // Handbrake off while on the foot brake, then a quick swap to gas.
            if t < 0.6 {
                return Controls {
                    clutch: 1.0,
                    brake: 1.0,
                    ..Default::default()
                };
            }
            let dt = t - 0.6;
            return Controls {
                clutch: (bite + 0.02 - dt / 1.5 * 0.22).max(0.0),
                throttle: 0.5,
                ..Default::default()
            };
        }
        let dt = t - 0.3;
        Controls {
            clutch: (bite + 0.02 - dt / 2.0 * 0.22).max(0.0),
            throttle: 0.5,
            handbrake: if dt < 0.9 { 1.0 } else { 0.0 },
            ..Default::default()
        }
    }
}

#[test]
fn hill_start_with_handbrake_passes() {
    let out = play(LessonId::HillHandbrake, 15.0, hill_start(false));
    assert!(matches!(out, Outcome::Passed { .. }), "{out:?}");
}

#[test]
fn hill_start_on_foot_brake_passes() {
    let out = play(LessonId::HillNoHandbrake, 15.0, hill_start(true));
    assert!(matches!(out, Outcome::Passed { .. }), "{out:?}");
}

#[test]
fn hill_start_without_gas_rolls_back_and_fails() {
    let out = play(LessonId::HillHandbrake, 10.0, |t, _| Controls {
        clutch: 1.0,
        shift: (t < DT).then_some(1),
        ..Default::default()
    });
    assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
}

#[test]
fn foot_brake_hill_start_using_handbrake_fails() {
    let out = play(LessonId::HillNoHandbrake, 15.0, hill_start(false));
    assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
}

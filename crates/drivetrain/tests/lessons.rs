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
    let mut braked_at: Option<f32> = None;
    let out = play(LessonId::Downshift, 15.0, |t, sim| {
        if braked_at.is_none() && sim.state().speed_mps * 3.6 < 27.0 {
            braked_at = Some(t);
        }
        match braked_at {
            None => Controls {
                brake: 0.3,
                ..Default::default()
            },
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

/// Pull away gently from a standstill in 1st (clutch already in).
fn launch(sim: &Sim, since: f32, throttle: f32) -> Controls {
    Controls {
        clutch: (sim.bite_point() + 0.02 - since / 1.5 * 0.22).max(0.0),
        throttle,
        ..Default::default()
    }
}

/// Brake pedal needed to stop within `d` metres (v² = 2ad).
fn brake_for(sim: &Sim, d: f32) -> f32 {
    let v = sim.state().speed_mps.max(0.0);
    v * v / (2.0 * d.max(0.3)) * sim.car.mass_kg / sim.car.brake_force_n
}

/// Drive on `throttle` (or a launch from standstill) until braking to stop at
/// `target` is needed, then brake with the clutch in below 15 km/h. Calls
/// `after` with the time since the car stopped.
fn stop_at_then(
    target: f32,
    throttle: f32,
    after: impl Fn(f32, &Sim) -> Controls,
) -> impl FnMut(f32, &Sim) -> Controls {
    let mut stopped_at: Option<f32> = None;
    let mut braking = false;
    move |t, sim| {
        let s = sim.state();
        if let Some(t0) = stopped_at {
            return after(t - t0, sim);
        }
        let brake = brake_for(sim, target - s.x);
        // Once braking starts, keep braking: coasting in gear at walking pace stalls.
        braking |= brake >= 0.15 || s.x >= target - 0.5;
        if !braking {
            return Controls {
                throttle,
                ..Default::default()
            };
        }
        if s.speed_mps < 0.05 {
            stopped_at = Some(t);
        }
        Controls {
            clutch: if s.speed_mps * 3.6 < 15.0 { 1.0 } else { 0.0 },
            // Rolling resistance and engine braking do some of the work.
            brake: (brake - 0.03).clamp(0.0, 1.0),
            ..Default::default()
        }
    }
}

#[test]
fn parking_creep_into_the_box_passes() {
    let target = (PARK_BOX.0 + PARK_BOX.1) / 2.0;
    let out = play(LessonId::Parking, 40.0, |t, sim| {
        let s = sim.state();
        if t < 0.3 {
            return Controls {
                clutch: 1.0,
                shift: Some(1),
                ..Default::default()
            };
        }
        let brake = brake_for(sim, target - s.x);
        if brake > 0.1 || s.x > target {
            return Controls {
                clutch: 1.0,
                brake: (brake + 0.1).min(1.0),
                ..Default::default()
            };
        }
        // Creep: slip the clutch at the bite, dip it when going too fast.
        if s.speed_mps > 1.4 {
            return Controls {
                clutch: 1.0,
                ..Default::default()
            };
        }
        let creep = sim.bite_point() - 0.35 * sim.car.bite_width;
        let clutch = creep.max(sim.bite_point() + 0.02 - (t - 0.3) / 1.5 * 0.1);
        Controls {
            clutch,
            throttle: 0.15,
            ..Default::default()
        }
    });
    assert!(matches!(out, Outcome::Passed { .. }), "{out:?}");
}

#[test]
fn parking_too_fast_fails() {
    let mut d = Driver::new(2, 0.6);
    let out = play(LessonId::Parking, 20.0, |t, sim| d.controls(t, sim));
    assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
}

/// Follows the lead car in 1st: stops 3 m behind it, pulls away once it
/// has moved off, and holds the gap with the gas.
fn follower() -> impl FnMut(f32, &Sim, f32) -> Controls {
    let mut launched_at: Option<f32> = None;
    move |t, sim, gap| {
        let s = sim.state();
        if t < 0.3 {
            return Controls {
                clutch: 1.0,
                shift: Some(1),
                ..Default::default()
            };
        }
        let brake = brake_for(sim, gap - 3.0);
        if brake > 0.15 || gap < 4.0 {
            launched_at = None;
            return Controls {
                clutch: 1.0,
                brake: (brake + 0.2).min(1.0),
                ..Default::default()
            };
        }
        if s.speed_mps < 0.3 && launched_at.is_none() {
            if gap < 10.0 {
                return Controls {
                    clutch: 1.0,
                    brake: 0.3,
                    ..Default::default()
                };
            }
            launched_at = Some(t);
        }
        let since = t - launched_at.unwrap_or(t);
        let mut c = launch(sim, since, ((gap - 7.0) * 0.05).clamp(0.25, 0.35));
        if s.clutch_locked {
            c.clutch = 0.0;
        }
        c
    }
}

#[test]
fn stop_and_go_traffic_passes() {
    let mut sim = Sim::new(CarSpec::miata(), 3);
    let mut run = LessonRun::new(LessonId::Traffic);
    run.setup(&mut sim);
    let mut drive = follower();
    let mut out = Outcome::Running;
    let mut t = 0.0;
    while t < 90.0 && out == Outcome::Running {
        let gap = run.lead.unwrap().x - sim.state().x - CAR_LEN;
        let c = drive(t, &sim, gap);
        let ev = sim.step(&c, run.env(), DT);
        out = run.update(sim.state(), &c, &ev, DT);
        t += DT;
    }
    assert!(matches!(out, Outcome::Passed { .. }), "{out:?} at t={t}");
}

#[test]
fn ramming_the_car_in_front_fails() {
    let mut d = Driver::new(2, 0.5);
    let out = play(LessonId::Traffic, 30.0, |t, sim| d.controls(t, sim));
    assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
}

#[test]
fn stop_sign_then_left_turn_passes() {
    let out = play(
        LessonId::StopSign,
        30.0,
        stop_at_then(STOP_LINE - 2.0, 0.0, |dt, sim| {
            if dt < 1.0 {
                return Controls {
                    clutch: 1.0,
                    brake: 0.5,
                    shift: Some(1),
                    ..Default::default()
                };
            }
            Controls {
                steer: 1.0,
                ..launch(sim, dt - 1.0, 0.25)
            }
        }),
    );
    assert!(matches!(out, Outcome::Passed { .. }), "{out:?}");
}

#[test]
fn rolling_the_stop_sign_fails() {
    let out = play(LessonId::StopSign, 15.0, |_, _| Controls {
        throttle: 0.2,
        ..Default::default()
    });
    assert!(matches!(out, Outcome::Failed(_)), "{out:?}");
}

#[test]
fn hill_stop_sign_then_hill_start_passes() {
    let out = play(
        LessonId::HillStopSign,
        30.0,
        stop_at_then(HILL_STOP_LINE - 2.0, 0.3, |dt, sim| {
            if dt < 1.0 {
                return Controls {
                    clutch: 1.0,
                    brake: 0.6,
                    shift: Some(1),
                    ..Default::default()
                };
            }
            launch(sim, dt - 1.0, 0.5)
        }),
    );
    assert!(matches!(out, Outcome::Passed { .. }), "{out:?}");
}

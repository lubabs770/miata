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
        match run.update(sim.state(), &ev, DT) {
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

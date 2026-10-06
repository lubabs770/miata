use drivetrain::*;

const DT: f32 = 1.0 / 60.0;

/// Run `secs` of frames, building each frame's controls from elapsed time.
fn drive(sim: &mut Sim, secs: f32, env: Env, mut f: impl FnMut(f32) -> Controls) -> Vec<Event> {
    let mut all = Vec::new();
    let mut t = 0.0;
    while t < secs {
        all.extend(sim.step(&f(t), env, DT));
        t += DT;
    }
    all
}

fn in_first(seed: u64) -> Sim {
    let mut sim = Sim::new(CarSpec::miata(), seed);
    let ev = sim.step(
        &Controls {
            clutch: 1.0,
            shift: Some(1),
            ..Default::default()
        },
        Env::default(),
        DT,
    );
    assert!(ev.contains(&Event::Shifted(1)));
    sim
}

#[test]
fn idles_in_neutral() {
    let mut sim = Sim::new(CarSpec::miata(), 1);
    drive(&mut sim, 3.0, Env::default(), |_| Controls::default());
    let rpm = sim.state().engine_rpm;
    assert!((800.0..900.0).contains(&rpm), "idle rpm {rpm}");
}

#[test]
fn very_slow_release_at_idle_pulls_away() {
    for seed in 0..5 {
        let mut sim = in_first(seed);
        let top = sim.bite_point() + 0.02;
        // Quickly up to the bite point, then ~6 s through the engagement band, no gas.
        let ev = drive(&mut sim, 9.0, Env::default(), |t| Controls {
            clutch: if t < 0.5 {
                1.0 - (1.0 - top) * t / 0.5
            } else {
                (top - (t - 0.5) / 8.0 * 0.25).max(0.0)
            },
            ..Default::default()
        });
        assert!(!ev.contains(&Event::Stalled), "seed {seed} stalled");
        assert!(
            sim.state().speed_mps > 1.5,
            "seed {seed} speed {}",
            sim.state().speed_mps
        );
        assert!(sim.state().clutch_locked);
    }
}

#[test]
fn normal_release_with_some_gas_pulls_away() {
    for seed in 0..5 {
        let mut sim = in_first(seed);
        let top = sim.bite_point() + 0.02;
        // 1.5 s through the band holding ~2000 rpm worth of throttle.
        let ev = drive(&mut sim, 4.0, Env::default(), |t| Controls {
            clutch: (top - t / 1.5 * 0.22).max(0.0),
            throttle: 0.25,
            ..Default::default()
        });
        assert!(!ev.contains(&Event::Stalled), "seed {seed} stalled");
        assert!(
            sim.state().speed_mps > 3.0,
            "seed {seed} speed {}",
            sim.state().speed_mps
        );
    }
}

#[test]
fn clutch_dump_at_idle_stalls() {
    let mut sim = in_first(0);
    let ev = drive(&mut sim, 1.0, Env::default(), |_| Controls::default());
    assert!(ev.contains(&Event::Stalled));
    assert!(!sim.state().engine_running);
}

#[test]
fn handbrake_release_on_hill_rolls_back() {
    let hill = Env { grade: 0.10 };
    let mut sim = Sim::new(CarSpec::miata(), 0);
    drive(&mut sim, 1.0, hill, |_| Controls {
        handbrake: 1.0,
        ..Default::default()
    });
    assert_eq!(sim.state().rollback_m, 0.0, "handbrake holds");
    drive(&mut sim, 2.0, hill, |_| Controls::default());
    assert!(
        sim.state().rollback_m > 1.0,
        "rolled {}",
        sim.state().rollback_m
    );
}

#[test]
fn shift_without_clutch_grinds() {
    let mut sim = Sim::new(CarSpec::miata(), 0);
    let ev = sim.step(
        &Controls {
            shift: Some(1),
            ..Default::default()
        },
        Env::default(),
        DT,
    );
    assert_eq!(ev, vec![Event::Grind]);
    assert_eq!(sim.state().gear, 0);
}

#[test]
fn money_shift_flags_over_rev() {
    let mut sim = Sim::new(CarSpec::miata(), 0);
    sim.set_moving(5, 100.0 / 3.6);
    let ev = sim.step(
        &Controls {
            clutch: 1.0,
            shift: Some(2),
            ..Default::default()
        },
        Env::default(),
        DT,
    );
    assert!(ev.contains(&Event::OverRev));
    assert_eq!(sim.state().gear, 2);
}

#[test]
fn braking_to_stop_in_gear_stalls_but_clutch_in_does_not() {
    let mut sim = Sim::new(CarSpec::miata(), 0);
    sim.set_moving(2, 30.0 / 3.6);
    let ev = drive(&mut sim, 5.0, Env::default(), |_| Controls {
        brake: 0.6,
        ..Default::default()
    });
    assert!(ev.contains(&Event::Stalled));

    let mut sim = Sim::new(CarSpec::miata(), 0);
    sim.set_moving(2, 30.0 / 3.6);
    let ev = drive(&mut sim, 5.0, Env::default(), |_| Controls {
        brake: 0.6,
        clutch: 1.0,
        ..Default::default()
    });
    assert!(!ev.contains(&Event::Stalled));
    assert_eq!(sim.state().speed_mps, 0.0);
}

#[test]
fn full_throttle_pulls_to_limiter_in_first() {
    let mut sim = in_first(0);
    drive(&mut sim, 6.0, Env::default(), |t| Controls {
        throttle: 1.0,
        clutch: (1.0 - t).max(0.0),
        ..Default::default()
    });
    let s = sim.state();
    assert!(s.engine_running);
    assert!(
        s.engine_rpm > 6500.0 && s.engine_rpm < 7300.0,
        "rpm {}",
        s.engine_rpm
    );
}

#[test]
fn restart_needs_clutch_in_when_in_gear() {
    let mut sim = in_first(0);
    drive(&mut sim, 1.0, Env::default(), |_| Controls::default());
    let ev = sim.step(
        &Controls {
            ignition: true,
            ..Default::default()
        },
        Env::default(),
        DT,
    );
    assert!(!ev.contains(&Event::Started));
    let ev = sim.step(
        &Controls {
            ignition: true,
            clutch: 1.0,
            ..Default::default()
        },
        Env::default(),
        DT,
    );
    assert!(ev.contains(&Event::Started));
}

#[test]
fn zero_dt_is_a_no_op() {
    let mut sim = Sim::new(CarSpec::miata(), 0);
    assert!(
        sim.step(&Controls::default(), Env::default(), 0.0)
            .is_empty()
    );
    assert!(sim.state().accel.is_finite());
}

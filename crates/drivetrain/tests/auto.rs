//! Auto-clutch and paddle modes: the computer works the clutch.
use drivetrain::*;

const DT: f32 = 1.0 / 60.0;

fn auto_in_first(car: CarSpec) -> Sim {
    let mut sim = Sim::new(car, 0);
    sim.set_mode(TransmissionMode::AutoClutch);
    let ev = sim.step(
        &Controls {
            shift: Some(1),
            ..Default::default()
        },
        Env::default(),
        DT,
    );
    assert!(
        ev.contains(&Event::Shifted(1)),
        "auto mode shifts without the clutch pedal"
    );
    sim
}

fn run(sim: &mut Sim, secs: f32, c: Controls) -> Vec<Event> {
    let mut all = Vec::new();
    for _ in 0..(secs / DT) as usize {
        all.extend(sim.step(&c, Env::default(), DT));
    }
    all
}

#[test]
fn full_throttle_from_stop_never_stalls() {
    for car in CarSpec::bundled() {
        let name = car.name.clone();
        let mut sim = auto_in_first(car);
        let ev = run(
            &mut sim,
            4.0,
            Controls {
                throttle: 1.0,
                ..Default::default()
            },
        );
        assert!(!ev.contains(&Event::Stalled), "{name} stalled");
        assert!(
            sim.state().speed_mps > 5.0,
            "{name} speed {}",
            sim.state().speed_mps
        );
    }
}

#[test]
fn creeps_and_stops_in_gear_without_stalling() {
    for car in CarSpec::bundled() {
        let name = car.name.clone();
        let mut sim = auto_in_first(car);
        let ev = run(&mut sim, 3.0, Controls::default());
        assert!(!ev.contains(&Event::Stalled), "{name} stalled creeping");
        assert!(sim.state().speed_mps > 0.3, "{name} didn't creep");
        let ev = run(
            &mut sim,
            4.0,
            Controls {
                brake: 0.5,
                ..Default::default()
            },
        );
        assert!(!ev.contains(&Event::Stalled), "{name} stalled stopping");
        assert_eq!(sim.state().speed_mps, 0.0);
        assert!(sim.state().engine_running);
    }
}

#[test]
fn over_revving_downshift_is_refused() {
    for mode in [TransmissionMode::AutoClutch, TransmissionMode::Paddles] {
        let mut sim = Sim::new(CarSpec::miata(), 0);
        sim.set_mode(mode);
        sim.set_moving(5, 100.0 / 3.6);
        let ev = sim.step(
            &Controls {
                shift: Some(2),
                ..Default::default()
            },
            Env::default(),
            DT,
        );
        assert_eq!(ev, vec![Event::ShiftRefused], "{mode:?}");
        assert_eq!(sim.state().gear, 5);
    }
}

#[test]
fn sequential_shifts_step_through_gears() {
    let mut sim = Sim::new(CarSpec::miata(), 0);
    sim.set_mode(TransmissionMode::Paddles);
    let up = Controls {
        sequential: 1,
        ..Default::default()
    };
    let down = Controls {
        sequential: -1,
        ..Default::default()
    };
    sim.step(&up, Env::default(), DT);
    assert_eq!(sim.state().gear, 1, "up from neutral selects 1st");
    sim.set_moving(1, 5.0);
    sim.step(&up, Env::default(), DT);
    assert_eq!(sim.state().gear, 2);
    sim.step(&down, Env::default(), DT);
    assert_eq!(sim.state().gear, 1);
    sim.step(&down, Env::default(), DT);
    assert_eq!(sim.state().gear, 1, "down from 1st stays in 1st");
}

#[test]
fn manual_mode_ignores_sequential_input() {
    let mut sim = Sim::new(CarSpec::miata(), 0);
    sim.step(
        &Controls {
            sequential: 1,
            clutch: 1.0,
            ..Default::default()
        },
        Env::default(),
        DT,
    );
    assert_eq!(sim.state().gear, 0);
}

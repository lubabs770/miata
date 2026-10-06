//! Every bundled car must behave like a manual car: a gentle launch with some
//! gas pulls away, a dumped clutch at idle stalls, and it idles in neutral.
use drivetrain::*;

const DT: f32 = 1.0 / 60.0;

fn cars() -> Vec<CarSpec> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../cars");
    let mut cars: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| CarSpec::from_toml(&std::fs::read_to_string(e.unwrap().path()).unwrap()).unwrap())
        .collect();
    cars.sort_by(|a, b| a.name.cmp(&b.name));
    assert!(cars.len() >= 3);
    cars
}

fn in_first(car: CarSpec, seed: u64) -> Sim {
    let mut sim = Sim::new(car, seed);
    sim.step(
        &Controls {
            clutch: 1.0,
            shift: Some(1),
            ..Default::default()
        },
        Env::default(),
        DT,
    );
    sim
}

#[test]
fn every_car_idles() {
    for car in cars() {
        let idle = car.idle_rpm;
        let name = car.name.clone();
        let mut sim = Sim::new(car, 0);
        for _ in 0..180 {
            sim.step(&Controls::default(), Env::default(), DT);
        }
        let rpm = sim.state().engine_rpm;
        assert!((rpm - idle).abs() < 80.0, "{name}: idle {rpm}");
    }
}

#[test]
fn every_car_pulls_away_with_some_gas() {
    for car in cars() {
        for seed in 0..5 {
            let name = car.name.clone();
            let mut sim = in_first(car.clone(), seed);
            let top = sim.bite_point() + 0.02;
            let mut stalled = false;
            for i in 0..240 {
                let t = i as f32 * DT;
                let c = Controls {
                    clutch: (top - t / 1.5 * 0.22).max(0.0),
                    throttle: 0.3,
                    ..Default::default()
                };
                stalled |= sim.step(&c, Env::default(), DT).contains(&Event::Stalled);
            }
            assert!(!stalled, "{name} seed {seed} stalled");
            assert!(
                sim.state().speed_mps > 2.0,
                "{name} seed {seed} speed {}",
                sim.state().speed_mps
            );
        }
    }
}

#[test]
fn every_car_stalls_on_a_dumped_clutch() {
    for car in cars() {
        let name = car.name.clone();
        let mut sim = in_first(car, 0);
        let mut stalled = false;
        for _ in 0..60 {
            stalled |= sim
                .step(&Controls::default(), Env::default(), DT)
                .contains(&Event::Stalled);
        }
        assert!(stalled, "{name} survived a clutch dump");
    }
}

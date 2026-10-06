use drivetrain::CarSpec;

#[test]
fn miata_loads_with_published_ratios() {
    let c = CarSpec::miata();
    assert_eq!(c.top_gear(), 5);
    assert!((c.ratio(1).unwrap() - 3.136 * 4.30).abs() < 1e-4);
    assert!(c.ratio(-1).unwrap() < 0.0, "reverse spins the wheels backwards");
    assert_eq!(c.ratio(0), None);
    assert_eq!(c.ratio(6), None);
}

#[test]
fn torque_curve_interpolates_and_clamps() {
    let c = CarSpec::miata();
    assert_eq!(c.torque_at(0.0), 70.0);
    assert_eq!(c.torque_at(9000.0), 115.0);
    assert!((c.torque_at(4750.0) - 132.0).abs() < 1e-3);
}

#[test]
fn every_bundled_car_file_parses() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../cars");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        CarSpec::from_toml(&text).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    }
}

//! Low-poly world and the driver's-seat view, built from Bevy primitives.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;
use drivetrain::{CAR_LEN, CORNER_X, LessonId, PARK_BOX, STOP_LINE, SimState};
use leafwing_input_manager::prelude::*;

use crate::driving::Drive;
use crate::input::{Action, Pedals};

const ROAD_LEN: f32 = 4000.0;
const TACH_MAX_RPM: f32 = 8000.0;
const SPEEDO_MAX_KMH: f32 = 220.0;
/// Needle sweep: 135° either side of straight up.
const SWEEP: f32 = 0.75 * PI;
const STEER_WHEEL_TURNS: f32 = 1.5 * PI;
/// Hill lessons: the slope starts this far behind the car's start point, so
/// rolling back still has road under it.
const HILL_RUN_IN: f32 = 30.0;
const HILL_LEN: f32 = 600.0;
/// Free-drive town: block size and how many avenues/streets.
const BLOCK: f32 = 100.0;
const TOWN_AVENUES: i32 = 6;
const TOWN_STREETS: i32 = 20;

#[derive(Component)]
struct CarRig;
#[derive(Component)]
struct DriverCamera;
/// Sloped road shown only during hill lessons.
#[derive(Component)]
struct Hill;
/// Outside body, hidden in the cockpit view so it doesn't block the camera.
#[derive(Component)]
struct Body;

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    #[default]
    Cockpit,
    Hood,
    Chase,
    BirdsEye,
}

impl View {
    fn next(self) -> Self {
        match self {
            View::Cockpit => View::Hood,
            View::Hood => View::Chase,
            View::Chase => View::BirdsEye,
            View::BirdsEye => View::Cockpit,
        }
    }

    /// Camera pose relative to the car (car faces -Z).
    fn camera(self) -> Transform {
        match self {
            View::Cockpit => {
                Transform::from_xyz(-0.35, 1.05, 0.0).with_rotation(Quat::from_rotation_x(-0.12))
            }
            View::Hood => {
                Transform::from_xyz(0.0, 1.0, -1.3).with_rotation(Quat::from_rotation_x(-0.05))
            }
            View::Chase => {
                Transform::from_xyz(0.0, 2.4, 6.5).looking_at(Vec3::new(0.0, 0.8, -4.0), Vec3::Y)
            }
            View::BirdsEye => {
                Transform::from_xyz(0.0, 28.0, 6.0).looking_at(Vec3::new(0.0, 0.0, -4.0), Vec3::Y)
            }
        }
    }
}
#[derive(Component)]
struct TachNeedle;
#[derive(Component)]
struct SpeedNeedle;
#[derive(Component)]
struct SteeringWheel(Quat);
#[derive(Component)]
struct ShifterKnob(Vec3);
#[derive(Component)]
struct Coffee;

pub fn plugin(app: &mut App) {
    app.insert_resource(ClearColor(Color::srgb(0.62, 0.78, 0.93)))
        .init_resource::<View>()
        .add_systems(Startup, (spawn_world, spawn_car, spawn_props))
        .add_systems(
            Update,
            (
                follow_sim,
                animate_cockpit,
                switch_view,
                show_hill,
                place_props,
            ),
        );
}

fn mat(materials: &mut Assets<StandardMaterial>, c: Color) -> MeshMaterial3d<StandardMaterial> {
    MeshMaterial3d(materials.add(StandardMaterial {
        base_color: c,
        perceptual_roughness: 0.9,
        ..default()
    }))
}

fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::OVERCAST_DAY,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(30.0, 60.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(ROAD_LEN, ROAD_LEN))),
        mat(&mut materials, Color::srgb(0.42, 0.55, 0.33)),
        Transform::from_xyz(0.0, -0.01, -ROAD_LEN / 2.0 + 50.0),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(8.0, 0.02, ROAD_LEN))),
        mat(&mut materials, Color::srgb(0.25, 0.25, 0.27)),
        Transform::from_xyz(0.0, 0.0, -ROAD_LEN / 2.0 + 50.0),
    ));
    commands.spawn((
        Hill,
        Visibility::Hidden,
        Mesh3d(meshes.add(Cuboid::new(8.0, 0.1, HILL_LEN))),
        mat(&mut materials, Color::srgb(0.3, 0.3, 0.32)),
        Transform::default(),
    ));
    // Centre dashes and roadside posts give a sense of speed.
    let dash = meshes.add(Cuboid::new(0.15, 0.03, 3.0));
    let dash_mat = mat(&mut materials, Color::srgb(0.95, 0.9, 0.6));
    let post = meshes.add(Cuboid::new(0.15, 1.0, 0.15));
    let post_mat = mat(&mut materials, Color::WHITE);
    for i in 0..(ROAD_LEN as i32 / 10) {
        let z = 50.0 - i as f32 * 10.0;
        commands.spawn((
            Mesh3d(dash.clone()),
            dash_mat.clone(),
            Transform::from_xyz(0.0, 0.0, z),
        ));
        if i % 2 == 0 {
            for x in [-5.0, 5.0] {
                commands.spawn((
                    Mesh3d(post.clone()),
                    post_mat.clone(),
                    Transform::from_xyz(x, 0.5, z),
                ));
            }
        }
    }
    // Town grid: avenues every BLOCK m across, cross streets every BLOCK m
    // ahead starting at the stop-sign lesson's line, buildings in the blocks.
    let asphalt = mat(&mut materials, Color::srgb(0.25, 0.25, 0.27));
    let half_w = TOWN_AVENUES as f32 * BLOCK / 2.0;
    for a in -(TOWN_AVENUES / 2)..=TOWN_AVENUES / 2 {
        if a != 0 {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(8.0, 0.02, ROAD_LEN))),
                asphalt.clone(),
                Transform::from_xyz(a as f32 * BLOCK, 0.0, -ROAD_LEN / 2.0 + 50.0),
            ));
        }
    }
    let street = meshes.add(Cuboid::new(2.0 * half_w + 8.0, 0.021, 8.0));
    for k in 0..TOWN_STREETS {
        commands.spawn((
            Mesh3d(street.clone()),
            asphalt.clone(),
            Transform::from_xyz(0.0, 0.0, -cross_street_z(k)),
        ));
    }
    let block = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let tints: Vec<_> = (0..5)
        .map(|t| {
            let v = 0.55 + t as f32 * 0.08;
            mat(&mut materials, Color::srgb(v, v * 0.9, v * 0.8))
        })
        .collect();
    for a in -(TOWN_AVENUES / 2)..TOWN_AVENUES / 2 {
        for k in 0..TOWN_STREETS - 1 {
            // Four buildings per block, sized by a cheap deterministic hash.
            let (x0, z0) = (a as f32 * BLOCK + 10.0, cross_street_z(k) + 10.0);
            for n in 0..4u32 {
                let hsh = (a as u32).wrapping_mul(73_856_093)
                    ^ (k as u32).wrapping_mul(19_349_663)
                    ^ n.wrapping_mul(83_492_791);
                let (w, d) = (14.0 + (hsh % 9) as f32, 14.0 + (hsh / 9 % 9) as f32);
                let h = 5.0 + (hsh / 81 % 20) as f32;
                let (cx, cz) = (
                    x0 + w / 2.0 + (n % 2) as f32 * 42.0,
                    z0 + d / 2.0 + (n / 2) as f32 * 42.0,
                );
                commands.spawn((
                    Mesh3d(block.clone()),
                    tints[(hsh % 5) as usize].clone(),
                    Transform::from_xyz(cx, h / 2.0, -cz).with_scale(Vec3::new(w, h, d)),
                ));
            }
        }
    }
}

/// Distance ahead of the start to cross street `k` (the first one is at
/// the stop-sign lesson's line, so its left turn goes onto a street).
fn cross_street_z(k: i32) -> f32 {
    STOP_LINE + 4.0 + k as f32 * BLOCK
}

fn spawn_car(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let red = mat(&mut materials, Color::srgb(0.75, 0.05, 0.05));
    let dark = mat(&mut materials, Color::srgb(0.08, 0.08, 0.09));
    let face = mat(&mut materials, Color::srgb(0.02, 0.02, 0.02));
    let needle = mat(&mut materials, Color::srgb(1.0, 0.45, 0.1));
    let white = mat(&mut materials, Color::srgb(0.95, 0.95, 0.92));
    let coffee = mat(&mut materials, Color::srgb(0.3, 0.17, 0.07));

    let gauge = meshes.add(Cylinder::new(0.075, 0.01));
    let needle_mesh = meshes.add(Cuboid::new(0.006, 0.065, 0.004));
    // Gauge faces point at the driver: cylinder axis (Y) turned to +Z.
    let face_rot = Quat::from_rotation_x(FRAC_PI_2);
    let wheel_rot = Quat::from_rotation_x(FRAC_PI_2 - 0.35);

    commands
        .spawn((CarRig, Transform::default(), Visibility::default()))
        .with_children(|car| {
            // US car: driver sits left of centre.
            car.spawn((Camera3d::default(), DriverCamera, View::Cockpit.camera()));
            car.spawn((
                Body,
                Visibility::Hidden,
                Mesh3d(meshes.add(Cuboid::new(1.68, 0.55, 3.95))),
                red.clone(),
                Transform::from_xyz(0.0, 0.5, -0.9),
            ));
            car.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.65, 0.12, 1.8))),
                red.clone(),
                Transform::from_xyz(0.0, 0.62, -1.7),
            ));
            car.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.5, 0.25, 0.45))),
                dark.clone(),
                Transform::from_xyz(0.0, 0.75, -0.65),
            ));
            for x in [-0.78, 0.78] {
                car.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.05, 0.6, 0.05))),
                    dark.clone(),
                    Transform::from_xyz(x, 1.05, -0.75).with_rotation(Quat::from_rotation_x(-0.5)),
                ));
            }
            for (x, marker) in [(-0.45, 0), (-0.27, 1)] {
                car.spawn((
                    Mesh3d(gauge.clone()),
                    face.clone(),
                    Transform::from_xyz(x, 0.9, -0.52).with_rotation(face_rot),
                ))
                .with_children(|g| {
                    // Pivot at the gauge centre; the needle sits above it.
                    let mut pivot =
                        g.spawn((Transform::from_xyz(0.0, 0.008, 0.0), Visibility::default()));
                    if marker == 0 {
                        pivot.insert(TachNeedle);
                    } else {
                        pivot.insert(SpeedNeedle);
                    }
                    pivot.with_children(|p| {
                        p.spawn((
                            Mesh3d(needle_mesh.clone()),
                            needle.clone(),
                            Transform::from_xyz(0.0, 0.0, -0.03)
                                .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
                        ));
                    });
                });
            }
            car.spawn((
                SteeringWheel(wheel_rot),
                Mesh3d(meshes.add(Torus::new(0.16, 0.185))),
                dark.clone(),
                Transform::from_xyz(-0.35, 0.82, -0.38).with_rotation(wheel_rot),
            ));
            let knob_home = Vec3::new(0.0, 0.62, -0.3);
            car.spawn((
                ShifterKnob(knob_home),
                Mesh3d(meshes.add(Sphere::new(0.035))),
                white.clone(),
                Transform::from_translation(knob_home),
            ));
            car.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.04, 0.1))),
                white.clone(),
                Transform::from_xyz(0.15, 0.93, -0.55),
            ))
            .with_children(|cup| {
                cup.spawn((
                    Coffee,
                    Mesh3d(meshes.add(Cylinder::new(0.037, 0.01))),
                    coffee.clone(),
                    Transform::from_xyz(0.0, 0.045, 0.0),
                ));
            });
        });
}

/// Sim x/y are a ground plane with heading 0 = sim +x. In Bevy the car
/// faces -Z, so sim +x → world -Z and sim +y (left) → world -X.
/// Road height at sim distance `x` on a slope of `g`. Uphill starts at
/// ground level; downhill starts at the top so the road never sinks below
/// the flat ground.
fn road_height(x: f32, g: f32) -> f32 {
    let along = if g >= 0.0 {
        HILL_RUN_IN + x
    } else {
        HILL_LEN - HILL_RUN_IN - x
    };
    (along * g.abs()).max(0.0)
}

fn grade(drive: &Drive) -> f32 {
    drive.lesson.as_ref().map_or(0.0, |l| l.env().grade)
}

/// On a hill the road height grows with distance along sim x (lessons drive
/// straight, heading 0), and the car pitches nose-up.
fn follow_sim(drive: Res<Drive>, mut rig: Single<&mut Transform, With<CarRig>>) {
    let s = drive.sim.state();
    let g = grade(&drive);
    rig.translation = Vec3::new(-s.y, road_height(s.x, g), -s.x);
    rig.rotation = Quat::from_rotation_y(s.heading) * Quat::from_rotation_x(g.atan());
}

fn show_hill(drive: Res<Drive>, mut hill: Single<(&mut Transform, &mut Visibility), With<Hill>>) {
    let g = grade(&drive);
    let (t, vis) = &mut *hill;
    **vis = if g != 0.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    // Centre of the slope, HILL_LEN/2 ahead of where it starts.
    let mid = HILL_LEN / 2.0 - HILL_RUN_IN;
    t.translation = Vec3::new(0.0, road_height(mid, g) - 0.05, -mid);
    t.rotation = Quat::from_rotation_x(g.atan());
    t.scale = Vec3::new(1.0, 1.0, (1.0 + g * g).sqrt());
}

fn needle_angle(value: f32, max: f32) -> Quat {
    // Gauge faces were rotated so local Y is "toward the driver"; spin about it.
    Quat::from_rotation_y(SWEEP - (value / max).clamp(0.0, 1.0) * 2.0 * SWEEP)
}

fn knob_offset(gear: i8) -> Vec3 {
    let (col, row) = match gear {
        1 => (-1.0, -1.0),
        2 => (-1.0, 1.0),
        3 => (0.0, -1.0),
        4 => (0.0, 1.0),
        5 => (1.0, -1.0),
        -1 => (1.0, 1.0),
        _ => (0.0, 0.0),
    };
    Vec3::new(col * 0.04, 0.0, row * 0.04)
}

#[allow(clippy::type_complexity)]
fn animate_cockpit(
    drive: Res<Drive>,
    pedals: Res<Pedals>,
    mut q: ParamSet<(
        Single<&mut Transform, With<TachNeedle>>,
        Single<&mut Transform, With<SpeedNeedle>>,
        Single<(&mut Transform, &SteeringWheel)>,
        Single<(&mut Transform, &ShifterKnob)>,
        Single<&mut Transform, With<Coffee>>,
    )>,
) {
    let s: &SimState = drive.sim.state();
    q.p0().rotation = needle_angle(s.engine_rpm, TACH_MAX_RPM);
    q.p1().rotation = needle_angle(s.speed_mps.abs() * 3.6, SPEEDO_MAX_KMH);
    {
        let mut w = q.p2();
        let (t, base) = &mut *w;
        t.rotation =
            base.0 * Quat::from_rotation_y(pedals.controls.steer * STEER_WHEEL_TURNS / 2.0);
    }
    {
        let mut k = q.p3();
        let (t, home) = &mut *k;
        t.translation = home.0 + knob_offset(s.gear);
    }
    let level = drive.cup().level;
    let mut c = q.p4();
    c.translation.y = -0.045 + 0.09 * level;
    // Coffee surface tilts with acceleration.
    c.rotation = Quat::from_rotation_x((s.accel * 0.04).clamp(-0.4, 0.4));
    c.scale = if level <= 0.01 { Vec3::ZERO } else { Vec3::ONE };
}

fn switch_view(
    actions: Single<&ActionState<Action>>,
    mut view: ResMut<View>,
    mut cam: Single<&mut Transform, With<DriverCamera>>,
    mut body: Single<&mut Visibility, With<Body>>,
) {
    if !actions.just_pressed(&Action::CycleView) {
        return;
    }
    *view = view.next();
    **cam = view.camera();
    **body = if *view == View::Cockpit {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_cycle_back_to_cockpit() {
        let mut v = View::Cockpit;
        for _ in 0..4 {
            v = v.next();
        }
        assert_eq!(v, View::Cockpit);
    }
}

/// Lesson props, spawned hidden and shown by `place_props`.
#[derive(Component)]
struct LeadCar;
#[derive(Component)]
struct StopProp;
#[derive(Component)]
struct ParkingLines;
#[derive(Component)]
struct CornerSign;

fn spawn_props(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let white = mat(&mut materials, Color::WHITE);
    commands.spawn((
        LeadCar,
        Visibility::Hidden,
        Mesh3d(meshes.add(Cuboid::new(1.75, 1.3, CAR_LEN))),
        mat(&mut materials, Color::srgb(0.15, 0.3, 0.75)),
        Transform::default(),
    ));
    // Stop line across the road plus an octagonal sign on the right.
    commands
        .spawn((StopProp, Visibility::Hidden, Transform::default()))
        .with_children(|p| {
            p.spawn((
                Mesh3d(meshes.add(Cuboid::new(8.0, 0.03, 0.4))),
                white.clone(),
                Transform::from_xyz(0.0, 0.02, 0.0),
            ));
            p.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.04, 2.2))),
                white.clone(),
                Transform::from_xyz(4.6, 1.1, 0.0),
            ));
            p.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.38, 0.03).mesh().resolution(8))),
                mat(&mut materials, Color::srgb(0.8, 0.05, 0.05)),
                Transform::from_xyz(4.6, 2.2, 0.03).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            ));
        });
    // Yellow chevron board marking the corner in the gear-choice lesson.
    commands
        .spawn((
            CornerSign,
            Visibility::Hidden,
            Transform::from_xyz(4.6, 0.0, -CORNER_X),
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.04, 1.6))),
                white.clone(),
                Transform::from_xyz(0.0, 0.8, 0.0),
            ));
            p.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.4, 0.6, 0.04))),
                mat(&mut materials, Color::srgb(0.95, 0.8, 0.1)),
                Transform::from_xyz(0.0, 1.7, 0.0),
            ));
        });
    // Parking space outline. PARK_BOX is where the driver's seat must stop,
    // so the painted space runs a car-length ahead of it.
    let (near, far) = (PARK_BOX.0 - 1.0, PARK_BOX.1 + 3.0);
    let side = meshes.add(Cuboid::new(0.12, 0.03, far - near));
    let end = meshes.add(Cuboid::new(2.6, 0.03, 0.12));
    commands
        .spawn((ParkingLines, Visibility::Hidden, Transform::default()))
        .with_children(|p| {
            let mid = -(near + far) / 2.0;
            for x in [-1.3, 1.3] {
                p.spawn((
                    Mesh3d(side.clone()),
                    white.clone(),
                    Transform::from_xyz(x, 0.02, mid),
                ));
            }
            p.spawn((
                Mesh3d(end.clone()),
                white.clone(),
                Transform::from_xyz(0.0, 0.02, -far),
            ));
        });
}

#[allow(clippy::type_complexity)]
fn place_props(
    drive: Res<Drive>,
    mut q: ParamSet<(
        Single<(&mut Transform, &mut Visibility), With<LeadCar>>,
        Single<(&mut Transform, &mut Visibility), With<StopProp>>,
        Single<&mut Visibility, With<ParkingLines>>,
        Single<&mut Visibility, With<CornerSign>>,
    )>,
) {
    let lesson = drive.lesson.as_ref();
    let shown = |on: bool| {
        if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    {
        let mut lead = q.p0();
        let (t, vis) = &mut *lead;
        let x = lesson.and_then(|l| l.lead).map(|l| l.x);
        **vis = shown(x.is_some());
        // At gap 0 the lead's rear bumper is CAR_LEN ahead of our reference point.
        t.translation = Vec3::new(0.0, 0.65, -(x.unwrap_or(0.0) + CAR_LEN / 2.0));
    }
    {
        let mut stop = q.p1();
        let (t, vis) = &mut *stop;
        let line = lesson.and_then(|l| l.id.stop_line());
        **vis = shown(line.is_some());
        let line = line.unwrap_or(0.0);
        t.translation = Vec3::new(0.0, road_height(line, grade(&drive)), -line);
    }
    **q.p2() = shown(lesson.is_some_and(|l| l.id == LessonId::Parking));
    **q.p3() = shown(lesson.is_some_and(|l| l.id == LessonId::CornerGear));
}

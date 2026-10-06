//! Low-poly world and the driver's-seat view, built from Bevy primitives.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;
use drivetrain::SimState;

use leafwing_input_manager::prelude::*;

use crate::driving::Drive;
use crate::input::{Action, Pedals};

const ROAD_LEN: f32 = 4000.0;
const TACH_MAX_RPM: f32 = 8000.0;
const SPEEDO_MAX_KMH: f32 = 220.0;
/// Needle sweep: 135° either side of straight up.
const SWEEP: f32 = 0.75 * PI;
const STEER_WHEEL_TURNS: f32 = 1.5 * PI;

#[derive(Component)]
struct CarRig;
#[derive(Component)]
struct DriverCamera;
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
        .add_systems(Startup, (spawn_world, spawn_car))
        .add_systems(Update, (follow_sim, animate_cockpit, switch_view));
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
    // A few buildings either side, placed deterministically.
    let block = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    for i in 0..120 {
        let h = 4.0 + (i * 7 % 13) as f32;
        let w = 6.0 + (i * 5 % 7) as f32;
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let tint = 0.55 + (i * 3 % 5) as f32 * 0.08;
        commands.spawn((
            Mesh3d(block.clone()),
            mat(&mut materials, Color::srgb(tint, tint * 0.9, tint * 0.8)),
            Transform::from_xyz(
                side * (14.0 + (i % 3) as f32 * 4.0),
                h / 2.0,
                20.0 - i as f32 * 30.0,
            )
            .with_scale(Vec3::new(w, h, w)),
        ));
    }
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
fn follow_sim(drive: Res<Drive>, mut rig: Single<&mut Transform, With<CarRig>>) {
    let s = drive.sim.state();
    rig.translation = Vec3::new(-s.y, 0.0, -s.x);
    rig.rotation = Quat::from_rotation_y(s.heading);
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

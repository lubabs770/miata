//! Keyboard + gamepad → `drivetrain::Controls`, via leafwing actions.

use bevy::prelude::*;
use drivetrain::Controls;
use leafwing_input_manager::plugin::InputManagerSystem;
use leafwing_input_manager::prelude::*;

/// Keyboard pedal ramp rates, pedal travel per second.
const KEY_PRESS_RATE: f32 = 4.0;
const KEY_RELEASE_RATE: f32 = 4.0;
/// Releasing the clutch key lets the pedal up slowly so keyboard players can
/// feather it through the bite point.
const CLUTCH_KEY_RELEASE_RATE: f32 = 0.35;
/// How far the right stick must move to select a gate on the H-pattern.
const GATE_THRESHOLD: f32 = 0.7;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum Action {
    // Analog (gamepad triggers / stick).
    Throttle,
    Clutch,
    #[actionlike(Axis)]
    BrakeStick,
    // Digital keys, ramped in software.
    ThrottleKey,
    BrakeKey,
    ClutchKey,
    #[actionlike(Axis)]
    Steer,
    /// Right stick as an H-pattern shifter.
    #[actionlike(DualAxis)]
    Shifter,
    Gear1,
    Gear2,
    Gear3,
    Gear4,
    Gear5,
    GearR,
    Neutral,
    Handbrake,
    Ignition,
    CycleView,
}

impl Action {
    pub fn default_map() -> InputMap<Self> {
        use Action::*;
        let mut m = InputMap::default();
        m.insert(Throttle, GamepadButton::RightTrigger2);
        m.insert(Clutch, GamepadButton::LeftTrigger2);
        m.insert_axis(BrakeStick, GamepadControlAxis::LEFT_Y);
        m.insert_axis(Steer, GamepadControlAxis::LEFT_X);
        m.insert_dual_axis(Shifter, GamepadStick::RIGHT);
        m.insert(Neutral, GamepadButton::RightThumb);
        m.insert(Handbrake, GamepadButton::West);
        m.insert(Ignition, GamepadButton::North);
        m.insert(CycleView, GamepadButton::Select);

        m.insert(ThrottleKey, KeyCode::KeyW);
        m.insert(BrakeKey, KeyCode::KeyS);
        m.insert(ClutchKey, KeyCode::ShiftLeft);
        m.insert_axis(Steer, VirtualAxis::ad());
        for (a, k) in [
            (Gear1, KeyCode::Digit1),
            (Gear2, KeyCode::Digit2),
            (Gear3, KeyCode::Digit3),
            (Gear4, KeyCode::Digit4),
            (Gear5, KeyCode::Digit5),
            (GearR, KeyCode::KeyR),
            (Neutral, KeyCode::KeyN),
            (Handbrake, KeyCode::Space),
            (Ignition, KeyCode::KeyI),
            (CycleView, KeyCode::KeyV),
        ] {
            m.insert(a, k);
        }
        m
    }
}

/// The player's current pedal/lever state, rebuilt each frame.
#[derive(Resource, Default)]
pub struct Pedals {
    pub controls: Controls,
    pub handbrake_on: bool,
    kb_throttle: f32,
    kb_brake: f32,
    kb_clutch: f32,
    last_gate: Option<i8>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Pedals>()
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Action::default_map());
        })
        .add_systems(PreUpdate, read_input.after(InputManagerSystem::Update));
}

fn ramp(current: f32, held: bool, up: f32, down: f32, dt: f32) -> f32 {
    if held {
        current + up * dt
    } else {
        current - down * dt
    }
    .clamp(0.0, 1.0)
}

/// Map a right-stick position to an H-pattern gate:
/// left column 1/2, middle 3/4, right 5/R.
pub fn gate(stick: Vec2) -> Option<i8> {
    let row = if stick.y > GATE_THRESHOLD {
        0
    } else if stick.y < -GATE_THRESHOLD {
        1
    } else {
        return None;
    };
    let col = if stick.x < -0.5 {
        0
    } else if stick.x > 0.5 {
        2
    } else {
        1
    };
    Some([[1, 2], [3, 4], [5, -1]][col][row])
}

fn read_input(a: Single<&ActionState<Action>>, time: Res<Time>, mut p: ResMut<Pedals>) {
    let dt = time.delta_secs();
    p.kb_throttle = ramp(
        p.kb_throttle,
        a.pressed(&Action::ThrottleKey),
        KEY_PRESS_RATE,
        KEY_RELEASE_RATE,
        dt,
    );
    p.kb_brake = ramp(
        p.kb_brake,
        a.pressed(&Action::BrakeKey),
        KEY_PRESS_RATE,
        KEY_RELEASE_RATE,
        dt,
    );
    p.kb_clutch = ramp(
        p.kb_clutch,
        a.pressed(&Action::ClutchKey),
        KEY_PRESS_RATE,
        CLUTCH_KEY_RELEASE_RATE,
        dt,
    );
    if a.just_pressed(&Action::Handbrake) {
        p.handbrake_on = !p.handbrake_on;
    }

    let gate_now = gate(a.axis_pair(&Action::Shifter));
    let stick_shift = gate_now.filter(|g| Some(*g) != p.last_gate);
    p.last_gate = gate_now;
    let key_shift = [
        (Action::Gear1, 1),
        (Action::Gear2, 2),
        (Action::Gear3, 3),
        (Action::Gear4, 4),
        (Action::Gear5, 5),
        (Action::GearR, -1),
        (Action::Neutral, 0),
    ]
    .into_iter()
    .find(|(act, _)| a.just_pressed(act))
    .map(|(_, g)| g);

    p.controls = Controls {
        clutch: a.button_value(&Action::Clutch).max(p.kb_clutch),
        throttle: a.button_value(&Action::Throttle).max(p.kb_throttle),
        brake: (-a.value(&Action::BrakeStick)).max(0.0).max(p.kb_brake),
        handbrake: if p.handbrake_on { 1.0 } else { 0.0 },
        // Sim steer is +left; the A/D axis and stick are +right.
        steer: -a.value(&Action::Steer),
        shift: key_shift.or(stick_shift),
        ignition: a.just_pressed(&Action::Ignition),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stick_gates_follow_h_pattern() {
        assert_eq!(gate(Vec2::new(-1.0, 1.0)), Some(1));
        assert_eq!(gate(Vec2::new(-1.0, -1.0)), Some(2));
        assert_eq!(gate(Vec2::new(0.0, 1.0)), Some(3));
        assert_eq!(gate(Vec2::new(0.0, -1.0)), Some(4));
        assert_eq!(gate(Vec2::new(1.0, 1.0)), Some(5));
        assert_eq!(gate(Vec2::new(1.0, -1.0)), Some(-1));
        assert_eq!(gate(Vec2::new(0.9, 0.1)), None);
    }

    #[test]
    fn clutch_key_releases_slowly() {
        let down = ramp(0.0, true, KEY_PRESS_RATE, CLUTCH_KEY_RELEASE_RATE, 1.0);
        assert_eq!(down, 1.0);
        let after_1s = ramp(down, false, KEY_PRESS_RATE, CLUTCH_KEY_RELEASE_RATE, 1.0);
        assert!((after_1s - 0.65).abs() < 1e-6);
    }
}

//! Keyboard + gamepad → `drivetrain::Controls`, via leafwing actions.

use bevy::prelude::*;
use drivetrain::Controls;
use leafwing_input_manager::plugin::InputManagerSystem;
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

use crate::controls_ui::ControlsUi;

/// Keyboard pedal ramp rates, pedal travel per second.
const KEY_PRESS_RATE: f32 = 4.0;
const KEY_RELEASE_RATE: f32 = 4.0;
/// Releasing the clutch key lets the pedal up slowly so keyboard players can
/// feather it through the bite point.
const CLUTCH_KEY_RELEASE_RATE: f32 = 0.35;
/// How far the right stick must move to select a gate on the H-pattern.
const GATE_THRESHOLD: f32 = 0.7;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect, Serialize, Deserialize)]
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
    /// Paddles / sequential lever (auto-clutch modes only).
    ShiftUp,
    ShiftDown,
}

/// Keys reserved for steering (a fixed axis), so they can't be rebound.
pub const STEER_KEYS: [KeyCode; 2] = [KeyCode::KeyA, KeyCode::KeyD];

/// Rows of the controls screen: label, keyboard action, gamepad action.
/// Brake, steering and the H-pattern stay on fixed sticks on a gamepad.
pub const BINDING_ROWS: [(&str, Option<Action>, Option<Action>); 15] = {
    use Action::*;
    [
        ("Gas", Some(ThrottleKey), Some(Throttle)),
        ("Brake", Some(BrakeKey), None),
        ("Clutch", Some(ClutchKey), Some(Clutch)),
        ("1st", Some(Gear1), None),
        ("2nd", Some(Gear2), None),
        ("3rd", Some(Gear3), None),
        ("4th", Some(Gear4), None),
        ("5th", Some(Gear5), None),
        ("Reverse", Some(GearR), None),
        ("Neutral", Some(Neutral), Some(Neutral)),
        ("Handbrake", Some(Handbrake), Some(Handbrake)),
        ("Ignition", Some(Ignition), Some(Ignition)),
        ("Camera view", Some(CycleView), Some(CycleView)),
        ("Shift up (paddle)", Some(ShiftUp), Some(ShiftUp)),
        ("Shift down (paddle)", Some(ShiftDown), Some(ShiftDown)),
    ]
};

/// Rebindable buttons: one key and one gamepad button per action. Axes
/// (steering, brake stick, H-pattern stick) are fixed.
#[derive(Resource, Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Bindings {
    pub keys: Vec<(Action, KeyCode)>,
    pub pads: Vec<(Action, GamepadButton)>,
}

impl Default for Bindings {
    fn default() -> Self {
        use Action::*;
        Self {
            keys: vec![
                (ThrottleKey, KeyCode::KeyW),
                (BrakeKey, KeyCode::KeyS),
                (ClutchKey, KeyCode::ShiftLeft),
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
                (ShiftUp, KeyCode::KeyE),
                (ShiftDown, KeyCode::KeyQ),
            ],
            pads: vec![
                (Throttle, GamepadButton::RightTrigger2),
                (Clutch, GamepadButton::LeftTrigger2),
                (Neutral, GamepadButton::RightThumb),
                (Handbrake, GamepadButton::West),
                (Ignition, GamepadButton::North),
                (CycleView, GamepadButton::Select),
                (ShiftUp, GamepadButton::RightTrigger),
                (ShiftDown, GamepadButton::LeftTrigger),
            ],
        }
    }
}

impl Bindings {
    pub fn key(&self, a: Action) -> Option<KeyCode> {
        self.keys.iter().find(|(x, _)| *x == a).map(|(_, k)| *k)
    }

    pub fn pad(&self, a: Action) -> Option<GamepadButton> {
        self.pads.iter().find(|(x, _)| *x == a).map(|(_, b)| *b)
    }

    /// Returns the action that had `key` and now has `a`'s old key, if any.
    pub fn bind_key(&mut self, a: Action, key: KeyCode) -> Option<Action> {
        rebind(&mut self.keys, a, key)
    }

    pub fn bind_pad(&mut self, a: Action, button: GamepadButton) -> Option<Action> {
        rebind(&mut self.pads, a, button)
    }

    pub fn input_map(&self) -> InputMap<Action> {
        use Action::*;
        let mut m = InputMap::default();
        m.insert_axis(BrakeStick, GamepadControlAxis::LEFT_Y);
        m.insert_axis(Steer, GamepadControlAxis::LEFT_X);
        m.insert_axis(Steer, VirtualAxis::new(STEER_KEYS[0], STEER_KEYS[1]));
        m.insert_dual_axis(Shifter, GamepadStick::RIGHT);
        for &(a, k) in &self.keys {
            m.insert(a, k);
        }
        for &(a, b) in &self.pads {
            m.insert(a, b);
        }
        m
    }
}

/// Bind `input` to `a`. Whoever already had `input` swaps to `a`'s old one
/// (or loses its binding if `a` had none). Returns that other action.
fn rebind<T: Copy + PartialEq>(list: &mut Vec<(Action, T)>, a: Action, input: T) -> Option<Action> {
    let old = list.iter().find(|(x, _)| *x == a).map(|(_, t)| *t);
    let clash = list.iter().position(|(x, t)| *t == input && *x != a);
    let swapped = clash.map(|j| list[j].0);
    if let Some(j) = clash {
        match old {
            Some(o) => list[j].1 = o,
            None => {
                list.remove(j);
            }
        }
    }
    match list.iter_mut().find(|(x, _)| *x == a) {
        Some(entry) => entry.1 = input,
        None => list.push((a, input)),
    }
    swapped
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
        .init_resource::<Bindings>()
        .add_systems(Startup, |mut commands: Commands, b: Res<Bindings>| {
            commands.spawn(b.input_map());
        })
        .add_systems(Update, apply_bindings)
        .add_systems(PreUpdate, read_input.after(InputManagerSystem::Update));
}

fn apply_bindings(bindings: Res<Bindings>, mut map: Single<&mut InputMap<Action>>) {
    if bindings.is_changed() {
        **map = bindings.input_map();
    }
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

fn read_input(
    a: Single<&ActionState<Action>>,
    time: Res<Time>,
    mut p: ResMut<Pedals>,
    controls_ui: Res<ControlsUi>,
) {
    if controls_ui.capturing() {
        // The press is for the controls screen, not the car.
        p.controls.shift = None;
        p.controls.ignition = false;
        p.controls.sequential = 0;
        return;
    }
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
        sequential: a.just_pressed(&Action::ShiftUp) as i8
            - a.just_pressed(&Action::ShiftDown) as i8,
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
    fn default_bindings_have_no_conflicts() {
        let b = Bindings::default();
        for list in [
            b.keys
                .iter()
                .map(|(_, k)| format!("{k:?}"))
                .collect::<Vec<_>>(),
            b.pads.iter().map(|(_, p)| format!("{p:?}")).collect(),
        ] {
            let mut sorted = list.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), list.len(), "duplicate binding in {list:?}");
        }
        assert!(b.keys.iter().all(|(_, k)| !STEER_KEYS.contains(k)));
        for (_, key, pad) in BINDING_ROWS {
            for a in key.into_iter().chain(pad) {
                assert!(b.key(a).is_some() || b.pad(a).is_some(), "{a:?} unbound");
            }
        }
    }

    #[test]
    fn rebinding_a_taken_key_swaps() {
        let mut b = Bindings::default();
        let swapped = b.bind_key(Action::Handbrake, KeyCode::KeyI);
        assert_eq!(swapped, Some(Action::Ignition));
        assert_eq!(b.key(Action::Handbrake), Some(KeyCode::KeyI));
        assert_eq!(b.key(Action::Ignition), Some(KeyCode::Space));
        assert_eq!(b.bind_key(Action::Handbrake, KeyCode::KeyH), None);
    }

    #[test]
    fn bindings_round_trip_through_json() {
        let mut b = Bindings::default();
        b.bind_pad(Action::Clutch, GamepadButton::LeftTrigger);
        let back: Bindings = serde_json::from_str(&serde_json::to_string(&b).unwrap()).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn clutch_key_releases_slowly() {
        let down = ramp(0.0, true, KEY_PRESS_RATE, CLUTCH_KEY_RELEASE_RATE, 1.0);
        assert_eq!(down, 1.0);
        let after_1s = ramp(down, false, KEY_PRESS_RATE, CLUTCH_KEY_RELEASE_RATE, 1.0);
        assert!((after_1s - 0.65).abs() < 1e-6);
    }
}

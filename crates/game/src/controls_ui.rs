//! Controls screen: click a binding, press a key or gamepad button.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use crate::AppState;
use crate::input::{Action, BINDING_ROWS, Bindings, STEER_KEYS};
use crate::pedals::Wizard;

#[derive(Resource, Default)]
pub struct ControlsUi {
    pub open: bool,
    /// Waiting for a press to bind: the action, and whether it's the gamepad column.
    capture: Option<(Action, bool)>,
    note: Option<String>,
}

impl ControlsUi {
    /// While capturing, game input should be ignored.
    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<ControlsUi>()
        .add_systems(
            EguiPrimaryContextPass,
            window.run_if(in_state(AppState::Driving)),
        )
        .add_systems(Update, capture.run_if(in_state(AppState::Driving)));
}

fn label_of(a: Action) -> &'static str {
    BINDING_ROWS
        .iter()
        .find(|(_, k, p)| *k == Some(a) || *p == Some(a))
        .map_or("another action", |(label, _, _)| label)
}

fn key_name(k: KeyCode) -> String {
    let s = format!("{k:?}");
    s.strip_prefix("Key")
        .or_else(|| s.strip_prefix("Digit"))
        .unwrap_or(&s)
        .to_string()
}

fn pad_name(b: GamepadButton) -> String {
    use GamepadButton::*;
    match b {
        South => "A / ✕".into(),
        East => "B / ○".into(),
        West => "X / □".into(),
        North => "Y / △".into(),
        LeftTrigger => "LB".into(),
        RightTrigger => "RB".into(),
        LeftTrigger2 => "LT".into(),
        RightTrigger2 => "RT".into(),
        LeftThumb => "L3".into(),
        RightThumb => "R3".into(),
        DPadUp => "D-pad up".into(),
        DPadDown => "D-pad down".into(),
        DPadLeft => "D-pad left".into(),
        DPadRight => "D-pad right".into(),
        other => format!("{other:?}"),
    }
}

fn window(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<ControlsUi>,
    mut bindings: ResMut<Bindings>,
    mut wizard: ResMut<Wizard>,
) -> Result {
    if !ui_state.open {
        return Ok(());
    }
    let mut open = true;
    egui::Window::new("Controls")
        .open(&mut open)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .show(contexts.ctx_mut()?, |ui| {
            ui.label("Click a binding, then press a key or gamepad button. Esc cancels.");
            egui::Grid::new("bindings").striped(true).show(ui, |ui| {
                ui.strong("Action");
                ui.strong("Keyboard");
                ui.strong("Gamepad");
                ui.end_row();
                for (label, key_action, pad_action) in BINDING_ROWS {
                    ui.label(label);
                    for (action, pad) in [(key_action, false), (pad_action, true)] {
                        let Some(a) = action else {
                            ui.weak(if pad { "stick" } else { "—" });
                            continue;
                        };
                        let waiting = ui_state.capture == Some((a, pad));
                        let text = if waiting {
                            "press…".to_string()
                        } else if pad {
                            bindings.pad(a).map_or("unbound".into(), pad_name)
                        } else {
                            bindings.key(a).map_or("unbound".into(), key_name)
                        };
                        if ui.selectable_label(waiting, text).clicked() {
                            ui_state.capture = Some((a, pad));
                            ui_state.note = None;
                        }
                    }
                    ui.end_row();
                }
                ui.label("Steer");
                ui.weak("A / D");
                ui.weak("left stick");
                ui.end_row();
                ui.label("Brake (gamepad)");
                ui.weak("");
                ui.weak("left stick down");
                ui.end_row();
                ui.label("Gears (gamepad)");
                ui.weak("");
                ui.weak("right stick H-pattern");
                ui.end_row();
            });
            if let Some(note) = &ui_state.note {
                ui.colored_label(egui::Color32::YELLOW, note);
            }
            ui.horizontal(|ui| {
                if ui.button("Reset to defaults").clicked() {
                    *bindings = Bindings::default();
                    ui_state.note = Some("Defaults restored.".into());
                }
                if ui.button("Wheel & pedals…").clicked() {
                    wizard.open = true;
                }
            });
        });
    if !open {
        ui_state.open = false;
        ui_state.capture = None;
    }
    Ok(())
}

fn capture(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut ui_state: ResMut<ControlsUi>,
    mut bindings: ResMut<Bindings>,
) {
    let Some((action, pad)) = ui_state.capture else {
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        ui_state.capture = None;
        return;
    }
    let swapped = if pad {
        let Some(button) = pads
            .iter()
            .find_map(|p| p.get_just_pressed().next().copied())
        else {
            return;
        };
        bindings.bind_pad(action, button)
    } else {
        let Some(&key) = keys.get_just_pressed().next() else {
            return;
        };
        if STEER_KEYS.contains(&key) {
            ui_state.note = Some("A and D steer; pick another key.".into());
            return;
        }
        bindings.bind_key(action, key)
    };
    ui_state.capture = None;
    ui_state.note = swapped.map(|other| format!("That was on {}; they swapped.", label_of(other)));
}

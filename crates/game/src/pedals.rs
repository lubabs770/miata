//! Steering wheel and pedal calibration. Wheels report pedals and steering
//! as raw gamepad axes with arbitrary (often inverted) ranges; the wizard
//! records which input each one is and where it rests and bottoms out.

use std::collections::HashMap;

use bevy::input::gamepad::GamepadInput;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use serde::{Deserialize, Serialize};

use crate::AppState;

/// Inputs must move at least this far to count as "the one you pressed".
const MIN_TRAVEL: f32 = 0.3;
/// Ignore the first bit of pedal travel (sensor noise at rest).
const DEADZONE: f32 = 0.03;

/// Serializable mirror of Bevy's `GamepadInput` (whose own serde support
/// covers the axis and button types but not the wrapper).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PadInput {
    Axis(GamepadAxis),
    Button(GamepadButton),
}

impl From<GamepadInput> for PadInput {
    fn from(i: GamepadInput) -> Self {
        match i {
            GamepadInput::Axis(a) => PadInput::Axis(a),
            GamepadInput::Button(b) => PadInput::Button(b),
        }
    }
}

/// One pedal: `rest` maps to 0, `full` to 1, whichever way round they are.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub struct AxisCal {
    pub input: PadInput,
    pub rest: f32,
    pub full: f32,
}

impl AxisCal {
    pub fn value(&self, raw: f32) -> f32 {
        let t = ((raw - self.rest) / (self.full - self.rest)).clamp(0.0, 1.0);
        ((t - DEADZONE) / (1.0 - DEADZONE)).max(0.0)
    }
}

/// Steering: `left` maps to -1, `right` to +1.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub struct SteerCal {
    pub input: PadInput,
    pub left: f32,
    pub right: f32,
}

impl SteerCal {
    pub fn value(&self, raw: f32) -> f32 {
        ((raw - self.left) / (self.right - self.left) * 2.0 - 1.0).clamp(-1.0, 1.0)
    }
}

#[derive(Resource, Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct PedalCal {
    pub clutch: Option<AxisCal>,
    pub brake: Option<AxisCal>,
    pub gas: Option<AxisCal>,
    pub steer: Option<SteerCal>,
    /// Square gas and brake for finer control near the top of the pedal.
    pub progressive: bool,
}

/// Calibrated pedal/wheel readings for this frame; `None` where uncalibrated
/// or no pad reports that input.
#[derive(Default, Debug, PartialEq)]
pub struct Readings {
    pub clutch: Option<f32>,
    pub brake: Option<f32>,
    pub gas: Option<f32>,
    /// -1 left .. +1 right.
    pub steer: Option<f32>,
}

impl PedalCal {
    pub fn read(&self, raw: impl Fn(PadInput) -> Option<f32>) -> Readings {
        let curve = |v: f32| if self.progressive { v * v } else { v };
        let pedal = |c: &Option<AxisCal>| c.and_then(|c| raw(c.input).map(|r| c.value(r)));
        Readings {
            clutch: pedal(&self.clutch),
            brake: pedal(&self.brake).map(curve),
            gas: pedal(&self.gas).map(curve),
            steer: self.steer.and_then(|c| raw(c.input).map(|r| c.value(r))),
        }
    }
}

/// The input that moved furthest from `baseline` (ignoring `taken` ones),
/// with its current value.
fn moved_most(
    baseline: &HashMap<PadInput, f32>,
    now: &HashMap<PadInput, f32>,
    taken: &[PadInput],
) -> Option<(PadInput, f32)> {
    now.iter()
        .filter(|(i, _)| !taken.contains(i))
        .map(|(i, v)| (*i, *v, (v - baseline.get(i).copied().unwrap_or(0.0)).abs()))
        .filter(|(_, _, d)| *d >= MIN_TRAVEL)
        .max_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(i, v, _)| (i, v))
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Step {
    Clutch,
    Brake,
    Gas,
    SteerLeft,
    SteerRight,
}

impl Step {
    fn prompt(self) -> &'static str {
        match self {
            Step::Clutch => "Press the CLUTCH pedal all the way down and hold it, then click Next.",
            Step::Brake => "Press the BRAKE pedal all the way down and hold it, then click Next.",
            Step::Gas => "Press the GAS pedal all the way down and hold it, then click Next.",
            Step::SteerLeft => "Turn the wheel fully LEFT and hold it, then click Next.",
            Step::SteerRight => "Turn the wheel fully RIGHT and hold it, then click Next.",
        }
    }
}

/// Wizard state: the open step and the resting values it compares against.
#[derive(Resource, Default)]
pub struct Wizard {
    pub open: bool,
    step: Option<Step>,
    baseline: HashMap<PadInput, f32>,
    draft: PedalCal,
    steer_input: Option<(PadInput, f32)>,
    note: Option<&'static str>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<PedalCal>()
        .init_resource::<Wizard>()
        .add_systems(
            EguiPrimaryContextPass,
            wizard_window.run_if(in_state(AppState::Driving)),
        );
}

/// All analog inputs on the first connected pad.
pub fn raw_inputs(pads: &Query<&Gamepad>) -> HashMap<PadInput, f32> {
    pads.iter()
        .next()
        .map(|p| {
            p.analog()
                .all_axes_and_values()
                .map(|(i, v)| ((*i).into(), v))
                .collect()
        })
        .unwrap_or_default()
}

fn wizard_window(
    mut contexts: EguiContexts,
    mut wiz: ResMut<Wizard>,
    mut cal: ResMut<PedalCal>,
    pads: Query<&Gamepad>,
) -> Result {
    if !wiz.open {
        return Ok(());
    }
    let now = raw_inputs(&pads);
    let mut open = true;
    egui::Window::new("Wheel & pedals")
        .open(&mut open)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .show(contexts.ctx_mut()?, |ui| {
            if now.is_empty() {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "No wheel or gamepad detected. Plug it in and press a button.",
                );
            }
            match wiz.step {
                None => {
                    let r = cal.read(|i| now.get(&i).copied());
                    for (name, v) in [("clutch", r.clutch), ("brake", r.brake), ("gas", r.gas)] {
                        ui.horizontal(|ui| {
                            ui.label(format!("{name:>7}"));
                            match v {
                                Some(v) => ui.add(egui::ProgressBar::new(v).desired_width(160.0)),
                                None => ui.weak("not calibrated"),
                            };
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label("  steer");
                        match r.steer {
                            Some(v) => {
                                ui.add(egui::ProgressBar::new((v + 1.0) / 2.0).desired_width(160.0))
                            }
                            None => ui.weak("not calibrated"),
                        };
                    });
                    ui.checkbox(
                        &mut cal.progressive,
                        "Progressive gas and brake (finer control at the top)",
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Calibrate…").clicked() {
                            wiz.draft = PedalCal {
                                progressive: cal.progressive,
                                ..default()
                            };
                            wiz.baseline = now.clone();
                            wiz.step = Some(Step::Clutch);
                            wiz.note = None;
                        }
                        if ui.button("Clear").clicked() {
                            *cal = PedalCal::default();
                        }
                    });
                    ui.small("Release all pedals and centre the wheel before you start.");
                }
                Some(step) => {
                    ui.label(step.prompt());
                    if let Some(note) = wiz.note {
                        ui.colored_label(egui::Color32::YELLOW, note);
                    }
                    ui.horizontal(|ui| {
                        let next = ui.button("Next").clicked();
                        let skip = ui.button("Skip (I don't have this)").clicked();
                        if ui.button("Cancel").clicked() {
                            wiz.step = None;
                        }
                        if next || skip {
                            advance(&mut wiz, &mut cal, step, &now, skip);
                        }
                    });
                }
            }
        });
    if !open {
        wiz.open = false;
        wiz.step = None;
    }
    Ok(())
}

fn advance(
    wiz: &mut Wizard,
    cal: &mut PedalCal,
    step: Step,
    now: &HashMap<PadInput, f32>,
    skip: bool,
) {
    let taken: Vec<_> = [wiz.draft.clutch, wiz.draft.brake, wiz.draft.gas]
        .into_iter()
        .flatten()
        .map(|c| c.input)
        .collect();
    let found = match (skip, step) {
        (true, _) => None,
        // Second steering step: same input as the first, just the other way.
        (false, Step::SteerRight) => wiz.steer_input.and_then(|(i, left)| {
            now.get(&i)
                .filter(|r| (**r - left).abs() >= MIN_TRAVEL)
                .map(|r| (i, *r))
        }),
        (false, _) => moved_most(&wiz.baseline, now, &taken),
    };
    if !skip && found.is_none() {
        wiz.note = Some("Didn't see anything move far enough. Press it all the way and hold.");
        return;
    }
    wiz.note = None;
    let pedal = found.map(|(input, full)| AxisCal {
        input,
        rest: wiz.baseline[&input],
        full,
    });
    wiz.step = match step {
        Step::Clutch => {
            wiz.draft.clutch = pedal;
            Some(Step::Brake)
        }
        Step::Brake => {
            wiz.draft.brake = pedal;
            Some(Step::Gas)
        }
        Step::Gas => {
            wiz.draft.gas = pedal;
            Some(Step::SteerLeft)
        }
        Step::SteerLeft => {
            wiz.steer_input = found;
            if skip {
                *cal = wiz.draft.clone();
                None
            } else {
                Some(Step::SteerRight)
            }
        }
        Step::SteerRight => {
            if let (Some((input, left)), Some((_, right))) = (wiz.steer_input, found) {
                wiz.draft.steer = Some(SteerCal { input, left, right });
            }
            *cal = wiz.draft.clone();
            None
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverted_pedal_maps_rest_to_zero_and_floor_to_one() {
        // Many wheels report a released pedal as +1 and floored as -1.
        let c = AxisCal {
            input: PadInput::Axis(GamepadAxis::LeftZ),
            rest: 1.0,
            full: -1.0,
        };
        assert_eq!(c.value(1.0), 0.0);
        assert_eq!(c.value(-1.0), 1.0);
        assert!((c.value(0.0) - (0.5 - DEADZONE) / (1.0 - DEADZONE)).abs() < 1e-6);
        assert_eq!(c.value(1.02), 0.0, "noise past rest stays at zero");
    }

    #[test]
    fn steering_maps_left_and_right_to_minus_and_plus_one() {
        let s = SteerCal {
            input: PadInput::Axis(GamepadAxis::LeftStickX),
            left: -0.9,
            right: 0.9,
        };
        assert_eq!(s.value(-0.9), -1.0);
        assert_eq!(s.value(0.9), 1.0);
        assert!(s.value(0.0).abs() < 1e-6);
    }

    #[test]
    fn picks_the_input_that_moved_most() {
        let z = PadInput::Axis(GamepadAxis::LeftZ);
        let x = PadInput::Axis(GamepadAxis::LeftStickX);
        let base = HashMap::from([(z, 1.0), (x, 0.0)]);
        let now = HashMap::from([(z, -0.95), (x, 0.1)]);
        assert_eq!(moved_most(&base, &now, &[]), Some((z, -0.95)));
        assert_eq!(moved_most(&base, &base, &[]), None, "nothing moved");
        assert_eq!(
            moved_most(&base, &now, &[z]),
            None,
            "a still-held clutch isn't picked again"
        );
    }

    #[test]
    fn progressive_curve_applies_to_gas_and_brake_only() {
        let z = PadInput::Axis(GamepadAxis::LeftZ);
        let cal = AxisCal {
            input: z,
            rest: 0.0,
            full: 1.0,
        };
        let p = PedalCal {
            clutch: Some(cal),
            gas: Some(cal),
            progressive: true,
            ..default()
        };
        let r = p.read(|_| Some(0.5));
        assert!(r.gas.unwrap() < r.clutch.unwrap());
        assert_eq!(r.brake, None);
    }
}

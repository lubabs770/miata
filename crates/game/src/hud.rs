//! egui overlays: start screen, instruments, coaching, lesson picker.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use drivetrain::{LessonId, Outcome, TransmissionMode};
use egui_plot::{Legend, Line, Plot, PlotPoints, VLine};

use crate::AppState;
use crate::driving::{Drive, Sample, Trace};
use crate::input::Pedals;

const KEYS: &str = "Keyboard: W gas · S brake · Left Shift clutch (hold; releases slowly) · A/D steer · \
1–5 / R / N gears · Space handbrake · I ignition · V view · Q/E paddles\n\
Gamepad: RT gas · LT clutch · left stick steer, down = brake · right stick H-shifter · \
R3 neutral · X handbrake · Y ignition · Select view · LB/RB paddles";

pub fn plugin(app: &mut App) {
    app.add_systems(
        EguiPrimaryContextPass,
        (
            start_screen.run_if(in_state(AppState::ClickToStart)),
            (instruments, lesson_panel).run_if(in_state(AppState::Driving)),
        ),
    )
    .add_systems(
        Update,
        any_input_starts.run_if(in_state(AppState::ClickToStart)),
    );
}

fn any_input_starts(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    mut next: ResMut<NextState<AppState>>,
) {
    let pad = pads.iter().any(|p| p.get_just_pressed().next().is_some());
    if keys.get_just_pressed().next().is_some() || mouse.get_just_pressed().next().is_some() || pad
    {
        next.set(AppState::Driving);
    }
}

fn start_screen(mut contexts: EguiContexts) -> Result {
    egui::Window::new("start")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(contexts.ctx_mut()?, |ui| {
        ui.vertical_centered(|ui| {
            ui.heading(egui::RichText::new("miata").size(48.0));
            ui.label("Learn to drive a manual transmission.");
            ui.add_space(24.0);
            ui.label(egui::RichText::new("Click or press any key to start").size(22.0).strong());
            ui.add_space(24.0);
            ui.label(KEYS);
            ui.add_space(16.0);
            ui.label(
                egui::RichText::new(
                    "With a keyboard or gamepad this trains the sequence, the timing and reading the \
                     revs and sound. The physical feel of a clutch pedal only carries over with real pedals.",
                )
                .italics(),
            );
        });
    });
    Ok(())
}

fn bar(ui: &mut egui::Ui, label: &str, v: f32, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(format!("{label:>8}"));
        ui.add(egui::ProgressBar::new(v).desired_width(140.0).fill(color));
    });
}

fn instruments(mut contexts: EguiContexts, drive: Res<Drive>, pedals: Res<Pedals>) -> Result {
    let ctx = contexts.ctx_mut()?;
    let s = drive.sim.state();
    let c = &pedals.controls;
    egui::Area::new("instruments".into())
        .anchor(egui::Align2::LEFT_BOTTOM, [12.0, -12.0])
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                let gear = match s.gear {
                    -1 => "R".to_string(),
                    0 => "N".to_string(),
                    g => g.to_string(),
                };
                ui.label(
                    egui::RichText::new(format!(
                        "{gear}   {:>3.0} km/h   {:>4.0} rpm",
                        s.speed_mps.abs() * 3.6,
                        s.engine_rpm
                    ))
                    .size(24.0)
                    .monospace(),
                );
                bar(
                    ui,
                    "clutch",
                    c.clutch,
                    egui::Color32::from_rgb(90, 140, 230),
                );
                bar(ui, "brake", c.brake, egui::Color32::from_rgb(220, 70, 60));
                bar(ui, "gas", c.throttle, egui::Color32::from_rgb(80, 190, 90));
                ui.horizontal(|ui| {
                    if pedals.handbrake_on {
                        ui.colored_label(egui::Color32::YELLOW, "(P) handbrake");
                    }
                    if !s.engine_running {
                        ui.colored_label(egui::Color32::RED, "engine off");
                    }
                    ui.label(format!("coffee {:.0}%", drive.cup().level * 100.0));
                });
            });
        });
    if let Some(h) = drive.hint {
        egui::Area::new("hint".into())
            .anchor(egui::Align2::CENTER_TOP, [0.0, 24.0])
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(egui::RichText::new(h).size(26.0).strong());
                });
            });
    }
    Ok(())
}

fn lesson_panel(
    mut contexts: EguiContexts,
    mut drive: ResMut<Drive>,
    mut pedals: ResMut<Pedals>,
    time: Res<Time>,
) -> Result {
    let seed = time.elapsed().as_nanos() as u64;
    let mut start: Option<Option<LessonId>> = None;
    let mut car = drive.car;
    let mut mode = drive.mode;
    egui::Window::new("Lessons")
        .anchor(egui::Align2::RIGHT_TOP, [-12.0, 12.0])
        .resizable(false)
        .show(contexts.ctx_mut()?, |ui| {
            egui::ComboBox::from_label("car")
                .selected_text(&drive.cars[drive.car].name)
                .show_ui(ui, |ui| {
                    for (i, c) in drive.cars.iter().enumerate() {
                        ui.selectable_value(&mut car, i, &c.name);
                    }
                });
            egui::ComboBox::from_label("free-drive gearbox")
                .selected_text(mode_name(mode))
                .show_ui(ui, |ui| {
                    for m in [
                        TransmissionMode::Manual,
                        TransmissionMode::AutoClutch,
                        TransmissionMode::Paddles,
                    ] {
                        ui.selectable_value(&mut mode, m, mode_name(m));
                    }
                });
            ui.separator();
            for id in LessonId::ALL {
                if ui
                    .selectable_label(
                        drive.lesson.as_ref().is_some_and(|l| l.id == id),
                        id.title(),
                    )
                    .clicked()
                {
                    start = Some(Some(id));
                }
            }
            if ui
                .selectable_label(drive.lesson.is_none(), "Free drive")
                .clicked()
            {
                start = Some(None);
            }
            if let Some(run) = &drive.lesson {
                ui.separator();
                ui.label(run.id.brief());
                match drive.outcome {
                    Outcome::Running => {}
                    Outcome::Passed { stars } => {
                        ui.heading(format!(
                            "Passed {}",
                            "★".repeat(stars as usize) + &"☆".repeat(3 - stars as usize)
                        ));
                    }
                    Outcome::Failed(why) => {
                        ui.colored_label(egui::Color32::RED, why);
                    }
                }
                if drive.outcome != Outcome::Running {
                    replay(ui, &drive.trace);
                }
                if ui.button("Retry (Enter)").clicked() {
                    start = Some(Some(run.id));
                }
            }
            ui.separator();
            ui.small(KEYS);
        });
    if start.is_none() && drive.lesson.is_some() && drive.outcome != Outcome::Running {
        // Enter retries a finished lesson without reaching for the mouse.
        if contexts
            .ctx_mut()?
            .input(|i| i.key_pressed(egui::Key::Enter))
        {
            start = drive.lesson.as_ref().map(|l| Some(l.id));
        }
    }
    if mode != drive.mode {
        drive.mode = mode;
        start = start.or(Some(None));
    }
    if car != drive.car {
        // A new car restarts whatever you were doing in it.
        drive.car = car;
        start = start.or(Some(drive.lesson.as_ref().map(|l| l.id)));
    }
    if let Some(choice) = start {
        drive.start(choice, seed, &mut pedals);
    }
    Ok(())
}

fn mode_name(m: TransmissionMode) -> &'static str {
    match m {
        TransmissionMode::Manual => "Manual (you work the clutch)",
        TransmissionMode::AutoClutch => "Auto-clutch (H-pattern, no clutch)",
        TransmissionMode::Paddles => "Paddles (+/-)",
    }
}

/// What your feet and the revs did during the attempt; stalls marked red.
fn replay(ui: &mut egui::Ui, trace: &Trace) {
    let series = |f: fn(&Sample) -> f32| -> PlotPoints<'static> {
        trace
            .samples
            .iter()
            .map(|s| [s.t as f64, f(s) as f64])
            .collect::<Vec<_>>()
            .into()
    };
    ui.label("Replay: pedals and revs");
    Plot::new("replay")
        .height(160.0)
        .width(320.0)
        .include_y(0.0)
        .include_y(1.0)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .legend(Legend::default())
        .show(ui, |p| {
            p.line(
                Line::new("clutch", series(|s| s.clutch))
                    .color(egui::Color32::from_rgb(90, 140, 230)),
            );
            p.line(
                Line::new("gas", series(|s| s.throttle))
                    .color(egui::Color32::from_rgb(80, 190, 90)),
            );
            p.line(
                Line::new("brake", series(|s| s.brake)).color(egui::Color32::from_rgb(220, 70, 60)),
            );
            p.line(
                Line::new("rpm / redline", series(|s| s.rpm))
                    .color(egui::Color32::from_rgb(240, 160, 40)),
            );
            for &t in &trace.stalls {
                p.vline(VLine::new("stall", t).color(egui::Color32::RED).width(2.0));
            }
        });
}

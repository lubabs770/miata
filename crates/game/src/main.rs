mod audio;
mod cockpit;
mod controls_ui;
mod driving;
mod hud;
mod input;
mod pedals;
mod save;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use leafwing_input_manager::prelude::*;

#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AppState {
    /// Browsers only allow audio after a user gesture, so we wait for one.
    #[default]
    ClickToStart,
    Driving,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "miata — learn to drive stick".into(),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(InputManagerPlugin::<input::Action>::default())
        .init_state::<AppState>()
        .add_plugins((
            input::plugin,
            driving::plugin,
            cockpit::plugin,
            hud::plugin,
            audio::plugin,
            save::plugin,
            controls_ui::plugin,
            pedals::plugin,
        ))
        .run();
}

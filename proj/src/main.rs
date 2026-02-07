use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::window::{PresentMode, PrimaryWindow};
mod tank;

const BASE_WINDOW_TITLE: &str = "Tank Test Window";

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: BASE_WINDOW_TITLE.to_string(),
                resolution: (1280.0, 720.0).into(),
                present_mode: PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins(tank::TankPlugin)
        .add_systems(Startup, setup_camera)
        .add_systems(Update, update_window_title_with_fps)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}

fn update_window_title_with_fps(
    diagnostics: Res<DiagnosticsStore>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Ok(mut window) = windows.get_single_mut() else {
        return;
    };

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed());

    if let Some(fps) = fps {
        window.title = format!("{BASE_WINDOW_TITLE} | FPS: {:.0}", fps);
    } else {
        window.title = BASE_WINDOW_TITLE.to_string();
    }
}

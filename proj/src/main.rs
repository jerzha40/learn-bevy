use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::window::{ExitCondition, PresentMode};
mod projectile;
mod portal;
mod save;
mod tank;
mod windowblob;

const BASE_WINDOW_TITLE: &str = "Tank Test Window";
const BLOB_PIXELS_PER_BM: f32 = windowblob::DEFAULT_PIXELS_PER_BM;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: BASE_WINDOW_TITLE.to_string(),
                resolution: (
                    windowblob::MAIN_BLOB_SIZE_BM.x * BLOB_PIXELS_PER_BM,
                    windowblob::MAIN_BLOB_SIZE_BM.y * BLOB_PIXELS_PER_BM,
                )
                    .into(),
                present_mode: PresentMode::AutoNoVsync,
                ..default()
            }),
            close_when_requested: false,
            exit_condition: ExitCondition::DontExit,
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins(windowblob::WindowBlobPlugin)
        .add_plugins(save::SavePlugin)
        .add_plugins(portal::PortalPlugin)
        .add_plugins(tank::TankPlugin)
        .add_plugins(projectile::ProjectilePlugin)
        .add_systems(Update, update_window_title_with_fps)
        .run();
}

fn update_window_title_with_fps(
    diagnostics: Res<DiagnosticsStore>,
    mut windows: Query<(&windowblob::BlobWindow, &mut Window)>,
) {
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed());

    for (blob_window, mut window) in &mut windows {
        let blob_label = format!("{} [{}]", BASE_WINDOW_TITLE, blob_window.save_file);
        if let Some(fps) = fps {
            window.title = format!("{blob_label} | FPS: {:.0}", fps);
        } else {
            window.title = blob_label;
        }
    }
}

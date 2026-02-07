use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::window::{PresentMode, PrimaryWindow};
mod projectile;
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
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .insert_resource(windowblob::BlobRenderSettings {
            pixels_per_bm: BLOB_PIXELS_PER_BM,
        })
        .add_plugins(windowblob::WindowBlobPlugin)
        .add_plugins(tank::TankPlugin)
        .add_plugins(projectile::ProjectilePlugin)
        .add_systems(Startup, setup_camera)
        .add_systems(Update, update_window_title_with_fps)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}

fn update_window_title_with_fps(
    active_blob: Res<windowblob::ActiveWindowBlob>,
    diagnostics: Res<DiagnosticsStore>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Ok(mut window) = windows.get_single_mut() else {
        return;
    };

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed());

    let blob_label = format!("{} [{}]", BASE_WINDOW_TITLE, active_blob.save_file);

    if let Some(fps) = fps {
        window.title = format!("{blob_label} | FPS: {:.0}", fps);
    } else {
        window.title = blob_label;
    }
}

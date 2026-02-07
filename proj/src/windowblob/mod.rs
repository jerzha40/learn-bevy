use bevy::prelude::*;
use bevy::render::camera::{OrthographicProjection, ScalingMode};
use bevy::window::PrimaryWindow;

use crate::tank::{Tank, TANK_BODY_RADIUS_BM};

pub const MAIN_BLOB_SAVE_FILE: &str = "main.blob.json";
pub const MAIN_BLOB_SIZE_BM: Vec2 = Vec2::new(10.0, 10.0);
pub const DEFAULT_PIXELS_PER_BM: f32 = 72.0;

pub struct WindowBlobPlugin;

impl Plugin for WindowBlobPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveWindowBlob>()
            .init_resource::<BlobRenderSettings>()
            .init_resource::<BlobRenderMetrics>()
            .add_systems(Update, apply_blob_projection_to_camera)
            .add_systems(
                Update,
                (
                    enforce_window_resolution_from_blob_settings,
                    update_blob_render_metrics_from_window,
                )
                    .chain(),
            )
            .add_systems(PostUpdate, clamp_tanks_inside_blob_boundary);
    }
}

#[derive(Resource, Debug, Clone)]
pub struct ActiveWindowBlob {
    pub save_file: String,
    pub size_bm: Vec2,
}

impl Default for ActiveWindowBlob {
    fn default() -> Self {
        Self {
            save_file: MAIN_BLOB_SAVE_FILE.to_string(),
            size_bm: MAIN_BLOB_SIZE_BM,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct BlobRenderSettings {
    pub pixels_per_bm: f32,
}

impl Default for BlobRenderSettings {
    fn default() -> Self {
        Self {
            pixels_per_bm: DEFAULT_PIXELS_PER_BM,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct BlobRenderMetrics {
    pub pixels_per_bm: Vec2,
}

impl Default for BlobRenderMetrics {
    fn default() -> Self {
        Self {
            pixels_per_bm: Vec2::new(128.0, 72.0),
        }
    }
}

fn enforce_window_resolution_from_blob_settings(
    active_blob: Res<ActiveWindowBlob>,
    render_settings: Res<BlobRenderSettings>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Ok(mut window) = windows.get_single_mut() else {
        return;
    };

    if active_blob.size_bm.x <= 0.0
        || active_blob.size_bm.y <= 0.0
        || render_settings.pixels_per_bm <= 0.0
    {
        return;
    }

    let target_width = active_blob.size_bm.x * render_settings.pixels_per_bm;
    let target_height = active_blob.size_bm.y * render_settings.pixels_per_bm;
    let epsilon = 0.5;

    if (window.width() - target_width).abs() > epsilon
        || (window.height() - target_height).abs() > epsilon
    {
        window.resolution.set(target_width, target_height);
    }
}

fn apply_blob_projection_to_camera(
    active_blob: Res<ActiveWindowBlob>,
    mut cameras: Query<&mut OrthographicProjection, With<Camera2d>>,
) {
    if active_blob.size_bm.x <= 0.0 || active_blob.size_bm.y <= 0.0 {
        return;
    }

    for mut projection in &mut cameras {
        projection.scale = 1.0;
        projection.scaling_mode = ScalingMode::Fixed {
            width: active_blob.size_bm.x,
            height: active_blob.size_bm.y,
        };
    }
}

fn update_blob_render_metrics_from_window(
    active_blob: Res<ActiveWindowBlob>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut metrics: ResMut<BlobRenderMetrics>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };

    if active_blob.size_bm.x <= 0.0 || active_blob.size_bm.y <= 0.0 {
        return;
    }

    metrics.pixels_per_bm = Vec2::new(
        window.width() / active_blob.size_bm.x,
        window.height() / active_blob.size_bm.y,
    );
}

fn clamp_tanks_inside_blob_boundary(
    active_blob: Res<ActiveWindowBlob>,
    mut tanks: Query<&mut Transform, With<Tank>>,
) {
    if active_blob.size_bm.x <= 0.0 || active_blob.size_bm.y <= 0.0 {
        return;
    }

    let half_extents_bm = active_blob.size_bm * 0.5;
    let clamp_x = (half_extents_bm.x - TANK_BODY_RADIUS_BM).max(0.0);
    let clamp_y = (half_extents_bm.y - TANK_BODY_RADIUS_BM).max(0.0);

    for mut transform in &mut tanks {
        transform.translation.x = transform.translation.x.clamp(-clamp_x, clamp_x);
        transform.translation.y = transform.translation.y.clamp(-clamp_y, clamp_y);
    }
}

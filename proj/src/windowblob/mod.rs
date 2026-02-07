use bevy::prelude::*;
use bevy::render::camera::{OrthographicProjection, ScalingMode};
use bevy::window::PresentMode;

use crate::tank::{Tank, TANK_BODY_RADIUS_BM};

pub const MAIN_BLOB_SAVE_FILE: &str = "main.blob.json";
pub const MAIN_BLOB_SIZE_BM: Vec2 = Vec2::new(10.0, 10.0);
pub const DEFAULT_PIXELS_PER_BM: f32 = 72.0;
pub const MAIN_BLOB_INSTANCE_ID: u32 = 1;
pub const BASE_WINDOW_TITLE: &str = "Tank Test Window";

pub struct WindowBlobPlugin;

impl Plugin for WindowBlobPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FocusedBlobInstance>()
            .init_resource::<NextBlobInstanceId>()
            .add_systems(
                Update,
                (
                    update_focused_blob_instance,
                    enforce_window_resolution_from_blob_settings,
                    apply_blob_projection_to_camera,
                )
                    .chain(),
            )
            .add_systems(PostUpdate, clamp_tanks_inside_blob_boundary);
    }
}

#[derive(Component, Debug, Clone)]
pub struct BlobWindow {
    pub instance_id: u32,
    pub save_file: String,
    pub size_bm: Vec2,
    pub pixels_per_bm: f32,
}

#[derive(Debug, Clone)]
pub struct WindowBlobPrefab {
    pub save_file: String,
    pub size_bm: Vec2,
    pub pixels_per_bm: f32,
}

impl WindowBlobPrefab {
    pub fn main_blob() -> Self {
        Self {
            save_file: MAIN_BLOB_SAVE_FILE.to_string(),
            size_bm: MAIN_BLOB_SIZE_BM,
            pixels_per_bm: DEFAULT_PIXELS_PER_BM,
        }
    }
}

#[derive(Bundle)]
pub struct WindowBlobWindowBundle {
    pub window: Window,
    pub blob_window: BlobWindow,
}

impl WindowBlobWindowBundle {
    pub fn from_prefab(instance_id: u32, prefab: &WindowBlobPrefab) -> Self {
        let width = prefab.size_bm.x * prefab.pixels_per_bm;
        let height = prefab.size_bm.y * prefab.pixels_per_bm;
        Self {
            window: Window {
                title: blob_window_title(&prefab.save_file),
                resolution: (width, height).into(),
                present_mode: PresentMode::AutoNoVsync,
                ..default()
            },
            blob_window: BlobWindow::from_prefab(instance_id, prefab),
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlobInstanceId(pub u32);

#[derive(Component, Debug, Clone, Copy)]
pub struct BlobRenderLayer(pub usize);

#[derive(Component, Debug, Clone, Copy)]
pub struct BlobCamera {
    pub instance_id: u32,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct FocusedBlobInstance(pub Option<u32>);

#[derive(Resource, Debug, Clone, Copy)]
pub struct NextBlobInstanceId(pub u32);

impl Default for NextBlobInstanceId {
    fn default() -> Self {
        Self(MAIN_BLOB_INSTANCE_ID + 1)
    }
}

impl BlobWindow {
    pub fn from_prefab(instance_id: u32, prefab: &WindowBlobPrefab) -> Self {
        Self {
            instance_id,
            save_file: prefab.save_file.clone(),
            size_bm: prefab.size_bm,
            pixels_per_bm: prefab.pixels_per_bm,
        }
    }
}

impl Default for BlobInstanceId {
    fn default() -> Self {
        Self(MAIN_BLOB_INSTANCE_ID)
    }
}

impl Default for BlobRenderLayer {
    fn default() -> Self {
        Self(blob_render_layer(MAIN_BLOB_INSTANCE_ID))
    }
}

pub fn blob_render_layer(instance_id: u32) -> usize {
    (((instance_id.saturating_sub(1)) % 31) + 1) as usize
}

pub fn blob_window_title(save_file: &str) -> String {
    format!("{BASE_WINDOW_TITLE} [{save_file}]")
}

fn update_focused_blob_instance(
    mut focused_blob: ResMut<FocusedBlobInstance>,
    windows: Query<(&Window, &BlobWindow)>,
) {
    focused_blob.0 = windows
        .iter()
        .find(|(window, _)| window.focused)
        .map(|(_, blob_window)| blob_window.instance_id);
}

fn enforce_window_resolution_from_blob_settings(mut windows: Query<(&BlobWindow, &mut Window)>) {
    for (blob_window, mut window) in &mut windows {
        if blob_window.size_bm.x <= 0.0
            || blob_window.size_bm.y <= 0.0
            || blob_window.pixels_per_bm <= 0.0
        {
            continue;
        }

        let target_width = blob_window.size_bm.x * blob_window.pixels_per_bm;
        let target_height = blob_window.size_bm.y * blob_window.pixels_per_bm;
        let epsilon = 0.5;

        if (window.width() - target_width).abs() > epsilon
            || (window.height() - target_height).abs() > epsilon
        {
            window.resolution.set(target_width, target_height);
        }
    }
}

fn apply_blob_projection_to_camera(
    blob_windows: Query<&BlobWindow>,
    mut cameras: Query<(&BlobCamera, &mut OrthographicProjection), With<Camera2d>>,
) {
    for (blob_camera, mut projection) in &mut cameras {
        let Some(blob_window) = blob_windows
            .iter()
            .find(|window| window.instance_id == blob_camera.instance_id)
        else {
            continue;
        };

        if blob_window.size_bm.x <= 0.0 || blob_window.size_bm.y <= 0.0 {
            continue;
        }

        projection.scale = 1.0;
        projection.scaling_mode = ScalingMode::Fixed {
            width: blob_window.size_bm.x,
            height: blob_window.size_bm.y,
        };
    }
}

fn clamp_tanks_inside_blob_boundary(
    blob_windows: Query<&BlobWindow>,
    mut tanks: Query<(&BlobInstanceId, &mut Transform), With<Tank>>,
) {
    for (blob_instance, mut transform) in &mut tanks {
        let Some(blob_window) = blob_windows
            .iter()
            .find(|window| window.instance_id == blob_instance.0)
        else {
            continue;
        };

        if blob_window.size_bm.x <= 0.0 || blob_window.size_bm.y <= 0.0 {
            continue;
        }

        let half_extents_bm = blob_window.size_bm * 0.5;
        let clamp_x = (half_extents_bm.x - TANK_BODY_RADIUS_BM).max(0.0);
        let clamp_y = (half_extents_bm.y - TANK_BODY_RADIUS_BM).max(0.0);

        transform.translation.x = transform.translation.x.clamp(-clamp_x, clamp_x);
        transform.translation.y = transform.translation.y.clamp(-clamp_y, clamp_y);
    }
}

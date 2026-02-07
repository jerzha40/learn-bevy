use bevy::log::{error, info};
use bevy::math::primitives::Circle;
use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::window::PrimaryWindow;

use crate::save::{
    load_blob_from_disk, save_blob_to_disk, BlobMetaV1, BlobSaveFileV1, LoadBlobRequest,
    PersistentEntityId, PortalSaveV1, SaveConfig, SAVE_SCHEMA_VERSION,
};
use crate::tank::Tank;
use crate::windowblob::{ActiveWindowBlob, BlobRenderSettings, MAIN_BLOB_SAVE_FILE, MAIN_BLOB_SIZE_BM};

pub const PORTAL_INTERACT_DIAMETER_BM: f32 = 2.0;
pub const DEFAULT_PORTAL_RADIUS_BM: f32 = 0.45;
pub const MAIN_PORTAL_ID: u64 = 1001;
pub const NEWLAND_RETURN_PORTAL_ID: u64 = 2001;
pub const NEWLAND_BLOB_SAVE_FILE: &str = "newland.blob.json";

pub struct PortalPlugin;

impl Plugin for PortalPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_default_main_portal_if_empty)
            .add_systems(
                Update,
                (
                    assemble_portal_visuals,
                    update_hovered_portal,
                    update_portal_hover_visuals,
                    activate_hovered_portal,
                    ensure_target_portal_exists_for_added_portals,
                ),
            );
    }
}

#[derive(Component, Debug, Clone)]
pub struct Portal {
    pub target_blob_save_file: String,
    pub target_portal_id: u64,
    pub radius_bm: f32,
}

impl Portal {
    pub fn new(target_blob_save_file: impl Into<String>, target_portal_id: u64) -> Self {
        Self {
            target_blob_save_file: target_blob_save_file.into(),
            target_portal_id,
            radius_bm: DEFAULT_PORTAL_RADIUS_BM,
        }
    }
}

#[derive(Component, Debug)]
pub struct PortalVisualBuilt;

#[derive(Component, Debug)]
pub struct PortalHoverVisual;

#[derive(Component, Debug)]
pub struct PortalHovered;

fn spawn_default_main_portal_if_empty(
    mut commands: Commands,
    active_blob: Res<ActiveWindowBlob>,
    portals: Query<Entity, With<Portal>>,
) {
    if active_blob.save_file != MAIN_BLOB_SAVE_FILE || !portals.is_empty() {
        return;
    }

    commands.spawn((
        PersistentEntityId(MAIN_PORTAL_ID),
        Portal::new(NEWLAND_BLOB_SAVE_FILE, NEWLAND_RETURN_PORTAL_ID),
        SpatialBundle::from_transform(Transform::from_xyz(MAIN_BLOB_SIZE_BM.x * 0.3, 0.0, 0.0)),
    ));
}

fn assemble_portal_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    portals: Query<(Entity, &Portal), (With<Portal>, Without<PortalVisualBuilt>)>,
) {
    for (portal_entity, portal) in &portals {
        let body_entity = commands
            .spawn((
                MaterialMesh2dBundle {
                    mesh: meshes.add(Mesh::from(Circle::new(portal.radius_bm))).into(),
                    material: materials.add(ColorMaterial::from(Color::srgb(0.22, 0.58, 0.95))),
                    transform: Transform::from_xyz(0.0, 0.0, 0.5),
                    ..default()
                },
            ))
            .id();

        let hover_entity = commands
            .spawn((
                PortalHoverVisual,
                SpriteBundle {
                    sprite: Sprite {
                        color: Color::srgba(1.0, 0.95, 0.45, 0.35),
                        custom_size: Some(Vec2::splat(portal.radius_bm * 2.4)),
                        ..default()
                    },
                    transform: Transform::from_xyz(0.0, 0.0, 0.6),
                    visibility: Visibility::Hidden,
                    ..default()
                },
            ))
            .id();

        commands
            .entity(portal_entity)
            .add_child(body_entity)
            .add_child(hover_entity)
            .insert(PortalVisualBuilt);
    }
}

fn update_hovered_portal(
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    portals: Query<(Entity, &GlobalTransform, &Portal, Option<&PortalHovered>), With<Portal>>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Ok((camera, camera_transform)) = cameras.get_single() else {
        return;
    };

    let hovered_entity = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world_2d(camera_transform, cursor))
        .and_then(|cursor_world| {
            let mut nearest: Option<(Entity, f32)> = None;

            for (entity, transform, portal, _) in &portals {
                let portal_position = transform.translation().truncate();
                let delta = cursor_world - portal_position;
                let distance_sq = delta.length_squared();
                if distance_sq > portal.radius_bm * portal.radius_bm {
                    continue;
                }

                match nearest {
                    Some((_, current_distance_sq)) if current_distance_sq <= distance_sq => {}
                    _ => {
                        nearest = Some((entity, distance_sq));
                    }
                }
            }

            nearest.map(|(entity, _)| entity)
        });

    for (entity, _, _, was_hovered) in &portals {
        if Some(entity) == hovered_entity {
            if was_hovered.is_none() {
                commands.entity(entity).insert(PortalHovered);
            }
        } else if was_hovered.is_some() {
            commands.entity(entity).remove::<PortalHovered>();
        }
    }
}

fn update_portal_hover_visuals(
    hovered_portals: Query<(), With<PortalHovered>>,
    mut hover_visuals: Query<(&Parent, &mut Visibility), With<PortalHoverVisual>>,
) {
    for (parent, mut visibility) in &mut hover_visuals {
        if hovered_portals.contains(parent.get()) {
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

fn activate_hovered_portal(
    mouse_button: Res<ButtonInput<MouseButton>>,
    active_blob: Res<ActiveWindowBlob>,
    hovered_portals: Query<(&Portal, &Transform), (With<PortalHovered>, Without<Tank>)>,
    all_portals: Query<(&PersistentEntityId, &Portal, &Transform), (With<Portal>, Without<Tank>)>,
    mut tanks: ParamSet<(
        Query<(Entity, &Transform), With<Tank>>,
        Query<&mut Transform, With<Tank>>,
    )>,
    mut load_blob_requests: EventWriter<LoadBlobRequest>,
) {
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    let Some((hovered_portal, hovered_transform)) = hovered_portals.iter().next() else {
        return;
    };

    let hovered_position = hovered_transform.translation.truncate();
    let activation_radius = PORTAL_INTERACT_DIAMETER_BM * 0.5;
    let mut selected_tank_entity = None;

    for (tank_entity, tank_transform) in tanks.p0().iter() {
        let tank_position = tank_transform.translation.truncate();
        if tank_position.distance_squared(hovered_position) <= activation_radius * activation_radius
        {
            selected_tank_entity = Some(tank_entity);
            break;
        }
    }

    let Some(selected_tank_entity) = selected_tank_entity else {
        return;
    };

    if hovered_portal.target_blob_save_file == active_blob.save_file {
        let target_portal = all_portals
            .iter()
            .find(|(portal_id, _, _)| portal_id.0 == hovered_portal.target_portal_id);

        let Some((_, _, target_transform)) = target_portal else {
            return;
        };

        let target_position = target_transform.translation.truncate();
        let offset = Vec2::X * (activation_radius + 0.1);

        if let Ok(mut tank_transform) = tanks.p1().get_mut(selected_tank_entity) {
            tank_transform.translation.x = target_position.x + offset.x;
            tank_transform.translation.y = target_position.y + offset.y;
        }
        return;
    }

    load_blob_requests.send(LoadBlobRequest {
        target_save_file: hovered_portal.target_blob_save_file.clone(),
        spawn_near_portal_id: Some(hovered_portal.target_portal_id),
    });
}

fn ensure_target_portal_exists_for_added_portals(
    save_config: Res<SaveConfig>,
    active_blob: Res<ActiveWindowBlob>,
    blob_render_settings: Res<BlobRenderSettings>,
    portals: Query<(&PersistentEntityId, &Portal), Added<Portal>>,
) {
    for (portal_id, portal) in &portals {
        match ensure_target_portal_exists(
            &save_config,
            &active_blob,
            &blob_render_settings,
            portal_id.0,
            portal,
        ) {
            Ok(true) => info!(
                "Ensured paired portal {} in {} for source portal {}",
                portal.target_portal_id, portal.target_blob_save_file, portal_id.0
            ),
            Ok(false) => {}
            Err(err) => error!("Failed to ensure paired portal for {}: {}", portal_id.0, err),
        }
    }
}

fn ensure_target_portal_exists(
    save_config: &SaveConfig,
    active_blob: &ActiveWindowBlob,
    blob_render_settings: &BlobRenderSettings,
    source_portal_id: u64,
    source_portal: &Portal,
) -> Result<bool, String> {
    if source_portal.target_blob_save_file == active_blob.save_file {
        return Ok(false);
    }

    let target_path = save_config.save_path_for(&source_portal.target_blob_save_file);

    let mut target_blob = if target_path.exists() {
        let loaded = load_blob_from_disk(&target_path)?;
        if loaded.schema_version != SAVE_SCHEMA_VERSION {
            return Err(format!(
                "unsupported schema_version {} in {}",
                loaded.schema_version,
                target_path.display()
            ));
        }
        loaded
    } else {
        BlobSaveFileV1 {
            schema_version: SAVE_SCHEMA_VERSION,
            blob: BlobMetaV1 {
                save_file: source_portal.target_blob_save_file.clone(),
                size_bm: [active_blob.size_bm.x, active_blob.size_bm.y],
                pixels_per_bm: blob_render_settings.pixels_per_bm,
            },
            tanks: Vec::new(),
            projectiles: Vec::new(),
            portals: Vec::new(),
        }
    };

    let mut changed = false;
    let mut has_target_portal = false;

    for portal in &mut target_blob.portals {
        if portal.id != source_portal.target_portal_id {
            continue;
        }
        has_target_portal = true;

        if portal.target_blob_save_file != active_blob.save_file {
            portal.target_blob_save_file = active_blob.save_file.clone();
            changed = true;
        }
        if portal.target_portal_id != source_portal_id {
            portal.target_portal_id = source_portal_id;
            changed = true;
        }
    }

    if !has_target_portal {
        target_blob.portals.push(PortalSaveV1 {
            id: source_portal.target_portal_id,
            position_bm: [0.0, 0.0],
            target_blob_save_file: active_blob.save_file.clone(),
            target_portal_id: source_portal_id,
            radius_bm: source_portal.radius_bm,
        });
        changed = true;
    }

    if !changed {
        return Ok(false);
    }

    target_blob.portals.sort_by_key(|portal| portal.id);
    save_blob_to_disk(&target_path, &target_blob)?;
    Ok(true)
}

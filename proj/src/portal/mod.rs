use bevy::log::{error, info};
use bevy::math::primitives::Circle;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::MaterialMesh2dBundle;

use crate::inventory::crafting::SelectedMarker;
use crate::inventory::Inventory;
use crate::save::{
    load_blob_from_disk, save_blob_to_disk, BlobMetaV1, BlobSaveFileV1, OpenBlobWindowRequest,
    PersistentEntityId, PortalSaveV1, SaveConfig, TravelerTankState, SAVE_SCHEMA_VERSION,
};
use crate::tank::{FactionId, Tank, TankStats};
use crate::windowblob::{
    blob_render_layer, BlobInstanceId, BlobRenderLayer, BlobWindow, MAIN_BLOB_INSTANCE_ID,
    MAIN_BLOB_SIZE_BM,
};

pub const DEFAULT_PORTAL_INTERACT_DIAMETER_BM: f32 = 2.0;
pub const DEFAULT_PORTAL_RADIUS_BM: f32 = 0.45;
pub const DEFAULT_PORTAL_COLOR_RGBA: [f32; 4] = [0.22, 0.58, 0.95, 1.0];
pub const MAIN_PORTAL_ID: u64 = 1001;
pub const NEWLAND_RETURN_PORTAL_ID: u64 = 2001;
pub const NEWLAND_BLOB_SAVE_FILE: &str = "newland.blob.json";

pub struct PortalPlugin;

impl Plugin for PortalPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PortalInteractRequest>()
            .add_systems(Startup, spawn_default_main_portal_if_empty)
            .add_systems(
                Update,
                (
                    assemble_portal_visuals,
                    update_portal_hover_visuals,
                    handle_portal_interact_requests,
                    ensure_target_portal_exists_for_added_portals,
                ),
            );
    }
}

#[derive(Event, Debug, Clone, Copy)]
pub struct PortalInteractRequest {
    pub portal_entity: Entity,
    pub player_tank_entity: Entity,
}

#[derive(Component, Debug, Clone)]
pub struct Portal {
    pub target_blob_save_file: String,
    pub target_portal_id: u64,
    pub radius_bm: f32,
    pub interact_diameter_bm: f32,
    pub color_rgba: [f32; 4],
}

impl Portal {
    pub fn new(target_blob_save_file: impl Into<String>, target_portal_id: u64) -> Self {
        Self {
            target_blob_save_file: target_blob_save_file.into(),
            target_portal_id,
            radius_bm: DEFAULT_PORTAL_RADIUS_BM,
            interact_diameter_bm: DEFAULT_PORTAL_INTERACT_DIAMETER_BM,
            color_rgba: DEFAULT_PORTAL_COLOR_RGBA,
        }
    }

    fn color(&self) -> Color {
        Color::srgba(
            self.color_rgba[0],
            self.color_rgba[1],
            self.color_rgba[2],
            self.color_rgba[3],
        )
    }
}

#[derive(Component, Debug)]
pub struct PortalVisualBuilt;

#[derive(Component, Debug)]
pub struct PortalHoverVisual;

fn spawn_default_main_portal_if_empty(
    mut commands: Commands,
    portals: Query<&BlobInstanceId, With<Portal>>,
) {
    if portals.iter().any(|blob_instance| blob_instance.0 == MAIN_BLOB_INSTANCE_ID) {
        return;
    }

    commands.spawn((
        PersistentEntityId(MAIN_PORTAL_ID),
        BlobInstanceId(MAIN_BLOB_INSTANCE_ID),
        BlobRenderLayer(blob_render_layer(MAIN_BLOB_INSTANCE_ID)),
        Portal::new(NEWLAND_BLOB_SAVE_FILE, NEWLAND_RETURN_PORTAL_ID),
        SpatialBundle::from_transform(Transform::from_xyz(MAIN_BLOB_SIZE_BM.x * 0.3, 0.0, 0.0)),
    ));
}

fn assemble_portal_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    portals: Query<
        (Entity, &Portal, &BlobInstanceId, &BlobRenderLayer),
        (With<Portal>, Without<PortalVisualBuilt>),
    >,
) {
    for (portal_entity, portal, blob_instance, blob_layer) in &portals {
        let body_entity = commands
            .spawn((
                *blob_instance,
                *blob_layer,
                RenderLayers::layer(blob_layer.0),
                MaterialMesh2dBundle {
                    mesh: meshes.add(Mesh::from(Circle::new(portal.radius_bm))).into(),
                    material: materials.add(ColorMaterial::from(portal.color())),
                    transform: Transform::from_xyz(0.0, 0.0, 0.5),
                    ..default()
                },
            ))
            .id();

        let hover_entity = commands
            .spawn((
                PortalHoverVisual,
                *blob_instance,
                *blob_layer,
                RenderLayers::layer(blob_layer.0),
                SpriteBundle {
                    sprite: Sprite {
                        color: Color::srgba(
                            portal.color_rgba[0].clamp(0.0, 1.0),
                            portal.color_rgba[1].clamp(0.0, 1.0),
                            portal.color_rgba[2].clamp(0.0, 1.0),
                            0.35,
                        ),
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

fn update_portal_hover_visuals(
    selected_portals: Query<(), (With<Portal>, With<SelectedMarker>)>,
    mut hover_visuals: Query<(&Parent, &mut Visibility), With<PortalHoverVisual>>,
) {
    for (parent, mut visibility) in &mut hover_visuals {
        if selected_portals.contains(parent.get()) {
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

fn handle_portal_interact_requests(
    mut portal_interact_requests: EventReader<PortalInteractRequest>,
    blob_windows: Query<&BlobWindow>,
    portals: Query<(Entity, &Portal, &Transform, &BlobInstanceId), (With<Portal>, Without<Tank>)>,
    all_portals: Query<(&PersistentEntityId, &Portal, &Transform, &BlobInstanceId), (With<Portal>, Without<Tank>)>,
    mut tanks: ParamSet<(
        Query<(Entity, &Transform, &TankStats, &FactionId, &BlobInstanceId, &Inventory), With<Tank>>,
        Query<&mut Transform, With<Tank>>,
    )>,
    mut open_blob_window_requests: EventWriter<OpenBlobWindowRequest>,
) {
    for request in portal_interact_requests.read() {
        let Ok((_, portal, portal_transform, portal_blob)) = portals.get(request.portal_entity) else {
            continue;
        };

        let Some((tank_entity, tank_blob_id, tank_position, traveler_tank_state)) = (|| {
            let tank_query = tanks.p0();
            let Ok((tank_entity, tank_transform, tank_stats, tank_faction, tank_blob, tank_inventory)) =
                tank_query.get(request.player_tank_entity)
            else {
                return None;
            };

            let tank_position = tank_transform.translation.truncate();
            let (_, _, rotation_rad) = tank_transform.rotation.to_euler(EulerRot::XYZ);

            Some((
                tank_entity,
                tank_blob.0,
                tank_position,
                TravelerTankState {
                    position_bm: [tank_position.x, tank_position.y],
                    rotation_rad,
                    hp: tank_stats.hp,
                    move_speed: tank_stats.move_speed,
                    turn_speed: tank_stats.turn_speed,
                    faction_id: tank_faction.0,
                    inventory: tank_inventory.clone(),
                },
            ))
        })() else {
            continue;
        };

        if tank_entity != request.player_tank_entity || tank_blob_id != portal_blob.0 {
            continue;
        }

        let activation_radius = portal.interact_diameter_bm * 0.5;
        let portal_position = portal_transform.translation.truncate();
        if tank_position.distance_squared(portal_position) > activation_radius * activation_radius {
            continue;
        }

        let Some(source_blob_window) = blob_windows
            .iter()
            .find(|window| window.instance_id == portal_blob.0)
        else {
            continue;
        };

        if portal.target_blob_save_file == source_blob_window.save_file {
            let target_portal = all_portals.iter().find(|(portal_id, _, _, blob_instance)| {
                blob_instance.0 == portal_blob.0 && portal_id.0 == portal.target_portal_id
            });

            let Some((_, _, target_transform, _)) = target_portal else {
                continue;
            };

            let target_position = target_transform.translation.truncate();
            let offset = Vec2::X * (activation_radius + 0.1);

            if let Ok(mut tank_transform) = tanks.p1().get_mut(tank_entity) {
                tank_transform.translation.x = target_position.x + offset.x;
                tank_transform.translation.y = target_position.y + offset.y;
            }
            continue;
        }

        open_blob_window_requests.send(OpenBlobWindowRequest {
            source_blob_instance_id: portal_blob.0,
            target_save_file: portal.target_blob_save_file.clone(),
            spawn_near_portal_id: Some(portal.target_portal_id),
            traveler_tank: Some(traveler_tank_state),
            source_tank_entity: Some(tank_entity),
        });
    }
}

fn ensure_target_portal_exists_for_added_portals(
    save_config: Res<SaveConfig>,
    blob_windows: Query<&BlobWindow>,
    portals: Query<(&PersistentEntityId, &Portal, &BlobInstanceId), Added<Portal>>,
) {
    for (portal_id, portal, blob_instance) in &portals {
        let Some(source_blob_window) = blob_windows
            .iter()
            .find(|window| window.instance_id == blob_instance.0)
        else {
            continue;
        };

        match ensure_target_portal_exists(&save_config, source_blob_window, portal_id.0, portal) {
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
    source_blob_window: &BlobWindow,
    source_portal_id: u64,
    source_portal: &Portal,
) -> Result<bool, String> {
    if source_portal.target_blob_save_file == source_blob_window.save_file {
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
                size_bm: [source_blob_window.size_bm.x, source_blob_window.size_bm.y],
                pixels_per_bm: source_blob_window.pixels_per_bm,
            },
            tanks: Vec::new(),
            projectiles: Vec::new(),
            portals: Vec::new(),
            items: Vec::new(),
        }
    };

    let mut changed = false;
    let mut has_target_portal = false;

    for portal in &mut target_blob.portals {
        if portal.id != source_portal.target_portal_id {
            continue;
        }
        has_target_portal = true;

        if portal.target_blob_save_file != source_blob_window.save_file {
            portal.target_blob_save_file = source_blob_window.save_file.clone();
            changed = true;
        }
        if portal.target_portal_id != source_portal_id {
            portal.target_portal_id = source_portal_id;
            changed = true;
        }
        if (portal.radius_bm - source_portal.radius_bm).abs() > f32::EPSILON {
            portal.radius_bm = source_portal.radius_bm;
            changed = true;
        }
        if (portal.interact_diameter_bm - source_portal.interact_diameter_bm).abs()
            > f32::EPSILON
        {
            portal.interact_diameter_bm = source_portal.interact_diameter_bm;
            changed = true;
        }
        if portal.color_rgba != source_portal.color_rgba {
            portal.color_rgba = source_portal.color_rgba;
            changed = true;
        }
    }

    if !has_target_portal {
        target_blob.portals.push(PortalSaveV1 {
            id: source_portal.target_portal_id,
            position_bm: [0.0, 0.0],
            target_blob_save_file: source_blob_window.save_file.clone(),
            target_portal_id: source_portal_id,
            radius_bm: source_portal.radius_bm,
            interact_diameter_bm: source_portal.interact_diameter_bm,
            color_rgba: source_portal.color_rgba,
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

use std::path::{Path, PathBuf};
use std::{ffi::OsString, fs};

use bevy::app::AppExit;
use bevy::log::{error, info};
use bevy::prelude::*;
use bevy::render::camera::RenderTarget;
use bevy::render::view::RenderLayers;
use bevy::window::{PrimaryWindow, WindowCloseRequested, WindowRef};
use serde::{Deserialize, Serialize};

use crate::portal::{Portal, DEFAULT_PORTAL_COLOR_RGBA, DEFAULT_PORTAL_INTERACT_DIAMETER_BM};
use crate::projectile::baseprojectile::BaseProjectile;
use crate::projectile::Projectile;
use crate::tank::{FactionId, Tank, TankStats, PLAYER_FACTION_ID};
use crate::windowblob::{
    blob_render_layer, BlobCamera, BlobInstanceId, BlobRenderLayer, BlobWindow, NextBlobInstanceId,
    WindowBlobPrefab, WindowBlobWindowBundle, MAIN_BLOB_INSTANCE_ID, MAIN_BLOB_SAVE_FILE,
};

pub const SAVE_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_AUTOSAVE_SECONDS: f32 = 5.0;

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<OpenBlobWindowRequest>()
            .init_resource::<SaveConfig>()
            .init_resource::<AutosaveTimer>()
            .init_resource::<NextPersistentEntityId>()
            .init_resource::<InitialSavePending>()
            .init_resource::<LastSaveError>()
            .add_systems(PreStartup, load_main_blob_or_bootstrap)
            .add_systems(Update, assign_persistent_ids)
            .add_systems(Update, perform_initial_save_if_pending.after(assign_persistent_ids))
            .add_systems(Update, autosave_blob_state.after(assign_persistent_ids))
            .add_systems(
                Update,
                handle_open_blob_window_requests.after(assign_persistent_ids),
            )
            .add_systems(Update, save_on_window_close_requested.after(assign_persistent_ids));
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TravelerTankState {
    pub position_bm: [f32; 2],
    pub rotation_rad: f32,
    pub hp: f32,
    pub move_speed: f32,
    pub turn_speed: f32,
    pub faction_id: u8,
}

#[derive(Event, Debug, Clone)]
pub struct OpenBlobWindowRequest {
    pub source_blob_instance_id: u32,
    pub target_save_file: String,
    pub spawn_near_portal_id: Option<u64>,
    pub traveler_tank: Option<TravelerTankState>,
    pub source_tank_entity: Option<Entity>,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PersistentEntityId(pub u64);

#[derive(Resource, Debug, Clone)]
pub struct SaveConfig {
    pub save_dir: PathBuf,
    pub autosave_seconds: f32,
}

impl Default for SaveConfig {
    fn default() -> Self {
        Self {
            save_dir: PathBuf::from("saves"),
            autosave_seconds: DEFAULT_AUTOSAVE_SECONDS,
        }
    }
}

impl SaveConfig {
    pub fn save_path_for(&self, file_name: &str) -> PathBuf {
        self.save_dir.join(file_name)
    }
}

#[derive(Resource, Debug, Clone)]
pub struct AutosaveTimer(pub Timer);

impl Default for AutosaveTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(
            DEFAULT_AUTOSAVE_SECONDS,
            TimerMode::Repeating,
        ))
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct NextPersistentEntityId(pub u64);

impl Default for NextPersistentEntityId {
    fn default() -> Self {
        Self(1)
    }
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct InitialSavePending(pub bool);

#[derive(Resource, Debug, Default, Clone)]
pub struct LastSaveError(pub Option<String>);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlobSaveFileV1 {
    pub schema_version: u32,
    pub blob: BlobMetaV1,
    pub tanks: Vec<TankSaveV1>,
    pub projectiles: Vec<ProjectileSaveV1>,
    #[serde(default)]
    pub portals: Vec<PortalSaveV1>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlobMetaV1 {
    pub save_file: String,
    pub size_bm: [f32; 2],
    pub pixels_per_bm: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TankSaveV1 {
    pub id: u64,
    pub position_bm: [f32; 2],
    pub rotation_rad: f32,
    pub hp: f32,
    pub move_speed: f32,
    pub turn_speed: f32,
    pub faction_id: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectileSaveV1 {
    pub id: u64,
    pub position_bm: [f32; 2],
    pub direction: [f32; 2],
    pub speed: f32,
    pub remaining_distance: f32,
    pub damage: f32,
    pub radius: f32,
    pub target_faction_id: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PortalSaveV1 {
    pub id: u64,
    pub position_bm: [f32; 2],
    pub target_blob_save_file: String,
    pub target_portal_id: u64,
    pub radius_bm: f32,
    #[serde(default = "default_portal_interact_diameter_bm")]
    pub interact_diameter_bm: f32,
    #[serde(default = "default_portal_color_rgba")]
    pub color_rgba: [f32; 4],
}

fn default_portal_interact_diameter_bm() -> f32 {
    DEFAULT_PORTAL_INTERACT_DIAMETER_BM
}

fn default_portal_color_rgba() -> [f32; 4] {
    DEFAULT_PORTAL_COLOR_RGBA
}

fn load_main_blob_or_bootstrap(
    mut commands: Commands,
    save_config: Res<SaveConfig>,
    mut autosave_timer: ResMut<AutosaveTimer>,
    mut initial_save_pending: ResMut<InitialSavePending>,
    mut next_id: ResMut<NextPersistentEntityId>,
    mut next_blob_instance_id: ResMut<NextBlobInstanceId>,
    primary_window_entities: Query<Entity, With<PrimaryWindow>>,
) {
    autosave_timer.0 = Timer::from_seconds(save_config.autosave_seconds, TimerMode::Repeating);

    let Ok(primary_window_entity) = primary_window_entities.get_single() else {
        return;
    };

    let save_path = save_config.save_path_for(MAIN_BLOB_SAVE_FILE);

    let (loaded_blob, should_bootstrap) = if save_path.exists() {
        let parsed_save = load_blob_from_disk(&save_path).unwrap_or_else(|err| {
            panic!("Failed to load save file {}: {err}", save_path.display());
        });

        if parsed_save.schema_version != SAVE_SCHEMA_VERSION {
            panic!(
                "Unsupported save schema_version {} in {}. Expected {}.",
                parsed_save.schema_version,
                save_path.display(),
                SAVE_SCHEMA_VERSION
            );
        }

        (parsed_save, false)
    } else {
        let main_prefab = WindowBlobPrefab::main_blob();
        (
            BlobSaveFileV1 {
                schema_version: SAVE_SCHEMA_VERSION,
                blob: BlobMetaV1 {
                    save_file: main_prefab.save_file.clone(),
                    size_bm: [main_prefab.size_bm.x, main_prefab.size_bm.y],
                    pixels_per_bm: main_prefab.pixels_per_bm,
                },
                tanks: Vec::new(),
                projectiles: Vec::new(),
                portals: Vec::new(),
            },
            true,
        )
    };

    let main_blob_prefab = WindowBlobPrefab {
        save_file: loaded_blob.blob.save_file.clone(),
        size_bm: Vec2::new(loaded_blob.blob.size_bm[0], loaded_blob.blob.size_bm[1]),
        pixels_per_bm: loaded_blob.blob.pixels_per_bm,
    };
    commands
        .entity(primary_window_entity)
        .insert(BlobWindow::from_prefab(MAIN_BLOB_INSTANCE_ID, &main_blob_prefab));

    spawn_camera_for_blob_window(
        &mut commands,
        primary_window_entity,
        MAIN_BLOB_INSTANCE_ID,
    );

    let max_id = spawn_blob_entities(
        &mut commands,
        MAIN_BLOB_INSTANCE_ID,
        &loaded_blob,
        None,
        None,
    );
    next_id.0 = max_id.saturating_add(1).max(1);
    next_blob_instance_id.0 = next_blob_instance_id.0.max(MAIN_BLOB_INSTANCE_ID + 1);

    initial_save_pending.0 = should_bootstrap;

    if should_bootstrap {
        info!(
            "Save file {} does not exist. Bootstrapping default main blob.",
            save_path.display()
        );
    } else {
        info!("Loaded save from {}", save_path.display());
    }
}

fn assign_persistent_ids(
    mut commands: Commands,
    mut next_id: ResMut<NextPersistentEntityId>,
    entities: Query<
        Entity,
        (
            Without<PersistentEntityId>,
            With<BlobInstanceId>,
            Or<(With<Tank>, With<Projectile>, With<Portal>)>,
        ),
    >,
) {
    for entity in &entities {
        let id = next_id.0;
        next_id.0 = next_id.0.saturating_add(1).max(1);
        commands.entity(entity).insert(PersistentEntityId(id));
    }
}

fn perform_initial_save_if_pending(
    mut initial_save_pending: ResMut<InitialSavePending>,
    save_config: Res<SaveConfig>,
    blob_windows: Query<&BlobWindow>,
    tanks: Query<
        (Entity, &BlobInstanceId, &PersistentEntityId, &Transform, &TankStats, &FactionId),
        With<Tank>,
    >,
    projectiles: Query<(&BlobInstanceId, &PersistentEntityId, &Transform, &BaseProjectile), With<Projectile>>,
    portals: Query<(&BlobInstanceId, &PersistentEntityId, &Transform, &Portal)>,
    mut last_save_error: ResMut<LastSaveError>,
) {
    if !initial_save_pending.0 {
        return;
    }

    match save_all_open_blobs(&save_config, &blob_windows, &tanks, &projectiles, &portals) {
        Ok(()) => {
            initial_save_pending.0 = false;
            last_save_error.0 = None;
            info!("Initial save created");
        }
        Err(err) => {
            last_save_error.0 = Some(err.clone());
            panic!("Failed to write initial save: {err}");
        }
    }
}

fn autosave_blob_state(
    time: Res<Time>,
    mut autosave_timer: ResMut<AutosaveTimer>,
    save_config: Res<SaveConfig>,
    blob_windows: Query<&BlobWindow>,
    tanks: Query<
        (Entity, &BlobInstanceId, &PersistentEntityId, &Transform, &TankStats, &FactionId),
        With<Tank>,
    >,
    projectiles: Query<(&BlobInstanceId, &PersistentEntityId, &Transform, &BaseProjectile), With<Projectile>>,
    portals: Query<(&BlobInstanceId, &PersistentEntityId, &Transform, &Portal)>,
    mut last_save_error: ResMut<LastSaveError>,
) {
    if !autosave_timer.0.tick(time.delta()).just_finished() {
        return;
    }

    match save_all_open_blobs(&save_config, &blob_windows, &tanks, &projectiles, &portals) {
        Ok(()) => {
            last_save_error.0 = None;
        }
        Err(err) => {
            error!("Autosave failed: {err}");
            last_save_error.0 = Some(err);
        }
    }
}

fn handle_open_blob_window_requests(
    mut commands: Commands,
    mut open_blob_window_requests: EventReader<OpenBlobWindowRequest>,
    save_config: Res<SaveConfig>,
    mut next_blob_instance_id: ResMut<NextBlobInstanceId>,
    mut next_id: ResMut<NextPersistentEntityId>,
    blob_windows: Query<(Entity, &BlobWindow)>,
    mut windows: Query<&mut Window>,
    tanks: Query<
        (Entity, &BlobInstanceId, &PersistentEntityId, &Transform, &TankStats, &FactionId),
        With<Tank>,
    >,
    projectiles: Query<(&BlobInstanceId, &PersistentEntityId, &Transform, &BaseProjectile), With<Projectile>>,
    portals: Query<(&BlobInstanceId, &PersistentEntityId, &Transform, &Portal)>,
    mut last_save_error: ResMut<LastSaveError>,
) {
    for request in open_blob_window_requests.read() {
        let Some((_, source_blob_window)) = blob_windows
            .iter()
            .find(|(_, window)| window.instance_id == request.source_blob_instance_id)
        else {
            let err = format!(
                "source blob instance {} is not open",
                request.source_blob_instance_id
            );
            error!("{}", err);
            last_save_error.0 = Some(err);
            continue;
        };

        let existing_target_window = blob_windows
            .iter()
            .find(|(_, window)| window.save_file == request.target_save_file)
            .map(|(entity, window)| (entity, window.clone()));

        let mut loaded_blob: Option<BlobSaveFileV1> = None;
        if existing_target_window.is_none() {
            let target_path = save_config.save_path_for(&request.target_save_file);
            if !target_path.exists() {
                let err = format!("target blob save {} does not exist", target_path.display());
                error!("{}", err);
                last_save_error.0 = Some(err);
                continue;
            }

            let parsed_blob = match load_blob_from_disk(&target_path) {
                Ok(loaded) => loaded,
                Err(err) => {
                    error!("Failed to load target blob {}: {}", target_path.display(), err);
                    last_save_error.0 = Some(err);
                    continue;
                }
            };

            if parsed_blob.schema_version != SAVE_SCHEMA_VERSION {
                let err = format!(
                    "unsupported schema_version {} in {}",
                    parsed_blob.schema_version,
                    target_path.display()
                );
                error!("{}", err);
                last_save_error.0 = Some(err);
                continue;
            }

            loaded_blob = Some(parsed_blob);
        }

        if let Err(err) = save_blob_instance_to_disk(
            &save_config,
            source_blob_window,
            &tanks,
            &projectiles,
            &portals,
            request.source_tank_entity,
        ) {
            error!("Failed to save source blob before transfer: {}", err);
            last_save_error.0 = Some(err);
            continue;
        }

        if let Some(source_tank_entity) = request.source_tank_entity {
            commands.entity(source_tank_entity).despawn_recursive();
        }

        if let Some((target_window_entity, target_window_blob)) = existing_target_window {
            if let Some(traveler_tank) = request.traveler_tank {
                let traveler_id = move_or_spawn_traveler_tank_in_open_blob(
                    &mut commands,
                    target_window_blob.instance_id,
                    request.spawn_near_portal_id,
                    traveler_tank,
                    &mut next_id,
                    &tanks,
                    &portals,
                );
                next_id.0 = next_id.0.max(traveler_id.saturating_add(1).max(1));
            }

            if let Ok(mut target_window) = windows.get_mut(target_window_entity) {
                target_window.visible = true;
                target_window.set_minimized(false);
                target_window.focused = true;
            }

            last_save_error.0 = None;
            info!("Reused open blob window for {}", request.target_save_file);
            continue;
        }

        let Some(loaded_blob) = loaded_blob else {
            let err = format!(
                "target blob {} load state missing unexpectedly",
                request.target_save_file
            );
            error!("{}", err);
            last_save_error.0 = Some(err);
            continue;
        };

        let blob_instance_id = next_blob_instance_id.0;
        next_blob_instance_id.0 = next_blob_instance_id.0.saturating_add(1);

        let blob_prefab = WindowBlobPrefab {
            save_file: loaded_blob.blob.save_file.clone(),
            size_bm: Vec2::new(loaded_blob.blob.size_bm[0], loaded_blob.blob.size_bm[1]),
            pixels_per_bm: loaded_blob.blob.pixels_per_bm,
        };
        let window_entity = commands
            .spawn(WindowBlobWindowBundle::from_prefab(blob_instance_id, &blob_prefab))
            .id();

        spawn_camera_for_blob_window(&mut commands, window_entity, blob_instance_id);

        let max_id = spawn_blob_entities(
            &mut commands,
            blob_instance_id,
            &loaded_blob,
            request.spawn_near_portal_id,
            request.traveler_tank,
        );
        next_id.0 = next_id.0.max(max_id.saturating_add(1).max(1));

        last_save_error.0 = None;
        info!("Opened blob window for {}", request.target_save_file);
    }
}

fn save_on_window_close_requested(
    mut commands: Commands,
    mut close_requests: EventReader<WindowCloseRequested>,
    mut app_exit: EventWriter<AppExit>,
    save_config: Res<SaveConfig>,
    blob_windows: Query<(Entity, &BlobWindow)>,
    blob_cameras: Query<(Entity, &BlobCamera)>,
    blob_entities: Query<Entity, (With<BlobInstanceId>, Or<(With<Tank>, With<Projectile>, With<Portal>)>)>,
    blob_entity_instances: Query<&BlobInstanceId>,
    tanks: Query<
        (Entity, &BlobInstanceId, &PersistentEntityId, &Transform, &TankStats, &FactionId),
        With<Tank>,
    >,
    projectiles: Query<(&BlobInstanceId, &PersistentEntityId, &Transform, &BaseProjectile), With<Projectile>>,
    portals: Query<(&BlobInstanceId, &PersistentEntityId, &Transform, &Portal)>,
    mut last_save_error: ResMut<LastSaveError>,
) {
    let mut successful_closes = 0usize;
    let open_window_count = blob_windows.iter().count();

    for close_request in close_requests.read() {
        let Some((window_entity, blob_window)) = blob_windows
            .iter()
            .find(|(entity, _)| *entity == close_request.window)
        else {
            continue;
        };

        if let Err(err) = save_blob_instance_to_disk(
            &save_config,
            blob_window,
            &tanks,
            &projectiles,
            &portals,
            None,
        ) {
            error!("Save on close failed. Blocking close: {}", err);
            last_save_error.0 = Some(err);
            continue;
        }

        for (camera_entity, blob_camera) in &blob_cameras {
            if blob_camera.instance_id == blob_window.instance_id {
                commands.entity(camera_entity).despawn_recursive();
            }
        }

        for entity in &blob_entities {
            let Ok(blob_instance) = blob_entity_instances.get(entity) else {
                continue;
            };
            if blob_instance.0 == blob_window.instance_id {
                commands.entity(entity).despawn_recursive();
            }
        }

        commands.entity(window_entity).despawn_recursive();
        successful_closes += 1;
        last_save_error.0 = None;
    }

    if successful_closes > 0 && successful_closes >= open_window_count {
        app_exit.send(AppExit::Success);
    }
}

fn move_or_spawn_traveler_tank_in_open_blob(
    commands: &mut Commands,
    target_blob_instance_id: u32,
    spawn_near_portal_id: Option<u64>,
    traveler_tank: TravelerTankState,
    next_id: &mut ResMut<NextPersistentEntityId>,
    tanks: &Query<
        (Entity, &BlobInstanceId, &PersistentEntityId, &Transform, &TankStats, &FactionId),
        With<Tank>,
    >,
    portals: &Query<(&BlobInstanceId, &PersistentEntityId, &Transform, &Portal)>,
) -> u64 {
    let target_position = resolve_spawn_position_for_blob(
        target_blob_instance_id,
        spawn_near_portal_id,
        portals,
    )
    .unwrap_or(Vec2::new(
        traveler_tank.position_bm[0],
        traveler_tank.position_bm[1],
    ));

    if let Some((existing_tank_entity, _, existing_tank_id, _, _, _)) = tanks.iter().find(
        |(_, tank_blob, _, _, _, tank_faction)| {
            tank_blob.0 == target_blob_instance_id && tank_faction.0 == traveler_tank.faction_id
        },
    ) {
        commands.entity(existing_tank_entity).insert((
            TankStats {
                hp: traveler_tank.hp,
                move_speed: traveler_tank.move_speed,
                turn_speed: traveler_tank.turn_speed,
            },
            Transform {
                translation: Vec3::new(target_position.x, target_position.y, 0.0),
                rotation: Quat::from_rotation_z(traveler_tank.rotation_rad),
                ..default()
            },
        ));
        return existing_tank_id.0;
    }

    let traveler_id = next_id.0;
    next_id.0 = next_id.0.saturating_add(1).max(1);

    commands.spawn((
        PersistentEntityId(traveler_id),
        BlobInstanceId(target_blob_instance_id),
        BlobRenderLayer(blob_render_layer(target_blob_instance_id)),
        Tank,
        FactionId(traveler_tank.faction_id),
        TankStats {
            hp: traveler_tank.hp,
            move_speed: traveler_tank.move_speed,
            turn_speed: traveler_tank.turn_speed,
        },
        SpatialBundle::from_transform(Transform {
            translation: Vec3::new(target_position.x, target_position.y, 0.0),
            rotation: Quat::from_rotation_z(traveler_tank.rotation_rad),
            ..default()
        }),
    ));

    traveler_id
}

fn resolve_spawn_position_for_blob(
    target_blob_instance_id: u32,
    spawn_near_portal_id: Option<u64>,
    portals: &Query<(&BlobInstanceId, &PersistentEntityId, &Transform, &Portal)>,
) -> Option<Vec2> {
    let portal_id = spawn_near_portal_id?;
    portals
        .iter()
        .find(|(blob_instance, persistent_id, _, _)| {
            blob_instance.0 == target_blob_instance_id && persistent_id.0 == portal_id
        })
        .map(|(_, _, portal_transform, _)| portal_transform.translation.truncate() + Vec2::X * 1.1)
}

fn spawn_camera_for_blob_window(commands: &mut Commands, window_entity: Entity, blob_instance_id: u32) {
    let render_layer = blob_render_layer(blob_instance_id);

    commands.spawn((
        Camera2dBundle {
            camera: Camera {
                target: RenderTarget::Window(WindowRef::Entity(window_entity)),
                ..default()
            },
            ..default()
        },
        BlobCamera {
            instance_id: blob_instance_id,
        },
        RenderLayers::layer(render_layer),
    ));
}

fn spawn_blob_entities(
    commands: &mut Commands,
    blob_instance_id: u32,
    loaded_blob: &BlobSaveFileV1,
    spawn_near_portal_id: Option<u64>,
    traveler_tank: Option<TravelerTankState>,
) -> u64 {
    let blob_layer = BlobRenderLayer(blob_render_layer(blob_instance_id));
    let blob_instance = BlobInstanceId(blob_instance_id);

    let spawn_position_override = spawn_near_portal_id.and_then(|portal_id| {
        loaded_blob
            .portals
            .iter()
            .find(|portal| portal.id == portal_id)
            .map(|portal| Vec2::new(portal.position_bm[0], portal.position_bm[1]) + Vec2::X * 1.1)
    });

    let mut max_id: u64 = 0;
    let mut traveler_placed = false;

    for tank in &loaded_blob.tanks {
        max_id = max_id.max(tank.id);

        let mut position = Vec2::new(tank.position_bm[0], tank.position_bm[1]);
        let mut rotation_rad = tank.rotation_rad;
        let mut hp = tank.hp;
        let mut move_speed = tank.move_speed;
        let mut turn_speed = tank.turn_speed;
        let mut faction_id = tank.faction_id;

        if !traveler_placed && tank.faction_id == PLAYER_FACTION_ID {
            if let Some(traveler) = traveler_tank {
                position = spawn_position_override
                    .unwrap_or(Vec2::new(traveler.position_bm[0], traveler.position_bm[1]));
                rotation_rad = traveler.rotation_rad;
                hp = traveler.hp;
                move_speed = traveler.move_speed;
                turn_speed = traveler.turn_speed;
                faction_id = traveler.faction_id;
                traveler_placed = true;
            }
        }

        commands.spawn((
            PersistentEntityId(tank.id),
            blob_instance,
            blob_layer,
            Tank,
            FactionId(faction_id),
            TankStats {
                hp,
                move_speed,
                turn_speed,
            },
            SpatialBundle::from_transform(Transform {
                translation: Vec3::new(position.x, position.y, 0.0),
                rotation: Quat::from_rotation_z(rotation_rad),
                ..default()
            }),
        ));
    }

    if let Some(traveler) = traveler_tank {
        if !traveler_placed {
            let position =
                spawn_position_override.unwrap_or(Vec2::new(traveler.position_bm[0], traveler.position_bm[1]));
            let tank_id = max_id.saturating_add(1).max(1);
            max_id = max_id.max(tank_id);

            commands.spawn((
                PersistentEntityId(tank_id),
                blob_instance,
                blob_layer,
                Tank,
                FactionId(traveler.faction_id),
                TankStats {
                    hp: traveler.hp,
                    move_speed: traveler.move_speed,
                    turn_speed: traveler.turn_speed,
                },
                SpatialBundle::from_transform(Transform {
                    translation: Vec3::new(position.x, position.y, 0.0),
                    rotation: Quat::from_rotation_z(traveler.rotation_rad),
                    ..default()
                }),
            ));
        }
    }

    for projectile in &loaded_blob.projectiles {
        max_id = max_id.max(projectile.id);
        commands.spawn((
            PersistentEntityId(projectile.id),
            blob_instance,
            blob_layer,
            Projectile,
            BaseProjectile {
                speed: projectile.speed,
                remaining_distance: projectile.remaining_distance,
                damage: projectile.damage,
                radius: projectile.radius,
                direction: Vec2::new(projectile.direction[0], projectile.direction[1]),
                target_faction_id: projectile.target_faction_id,
            },
            SpatialBundle::from_transform(Transform::from_xyz(
                projectile.position_bm[0],
                projectile.position_bm[1],
                4.0,
            )),
        ));
    }

    for portal in &loaded_blob.portals {
        max_id = max_id.max(portal.id);
        commands.spawn((
            PersistentEntityId(portal.id),
            blob_instance,
            blob_layer,
            Portal {
                target_blob_save_file: portal.target_blob_save_file.clone(),
                target_portal_id: portal.target_portal_id,
                radius_bm: portal.radius_bm,
                interact_diameter_bm: portal.interact_diameter_bm,
                color_rgba: portal.color_rgba,
            },
            SpatialBundle::from_transform(Transform::from_xyz(
                portal.position_bm[0],
                portal.position_bm[1],
                0.0,
            )),
        ));
    }

    max_id
}

fn save_all_open_blobs(
    save_config: &SaveConfig,
    blob_windows: &Query<&BlobWindow>,
    tanks: &Query<
        (Entity, &BlobInstanceId, &PersistentEntityId, &Transform, &TankStats, &FactionId),
        With<Tank>,
    >,
    projectiles: &Query<(&BlobInstanceId, &PersistentEntityId, &Transform, &BaseProjectile), With<Projectile>>,
    portals: &Query<(&BlobInstanceId, &PersistentEntityId, &Transform, &Portal)>,
) -> Result<(), String> {
    for blob_window in blob_windows.iter() {
        save_blob_instance_to_disk(
            save_config,
            blob_window,
            tanks,
            projectiles,
            portals,
            None,
        )?;
    }

    Ok(())
}

fn save_blob_instance_to_disk(
    save_config: &SaveConfig,
    blob_window: &BlobWindow,
    tanks: &Query<
        (Entity, &BlobInstanceId, &PersistentEntityId, &Transform, &TankStats, &FactionId),
        With<Tank>,
    >,
    projectiles: &Query<(&BlobInstanceId, &PersistentEntityId, &Transform, &BaseProjectile), With<Projectile>>,
    portals: &Query<(&BlobInstanceId, &PersistentEntityId, &Transform, &Portal)>,
    skip_tank_entity: Option<Entity>,
) -> Result<(), String> {
    let mut saved_tanks = Vec::new();
    for (entity, tank_blob, id, transform, stats, faction) in tanks.iter() {
        if tank_blob.0 != blob_window.instance_id || Some(entity) == skip_tank_entity {
            continue;
        }

        let (_, _, rotation_rad) = transform.rotation.to_euler(EulerRot::XYZ);
        saved_tanks.push(TankSaveV1 {
            id: id.0,
            position_bm: [transform.translation.x, transform.translation.y],
            rotation_rad,
            hp: stats.hp,
            move_speed: stats.move_speed,
            turn_speed: stats.turn_speed,
            faction_id: faction.0,
        });
    }
    saved_tanks.sort_by_key(|tank| tank.id);

    let mut saved_projectiles = Vec::new();
    for (projectile_blob, id, transform, projectile) in projectiles.iter() {
        if projectile_blob.0 != blob_window.instance_id {
            continue;
        }

        saved_projectiles.push(ProjectileSaveV1 {
            id: id.0,
            position_bm: [transform.translation.x, transform.translation.y],
            direction: [projectile.direction.x, projectile.direction.y],
            speed: projectile.speed,
            remaining_distance: projectile.remaining_distance,
            damage: projectile.damage,
            radius: projectile.radius,
            target_faction_id: projectile.target_faction_id,
        });
    }
    saved_projectiles.sort_by_key(|projectile| projectile.id);

    let mut saved_portals = Vec::new();
    for (portal_blob, id, transform, portal) in portals.iter() {
        if portal_blob.0 != blob_window.instance_id {
            continue;
        }

        saved_portals.push(PortalSaveV1 {
            id: id.0,
            position_bm: [transform.translation.x, transform.translation.y],
            target_blob_save_file: portal.target_blob_save_file.clone(),
            target_portal_id: portal.target_portal_id,
            radius_bm: portal.radius_bm,
            interact_diameter_bm: portal.interact_diameter_bm,
            color_rgba: portal.color_rgba,
        });
    }
    saved_portals.sort_by_key(|portal| portal.id);

    let save_file = BlobSaveFileV1 {
        schema_version: SAVE_SCHEMA_VERSION,
        blob: BlobMetaV1 {
            save_file: blob_window.save_file.clone(),
            size_bm: [blob_window.size_bm.x, blob_window.size_bm.y],
            pixels_per_bm: blob_window.pixels_per_bm,
        },
        tanks: saved_tanks,
        projectiles: saved_projectiles,
        portals: saved_portals,
    };

    let save_path = save_config.save_path_for(&blob_window.save_file);
    save_blob_to_disk(&save_path, &save_file)
}

pub(crate) fn load_blob_from_disk(path: &Path) -> Result<BlobSaveFileV1, String> {
    let content = fs::read_to_string(path)
        .map_err(|err| format!("read {} failed: {err}", path.display()))?;
    serde_json::from_str::<BlobSaveFileV1>(&content)
        .map_err(|err| format!("parse {} failed: {err}", path.display()))
}

pub(crate) fn save_blob_to_disk(path: &Path, save_file: &BlobSaveFileV1) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create dir {} failed: {err}", parent.display()))?;
    }

    let serialized = serde_json::to_string_pretty(save_file)
        .map_err(|err| format!("serialize save file failed: {err}"))?;

    let mut tmp_os: OsString = path.as_os_str().to_os_string();
    tmp_os.push(".tmp");
    let tmp_path = PathBuf::from(tmp_os);

    fs::write(&tmp_path, &serialized)
        .map_err(|err| format!("write temp file {} failed: {err}", tmp_path.display()))?;

    match fs::rename(&tmp_path, path) {
        Ok(()) => return Ok(()),
        Err(rename_err) => {
            error!(
                "Atomic rename failed ({} -> {}): {}. Falling back to direct write.",
                tmp_path.display(),
                path.display(),
                rename_err
            );

            fs::write(path, &serialized)
                .map_err(|err| format!("fallback write {} failed: {err}", path.display()))?;

            let _ = fs::remove_file(&tmp_path);
        }
    }

    Ok(())
}

use std::path::{Path, PathBuf};
use std::{ffi::OsString, fs};

use bevy::app::AppExit;
use bevy::log::{error, info};
use bevy::prelude::*;
use bevy::window::WindowCloseRequested;
use serde::{Deserialize, Serialize};

use crate::portal::Portal;
use crate::projectile::baseprojectile::BaseProjectile;
use crate::projectile::Projectile;
use crate::tank::{FactionId, Tank, TankStats, PLAYER_FACTION_ID};
use crate::windowblob::{ActiveWindowBlob, BlobRenderSettings, MAIN_BLOB_SAVE_FILE};

pub const SAVE_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_AUTOSAVE_SECONDS: f32 = 5.0;

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<LoadBlobRequest>()
            .init_resource::<SaveConfig>()
            .init_resource::<AutosaveTimer>()
            .init_resource::<NextPersistentEntityId>()
            .init_resource::<InitialSavePending>()
            .init_resource::<LastSaveError>()
            .add_systems(PreStartup, load_main_blob_or_bootstrap)
            .add_systems(Update, assign_persistent_ids)
            .add_systems(Update, perform_initial_save_if_pending.after(assign_persistent_ids))
            .add_systems(Update, autosave_blob_state.after(assign_persistent_ids))
            .add_systems(Update, handle_load_blob_requests.after(assign_persistent_ids))
            .add_systems(Update, save_on_window_close_requested.after(assign_persistent_ids));
    }
}

#[derive(Event, Debug, Clone)]
pub struct LoadBlobRequest {
    pub target_save_file: String,
    pub spawn_near_portal_id: Option<u64>,
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
}

fn load_main_blob_or_bootstrap(
    mut commands: Commands,
    save_config: Res<SaveConfig>,
    mut autosave_timer: ResMut<AutosaveTimer>,
    mut initial_save_pending: ResMut<InitialSavePending>,
    mut next_id: ResMut<NextPersistentEntityId>,
    mut active_blob: ResMut<ActiveWindowBlob>,
    mut blob_render_settings: ResMut<BlobRenderSettings>,
) {
    autosave_timer.0 = Timer::from_seconds(save_config.autosave_seconds, TimerMode::Repeating);

    let save_path = save_config.save_path_for(MAIN_BLOB_SAVE_FILE);

    if !save_path.exists() {
        initial_save_pending.0 = true;
        next_id.0 = 1;
        info!(
            "Save file {} does not exist. Bootstrapping default main blob.",
            save_path.display()
        );
        return;
    }

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

    next_id.0 = apply_loaded_blob_to_world(
        &mut commands,
        &parsed_save,
        &mut active_blob,
        &mut blob_render_settings,
        None,
    )
    .saturating_add(1)
    .max(1);
    initial_save_pending.0 = false;

    info!("Loaded save from {}", save_path.display());
}

fn assign_persistent_ids(
    mut commands: Commands,
    mut next_id: ResMut<NextPersistentEntityId>,
    entities: Query<
        Entity,
        (
            Without<PersistentEntityId>,
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
    active_blob: Res<ActiveWindowBlob>,
    blob_render_settings: Res<BlobRenderSettings>,
    tanks: Query<(&PersistentEntityId, &Transform, &TankStats, &FactionId), With<Tank>>,
    projectiles: Query<(&PersistentEntityId, &Transform, &BaseProjectile), With<Projectile>>,
    portals: Query<(&PersistentEntityId, &Transform, &Portal)>,
    mut last_save_error: ResMut<LastSaveError>,
) {
    if !initial_save_pending.0 {
        return;
    }

    match save_current_blob_state(
        &save_config,
        &active_blob,
        &blob_render_settings,
        &tanks,
        &projectiles,
        &portals,
    ) {
        Ok(()) => {
            initial_save_pending.0 = false;
            last_save_error.0 = None;
            info!("Initial save created for {}", active_blob.save_file);
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
    active_blob: Res<ActiveWindowBlob>,
    blob_render_settings: Res<BlobRenderSettings>,
    tanks: Query<(&PersistentEntityId, &Transform, &TankStats, &FactionId), With<Tank>>,
    projectiles: Query<(&PersistentEntityId, &Transform, &BaseProjectile), With<Projectile>>,
    portals: Query<(&PersistentEntityId, &Transform, &Portal)>,
    mut last_save_error: ResMut<LastSaveError>,
) {
    if !autosave_timer.0.tick(time.delta()).just_finished() {
        return;
    }

    match save_current_blob_state(
        &save_config,
        &active_blob,
        &blob_render_settings,
        &tanks,
        &projectiles,
        &portals,
    ) {
        Ok(()) => {
            last_save_error.0 = None;
        }
        Err(err) => {
            error!("Autosave failed: {err}");
            last_save_error.0 = Some(err);
        }
    }
}

fn save_on_window_close_requested(
    mut close_requests: EventReader<WindowCloseRequested>,
    mut app_exit: EventWriter<AppExit>,
    save_config: Res<SaveConfig>,
    active_blob: Res<ActiveWindowBlob>,
    blob_render_settings: Res<BlobRenderSettings>,
    tanks: Query<(&PersistentEntityId, &Transform, &TankStats, &FactionId), With<Tank>>,
    projectiles: Query<(&PersistentEntityId, &Transform, &BaseProjectile), With<Projectile>>,
    portals: Query<(&PersistentEntityId, &Transform, &Portal)>,
    mut last_save_error: ResMut<LastSaveError>,
) {
    let close_requested = close_requests.read().next().is_some();
    if !close_requested {
        return;
    }

    match save_current_blob_state(
        &save_config,
        &active_blob,
        &blob_render_settings,
        &tanks,
        &projectiles,
        &portals,
    ) {
        Ok(()) => {
            last_save_error.0 = None;
            app_exit.send(AppExit::Success);
        }
        Err(err) => {
            error!("Save on exit failed. Blocking exit: {err}");
            last_save_error.0 = Some(err);
        }
    }
}

fn handle_load_blob_requests(
    mut commands: Commands,
    mut load_blob_requests: EventReader<LoadBlobRequest>,
    save_config: Res<SaveConfig>,
    mut active_blob: ResMut<ActiveWindowBlob>,
    mut blob_render_settings: ResMut<BlobRenderSettings>,
    tanks: Query<(&PersistentEntityId, &Transform, &TankStats, &FactionId), With<Tank>>,
    projectiles: Query<(&PersistentEntityId, &Transform, &BaseProjectile), With<Projectile>>,
    portals: Query<(&PersistentEntityId, &Transform, &Portal)>,
    blob_entities: Query<Entity, Or<(With<Tank>, With<Projectile>, With<Portal>)>>,
    mut next_id: ResMut<NextPersistentEntityId>,
    mut last_save_error: ResMut<LastSaveError>,
) {
    let Some(request) = load_blob_requests.read().last().cloned() else {
        return;
    };

    if let Err(err) = save_current_blob_state(
        &save_config,
        &active_blob,
        &blob_render_settings,
        &tanks,
        &projectiles,
        &portals,
    ) {
        error!("Failed to save current blob before portal transfer: {}", err);
        last_save_error.0 = Some(err);
        return;
    }

    let target_path = save_config.save_path_for(&request.target_save_file);
    if !target_path.exists() {
        let err = format!("target blob save {} does not exist", target_path.display());
        error!("{}", err);
        last_save_error.0 = Some(err);
        return;
    }

    let loaded_blob = match load_blob_from_disk(&target_path) {
        Ok(loaded) => loaded,
        Err(err) => {
            error!("Failed to load target blob {}: {}", target_path.display(), err);
            last_save_error.0 = Some(err);
            return;
        }
    };

    if loaded_blob.schema_version != SAVE_SCHEMA_VERSION {
        let err = format!(
            "unsupported schema_version {} in {}",
            loaded_blob.schema_version,
            target_path.display()
        );
        error!("{}", err);
        last_save_error.0 = Some(err);
        return;
    }

    for entity in &blob_entities {
        commands.entity(entity).despawn_recursive();
    }

    next_id.0 = apply_loaded_blob_to_world(
        &mut commands,
        &loaded_blob,
        &mut active_blob,
        &mut blob_render_settings,
        request.spawn_near_portal_id,
    )
    .saturating_add(1)
    .max(1);

    last_save_error.0 = None;
    info!("Loaded blob {}", request.target_save_file);
}

fn save_current_blob_state(
    save_config: &SaveConfig,
    active_blob: &ActiveWindowBlob,
    blob_render_settings: &BlobRenderSettings,
    tanks: &Query<(&PersistentEntityId, &Transform, &TankStats, &FactionId), With<Tank>>,
    projectiles: &Query<(&PersistentEntityId, &Transform, &BaseProjectile), With<Projectile>>,
    portals: &Query<(&PersistentEntityId, &Transform, &Portal)>,
) -> Result<(), String> {
    let save_file =
        build_blob_save_file(active_blob, blob_render_settings, tanks, projectiles, portals);
    let save_path = save_config.save_path_for(&active_blob.save_file);
    save_blob_to_disk(&save_path, &save_file)
}

fn apply_loaded_blob_to_world(
    commands: &mut Commands,
    loaded_blob: &BlobSaveFileV1,
    active_blob: &mut ActiveWindowBlob,
    blob_render_settings: &mut BlobRenderSettings,
    spawn_near_portal_id: Option<u64>,
) -> u64 {
    active_blob.save_file = loaded_blob.blob.save_file.clone();
    active_blob.size_bm = Vec2::new(loaded_blob.blob.size_bm[0], loaded_blob.blob.size_bm[1]);
    blob_render_settings.pixels_per_bm = loaded_blob.blob.pixels_per_bm;

    let spawn_position_override = spawn_near_portal_id.and_then(|portal_id| {
        loaded_blob
            .portals
            .iter()
            .find(|portal| portal.id == portal_id)
            .map(|portal| Vec2::new(portal.position_bm[0], portal.position_bm[1]) + Vec2::X * 1.1)
    });

    let mut max_id: u64 = 0;
    let mut relocated_player = false;

    for tank in &loaded_blob.tanks {
        max_id = max_id.max(tank.id);

        let mut position = Vec2::new(tank.position_bm[0], tank.position_bm[1]);
        if !relocated_player && tank.faction_id == PLAYER_FACTION_ID {
            if let Some(spawn_position) = spawn_position_override {
                position = spawn_position;
                relocated_player = true;
            }
        }

        commands.spawn((
            PersistentEntityId(tank.id),
            Tank,
            FactionId(tank.faction_id),
            TankStats {
                hp: tank.hp,
                move_speed: tank.move_speed,
                turn_speed: tank.turn_speed,
            },
            SpatialBundle::from_transform(Transform {
                translation: Vec3::new(position.x, position.y, 0.0),
                rotation: Quat::from_rotation_z(tank.rotation_rad),
                ..default()
            }),
        ));
    }

    if loaded_blob.tanks.is_empty() {
        if let Some(spawn_position) = spawn_position_override {
            let tank_id = max_id.saturating_add(1).max(1);
            max_id = max_id.max(tank_id);
            commands.spawn((
                PersistentEntityId(tank_id),
                Tank,
                FactionId(PLAYER_FACTION_ID),
                TankStats::default(),
                SpatialBundle::from_transform(Transform::from_xyz(
                    spawn_position.x,
                    spawn_position.y,
                    0.0,
                )),
            ));
        }
    }

    for projectile in &loaded_blob.projectiles {
        max_id = max_id.max(projectile.id);
        commands.spawn((
            PersistentEntityId(projectile.id),
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
            Portal {
                target_blob_save_file: portal.target_blob_save_file.clone(),
                target_portal_id: portal.target_portal_id,
                radius_bm: portal.radius_bm,
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

fn build_blob_save_file(
    active_blob: &ActiveWindowBlob,
    blob_render_settings: &BlobRenderSettings,
    tanks: &Query<(&PersistentEntityId, &Transform, &TankStats, &FactionId), With<Tank>>,
    projectiles: &Query<(&PersistentEntityId, &Transform, &BaseProjectile), With<Projectile>>,
    portals: &Query<(&PersistentEntityId, &Transform, &Portal)>,
) -> BlobSaveFileV1 {
    let mut saved_tanks = Vec::new();
    for (id, transform, stats, faction) in tanks.iter() {
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
    for (id, transform, projectile) in projectiles.iter() {
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
    for (id, transform, portal) in portals.iter() {
        saved_portals.push(PortalSaveV1 {
            id: id.0,
            position_bm: [transform.translation.x, transform.translation.y],
            target_blob_save_file: portal.target_blob_save_file.clone(),
            target_portal_id: portal.target_portal_id,
            radius_bm: portal.radius_bm,
        });
    }
    saved_portals.sort_by_key(|portal| portal.id);

    BlobSaveFileV1 {
        schema_version: SAVE_SCHEMA_VERSION,
        blob: BlobMetaV1 {
            save_file: active_blob.save_file.clone(),
            size_bm: [active_blob.size_bm.x, active_blob.size_bm.y],
            pixels_per_bm: blob_render_settings.pixels_per_bm,
        },
        tanks: saved_tanks,
        projectiles: saved_projectiles,
        portals: saved_portals,
    }
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
            // Some environments disallow rename/delete even if writing is allowed.
            // Fall back to direct write to preserve save functionality.
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

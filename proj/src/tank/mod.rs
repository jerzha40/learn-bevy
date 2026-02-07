use bevy::math::primitives::Circle;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::MaterialMesh2dBundle;

use crate::inventory::Inventory;
use crate::windowblob::{
    blob_render_layer, BlobCamera, BlobInstanceId, BlobRenderLayer, BlobWindow,
    FocusedBlobInstance, MAIN_BLOB_INSTANCE_ID,
};

pub const TANK_BODY_RADIUS_BM: f32 = 0.35;
pub const TANK_TURRET_BARREL_LENGTH_BM: f32 = 0.6;
pub const TANK_TURRET_BARREL_THICKNESS_BM: f32 = 0.14;
pub const PLAYER_FACTION_ID: u8 = 1;

pub struct TankPlugin;

impl Plugin for TankPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_default_tank_if_empty)
            .add_systems(
                Update,
                (
                    assemble_tank_visuals,
                    move_tanks_with_wasd,
                    aim_turrets_at_cursor,
                    validate_tank_stats,
                ),
            );
    }
}

#[derive(Component, Debug, Default)]
pub struct Tank;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactionId(pub u8);

impl Default for FactionId {
    fn default() -> Self {
        Self(PLAYER_FACTION_ID)
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct TankStats {
    pub hp: f32,
    pub move_speed: f32,
    pub turn_speed: f32,
}

impl Default for TankStats {
    fn default() -> Self {
        Self {
            hp: 100.0,
            move_speed: 2.4,
            turn_speed: 3.2,
        }
    }
}

#[derive(Component, Debug)]
pub struct TankBodyVisual;

#[derive(Component, Debug)]
pub struct TankTurretVisual;

#[derive(Component, Debug)]
pub struct TankTurretBarrelVisual;

#[derive(Component, Debug)]
pub struct TankVisualBuilt;

#[derive(Bundle, Default)]
pub struct TankBundle {
    pub tank: Tank,
    pub faction: FactionId,
    pub stats: TankStats,
    pub inventory: Inventory,
    pub blob_instance: BlobInstanceId,
    pub blob_render_layer: BlobRenderLayer,
    pub spatial: SpatialBundle,
}

fn spawn_default_tank_if_empty(mut commands: Commands, existing_tanks: Query<Entity, With<Tank>>) {
    if !existing_tanks.is_empty() {
        return;
    }

    commands.spawn(TankBundle {
        faction: FactionId(PLAYER_FACTION_ID),
        blob_instance: BlobInstanceId(MAIN_BLOB_INSTANCE_ID),
        blob_render_layer: BlobRenderLayer(blob_render_layer(MAIN_BLOB_INSTANCE_ID)),
        spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, 0.0, 0.0)),
        ..default()
    });
}

fn assemble_tank_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    tanks: Query<(Entity, &BlobInstanceId, &BlobRenderLayer), (With<Tank>, Without<TankVisualBuilt>)>,
) {
    for (tank_entity, blob_instance, blob_layer) in &tanks {
        let body_mesh = meshes.add(Mesh::from(Circle::new(TANK_BODY_RADIUS_BM)));
        let body_material = materials.add(ColorMaterial::from(Color::srgb(0.25, 0.72, 0.32)));

        let body_entity = commands
            .spawn((
                TankBodyVisual,
                *blob_instance,
                *blob_layer,
                RenderLayers::layer(blob_layer.0),
                MaterialMesh2dBundle {
                    mesh: body_mesh.into(),
                    material: body_material,
                    transform: Transform::from_xyz(0.0, 0.0, 1.0),
                    ..default()
                },
            ))
            .id();

        let turret_entity = commands
            .spawn((
                TankTurretVisual,
                *blob_instance,
                *blob_layer,
                RenderLayers::layer(blob_layer.0),
                SpatialBundle::from_transform(Transform::from_xyz(0.0, 0.0, 2.0)),
            ))
            .id();

        let turret_barrel_entity = commands
            .spawn((
                TankTurretBarrelVisual,
                *blob_instance,
                *blob_layer,
                RenderLayers::layer(blob_layer.0),
                SpriteBundle {
                    sprite: Sprite {
                        color: Color::srgb(0.16, 0.35, 0.2),
                        custom_size: Some(Vec2::new(
                            TANK_TURRET_BARREL_LENGTH_BM,
                            TANK_TURRET_BARREL_THICKNESS_BM,
                        )),
                        ..default()
                    },
                    transform: Transform::from_xyz(TANK_TURRET_BARREL_LENGTH_BM * 0.5, 0.0, 0.0),
                    ..default()
                },
            ))
            .id();

        commands.entity(turret_entity).add_child(turret_barrel_entity);

        commands
            .entity(tank_entity)
            .add_child(body_entity)
            .add_child(turret_entity)
            .insert(TankVisualBuilt);
    }
}

fn move_tanks_with_wasd(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    focused_blob: Res<FocusedBlobInstance>,
    mut tanks: Query<(&TankStats, &BlobInstanceId, &mut Transform), With<Tank>>,
) {
    let Some(focused_blob_id) = focused_blob.0 else {
        return;
    };

    let mut movement_input = Vec2::ZERO;

    if keyboard_input.pressed(KeyCode::KeyW) {
        movement_input.y += 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        movement_input.y -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        movement_input.x -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        movement_input.x += 1.0;
    }

    if movement_input == Vec2::ZERO {
        return;
    }

    let movement_direction = movement_input.normalize();

    for (stats, blob_instance, mut transform) in &mut tanks {
        if blob_instance.0 != focused_blob_id {
            continue;
        }

        let movement_delta = movement_direction * stats.move_speed * time.delta_seconds();
        transform.translation.x += movement_delta.x;
        transform.translation.y += movement_delta.y;
    }
}

fn aim_turrets_at_cursor(
    focused_blob: Res<FocusedBlobInstance>,
    windows: Query<(&Window, &BlobWindow)>,
    cameras: Query<(&Camera, &GlobalTransform, &BlobCamera), With<Camera2d>>,
    tanks: Query<(&GlobalTransform, &BlobInstanceId), With<Tank>>,
    mut turrets: Query<(&Parent, &mut Transform), With<TankTurretVisual>>,
) {
    let Some(focused_blob_id) = focused_blob.0 else {
        return;
    };

    let Some((window, _)) = windows
        .iter()
        .find(|(_, blob_window)| blob_window.instance_id == focused_blob_id)
    else {
        return;
    };
    let Some(cursor_position) = window.cursor_position() else {
        return;
    };
    let Some((camera, camera_transform, _)) = cameras
        .iter()
        .find(|(_, _, blob_camera)| blob_camera.instance_id == focused_blob_id)
    else {
        return;
    };
    let Some(cursor_world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position)
    else {
        return;
    };

    for (parent, mut turret_transform) in &mut turrets {
        let Ok((tank_transform, tank_blob_instance)) = tanks.get(parent.get()) else {
            continue;
        };
        if tank_blob_instance.0 != focused_blob_id {
            continue;
        }
        let tank_world_position = tank_transform.translation().truncate();
        let to_cursor = cursor_world_position - tank_world_position;

        if to_cursor.length_squared() <= f32::EPSILON {
            continue;
        }

        let target_angle = to_cursor.y.atan2(to_cursor.x);
        turret_transform.rotation = Quat::from_rotation_z(target_angle);
    }
}

fn validate_tank_stats(tanks: Query<&TankStats, Added<Tank>>) {
    for stats in &tanks {
        debug_assert!(stats.hp > 0.0);
        debug_assert!(stats.move_speed > 0.0);
        debug_assert!(stats.turn_speed > 0.0);
    }
}

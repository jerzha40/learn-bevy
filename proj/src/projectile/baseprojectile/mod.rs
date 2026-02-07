use bevy::math::primitives::Circle;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::MaterialMesh2dBundle;

use crate::projectile::Projectile;
use crate::tank::{
    FactionId, Tank, TankStats, TankTurretVisual, TANK_BODY_RADIUS_BM, TANK_TURRET_BARREL_LENGTH_BM,
};
use crate::windowblob::{BlobInstanceId, BlobRenderLayer, FocusedBlobInstance};

pub const DEFAULT_TARGET_FACTION_ID: u8 = 2;

pub struct BaseProjectilePlugin;

impl Plugin for BaseProjectilePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                fire_base_projectile,
                assemble_base_projectile_visuals,
                move_base_projectiles,
                hit_tanks_with_base_projectiles,
            ),
        );
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct BaseProjectile {
    pub speed: f32,
    pub remaining_distance: f32,
    pub damage: f32,
    pub radius: f32,
    pub direction: Vec2,
    pub target_faction_id: u8,
}

impl BaseProjectile {
    pub fn new(direction: Vec2, target_faction_id: u8) -> Self {
        Self {
            speed: 8.0,
            remaining_distance: 8.0,
            damage: 12.0,
            radius: 0.05,
            direction,
            target_faction_id,
        }
    }
}

#[derive(Component, Debug)]
pub struct BaseProjectileVisualBuilt;

#[derive(Bundle)]
pub struct BaseProjectileBundle {
    pub projectile: Projectile,
    pub base_projectile: BaseProjectile,
    pub blob_instance: BlobInstanceId,
    pub blob_render_layer: BlobRenderLayer,
    pub spatial: SpatialBundle,
}

fn fire_base_projectile(
    mouse_button: Res<ButtonInput<MouseButton>>,
    focused_blob: Res<FocusedBlobInstance>,
    mut commands: Commands,
    turrets: Query<(&GlobalTransform, &BlobInstanceId, &BlobRenderLayer), With<TankTurretVisual>>,
) {
    let Some(focused_blob_id) = focused_blob.0 else {
        return;
    };

    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    let Some((turret_transform, turret_blob, turret_layer)) = turrets
        .iter()
        .find(|(_, blob_instance, _)| blob_instance.0 == focused_blob_id)
    else {
        return;
    };

    let turret_world = turret_transform.compute_transform();
    let forward = (turret_world.rotation * Vec3::X)
        .truncate()
        .normalize_or_zero();
    if forward == Vec2::ZERO {
        return;
    }

    let projectile = BaseProjectile::new(forward, DEFAULT_TARGET_FACTION_ID);
    let spawn_offset = TANK_TURRET_BARREL_LENGTH_BM + projectile.radius;
    let spawn_position = turret_world.translation.truncate() + forward * spawn_offset;

    commands.spawn(BaseProjectileBundle {
        projectile: Projectile,
        base_projectile: projectile,
        blob_instance: *turret_blob,
        blob_render_layer: *turret_layer,
        spatial: SpatialBundle::from_transform(Transform::from_xyz(
            spawn_position.x,
            spawn_position.y,
            4.0,
        )),
    });
}

fn assemble_base_projectile_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    projectiles: Query<
        (Entity, &BaseProjectile, &BlobRenderLayer),
        (With<BaseProjectile>, Without<BaseProjectileVisualBuilt>),
    >,
) {
    for (projectile_entity, projectile, blob_layer) in &projectiles {
        let bullet_mesh = meshes.add(Mesh::from(Circle::new(projectile.radius)));
        let bullet_material = materials.add(ColorMaterial::from(Color::srgb(0.96, 0.9, 0.2)));

        let visual_entity = commands
            .spawn((
                *blob_layer,
                RenderLayers::layer(blob_layer.0),
                MaterialMesh2dBundle {
                    mesh: bullet_mesh.into(),
                    material: bullet_material,
                    transform: Transform::from_xyz(0.0, 0.0, 0.0),
                    ..default()
                },
            ))
            .id();

        commands
            .entity(projectile_entity)
            .add_child(visual_entity)
            .insert(BaseProjectileVisualBuilt);
    }
}

fn move_base_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    mut projectiles: Query<(Entity, &mut Transform, &mut BaseProjectile)>,
) {
    for (entity, mut transform, mut projectile) in &mut projectiles {
        let delta = projectile.direction * projectile.speed * time.delta_seconds();
        transform.translation += delta.extend(0.0);
        projectile.remaining_distance -= delta.length();

        if projectile.remaining_distance <= 0.0 {
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn hit_tanks_with_base_projectiles(
    mut commands: Commands,
    mut tanks: Query<(Entity, &Transform, &FactionId, &BlobInstanceId, &mut TankStats), With<Tank>>,
    projectiles: Query<(Entity, &Transform, &BaseProjectile, &BlobInstanceId), With<Projectile>>,
) {
    for (projectile_entity, projectile_transform, projectile, projectile_blob) in &projectiles {
        let projectile_position = projectile_transform.translation.truncate();
        let mut has_hit = false;

        for (_, tank_transform, tank_faction, tank_blob, mut tank_stats) in &mut tanks {
            if tank_blob.0 != projectile_blob.0 {
                continue;
            }
            if tank_faction.0 != projectile.target_faction_id {
                continue;
            }

            let tank_position = tank_transform.translation.truncate();
            let hit_distance = projectile.radius + TANK_BODY_RADIUS_BM;
            let delta = projectile_position - tank_position;

            if delta.length_squared() <= hit_distance * hit_distance {
                tank_stats.hp -= projectile.damage;
                has_hit = true;
                break;
            }
        }

        if has_hit {
            commands.entity(projectile_entity).despawn_recursive();
        }
    }
}

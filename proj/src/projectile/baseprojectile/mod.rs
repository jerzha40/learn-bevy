use bevy::math::primitives::Circle;
use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;

use crate::projectile::Projectile;
use crate::tank::{Tank, TankStats, TankTurretVisual};

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
    pub owner: Entity,
}

impl BaseProjectile {
    pub fn new(direction: Vec2, owner: Entity) -> Self {
        Self {
            speed: 520.0,
            remaining_distance: 600.0,
            damage: 12.0,
            radius: 4.0,
            direction,
            owner,
        }
    }
}

#[derive(Component, Debug)]
pub struct BaseProjectileVisualBuilt;

#[derive(Bundle)]
pub struct BaseProjectileBundle {
    pub projectile: Projectile,
    pub base_projectile: BaseProjectile,
    pub spatial: SpatialBundle,
}

fn fire_base_projectile(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut commands: Commands,
    turrets: Query<(&Parent, &GlobalTransform), With<TankTurretVisual>>,
    tanks: Query<Entity, With<Tank>>,
) {
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    let Some((owner, turret_transform)) = turrets.iter().next() else {
        return;
    };
    let owner_entity = owner.get();
    if !tanks.contains(owner_entity) {
        return;
    }

    let turret_world = turret_transform.compute_transform();
    let forward = (turret_world.rotation * Vec3::X).truncate().normalize_or_zero();
    if forward == Vec2::ZERO {
        return;
    }

    let spawn_position = turret_world.translation.truncate() + forward * 32.0;

    commands.spawn(BaseProjectileBundle {
        projectile: Projectile,
        base_projectile: BaseProjectile::new(forward, owner_entity),
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
    projectiles: Query<Entity, (With<BaseProjectile>, Without<BaseProjectileVisualBuilt>)>,
) {
    for projectile_entity in &projectiles {
        let bullet_mesh = meshes.add(Mesh::from(Circle::new(4.0)));
        let bullet_material = materials.add(ColorMaterial::from(Color::srgb(0.96, 0.9, 0.2)));

        let visual_entity = commands
            .spawn(MaterialMesh2dBundle {
                mesh: bullet_mesh.into(),
                material: bullet_material,
                transform: Transform::from_xyz(0.0, 0.0, 0.0),
                ..default()
            })
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
    mut tanks: Query<(Entity, &Transform, &mut TankStats), With<Tank>>,
    projectiles: Query<(Entity, &Transform, &BaseProjectile), With<Projectile>>,
) {
    for (projectile_entity, projectile_transform, projectile) in &projectiles {
        let projectile_position = projectile_transform.translation.truncate();
        let mut has_hit = false;

        for (tank_entity, tank_transform, mut tank_stats) in &mut tanks {
            if tank_entity == projectile.owner {
                continue;
            }

            let tank_position = tank_transform.translation.truncate();
            let hit_distance = projectile.radius + 24.0;
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

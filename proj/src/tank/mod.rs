use bevy::math::primitives::Circle;
use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::window::PrimaryWindow;

pub struct TankPlugin;

impl Plugin for TankPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_test_tank)
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
            move_speed: 180.0,
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
    pub stats: TankStats,
    pub spatial: SpatialBundle,
}

fn spawn_test_tank(mut commands: Commands) {
    commands.spawn(TankBundle {
        spatial: SpatialBundle::from_transform(Transform::from_xyz(0.0, 0.0, 0.0)),
        ..default()
    });
}

fn assemble_tank_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    tanks: Query<Entity, (With<Tank>, Without<TankVisualBuilt>)>,
) {
    for tank_entity in &tanks {
        let body_mesh = meshes.add(Mesh::from(Circle::new(24.0)));
        let body_material = materials.add(ColorMaterial::from(Color::srgb(0.25, 0.72, 0.32)));

        let body_entity = commands
            .spawn((
                TankBodyVisual,
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
                SpatialBundle::from_transform(Transform::from_xyz(0.0, 0.0, 2.0)),
            ))
            .id();

        let turret_barrel_entity = commands
            .spawn((
                TankTurretBarrelVisual,
                SpriteBundle {
                    sprite: Sprite {
                        color: Color::srgb(0.16, 0.35, 0.2),
                        custom_size: Some(Vec2::new(30.0, 10.0)),
                        ..default()
                    },
                    transform: Transform::from_xyz(15.0, 0.0, 0.0),
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
    mut tanks: Query<(&TankStats, &mut Transform), With<Tank>>,
) {
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

    for (stats, mut transform) in &mut tanks {
        let movement_delta = movement_direction * stats.move_speed * time.delta_seconds();
        transform.translation.x += movement_delta.x;
        transform.translation.y += movement_delta.y;
    }
}

fn aim_turrets_at_cursor(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    tanks: Query<&GlobalTransform, With<Tank>>,
    mut turrets: Query<(&Parent, &mut Transform), With<TankTurretVisual>>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Some(cursor_position) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_transform)) = cameras.get_single() else {
        return;
    };
    let Some(cursor_world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position)
    else {
        return;
    };

    for (parent, mut turret_transform) in &mut turrets {
        let Ok(tank_transform) = tanks.get(parent.get()) else {
            continue;
        };
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

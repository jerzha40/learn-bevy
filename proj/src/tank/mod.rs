use bevy::prelude::*;

pub struct TankPlugin;

impl Plugin for TankPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_test_tank)
            .add_systems(Update, (assemble_tank_visuals, validate_tank_stats));
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
pub struct TankVisual;

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
    tanks: Query<Entity, (With<Tank>, Without<TankVisualBuilt>)>,
) {
    for tank_entity in &tanks {
        let visual_entity = commands
            .spawn((
                TankVisual,
                SpriteBundle {
                    sprite: Sprite {
                        color: Color::srgb(0.25, 0.72, 0.32),
                        custom_size: Some(Vec2::new(56.0, 34.0)),
                        ..default()
                    },
                    transform: Transform::from_xyz(0.0, 0.0, 1.0),
                    ..default()
                },
            ))
            .id();

        commands
            .entity(tank_entity)
            .add_child(visual_entity)
            .insert(TankVisualBuilt);
    }
}

fn validate_tank_stats(tanks: Query<&TankStats, Added<Tank>>) {
    for stats in &tanks {
        debug_assert!(stats.hp > 0.0);
        debug_assert!(stats.move_speed > 0.0);
        debug_assert!(stats.turn_speed > 0.0);
    }
}

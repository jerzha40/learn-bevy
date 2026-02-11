use bevy::prelude::*;

/// Team (player/enemy for now)
#[derive(Component, Copy, Clone, Debug, Eq, PartialEq)]
pub enum TankTeam {
    Player,
    Enemy,
}

/// Tank core data
#[derive(Component, Debug)]
pub struct Tank {
    pub team: TankTeam,
    pub hp: i32,
    pub move_speed: f32,
    pub turn_speed: f32,
}

#[derive(Component)]
pub struct TankBody;

#[derive(Component)]
pub struct Turret;

/// Marks that visuals have been assembled for this tank
#[derive(Component)]
pub struct TankVisual;

/// Stat config
pub struct TankStats {
    pub hp: i32,
    pub move_speed: f32,
    pub turn_speed: f32,
}

pub fn stats(team: TankTeam) -> TankStats {
    match team {
        TankTeam::Player => TankStats {
            hp: 3,
            move_speed: 140.0,
            turn_speed: 4.0,
        },
        TankTeam::Enemy => TankStats {
            hp: 2,
            move_speed: 110.0,
            turn_speed: 3.2,
        },
    }
}

/// Spawn a tank logic prefab (no visuals attached)
pub fn spawn_tank(commands: &mut Commands, world_pos: Vec3, team: TankTeam) -> Entity {
    let s = stats(team);

    commands
        .spawn((
            Name::new("Tank"),
            Tank {
                team,
                hp: s.hp,
                move_speed: s.move_speed,
                turn_speed: s.turn_speed,
            },
            TankBody,
            Transform::from_translation(world_pos),
            GlobalTransform::default(),
        ))
        .id()
}

/// Visual assembly system: add sprites + turret/barrel children
pub fn tank_visual_system(
    mut commands: Commands,
    q: Query<(Entity, &Tank, &Transform), Without<TankVisual>>,
) {
    for (e, tank, t) in &q {
        let body_color = match tank.team {
            TankTeam::Player => Color::srgb(0.20, 0.65, 0.30),
            TankTeam::Enemy => Color::srgb(0.80, 0.20, 0.20),
        };
        let turret_color = match tank.team {
            TankTeam::Player => Color::srgb(0.12, 0.45, 0.20),
            TankTeam::Enemy => Color::srgb(0.60, 0.12, 0.12),
        };

        let body_size = Vec2::new(44.0, 44.0);
        let turret_size = Vec2::new(16.0, 26.0);
        let barrel_size = Vec2::new(8.0, 18.0);

        commands.entity(e).insert((
            TankVisual,
            SpriteBundle {
                sprite: Sprite {
                    color: body_color,
                    custom_size: Some(body_size),
                    ..default()
                },
                transform: *t,
                ..default()
            },
        ));

        commands.entity(e).with_children(|parent| {
            parent
                .spawn((
                    Name::new("Turret"),
                    Turret,
                    SpriteBundle {
                        sprite: Sprite {
                            color: turret_color,
                            custom_size: Some(turret_size),
                            ..default()
                        },
                        transform: Transform::from_translation(Vec3::new(0.0, 0.0, 1.0)),
                        ..default()
                    },
                ))
                .with_children(|turret| {
                    turret.spawn((
                        Name::new("Barrel"),
                        SpriteBundle {
                            sprite: Sprite {
                                color: turret_color,
                                custom_size: Some(barrel_size),
                                ..default()
                            },
                            transform: Transform::from_translation(Vec3::new(
                                0.0,
                                turret_size.y * 0.55,
                                1.0,
                            )),
                            ..default()
                        },
                    ));
                });
        });
    }
}

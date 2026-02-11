use bevy::prelude::*;

#[derive(Component)]
pub struct Bullet {
    pub velocity: Vec2,
    pub damage: i32,
    pub radius: f32,
}

#[derive(Component)]
pub struct BulletVisual;

struct BulletStats {
    speed: f32,
    damage: i32,
    radius: f32,
}

fn stats() -> BulletStats {
    BulletStats {
        speed: 420.0,
        damage: 1,
        radius: 6.0,
    }
}

/// Spawn a bullet logic prefab (no visuals attached)
pub fn spawn_bullet(commands: &mut Commands, world_pos: Vec3, dir: Vec2) -> Entity {
    let s = stats();
    let dir = if dir.length_squared() > 0.0001 {
        dir.normalize()
    } else {
        Vec2::Y
    };

    commands
        .spawn((
            Name::new("Bullet"),
            Bullet {
                velocity: dir * s.speed,
                damage: s.damage,
                radius: s.radius,
            },
            Transform::from_translation(world_pos),
            GlobalTransform::default(),
        ))
        .id()
}

/// Visual assembly system for bullets
pub fn bullet_visual_system(
    mut commands: Commands,
    q: Query<(Entity, &Bullet, &Transform), Without<BulletVisual>>,
) {
    for (e, b, t) in &q {
        let size = b.radius * 2.0;
        commands.entity(e).insert((
            BulletVisual,
            SpriteBundle {
                sprite: Sprite {
                    color: Color::srgb(1.0, 0.90, 0.20),
                    custom_size: Some(Vec2::splat(size)),
                    ..default()
                },
                transform: *t,
                ..default()
            },
        ));
    }
}

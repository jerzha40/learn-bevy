use bevy::prelude::*;

#[derive(Component)]
pub struct DummyTarget {
    pub hp: i32,
    pub radius: f32,
}

#[derive(Component)]
pub struct DummyTargetVisual;

struct DummyTargetStats {
    hp: i32,
    radius: f32,
}

fn stats() -> DummyTargetStats {
    DummyTargetStats { hp: 3, radius: 18.0 }
}

/// Spawn a dummy target logic prefab (no visuals attached)
pub fn spawn_dummy_target(commands: &mut Commands, world_pos: Vec3) -> Entity {
    let s = stats();

    commands
        .spawn((
            Name::new("DummyTarget"),
            DummyTarget {
                hp: s.hp,
                radius: s.radius,
            },
            Transform::from_translation(world_pos),
            GlobalTransform::default(),
        ))
        .id()
}

/// Visual assembly system for dummy targets
pub fn dummy_target_visual_system(
    mut commands: Commands,
    q: Query<(Entity, &DummyTarget, &Transform), Without<DummyTargetVisual>>,
) {
    for (e, tdata, t) in &q {
        let size = tdata.radius * 2.0;
        commands.entity(e).insert((
            DummyTargetVisual,
            SpriteBundle {
                sprite: Sprite {
                    color: Color::srgb(0.95, 0.35, 0.35),
                    custom_size: Some(Vec2::splat(size)),
                    ..default()
                },
                transform: *t,
                ..default()
            },
        ));
    }
}

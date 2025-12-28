use bevy::prelude::*;

/// 子弹类型（以后冰豆/火豆/穿透/爆炸都加这里）
#[derive(Component, Copy, Clone, Debug)]
pub enum BulletKind {
    Pea,
}

/// 子弹组件
#[derive(Component)]
pub struct Bullet {
    pub row: i32,
    pub kind: BulletKind,
}

/// 子弹数值（像配置表）
pub struct BulletStats {
    pub speed: f32,
    pub damage: i32,
    pub radius: f32,
}

pub fn stats(kind: BulletKind) -> BulletStats {
    match kind {
        BulletKind::Pea => BulletStats {
            speed: 260.0,
            damage: 1,
            radius: 8.0,
        },
    }
}

/// 生成子弹实体
pub fn spawn_bullet(
    commands: &mut Commands,
    row: i32,
    world_pos: Vec3,
    kind: BulletKind,
) -> Entity {
    // 颜色先按类型区分，后面换贴图也可以
    let color = match kind {
        BulletKind::Pea => Color::srgb(1.0, 0.9, 0.2),
    };

    commands
        .spawn((
            Bullet { row, kind },
            SpriteBundle {
                sprite: Sprite {
                    color,
                    custom_size: Some(Vec2::splat(16.0)),
                    ..default()
                },
                transform: Transform::from_translation(world_pos),
                ..default()
            },
        ))
        .id()
}

/// 子弹移动系统
pub fn bullet_move_system(time: Res<Time>, mut q: Query<(&Bullet, &mut Transform)>) {
    for (b, mut t) in &mut q {
        let s = stats(b.kind);
        t.translation.x += s.speed * time.delta_seconds();
    }
}

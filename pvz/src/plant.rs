use bevy::prelude::*;

/// 植物类型（以后加新植物就在这里加 enum 分支）
#[derive(Component, Copy, Clone, Debug)]
pub enum PlantKind {
    Peashooter, // 豌豆射手：会开火
    Sunflower,  // 向日葵：以后产阳光
    Wallnut,    // 坚果墙：以后抗打
}

/// 植物组件：挂在植物实体上
#[derive(Component)]
pub struct Plant {
    pub row: i32,
    pub kind: PlantKind,
    pub fire: Timer, // 先保留：只有射手会用到，其他种类我们后面再扩展/拆组件
}

/// 以后你想不同植物不同数值：都集中在这里（像配置表）
pub struct PlantStats {
    pub fire_every: f32,
    pub bullet_damage: i32,
    pub bullet_speed: f32,
}

pub fn stats(kind: PlantKind) -> PlantStats {
    match kind {
        PlantKind::Peashooter => PlantStats {
            fire_every: 0.8,
            bullet_damage: 1,
            bullet_speed: 260.0,
        },
        PlantKind::Sunflower => PlantStats {
            fire_every: 99999.0, // 不开火
            bullet_damage: 0,
            bullet_speed: 0.0,
        },
        PlantKind::Wallnut => PlantStats {
            fire_every: 99999.0, // 不开火
            bullet_damage: 0,
            bullet_speed: 0.0,
        },
    }
}

/// 生成一个植物实体（给 main.rs / 输入系统调用）
///
/// 注意：这里只负责 spawn 植物，不负责 tile 的 occupant（那个逻辑在 click 系统里写回）
pub fn spawn_plant(
    commands: &mut Commands,
    world_pos: Vec3,
    row: i32,
    kind: PlantKind,
    tile_size: f32,
) -> Entity {
    let s = stats(kind);

    // 不同 kind 给不同颜色，方便你调试
    let color = match kind {
        PlantKind::Peashooter => Color::srgb(0.2, 0.6, 1.0),
        PlantKind::Sunflower => Color::srgb(1.0, 0.85, 0.2),
        PlantKind::Wallnut => Color::srgb(0.55, 0.35, 0.2),
    };

    commands
        .spawn((
            Plant {
                row,
                kind,
                fire: Timer::from_seconds(s.fire_every, TimerMode::Repeating),
            },
            SpriteBundle {
                sprite: Sprite {
                    color,
                    custom_size: Some(Vec2::splat(tile_size * 0.55)),
                    ..default()
                },
                transform: Transform::from_translation(world_pos),
                ..default()
            },
        ))
        .id()
}

use crate::tilemap::{COLS, ROWS, TILE};

// 注意：Zombie 和 Bullet 目前还在 main.rs（crate 根模块）里定义
// 子模块可以访问父模块的私有项，所以这里用 super::Zombie / super::Bullet
use super::Zombie;
use crate::bullet::{BulletKind, spawn_bullet};

/// 植物开火系统：只有 Peashooter 会开火
pub fn plant_fire_system(
    mut commands: Commands,
    time: Res<Time>,
    mut plants: Query<(&mut Plant, &Transform)>,
    zombies: Query<(&Zombie, &Transform)>,
) {
    // 统计每一行是否有僵尸
    let mut row_has_zombie = [false; ROWS as usize];
    for (z, zt) in &zombies {
        // 只要还在屏幕附近就算威胁
        if zt.translation.x > -(COLS as f32) * TILE / 2.0 - 100.0 {
            row_has_zombie[z.row as usize] = true;
        }
    }

    for (mut plant, pt) in &mut plants {
        // 只有豌豆射手开火，其他植物先啥也不干
        if !matches!(plant.kind, PlantKind::Peashooter) {
            continue;
        }

        plant.fire.tick(time.delta());
        if !plant.fire.just_finished() {
            continue;
        }

        if !row_has_zombie[plant.row as usize] {
            continue;
        }

        // stats 决定伤害等参数
        let s = stats(plant.kind);

        // 发射子弹
        let spawn = pt.translation + Vec3::new(TILE * 0.35, 0.0, 5.0);
        spawn_bullet(&mut commands, plant.row, spawn, BulletKind::Pea);
    }
}

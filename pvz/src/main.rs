use bevy::prelude::*;

mod tile;
use crate::tile::{Occupant, Terrain, Tile, TilePos};
mod tilemap;
use crate::tilemap::{COLS, ROWS, TILE, cell_center_world, world_to_cell};
mod plant;
use crate::plant::{Plant, PlantKind, plant_fire_system, spawn_plant};
mod bullet;
use crate::bullet::{Bullet, bullet_move_system, stats as bullet_stats};

// 玩法参数（你后面可随便调）
const ZOMBIE_SPAWN_EVERY: f32 = 2.0; // 每 2 秒刷一个
const ZOMBIE_SPEED: f32 = 60.0; // 僵尸水平速度（像素/秒）
const ZOMBIE_HP: i32 = 5;

// 简易碰撞半径（方块也当圆近似）
const ZOMBIE_RADIUS: f32 = 22.0;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.06, 0.08, 0.06)))
        .insert_resource(ZombieSpawnTimer(Timer::from_seconds(
            ZOMBIE_SPAWN_EVERY,
            TimerMode::Repeating,
        )))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "PVZ demo (Bevy)".into(),
                resolution: (1000.0, 600.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                click_place_plant,
                zombie_spawner,
                zombie_move,
                plant_fire_system,
                bullet_move_system,
                bullet_hit_zombie,
                cleanup_out_of_bounds,
            ),
        )
        .run();
}

/* ---------- Components ---------- */

#[derive(Component)]
struct Zombie {
    row: i32,
    hp: i32,
}

/* ---------- Resources ---------- */

#[derive(Resource)]
struct ZombieSpawnTimer(Timer);

/* ---------- Setup ---------- */

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());

    // 草坪网格
    for r in 0..ROWS {
        for c in 0..COLS {
            let pos = cell_center_world(r, c);

            // 先都当草地（你以后可以把某些格子改成 Water/Stone）
            let terrain = Terrain::Grass;

            let color = match terrain {
                Terrain::Grass => {
                    if (r + c) % 2 == 0 {
                        Color::srgb(0.18, 0.45, 0.18)
                    } else {
                        Color::srgb(0.16, 0.40, 0.16)
                    }
                }
                Terrain::Water => Color::srgb(0.10, 0.25, 0.60),
                Terrain::Stone => Color::srgb(0.35, 0.35, 0.35),
            };

            commands.spawn((
                Tile,
                TilePos { r, c },
                terrain,
                Occupant(None), // None = 空气
                SpriteBundle {
                    sprite: Sprite {
                        color,
                        custom_size: Some(Vec2::splat(TILE - 4.0)),
                        ..default()
                    },
                    transform: Transform::from_translation(pos),
                    ..default()
                },
            ));
        }
    }
}

/* ---------- Input: click to place plant ---------- */

fn click_place_plant(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
    mut tiles: Query<(&TilePos, &Terrain, &mut Occupant), With<Tile>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let window = windows.single();
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    let (camera, cam_transform) = cam_q.single();
    let Some(world_pos) = camera.viewport_to_world_2d(cam_transform, cursor) else {
        return;
    };

    let Some((r, c)) = world_to_cell(world_pos) else {
        return;
    };

    // 找到被点中的 Tile（通过 r,c 匹配）
    for (pos, terrain, mut occ) in &mut tiles {
        if pos.r != r || pos.c != c {
            continue;
        }

        // 只有草地能种（以后你可以扩展 Water/Stone）
        if !matches!(*terrain, Terrain::Grass) {
            return;
        }

        // occ.0 == None 表示“空气”，可以种
        if occ.0.is_some() {
            return;
        }

        // 先选一种植物：现在先默认 Peashooter（以后你再做 UI/快捷键选择）
        let kind = PlantKind::Peashooter;

        // 生成植物（返回 entity）
        let plant_entity = spawn_plant(
            &mut commands,
            cell_center_world(r, c) + Vec3::new(0.0, 0.0, 10.0),
            r,
            kind,
            TILE,
        );

        occ.0 = Some(plant_entity);
        info!("Placed plant at cell r={}, c={}", r, c);
        return;
    }
}

/* ---------- Zombies ---------- */

fn zombie_spawner(mut commands: Commands, time: Res<Time>, mut timer: ResMut<ZombieSpawnTimer>) {
    timer.0.tick(time.delta());

    if !timer.0.just_finished() {
        return;
    }

    // 随机选一行刷（不引入 rand crate，做个简单伪随机）
    // 用运行时间的毫秒数取模
    let ms = (time.elapsed_seconds() * 1000.0) as i32;
    let row = (ms.abs() % ROWS).clamp(0, ROWS - 1);

    // 从最右侧外一点刷出来
    let x_spawn = (COLS as f32) * TILE / 2.0 + 120.0;
    let y = cell_center_world(row, 0).y;

    commands.spawn((
        Zombie { row, hp: ZOMBIE_HP },
        SpriteBundle {
            sprite: Sprite {
                color: Color::srgb(0.95, 0.25, 0.25),
                custom_size: Some(Vec2::new(TILE * 0.6, TILE * 0.75)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(x_spawn, y, 20.0)),
            ..default()
        },
    ));

    info!("Spawn zombie at row {}", row);
}

fn zombie_move(time: Res<Time>, mut q: Query<&mut Transform, With<Zombie>>) {
    let dx = ZOMBIE_SPEED * time.delta_seconds();
    for mut t in &mut q {
        t.translation.x -= dx;
    }
}

/* ---------- Bullets ---------- */

fn bullet_hit_zombie(
    mut commands: Commands,
    bullets: Query<(Entity, &Bullet, &Transform)>,
    mut zombies: Query<(Entity, &mut Zombie, &Transform)>,
) {
    // O(nb*nz) 简单写法：先跑通 demo，后面可优化成按行分桶
    for (b_e, b, bt) in &bullets {
        let bpos = bt.translation.truncate();

        for (z_e, mut z, zt) in &mut zombies {
            if z.row != b.row {
                continue;
            }

            let zpos = zt.translation.truncate();
            let dist2 = bpos.distance_squared(zpos);
            let bs = bullet_stats(b.kind);

            let hit_r = (bs.radius + ZOMBIE_RADIUS).powi(2);

            if dist2 <= hit_r {
                commands.entity(b_e).despawn();
                z.hp -= bs.damage;

                if z.hp <= 0 {
                    commands.entity(z_e).despawn();
                    info!("Zombie down!");
                }
                break; // 这个子弹只打一次
            }
        }
    }
}

/* ---------- Cleanup / lose condition ---------- */

fn cleanup_out_of_bounds(
    mut commands: Commands,
    bullets: Query<(Entity, &Transform), With<Bullet>>,
    zombies: Query<(Entity, &Transform), With<Zombie>>,
) {
    // 子弹飞太右边就删
    let x_right = (COLS as f32) * TILE / 2.0 + 260.0;
    for (e, t) in &bullets {
        if t.translation.x > x_right {
            commands.entity(e).despawn();
        }
    }

    // 僵尸到最左边（越界）就判负（这里先打印并删掉）
    let x_left = -(COLS as f32) * TILE / 2.0 - 120.0;
    for (e, t) in &zombies {
        if t.translation.x < x_left {
            info!("A zombie reached the house! (lose condition)");
            commands.entity(e).despawn();
        }
    }
}

use bevy::prelude::*;
use std::collections::HashSet;

const ROWS: i32 = 5;
const COLS: i32 = 9;
const TILE: f32 = 80.0;

// 玩法参数（你后面可随便调）
const ZOMBIE_SPAWN_EVERY: f32 = 2.0; // 每 2 秒刷一个
const ZOMBIE_SPEED: f32 = 60.0; // 僵尸水平速度（像素/秒）
const ZOMBIE_HP: i32 = 5;

const BULLET_SPEED: f32 = 260.0;
const PLANT_FIRE_EVERY: f32 = 0.8;
const BULLET_DAMAGE: i32 = 1;

// 简易碰撞半径（方块也当圆近似）
const ZOMBIE_RADIUS: f32 = 22.0;
const BULLET_RADIUS: f32 = 8.0;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.06, 0.08, 0.06)))
        .insert_resource(OccupiedCells::default())
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
                plant_fire,
                bullet_move,
                bullet_hit_zombie,
                cleanup_out_of_bounds,
            ),
        )
        .run();
}

/* ---------- Components ---------- */

#[derive(Component)]
struct Plant {
    row: i32,
    fire: Timer,
}

#[derive(Component)]
struct Zombie {
    row: i32,
    hp: i32,
}

#[derive(Component)]
struct Bullet {
    row: i32,
    damage: i32,
}

/* ---------- Resources ---------- */

#[derive(Resource, Default)]
struct OccupiedCells(HashSet<(i32, i32)>);

#[derive(Resource)]
struct ZombieSpawnTimer(Timer);

/* ---------- Setup ---------- */

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());

    // 草坪网格
    for r in 0..ROWS {
        for c in 0..COLS {
            let pos = cell_center_world(r, c);
            let color = if (r + c) % 2 == 0 {
                Color::srgb(0.18, 0.45, 0.18)
            } else {
                Color::srgb(0.16, 0.40, 0.16)
            };

            commands.spawn(SpriteBundle {
                sprite: Sprite {
                    color,
                    custom_size: Some(Vec2::splat(TILE - 4.0)),
                    ..default()
                },
                transform: Transform::from_translation(pos),
                ..default()
            });
        }
    }
}

/* ---------- Input: click to place plant ---------- */

fn click_place_plant(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
    mut occupied: ResMut<OccupiedCells>,
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

    if occupied.0.contains(&(r, c)) {
        return;
    }
    occupied.0.insert((r, c));

    commands.spawn((
        Plant {
            row: r,
            fire: Timer::from_seconds(PLANT_FIRE_EVERY, TimerMode::Repeating),
        },
        SpriteBundle {
            sprite: Sprite {
                color: Color::srgb(0.2, 0.6, 1.0),
                custom_size: Some(Vec2::splat(TILE * 0.55)),
                ..default()
            },
            transform: Transform::from_translation(
                cell_center_world(r, c) + Vec3::new(0.0, 0.0, 10.0),
            ),
            ..default()
        },
    ));

    info!("Placed plant at cell r={}, c={}", r, c);
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

/* ---------- Plant firing ---------- */

fn plant_fire(
    mut commands: Commands,
    time: Res<Time>,
    mut plants: Query<(&mut Plant, &Transform)>,
    zombies: Query<(&Zombie, &Transform)>,
) {
    // 先统计每一行是否有僵尸（有才开火）
    let mut row_has_zombie = [false; ROWS as usize];
    for (z, zt) in &zombies {
        // 只要还在屏幕附近就算威胁
        if zt.translation.x > -(COLS as f32) * TILE / 2.0 - 100.0 {
            row_has_zombie[z.row as usize] = true;
        }
    }

    for (mut plant, pt) in &mut plants {
        plant.fire.tick(time.delta());
        if !plant.fire.just_finished() {
            continue;
        }

        if !row_has_zombie[plant.row as usize] {
            continue;
        }

        // 发射子弹
        let spawn = pt.translation + Vec3::new(TILE * 0.35, 0.0, 5.0);
        commands.spawn((
            Bullet {
                row: plant.row,
                damage: BULLET_DAMAGE,
            },
            SpriteBundle {
                sprite: Sprite {
                    color: Color::srgb(1.0, 0.9, 0.2),
                    custom_size: Some(Vec2::splat(16.0)),
                    ..default()
                },
                transform: Transform::from_translation(spawn),
                ..default()
            },
        ));
    }
}

/* ---------- Bullets ---------- */

fn bullet_move(time: Res<Time>, mut q: Query<&mut Transform, With<Bullet>>) {
    let dx = BULLET_SPEED * time.delta_seconds();
    for mut t in &mut q {
        t.translation.x += dx;
    }
}

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
            let hit_r = (BULLET_RADIUS + ZOMBIE_RADIUS).powi(2);

            if dist2 <= hit_r {
                // 命中：子弹消失，僵尸扣血
                commands.entity(b_e).despawn();
                z.hp -= b.damage;

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

/* ---------- Grid helpers ---------- */

fn cell_center_world(r: i32, c: i32) -> Vec3 {
    let left = -(COLS as f32) * TILE / 2.0 + TILE / 2.0;
    let top = (ROWS as f32) * TILE / 2.0 - TILE / 2.0;

    let x = left + (c as f32) * TILE;
    let y = top - (r as f32) * TILE;
    Vec3::new(x, y, 0.0)
}

fn world_to_cell(p: Vec2) -> Option<(i32, i32)> {
    let left = -(COLS as f32) * TILE / 2.0;
    let top = (ROWS as f32) * TILE / 2.0;

    let c = ((p.x - left) / TILE).floor() as i32;
    let r = ((top - p.y) / TILE).floor() as i32;

    if r >= 0 && r < ROWS && c >= 0 && c < COLS {
        Some((r, c))
    } else {
        None
    }
}

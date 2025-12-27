use bevy::prelude::*;
use std::collections::HashSet;

const ROWS: i32 = 5;
const COLS: i32 = 9;
const TILE: f32 = 80.0;

fn main() {
    App::new()
        // 背景色（不然默认黑）
        .insert_resource(ClearColor(Color::srgb(0.06, 0.08, 0.06)))
        .insert_resource(OccupiedCells::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "PVZ demo (Bevy)".into(),
                resolution: (1000.0, 600.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, click_place_plant)
        .run();
}

#[derive(Component)]
struct Plant;

#[derive(Resource, Default)]
struct OccupiedCells(HashSet<(i32, i32)>);

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());

    // 生成草坪网格（Sprite 方块）
    for r in 0..ROWS {
        for c in 0..COLS {
            let pos = cell_center_world(r, c);

            // 交替两种绿色，方便看格子
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

    // 屏幕坐标 -> 世界坐标
    let Some(world_pos) = camera.viewport_to_world_2d(cam_transform, cursor) else {
        return;
    };

    // 世界坐标 -> 格子坐标
    let Some((r, c)) = world_to_cell(world_pos) else {
        return;
    };

    // 防止重复放同一格
    if occupied.0.contains(&(r, c)) {
        return;
    }
    occupied.0.insert((r, c));

    // 放一个“植物”（蓝色小方块）
    commands.spawn((
        Plant,
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

// 格子中心（世界坐标）
fn cell_center_world(r: i32, c: i32) -> Vec3 {
    let left = -(COLS as f32) * TILE / 2.0 + TILE / 2.0;
    let top = (ROWS as f32) * TILE / 2.0 - TILE / 2.0;

    let x = left + (c as f32) * TILE;
    let y = top - (r as f32) * TILE;
    Vec3::new(x, y, 0.0)
}

// 世界坐标 -> (行, 列)
// 返回 None 表示点在草坪外
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

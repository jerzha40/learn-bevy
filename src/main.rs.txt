use bevy::prelude::{
    App, Assets, Camera2d, Color, Commands, Component, DefaultPlugins, Image, Query, Res, ResMut,
    Sprite, Startup, Time, Transform, Update, Vec2, Window, With, default,
};

use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;

use worldio::{WorldTex, create_rgba32u, save};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Startup：只跑一次，通常用来“创建场景/初始化实体”
        .add_systems(Startup, (setup, setup_world))
        // Update：每帧跑，通常用来“更新逻辑”
        .add_systems(Update, (move_player, bounce_in_window, save_world_on_s))
        .run();
}

/// Component：挂在 Entity 上的数据（可以是“标签”或“属性”）
#[derive(Component)]
struct Player;

#[derive(Component)]
struct Velocity(Vec2);

fn setup_world(images: ResMut<Assets<Image>>, commands: Commands) {
    create_rgba32u(images, commands);
}

fn setup(mut commands: Commands) {
    // 摄像机（2D 必备）
    commands.spawn(Camera2d);

    // Entity = 一个“对象”，组件 = 它身上的“数据/标签”
    commands.spawn((
        Player,                            // 标签组件
        Velocity(Vec2::new(200.0, 120.0)), // 数据组件
        Sprite {
            color: Color::srgb(0.2, 0.9, 0.4),
            custom_size: Some(Vec2::new(40.0, 40.0)),
            ..default()
        },
        Transform::default(),
    ));
}

/// System：一个普通函数，Bevy 会“注入参数”
/// Query：按组件类型查找实体
/// Time：引擎自带资源，告诉你每帧的 delta time
fn move_player(time: Res<Time>, mut q: Query<(&Velocity, &mut Transform), With<Player>>) {
    let dt = time.delta_secs();
    for (vel, mut tf) in &mut q {
        tf.translation.x += vel.0.x * dt;
        tf.translation.y += vel.0.y * dt;
    }
}

/// 演示另一个 system：修改组件数据（速度反弹）
/// Window：读窗口大小
fn bounce_in_window(
    window_q: Query<&Window>,
    mut q: Query<(&mut Velocity, &Transform), With<Player>>,
) {
    let window = window_q.single().expect("找不到唯一 Window");
    let half_w = window.width() * 0.5;
    let half_h = window.height() * 0.5;

    for (mut vel, tf) in &mut q {
        let x = tf.translation.x;
        let y = tf.translation.y;

        // 简单边界反弹（留点边距）
        let margin = 20.0;
        if x > half_w - margin || x < -half_w + margin {
            vel.0.x = -vel.0.x;
        }
        if y > half_h - margin || y < -half_h + margin {
            vel.0.y = -vel.0.y;
        }
    }
}
fn save_world_on_s(
    keys: Res<ButtonInput<KeyCode>>,
    world: Option<Res<WorldTex>>,
    images: Res<Assets<Image>>,
) {
    if !keys.just_pressed(KeyCode::KeyS) {
        return;
    }
    save(world, images);
}

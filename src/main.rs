use bevy::prelude::{
    App, Assets, Camera2d, Color, Commands, Component, DefaultPlugins, Handle, Image, Query, Res,
    ResMut, Resource, Sprite, Startup, Time, Transform, Update, Vec2, Window, With, default, error,
    info, warn,
};

use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy_asset::RenderAssetUsages;

use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use std::fs;

#[derive(Resource, Clone)]
struct WorldTex(pub Handle<Image>);
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Startup：只跑一次，通常用来“创建场景/初始化实体”
        .add_systems(Startup, (setup, setup_world_texture))
        // Update：每帧跑，通常用来“更新逻辑”
        .add_systems(Update, (move_player, bounce_in_window, save_world_on_s))
        .run();
}

/// Component：挂在 Entity 上的数据（可以是“标签”或“属性”）
#[derive(Component)]
struct Player;

#[derive(Component)]
struct Velocity(Vec2);

fn setup_world_texture(mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    let w = 64;
    let h = 64;

    // RGBA32Uint: 每像素 16 bytes（4 * u32）
    // 用全 0 初始化：pixel = 16 个 0 字节
    let mut image = Image::new_fill(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0u8; 16],
        TextureFormat::Rgba32Uint,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    // 关键：让它能当 storage texture 给 compute 用
    image.texture_descriptor.usage = TextureUsages::COPY_DST
        | TextureUsages::COPY_SRC
        | TextureUsages::STORAGE_BINDING
        | TextureUsages::TEXTURE_BINDING;

    let handle = images.add(image);
    commands.insert_resource(WorldTex(handle));
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

    let Some(world) = world else {
        warn!("WorldTex 还没创建好");
        return;
    };

    let Some(img) = images.get(&world.0) else {
        warn!("Assets<Image> 里还拿不到 world image（可能还在准备中）");
        return;
    };

    // 只允许我们预期的格式
    if img.texture_descriptor.format != TextureFormat::Rgba32Uint {
        warn!("格式不是 Rgba32Uint：{:?}", img.texture_descriptor.format);
        return;
    }

    let w = img.texture_descriptor.size.width;
    let h = img.texture_descriptor.size.height;

    // img.data 是 Vec<u8>，里面按 RGBA32Uint 存：每像素 16 bytes
    // 我们把它原样写入 bin（前面加一个小头部）
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"CWLD"); // magic
    out.extend_from_slice(&1u32.to_le_bytes()); // version
    out.extend_from_slice(&(w as u32).to_le_bytes());
    out.extend_from_slice(&(h as u32).to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes()); // format_id: 1 = RGBA32Uint
    let data = img
        .data
        .as_ref()
        .expect("Image.data 为空：可能没有 CPU 侧数据可用");

    out.extend_from_slice(data);

    let path = "world.bin";
    if let Err(e) = fs::write(path, out) {
        error!("写入 {} 失败：{}", path, e);
    } else {
        info!("已保存 {}（{}x{} RGBA32Uint）", path, w, h);
    }
}

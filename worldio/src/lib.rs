use bevy::prelude::{Assets, Commands, Handle, Image, Res, ResMut, Resource, error, info, warn};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy_asset::RenderAssetUsages;
use std::fs;

#[derive(Resource, Clone)]
pub struct WorldTex(pub Handle<Image>);

pub fn create_rgba32u(mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    let w = 64;
    let h = 64;
    let pixel: [u8; 16] = [
        1, 0, 0, 0, // R = 1u32 (小端)
        2, 0, 0, 0, // G = 2u32
        3, 0, 0, 0, // B = 3u32
        4, 0, 0, 0, // A = 4u32
    ];
    // RGBA32Uint: 每像素 16 bytes（4 * u32）
    // 用全 0 初始化：pixel = 16 个 0 字节
    let mut image = Image::new_fill(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &pixel,
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

pub fn save(world: Option<Res<WorldTex>>, images: Res<Assets<Image>>) {
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

#[cfg(test)]
mod tests {
    use bevy::prelude::{App, Assets, Image, MinimalPlugins, Startup, Update};

    use crate::{create_rgba32u, save};

    #[test]
    fn it_works() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Assets<Image>>();
        // Startup：只跑一次，通常用来“创建场景/初始化实体”
        app.add_systems(Startup, create_rgba32u);
        // Update：每帧跑，通常用来“更新逻辑”
        app.add_systems(Update, save);
        app.world_mut().run_schedule(Startup);
        app.update();
    }
}

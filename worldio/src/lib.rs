use bevy::prelude::{Assets, Commands, Handle, Image, Res, ResMut, Resource, error, info, warn};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy_asset::RenderAssetUsages;
use std::{fs, io};

#[derive(Resource, Clone)]
pub struct WorldTex(pub Handle<Image>);

pub fn create_rgba32u(mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    let w = 64;
    let h = 64;
    let pixel: [u8; 16] = [
        255, 255, 127, 255, // R = 1u32 (小端)
        0, 0, 0, 0, // G = 2u32
        0, 0, 0, 0, // B = 3u32
        255, 0, 0, 0, // A = 4u32
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

use std::path::Path;
#[derive(Debug)]
pub struct WorldBin {
    pub w: u32,
    pub h: u32,
    pub rgba32u: Vec<[u32; 4]>, // 每像素 RGBA u32
}

// 读取 world.bin
pub fn read_world_bin(path: impl AsRef<Path>) -> io::Result<WorldBin> {
    let bytes = fs::read(path)?;

    // 最小头部长度：4 + 4*4 = 20
    if bytes.len() < 20 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file too small"));
    }

    if &bytes[0..4] != b"CWLD" {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "bad magic"));
    }

    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let w = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let h = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    let format_id = u32::from_le_bytes(bytes[16..20].try_into().unwrap());

    if version != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported version",
        ));
    }
    if format_id != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported format_id",
        ));
    }

    let expected = (w as usize)
        .checked_mul(h as usize)
        .and_then(|n| n.checked_mul(16))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "size overflow"))?;

    if bytes.len() != 20 + expected {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "size mismatch"));
    }

    let data = &bytes[20..];
    let mut out = Vec::with_capacity((w as usize) * (h as usize));

    for i in 0..(w as usize * h as usize) {
        let base = i * 16;
        let r = u32::from_le_bytes(data[base..base + 4].try_into().unwrap());
        let g = u32::from_le_bytes(data[base + 4..base + 8].try_into().unwrap());
        let b = u32::from_le_bytes(data[base + 8..base + 12].try_into().unwrap());
        let a = u32::from_le_bytes(data[base + 12..base + 16].try_into().unwrap());
        out.push([r, g, b, a]);
    }

    Ok(WorldBin { w, h, rgba32u: out })
}

// 可视化模式
pub enum VizMode {
    Binary,    // v==0 黑 else 白
    Normalize, // v/max 灰度
    HashColor, // id->hash color
    Test,
}

// u32 -> RGBA8 buffer
pub fn visualize_rgba32u(world: &WorldBin, mode: VizMode) -> Vec<u8> {
    let mut rgba8 = vec![0u8; (world.w as usize) * (world.h as usize) * 4];

    // 这里用 R 通道当“主值”，你也可以换成 G/B/A 或组合
    let max_v = if matches!(mode, VizMode::Normalize) {
        world.rgba32u.iter().map(|p| p[0]).max().unwrap_or(0)
    } else {
        0
    };

    for (i, px) in world.rgba32u.iter().enumerate() {
        println!("i={i} px={:02X?}", px);
        let bytes = px[0].to_le_bytes();
        println!("i={i} v={} le_bytes={:02X?}", px[0], bytes[0]);

        let v = px[0]; // 主值：R 通道
        let (r, g, b) = match mode {
            VizMode::Binary => {
                if v == 0 {
                    (0, 0, 0)
                } else {
                    (255, 255, 255)
                }
            }
            VizMode::Normalize => {
                if max_v == 0 {
                    (0, 0, 0)
                } else {
                    let t = (v as f64) / (max_v as f64);
                    let gray = (t * 255.0).clamp(0.0, 255.0) as u8;
                    (gray, gray, gray)
                }
            }
            VizMode::HashColor => {
                // 一个简单稳定的 hash -> RGB（同 id 同色）
                let mut x = v.wrapping_mul(0x9E3779B1);
                x ^= x >> 16;
                let r = (x & 0xFF) as u8;
                let g = ((x >> 8) & 0xFF) as u8;
                let b = ((x >> 16) & 0xFF) as u8;
                (r, g, b)
            }
            VizMode::Test => (255, 12, 12),
        };
        let r = px[0].to_le_bytes()[0]; // 主值：R 通道
        let g = px[1].to_le_bytes()[0]; // 主值：R 通道
        let b = px[2].to_le_bytes()[0]; // 主值：R 通道
        let a = px[3].to_le_bytes()[0]; // 主值：R 通道

        let o = i * 4;
        rgba8[o] = r;
        rgba8[o + 1] = g;
        rgba8[o + 2] = b;
        rgba8[o + 3] = a;
    }

    rgba8
}
// 写 png（需要 png crate）
pub fn write_png_rgba8(path: impl AsRef<Path>, w: u32, h: u32, rgba8: &[u8]) -> io::Result<()> {
    use std::fs::File;
    use std::io::BufWriter;

    let file = File::create(path)?;
    let wtr = BufWriter::new(file);

    let mut encoder = png::Encoder::new(wtr, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba8)?;
    Ok(())
}

// 总入口：bin -> png
pub fn bin_to_png(bin_path: &str, png_path: &str, mode: VizMode) -> io::Result<()> {
    let world = read_world_bin(bin_path)?;
    let rgba8 = visualize_rgba32u(&world, mode);
    write_png_rgba8(png_path, world.w, world.h, &rgba8)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use bevy::prelude::{App, Assets, Image, MinimalPlugins, Startup, Update};

    use crate::{VizMode, bin_to_png, create_rgba32u, save};

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
        let _ = bin_to_png("./world.bin", "./world.png", VizMode::Test);
    }
}

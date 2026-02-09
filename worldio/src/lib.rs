use bevy::prelude::{Assets, Commands, Handle, Image, ResMut, Resource};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy_asset::RenderAssetUsages;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}
#[derive(Resource, Clone)]
pub struct WorldTex(pub Handle<Image>);

pub fn setup_world_texture(mut images: ResMut<Assets<Image>>, mut commands: Commands) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// 你的 WorldTex：Rgba32Uint，所以这里是 texture_2d<u32>，用 textureLoad 读
@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var world_tex: texture_2d<u32>;

struct Params {
    mode: u32,   // 0=RawBytes(按 low byte 显示), 1=Binary, 2=HashColor, 3=Test
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var<uniform> params: Params;

fn byte01(x: u32) -> f32 {
    return f32(x & 255u) / 255.0;
}

fn hash_color(v: u32) -> vec3<f32> {
    var x = v * 0x9E3779B1u;
    x = x ^ (x >> 16u);
    return vec3<f32>(
        f32(x & 255u) / 255.0,
        f32((x >> 8u) & 255u) / 255.0,
        f32((x >> 16u) & 255u) / 255.0
    );
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let dim = textureDimensions(world_tex); // vec2<u32>

    // uv(0..1) -> 像素坐标
    // 这里把 y 翻一下，让纹理“正着”显示（你也可以去掉 1.0-）
    let fx = clamp(in.uv.x, 0.0, 0.999999);
    let fy = clamp(1.0 - in.uv.y, 0.0, 0.999999);

    let x = i32(fx * f32(dim.x));
    let y = i32(fy * f32(dim.y));

    let px: vec4<u32> = textureLoad(world_tex, vec2<i32>(x, y), 0);

    // 默认：按你 worldio 现在“真正输出”的方式（每个 u32 的低 8bit 当 rgba8）
    if (params.mode == 0u) {
        return vec4<f32>(byte01(px.x), byte01(px.y), byte01(px.z), byte01(px.w));
    }

    // Binary：用 R 通道判断 0/非0
    if (params.mode == 1u) {
        let v = px.x;
        let c = select(0.0, 1.0, v != 0u);
        return vec4<f32>(c, c, c, 1.0);
    }

    // HashColor：用 R 通道做稳定 hash 着色
    if (params.mode == 2u) {
        let rgb = hash_color(px.x);
        return vec4<f32>(rgb, 1.0);
    }

    // Test：纯红色
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}

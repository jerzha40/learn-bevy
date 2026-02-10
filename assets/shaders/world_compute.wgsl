struct Params {
    tick: vec4<u32>,
};

@group(0) @binding(0)
var world_tex : texture_storage_2d<rgba32uint, write>;

@group(0) @binding(1)
var<uniform> params : Params;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let v = params.tick.x;
    textureStore(world_tex, vec2<i32>(i32(id.x), i32(id.y)), vec4<u32>(v, id.x, id.y, 255u));
}

use bevy::prelude::{
    App, Assets, Camera2d, Color, Commands, Component, DefaultPlugins, Image, Query, Res, ResMut,
    Sprite, Startup, Time, Transform, Update, Vec2, Window, With, default,
};

use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;

use worldio::{WorldTex, create_rgba32u, save};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            WorldComputePlugin,
            Material2dPlugin::<WorldDisplayMaterial>::default(),
        ))
        .add_systems(Startup, (setup, setup_world, setup_world_view).chain())
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
// ---- compute shader plugin (Bevy 0.18) ----
use std::borrow::Cow;

use bevy::{
    core_pipeline::core_2d::graph::{Core2d, Node2d},
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_graph::{Node, NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel},
        render_resource::{
            binding_types::{texture_storage_2d, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::GpuImage,
    },
};

const WORLD_SHADER: &str = "shaders/world_compute.wgsl"; // assets/shaders/world_compute.wgsl
const WORKGROUP_SIZE: u32 = 8;

#[derive(Resource)]
struct ComputeTimer(Timer);

#[derive(Resource, Clone, Copy, ExtractResource, ShaderType)]
struct WorldComputeUniforms {
    tick: UVec4, // tick.x 每秒 +1
}
impl Default for WorldComputeUniforms {
    fn default() -> Self {
        Self { tick: UVec4::ZERO }
    }
}

fn tick_compute_every_second(
    time: Res<Time>,
    mut timer: ResMut<ComputeTimer>,
    mut u: ResMut<WorldComputeUniforms>,
) {
    timer.0.tick(time.delta());
    if timer.0.just_finished() {
        u.tick.x = u.tick.x.wrapping_add(1);
        println!("sdf");
    }
}

pub struct WorldComputePlugin;

impl Plugin for WorldComputePlugin {
    fn build(&self, app: &mut App) {
        // 主世界：每秒更新 tick
        app.insert_resource(ComputeTimer(Timer::from_seconds(1.0, TimerMode::Repeating)))
            .init_resource::<WorldComputeUniforms>()
            .add_systems(Update, tick_compute_every_second)
            // 把资源抽到 RenderApp（RenderWorld）
            .add_plugins((
                ExtractResourcePlugin::<worldio::WorldTex>::default(),
                ExtractResourcePlugin::<WorldComputeUniforms>::default(),
            ));

        let render_app = app.sub_app_mut(RenderApp);

        // RenderWorld：建 pipeline + 每帧准备 bind group
        render_app
            .add_systems(RenderStartup, init_world_compute_pipeline)
            .add_systems(
                Render,
                prepare_world_compute_bind_group.in_set(RenderSystems::PrepareBindGroups),
            )
            // 插入一个 RenderGraph Node：在 2D 主 pass 之前 dispatch
            .add_render_graph_node::<WorldComputeNode>(Core2d, WorldComputeLabel)
            .add_render_graph_edge(Core2d, WorldComputeLabel, Node2d::StartMainPass);
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct WorldComputeLabel;

#[derive(Resource)]
struct WorldComputePipeline {
    layout: BindGroupLayoutDescriptor,
    pipeline: CachedComputePipelineId,
}

fn init_world_compute_pipeline(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "WorldComputeLayout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_storage_2d(TextureFormat::Rgba32Uint, StorageTextureAccess::WriteOnly),
                uniform_buffer::<WorldComputeUniforms>(false),
            ),
        ),
    );

    let shader = asset_server.load(WORLD_SHADER);
    let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        layout: vec![layout.clone()],
        shader,
        entry_point: Some(Cow::from("main")),
        ..default()
    });

    commands.insert_resource(WorldComputePipeline { layout, pipeline });
}

#[derive(Resource)]
struct WorldComputeBindGroup {
    bind_group: BindGroup,
    size: Extent3d,
}

fn prepare_world_compute_bind_group(
    mut commands: Commands,
    pipeline: Res<WorldComputePipeline>,
    pipeline_cache: Res<PipelineCache>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    world_tex: Res<worldio::WorldTex>,
    uniforms: Res<WorldComputeUniforms>,
    render_device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let Some(gpu_image) = gpu_images.get(&world_tex.0) else {
        return; // 纹理还没在 GPU 侧准备好
    };

    // 注意：WorldComputeUniforms 我们让它 Copy 了，所以这里能直接 *uniforms
    let mut uniform_buffer = UniformBuffer::from(*uniforms);
    uniform_buffer.write_buffer(&render_device, &queue);

    let bind_group = render_device.create_bind_group(
        None,
        &pipeline_cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((&gpu_image.texture_view, &uniform_buffer)),
    );

    commands.insert_resource(WorldComputeBindGroup {
        bind_group,
        size: gpu_image.size,
    });
}

#[derive(Default)]
struct WorldComputeNode {
    last_tick: u32,
    run_this_frame: bool,
}

impl Node for WorldComputeNode {
    fn update(&mut self, world: &mut World) {
        let Some(u) = world.get_resource::<WorldComputeUniforms>() else {
            self.run_this_frame = false;
            return;
        };
        let tick = u.tick.x;

        self.run_this_frame = tick != self.last_tick;
        if self.run_this_frame {
            self.last_tick = tick;
        }
    }

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext<'_>,
        render_context: &mut RenderContext<'w>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        if !self.run_this_frame {
            return Ok(());
        }

        let Some(bg) = world.get_resource::<WorldComputeBindGroup>() else {
            return Ok(());
        };
        let Some(pipeline) = world.get_resource::<WorldComputePipeline>() else {
            return Ok(());
        };
        let Some(pipeline_cache) = world.get_resource::<PipelineCache>() else {
            return Ok(());
        };
        let Some(p) = pipeline_cache.get_compute_pipeline(pipeline.pipeline) else {
            return Ok(()); // pipeline 还在编译
        };

        let w = bg.size.width.max(1);
        let h = bg.size.height.max(1);
        let gx = (w + WORKGROUP_SIZE - 1) / WORKGROUP_SIZE;
        let gy = (h + WORKGROUP_SIZE - 1) / WORKGROUP_SIZE;

        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor::default());
        pass.set_pipeline(p);
        pass.set_bind_group(0, &bg.bind_group, &[]);
        pass.dispatch_workgroups(gx, gy, 1);

        Ok(())
    }
}
use bevy::{
    reflect::TypePath,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
    sprite_render::{Material2d, Material2dPlugin, MeshMaterial2d},
};

const WORLD_DISPLAY_SHADER: &str = "shaders/world_display.wgsl";

#[derive(Clone, Copy, Debug, ShaderType)]
struct WorldDisplayParams {
    mode: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct WorldDisplayMaterial {
    // 关键：sample_type = "u_int" 才能绑定 Uint 纹理
    #[texture(0, sample_type = "u_int")]
    world_tex: Handle<Image>,

    #[uniform(1)]
    params: WorldDisplayParams,
}

impl Material2d for WorldDisplayMaterial {
    fn fragment_shader() -> ShaderRef {
        WORLD_DISPLAY_SHADER.into()
    }
}

fn setup_world_view(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WorldDisplayMaterial>>,
    world: Res<WorldTex>,
) {
    // 一个大矩形，把 world texture 显示出来（z=-1 放到玩家后面）
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::default())),
        MeshMaterial2d(materials.add(WorldDisplayMaterial {
            world_tex: world.0.clone(),
            params: WorldDisplayParams {
                mode: 0, // 0=RawBytes，最像你 worldio 当前的输出
                _pad0: 0,
                _pad1: 0,
                _pad2: 0,
            },
        })),
        Transform::from_translation(Vec3::new(0.0, 0.0, -1.0)).with_scale(Vec3::splat(600.0)),
    ));
}

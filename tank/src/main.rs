use bevy::prelude::*;
use bevy::log::LogPlugin;
use bevy::render::{
    settings::{Backends, Dx12Compiler, InstanceFlags, WgpuSettings},
    RenderPlugin,
};

use tank::{
    Bullet, DummyTarget, Tank, TankTeam, Turret, bullet_visual_system, dummy_target_visual_system,
    spawn_bullet, spawn_dummy_target, spawn_tank, tank_visual_system,
};

const FIRE_COOLDOWN: f32 = 0.25;
const TANK_RADIUS: f32 = 22.0;
const BULLET_OUT_PAD: f32 = 60.0;

#[derive(Resource)]
struct WorldBounds {
    half_w: f32,
    half_h: f32,
}

#[derive(Component)]
struct PlayerControlled;

#[derive(Component)]
struct FireCooldown(Timer);

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.07, 0.07, 0.09)))
        .insert_resource(WorldBounds {
            half_w: 400.0,
            half_h: 250.0,
        })
        .add_plugins(
            DefaultPlugins
                .set(LogPlugin {
                    filter: "wgpu_core=error,wgpu_hal=error,naga=warn".into(),
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: WgpuSettings {
                        backends: Some(Backends::DX12),
                        dx12_shader_compiler: Dx12Compiler::Fxc,
                        instance_flags: InstanceFlags::empty(),
                        ..default()
                    }
                    .into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Tank prefab demo".into(),
                        resolution: (900.0, 600.0).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                tank_visual_system,
                bullet_visual_system,
                dummy_target_visual_system,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                tank_move_system,
                turret_aim_system,
                tank_fire_system,
                bullet_move_system,
                bullet_cleanup_system,
                bullet_hit_dummy_target_system,
            ),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());

    let tank = spawn_tank(&mut commands, Vec3::new(0.0, 0.0, 0.0), TankTeam::Player);
    let mut fire_timer = Timer::from_seconds(FIRE_COOLDOWN, TimerMode::Repeating);
    fire_timer.set_elapsed(fire_timer.duration());

    commands
        .entity(tank)
        .insert(PlayerControlled)
        .insert(FireCooldown(fire_timer));

    spawn_dummy_target(&mut commands, Vec3::new(200.0, 0.0, 0.0));
}

fn tank_move_system(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    bounds: Res<WorldBounds>,
    mut q: Query<(&Tank, &mut Transform), With<PlayerControlled>>,
) {
    for (tank, mut t) in &mut q {
        let mut dir = Vec2::ZERO;
        if keys.pressed(KeyCode::KeyW) {
            dir.y += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) {
            dir.y -= 1.0;
        }
        if keys.pressed(KeyCode::KeyA) {
            dir.x -= 1.0;
        }
        if keys.pressed(KeyCode::KeyD) {
            dir.x += 1.0;
        }

        if dir.length_squared() > 0.0001 {
            let d = dir.normalize() * tank.move_speed * time.delta_seconds();
            t.translation.x += d.x;
            t.translation.y += d.y;
        }

        let max_x = bounds.half_w - TANK_RADIUS;
        let max_y = bounds.half_h - TANK_RADIUS;
        t.translation.x = t.translation.x.clamp(-max_x, max_x);
        t.translation.y = t.translation.y.clamp(-max_y, max_y);
    }
}

fn turret_aim_system(
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
    tanks: Query<&GlobalTransform, With<PlayerControlled>>,
    mut turrets: Query<(&Parent, &mut Transform), With<Turret>>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    let Ok((camera, cam_transform)) = cam_q.get_single() else {
        return;
    };
    let Some(world_pos) = camera.viewport_to_world_2d(cam_transform, cursor) else {
        return;
    };

    for (parent, mut turret_t) in &mut turrets {
        let Ok(tank_t) = tanks.get(parent.get()) else {
            continue;
        };
        let dir = world_pos - tank_t.translation().truncate();
        if dir.length_squared() < 0.0001 {
            continue;
        }
        let angle = dir.y.atan2(dir.x) - std::f32::consts::FRAC_PI_2;
        turret_t.rotation = Quat::from_rotation_z(angle);
    }
}

fn tank_fire_system(
    mut commands: Commands,
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
    mut q: Query<(&Transform, &mut FireCooldown), With<PlayerControlled>>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    let Ok((camera, cam_transform)) = cam_q.get_single() else {
        return;
    };
    let Some(world_pos) = camera.viewport_to_world_2d(cam_transform, cursor) else {
        return;
    };

    for (t, mut cooldown) in &mut q {
        cooldown.0.tick(time.delta());

        if !mouse.just_pressed(MouseButton::Left) {
            continue;
        }
        if !cooldown.0.finished() {
            continue;
        }

        let dir = world_pos - t.translation.truncate();
        if dir.length_squared() < 0.0001 {
            continue;
        }

        let dir = dir.normalize();
        let spawn = t.translation + (dir * 26.0).extend(1.0);
        spawn_bullet(&mut commands, spawn, dir);
        cooldown.0.reset();
    }
}

fn bullet_move_system(time: Res<Time>, mut q: Query<(&Bullet, &mut Transform)>) {
    for (b, mut t) in &mut q {
        let d = b.velocity * time.delta_seconds();
        t.translation.x += d.x;
        t.translation.y += d.y;
    }
}

fn bullet_cleanup_system(
    mut commands: Commands,
    bounds: Res<WorldBounds>,
    q: Query<(Entity, &Transform), With<Bullet>>,
) {
    let max_x = bounds.half_w + BULLET_OUT_PAD;
    let max_y = bounds.half_h + BULLET_OUT_PAD;
    for (e, t) in &q {
        if t.translation.x.abs() > max_x || t.translation.y.abs() > max_y {
            commands.entity(e).despawn();
        }
    }
}

fn bullet_hit_dummy_target_system(
    mut commands: Commands,
    bullets: Query<(Entity, &Bullet, &Transform)>,
    mut targets: Query<(Entity, &mut DummyTarget, &Transform)>,
) {
    for (b_e, b, bt) in &bullets {
        let bpos = bt.translation.truncate();
        for (t_e, mut t, tt) in &mut targets {
            let tpos = tt.translation.truncate();
            let hit_r = b.radius + t.radius;
            if bpos.distance_squared(tpos) <= hit_r * hit_r {
                commands.entity(b_e).despawn();
                t.hp -= b.damage;
                if t.hp <= 0 {
                    commands.entity(t_e).despawn();
                }
                break;
            }
        }
    }
}

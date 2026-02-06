use bevy::prelude::*;

use tank::{spawn_tank, TankTeam};

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.07, 0.07, 0.09)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Tank prefab demo".into(),
                resolution: (900.0, 600.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
    spawn_tank(&mut commands, Vec3::new(0.0, 0.0, 0.0), TankTeam::Player);
}

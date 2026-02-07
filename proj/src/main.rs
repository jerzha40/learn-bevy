use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::window::{ExitCondition, PresentMode};
mod inventory;
#[path = "inventoryAvatars/mod.rs"]
mod inventory_avatars;
mod item;
mod projectile;
mod portal;
mod save;
mod tank;
mod windowblob;

const BASE_WINDOW_TITLE: &str = windowblob::BASE_WINDOW_TITLE;

fn main() {
    let startup_prefab = save::load_main_blob_prefab_for_startup();
    let primary_window =
        windowblob::WindowBlobWindowBundle::from_prefab(windowblob::MAIN_BLOB_INSTANCE_ID, &startup_prefab).window;

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                present_mode: PresentMode::AutoNoVsync,
                ..primary_window
            }),
            close_when_requested: false,
            exit_condition: ExitCondition::DontExit,
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins(windowblob::WindowBlobPlugin)
        .add_plugins(save::SavePlugin)
        .add_plugins(item::ItemPlugin)
        .add_plugins(tank::TankPlugin)
        .add_plugins(inventory::InventoryPlugin)
        .add_plugins(inventory_avatars::InventoryAvatarsPlugin)
        .add_plugins(inventory::crafting::CraftingPlugin)
        .add_plugins(portal::PortalPlugin)
        .add_plugins(projectile::ProjectilePlugin)
        .add_systems(Update, update_window_title_with_fps)
        .run();
}

fn update_window_title_with_fps(
    diagnostics: Res<DiagnosticsStore>,
    mut windows: Query<(&windowblob::BlobWindow, &mut Window)>,
) {
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed());

    for (blob_window, mut window) in &mut windows {
        let blob_label = format!("{} [{}]", BASE_WINDOW_TITLE, blob_window.save_file);
        if let Some(fps) = fps {
            window.title = format!("{blob_label} | FPS: {:.0}", fps);
        } else {
            window.title = blob_label;
        }
    }
}

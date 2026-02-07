use bevy::log::warn;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;

use crate::inventory::{
    Inventory, OpenInventoryState, PlacementStage, SelectedAvatarForPlacement,
    SelectedAvatarPlacement,
};
use crate::item::{BaseItem, Item, ItemBundle, default_item_radius_for_archetype, metadata_from_archetype};
use crate::tank::Tank;
use crate::windowblob::{
    BlobCamera, BlobInstanceId, BlobRenderLayer, BlobWindow, FocusedBlobInstance,
};

const INVENTORY_PANEL_PADDING_BM: f32 = 0.08;
const INVENTORY_SLOT_SIZE_BM: f32 = 0.42;
const INVENTORY_SLOT_GAP_BM: f32 = 0.06;
const INVENTORY_PANEL_Z: f32 = 50.0;
const INVENTORY_SLOT_Z: f32 = 0.1;
const INVENTORY_AVATAR_Z: f32 = 0.01;
const INVENTORY_PREVIEW_Z: f32 = 52.0;

pub struct InventoryAvatarsPlugin;

impl Plugin for InventoryAvatarsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoveredInventorySlot>()
            .add_systems(
                Update,
                (
                    sync_inventory_ui_root,
                    update_hovered_slot_and_close_inventory,
                    select_inventory_slot_on_click,
                    update_inventory_slot_visuals,
                    ensure_preview_entity_for_active_selection,
                    update_preview_transform,
                    confirm_or_cancel_preview_placement,
                )
                    .chain(),
            );
    }
}

#[derive(Resource, Debug, Default)]
pub struct HoveredInventorySlot(pub Option<usize>);

#[derive(Component, Debug)]
pub struct InventoryUiRoot {
    pub owner_tank_entity: Entity,
    pub blob_instance_id: u32,
    pub width: u16,
    pub height: u16,
    pub panel_size_bm: Vec2,
}

#[derive(Component, Debug)]
pub struct InventorySlotVisual {
    pub index: usize,
}

#[derive(Component, Debug)]
pub struct InventorySlotAvatarVisual;

#[derive(Component, Debug)]
pub struct InventoryPlacementPreview;

fn sync_inventory_ui_root(
    mut commands: Commands,
    mut open_inventory_state: ResMut<OpenInventoryState>,
    mut hovered_slot: ResMut<HoveredInventorySlot>,
    owner_tanks: Query<(&Inventory, &BlobInstanceId, &BlobRenderLayer), With<Tank>>,
    ui_roots: Query<&InventoryUiRoot>,
) {
    if !open_inventory_state.is_open() {
        if let Some(existing_root) = open_inventory_state.ui_root_entity.take() {
            commands.entity(existing_root).despawn_recursive();
        }
        hovered_slot.0 = None;
        return;
    }

    let Some(owner_tank_entity) = open_inventory_state.owner_tank_entity else {
        open_inventory_state.close();
        hovered_slot.0 = None;
        if let Some(existing_root) = open_inventory_state.ui_root_entity.take() {
            commands.entity(existing_root).despawn_recursive();
        }
        return;
    };

    let Ok((inventory, blob_instance, blob_layer)) = owner_tanks.get(owner_tank_entity) else {
        open_inventory_state.close();
        hovered_slot.0 = None;
        if let Some(existing_root) = open_inventory_state.ui_root_entity.take() {
            commands.entity(existing_root).despawn_recursive();
        }
        return;
    };

    let needs_rebuild = match open_inventory_state.ui_root_entity {
        Some(existing_root) => match ui_roots.get(existing_root) {
            Ok(ui_root) => {
                ui_root.owner_tank_entity != owner_tank_entity
                    || ui_root.blob_instance_id != blob_instance.0
                    || ui_root.width != inventory.meta.width
                    || ui_root.height != inventory.meta.height
            }
            Err(_) => true,
        },
        None => true,
    };

    if !needs_rebuild {
        return;
    }

    if let Some(existing_root) = open_inventory_state.ui_root_entity.take() {
        commands.entity(existing_root).despawn_recursive();
    }

    let spawned_root = spawn_inventory_ui(
        &mut commands,
        owner_tank_entity,
        *blob_instance,
        *blob_layer,
        inventory,
    );
    open_inventory_state.ui_root_entity = Some(spawned_root);
}

fn spawn_inventory_ui(
    commands: &mut Commands,
    owner_tank_entity: Entity,
    blob_instance: BlobInstanceId,
    blob_layer: BlobRenderLayer,
    inventory: &Inventory,
) -> Entity {
    let panel_size = inventory_panel_size(inventory.meta.width, inventory.meta.height);
    let root_entity = commands
        .spawn((
            InventoryUiRoot {
                owner_tank_entity,
                blob_instance_id: blob_instance.0,
                width: inventory.meta.width,
                height: inventory.meta.height,
                panel_size_bm: panel_size,
            },
            blob_instance,
            blob_layer,
            RenderLayers::layer(blob_layer.0),
            SpatialBundle::from_transform(Transform::from_xyz(0.0, 0.0, INVENTORY_PANEL_Z)),
        ))
        .id();

    let background_entity = commands
        .spawn((
            blob_instance,
            blob_layer,
            RenderLayers::layer(blob_layer.0),
            SpriteBundle {
                sprite: Sprite {
                    color: Color::srgba(0.05, 0.05, 0.06, 0.86),
                    custom_size: Some(panel_size),
                    ..default()
                },
                transform: Transform::from_xyz(0.0, 0.0, 0.0),
                ..default()
            },
        ))
        .id();
    commands.entity(root_entity).add_child(background_entity);

    for slot_index in 0..inventory.slot_count() {
        let slot_entity = commands
            .spawn((
                InventorySlotVisual { index: slot_index },
                blob_instance,
                blob_layer,
                RenderLayers::layer(blob_layer.0),
                SpriteBundle {
                    sprite: Sprite {
                        color: Color::srgba(0.2, 0.2, 0.22, 0.92),
                        custom_size: Some(Vec2::splat(INVENTORY_SLOT_SIZE_BM)),
                        ..default()
                    },
                    transform: Transform::from_translation(slot_local_translation(
                        slot_index,
                        inventory.meta.width,
                        inventory.meta.height,
                        panel_size,
                    )),
                    ..default()
                },
            ))
            .id();

        let avatar_entity = commands
            .spawn((
                InventorySlotAvatarVisual,
                blob_instance,
                blob_layer,
                RenderLayers::layer(blob_layer.0),
                SpriteBundle {
                    sprite: Sprite {
                        color: Color::srgba(1.0, 1.0, 1.0, 0.9),
                        custom_size: Some(Vec2::splat(INVENTORY_SLOT_SIZE_BM * 0.74)),
                        ..default()
                    },
                    transform: Transform::from_xyz(0.0, 0.0, INVENTORY_AVATAR_Z),
                    visibility: Visibility::Hidden,
                    ..default()
                },
            ))
            .id();

        commands.entity(slot_entity).add_child(avatar_entity);
        commands.entity(root_entity).add_child(slot_entity);
    }

    root_entity
}

fn update_hovered_slot_and_close_inventory(
    mut open_inventory_state: ResMut<OpenInventoryState>,
    mut hovered_slot: ResMut<HoveredInventorySlot>,
    mut selected_avatar: ResMut<SelectedAvatarForPlacement>,
    ui_roots: Query<(&InventoryUiRoot, &GlobalTransform)>,
    windows: Query<(&Window, &BlobWindow)>,
    cameras: Query<(&Camera, &GlobalTransform, &BlobCamera), With<Camera2d>>,
) {
    if !open_inventory_state.is_open() {
        hovered_slot.0 = None;
        return;
    }

    let Some(ui_root_entity) = open_inventory_state.ui_root_entity else {
        hovered_slot.0 = None;
        return;
    };
    let Ok((ui_root, ui_root_transform)) = ui_roots.get(ui_root_entity) else {
        hovered_slot.0 = None;
        open_inventory_state.close();
        return;
    };

    let cursor_world =
        cursor_world_position_for_blob(ui_root.blob_instance_id, &windows, &cameras);
    let Some(cursor_world) = cursor_world else {
        hovered_slot.0 = None;
        return;
    };

    let local_cursor = cursor_world - ui_root_transform.translation().truncate();
    let cursor_inside_panel = local_cursor.x >= -ui_root.panel_size_bm.x * 0.5
        && local_cursor.x <= ui_root.panel_size_bm.x * 0.5
        && local_cursor.y >= -ui_root.panel_size_bm.y * 0.5
        && local_cursor.y <= ui_root.panel_size_bm.y * 0.5;

    if !cursor_inside_panel {
        hovered_slot.0 = None;
        open_inventory_state.close();

        if let Some(selection) = selected_avatar.0.as_mut() {
            if selection.owner_tank_entity == ui_root.owner_tank_entity
                && selection.stage == PlacementStage::PendingInventoryExit
            {
                selection.stage = PlacementStage::ActivePreview;
                selection.blob_instance_id = ui_root.blob_instance_id;
            }
        }
        return;
    }

    hovered_slot.0 = slot_index_from_local_cursor(local_cursor, ui_root);
}

fn select_inventory_slot_on_click(
    mouse_button: Res<ButtonInput<MouseButton>>,
    open_inventory_state: Res<OpenInventoryState>,
    hovered_slot: Res<HoveredInventorySlot>,
    ui_roots: Query<&InventoryUiRoot>,
    tank_inventories: Query<&Inventory, With<Tank>>,
    mut selected_avatar: ResMut<SelectedAvatarForPlacement>,
) {
    if !open_inventory_state.is_open() {
        return;
    }
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    let Some(slot_index) = hovered_slot.0 else {
        return;
    };
    let Some(owner_tank_entity) = open_inventory_state.owner_tank_entity else {
        return;
    };

    let Some(ui_root_entity) = open_inventory_state.ui_root_entity else {
        return;
    };
    let Ok(ui_root) = ui_roots.get(ui_root_entity) else {
        return;
    };

    let Ok(inventory) = tank_inventories.get(owner_tank_entity) else {
        return;
    };
    let Some(stack) = inventory
        .slot(slot_index)
        .and_then(|slot| slot.as_ref())
        .cloned()
    else {
        return;
    };
    if stack.quantity == 0 {
        return;
    }
    if !stack.is_allowed_in_inventory() {
        warn!(
            "Inventory slot {} contains unsupported item {}; selection ignored",
            slot_index, stack.item_archetype_id
        );
        return;
    }

    selected_avatar.0 = Some(SelectedAvatarPlacement {
        owner_tank_entity,
        slot_index,
        stack,
        blob_instance_id: ui_root.blob_instance_id,
        preview_entity: None,
        stage: PlacementStage::PendingInventoryExit,
    });
}

fn update_inventory_slot_visuals(
    open_inventory_state: Res<OpenInventoryState>,
    hovered_slot: Res<HoveredInventorySlot>,
    selected_avatar: Res<SelectedAvatarForPlacement>,
    tank_inventories: Query<&Inventory, With<Tank>>,
    mut slot_visuals: Query<
        (&Parent, &InventorySlotVisual, &mut Sprite, &Children),
        (With<InventorySlotVisual>, Without<InventorySlotAvatarVisual>),
    >,
    mut avatar_visuals: Query<
        (&mut Sprite, &mut Visibility),
        (With<InventorySlotAvatarVisual>, Without<InventorySlotVisual>),
    >,
) {
    let Some(ui_root_entity) = open_inventory_state.ui_root_entity else {
        return;
    };
    let Some(owner_tank_entity) = open_inventory_state.owner_tank_entity else {
        return;
    };

    let Ok(inventory) = tank_inventories.get(owner_tank_entity) else {
        return;
    };

    let selected_slot_index = selected_avatar
        .0
        .as_ref()
        .filter(|selection| selection.owner_tank_entity == owner_tank_entity)
        .map(|selection| selection.slot_index);

    for (parent, slot, mut slot_sprite, children) in &mut slot_visuals {
        if parent.get() != ui_root_entity {
            continue;
        }

        slot_sprite.color = if Some(slot.index) == hovered_slot.0 {
            Color::srgba(0.35, 0.35, 0.38, 0.98)
        } else if Some(slot.index) == selected_slot_index {
            Color::srgba(0.28, 0.34, 0.3, 0.98)
        } else {
            Color::srgba(0.2, 0.2, 0.22, 0.92)
        };

        let slot_stack = inventory.slot(slot.index).and_then(|value| value.as_ref());
        for child in children {
            let Ok((mut avatar_sprite, mut avatar_visibility)) = avatar_visuals.get_mut(*child)
            else {
                continue;
            };

            if let Some(stack) = slot_stack {
                avatar_sprite.color = Color::srgba(
                    stack.color_rgba[0].clamp(0.0, 1.0),
                    stack.color_rgba[1].clamp(0.0, 1.0),
                    stack.color_rgba[2].clamp(0.0, 1.0),
                    stack.color_rgba[3].clamp(0.15, 1.0),
                );
                *avatar_visibility = Visibility::Visible;
            } else {
                *avatar_visibility = Visibility::Hidden;
            }
        }
    }
}

fn ensure_preview_entity_for_active_selection(
    mut commands: Commands,
    focused_blob: Res<FocusedBlobInstance>,
    mut selected_avatar: ResMut<SelectedAvatarForPlacement>,
    owner_tanks: Query<(&BlobInstanceId, &BlobRenderLayer), With<Tank>>,
    preview_entities: Query<(), With<InventoryPlacementPreview>>,
) {
    let Some(selection) = selected_avatar.0.clone() else {
        return;
    };
    if selection.stage != PlacementStage::ActivePreview {
        return;
    }

    if focused_blob.0 != Some(selection.blob_instance_id) {
        selected_avatar.clear(&mut commands);
        return;
    }

    let Ok((owner_blob, owner_layer)) = owner_tanks.get(selection.owner_tank_entity) else {
        selected_avatar.clear(&mut commands);
        return;
    };
    if owner_blob.0 != selection.blob_instance_id {
        selected_avatar.clear(&mut commands);
        return;
    }

    if let Some(preview_entity) = selection.preview_entity {
        if preview_entities.get(preview_entity).is_ok() {
            return;
        }
    }

    let preview_radius = default_item_radius_for_archetype(&selection.stack.item_archetype_id);
    let preview_entity = commands
        .spawn((
            InventoryPlacementPreview,
            *owner_blob,
            *owner_layer,
            RenderLayers::layer(owner_layer.0),
            SpriteBundle {
                sprite: Sprite {
                    color: Color::srgba(
                        selection.stack.color_rgba[0].clamp(0.0, 1.0),
                        selection.stack.color_rgba[1].clamp(0.0, 1.0),
                        selection.stack.color_rgba[2].clamp(0.0, 1.0),
                        0.5,
                    ),
                    custom_size: Some(Vec2::splat(preview_radius * 2.0)),
                    ..default()
                },
                transform: Transform::from_xyz(0.0, 0.0, INVENTORY_PREVIEW_Z),
                ..default()
            },
        ))
        .id();

    if let Some(selection_mut) = selected_avatar.0.as_mut() {
        selection_mut.preview_entity = Some(preview_entity);
    }
}

fn update_preview_transform(
    windows: Query<(&Window, &BlobWindow)>,
    cameras: Query<(&Camera, &GlobalTransform, &BlobCamera), With<Camera2d>>,
    mut selected_avatar: ResMut<SelectedAvatarForPlacement>,
    mut preview_transforms: Query<&mut Transform, With<InventoryPlacementPreview>>,
) {
    let Some(selection) = selected_avatar.0.as_ref() else {
        return;
    };
    if selection.stage != PlacementStage::ActivePreview {
        return;
    }

    let Some(preview_entity) = selection.preview_entity else {
        return;
    };

    let Some(cursor_world) =
        cursor_world_position_for_blob(selection.blob_instance_id, &windows, &cameras)
    else {
        return;
    };

    let Ok(mut preview_transform) = preview_transforms.get_mut(preview_entity) else {
        if let Some(selection_mut) = selected_avatar.0.as_mut() {
            selection_mut.preview_entity = None;
        }
        return;
    };

    preview_transform.translation.x = cursor_world.x;
    preview_transform.translation.y = cursor_world.y;
}

fn confirm_or_cancel_preview_placement(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    focused_blob: Res<FocusedBlobInstance>,
    windows: Query<(&Window, &BlobWindow)>,
    cameras: Query<(&Camera, &GlobalTransform, &BlobCamera), With<Camera2d>>,
    mut owner_tanks: Query<(&BlobInstanceId, &BlobRenderLayer, &mut Inventory), With<Tank>>,
    mut selected_avatar: ResMut<SelectedAvatarForPlacement>,
) {
    let Some(mut selection) = selected_avatar.0.clone() else {
        return;
    };
    if selection.stage != PlacementStage::ActivePreview {
        return;
    }

    if keyboard_input.just_pressed(KeyCode::Escape)
        || mouse_button.just_pressed(MouseButton::Right)
    {
        selected_avatar.clear(&mut commands);
        return;
    }

    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    if focused_blob.0 != Some(selection.blob_instance_id) {
        return;
    }

    let Some(cursor_world) =
        cursor_world_position_for_blob(selection.blob_instance_id, &windows, &cameras)
    else {
        return;
    };

    let Ok((owner_blob, owner_layer, mut owner_inventory)) =
        owner_tanks.get_mut(selection.owner_tank_entity)
    else {
        selected_avatar.clear(&mut commands);
        return;
    };
    if owner_blob.0 != selection.blob_instance_id {
        selected_avatar.clear(&mut commands);
        return;
    }

    let Some(slot) = owner_inventory.slot_mut(selection.slot_index) else {
        selected_avatar.clear(&mut commands);
        return;
    };
    let Some(slot_stack) = slot.as_mut() else {
        selected_avatar.clear(&mut commands);
        return;
    };
    if slot_stack.quantity == 0 || !slot_stack.is_allowed_in_inventory() {
        *slot = None;
        selected_avatar.clear(&mut commands);
        return;
    }

    let (base_kind, sub_kind, ore_data) = metadata_from_archetype(&slot_stack.item_archetype_id);
    let item_radius = default_item_radius_for_archetype(&slot_stack.item_archetype_id);
    let item_color = slot_stack.color_rgba;

    let item_entity = commands
        .spawn(ItemBundle {
            item: Item,
            base_item: BaseItem {
                radius_bm: item_radius,
                color_rgba: item_color,
                base_kind,
                sub_kind,
            },
            blob_instance: *owner_blob,
            blob_render_layer: *owner_layer,
            spatial: SpatialBundle::from_transform(Transform::from_xyz(
                cursor_world.x,
                cursor_world.y,
                0.2,
            )),
        })
        .id();
    if let Some(ore_data) = ore_data {
        commands.entity(item_entity).insert(ore_data);
    }

    slot_stack.quantity = slot_stack.quantity.saturating_sub(1);
    if slot_stack.quantity == 0 {
        *slot = None;
        selected_avatar.clear(&mut commands);
        return;
    }

    selection.stack.quantity = slot_stack.quantity;
    selected_avatar.0 = Some(selection);
}

fn inventory_panel_size(width: u16, height: u16) -> Vec2 {
    let width_slots = f32::from(width);
    let height_slots = f32::from(height);
    Vec2::new(
        width_slots * INVENTORY_SLOT_SIZE_BM
            + (width_slots - 1.0).max(0.0) * INVENTORY_SLOT_GAP_BM
            + INVENTORY_PANEL_PADDING_BM * 2.0,
        height_slots * INVENTORY_SLOT_SIZE_BM
            + (height_slots - 1.0).max(0.0) * INVENTORY_SLOT_GAP_BM
            + INVENTORY_PANEL_PADDING_BM * 2.0,
    )
}

fn slot_local_translation(index: usize, width: u16, height: u16, panel_size: Vec2) -> Vec3 {
    let width_usize = usize::from(width);
    let height_usize = usize::from(height);
    if width_usize == 0 || height_usize == 0 {
        return Vec3::ZERO;
    }

    let col = index % width_usize;
    let row = index / width_usize;

    let left = -panel_size.x * 0.5 + INVENTORY_PANEL_PADDING_BM;
    let top = panel_size.y * 0.5 - INVENTORY_PANEL_PADDING_BM;

    let x = left + INVENTORY_SLOT_SIZE_BM * 0.5 + col as f32 * (INVENTORY_SLOT_SIZE_BM + INVENTORY_SLOT_GAP_BM);
    let y = top - INVENTORY_SLOT_SIZE_BM * 0.5 - row as f32 * (INVENTORY_SLOT_SIZE_BM + INVENTORY_SLOT_GAP_BM);

    Vec3::new(x, y, INVENTORY_SLOT_Z)
}

fn slot_index_from_local_cursor(local_cursor: Vec2, ui_root: &InventoryUiRoot) -> Option<usize> {
    let width = i32::from(ui_root.width);
    let height = i32::from(ui_root.height);
    if width <= 0 || height <= 0 {
        return None;
    }

    let left = -ui_root.panel_size_bm.x * 0.5 + INVENTORY_PANEL_PADDING_BM;
    let top = ui_root.panel_size_bm.y * 0.5 - INVENTORY_PANEL_PADDING_BM;
    let step = INVENTORY_SLOT_SIZE_BM + INVENTORY_SLOT_GAP_BM;

    let col_f = (local_cursor.x - left) / step;
    let row_f = (top - local_cursor.y) / step;
    if col_f < 0.0 || row_f < 0.0 {
        return None;
    }

    let col = col_f.floor() as i32;
    let row = row_f.floor() as i32;
    if col < 0 || col >= width || row < 0 || row >= height {
        return None;
    }

    let x_within = (local_cursor.x - left) - col as f32 * step;
    let y_within = (top - local_cursor.y) - row as f32 * step;
    if x_within > INVENTORY_SLOT_SIZE_BM || y_within > INVENTORY_SLOT_SIZE_BM {
        return None;
    }

    Some(row as usize * usize::from(ui_root.width) + col as usize)
}

fn cursor_world_position_for_blob(
    blob_instance_id: u32,
    windows: &Query<(&Window, &BlobWindow)>,
    cameras: &Query<(&Camera, &GlobalTransform, &BlobCamera), With<Camera2d>>,
) -> Option<Vec2> {
    let (window, _) = windows
        .iter()
        .find(|(_, blob_window)| blob_window.instance_id == blob_instance_id)?;
    let cursor_position = window.cursor_position()?;

    let (camera, camera_transform, _) = cameras
        .iter()
        .find(|(_, _, blob_camera)| blob_camera.instance_id == blob_instance_id)?;
    camera.viewport_to_world_2d(camera_transform, cursor_position)
}

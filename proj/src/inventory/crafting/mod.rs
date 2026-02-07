use bevy::prelude::*;
use bevy::render::view::RenderLayers;

use crate::inventory::{Inventory, InventoryAvatarStack, OpenInventoryState, SelectedAvatarForPlacement};
use crate::inventory_avatars::InventoryUiRoot;
use crate::item::{
    BASE_KIND_FACTORY, BaseItem, DrillData, Item, OreData, SelectableBaseItem, archetype_from_world_item,
    default_color_for_archetype, station_kind_for_world_item,
};
use crate::portal::{Portal, PortalInteractRequest};
use crate::tank::{FactionId, PLAYER_FACTION_ID, Tank, TankInteractionRadius};
use crate::windowblob::{BlobCamera, BlobInstanceId, BlobRenderLayer, BlobWindow, FocusedBlobInstance};

const ITEM_SELECTION_COLOR: [f32; 4] = [0.98, 0.94, 0.42, 0.35];
const CRAFTING_PANEL_WIDTH_BM: f32 = 2.1;
const CRAFTING_PANEL_HEIGHT_BM: f32 = 2.0;
const CRAFTING_PANEL_X_GAP_BM: f32 = 0.24;
const CRAFTING_BUTTON_WIDTH_BM: f32 = 1.76;
const CRAFTING_BUTTON_HEIGHT_BM: f32 = 0.34;
const CRAFTING_BUTTON_GAP_BM: f32 = 0.14;
const CRAFTING_PANEL_Z: f32 = 55.0;
const CRAFTING_BUTTON_Z: f32 = 0.1;

pub struct CraftingPlugin;

impl Plugin for CraftingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RecipeBook>()
            .init_resource::<ActiveInteractionTarget>()
            .init_resource::<HoveredInteractionTarget>()
            .init_resource::<CraftingPanelState>()
            .add_systems(
                PreUpdate,
                (
                    validate_active_interaction_target,
                    update_hovered_interaction_target,
                    process_left_click_world_interaction,
                    process_right_click_recycle_selected_factory,
                    sync_selected_marker_component,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    sync_item_selected_outline_visuals,
                    sync_crafting_panel_root,
                    update_crafting_button_visuals,
                    handle_crafting_button_click,
                )
                    .chain(),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionTargetKind {
    Portal,
    Item,
}

#[derive(Debug, Clone, Copy)]
pub struct InteractionTarget {
    pub entity: Entity,
    pub kind: InteractionTargetKind,
    pub blob_instance_id: u32,
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct ActiveInteractionTarget(pub Option<InteractionTarget>);

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct HoveredInteractionTarget(pub Option<InteractionTarget>);

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct SelectedMarker;

#[derive(Resource, Debug, Default)]
pub struct CraftingPanelState {
    pub active_station_entity: Option<Entity>,
    pub ui_root_entity: Option<Entity>,
}

#[derive(Resource, Debug, Clone)]
pub struct RecipeBook {
    pub recipes: Vec<Recipe>,
}

impl Default for RecipeBook {
    fn default() -> Self {
        Self {
            recipes: vec![Recipe {
                id: "drill_from_iron_ore".to_string(),
                station_kind: "drill".to_string(),
                inputs: vec![RecipeStack {
                    item_archetype_id: "material/ore/iron".to_string(),
                    quantity: 1,
                    color_rgba: default_color_for_archetype("material/ore/iron"),
                }],
                outputs: vec![RecipeStack {
                    item_archetype_id: "factory/drill/iron".to_string(),
                    quantity: 1,
                    color_rgba: default_color_for_archetype("factory/drill/iron"),
                }],
                craft_time_sec: 0.0,
            }],
        }
    }
}

impl RecipeBook {
    pub fn recipes_for_station<'a>(&'a self, station_kind: &str) -> Vec<&'a Recipe> {
        self.recipes
            .iter()
            .filter(|recipe| recipe.station_kind == station_kind)
            .collect()
    }

    pub fn recipe_by_id(&self, recipe_id: &str) -> Option<&Recipe> {
        self.recipes.iter().find(|recipe| recipe.id == recipe_id)
    }
}

#[derive(Debug, Clone)]
pub struct Recipe {
    pub id: String,
    pub station_kind: String,
    pub inputs: Vec<RecipeStack>,
    pub outputs: Vec<RecipeStack>,
    pub craft_time_sec: f32,
}

#[derive(Debug, Clone)]
pub struct RecipeStack {
    pub item_archetype_id: String,
    pub quantity: u32,
    pub color_rgba: [f32; 4],
}

#[derive(Component, Debug, Clone, Copy)]
struct ItemSelectedOutline;

#[derive(Component, Debug, Clone)]
struct CraftingPanelUiRoot {
    owner_tank_entity: Entity,
    station_entity: Entity,
    station_kind: String,
}

#[derive(Component, Debug, Clone)]
struct CraftingRecipeButton {
    recipe_id: String,
    size_bm: Vec2,
}

fn validate_active_interaction_target(
    focused_blob: Res<FocusedBlobInstance>,
    mut active_target: ResMut<ActiveInteractionTarget>,
    mut crafting_panel_state: ResMut<CraftingPanelState>,
    existing_entities: Query<Entity>,
) {
    if let Some(target) = active_target.0 {
        let target_exists = existing_entities.get(target.entity).is_ok();
        let in_focused_blob = focused_blob.0 == Some(target.blob_instance_id);
        if !target_exists || !in_focused_blob {
            active_target.0 = None;
            crafting_panel_state.active_station_entity = None;
        }
    }

    if let Some(station_entity) = crafting_panel_state.active_station_entity {
        if existing_entities.get(station_entity).is_err() {
            crafting_panel_state.active_station_entity = None;
        }
    }
}

fn update_hovered_interaction_target(
    focused_blob: Res<FocusedBlobInstance>,
    open_inventory_state: Res<OpenInventoryState>,
    selected_avatar: Res<SelectedAvatarForPlacement>,
    windows: Query<(&Window, &BlobWindow)>,
    cameras: Query<(&Camera, &GlobalTransform, &BlobCamera), With<Camera2d>>,
    player_tanks: Query<
        (Entity, &Transform, &BlobInstanceId, &FactionId, &TankInteractionRadius),
        With<Tank>,
    >,
    portals: Query<(Entity, &Transform, &Portal, &BlobInstanceId), With<Portal>>,
    items: Query<(Entity, &Transform, &BaseItem, &BlobInstanceId), (With<Item>, With<SelectableBaseItem>)>,
    mut hovered_target: ResMut<HoveredInteractionTarget>,
) {
    if open_inventory_state.is_open() || selected_avatar.is_active_preview() {
        hovered_target.0 = None;
        return;
    }

    let Some(player_context) = focused_player_context(&focused_blob, &player_tanks) else {
        hovered_target.0 = None;
        return;
    };
    let Some(cursor_world) =
        cursor_world_position_for_blob(player_context.blob_instance_id, &windows, &cameras)
    else {
        hovered_target.0 = None;
        return;
    };

    let mut nearest: Option<(InteractionTarget, f32)> = None;

    for (portal_entity, portal_transform, portal, portal_blob) in &portals {
        if portal_blob.0 != player_context.blob_instance_id {
            continue;
        }

        let portal_position = portal_transform.translation.truncate();
        let player_delta = player_context.position_bm - portal_position;
        if player_delta.length_squared()
            > player_context.interaction_radius_bm * player_context.interaction_radius_bm
        {
            continue;
        }

        let cursor_delta = cursor_world - portal_position;
        let cursor_distance_sq = cursor_delta.length_squared();
        if cursor_distance_sq > portal.radius_bm * portal.radius_bm {
            continue;
        }

        let target = InteractionTarget {
            entity: portal_entity,
            kind: InteractionTargetKind::Portal,
            blob_instance_id: portal_blob.0,
        };

        match nearest {
            Some((_, nearest_distance_sq)) if nearest_distance_sq <= cursor_distance_sq => {}
            _ => nearest = Some((target, cursor_distance_sq)),
        }
    }

    for (item_entity, item_transform, base_item, item_blob) in &items {
        if item_blob.0 != player_context.blob_instance_id {
            continue;
        }
        if base_item.base_kind != "natural" && base_item.base_kind != "factory" {
            continue;
        }

        let item_position = item_transform.translation.truncate();
        let player_delta = player_context.position_bm - item_position;
        if player_delta.length_squared()
            > player_context.interaction_radius_bm * player_context.interaction_radius_bm
        {
            continue;
        }

        let pick_radius = base_item.radius_bm.max(0.2);
        let cursor_delta = cursor_world - item_position;
        let cursor_distance_sq = cursor_delta.length_squared();
        if cursor_distance_sq > pick_radius * pick_radius {
            continue;
        }

        let target = InteractionTarget {
            entity: item_entity,
            kind: InteractionTargetKind::Item,
            blob_instance_id: item_blob.0,
        };

        match nearest {
            Some((_, nearest_distance_sq)) if nearest_distance_sq <= cursor_distance_sq => {}
            _ => nearest = Some((target, cursor_distance_sq)),
        }
    }

    hovered_target.0 = nearest.map(|(target, _)| target);
}

fn process_left_click_world_interaction(
    mouse_button: Res<ButtonInput<MouseButton>>,
    focused_blob: Res<FocusedBlobInstance>,
    mut open_inventory_state: ResMut<OpenInventoryState>,
    selected_avatar: Res<SelectedAvatarForPlacement>,
    hovered_target: Res<HoveredInteractionTarget>,
    mut active_target: ResMut<ActiveInteractionTarget>,
    mut crafting_panel_state: ResMut<CraftingPanelState>,
    player_tanks: Query<
        (Entity, &Transform, &BlobInstanceId, &FactionId, &TankInteractionRadius),
        With<Tank>,
    >,
    portal_targets: Query<(&Transform, &BlobInstanceId), With<Portal>>,
    item_targets: Query<
        (Entity, &Transform, &BaseItem, Option<&Inventory>, &BlobInstanceId),
        With<Item>,
    >,
    mut portal_requests: EventWriter<PortalInteractRequest>,
) {
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }
    if open_inventory_state.is_open() || selected_avatar.is_active_preview() {
        return;
    }

    let Some(player_context) = focused_player_context(&focused_blob, &player_tanks) else {
        return;
    };

    if let Some(active) = active_target.0 {
        let active_in_range = match active.kind {
            InteractionTargetKind::Portal => portal_targets
                .get(active.entity)
                .ok()
                .map(|(transform, blob_instance)| {
                    blob_instance.0 == player_context.blob_instance_id
                        && player_context
                            .position_bm
                            .distance_squared(transform.translation.truncate())
                            <= player_context.interaction_radius_bm
                                * player_context.interaction_radius_bm
                })
                .unwrap_or(false),
            InteractionTargetKind::Item => item_targets
                .get(active.entity)
                .ok()
                .map(|(_, transform, _, _, blob_instance)| {
                    blob_instance.0 == player_context.blob_instance_id
                        && player_context
                            .position_bm
                            .distance_squared(transform.translation.truncate())
                            <= player_context.interaction_radius_bm
                                * player_context.interaction_radius_bm
                })
                .unwrap_or(false),
        };

        if !active_in_range {
            active_target.0 = None;
            crafting_panel_state.active_station_entity = None;
        } else {
            match active.kind {
                InteractionTargetKind::Portal => {
                    portal_requests.send(PortalInteractRequest {
                        portal_entity: active.entity,
                        player_tank_entity: player_context.tank_entity,
                    });
                    return;
                }
                InteractionTargetKind::Item => {
                    let Ok((item_entity, _, base_item, maybe_item_inventory, _)) =
                        item_targets.get(active.entity)
                    else {
                        active_target.0 = None;
                        crafting_panel_state.active_station_entity = None;
                        return;
                    };

                    if station_kind_for_world_item(base_item).is_some() && maybe_item_inventory.is_some() {
                        open_inventory_state.open_for(player_context.tank_entity);
                        crafting_panel_state.active_station_entity = Some(item_entity);
                    } else {
                        crafting_panel_state.active_station_entity = None;
                    }
                    return;
                }
            }
        }
    }

    if let Some(hovered) = hovered_target.0 {
        active_target.0 = Some(hovered);
        crafting_panel_state.active_station_entity = None;
    } else {
        active_target.0 = None;
        crafting_panel_state.active_station_entity = None;
    }
}

fn process_right_click_recycle_selected_factory(
    mut commands: Commands,
    mouse_button: Res<ButtonInput<MouseButton>>,
    focused_blob: Res<FocusedBlobInstance>,
    open_inventory_state: Res<OpenInventoryState>,
    selected_avatar: Res<SelectedAvatarForPlacement>,
    mut active_target: ResMut<ActiveInteractionTarget>,
    mut crafting_panel_state: ResMut<CraftingPanelState>,
    mut player_tanks: Query<
        (
            Entity,
            &Transform,
            &BlobInstanceId,
            &FactionId,
            &TankInteractionRadius,
            &mut Inventory,
        ),
        With<Tank>,
    >,
    items: Query<
        (
            Entity,
            &Transform,
            &BaseItem,
            Option<&OreData>,
            Option<&DrillData>,
            &BlobInstanceId,
        ),
        With<Item>,
    >,
) {
    if !mouse_button.just_pressed(MouseButton::Right) {
        return;
    }
    if open_inventory_state.is_open() || selected_avatar.is_active_preview() {
        return;
    }

    let Some(active) = active_target.0 else {
        return;
    };
    if active.kind != InteractionTargetKind::Item {
        return;
    }

    let Some(focused_blob_id) = focused_blob.0 else {
        return;
    };

    let Some((_, player_position, _, _, interaction_radius, mut player_inventory)) =
        player_tanks.iter_mut().find(
            |(_, _, tank_blob, tank_faction, _, _)| {
                tank_blob.0 == focused_blob_id && tank_faction.0 == PLAYER_FACTION_ID
            },
        )
    else {
        return;
    };

    let Ok((item_entity, item_transform, base_item, ore_data, drill_data, item_blob)) =
        items.get(active.entity)
    else {
        active_target.0 = None;
        if crafting_panel_state.active_station_entity == Some(active.entity) {
            crafting_panel_state.active_station_entity = None;
        }
        return;
    };
    if item_blob.0 != focused_blob_id {
        return;
    }
    if base_item.base_kind != BASE_KIND_FACTORY {
        return;
    }

    let distance_sq = player_position.translation.truncate().distance_squared(item_transform.translation.truncate());
    if distance_sq > interaction_radius.radius_bm * interaction_radius.radius_bm {
        return;
    }

    let recycled_archetype = archetype_from_world_item(base_item, ore_data, drill_data);
    let recycled_stack = InventoryAvatarStack {
        item_archetype_id: recycled_archetype.clone(),
        quantity: 1,
        color_rgba: default_color_for_archetype(&recycled_archetype),
    };

    if player_inventory.try_add_stack(recycled_stack).is_err() {
        return;
    }

    commands.entity(item_entity).despawn_recursive();
    active_target.0 = None;
    if crafting_panel_state.active_station_entity == Some(item_entity) {
        crafting_panel_state.active_station_entity = None;
    }
}

fn sync_selected_marker_component(
    mut commands: Commands,
    active_target: Res<ActiveInteractionTarget>,
    selected_markers: Query<Entity, With<SelectedMarker>>,
) {
    let selected_entity = active_target.0.map(|target| target.entity);

    for marked_entity in &selected_markers {
        if Some(marked_entity) != selected_entity {
            commands.entity(marked_entity).remove::<SelectedMarker>();
        }
    }

    if let Some(selected_entity) = selected_entity {
        commands.entity(selected_entity).insert(SelectedMarker);
    }
}

fn sync_item_selected_outline_visuals(
    mut commands: Commands,
    items: Query<
        (
            Entity,
            &BaseItem,
            &BlobRenderLayer,
            Option<&SelectedMarker>,
            Option<&Children>,
        ),
        With<Item>,
    >,
    existing_outlines: Query<Entity, With<ItemSelectedOutline>>,
) {
    for (item_entity, base_item, blob_layer, selected, children) in &items {
        let existing_outline = children.and_then(|children| {
            children
                .iter()
                .find(|child| existing_outlines.get(**child).is_ok())
                .copied()
        });

        if selected.is_some() {
            if existing_outline.is_none() {
                let outline = commands
                    .spawn((
                        ItemSelectedOutline,
                        *blob_layer,
                        RenderLayers::layer(blob_layer.0),
                        SpriteBundle {
                            sprite: Sprite {
                                color: Color::srgba(
                                    ITEM_SELECTION_COLOR[0],
                                    ITEM_SELECTION_COLOR[1],
                                    ITEM_SELECTION_COLOR[2],
                                    ITEM_SELECTION_COLOR[3],
                                ),
                                custom_size: Some(Vec2::splat(base_item.radius_bm * 2.8)),
                                ..default()
                            },
                            transform: Transform::from_xyz(0.0, 0.0, 0.15),
                            ..default()
                        },
                    ))
                    .id();
                commands.entity(item_entity).add_child(outline);
            }
        } else if let Some(outline_entity) = existing_outline {
            commands.entity(outline_entity).despawn_recursive();
        }
    }
}

fn sync_crafting_panel_root(
    mut commands: Commands,
    open_inventory_state: Res<OpenInventoryState>,
    recipe_book: Res<RecipeBook>,
    mut crafting_panel_state: ResMut<CraftingPanelState>,
    inventory_ui_roots: Query<(&InventoryUiRoot, &GlobalTransform)>,
    stations: Query<(Entity, &BaseItem, &BlobRenderLayer, &Inventory), With<Item>>,
    panel_roots: Query<&CraftingPanelUiRoot>,
) {
    let should_show = if !open_inventory_state.is_open() {
        None
    } else if let (Some(owner_tank_entity), Some(station_entity), Some(inventory_ui_root_entity)) = (
        open_inventory_state.owner_tank_entity,
        crafting_panel_state.active_station_entity,
        open_inventory_state.ui_root_entity,
    ) {
        if let (Ok((_, inventory_ui_root_transform)), Ok((_, station_base_item, station_blob_layer, _))) = (
            inventory_ui_roots.get(inventory_ui_root_entity),
            stations.get(station_entity),
        ) {
            if let Some(station_kind) = station_kind_for_world_item(station_base_item) {
                let recipes = recipe_book.recipes_for_station(station_kind);
                if !recipes.is_empty() {
                    Some((
                        owner_tank_entity,
                        station_entity,
                        station_kind.to_string(),
                        *station_blob_layer,
                        inventory_ui_root_transform.translation().truncate(),
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let Some((owner_tank_entity, station_entity, station_kind, station_blob_layer, panel_anchor)) = should_show else {
        if let Some(existing_panel_root) = crafting_panel_state.ui_root_entity.take() {
            commands.entity(existing_panel_root).despawn_recursive();
        }
        return;
    };

    let needs_rebuild = match crafting_panel_state.ui_root_entity {
        Some(root_entity) => match panel_roots.get(root_entity) {
            Ok(root) => {
                root.owner_tank_entity != owner_tank_entity
                    || root.station_entity != station_entity
                    || root.station_kind != station_kind
            }
            Err(_) => true,
        },
        None => true,
    };

    if needs_rebuild {
        if let Some(existing_root) = crafting_panel_state.ui_root_entity.take() {
            commands.entity(existing_root).despawn_recursive();
        }

        let panel_position = panel_anchor
            + Vec2::new(CRAFTING_PANEL_WIDTH_BM * 0.5 + CRAFTING_PANEL_X_GAP_BM + 2.1, 0.0);
        let root_entity = commands
            .spawn((
                CraftingPanelUiRoot {
                    owner_tank_entity,
                    station_entity,
                    station_kind: station_kind.clone(),
                },
                station_blob_layer,
                RenderLayers::layer(station_blob_layer.0),
                SpatialBundle::from_transform(Transform::from_xyz(
                    panel_position.x,
                    panel_position.y,
                    CRAFTING_PANEL_Z,
                )),
            ))
            .id();

        let background_entity = commands
            .spawn((
                station_blob_layer,
                RenderLayers::layer(station_blob_layer.0),
                SpriteBundle {
                    sprite: Sprite {
                        color: Color::srgba(0.06, 0.06, 0.08, 0.88),
                        custom_size: Some(Vec2::new(CRAFTING_PANEL_WIDTH_BM, CRAFTING_PANEL_HEIGHT_BM)),
                        ..default()
                    },
                    ..default()
                },
            ))
            .id();
        commands.entity(root_entity).add_child(background_entity);

        let recipes = recipe_book.recipes_for_station(&station_kind);
        let total_height = recipes.len() as f32 * CRAFTING_BUTTON_HEIGHT_BM
            + recipes.len().saturating_sub(1) as f32 * CRAFTING_BUTTON_GAP_BM;
        let mut y = total_height * 0.5 - CRAFTING_BUTTON_HEIGHT_BM * 0.5;

        for recipe in recipes {
            let button_entity = commands
                .spawn((
                    CraftingRecipeButton {
                        recipe_id: recipe.id.clone(),
                        size_bm: Vec2::new(CRAFTING_BUTTON_WIDTH_BM, CRAFTING_BUTTON_HEIGHT_BM),
                    },
                    station_blob_layer,
                    RenderLayers::layer(station_blob_layer.0),
                    SpriteBundle {
                        sprite: Sprite {
                            color: Color::srgba(0.32, 0.32, 0.34, 0.95),
                            custom_size: Some(Vec2::new(CRAFTING_BUTTON_WIDTH_BM, CRAFTING_BUTTON_HEIGHT_BM)),
                            ..default()
                        },
                        transform: Transform::from_xyz(0.0, y, CRAFTING_BUTTON_Z),
                        ..default()
                    },
                ))
                .id();
            commands.entity(root_entity).add_child(button_entity);
            y -= CRAFTING_BUTTON_HEIGHT_BM + CRAFTING_BUTTON_GAP_BM;
        }

        crafting_panel_state.ui_root_entity = Some(root_entity);
    }
}

fn update_crafting_button_visuals(
    recipe_book: Res<RecipeBook>,
    panel_roots: Query<&CraftingPanelUiRoot>,
    station_inventories: Query<&Inventory, With<Item>>,
    mut recipe_buttons: Query<(&Parent, &CraftingRecipeButton, &mut Sprite), With<CraftingRecipeButton>>,
) {
    for (parent, button, mut button_sprite) in &mut recipe_buttons {
        let Ok(panel_root) = panel_roots.get(parent.get()) else {
            continue;
        };
        let Some(recipe) = recipe_book.recipe_by_id(&button.recipe_id) else {
            continue;
        };
        let Ok(station_inventory) = station_inventories.get(panel_root.station_entity) else {
            continue;
        };

        let craftable = can_craft_recipe(station_inventory, recipe);
        button_sprite.color = if craftable {
            Color::srgba(0.26, 0.62, 0.32, 0.96)
        } else {
            Color::srgba(0.32, 0.32, 0.34, 0.95)
        };
    }
}

fn handle_crafting_button_click(
    mouse_button: Res<ButtonInput<MouseButton>>,
    focused_blob: Res<FocusedBlobInstance>,
    open_inventory_state: Res<OpenInventoryState>,
    recipe_book: Res<RecipeBook>,
    windows: Query<(&Window, &BlobWindow)>,
    cameras: Query<(&Camera, &GlobalTransform, &BlobCamera), With<Camera2d>>,
    panel_roots: Query<&CraftingPanelUiRoot>,
    recipe_buttons: Query<(&Parent, &GlobalTransform, &CraftingRecipeButton), With<CraftingRecipeButton>>,
    mut station_inventories: Query<&mut Inventory, With<Item>>,
) {
    if !mouse_button.just_pressed(MouseButton::Left) || !open_inventory_state.is_open() {
        return;
    }
    let Some(focused_blob_id) = focused_blob.0 else {
        return;
    };
    let Some(cursor_world) = cursor_world_position_for_blob(focused_blob_id, &windows, &cameras) else {
        return;
    };

    let mut clicked: Option<(Entity, String)> = None;
    for (parent, button_transform, button) in &recipe_buttons {
        let button_pos = button_transform.translation().truncate();
        let half = button.size_bm * 0.5;
        let local = cursor_world - button_pos;
        if local.x.abs() <= half.x && local.y.abs() <= half.y {
            clicked = Some((parent.get(), button.recipe_id.clone()));
            break;
        }
    }

    let Some((panel_root_entity, recipe_id)) = clicked else {
        return;
    };
    let Ok(panel_root) = panel_roots.get(panel_root_entity) else {
        return;
    };
    let Some(recipe) = recipe_book.recipe_by_id(&recipe_id) else {
        return;
    };

    let Ok(mut station_inventory) = station_inventories.get_mut(panel_root.station_entity) else {
        return;
    };
    if !can_craft_recipe(&station_inventory, recipe) {
        return;
    }
    let _ = perform_craft_recipe(&mut station_inventory, recipe);
}

fn can_craft_recipe(inventory: &Inventory, recipe: &Recipe) -> bool {
    let _craft_time_sec = recipe.craft_time_sec;

    if recipe
        .inputs
        .iter()
        .any(|input| inventory.count_item(&input.item_archetype_id) < input.quantity)
    {
        return false;
    }

    !recipe.outputs.iter().any(|output| {
        !inventory.can_accept_stack(&InventoryAvatarStack {
            item_archetype_id: output.item_archetype_id.clone(),
            quantity: output.quantity,
            color_rgba: output.color_rgba,
        })
    })
}

fn perform_craft_recipe(inventory: &mut Inventory, recipe: &Recipe) -> Result<(), String> {
    if !can_craft_recipe(inventory, recipe) {
        return Err(format!("recipe {} is not craftable", recipe.id));
    }

    for input in &recipe.inputs {
        inventory.try_remove_quantity(&input.item_archetype_id, input.quantity)?;
    }

    for output in &recipe.outputs {
        inventory.try_add_stack(InventoryAvatarStack {
            item_archetype_id: output.item_archetype_id.clone(),
            quantity: output.quantity,
            color_rgba: output.color_rgba,
        })?;
    }

    Ok(())
}

struct PlayerInteractionContext {
    tank_entity: Entity,
    blob_instance_id: u32,
    position_bm: Vec2,
    interaction_radius_bm: f32,
}

fn focused_player_context(
    focused_blob: &FocusedBlobInstance,
    player_tanks: &Query<
        (Entity, &Transform, &BlobInstanceId, &FactionId, &TankInteractionRadius),
        With<Tank>,
    >,
) -> Option<PlayerInteractionContext> {
    let focused_blob_id = focused_blob.0?;

    player_tanks
        .iter()
        .find(|(_, _, blob_instance, faction, _)| {
            blob_instance.0 == focused_blob_id && faction.0 == PLAYER_FACTION_ID
        })
        .map(
            |(tank_entity, tank_transform, blob_instance, _, interaction_radius)| {
                PlayerInteractionContext {
                    tank_entity,
                    blob_instance_id: blob_instance.0,
                    position_bm: tank_transform.translation.truncate(),
                    interaction_radius_bm: interaction_radius.radius_bm,
                }
            },
        )
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

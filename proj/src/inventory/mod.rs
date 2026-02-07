use bevy::log::warn;
use bevy::prelude::*;

use crate::item::{BASE_KIND_NATURAL, base_kind_from_archetype};
use crate::tank::{FactionId, PLAYER_FACTION_ID, Tank};
use crate::windowblob::{BlobInstanceId, FocusedBlobInstance};

pub const DEFAULT_INVENTORY_WIDTH: u16 = 9;
pub const DEFAULT_INVENTORY_HEIGHT: u16 = 4;
pub const INVENTORY_TOGGLE_KEY: KeyCode = KeyCode::KeyE;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OpenInventoryState>()
            .init_resource::<SelectedAvatarForPlacement>()
            .add_systems(Update, ensure_inventory_on_tanks)
            .add_systems(Update, sanitize_inventories_when_added.after(ensure_inventory_on_tanks))
            .add_systems(
                Update,
                (
                    toggle_inventory_with_key,
                    close_inventory_when_owner_is_invalid,
                )
                    .chain()
                    .after(sanitize_inventories_when_added),
            );
    }
}

#[derive(Component, Debug, Clone)]
pub struct Inventory {
    pub meta: InventoryGridMeta,
    pub slots: InventorySlots,
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new(DEFAULT_INVENTORY_WIDTH, DEFAULT_INVENTORY_HEIGHT)
    }
}

impl Inventory {
    pub fn new(width: u16, height: u16) -> Self {
        let slot_count = usize::from(width).saturating_mul(usize::from(height));
        Self {
            meta: InventoryGridMeta { width, height },
            slots: InventorySlots(vec![None; slot_count]),
        }
    }

    pub fn slot_count(&self) -> usize {
        self.slots.0.len()
    }

    pub fn slot(&self, index: usize) -> Option<&Option<InventoryAvatarStack>> {
        self.slots.0.get(index)
    }

    pub fn slot_mut(&mut self, index: usize) -> Option<&mut Option<InventoryAvatarStack>> {
        self.slots.0.get_mut(index)
    }

    pub fn try_set_slot(
        &mut self,
        index: usize,
        stack: Option<InventoryAvatarStack>,
    ) -> Result<(), String> {
        let Some(slot_ref) = self.slot_mut(index) else {
            return Err(format!("inventory slot {index} out of range"));
        };

        if let Some(candidate) = &stack {
            if !candidate.is_allowed_in_inventory() {
                return Err(format!(
                    "item {} cannot be inserted into inventory",
                    candidate.item_archetype_id
                ));
            }
            if candidate.quantity == 0 {
                return Err(format!(
                    "item {} has zero quantity and cannot be inserted",
                    candidate.item_archetype_id
                ));
            }
        }

        *slot_ref = stack;
        Ok(())
    }

    pub fn sanitize_disallowed_stacks(&mut self) -> usize {
        let mut removed = 0usize;

        for slot in &mut self.slots.0 {
            let Some(stack) = slot else {
                continue;
            };
            if stack.quantity == 0 || !stack.is_allowed_in_inventory() {
                *slot = None;
                removed += 1;
            }
        }

        removed
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InventoryGridMeta {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Default)]
pub struct InventorySlots(pub Vec<Option<InventoryAvatarStack>>);

#[derive(Debug, Clone)]
pub struct InventoryAvatarStack {
    pub item_archetype_id: String,
    pub quantity: u32,
    pub color_rgba: [f32; 4],
}

impl InventoryAvatarStack {
    pub fn is_allowed_in_inventory(&self) -> bool {
        let base_kind = base_kind_from_archetype(&self.item_archetype_id);
        base_kind != BASE_KIND_NATURAL
    }
}

#[derive(Resource, Debug, Default)]
pub struct OpenInventoryState {
    pub owner_tank_entity: Option<Entity>,
    pub ui_root_entity: Option<Entity>,
}

impl OpenInventoryState {
    pub fn is_open(&self) -> bool {
        self.owner_tank_entity.is_some()
    }

    pub fn open_for(&mut self, owner_tank_entity: Entity) {
        self.owner_tank_entity = Some(owner_tank_entity);
    }

    pub fn close(&mut self) {
        self.owner_tank_entity = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementStage {
    PendingInventoryExit,
    ActivePreview,
}

#[derive(Debug, Clone)]
pub struct SelectedAvatarPlacement {
    pub owner_tank_entity: Entity,
    pub slot_index: usize,
    pub stack: InventoryAvatarStack,
    pub blob_instance_id: u32,
    pub preview_entity: Option<Entity>,
    pub stage: PlacementStage,
}

#[derive(Resource, Debug, Default)]
pub struct SelectedAvatarForPlacement(pub Option<SelectedAvatarPlacement>);

impl SelectedAvatarForPlacement {
    pub fn has_selection(&self) -> bool {
        self.0.is_some()
    }

    pub fn is_active_preview(&self) -> bool {
        self.0
            .as_ref()
            .map(|selection| selection.stage == PlacementStage::ActivePreview)
            .unwrap_or(false)
    }

    pub fn clear(&mut self, commands: &mut Commands) {
        if let Some(selection) = self.0.take() {
            if let Some(preview_entity) = selection.preview_entity {
                commands.entity(preview_entity).despawn_recursive();
            }
        }
    }
}

fn ensure_inventory_on_tanks(
    mut commands: Commands,
    tanks_without_inventory: Query<Entity, (With<Tank>, Without<Inventory>)>,
) {
    for tank_entity in &tanks_without_inventory {
        commands.entity(tank_entity).insert(Inventory::default());
    }
}

fn sanitize_inventories_when_added(
    mut inventories: Query<(Entity, &mut Inventory), Added<Inventory>>,
) {
    for (tank_entity, mut inventory) in &mut inventories {
        let removed = inventory.sanitize_disallowed_stacks();
        if removed > 0 {
            warn!(
                "Removed {} invalid inventory stacks from tank {:?}",
                removed, tank_entity
            );
        }
    }
}

fn toggle_inventory_with_key(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    focused_blob: Res<FocusedBlobInstance>,
    tanks: Query<(Entity, &BlobInstanceId, &FactionId), With<Tank>>,
    mut open_inventory_state: ResMut<OpenInventoryState>,
    mut selected_avatar: ResMut<SelectedAvatarForPlacement>,
) {
    if !keyboard_input.just_pressed(INVENTORY_TOGGLE_KEY) {
        return;
    }

    if open_inventory_state.is_open() {
        open_inventory_state.close();
        selected_avatar.clear(&mut commands);
        return;
    }

    let Some(focused_blob_id) = focused_blob.0 else {
        return;
    };

    let Some((player_tank_entity, _, _)) = tanks.iter().find(|(_, blob_instance, faction)| {
        blob_instance.0 == focused_blob_id && faction.0 == PLAYER_FACTION_ID
    }) else {
        return;
    };

    if selected_avatar.has_selection() {
        selected_avatar.clear(&mut commands);
    }

    open_inventory_state.open_for(player_tank_entity);
}

fn close_inventory_when_owner_is_invalid(
    mut commands: Commands,
    focused_blob: Res<FocusedBlobInstance>,
    tanks: Query<&BlobInstanceId, With<Tank>>,
    mut open_inventory_state: ResMut<OpenInventoryState>,
    mut selected_avatar: ResMut<SelectedAvatarForPlacement>,
) {
    let Some(owner_tank_entity) = open_inventory_state.owner_tank_entity else {
        return;
    };

    let Some(focused_blob_id) = focused_blob.0 else {
        open_inventory_state.close();
        selected_avatar.clear(&mut commands);
        return;
    };

    let Ok(owner_blob_instance) = tanks.get(owner_tank_entity) else {
        open_inventory_state.close();
        selected_avatar.clear(&mut commands);
        return;
    };

    if owner_blob_instance.0 != focused_blob_id {
        open_inventory_state.close();
        selected_avatar.clear(&mut commands);
    }
}

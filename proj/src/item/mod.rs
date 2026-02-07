use bevy::math::primitives::Circle;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::MaterialMesh2dBundle;

use crate::inventory::{Inventory, InventoryAvatarStack};
use crate::windowblob::{blob_render_layer, BlobInstanceId, BlobRenderLayer, MAIN_BLOB_INSTANCE_ID};

pub const BASE_KIND_NATURAL: &str = "natural";
pub const BASE_KIND_FACTORY: &str = "factory";
pub const BASE_KIND_MATERIAL: &str = "material";
pub const SUB_KIND_ORE: &str = "ore";
pub const SUB_KIND_DRILL: &str = "drill";
pub const DEFAULT_ITEM_RADIUS_BM: f32 = 0.22;
pub const DEFAULT_ORE_YIELD_PER_SECOND: f32 = 1.0;
pub const DEFAULT_DRILL_MINING_SPEED_PER_SECOND: f32 = 1.0;
pub const DEFAULT_DRILL_MINING_RADIUS_BM: f32 = 2.5;
pub const DEFAULT_DRILL_INVENTORY_WIDTH: u16 = 3;
pub const DEFAULT_DRILL_INVENTORY_HEIGHT: u16 = 3;
pub const DEFAULT_MAIN_ORE_KIND: &str = "iron";
pub const DEFAULT_MAIN_ORE_POSITION_BM: Vec2 = Vec2::new(-2.4, 1.2);
pub const DEFAULT_MAIN_ORE_COLOR_RGBA: [f32; 4] = [0.7, 0.7, 0.76, 1.0];

pub struct ItemPlugin;

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_default_main_blob_ore_if_missing)
            .add_systems(
                Update,
                (
                    ensure_selectable_base_items,
                    ensure_drill_runtime_components,
                    run_drill_auto_mining,
                    assemble_item_visuals,
                )
                    .chain(),
            );
    }
}

#[derive(Component, Debug, Default)]
pub struct Item;

#[derive(Component, Debug, Clone)]
pub struct BaseItem {
    pub radius_bm: f32,
    pub color_rgba: [f32; 4],
    pub base_kind: String,
    pub sub_kind: String,
}

impl BaseItem {
    pub fn color(&self) -> Color {
        Color::srgba(
            self.color_rgba[0].clamp(0.0, 1.0),
            self.color_rgba[1].clamp(0.0, 1.0),
            self.color_rgba[2].clamp(0.0, 1.0),
            self.color_rgba[3].clamp(0.0, 1.0),
        )
    }
}

#[derive(Component, Debug, Clone)]
pub struct OreData {
    pub ore_kind: String,
    pub yield_per_second: f32,
}

#[derive(Component, Debug, Clone)]
pub struct DrillData {
    pub mineable_ore_kinds: Vec<String>,
    pub mining_speed_per_second: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DrillMiningRadius {
    pub radius_bm: f32,
}

impl Default for DrillMiningRadius {
    fn default() -> Self {
        Self {
            radius_bm: DEFAULT_DRILL_MINING_RADIUS_BM,
        }
    }
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct DrillMiningProgress {
    pub produced_fractional_amount: f32,
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct SelectableBaseItem;

#[derive(Component, Debug)]
pub struct ItemVisualBuilt;

#[derive(Bundle)]
pub struct ItemBundle {
    pub item: Item,
    pub base_item: BaseItem,
    pub blob_instance: BlobInstanceId,
    pub blob_render_layer: BlobRenderLayer,
    pub spatial: SpatialBundle,
}

fn spawn_default_main_blob_ore_if_missing(
    mut commands: Commands,
    existing_items: Query<(&BlobInstanceId, &BaseItem), With<Item>>,
) {
    let main_has_ore = existing_items.iter().any(|(blob_instance, base_item)| {
        blob_instance.0 == MAIN_BLOB_INSTANCE_ID
            && base_item.base_kind == BASE_KIND_NATURAL
            && base_item.sub_kind == SUB_KIND_ORE
    });

    if main_has_ore {
        return;
    }

    commands.spawn((
        ItemBundle {
            item: Item,
            base_item: BaseItem {
                radius_bm: default_item_radius_for_archetype("natural/ore/iron"),
                color_rgba: DEFAULT_MAIN_ORE_COLOR_RGBA,
                base_kind: BASE_KIND_NATURAL.to_string(),
                sub_kind: SUB_KIND_ORE.to_string(),
            },
            blob_instance: BlobInstanceId(MAIN_BLOB_INSTANCE_ID),
            blob_render_layer: BlobRenderLayer(blob_render_layer(MAIN_BLOB_INSTANCE_ID)),
            spatial: SpatialBundle::from_transform(Transform::from_xyz(
                DEFAULT_MAIN_ORE_POSITION_BM.x,
                DEFAULT_MAIN_ORE_POSITION_BM.y,
                0.2,
            )),
        },
        OreData {
            ore_kind: DEFAULT_MAIN_ORE_KIND.to_string(),
            yield_per_second: DEFAULT_ORE_YIELD_PER_SECOND,
        },
    ));
}

fn ensure_selectable_base_items(world: &mut World) {
    let mut query =
        world.query_filtered::<(Entity, &BaseItem), (With<Item>, Without<SelectableBaseItem>)>();
    let to_mark: Vec<Entity> = query
        .iter(world)
        .filter_map(|(entity, base_item)| {
            if base_item.base_kind == BASE_KIND_NATURAL || base_item.base_kind == BASE_KIND_FACTORY {
                Some(entity)
            } else {
                None
            }
        })
        .collect();

    for entity in to_mark {
        if let Some(mut entity_ref) = world.get_entity_mut(entity) {
            entity_ref.insert(SelectableBaseItem);
        }
    }
}

fn ensure_drill_runtime_components(world: &mut World) {
    let mut query = world.query_filtered::<
        (
            Entity,
            &BaseItem,
            Option<&DrillData>,
            Option<&Inventory>,
            Option<&DrillMiningRadius>,
            Option<&DrillMiningProgress>,
        ),
        With<Item>,
    >();

    let to_patch: Vec<(Entity, bool, bool)> = query
        .iter(world)
        .filter_map(
            |(entity, base_item, drill_data, maybe_inventory, mining_radius, mining_progress)| {
                if base_item.base_kind != BASE_KIND_FACTORY || base_item.sub_kind != SUB_KIND_DRILL {
                    return None;
                }
                if drill_data.is_none() {
                    return None;
                }

                let needs_runtime = mining_radius.is_none() || mining_progress.is_none();
                let needs_inventory = maybe_inventory.is_none();
                if !needs_runtime && !needs_inventory {
                    return None;
                }

                Some((entity, needs_runtime, needs_inventory))
            },
        )
        .collect();

    for (entity, needs_runtime, needs_inventory) in to_patch {
        if let Some(mut entity_ref) = world.get_entity_mut(entity) {
            if needs_runtime {
                entity_ref.insert((DrillMiningRadius::default(), DrillMiningProgress::default()));
            }
            if needs_inventory {
                entity_ref.insert(Inventory::new(
                    DEFAULT_DRILL_INVENTORY_WIDTH,
                    DEFAULT_DRILL_INVENTORY_HEIGHT,
                ));
            }
        }
    }
}

fn run_drill_auto_mining(
    time: Res<Time>,
    ores: Query<(&Transform, &BlobInstanceId, &BaseItem, &OreData), With<Item>>,
    mut drills: Query<
        (
            &Transform,
            &BlobInstanceId,
            &BaseItem,
            &DrillData,
            &DrillMiningRadius,
            &mut DrillMiningProgress,
            &mut Inventory,
        ),
        With<Item>,
    >,
) {
    for (
        drill_transform,
        drill_blob,
        base_item,
        drill_data,
        drill_radius,
        mut drill_progress,
        mut drill_inventory,
    ) in &mut drills
    {
        if base_item.base_kind != BASE_KIND_FACTORY || base_item.sub_kind != SUB_KIND_DRILL {
            continue;
        }
        if drill_radius.radius_bm <= 0.0 || drill_data.mining_speed_per_second <= 0.0 {
            continue;
        }

        let drill_position = drill_transform.translation.truncate();

        let maybe_nearby_ore = ores.iter().find(|(ore_transform, ore_blob, ore_base_item, ore_data)| {
            if ore_blob.0 != drill_blob.0 {
                return false;
            }
            if ore_base_item.base_kind != BASE_KIND_NATURAL || ore_base_item.sub_kind != SUB_KIND_ORE {
                return false;
            }
            if !drill_data.mineable_ore_kinds.iter().any(|kind| kind == &ore_data.ore_kind) {
                return false;
            }
            drill_position.distance_squared(ore_transform.translation.truncate())
                <= drill_radius.radius_bm * drill_radius.radius_bm
        });

        let Some((_, _, _, ore_data)) = maybe_nearby_ore else {
            continue;
        };

        drill_progress.produced_fractional_amount +=
            drill_data.mining_speed_per_second * time.delta_seconds();

        while drill_progress.produced_fractional_amount >= 1.0 {
            drill_progress.produced_fractional_amount -= 1.0;
            let output_archetype = material_ore_archetype(&ore_data.ore_kind);
            let output_stack = InventoryAvatarStack {
                color_rgba: default_color_for_archetype(&output_archetype),
                item_archetype_id: output_archetype,
                quantity: 1,
            };
            let _ = drill_inventory.try_add_stack(output_stack);
        }
    }
}

pub fn base_kind_from_archetype(item_archetype_id: &str) -> &str {
    item_archetype_id.split('/').next().unwrap_or("unknown")
}

pub fn sub_kind_from_archetype(item_archetype_id: &str) -> &str {
    item_archetype_id.split('/').nth(1).unwrap_or("unknown")
}

pub fn default_item_radius_for_archetype(item_archetype_id: &str) -> f32 {
    let base_kind = base_kind_from_archetype(item_archetype_id);
    let sub_kind = sub_kind_from_archetype(item_archetype_id);

    if base_kind == BASE_KIND_NATURAL && sub_kind == SUB_KIND_ORE {
        0.26
    } else if base_kind == BASE_KIND_FACTORY && sub_kind == SUB_KIND_DRILL {
        0.3
    } else if base_kind == BASE_KIND_MATERIAL && sub_kind == SUB_KIND_ORE {
        0.2
    } else {
        DEFAULT_ITEM_RADIUS_BM
    }
}

pub fn default_color_for_archetype(item_archetype_id: &str) -> [f32; 4] {
    let base_kind = base_kind_from_archetype(item_archetype_id);
    let sub_kind = sub_kind_from_archetype(item_archetype_id);
    let ore_kind = item_archetype_id.split('/').nth(2).unwrap_or("generic");

    if base_kind == BASE_KIND_FACTORY && sub_kind == SUB_KIND_DRILL {
        [0.96, 0.68, 0.18, 1.0]
    } else if base_kind == BASE_KIND_NATURAL && sub_kind == SUB_KIND_ORE {
        [0.7, 0.7, 0.76, 1.0]
    } else if base_kind == BASE_KIND_MATERIAL && sub_kind == SUB_KIND_ORE {
        match ore_kind {
            "iron" => [0.82, 0.83, 0.87, 1.0],
            "copper" => [0.86, 0.54, 0.36, 1.0],
            _ => [0.78, 0.78, 0.78, 1.0],
        }
    } else {
        [0.8, 0.8, 0.8, 1.0]
    }
}

pub fn material_ore_archetype(ore_kind: &str) -> String {
    format!("{BASE_KIND_MATERIAL}/{SUB_KIND_ORE}/{ore_kind}")
}

pub fn archetype_from_world_item(
    base_item: &BaseItem,
    ore_data: Option<&OreData>,
    drill_data: Option<&DrillData>,
) -> String {
    if base_item.base_kind == BASE_KIND_FACTORY && base_item.sub_kind == SUB_KIND_DRILL {
        let preferred_ore_kind = drill_data
            .and_then(|drill| drill.mineable_ore_kinds.first())
            .cloned()
            .unwrap_or_else(|| "generic".to_string());
        format!("{BASE_KIND_FACTORY}/{SUB_KIND_DRILL}/{preferred_ore_kind}")
    } else if base_item.base_kind == BASE_KIND_NATURAL && base_item.sub_kind == SUB_KIND_ORE {
        let ore_kind = ore_data
            .map(|ore| ore.ore_kind.clone())
            .unwrap_or_else(|| "generic".to_string());
        format!("{BASE_KIND_NATURAL}/{SUB_KIND_ORE}/{ore_kind}")
    } else {
        format!("{}/{}", base_item.base_kind, base_item.sub_kind)
    }
}

pub fn station_kind_for_world_item(base_item: &BaseItem) -> Option<&'static str> {
    if base_item.base_kind == BASE_KIND_FACTORY && base_item.sub_kind == SUB_KIND_DRILL {
        Some("drill")
    } else {
        None
    }
}

pub fn metadata_from_archetype(
    item_archetype_id: &str,
) -> (String, String, Option<OreData>, Option<DrillData>) {
    let base_kind = base_kind_from_archetype(item_archetype_id).to_string();
    let sub_kind = sub_kind_from_archetype(item_archetype_id).to_string();

    let ore_data = if base_kind == BASE_KIND_NATURAL && sub_kind == SUB_KIND_ORE {
        let ore_kind = item_archetype_id
            .split('/')
            .nth(2)
            .unwrap_or("generic")
            .to_string();
        Some(OreData {
            ore_kind,
            yield_per_second: DEFAULT_ORE_YIELD_PER_SECOND,
        })
    } else {
        None
    };

    let drill_data = if base_kind == BASE_KIND_FACTORY && sub_kind == SUB_KIND_DRILL {
        let mineable_ore_kinds: Vec<String> = item_archetype_id
            .split('/')
            .skip(2)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .collect();

        Some(DrillData {
            mineable_ore_kinds: if mineable_ore_kinds.is_empty() {
                vec!["generic".to_string()]
            } else {
                mineable_ore_kinds
            },
            mining_speed_per_second: DEFAULT_DRILL_MINING_SPEED_PER_SECOND,
        })
    } else {
        None
    };

    (base_kind, sub_kind, ore_data, drill_data)
}

fn assemble_item_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    items: Query<
        (Entity, &BaseItem, &BlobRenderLayer),
        (With<Item>, Without<ItemVisualBuilt>),
    >,
) {
    for (item_entity, base_item, blob_layer) in &items {
        let item_visual = commands
            .spawn((
                *blob_layer,
                RenderLayers::layer(blob_layer.0),
                MaterialMesh2dBundle {
                    mesh: meshes.add(Mesh::from(Circle::new(base_item.radius_bm))).into(),
                    material: materials.add(ColorMaterial::from(base_item.color())),
                    transform: Transform::from_xyz(0.0, 0.0, 0.0),
                    ..default()
                },
            ))
            .id();

        commands
            .entity(item_entity)
            .add_child(item_visual)
            .insert(ItemVisualBuilt);
    }
}

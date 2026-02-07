use bevy::math::primitives::Circle;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::MaterialMesh2dBundle;

use crate::windowblob::{blob_render_layer, BlobInstanceId, BlobRenderLayer, MAIN_BLOB_INSTANCE_ID};

pub const BASE_KIND_NATURAL: &str = "natural";
pub const BASE_KIND_FACTORY: &str = "factory";
pub const SUB_KIND_ORE: &str = "ore";
pub const SUB_KIND_DRILL: &str = "drill";
pub const DEFAULT_ITEM_RADIUS_BM: f32 = 0.22;
pub const DEFAULT_ORE_YIELD_PER_SECOND: f32 = 1.0;
pub const DEFAULT_DRILL_MINING_SPEED_PER_SECOND: f32 = 1.0;
pub const DEFAULT_MAIN_ORE_KIND: &str = "iron";
pub const DEFAULT_MAIN_ORE_POSITION_BM: Vec2 = Vec2::new(-2.4, 1.2);
pub const DEFAULT_MAIN_ORE_COLOR_RGBA: [f32; 4] = [0.7, 0.7, 0.76, 1.0];

pub struct ItemPlugin;

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_default_main_blob_ore_if_missing)
            .add_systems(Update, assemble_item_visuals);
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
    } else {
        DEFAULT_ITEM_RADIUS_BM
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

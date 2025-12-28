use bevy::prelude::*;

/// ===== Tile 相关组件 =====

/// Tile 的网格坐标（第 r 行，第 c 列）
#[derive(Component, Copy, Clone, Eq, PartialEq, Hash)]
pub struct TilePos {
    pub r: i32,
    pub c: i32,
}

/// 地形类型（决定能不能种、颜色等）
#[derive(Component, Copy, Clone)]
pub enum Terrain {
    Grass,
    Water,
    Stone,
}

/// Tile 上面“种了什么”
///
/// - None  = 空气（没种）
/// - Some(entity) = 上面有某个实体（植物、墙等）
#[derive(Component)]
pub struct Occupant(pub Option<Entity>);

/// Tile 标记组件（tag）
#[derive(Component)]
pub struct Tile;

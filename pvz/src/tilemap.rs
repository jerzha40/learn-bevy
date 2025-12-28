use bevy::prelude::*;

/// ===== 网格参数 =====

/// 网格行数（从上到下 0..ROWS-1）
pub const ROWS: i32 = 5;

/// 网格列数（从左到右 0..COLS-1）
pub const COLS: i32 = 9;

/// 单个格子边长（世界坐标单位，像素）
pub const TILE: f32 = 80.0;

/// ===== 坐标换算函数 =====

/// 给定 (row, col)，返回该格子的世界坐标中心点（Vec3，z = 0）
pub fn cell_center_world(r: i32, c: i32) -> Vec3 {
    // 左边界：最左格子的左边缘
    let left = -(COLS as f32) * TILE / 2.0 + TILE / 2.0;
    // 顶边界：最上格子的上边缘
    let top = (ROWS as f32) * TILE / 2.0 - TILE / 2.0;

    let x = left + (c as f32) * TILE;
    let y = top - (r as f32) * TILE;

    Vec3::new(x, y, 0.0)
}

/// 给定世界坐标 (x, y)，返回它落在哪个格子 (row, col)
///
/// - 如果点在草坪外，返回 None
pub fn world_to_cell(p: Vec2) -> Option<(i32, i32)> {
    let left = -(COLS as f32) * TILE / 2.0;
    let top = (ROWS as f32) * TILE / 2.0;

    let c = ((p.x - left) / TILE).floor() as i32;
    let r = ((top - p.y) / TILE).floor() as i32;

    if r >= 0 && r < ROWS && c >= 0 && c < COLS {
        Some((r, c))
    } else {
        None
    }
}

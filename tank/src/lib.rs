pub mod bullet;
pub mod dummy;
pub mod tank;

pub use bullet::{bullet_visual_system, spawn_bullet, Bullet, BulletVisual};
pub use dummy::{dummy_target_visual_system, spawn_dummy_target, DummyTarget, DummyTargetVisual};
pub use tank::{
    spawn_tank, stats, tank_visual_system, Tank, TankBody, TankStats, TankTeam, TankVisual, Turret,
};

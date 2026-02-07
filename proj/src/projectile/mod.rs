use bevy::prelude::*;

pub mod baseprojectile;

#[derive(Component, Debug, Default)]
pub struct Projectile;

pub struct ProjectilePlugin;

impl Plugin for ProjectilePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(baseprojectile::BaseProjectilePlugin);
    }
}

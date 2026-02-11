use bevy::prelude::bevy_main;

pub mod app;

pub use app::run;

#[bevy_main]
fn main() {
    run();
}

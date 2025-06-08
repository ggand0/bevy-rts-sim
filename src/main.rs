mod constants;
mod types;
mod combat;
mod formation;
mod movement;
mod setup;
mod commander;

use bevy::prelude::*;
use types::*;
use combat::*;
use formation::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin)
        .insert_resource(SpatialGrid::new())
        .insert_resource(SquadManager::new())
        .add_systems(Startup, (setup::setup_scene, setup::spawn_army_with_squads))
        .add_systems(Update, (
            // Formation and squad management systems run first
            squad_formation_system,
            squad_casualty_management_system,
            squad_movement_system,
            commander::commander_promotion_system,
            commander::commander_visual_update_system,
            // Commander debug markers (glowing cubes above commanders)
            commander_visual_marker_system,
            update_commander_markers_system,
            // Animation and movement systems run after formation corrections
            movement::animate_march,
            movement::update_camera_info,
            movement::rts_camera_movement,
            target_acquisition_system,
            auto_fire_system,
            volley_fire_system,
            update_projectiles,
            collision_detection_system,
        ))
        .run();
}
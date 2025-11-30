mod constants;
mod types;
mod combat;
mod formation;
mod movement;
mod setup;
mod commander;
mod objective;
mod explosion_shader;
mod explosion_system;
mod particles;
mod wfx_materials;
mod wfx_spawn;
mod selection;
mod terrain;
use explosion_shader::ExplosionShaderPlugin;
use particles::ParticleEffectsPlugin;
use terrain::TerrainPlugin;
use wfx_materials::{SmokeScrollMaterial, AdditiveMaterial, SmokeOnlyMaterial};

use bevy::prelude::*;
use types::*;
use combat::*;
use formation::*;
use objective::*;


fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin)
        .add_plugins(ExplosionShaderPlugin)
        .add_plugins(ParticleEffectsPlugin)
        .add_plugins(TerrainPlugin)
        .add_plugins(MaterialPlugin::<SmokeScrollMaterial>::default())
        .add_plugins(MaterialPlugin::<AdditiveMaterial>::default())
        .add_plugins(MaterialPlugin::<SmokeOnlyMaterial>::default())
        .insert_resource(SpatialGrid::new())
        .insert_resource(SquadManager::new())
        .insert_resource(GameState::default())
        .insert_resource(ExplosionDebugMode::default())
        .insert_resource(selection::SelectionState::default())
        .add_systems(Startup, (setup::setup_scene, spawn_uplink_towers, spawn_objective_ui))
        // Army spawning runs after terrain is ready (terrain spawns in TerrainPlugin's Startup)
        .add_systems(Startup, setup::spawn_army_with_squads.after(terrain::spawn_initial_terrain))
        // Turret spawning runs after terrain is ready
        .add_systems(Startup, (
            spawn_functional_turret.after(terrain::spawn_initial_terrain),
            spawn_mg_turret.after(terrain::spawn_initial_terrain),
        ))
        .add_systems(Update, (
            // Formation and squad management systems run first
            squad_formation_system,
            squad_casualty_management_system,
            squad_movement_system,
            formation::squad_rotation_system,
            commander::commander_promotion_system,
            commander::commander_visual_update_system,
            // Commander debug markers (glowing cubes above commanders)
            commander_visual_marker_system,
            update_commander_markers_system,
        ))
        .add_systems(Update, (
            // Animation and movement systems run after formation corrections
            movement::animate_march,
            movement::update_camera_info,
            movement::rts_camera_movement,
        ))
        .add_systems(Update, (
            // Selection and command systems (Total War style controls)
            selection::selection_input_system,
            selection::box_selection_update_system,
            selection::move_command_system,
            selection::group_command_system,
        ).chain())
        .add_systems(Update, (
            // Selection visual feedback
            selection::selection_visual_system,
            selection::move_visual_cleanup_system,
            selection::orientation_arrow_system,
            selection::box_selection_visual_system,
            selection::update_group_orientation_markers,
            selection::update_group_bounding_box_debug,
            selection::update_squad_path_arrows,
        ))
        .add_systems(Update, (
            // Combat systems
            target_acquisition_system,
            auto_fire_system,
            volley_fire_system,
            update_projectiles,
            collision_detection_system,
            turret_rotation_system,
        ))
        .add_systems(Update, (
            // Objective system
            tower_targeting_system,
            tower_destruction_system,
            pending_explosion_system,
            explosion_effect_system,
            win_condition_system,
            update_objective_ui_system,
            update_debug_mode_ui,
            debug_explosion_hotkey_system,
            debug_warfx_test_system,
        ))
        .add_systems(Update, (
            // War FX explosion animations
            wfx_spawn::update_warfx_explosions,
            wfx_spawn::animate_explosion_flames,
            wfx_spawn::animate_warfx_billboards,
            wfx_spawn::animate_warfx_smoke_billboards,
            wfx_spawn::animate_explosion_billboards,
            wfx_spawn::animate_smoke_only_billboards,
            wfx_spawn::animate_glow_sparkles,
        ))
        .run();
}
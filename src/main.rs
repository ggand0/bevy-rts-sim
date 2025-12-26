mod constants;
mod types;
mod math_utils;
mod combat;
mod formation;
mod movement;
mod setup;
mod commander;
mod objective;
mod procedural_meshes;
mod turrets;
mod explosion_shader;
mod explosion_system;
mod particles;
mod wfx_materials;
mod wfx_spawn;
mod selection;
mod terrain;
mod terrain_decor;
mod shield;
mod decals;
mod scenario;
mod ground_explosion;
mod area_damage;
mod artillery;
mod collision;
use explosion_shader::ExplosionShaderPlugin;
use particles::ParticleEffectsPlugin;
use terrain::TerrainPlugin;
use terrain_decor::TerrainDecorPlugin;
use wfx_materials::{SmokeScrollMaterial, AdditiveMaterial, SmokeOnlyMaterial};
use shield::ShieldPlugin;
use decals::DecalPlugin;
use scenario::ScenarioPlugin;

use bevy::prelude::*;
use types::*;
use combat::*;
use formation::*;
use objective::*;
use turrets::*;


fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
        .add_plugins(ExplosionShaderPlugin)
        .add_plugins(ParticleEffectsPlugin)
        .add_plugins(TerrainPlugin)
        .add_plugins(TerrainDecorPlugin)
        .add_plugins(ShieldPlugin)
        .add_plugins(DecalPlugin)
        .add_plugins(ScenarioPlugin)
        .add_plugins(MaterialPlugin::<SmokeScrollMaterial>::default())
        .add_plugins(MaterialPlugin::<AdditiveMaterial>::default())
        .add_plugins(MaterialPlugin::<SmokeOnlyMaterial>::default())
        .add_plugins(MaterialPlugin::<ground_explosion::FlipbookMaterial>::default())
        .add_plugins(MaterialPlugin::<turrets::HealthBarMaterial>::default())
        .add_plugins(MaterialPlugin::<objective::ShieldBarMaterial>::default())
        .insert_resource(SpatialGrid::new())
        .insert_resource(SquadManager::new())
        .insert_resource(GameState::default())
        .insert_resource(ExplosionDebugMode::default())
        .insert_resource(selection::SelectionState::default())
        .insert_resource(ground_explosion::GroundExplosionDebugMenu::default())
        .insert_resource(artillery::ArtilleryState::default())
        .add_event::<AreaDamageEvent>()
        .add_systems(Startup, (setup::setup_scene, spawn_uplink_towers, spawn_debug_mode_ui, setup_laser_assets, ground_explosion::setup_ground_explosion_assets, ground_explosion::setup_ground_explosion_debug_ui))
        // Army spawning runs after terrain is ready (terrain spawns in TerrainPlugin's Startup)
        .add_systems(Startup, setup::spawn_army_with_squads.after(terrain::spawn_initial_terrain))
        // Turret spawning runs after terrain is ready
        .add_systems(Startup, (
            spawn_functional_turret.after(terrain::spawn_initial_terrain),
            spawn_mg_turret.after(terrain::spawn_initial_terrain),
        ))
        .add_systems(Update, (
            // Map switching - respawn turrets when terrain changes
            respawn_turrets_on_map_switch,
            // Debug turret toggle (M=MG, H=Heavy when debug mode active)
            debug_turret_toggle_system,
            // Turret health bars
            spawn_turret_health_bars,
            update_turret_health_bars,
            // Turret death with explosion
            turrets::turret_death_system,
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
            // Movement tracking for accuracy system (must run before animate_march)
            movement::update_movement_tracker,
            // Animation and movement systems run after formation corrections
            movement::animate_march.after(movement::update_movement_tracker),
            movement::update_fps_display,
            movement::rts_camera_movement,
        ))
        .add_systems(Update,
            // Unit-to-unit collision resolution (M2TW-style mass-based pushing)
            collision::unit_collision_system.after(movement::animate_march)
        )
        .add_systems(Update, (
            // Selection and command systems (Total War style controls)
            selection::selection_input_system,
            selection::box_selection_update_system,
            selection::move_command_system,
            selection::group_command_system,
            selection::hold_command_system,
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
            selection::update_squad_details_ui,
        ))
        .add_systems(Update, (
            // Combat systems
            target_acquisition_system,
            clear_blocked_targets_system, // Stuck prevention for AttackMove
            hitscan_fire_system,      // Infantry use hitscan (instant damage + visual tracer)
            auto_fire_system,         // Turrets still use projectiles
            volley_fire_system,
            update_projectiles,
            update_hitscan_tracers,   // Update visual tracers
        ))
        .add_systems(Update, (
            // Shield collision detection runs BEFORE unit collision
            shield::shield_collision_system,
            shield::shield_destruction_check_system, // Handles hitscan shield destruction
            shield::shield_regeneration_system,
            shield::shield_impact_flash_system,
            shield::shield_health_visual_system,
            shield::shield_tower_death_system,
            shield::shield_respawn_system,
            shield::animate_shields,
            shield::debug_destroy_enemy_shield, // Debug: Press '0' to destroy enemy shield
        ).before(collision_detection_system))
        .add_systems(Update, (
            // Unit collision and turret systems
            collision_detection_system,
            turret_rotation_system,
            visualize_collision_spheres_system, // Debug visualization
        ))
        .add_systems(Update, (
            // Objective system (tower targeting runs after shields)
            tower_targeting_system.after(shield::shield_collision_system),
            tower_destruction_system,
            pending_explosion_system,
            explosion_effect_system,
            win_condition_system,
            update_debug_mode_ui,
            debug_explosion_hotkey_system,
            debug_warfx_test_system,
            objective::debug_ground_explosion_system,
            objective::debug_spawn_shield_system,
            // Tower and shield health bars
            objective::spawn_tower_health_bars,
            objective::update_tower_health_bars,
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
        .add_systems(Update, (
            // Ground explosion animations (UE5 Niagara-style)
            ground_explosion::animate_flipbook_sprites,
            ground_explosion::update_velocity_aligned_billboards,
            ground_explosion::update_camera_facing_billboards,
            ground_explosion::update_smoke_physics,
            ground_explosion::update_dirt_physics,
            ground_explosion::update_smoke_scale,
            ground_explosion::update_fireball_scale,
            ground_explosion::update_fireball_uv_zoom,
            ground_explosion::update_dirt_scale,
            ground_explosion::update_dirt_alpha,
            ground_explosion::update_dirt001_scale,
            ground_explosion::update_dirt001_alpha,
        ))
        .add_systems(Update, (
            // Ground explosion animations continued
            ground_explosion::update_dust_scale,
            ground_explosion::update_dust_alpha,
            ground_explosion::update_wisp_physics,
            ground_explosion::update_wisp_scale,
            ground_explosion::update_wisp_alpha,
            ground_explosion::update_smoke_color,
            // Spark HDR color curves and physics
            ground_explosion::update_spark_color,
            ground_explosion::update_spark_l_color,
            ground_explosion::update_spark_l_physics,
            // Parts debris (3D mesh) physics and scale
            ground_explosion::update_parts_physics,
            ground_explosion::update_parts_scale,
            ground_explosion::animate_additive_sprites,
            ground_explosion::update_impact_lights,
            ground_explosion::cleanup_ground_explosions,
            ground_explosion::ground_explosion_debug_menu_system,
            ground_explosion::update_ground_explosion_debug_ui,
        ))
        .add_systems(Update, (
            // Artillery barrage system (V/B/N hotkeys)
            artillery::artillery_input_system,
            artillery::artillery_visual_system,
            artillery::artillery_spawn_system,
            artillery::artillery_cursor_system,
        ))
        .add_systems(Update, (
            // Area damage system (processes AreaDamageEvent from explosions)
            area_damage::area_damage_system,
            area_damage::knockback_physics_system,
            area_damage::ragdoll_death_system,
        ))
        .run();
}
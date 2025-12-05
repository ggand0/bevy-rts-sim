// Firebase Delta scenario implementation
// Wave-based defense scenario where players defend a hilltop command bunker

use bevy::prelude::*;
use crate::terrain::{MapPreset, MapSwitchEvent, TerrainHeightmap, handle_map_switch_units};
use crate::types::*;
use crate::setup::{spawn_single_squad, create_team_materials, create_droid_mesh};
use crate::turrets::{spawn_mg_turret_at, spawn_heavy_turret_at};
use crate::procedural_meshes::create_uplink_tower_mesh;

// ============================================================================
// SCENARIO CONSTANTS (easily tunable)
// ============================================================================

/// Total number of waves in the scenario
pub const TOTAL_WAVES: u32 = 10;

/// Delay between waves in seconds
pub const INTER_WAVE_DELAY: f32 = 30.0;

/// Units spawned per second during wave spawning
pub const SPAWN_RATE: f32 = 10.0;

/// Wave sizes (200 → 1000 enemies, 10 waves)
pub const WAVE_SIZES: [u32; 10] = [200, 200, 300, 300, 400, 400, 500, 600, 800, 1000];

/// Initial garrison size (number of friendly units)
pub const INITIAL_GARRISON_SIZE: u32 = 300; // 6 squads of 50

/// Number of turrets to spawn
pub const TURRET_COUNT: u32 = 5;

/// Radius around command bunker for breach detection
pub const BREACH_RADIUS: f32 = 50.0;

/// Number of enemies required to trigger breach defeat
pub const BREACH_THRESHOLD: u32 = 50;

/// North spawn point (enemies come from here in waves 1-10)
pub const NORTH_SPAWN: Vec3 = Vec3::new(0.0, 0.0, -200.0);

/// East spawn point (enemies come from here in waves 6-10)
pub const EAST_SPAWN: Vec3 = Vec3::new(200.0, 0.0, 0.0);

/// Turret positions around the hilltop (relative to map center)
pub const TURRET_POSITIONS: [Vec3; 5] = [
    Vec3::new(-50.0, 0.0, -50.0),  // Northwest
    Vec3::new(50.0, 0.0, -50.0),   // Northeast
    Vec3::new(-60.0, 0.0, 20.0),   // West
    Vec3::new(60.0, 0.0, 20.0),    // East
    Vec3::new(0.0, 0.0, 50.0),     // South
];

// ============================================================================
// SCENARIO TYPES
// ============================================================================

/// Type of scenario being played
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ScenarioType {
    #[default]
    None,
    FirebaseDelta,
}

/// Current state of wave progression
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum WaveState {
    #[default]
    Idle,
    /// Spawning enemies for current wave
    Spawning,
    /// All enemies spawned, combat ongoing
    Fighting,
    /// Wave cleared, waiting for next wave
    Cooldown,
    /// All waves completed (victory)
    Complete,
}

// ============================================================================
// SCENARIO RESOURCES
// ============================================================================

/// Global scenario state
#[derive(Resource, Default)]
pub struct ScenarioState {
    /// Whether a scenario is currently active
    pub active: bool,
    /// Type of active scenario
    pub scenario_type: ScenarioType,
}

/// Wave management resource
#[derive(Resource)]
pub struct WaveManager {
    /// Current wave number (1-indexed, 0 = not started)
    pub current_wave: u32,
    /// Total waves in scenario
    pub total_waves: u32,
    /// Current state of wave progression
    pub wave_state: WaveState,
    /// Timer for inter-wave cooldown
    pub inter_wave_timer: Timer,
    /// Number of enemies remaining in current wave
    pub enemies_remaining: u32,
    /// Number of enemies spawned so far in current wave
    pub enemies_spawned: u32,
    /// Target enemy count for current wave
    pub wave_target: u32,
    /// Timer for progressive spawning
    pub spawn_timer: Timer,
}

impl Default for WaveManager {
    fn default() -> Self {
        Self {
            current_wave: 0,
            total_waves: TOTAL_WAVES,
            wave_state: WaveState::Idle,
            inter_wave_timer: Timer::from_seconds(INTER_WAVE_DELAY, TimerMode::Once),
            enemies_remaining: 0,
            enemies_spawned: 0,
            wave_target: 0,
            spawn_timer: Timer::from_seconds(1.0 / SPAWN_RATE, TimerMode::Repeating),
        }
    }
}

// ============================================================================
// SCENARIO COMPONENTS
// ============================================================================

/// Marker component for enemies spawned by waves
#[derive(Component)]
pub struct WaveEnemy {
    /// Which wave this enemy belongs to
    pub wave_number: u32,
}

/// Marker for wave enemies that need their move orders set (deferred spawn workaround)
#[derive(Component)]
pub struct NeedsMoveOrder;

/// Marker component for all scenario-spawned entities (for cleanup)
#[derive(Component)]
pub struct ScenarioUnit;

/// Marker component for the command bunker
#[derive(Component)]
pub struct CommandBunker;

// ============================================================================
// UI COMPONENTS
// ============================================================================

/// Marker for wave counter UI text
#[derive(Component)]
pub struct WaveCounterUI;

/// Marker for enemy count UI text
#[derive(Component)]
pub struct EnemyCountUI;

/// Marker for all scenario UI (for cleanup)
#[derive(Component)]
pub struct ScenarioUI;

// ============================================================================
// PLUGIN
// ============================================================================

pub struct ScenarioPlugin;

impl Plugin for ScenarioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScenarioState>()
            .init_resource::<WaveManager>()
            .add_systems(Update, (
                // Must run after handle_map_switch_units clears default units/squads
                scenario_initialization_system.after(handle_map_switch_units),
                wave_state_machine_system,
                wave_spawner_system,
                wave_enemy_move_order_system, // Process move orders for newly spawned enemies
                enemy_death_tracking_system,
                update_wave_counter_ui,
                update_enemy_count_ui,
                victory_defeat_check_system,
            ).chain());
    }
}

// ============================================================================
// SYSTEMS
// ============================================================================

/// System that initializes the scenario when FirebaseDelta map is loaded
fn scenario_initialization_system(
    mut map_switch_events: EventReader<MapSwitchEvent>,
    mut scenario_state: ResMut<ScenarioState>,
    mut wave_manager: ResMut<WaveManager>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut squad_manager: ResMut<SquadManager>,
    heightmap: Res<TerrainHeightmap>,
    // Query to check if scenario is already initialized
    bunker_query: Query<Entity, With<CommandBunker>>,
    // Query for cleanup
    scenario_entities: Query<Entity, With<ScenarioUnit>>,
    scenario_ui: Query<Entity, With<ScenarioUI>>,
) {
    for event in map_switch_events.read() {
        if event.new_map == MapPreset::FirebaseDelta {
            // Check if already initialized
            if !bunker_query.is_empty() {
                info!("Firebase Delta scenario already initialized");
                continue;
            }

            info!("Initializing Firebase Delta scenario...");

            // Set scenario state
            scenario_state.active = true;
            scenario_state.scenario_type = ScenarioType::FirebaseDelta;

            // Reset wave manager
            *wave_manager = WaveManager::default();

            // Get terrain height at center for command bunker
            let bunker_y = heightmap.sample_height(0.0, 0.0);
            let bunker_pos = Vec3::new(0.0, bunker_y, 0.0);

            // Spawn command bunker (reusing UplinkTower mesh)
            let tower_mesh = create_uplink_tower_mesh(&mut meshes);
            let tower_material = materials.add(StandardMaterial {
                base_color: Color::srgb(0.3, 0.5, 0.8), // Blue for Team A
                metallic: 0.3,
                perceptual_roughness: 0.7,
                ..default()
            });

            commands.spawn((
                Mesh3d(tower_mesh),
                MeshMaterial3d(tower_material),
                Transform::from_translation(bunker_pos),
                UplinkTower {
                    team: Team::A,
                    destruction_radius: 80.0, // Same as normal tower
                },
                Health::new(2000.0), // More health than normal tower
                CommandBunker,
                ScenarioUnit,
                Name::new("CommandBunker"),
            ));
            info!("Spawned command bunker at {:?}", bunker_pos);

            // Spawn 5 turrets at TURRET_POSITIONS
            for (i, &offset) in TURRET_POSITIONS.iter().enumerate() {
                let turret_x = offset.x;
                let turret_z = offset.z;
                let turret_y = heightmap.sample_height(turret_x, turret_z);
                let turret_pos = Vec3::new(turret_x, turret_y, turret_z);

                // Alternate between MG and Heavy turrets
                if i % 2 == 0 {
                    spawn_mg_turret_at(&mut commands, &mut meshes, &mut materials, turret_pos);
                    info!("Spawned MG turret {} at {:?}", i, turret_pos);
                } else {
                    spawn_heavy_turret_at(&mut commands, &mut meshes, &mut materials, turret_pos);
                    info!("Spawned Heavy turret {} at {:?}", i, turret_pos);
                }
            }

            // Spawn initial garrison (6 squads around the bunker)
            let droid_mesh = create_droid_mesh(&mut meshes);
            let unit_materials = create_team_materials(&mut materials, Team::A);
            let num_garrison_squads = (INITIAL_GARRISON_SIZE / 50) as usize; // 6 squads

            for i in 0..num_garrison_squads {
                // Position squads in a circle around the bunker
                let angle = (i as f32 / num_garrison_squads as f32) * std::f32::consts::TAU;
                let radius = 40.0;
                let squad_x = radius * angle.cos();
                let squad_z = radius * angle.sin();
                let squad_y = heightmap.sample_height(squad_x, squad_z);
                let squad_pos = Vec3::new(squad_x, squad_y, squad_z);

                // Face outward from bunker
                let facing = Vec3::new(squad_x, 0.0, squad_z).normalize();

                spawn_single_squad(
                    &mut commands,
                    &mut squad_manager,
                    &droid_mesh,
                    &unit_materials,
                    &mut materials,
                    Team::A,
                    squad_pos,
                    facing,
                    &heightmap,
                );
            }
            info!("Spawned {} garrison squads", num_garrison_squads);

            // Spawn scenario UI
            spawn_scenario_ui(&mut commands);

            info!("Firebase Delta scenario initialized - press SPACE to start wave 1");

        } else if scenario_state.active {
            // Switching away from Firebase Delta - cleanup
            info!("Leaving Firebase Delta scenario, cleaning up...");
            scenario_state.active = false;
            scenario_state.scenario_type = ScenarioType::None;
            *wave_manager = WaveManager::default();

            // Cleanup scenario entities
            for entity in scenario_entities.iter() {
                commands.entity(entity).despawn();
            }
            for entity in scenario_ui.iter() {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Spawn the scenario UI elements
fn spawn_scenario_ui(commands: &mut Commands) {
    // Wave counter UI
    commands.spawn((
        Text::new("Wave: 0/10"),
        TextFont {
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.9, 0.3)), // Yellow
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(80.0),
            left: Val::Px(10.0),
            ..default()
        },
        WaveCounterUI,
        ScenarioUI,
    ));

    // Enemy count UI
    commands.spawn((
        Text::new("Enemies: 0"),
        TextFont {
            font_size: 22.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.3, 0.3)), // Red
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(105.0),
            left: Val::Px(10.0),
            ..default()
        },
        EnemyCountUI,
        ScenarioUI,
    ));

    // Instructions
    commands.spawn((
        Text::new("Press SPACE to start wave 1"),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.7, 0.7)), // Gray
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(130.0),
            left: Val::Px(10.0),
            ..default()
        },
        ScenarioUI,
    ));
}

/// Wave state machine - handles transitions between wave states
fn wave_state_machine_system(
    mut wave_manager: ResMut<WaveManager>,
    scenario_state: Res<ScenarioState>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    if !scenario_state.active {
        return;
    }

    match wave_manager.wave_state {
        WaveState::Idle => {
            // Wait for player to start (SPACE key)
            if keys.just_pressed(KeyCode::Space) && wave_manager.current_wave == 0 {
                wave_manager.current_wave = 1;
                wave_manager.wave_state = WaveState::Spawning;
                wave_manager.enemies_spawned = 0;
                wave_manager.wave_target = WAVE_SIZES[0];
                wave_manager.enemies_remaining = wave_manager.wave_target;
                info!("Wave 1 starting! Target: {} enemies", wave_manager.wave_target);
            }
        }
        WaveState::Spawning => {
            // Handled by wave_spawner_system
            // Transition to Fighting when all enemies spawned
            if wave_manager.enemies_spawned >= wave_manager.wave_target {
                wave_manager.wave_state = WaveState::Fighting;
                info!("Wave {} fully spawned, {} enemies in combat",
                    wave_manager.current_wave, wave_manager.enemies_remaining);
            }
        }
        WaveState::Fighting => {
            // Transition to Cooldown when all enemies dead
            if wave_manager.enemies_remaining == 0 {
                if wave_manager.current_wave >= wave_manager.total_waves {
                    wave_manager.wave_state = WaveState::Complete;
                    info!("All waves completed! Victory!");
                } else {
                    wave_manager.wave_state = WaveState::Cooldown;
                    wave_manager.inter_wave_timer.reset();
                    info!("Wave {} cleared! Next wave in {:.0}s",
                        wave_manager.current_wave, INTER_WAVE_DELAY);
                }
            }
        }
        WaveState::Cooldown => {
            // Wait for timer, then start next wave
            wave_manager.inter_wave_timer.tick(time.delta());
            if wave_manager.inter_wave_timer.finished() {
                wave_manager.current_wave += 1;
                wave_manager.wave_state = WaveState::Spawning;
                wave_manager.enemies_spawned = 0;
                let wave_idx = (wave_manager.current_wave - 1) as usize;
                wave_manager.wave_target = WAVE_SIZES.get(wave_idx).copied().unwrap_or(1000);
                wave_manager.enemies_remaining = wave_manager.wave_target;
                info!("Wave {} starting! Target: {} enemies",
                    wave_manager.current_wave, wave_manager.wave_target);
            }
        }
        WaveState::Complete => {
            // Victory state - handled by victory_defeat_check_system
        }
    }
}

/// Progressive spawning of enemies during wave
fn wave_spawner_system(
    mut wave_manager: ResMut<WaveManager>,
    scenario_state: Res<ScenarioState>,
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut squad_manager: ResMut<SquadManager>,
    heightmap: Res<TerrainHeightmap>,
    bunker_query: Query<&Transform, With<CommandBunker>>,
) {
    if !scenario_state.active || wave_manager.wave_state != WaveState::Spawning {
        return;
    }

    // Get bunker position for enemy targeting
    let bunker_pos = bunker_query.single().map(|t| t.translation).unwrap_or(Vec3::ZERO);

    // Tick spawn timer
    wave_manager.spawn_timer.tick(time.delta());

    // Spawn enemies when timer fires (spawn 1 squad = 50 units per tick)
    if wave_manager.spawn_timer.finished() && wave_manager.enemies_spawned < wave_manager.wave_target {
        wave_manager.spawn_timer.reset();

        // Spawn one squad at a time (50 units)
        let squad_size = 50u32;
        let remaining = wave_manager.wave_target - wave_manager.enemies_spawned;
        if remaining == 0 {
            return;
        }

        // Determine spawn location based on wave number and progress
        let base_spawn_pos = if wave_manager.current_wave <= 5 {
            // Waves 1-5: North spawn only
            NORTH_SPAWN
        } else {
            // Waves 6-10: alternate between north and east
            let squad_num = wave_manager.enemies_spawned / squad_size;
            if squad_num % 2 == 0 {
                NORTH_SPAWN
            } else {
                EAST_SPAWN
            }
        };

        // Add some spread to spawn position so squads don't stack
        let squad_num = wave_manager.enemies_spawned / squad_size;
        let spread_offset = Vec3::new(
            ((squad_num % 5) as f32 - 2.0) * 15.0, // -30 to +30 spread on X
            0.0,
            ((squad_num / 5 % 3) as f32 - 1.0) * 15.0, // -15 to +15 spread on Z
        );
        let spawn_x = base_spawn_pos.x + spread_offset.x;
        let spawn_z = base_spawn_pos.z + spread_offset.z;
        let spawn_y = heightmap.sample_height(spawn_x, spawn_z);
        let spawn_pos = Vec3::new(spawn_x, spawn_y, spawn_z);

        // Face toward bunker
        let facing = (bunker_pos - spawn_pos).normalize();

        // Create materials for enemy team (Team B)
        let unit_materials = create_team_materials(&mut materials, Team::B);
        let droid_mesh = create_droid_mesh(&mut meshes);

        // Spawn the squad
        let squad_id = spawn_single_squad(
            &mut commands,
            &mut squad_manager,
            &droid_mesh,
            &unit_materials,
            &mut materials,
            Team::B,
            spawn_pos,
            facing,
            &heightmap,
        );

        // Set squad target to bunker position (movement happens when target != center)
        if let Some(squad) = squad_manager.get_squad_mut(squad_id) {
            squad.target_position = bunker_pos;
            // Also set facing direction toward bunker
            squad.target_facing_direction = facing;
        }

        // Add WaveEnemy and NeedsMoveOrder markers to all units in this squad
        // NeedsMoveOrder will be processed next frame by wave_enemy_move_order_system
        if let Some(squad) = squad_manager.get_squad(squad_id) {
            for &unit_entity in &squad.members {
                commands.entity(unit_entity).insert((
                    WaveEnemy {
                        wave_number: wave_manager.current_wave,
                    },
                    NeedsMoveOrder,
                ));
            }
        }

        wave_manager.enemies_spawned += squad_size.min(remaining);

        // Log every 4th squad to reduce spam
        if (wave_manager.enemies_spawned / squad_size) % 4 == 0 {
            info!("Wave {}: Spawned squad ({}/{} enemies)",
                wave_manager.current_wave,
                wave_manager.enemies_spawned, wave_manager.wave_target);
        }
    }
}

/// Track enemy deaths and update wave manager
/// Counts remaining WaveEnemy entities to detect deaths
fn enemy_death_tracking_system(
    mut wave_manager: ResMut<WaveManager>,
    scenario_state: Res<ScenarioState>,
    wave_enemy_query: Query<Entity, With<WaveEnemy>>,
) {
    if !scenario_state.active {
        return;
    }

    // Only track during active combat (Spawning or Fighting states)
    if wave_manager.wave_state != WaveState::Spawning && wave_manager.wave_state != WaveState::Fighting {
        return;
    }

    // Count actual remaining enemies
    let actual_remaining = wave_enemy_query.iter().count() as u32;

    // Update enemies_remaining if it differs from actual count
    // This detects deaths caused by combat system
    if actual_remaining < wave_manager.enemies_remaining {
        let deaths = wave_manager.enemies_remaining - actual_remaining;
        if deaths > 0 {
            // Log significant death counts (not every single death to avoid spam)
            if deaths >= 5 || actual_remaining == 0 {
                info!("Wave {}: {} enemies killed, {} remaining",
                    wave_manager.current_wave, deaths, actual_remaining);
            }
        }
        wave_manager.enemies_remaining = actual_remaining;
    }
}

/// System to give move orders to newly spawned wave enemies
/// Runs after spawner to handle deferred entity spawning
fn wave_enemy_move_order_system(
    mut commands: Commands,
    scenario_state: Res<ScenarioState>,
    squad_manager: Res<SquadManager>,
    heightmap: Res<TerrainHeightmap>,
    bunker_query: Query<&Transform, With<CommandBunker>>,
    mut needs_order_query: Query<
        (Entity, &SquadMember, &mut BattleDroid, &mut FormationOffset),
        With<NeedsMoveOrder>,
    >,
) {
    if !scenario_state.active {
        return;
    }

    // Ensure bunker exists before processing
    let Ok(_bunker_transform) = bunker_query.single() else {
        return;
    };

    // Process all units that need move orders
    for (entity, squad_member, mut droid, mut formation_offset) in needs_order_query.iter_mut() {
        // Get the squad's target position (should be bunker)
        if let Some(squad) = squad_manager.get_squad(squad_member.squad_id) {
            // Calculate this unit's target position (squad target + formation offset)
            let target_xz = squad.target_position + formation_offset.local_offset;
            let target_y = heightmap.sample_height(target_xz.x, target_xz.z) + 1.28;
            let unit_target = Vec3::new(target_xz.x, target_y, target_xz.z);

            droid.target_position = unit_target;
            droid.returning_to_spawn = false;
            formation_offset.target_world_position = unit_target;
        }

        // Remove the marker - order has been given
        commands.entity(entity).remove::<NeedsMoveOrder>();
    }
}

/// Update wave counter UI
fn update_wave_counter_ui(
    wave_manager: Res<WaveManager>,
    scenario_state: Res<ScenarioState>,
    mut query: Query<&mut Text, With<WaveCounterUI>>,
) {
    if !scenario_state.active {
        return;
    }

    for mut text in query.iter_mut() {
        *text = Text::new(format!("Wave: {}/{}", wave_manager.current_wave, wave_manager.total_waves));
    }
}

/// Update enemy count UI
fn update_enemy_count_ui(
    wave_manager: Res<WaveManager>,
    scenario_state: Res<ScenarioState>,
    mut query: Query<&mut Text, With<EnemyCountUI>>,
) {
    if !scenario_state.active {
        return;
    }

    for mut text in query.iter_mut() {
        *text = Text::new(format!("Enemies: {}", wave_manager.enemies_remaining));
    }
}

/// Check victory and defeat conditions
fn victory_defeat_check_system(
    wave_manager: Res<WaveManager>,
    scenario_state: Res<ScenarioState>,
    mut game_state: ResMut<GameState>,
    bunker_query: Query<&Transform, With<CommandBunker>>,
    enemy_query: Query<&Transform, With<WaveEnemy>>,
) {
    if !scenario_state.active || game_state.game_ended {
        return;
    }

    // Victory: all waves complete
    if wave_manager.wave_state == WaveState::Complete {
        info!("VICTORY! All {} waves cleared!", wave_manager.total_waves);
        game_state.game_ended = true;
        game_state.winner = Some(Team::A);
        return;
    }

    // Defeat: bunker destroyed
    if bunker_query.is_empty() && scenario_state.scenario_type == ScenarioType::FirebaseDelta {
        // Only trigger if we've started (bunker should exist after initialization)
        if wave_manager.current_wave > 0 {
            info!("DEFEAT! Command bunker destroyed!");
            game_state.game_ended = true;
            game_state.winner = Some(Team::B);
            return;
        }
    }

    // Defeat: bunker breach (too many enemies near bunker)
    if let Ok(bunker_transform) = bunker_query.single() {
        let bunker_pos = bunker_transform.translation;
        let mut enemies_in_radius = 0;

        for enemy_transform in enemy_query.iter() {
            let dist = (enemy_transform.translation - bunker_pos).length();
            if dist <= BREACH_RADIUS {
                enemies_in_radius += 1;
            }
        }

        if enemies_in_radius >= BREACH_THRESHOLD {
            info!("DEFEAT! Command bunker breached! ({} enemies inside perimeter)", enemies_in_radius);
            game_state.game_ended = true;
            game_state.winner = Some(Team::B);
        }
    }
}

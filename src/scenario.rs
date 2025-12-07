// Firebase Delta scenario implementation
// Wave-based defense scenario where players defend a hilltop command bunker

use bevy::prelude::*;
use crate::terrain::{MapPreset, MapSwitchEvent, TerrainHeightmap, handle_map_switch_units};
use crate::types::*;
use crate::setup::{spawn_single_squad, create_team_materials, create_droid_mesh};
use crate::turrets::{spawn_mg_turret_at, spawn_heavy_turret_at};
use crate::procedural_meshes::create_uplink_tower_mesh;
use crate::selection::screen_to_ground_with_heightmap;

// ============================================================================
// SCENARIO CONSTANTS (easily tunable)
// ============================================================================

/// Total number of waves in the scenario
pub const TOTAL_WAVES: u32 = 3;

/// Delay between wave spawn starts (waves overlap, so this is shorter)
pub const INTER_WAVE_DELAY: f32 = 8.0;

/// Units spawned per second during wave spawning
pub const SPAWN_RATE: f32 = 10.0;

/// Wave sizes for 3 waves (largest first, then reinforcement waves)
pub const WAVE_SIZES: [u32; 3] = [200, 200, 300];

/// Initial garrison size (number of friendly units)
pub const INITIAL_GARRISON_SIZE: u32 = 300; // 6 squads of 50

/// Number of turrets player can place during preparation
pub const TURRET_BUDGET: u32 = 5;

/// Radius around command bunker for breach detection
pub const BREACH_RADIUS: f32 = 50.0;

/// Number of enemies required to trigger breach defeat
pub const BREACH_THRESHOLD: u32 = 50;

/// North spawn point (enemies come from here in waves 1-10)
pub const NORTH_SPAWN: Vec3 = Vec3::new(0.0, 0.0, -200.0);

/// East spawn point (enemies come from here in waves 6-10)
pub const EAST_SPAWN: Vec3 = Vec3::new(200.0, 0.0, 0.0);

/// Turret positions around the hilltop (relative to map center)
/// Note: These are no longer used since turrets are player-placed, kept for reference
#[allow(dead_code)]
const TURRET_POSITIONS: [Vec3; 5] = [
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
    /// Preparation phase - player places turrets before waves start
    Preparation,
    /// Active combat - waves spawn continuously, overlapping
    Combat,
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
    /// Timer until next wave starts spawning (runs during combat)
    pub next_wave_timer: Timer,
    /// Number of enemies remaining across all waves
    pub enemies_remaining: u32,
    /// Number of enemies spawned so far in current wave
    pub enemies_spawned: u32,
    /// Target enemy count for current wave
    pub wave_target: u32,
    /// Timer for progressive spawning
    pub spawn_timer: Timer,
    /// Turrets remaining to place during preparation
    pub turrets_remaining: u32,
    /// Currently selected turret type for placement (true = MG, false = Heavy)
    pub place_mg_turret: bool,
    /// Stack of placed turret entities for undo (most recent last)
    pub placed_turrets: Vec<Entity>,
    /// Whether we're actively spawning the current wave
    pub spawning_active: bool,
}

impl Default for WaveManager {
    fn default() -> Self {
        Self {
            current_wave: 0,
            total_waves: TOTAL_WAVES,
            wave_state: WaveState::Idle,
            next_wave_timer: Timer::from_seconds(INTER_WAVE_DELAY, TimerMode::Once),
            enemies_remaining: 0,
            enemies_spawned: 0,
            wave_target: 0,
            spawn_timer: Timer::from_seconds(1.0 / SPAWN_RATE, TimerMode::Repeating),
            turrets_remaining: TURRET_BUDGET,
            place_mg_turret: true, // Default to MG turret
            placed_turrets: Vec::new(),
            spawning_active: false,
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
    #[allow(dead_code)]
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

/// Marker for preparation phase instructions UI
#[derive(Component)]
pub struct PreparationInstructionsUI;

/// Marker for spawn point visual markers
#[derive(Component)]
pub struct SpawnPointMarker;

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
                turret_placement_system,
                wave_state_machine_system,
                wave_spawner_system,
                wave_enemy_move_order_system, // Process move orders for newly spawned enemies
                enemy_death_tracking_system,
                update_wave_counter_ui,
                update_enemy_count_ui,
                update_preparation_ui,
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
    // Query to hide/show default UI during scenario
    mut game_info_ui: Query<&mut Visibility, With<GameInfoUI>>,
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

            // Set wave state to Preparation so player can place turrets
            wave_manager.wave_state = WaveState::Preparation;
            wave_manager.turrets_remaining = TURRET_BUDGET;

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

            // Spawn spawn point markers
            spawn_spawn_point_markers(&mut commands, &mut meshes, &mut materials, &heightmap);

            // Spawn scenario UI
            spawn_scenario_ui(&mut commands);

            // Hide default UI during scenario
            for mut visibility in game_info_ui.iter_mut() {
                *visibility = Visibility::Hidden;
            }

            info!("Firebase Delta scenario initialized - place {} turrets (T to toggle type, click to place, SPACE to start)", TURRET_BUDGET);

        } else if scenario_state.active {
            // Switching away from Firebase Delta - cleanup
            info!("Leaving Firebase Delta scenario, cleaning up...");
            scenario_state.active = false;
            scenario_state.scenario_type = ScenarioType::None;
            *wave_manager = WaveManager::default();

            // Show default UI again
            for mut visibility in game_info_ui.iter_mut() {
                *visibility = Visibility::Visible;
            }

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
        Text::new("Wave: 0/3"),
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

    // Preparation phase instructions
    commands.spawn((
        Text::new(format!("PREPARATION - Turrets: {}/{} | T: toggle type | Click: place | SPACE: start",
            TURRET_BUDGET, TURRET_BUDGET)),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.3, 1.0, 0.3)), // Green
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(130.0),
            left: Val::Px(10.0),
            ..default()
        },
        PreparationInstructionsUI,
        ScenarioUI,
    ));
}

/// Spawn visual markers at enemy spawn points
fn spawn_spawn_point_markers(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    heightmap: &TerrainHeightmap,
) {
    // Create a simple pole/beacon mesh for spawn points
    let pole_mesh = meshes.add(Cylinder::new(0.5, 15.0));
    let beacon_mesh = meshes.add(Sphere::new(2.0));

    // Red material for enemy spawn points
    let pole_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.2, 0.2),
        emissive: bevy::color::LinearRgba::new(1.0, 0.3, 0.3, 1.0),
        ..default()
    });
    let beacon_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.3, 0.3),
        emissive: bevy::color::LinearRgba::new(2.0, 0.5, 0.5, 1.0),
        unlit: true,
        ..default()
    });

    let spawn_points = [
        (NORTH_SPAWN, "North Spawn"),
        (EAST_SPAWN, "East Spawn"),
    ];

    for (spawn_pos, name) in spawn_points {
        let y = heightmap.sample_height(spawn_pos.x, spawn_pos.z);
        let marker_pos = Vec3::new(spawn_pos.x, y + 7.5, spawn_pos.z); // Pole center
        let beacon_pos = Vec3::new(spawn_pos.x, y + 16.0, spawn_pos.z); // Beacon on top

        // Spawn pole
        commands.spawn((
            Mesh3d(pole_mesh.clone()),
            MeshMaterial3d(pole_material.clone()),
            Transform::from_translation(marker_pos),
            SpawnPointMarker,
            ScenarioUnit,
            Name::new(format!("{} Pole", name)),
        ));

        // Spawn glowing beacon on top
        commands.spawn((
            Mesh3d(beacon_mesh.clone()),
            MeshMaterial3d(beacon_material.clone()),
            Transform::from_translation(beacon_pos),
            SpawnPointMarker,
            ScenarioUnit,
            Name::new(format!("{} Beacon", name)),
        ));
    }

    info!("Spawned spawn point markers at North and East");
}

/// Wave state machine - handles transitions between wave states
/// Waves now overlap: next wave starts spawning while previous wave enemies are still fighting
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
            // Idle state - shouldn't happen in FirebaseDelta, goes straight to Preparation
        }
        WaveState::Preparation => {
            // Player is placing turrets. SPACE key starts wave 1.
            if keys.just_pressed(KeyCode::Space) {
                wave_manager.current_wave = 1;
                wave_manager.wave_state = WaveState::Combat;
                wave_manager.spawning_active = true;
                wave_manager.enemies_spawned = 0;
                wave_manager.wave_target = WAVE_SIZES[0];
                wave_manager.enemies_remaining = wave_manager.wave_target;
                info!("Preparation complete! Wave 1 starting! Target: {} enemies", wave_manager.wave_target);
            }
            // T key toggles turret type
            if keys.just_pressed(KeyCode::KeyT) {
                wave_manager.place_mg_turret = !wave_manager.place_mg_turret;
                let turret_type = if wave_manager.place_mg_turret { "MG" } else { "Heavy" };
                info!("Turret type: {}", turret_type);
            }
        }
        WaveState::Combat => {
            // Victory: all waves done spawning AND all enemies dead
            let all_waves_spawned = wave_manager.current_wave >= wave_manager.total_waves
                                    && !wave_manager.spawning_active;

            if all_waves_spawned && wave_manager.enemies_remaining == 0 {
                wave_manager.wave_state = WaveState::Complete;
                info!("All waves completed! Victory!");
                return;
            }

            // When current wave finishes spawning, start timer for next wave
            if !wave_manager.spawning_active
               && wave_manager.current_wave < wave_manager.total_waves
            {
                wave_manager.next_wave_timer.tick(time.delta());

                if wave_manager.next_wave_timer.just_finished() {
                    // Start next wave (enemies ADD to remaining, waves overlap)
                    wave_manager.current_wave += 1;
                    wave_manager.enemies_spawned = 0;
                    let wave_idx = (wave_manager.current_wave - 1) as usize;
                    wave_manager.wave_target = WAVE_SIZES.get(wave_idx).copied().unwrap_or(200);
                    wave_manager.enemies_remaining += wave_manager.wave_target;
                    wave_manager.spawning_active = true;
                    wave_manager.next_wave_timer.reset();
                    info!("Wave {} starting! Target: {} enemies, total remaining: {}",
                        wave_manager.current_wave, wave_manager.wave_target, wave_manager.enemies_remaining);
                }
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
    if !scenario_state.active || wave_manager.wave_state != WaveState::Combat {
        return;
    }

    // Only spawn if actively spawning this wave
    if !wave_manager.spawning_active {
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
        let squad_num = wave_manager.enemies_spawned / squad_size;
        let is_final_wave = wave_manager.current_wave == wave_manager.total_waves;

        let base_spawn_pos = if is_final_wave {
            // Final wave: first 2 squads from east (flanking), rest from north
            if squad_num < 2 {
                EAST_SPAWN
            } else {
                NORTH_SPAWN
            }
        } else {
            // Other waves: north spawn only
            NORTH_SPAWN
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

        // When done spawning this wave, deactivate spawning and start next wave timer
        if wave_manager.enemies_spawned >= wave_manager.wave_target {
            wave_manager.spawning_active = false;
            wave_manager.next_wave_timer.reset();
            if wave_manager.current_wave < wave_manager.total_waves {
                info!("Wave {} fully spawned, next wave in {:.0}s",
                    wave_manager.current_wave, INTER_WAVE_DELAY);
            } else {
                info!("Wave {} fully spawned (final wave!)",
                    wave_manager.current_wave);
            }
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

    // Only track during active combat
    if wave_manager.wave_state != WaveState::Combat {
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

/// Turret placement system - handles mouse clicks during Preparation phase
/// LMB: Place turret (only if not clicking on a unit), RMB: Undo last placement (only if no squads selected)
fn turret_placement_system(
    mut wave_manager: ResMut<WaveManager>,
    scenario_state: Res<ScenarioState>,
    selection_state: Res<crate::selection::SelectionState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    heightmap: Res<TerrainHeightmap>,
    unit_query: Query<(&Transform, &SquadMember), With<BattleDroid>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Only active during preparation phase
    if !scenario_state.active || wave_manager.wave_state != WaveState::Preparation {
        return;
    }

    // RMB: Undo last turret placement - but only if no squads are selected (let movement handle it)
    if mouse_button.just_pressed(MouseButton::Right) {
        // Only undo if no squads are selected - otherwise let movement system handle RMB
        if selection_state.selected_squads.is_empty() {
            if let Some(turret_entity) = wave_manager.placed_turrets.pop() {
                commands.entity(turret_entity).despawn();
                wave_manager.turrets_remaining += 1;
                info!("Undid turret placement ({} remaining)", wave_manager.turrets_remaining);
            }
        }
        return;
    }

    // No turrets left to place
    if wave_manager.turrets_remaining == 0 {
        return;
    }

    // Check for left mouse button click
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    // Get cursor position
    let Ok(window) = window_query.single() else { return };
    let Ok((camera, camera_transform)) = camera_query.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };

    // Convert to world position
    let Some(world_pos) = screen_to_ground_with_heightmap(cursor_pos, camera, camera_transform, Some(&heightmap)) else {
        return;
    };

    // Check if clicking on a friendly unit - if so, don't place turret (let selection handle it)
    let squad_centers = calculate_squad_centers_for_team(&unit_query, Team::A);
    if is_position_near_squad(world_pos, &squad_centers, crate::constants::SELECTION_CLICK_RADIUS) {
        // User clicked on a unit, let selection system handle it
        return;
    }

    // Spawn the selected turret type at click position and track it
    let turret_entity = if wave_manager.place_mg_turret {
        let entity = spawn_mg_turret_at(&mut commands, &mut meshes, &mut materials, world_pos);
        info!("Placed MG turret at {:?} ({} remaining)", world_pos, wave_manager.turrets_remaining - 1);
        entity
    } else {
        let entity = spawn_heavy_turret_at(&mut commands, &mut meshes, &mut materials, world_pos);
        info!("Placed Heavy turret at {:?} ({} remaining)", world_pos, wave_manager.turrets_remaining - 1);
        entity
    };

    wave_manager.placed_turrets.push(turret_entity);
    wave_manager.turrets_remaining -= 1;
}

/// Calculate squad centers for all friendly (Team::A) squads
/// Only calculates centers for squads with members present
fn calculate_squad_centers_for_team(
    unit_query: &Query<(&Transform, &SquadMember), With<BattleDroid>>,
    _team: Team,
) -> std::collections::HashMap<u32, Vec3> {
    use std::collections::HashMap;

    let mut squad_positions: HashMap<u32, Vec<Vec3>> = HashMap::new();

    // Collect all unit positions by squad
    // We'll filter by team A squads later (squads 0-5 are player garrison)
    for (transform, squad_member) in unit_query.iter() {
        // Player garrison squads have IDs 0-5 (6 squads of 50 = 300 units)
        // This is simpler than passing SquadManager through
        if squad_member.squad_id < 100 {
            squad_positions.entry(squad_member.squad_id)
                .or_insert_with(Vec::new)
                .push(transform.translation);
        }
    }

    let mut centers = HashMap::new();
    for (squad_id, positions) in squad_positions {
        if !positions.is_empty() {
            let sum: Vec3 = positions.iter().sum();
            centers.insert(squad_id, sum / positions.len() as f32);
        }
    }
    centers
}

/// Check if a world position is near any squad center
fn is_position_near_squad(
    world_pos: Vec3,
    squad_centers: &std::collections::HashMap<u32, Vec3>,
    max_distance: f32,
) -> bool {
    for center in squad_centers.values() {
        let dx = world_pos.x - center.x;
        let dz = world_pos.z - center.z;
        let distance = (dx * dx + dz * dz).sqrt();
        if distance < max_distance {
            return true;
        }
    }
    false
}

/// Update preparation phase instructions UI
fn update_preparation_ui(
    wave_manager: Res<WaveManager>,
    scenario_state: Res<ScenarioState>,
    mut query: Query<(&mut Text, &mut TextColor), With<PreparationInstructionsUI>>,
) {
    if !scenario_state.active {
        return;
    }

    for (mut text, mut color) in query.iter_mut() {
        match wave_manager.wave_state {
            WaveState::Preparation => {
                let turret_type = if wave_manager.place_mg_turret { "MG" } else { "Heavy" };
                *text = Text::new(format!(
                    "PREPARATION - Turrets: {}/{} | Type: {} | T: toggle | LMB: place | RMB: undo | SPACE: start",
                    wave_manager.turrets_remaining, TURRET_BUDGET, turret_type
                ));
                *color = TextColor(Color::srgb(0.3, 1.0, 0.3)); // Green
            }
            WaveState::Combat => {
                let status_text = if wave_manager.spawning_active {
                    format!("COMBAT - Wave {} spawning...", wave_manager.current_wave)
                } else if wave_manager.current_wave < wave_manager.total_waves {
                    let remaining = INTER_WAVE_DELAY - wave_manager.next_wave_timer.elapsed_secs();
                    format!("COMBAT - Next wave in {:.0}s", remaining.max(0.0))
                } else {
                    "COMBAT - Final wave!".to_string()
                };
                *text = Text::new(status_text);
                *color = TextColor(Color::srgb(1.0, 0.5, 0.3)); // Orange
            }
            WaveState::Complete => {
                *text = Text::new("VICTORY! All waves cleared!");
                *color = TextColor(Color::srgb(0.3, 1.0, 0.3)); // Green
            }
            WaveState::Idle => {
                *text = Text::new("");
            }
        }
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

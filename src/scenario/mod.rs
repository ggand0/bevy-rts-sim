// Firebase Delta scenario implementation
// Wave-based defense scenario where players defend a hilltop command bunker

mod wave;
mod ui;
mod placement;

use bevy::prelude::*;
use crate::terrain::{MapPreset, MapSwitchEvent, TerrainHeightmap, handle_map_switch_units};
use crate::types::*;
use crate::setup::{spawn_single_squad, create_team_materials, create_droid_mesh};
use crate::procedural_meshes::create_uplink_tower_mesh;

// Re-export submodule systems
use wave::{
    wave_state_machine_system, wave_spawner_system, reinforcement_spawner_system,
    enemy_death_tracking_system, wave_enemy_move_order_system, victory_defeat_check_system,
};
use ui::{spawn_scenario_ui, update_wave_counter_ui, update_enemy_count_ui, update_preparation_ui};
use placement::turret_placement_system;

// ============================================================================
// SCENARIO CONSTANTS (easily tunable)
// ============================================================================

/// Number of strategic waves (major assault phases)
/// Set to 1 to use only tactical waves (continuous spawning)
pub const STRATEGIC_WAVES: u32 = 2;

/// Delay between strategic waves (after all enemies cleared)
pub const STRATEGIC_WAVE_DELAY: f32 = 15.0;

/// Total number of tactical waves per strategic wave
pub const TACTICAL_WAVES: u32 = 3;

/// Delay between tactical wave spawn starts (waves overlap, so this is shorter)
pub const INTER_WAVE_DELAY: f32 = 8.0;

/// Units spawned per second during wave spawning
pub const SPAWN_RATE: f32 = 10.0;

/// Tactical wave sizes (within each strategic wave)
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

/// South spawn point (player reinforcements arrive here between assaults)
pub const SOUTH_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 200.0);

/// Number of reinforcement squads spawned after each strategic wave
pub const REINFORCEMENT_SQUADS: u32 = 2;

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
    /// Active combat - tactical waves spawn continuously, overlapping
    Combat,
    /// Cooldown between strategic waves (all tactical waves done, enemies cleared)
    StrategicCooldown,
    /// All strategic waves completed (victory)
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
    // === Strategic Wave Tracking ===
    /// Current strategic wave number (1-indexed, 0 = not started)
    pub strategic_wave: u32,
    /// Total strategic waves in scenario
    pub total_strategic_waves: u32,
    /// Timer for cooldown between strategic waves
    pub strategic_cooldown_timer: Timer,

    // === Tactical Wave Tracking (within current strategic wave) ===
    /// Current tactical wave number within strategic wave (1-indexed, 0 = not started)
    pub tactical_wave: u32,
    /// Total tactical waves per strategic wave
    pub total_tactical_waves: u32,
    /// Timer until next tactical wave starts spawning
    pub next_wave_timer: Timer,
    /// Whether we're actively spawning the current tactical wave
    pub spawning_active: bool,

    // === Enemy Tracking ===
    /// Number of enemies remaining across all tactical waves in current strategic wave
    pub enemies_remaining: u32,
    /// Number of enemies spawned so far in current tactical wave
    pub enemies_spawned: u32,
    /// Target enemy count for current tactical wave
    pub wave_target: u32,
    /// Timer for progressive spawning
    pub spawn_timer: Timer,

    // === Overall State ===
    /// Current state of wave progression
    pub wave_state: WaveState,

    // === Turret Placement ===
    /// Turrets remaining to place during preparation
    pub turrets_remaining: u32,
    /// Currently selected turret type for placement (true = MG, false = Heavy)
    pub place_mg_turret: bool,
    /// Stack of placed turret entities for undo (most recent last)
    pub placed_turrets: Vec<Entity>,

    // === Reinforcements ===
    /// Whether reinforcements have been spawned for the current strategic cooldown
    pub reinforcements_spawned: bool,
}

impl Default for WaveManager {
    fn default() -> Self {
        Self {
            // Strategic
            strategic_wave: 0,
            total_strategic_waves: STRATEGIC_WAVES,
            strategic_cooldown_timer: Timer::from_seconds(STRATEGIC_WAVE_DELAY, TimerMode::Once),

            // Tactical
            tactical_wave: 0,
            total_tactical_waves: TACTICAL_WAVES,
            next_wave_timer: Timer::from_seconds(INTER_WAVE_DELAY, TimerMode::Once),
            spawning_active: false,

            // Enemies
            enemies_remaining: 0,
            enemies_spawned: 0,
            wave_target: 0,
            spawn_timer: Timer::from_seconds(1.0 / SPAWN_RATE, TimerMode::Repeating),

            // State
            wave_state: WaveState::Idle,

            // Turrets
            turrets_remaining: TURRET_BUDGET,
            place_mg_turret: true,
            placed_turrets: Vec::new(),

            // Reinforcements
            reinforcements_spawned: false,
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
                reinforcement_spawner_system,
                wave_enemy_move_order_system,
                enemy_death_tracking_system,
                update_wave_counter_ui,
                update_enemy_count_ui,
                update_preparation_ui,
                victory_defeat_check_system,
            ).chain());
    }
}

// ============================================================================
// INITIALIZATION SYSTEM
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

            // Despawn all scenario entities
            for entity in scenario_entities.iter() {
                commands.entity(entity).despawn();
            }

            // Despawn scenario UI
            for entity in scenario_ui.iter() {
                commands.entity(entity).despawn();
            }

            // Show default UI again
            for mut visibility in game_info_ui.iter_mut() {
                *visibility = Visibility::Visible;
            }
        }
    }
}

// ============================================================================
// SPAWN POINT MARKERS
// ============================================================================

/// Spawn visual markers at spawn points
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
    let enemy_pole_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.2, 0.2),
        emissive: bevy::color::LinearRgba::new(1.0, 0.3, 0.3, 1.0),
        ..default()
    });
    let enemy_beacon_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.3, 0.3),
        emissive: bevy::color::LinearRgba::new(2.0, 0.5, 0.5, 1.0),
        unlit: true,
        ..default()
    });

    // Blue material for friendly reinforcement spawn point
    let friendly_pole_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.4, 0.8),
        emissive: bevy::color::LinearRgba::new(0.3, 0.5, 1.0, 1.0),
        ..default()
    });
    let friendly_beacon_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.5, 1.0),
        emissive: bevy::color::LinearRgba::new(0.5, 0.7, 2.0, 1.0),
        unlit: true,
        ..default()
    });

    // Enemy spawn points (red)
    let enemy_spawn_points = [
        (NORTH_SPAWN, "North Spawn"),
        (EAST_SPAWN, "East Spawn"),
    ];

    for (spawn_pos, name) in enemy_spawn_points {
        let y = heightmap.sample_height(spawn_pos.x, spawn_pos.z);
        let marker_pos = Vec3::new(spawn_pos.x, y + 7.5, spawn_pos.z); // Pole center
        let beacon_pos = Vec3::new(spawn_pos.x, y + 16.0, spawn_pos.z); // Beacon on top

        // Spawn pole
        commands.spawn((
            Mesh3d(pole_mesh.clone()),
            MeshMaterial3d(enemy_pole_material.clone()),
            Transform::from_translation(marker_pos),
            SpawnPointMarker,
            ScenarioUnit,
            Name::new(format!("{} Pole", name)),
        ));

        // Spawn glowing beacon on top
        commands.spawn((
            Mesh3d(beacon_mesh.clone()),
            MeshMaterial3d(enemy_beacon_material.clone()),
            Transform::from_translation(beacon_pos),
            SpawnPointMarker,
            ScenarioUnit,
            Name::new(format!("{} Beacon", name)),
        ));
    }

    // Friendly reinforcement spawn point (blue)
    {
        let y = heightmap.sample_height(SOUTH_SPAWN.x, SOUTH_SPAWN.z);
        let marker_pos = Vec3::new(SOUTH_SPAWN.x, y + 7.5, SOUTH_SPAWN.z);
        let beacon_pos = Vec3::new(SOUTH_SPAWN.x, y + 16.0, SOUTH_SPAWN.z);

        commands.spawn((
            Mesh3d(pole_mesh.clone()),
            MeshMaterial3d(friendly_pole_material),
            Transform::from_translation(marker_pos),
            SpawnPointMarker,
            ScenarioUnit,
            Name::new("South Spawn Pole"),
        ));

        commands.spawn((
            Mesh3d(beacon_mesh.clone()),
            MeshMaterial3d(friendly_beacon_material),
            Transform::from_translation(beacon_pos),
            SpawnPointMarker,
            ScenarioUnit,
            Name::new("South Spawn Beacon"),
        ));
    }

    info!("Spawned spawn point markers at North, East (enemy) and South (reinforcements)");
}

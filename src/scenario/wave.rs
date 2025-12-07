// Wave spawning and state machine systems

use bevy::prelude::*;
use crate::terrain::TerrainHeightmap;
use crate::types::*;
use crate::setup::{spawn_single_squad, create_team_materials, create_droid_mesh};

use super::{
    ScenarioState, WaveManager, WaveState, WaveEnemy, NeedsMoveOrder, CommandBunker,
    WAVE_SIZES, INTER_WAVE_DELAY, STRATEGIC_WAVE_DELAY, NORTH_SPAWN, EAST_SPAWN, SOUTH_SPAWN,
    REINFORCEMENT_SQUADS,
};

/// Wave state machine - handles transitions between wave states
/// Two-tier system: Strategic waves contain multiple tactical waves that spawn continuously
pub fn wave_state_machine_system(
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
            // Player is placing turrets. SPACE key starts strategic wave 1.
            if keys.just_pressed(KeyCode::Space) {
                start_strategic_wave(&mut wave_manager, 1);
                info!("Preparation complete! Strategic Wave 1 starting!");
            }
            // T key toggles turret type
            if keys.just_pressed(KeyCode::KeyT) {
                wave_manager.place_mg_turret = !wave_manager.place_mg_turret;
                let turret_type = if wave_manager.place_mg_turret { "MG" } else { "Heavy" };
                info!("Turret type: {}", turret_type);
            }
        }
        WaveState::Combat => {
            // Check if all tactical waves in this strategic wave are done
            let all_tactical_waves_spawned = wave_manager.tactical_wave >= wave_manager.total_tactical_waves
                                              && !wave_manager.spawning_active;

            if all_tactical_waves_spawned && wave_manager.enemies_remaining == 0 {
                // All tactical waves cleared - check if more strategic waves remain
                if wave_manager.strategic_wave >= wave_manager.total_strategic_waves {
                    // Victory!
                    wave_manager.wave_state = WaveState::Complete;
                    info!("All strategic waves completed! Victory!");
                } else {
                    // Start cooldown before next strategic wave
                    wave_manager.wave_state = WaveState::StrategicCooldown;
                    wave_manager.strategic_cooldown_timer.reset();
                    wave_manager.reinforcements_spawned = false; // Allow reinforcements to spawn
                    info!("Strategic Wave {} cleared! Next assault in {:.0}s - Reinforcements incoming!",
                        wave_manager.strategic_wave, STRATEGIC_WAVE_DELAY);
                }
                return;
            }

            // When current tactical wave finishes spawning, start timer for next tactical wave
            if !wave_manager.spawning_active
               && wave_manager.tactical_wave < wave_manager.total_tactical_waves
            {
                wave_manager.next_wave_timer.tick(time.delta());

                if wave_manager.next_wave_timer.just_finished() {
                    // Start next tactical wave (enemies ADD to remaining, waves overlap)
                    wave_manager.tactical_wave += 1;
                    wave_manager.enemies_spawned = 0;
                    let wave_idx = (wave_manager.tactical_wave - 1) as usize;
                    wave_manager.wave_target = WAVE_SIZES.get(wave_idx).copied().unwrap_or(200);
                    wave_manager.enemies_remaining += wave_manager.wave_target;
                    wave_manager.spawning_active = true;
                    wave_manager.next_wave_timer.reset();
                    info!("Strategic {}, Tactical {} starting! Target: {} enemies, total remaining: {}",
                        wave_manager.strategic_wave, wave_manager.tactical_wave,
                        wave_manager.wave_target, wave_manager.enemies_remaining);
                }
            }
        }
        WaveState::StrategicCooldown => {
            // Wait for timer, then start next strategic wave
            wave_manager.strategic_cooldown_timer.tick(time.delta());

            if wave_manager.strategic_cooldown_timer.just_finished() {
                let next_strategic = wave_manager.strategic_wave + 1;
                start_strategic_wave(&mut wave_manager, next_strategic);
                info!("Strategic Wave {} starting!", wave_manager.strategic_wave);
            }
        }
        WaveState::Complete => {
            // Victory state - handled by victory_defeat_check_system
        }
    }
}

/// Helper to start a new strategic wave
pub fn start_strategic_wave(wave_manager: &mut WaveManager, strategic_wave_num: u32) {
    wave_manager.strategic_wave = strategic_wave_num;
    wave_manager.tactical_wave = 1;
    wave_manager.wave_state = WaveState::Combat;
    wave_manager.spawning_active = true;
    wave_manager.enemies_spawned = 0;
    wave_manager.wave_target = WAVE_SIZES[0];
    wave_manager.enemies_remaining = wave_manager.wave_target;
}

/// Progressive spawning of enemies during wave
pub fn wave_spawner_system(
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
        let is_final_strategic_wave = wave_manager.strategic_wave == wave_manager.total_strategic_waves;
        let is_final_tactical_wave = wave_manager.tactical_wave == wave_manager.total_tactical_waves;

        let base_spawn_pos = if is_final_strategic_wave && is_final_tactical_wave {
            // Final tactical wave of final strategic wave: first 2 squads from east (flanking), rest from north
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
                        wave_number: wave_manager.tactical_wave,
                    },
                    NeedsMoveOrder,
                ));
            }
        }

        wave_manager.enemies_spawned += squad_size.min(remaining);

        // Log every 4th squad to reduce spam
        if (wave_manager.enemies_spawned / squad_size) % 4 == 0 {
            info!("S{}/T{}: Spawned squad ({}/{} enemies)",
                wave_manager.strategic_wave, wave_manager.tactical_wave,
                wave_manager.enemies_spawned, wave_manager.wave_target);
        }

        // When done spawning this tactical wave, deactivate spawning and start next wave timer
        if wave_manager.enemies_spawned >= wave_manager.wave_target {
            wave_manager.spawning_active = false;
            wave_manager.next_wave_timer.reset();
            if wave_manager.tactical_wave < wave_manager.total_tactical_waves {
                info!("S{}/T{} fully spawned, next tactical wave in {:.0}s",
                    wave_manager.strategic_wave, wave_manager.tactical_wave, INTER_WAVE_DELAY);
            } else {
                info!("S{}/T{} fully spawned (final tactical wave of this assault!)",
                    wave_manager.strategic_wave, wave_manager.tactical_wave);
            }
        }
    }
}

/// Spawn player reinforcements during strategic cooldown
pub fn reinforcement_spawner_system(
    mut wave_manager: ResMut<WaveManager>,
    scenario_state: Res<ScenarioState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut squad_manager: ResMut<SquadManager>,
    heightmap: Res<TerrainHeightmap>,
) {
    // Only spawn during strategic cooldown and if not already spawned
    if !scenario_state.active
        || wave_manager.wave_state != WaveState::StrategicCooldown
        || wave_manager.reinforcements_spawned
    {
        return;
    }

    // Mark as spawned so we don't spawn again this cooldown
    wave_manager.reinforcements_spawned = true;

    // Create materials and mesh for friendly units
    let droid_mesh = create_droid_mesh(&mut meshes);
    let unit_materials = create_team_materials(&mut materials, Team::A);

    // Get spawn position height
    let spawn_y = heightmap.sample_height(SOUTH_SPAWN.x, SOUTH_SPAWN.z);
    let base_spawn_pos = Vec3::new(SOUTH_SPAWN.x, spawn_y, SOUTH_SPAWN.z);

    // Facing direction (toward the bunker/north)
    let facing = Vec3::new(0.0, 0.0, -1.0);

    for i in 0..REINFORCEMENT_SQUADS {
        // Spread squads slightly apart
        let offset = Vec3::new((i as f32 - 0.5) * 20.0, 0.0, 0.0);
        let spawn_pos = base_spawn_pos + offset;

        spawn_single_squad(
            &mut commands,
            &mut squad_manager,
            &droid_mesh,
            &unit_materials,
            &mut materials,
            Team::A,
            spawn_pos,
            facing,
            &heightmap,
        );
    }

    info!("Reinforcements arrived! {} squads spawned at south spawn point",
        REINFORCEMENT_SQUADS);
}

/// Track enemy deaths and update wave manager
/// Counts remaining WaveEnemy entities to detect deaths
pub fn enemy_death_tracking_system(
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
                info!("Strategic {} / Tactical {}: {} enemies killed, {} remaining",
                    wave_manager.strategic_wave, wave_manager.tactical_wave, deaths, actual_remaining);
            }
        }
        wave_manager.enemies_remaining = actual_remaining;
    }
}

/// System to give move orders to newly spawned wave enemies
/// Runs after spawner to handle deferred entity spawning
pub fn wave_enemy_move_order_system(
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

/// Check victory and defeat conditions
pub fn victory_defeat_check_system(
    wave_manager: Res<WaveManager>,
    scenario_state: Res<ScenarioState>,
    mut game_state: ResMut<GameState>,
    bunker_query: Query<&Transform, With<CommandBunker>>,
    enemy_query: Query<&Transform, With<WaveEnemy>>,
) {
    use super::{ScenarioType, BREACH_RADIUS, BREACH_THRESHOLD};

    if !scenario_state.active || game_state.game_ended {
        return;
    }

    // Victory: all strategic waves complete
    if wave_manager.wave_state == WaveState::Complete {
        info!("VICTORY! All {} assaults repelled!", wave_manager.total_strategic_waves);
        game_state.game_ended = true;
        game_state.winner = Some(Team::A);
        return;
    }

    // Defeat: bunker destroyed
    if bunker_query.is_empty() && scenario_state.scenario_type == ScenarioType::FirebaseDelta {
        // Only trigger if we've started (bunker should exist after initialization)
        if wave_manager.strategic_wave > 0 {
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

// Objective system module - Uplink Tower mechanics
use bevy::prelude::*;
use crate::types::*;
use crate::constants::*;
use crate::procedural_meshes::*;
use crate::shield::{spawn_shield, ShieldMaterial, ShieldConfig};

// ===== TOWER CREATION =====

pub fn spawn_uplink_towers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shield_materials: ResMut<Assets<ShieldMaterial>>,
    shield_config: Res<ShieldConfig>,
) {
    let tower_mesh = create_uplink_tower_mesh(&mut meshes);
    
    // Team A tower material (blue/cyan sci-fi glow)
    let team_a_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.6, 0.9),
        emissive: Color::srgb(0.1, 0.3, 0.6).into(),
        metallic: 0.8,
        perceptual_roughness: 0.2,
        ..default()
    });
    
    // Team B tower material (red/orange sci-fi glow)
    let team_b_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.3, 0.2),
        emissive: Color::srgb(0.6, 0.2, 0.1).into(),
        metallic: 0.8,
        perceptual_roughness: 0.2,
        ..default()
    });
    
    // Spawn Team A tower (left side, behind army)
    let team_a_pos = Vec3::new(-BATTLEFIELD_SIZE / 2.0 - 30.0, 0.0, 0.0);
    commands.spawn((
        Mesh3d(tower_mesh.clone()),
        MeshMaterial3d(team_a_material),
        Transform::from_translation(team_a_pos)
            .with_scale(Vec3::splat(1.0)),
        UplinkTower {
            team: Team::A,
            destruction_radius: TOWER_DESTRUCTION_RADIUS,
        },
        ObjectiveTarget {
            team: Team::A,
            is_primary: true,
        },
        Health::new(TOWER_MAX_HEALTH),
        crate::types::BuildingCollider { radius: 5.0 }, // Collision radius for laser blocking
    ));

    // Spawn shield for Team A tower
    spawn_shield(
        &mut commands,
        &mut meshes,
        &mut shield_materials,
        team_a_pos,
        50.0, // Shield radius (covers tower and surrounding area)
        Team::A.shield_color(),
        Team::A,
        &shield_config,
    );

    // Spawn Team B tower (right side, behind army)
    let team_b_pos = Vec3::new(BATTLEFIELD_SIZE / 2.0 + 30.0, 0.0, 0.0);
    commands.spawn((
        Mesh3d(tower_mesh),
        MeshMaterial3d(team_b_material),
        Transform::from_translation(team_b_pos)
            .with_scale(Vec3::splat(1.0)),
        UplinkTower {
            team: Team::B,
            destruction_radius: TOWER_DESTRUCTION_RADIUS,
        },
        ObjectiveTarget {
            team: Team::B,
            is_primary: true,
        },
        Health::new(TOWER_MAX_HEALTH),
        crate::types::BuildingCollider { radius: 5.0 }, // Collision radius for laser blocking
    ));

    // Spawn shield for Team B tower
    spawn_shield(
        &mut commands,
        &mut meshes,
        &mut shield_materials,
        team_b_pos,
        50.0, // Shield radius (covers tower and surrounding area)
        Team::B.shield_color(),
        Team::B,
        &shield_config,
    );

    info!("Spawned Uplink Towers with shields for both teams");
}

// ===== TOWER TARGETING & DAMAGE =====

pub fn tower_targeting_system(
    mut tower_query: Query<(&Transform, &mut Health, &UplinkTower), With<UplinkTower>>,
    laser_query: Query<(&Transform, &LaserProjectile), With<LaserProjectile>>,
    _commands: Commands,
) {
    for (tower_transform, mut tower_health, tower) in tower_query.iter_mut() {
        for (laser_transform, laser_projectile) in laser_query.iter() {
            // Only enemy lasers can damage towers
            if laser_projectile.team == tower.team {
                continue;
            }
            
            let distance = tower_transform.translation.distance(laser_transform.translation);
            
            // Tower collision detection (larger collision radius due to size)
            if distance < TOWER_BASE_WIDTH {
                tower_health.damage(25.0); // Moderate damage per laser hit
                
                // TODO: Add hit effect/particle system here
                
                if tower_health.is_dead() {
                    info!("Tower {:?} destroyed! Health: {:.1}/{:.1}", 
                          tower.team, tower_health.current, tower_health.max);
                }
            }
        }
    }
}

// ===== TOWER DESTRUCTION CASCADE =====

pub fn tower_destruction_system(
    mut commands: Commands,
    tower_query: Query<(Entity, &Transform, &UplinkTower, &Health), (With<UplinkTower>, Without<PendingExplosion>)>,
    droid_query: Query<(Entity, &Transform, &BattleDroid), With<BattleDroid>>,
    particle_effects: Option<Res<crate::particles::ExplosionParticleEffects>>,
    time: Res<Time>,
    mut game_state: ResMut<GameState>,
) {
    let current_time = time.elapsed_secs_f64();

    for (tower_entity, tower_transform, tower, tower_health) in tower_query.iter() {
        if tower_health.is_dead() {
            info!("Processing tower destruction for team {:?}", tower.team);

            // Mark game as ended
            game_state.tower_destroyed(tower.team);

            // Find and despawn all friendly units within destruction radius
            // Spawn a death flash at each unit position
            let mut unit_count = 0;
            let mut flash_count = 0;
            // Collect unit positions FIRST
            let mut units_to_destroy: Vec<(Entity, Vec3)> = Vec::new();
            for (droid_entity, droid_transform, droid) in droid_query.iter() {
                if droid.team == tower.team {
                    let distance = tower_transform.translation.distance(droid_transform.translation);
                    if distance <= tower.destruction_radius {
                        units_to_destroy.push((droid_entity, droid_transform.translation));
                    }
                }
            }

            // Spawn mass explosion FIRST (at tower position - this works)
            if let Some(ref effects) = particle_effects {
                crate::particles::spawn_mass_explosion(
                    &mut commands,
                    effects,
                    tower_transform.translation,
                    current_time,
                );
            }

            // Add PendingExplosion to units with staggered delays
            // This uses the WORKING pending_explosion_system to spawn particles
            for (i, (droid_entity, _position)) in units_to_destroy.iter().enumerate() {
                // Stagger delays from 0.05s to 0.5s based on index
                let delay = 0.05 + (i as f32 * 0.0005).min(0.45);
                if let Ok(mut entity_commands) = commands.get_entity(*droid_entity) {
                    entity_commands.try_insert(PendingExplosion {
                        delay_timer: delay,
                        explosion_power: 1.0,
                    });
                }
                unit_count += 1;
                flash_count += 1;
            }

            // Add PendingExplosion to tower - the actual WFX explosion is spawned in pending_explosion_system
            if let Ok(mut entity_commands) = commands.get_entity(tower_entity) {
                entity_commands.try_insert(PendingExplosion {
                    delay_timer: 0.1, // Very short delay before removing tower
                    explosion_power: 3.0,
                });
            }

            info!("Tower {:?} destroyed! {} units despawned with death flashes, 1 mass explosion spawned",
                  tower.team, unit_count);
        }
    }
}

// Explosion systems moved to src/explosion_system.rs
// Re-export for backwards compatibility
pub use crate::explosion_system::{pending_explosion_system, explosion_effect_system, PendingExplosion};

// ===== WIN CONDITION SYSTEM =====

pub fn win_condition_system(
    game_state: Res<GameState>,
) {
    // Only log the victory message when the state first changes
    if game_state.game_ended && game_state.is_changed() {
        if let Some(winner) = game_state.winner {
            info!("🎉 VICTORY! Team {:?} wins the battle! 🎉", winner);
            // TODO: Display victory screen, stop unit AI, etc.
        }
    }
}

// ===== UI SYSTEM =====

pub fn update_objective_ui_system(
    mut ui_query: Query<&mut Text, With<ObjectiveUI>>,
    tower_query: Query<(&UplinkTower, &Health), With<UplinkTower>>,
    shield_query: Query<&crate::shield::Shield>,
    destroyed_shield_query: Query<&crate::shield::DestroyedShield>,
    game_state: Res<GameState>,
) {
    for mut text in ui_query.iter_mut() {
        let mut ui_text = String::new();

        // Tower and Shield health display
        ui_text.push_str("=== UPLINK TOWERS ===\n");
        for (tower, health) in tower_query.iter() {
            // Find shield for this team
            let shield_status = if let Some(shield) = shield_query.iter().find(|s| s.team == tower.team) {
                format!("Shield: {:.0}/{:.0} ({:.0}%)",
                    shield.current_hp,
                    shield.max_hp,
                    shield.health_percent() * 100.0)
            } else if let Some(destroyed) = destroyed_shield_query.iter().find(|d| d.team == tower.team) {
                format!("Shield: RESPAWN IN {:.1}s", destroyed.respawn_timer)
            } else {
                "Shield: OFFLINE".to_string()
            };

            ui_text.push_str(&format!(
                "Team {:?}:\n  Tower: {:.0}/{:.0} HP ({:.1}%)\n  {}\n",
                tower.team,
                health.current,
                health.max,
                health.health_percentage() * 100.0,
                shield_status
            ));
        }

        // Game status
        if game_state.game_ended {
            if let Some(winner) = game_state.winner {
                ui_text.push_str(&format!("\n🏆 VICTORY: Team {:?} Wins! 🏆", winner));
            }
        } else {
            ui_text.push_str("\n⚔️ Battle in Progress ⚔️");
        }

        **text = ui_text;
    }
}

#[derive(Component)]
pub struct ObjectiveUI;

#[derive(Component)]
pub struct DebugModeUI;

pub fn spawn_objective_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("Loading objective data..."),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(120.0),
            left: Val::Px(10.0),
            ..default()
        },
        ObjectiveUI,
    ));

    // Debug mode indicator (hidden by default)
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.8, 0.2)), // Yellow/gold color
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        DebugModeUI,
    ));
}

// ===== DEBUG SYSTEMS =====

pub fn debug_explosion_hotkey_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut tower_query: Query<(&UplinkTower, &mut Health), With<UplinkTower>>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyE) {
        info!("🔥 DEBUG: Explosion hotkey pressed! Setting Team B tower health to 0...");

        // Find Team B tower and set health to 0
        // tower_destruction_system will handle the rest
        for (tower, mut tower_health) in tower_query.iter_mut() {
            if tower.team == Team::B {
                tower_health.current = 0.0;
                info!("🔥 DEBUG: Team B tower health set to 0");
                break;
            }
        }
    }
}

/// Resource to track debug visualization modes (key 0 toggles debug menu)
#[derive(Resource)]
pub struct ExplosionDebugMode {
    pub explosion_mode: bool,
    pub show_collision_spheres: bool,
}

impl Default for ExplosionDebugMode {
    fn default() -> Self {
        Self {
            explosion_mode: false,
            show_collision_spheres: false,
        }
    }
}

/// System to update debug mode UI indicator
pub fn update_debug_mode_ui(
    debug_mode: Res<ExplosionDebugMode>,
    mut ui_query: Query<&mut Text, With<DebugModeUI>>,
) {
    if !debug_mode.is_changed() {
        return;
    }

    for mut text in ui_query.iter_mut() {
        if debug_mode.explosion_mode {
            **text = "[0] DEBUG: 1=glow 2=flames 3=smoke 4=sparkles 5=combined 6=dots | C=collision | S=destroy enemy shield".to_string();
        } else {
            **text = String::new();
        }
    }
}

// Debug system to test War FX explosion at map center
pub fn debug_warfx_test_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut additive_materials: ResMut<Assets<crate::wfx_materials::AdditiveMaterial>>,
    mut smoke_materials: ResMut<Assets<crate::wfx_materials::SmokeScrollMaterial>>,
    mut smoke_only_materials: ResMut<Assets<crate::wfx_materials::SmokeOnlyMaterial>>,
    asset_server: Res<AssetServer>,
    mut debug_mode: ResMut<ExplosionDebugMode>,
) {
    // 0 key: Toggle explosion debug mode
    if keyboard_input.just_pressed(KeyCode::Digit0) {
        debug_mode.explosion_mode = !debug_mode.explosion_mode;
        return;
    }

    // C key: Toggle collision sphere visualization
    if keyboard_input.just_pressed(KeyCode::KeyC) {
        debug_mode.show_collision_spheres = !debug_mode.show_collision_spheres;
        info!("Collision sphere visualization: {}", debug_mode.show_collision_spheres);
        return;
    }

    // Only process 1-6 keys when debug mode is active
    if !debug_mode.explosion_mode {
        return;
    }

    // 1 key: Spawn center glow billboards
    if keyboard_input.just_pressed(KeyCode::Digit1) {
        info!("🎆 DEBUG: War FX test hotkey (1) pressed! Spawning glow...");

        let position = Vec3::new(0.0, 10.0, 0.0);
        let scale = 2.0;

        // Spawn center glow billboards
        crate::wfx_spawn::spawn_warfx_center_glow(
            &mut commands,
            &mut meshes,
            &mut additive_materials,
            &asset_server,
            position,
            scale,
        );

        info!("💡 War FX glow spawned at center (0, 10, 0)");
    }

    // 2 key: Spawn COMPLETE explosion (center glow + smoke particles)
    // This matches Unity's WFX_ExplosiveSmoke_Big prefab which has multiple emitters
    if keyboard_input.just_pressed(KeyCode::Digit2) {
        info!("🔥 DEBUG: War FX explosion hotkey (2) pressed! Spawning complete explosion...");

        let position = Vec3::new(0.0, 10.0, 0.0);
        let scale = 2.0;

        // Spawn smoke/flame particles only (Explosion emitter)
        crate::wfx_spawn::spawn_explosion_flames(
            &mut commands,
            &mut meshes,
            &mut smoke_materials,
            &asset_server,
            position,
            scale,
        );

        info!("🔥 War FX complete explosion spawned at center (0, 10, 0)");
    }

    // 3 key: Spawn smoke emitter (lingering smoke trail)
    // This is the second phase of the Unity WFX_ExplosiveSmoke_Big effect
    if keyboard_input.just_pressed(KeyCode::Digit3) {
        info!("💨 DEBUG: War FX smoke hotkey (3) pressed! Spawning smoke emitter...");

        let position = Vec3::new(0.0, 10.0, 0.0);
        let scale = 2.0;

        // Spawn smoke emitter (delayed start, continuous emission)
        crate::wfx_spawn::spawn_smoke_emitter(
            &mut commands,
            &mut meshes,
            &mut smoke_only_materials,
            &asset_server,
            position,
            scale,
        );

        info!("💨 War FX smoke emitter spawned at center (0, 10, 0)");
    }

    // 4 key: Spawn glow sparkles (fast-moving embers with gravity)
    if keyboard_input.just_pressed(KeyCode::Digit4) {
        info!("✨ DEBUG: War FX sparkles hotkey (4) pressed! Spawning glow sparkles...");

        let position = Vec3::new(0.0, 10.0, 0.0);
        let scale = 2.0;

        crate::wfx_spawn::spawn_glow_sparkles(
            &mut commands,
            &mut meshes,
            &mut additive_materials,
            &asset_server,
            position,
            scale,
        );

        info!("✨ War FX glow sparkles spawned at center (0, 10, 0)");
    }

    // 5 key: Spawn combined explosion (all 4 emitters together)
    if keyboard_input.just_pressed(KeyCode::Digit5) {
        info!("💥 DEBUG: War FX COMBINED explosion hotkey (5) pressed!");

        let position = Vec3::new(0.0, 10.0, 0.0);
        let scale = 4.0; // Adjustable scale parameter

        crate::wfx_spawn::spawn_combined_explosion(
            &mut commands,
            &mut meshes,
            &mut additive_materials,
            &mut smoke_materials,
            &mut smoke_only_materials,
            &asset_server,
            position,
            scale,
        );

        info!("💥 War FX COMBINED explosion spawned at center (0, 10, 0) with scale {}", scale);
    }

    // 6 key: Spawn dot sparkles (both regular and vertical)
    if keyboard_input.just_pressed(KeyCode::Digit6) {
        info!("🔶 DEBUG: War FX dot sparkles hotkey (6) pressed!");

        let position = Vec3::new(0.0, 10.0, 0.0);
        let scale = 2.0;

        // Regular dot sparkles (75 particles, gravity-affected)
        crate::wfx_spawn::spawn_dot_sparkles(
            &mut commands,
            &mut meshes,
            &mut additive_materials,
            &asset_server,
            position,
            scale,
        );

        // Vertical dot sparkles (15 particles, float upward)
        crate::wfx_spawn::spawn_dot_sparkles_vertical(
            &mut commands,
            &mut meshes,
            &mut additive_materials,
            &asset_server,
            position,
            scale,
        );

        info!("🔶 War FX dot sparkles (75 + 15) spawned at center (0, 10, 0)");
    }
} 
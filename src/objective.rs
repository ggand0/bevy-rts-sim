// Objective system module - Uplink Tower mechanics
use bevy::prelude::*;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError, ShaderRef,
};
use bevy::render::alpha::AlphaMode;
use crate::types::*;
use crate::constants::*;
use crate::procedural_meshes::*;
use crate::shield::{spawn_shield, ShieldMaterial, ShieldConfig, Shield, DestroyedShield};

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
            let mut _flash_count = 0;
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
                _flash_count += 1;
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


#[derive(Component)]
pub struct DebugModeUI;

/// Spawn debug mode UI indicator (shown at bottom left when debug mode active)
pub fn spawn_debug_mode_ui(mut commands: Commands) {
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
    pub mg_turret_enabled: bool,
    pub heavy_turret_enabled: bool,
}

impl Default for ExplosionDebugMode {
    fn default() -> Self {
        Self {
            explosion_mode: false,
            show_collision_spheres: false,
            mg_turret_enabled: false,   // Turrets disabled by default for perf testing
            heavy_turret_enabled: false,
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
            let mg_status = if debug_mode.mg_turret_enabled { "ON" } else { "OFF" };
            let heavy_status = if debug_mode.heavy_turret_enabled { "ON" } else { "OFF" };
            **text = format!(
                "[0] DEBUG: 1-7=explosions 8=spawn shield | C=collision | S=destroy shield | M=MG turret ({}) | H=Heavy turret ({})",
                mg_status, heavy_status
            );
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

    // 7 key: Spawn turret WFX explosion (lighter version)
    if keyboard_input.just_pressed(KeyCode::Digit7) {
        info!("💥 DEBUG: War FX TURRET explosion hotkey (7) pressed!");

        let position = Vec3::new(0.0, 10.0, 0.0);
        let scale = 1.5; // Smaller scale for turret explosion

        crate::wfx_spawn::spawn_turret_wfx_explosion(
            &mut commands,
            &mut meshes,
            &mut additive_materials,
            &mut smoke_materials,
            &asset_server,
            position,
            scale,
        );

        info!("💥 War FX TURRET explosion spawned at center (0, 10, 0) with scale {}", scale);
    }
}

/// Debug system to spawn UE5-style ground explosion (9 key when debug mode active)
pub fn debug_ground_explosion_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    ground_assets: Option<Res<crate::ground_explosion::GroundExplosionAssets>>,
    mut flipbook_materials: ResMut<Assets<crate::ground_explosion::FlipbookMaterial>>,
    mut additive_materials: ResMut<Assets<crate::wfx_materials::AdditiveMaterial>>,
    debug_mode: Res<ExplosionDebugMode>,
    camera_query: Query<&GlobalTransform, With<Camera>>,
) {
    // Only work when debug mode is active
    if !debug_mode.explosion_mode {
        return;
    }

    // 9 key: Spawn UE5-style ground explosion
    if keyboard_input.just_pressed(KeyCode::Digit9) {
        info!("🌋 DEBUG: Ground explosion hotkey (9) pressed!");

        let Some(assets) = ground_assets else {
            warn!("Ground explosion assets not loaded yet!");
            return;
        };

        let position = Vec3::new(0.0, 0.0, 0.0); // Spawn at ground level
        let scale = 1.0;

        // Get camera transform for local-space velocity calculation
        let camera_transform = camera_query.iter().next();

        crate::ground_explosion::spawn_ground_explosion(
            &mut commands,
            &assets,
            &mut flipbook_materials,
            &mut additive_materials,
            position,
            scale,
            camera_transform,
        );

        info!("🌋 UE5 Ground explosion spawned at (0, 0, 0) with scale {}", scale);
    }
}

/// Debug system to spawn test tower + shield (8 key)
pub fn debug_spawn_shield_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut shield_materials: ResMut<Assets<ShieldMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    debug_mode: Res<ExplosionDebugMode>,
    shield_config: Res<ShieldConfig>,
) {
    // Only work when debug mode is active
    if !debug_mode.explosion_mode {
        return;
    }

    // 8 key: Spawn test tower + shield at center
    if keyboard_input.just_pressed(KeyCode::Digit8) {
        info!("🛡️ DEBUG: Spawning test tower + shield at center...");

        let position = Vec3::new(0.0, 0.0, 0.0);
        let shield_radius = 30.0;
        let team = Team::A; // Team A so Team B droids will shoot at it

        // Create tower mesh (same as spawn_uplink_towers)
        let tower_mesh = create_uplink_tower_mesh(&mut meshes);

        // Team A tower material (cyan/blue sci-fi glow)
        let tower_material = standard_materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.6, 0.9),
            emissive: Color::srgb(0.1, 0.3, 0.5).into(),
            metallic: 0.8,
            perceptual_roughness: 0.2,
            ..default()
        });

        // Spawn the tower
        commands.spawn((
            Mesh3d(tower_mesh),
            MeshMaterial3d(tower_material),
            Transform::from_translation(position),
            UplinkTower {
                team,
                destruction_radius: crate::constants::TOWER_DESTRUCTION_RADIUS,
            },
            ObjectiveTarget {
                team,
                is_primary: false, // Debug tower is not primary objective
            },
            Health::new(crate::constants::TOWER_MAX_HEALTH),
            crate::types::BuildingCollider { radius: 5.0 },
        ));

        // Spawn shield around the tower
        spawn_shield(
            &mut commands,
            &mut meshes,
            &mut shield_materials,
            position,
            shield_radius,
            team.shield_color(),
            team,
            &shield_config,
        );

        info!("🛡️ Test tower + shield spawned at center (0, 0, 0) with shield radius {}", shield_radius);
    }
}

// ============================================================================
// TOWER & SHIELD HEALTH BAR SYSTEM
// ============================================================================

/// Tower health bar width
const TOWER_HEALTH_BAR_WIDTH: f32 = 12.0;
/// Tower health bar height
const TOWER_HEALTH_BAR_HEIGHT: f32 = 0.8;
/// Height offset above tower for health bar
const TOWER_HEALTH_BAR_Y_OFFSET: f32 = 25.0;
/// Shield bar offset above tower health bar
const SHIELD_BAR_Y_OFFSET: f32 = 1.5;

/// Component linking tower health bar to its parent tower
#[derive(Component)]
pub struct TowerHealthBar {
    pub tower_entity: Entity,
}

/// Component linking shield health bar to its parent tower
#[derive(Component)]
pub struct ShieldHealthBar {
    pub tower_entity: Entity,
    pub team: Team,
}

/// Shader-based shield bar material - renders cyan/gray split based on health
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ShieldBarMaterial {
    /// x: health_fraction (0.0-1.0), yzw: unused
    #[uniform(0)]
    pub health_data: Vec4,
}

impl Material for ShieldBarMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/shield_bar.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// Spawn a health bar for a tower
fn spawn_tower_health_bar(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    health_bar_materials: &mut Assets<crate::turrets::HealthBarMaterial>,
    tower_entity: Entity,
    tower_pos: Vec3,
) {
    let bar_mesh = meshes.add(Mesh::from(Rectangle::new(TOWER_HEALTH_BAR_WIDTH, TOWER_HEALTH_BAR_HEIGHT)));

    let bar_material = health_bar_materials.add(crate::turrets::HealthBarMaterial {
        health_data: Vec4::new(1.0, 0.0, 0.0, 0.0),
    });

    let bar_y = tower_pos.y + TOWER_HEALTH_BAR_Y_OFFSET;

    commands.spawn((
        Mesh3d(bar_mesh),
        MeshMaterial3d(bar_material),
        Transform::from_translation(Vec3::new(tower_pos.x, bar_y, tower_pos.z)),
        TowerHealthBar { tower_entity },
        bevy::pbr::NotShadowCaster,
    ));
}

/// Spawn a shield bar for a tower
fn spawn_shield_health_bar(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    shield_bar_materials: &mut Assets<ShieldBarMaterial>,
    tower_entity: Entity,
    tower_pos: Vec3,
    team: Team,
) {
    let bar_mesh = meshes.add(Mesh::from(Rectangle::new(TOWER_HEALTH_BAR_WIDTH, TOWER_HEALTH_BAR_HEIGHT)));

    let bar_material = shield_bar_materials.add(ShieldBarMaterial {
        health_data: Vec4::new(1.0, 0.0, 0.0, 0.0),
    });

    let bar_y = tower_pos.y + TOWER_HEALTH_BAR_Y_OFFSET + SHIELD_BAR_Y_OFFSET;

    commands.spawn((
        Mesh3d(bar_mesh),
        MeshMaterial3d(bar_material),
        Transform::from_translation(Vec3::new(tower_pos.x, bar_y, tower_pos.z)),
        ShieldHealthBar { tower_entity, team },
        bevy::pbr::NotShadowCaster,
    ));
}

/// System to spawn health bars for towers that don't have them yet
pub fn spawn_tower_health_bars(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut health_bar_materials: ResMut<Assets<crate::turrets::HealthBarMaterial>>,
    mut shield_bar_materials: ResMut<Assets<ShieldBarMaterial>>,
    tower_query: Query<(Entity, &Transform, &UplinkTower), With<Health>>,
    tower_health_bar_query: Query<&TowerHealthBar>,
    shield_health_bar_query: Query<&ShieldHealthBar>,
) {
    for (tower_entity, transform, uplink_tower) in tower_query.iter() {
        // Spawn tower health bar if missing
        let has_tower_bar = tower_health_bar_query.iter().any(|bar| bar.tower_entity == tower_entity);
        if !has_tower_bar {
            spawn_tower_health_bar(
                &mut commands,
                &mut meshes,
                &mut health_bar_materials,
                tower_entity,
                transform.translation,
            );
        }

        // Spawn shield health bar if missing
        let has_shield_bar = shield_health_bar_query.iter().any(|bar| bar.tower_entity == tower_entity);
        if !has_shield_bar {
            spawn_shield_health_bar(
                &mut commands,
                &mut meshes,
                &mut shield_bar_materials,
                tower_entity,
                transform.translation,
                uplink_tower.team,
            );
        }
    }
}

/// Distance to offset health bars towards camera (to prevent occlusion by tower)
const HEALTH_BAR_CAMERA_OFFSET: f32 = 5.0;

/// System to update tower and shield health bars
pub fn update_tower_health_bars(
    mut commands: Commands,
    tower_query: Query<(Entity, &Transform, &Health, &UplinkTower), With<UplinkTower>>,
    mut tower_bar_query: Query<(Entity, &TowerHealthBar, &mut Transform, &MeshMaterial3d<crate::turrets::HealthBarMaterial>), Without<UplinkTower>>,
    mut shield_bar_query: Query<(Entity, &ShieldHealthBar, &mut Transform, &MeshMaterial3d<ShieldBarMaterial>), (Without<UplinkTower>, Without<TowerHealthBar>)>,
    mut health_bar_materials: ResMut<Assets<crate::turrets::HealthBarMaterial>>,
    mut shield_bar_materials: ResMut<Assets<ShieldBarMaterial>>,
    shield_query: Query<&Shield>,
    destroyed_shield_query: Query<&DestroyedShield>,
    camera_query: Query<&Transform, (With<RtsCamera>, Without<UplinkTower>, Without<TowerHealthBar>, Without<ShieldHealthBar>)>,
) {
    let Ok(camera_transform) = camera_query.single() else { return };
    let camera_pos = camera_transform.translation;

    // Create lookup for tower data
    let towers: std::collections::HashMap<Entity, (&Transform, &Health, &UplinkTower)> = tower_query
        .iter()
        .map(|(e, t, h, u)| (e, (t, h, u)))
        .collect();

    // Billboard rotation: use camera's rotation so bars are always parallel to camera view plane
    // This works for any camera angle including top-down
    let billboard_rotation = camera_transform.rotation;

    // Camera's up direction in world space - used to offset shield bar above tower bar
    let camera_up = camera_transform.up();

    // Update tower health bars
    for (bar_entity, tower_bar, mut bar_transform, material_handle) in tower_bar_query.iter_mut() {
        if let Some((tower_transform, health, _)) = towers.get(&tower_bar.tower_entity) {
            let health_fraction = health.current / health.max;

            // Base position above tower
            let tower_pos = tower_transform.translation;
            let bar_base_pos = Vec3::new(
                tower_pos.x,
                tower_pos.y + TOWER_HEALTH_BAR_Y_OFFSET,
                tower_pos.z,
            );

            // Calculate direction from bar to camera (horizontal only for offset)
            let horizontal_to_camera = Vec3::new(
                camera_pos.x - bar_base_pos.x,
                0.0,
                camera_pos.z - bar_base_pos.z,
            ).normalize_or_zero();

            // Offset bar position towards camera to prevent occlusion
            let bar_pos = bar_base_pos + horizontal_to_camera * HEALTH_BAR_CAMERA_OFFSET;
            bar_transform.translation = bar_pos;
            bar_transform.rotation = billboard_rotation;

            if let Some(material) = health_bar_materials.get_mut(&material_handle.0) {
                material.health_data.x = health_fraction;
            }
        } else {
            commands.entity(bar_entity).despawn();
        }
    }

    // Update shield health bars
    for (bar_entity, shield_bar, mut bar_transform, material_handle) in shield_bar_query.iter_mut() {
        if let Some((tower_transform, _, _)) = towers.get(&shield_bar.tower_entity) {
            // Find shield for this team - use the shield closest to this tower
            // (handles multiple shields of same team)
            let tower_pos = tower_transform.translation;
            let shield_fraction = if let Some(shield) = shield_query.iter()
                .filter(|s| s.team == shield_bar.team)
                .min_by(|a, b| {
                    let dist_a = a.center.distance_squared(tower_pos);
                    let dist_b = b.center.distance_squared(tower_pos);
                    dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
                })
            {
                // Debug: log shield HP updates occasionally
                if shield.current_hp < shield.max_hp && (shield.current_hp as i32) % 500 == 0 {
                    info!("SHIELD BAR UPDATE: team {:?}, hp {}/{}, fraction {}, tower {:?}, shield {:?}",
                        shield_bar.team, shield.current_hp, shield.max_hp, shield.current_hp / shield.max_hp,
                        tower_pos, shield.center);
                }
                shield.current_hp / shield.max_hp
            } else if destroyed_shield_query.iter().any(|d| d.team == shield_bar.team) {
                // Shield destroyed - show empty bar
                0.0
            } else {
                0.0
            };

            // Base position above tower (same as tower bar base)
            let tower_pos = tower_transform.translation;
            let bar_base_pos = Vec3::new(
                tower_pos.x,
                tower_pos.y + TOWER_HEALTH_BAR_Y_OFFSET,
                tower_pos.z,
            );

            // Calculate direction from bar to camera (horizontal only for offset)
            let horizontal_to_camera = Vec3::new(
                camera_pos.x - bar_base_pos.x,
                0.0,
                camera_pos.z - bar_base_pos.z,
            ).normalize_or_zero();

            // Offset bar position towards camera to prevent occlusion
            // Then offset in camera's up direction so shield bar is always above tower bar on screen
            let bar_pos = bar_base_pos + horizontal_to_camera * HEALTH_BAR_CAMERA_OFFSET + camera_up * SHIELD_BAR_Y_OFFSET;
            bar_transform.translation = bar_pos;
            bar_transform.rotation = billboard_rotation;

            if let Some(material) = shield_bar_materials.get_mut(&material_handle.0) {
                material.health_data.x = shield_fraction;
            }
        } else {
            commands.entity(bar_entity).despawn();
        }
    }
} 
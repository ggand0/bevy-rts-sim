// Turret spawn systems module
use bevy::prelude::*;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError, ShaderRef,
};
use bevy::render::alpha::AlphaMode;
use crate::types::*;
use crate::terrain::{TerrainHeightmap, MapSwitchEvent};
use crate::procedural_meshes::*;

/// MG turret health points
pub const MG_TURRET_HEALTH: f32 = 10_000.0;
/// Heavy turret health points
pub const HEAVY_TURRET_HEALTH: f32 = 20_000.0;
/// Health bar width in world units
const HEALTH_BAR_WIDTH: f32 = 6.0;
/// Health bar height in world units
const HEALTH_BAR_HEIGHT: f32 = 0.5;
/// Health bar offset above turret
const HEALTH_BAR_Y_OFFSET: f32 = 8.0;

/// Component linking health bar to its parent turret
#[derive(Component)]
pub struct TurretHealthBar {
    /// The turret base entity this bar belongs to
    pub turret_entity: Entity,
}

// ============================================================================
// HEALTH BAR SHADER MATERIAL
// ============================================================================

/// Shader-based health bar material - renders green/gray split based on health
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct HealthBarMaterial {
    /// x: health_fraction (0.0-1.0), yzw: unused
    #[uniform(0)]
    pub health_data: Vec4,
}

impl Material for HealthBarMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/health_bar.wgsl".into()
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
        // Disable backface culling so the bar is visible from both sides
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// Internal helper to spawn MG turret at specified position
/// Returns the base entity for tracking/undo purposes
fn spawn_mg_turret_internal(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    x: f32,
    z: f32,
    terrain_height: f32,
) -> Entity {
    let turret_world_pos = Vec3::new(x, terrain_height, z);

    // Create meshes
    let base_mesh = create_mg_turret_base_mesh(meshes);
    let assembly_mesh = create_mg_turret_assembly_mesh(meshes);

    // Create materials (Darker, more industrial)
    let base_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.2, 0.2),
        metallic: 0.8,
        perceptual_roughness: 0.6,
        ..default()
    });

    let gun_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.35, 0.3), // Slightly greenish military metal
        metallic: 0.9,
        perceptual_roughness: 0.4,
        ..default()
    });

    // Spawn base entity (parent)
    let base_entity = commands.spawn((
        Mesh3d(base_mesh),
        MeshMaterial3d(base_material),
        Transform::from_translation(turret_world_pos),
        TurretBase { team: Team::A },
        BuildingCollider { radius: 3.0 }, // Smaller collision radius
        Health::new(MG_TURRET_HEALTH),
    )).id();

    // Spawn rotating assembly entity (child)
    let assembly_entity = commands.spawn((
        Mesh3d(assembly_mesh),
        MeshMaterial3d(gun_material),
        Transform::from_xyz(0.0, 1.0, 0.0), // Mounted on top of base
        BattleDroid {
            team: Team::A,
            march_speed: 0.0,
            spawn_position: turret_world_pos,
            target_position: turret_world_pos,
            march_offset: 0.0,
            returning_to_spawn: false,
        },
        CombatUnit {
            target_scan_timer: 0.0,
            auto_fire_timer: 0.3, // Faster fire rate for MG
            current_target: None,
        },
        TurretRotatingAssembly {
            current_barrel_index: 0,
        },
        MgTurret {
            firing_mode: FiringMode::Continuous, // Start with Continuous mode for mowing down
            shots_in_burst: 0,
            max_burst_shots: 45,   // 45 shots at 0.05s = ~2.25 seconds before pause (20 shots/sec)
            cooldown_timer: 0.0,   // No cooldown initially
            cooldown_duration: 1.5, // 1.5 second pause between bursts/sweeps
        },
    )).id();

    // Link child to parent
    commands.entity(base_entity).add_children(&[assembly_entity]);

    base_entity
}

/// Spawn a functional MG turret (only if enabled in debug mode)
pub fn spawn_mg_turret(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    heightmap: Res<TerrainHeightmap>,
    debug_mode: Res<crate::objective::ExplosionDebugMode>,
) {
    if !debug_mode.mg_turret_enabled {
        info!("MG turret disabled at startup (press 0 then M to enable)");
        return;
    }

    let x = 10.0;
    let z = 10.0;
    let terrain_height = heightmap.sample_height(x, z);

    let base_entity = spawn_mg_turret_internal(&mut commands, &mut meshes, &mut materials, x, z, terrain_height);
    info!("Spawned MG turret BASE ENTITY {:?} at position ({}, {}, {})", base_entity, x, terrain_height, z);
}

/// Internal helper to spawn heavy turret at specified position
/// Returns the base entity for tracking/undo purposes
fn spawn_heavy_turret_internal(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    x: f32,
    z: f32,
    terrain_height: f32,
) -> Entity {
    let turret_world_pos = Vec3::new(x, terrain_height, z);

    // Create meshes
    let base_mesh = create_turret_base_mesh(meshes);
    let assembly_mesh = create_turret_rotating_assembly_mesh(meshes);

    // Create materials
    let base_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.3, 0.35),
        metallic: 0.1,
        perceptual_roughness: 0.8,
        ..default()
    });

    let gun_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.25, 0.3),
        metallic: 0.8,
        perceptual_roughness: 0.4,
        ..default()
    });

    // Spawn base entity (parent)
    let base_entity = commands.spawn((
        Mesh3d(base_mesh),
        MeshMaterial3d(base_material),
        Transform::from_translation(turret_world_pos),
        TurretBase { team: Team::A },
        BuildingCollider { radius: 4.0 }, // Collision radius for laser blocking
        Health::new(HEAVY_TURRET_HEALTH),
    )).id();

    // Spawn rotating assembly entity (child)
    let assembly_entity = commands.spawn((
        Mesh3d(assembly_mesh),
        MeshMaterial3d(gun_material),
        Transform::from_xyz(0.0, 2.7, 0.0), // Mounted on top of base
        BattleDroid {
            team: Team::A,
            march_speed: 0.0,
            spawn_position: turret_world_pos,
            target_position: turret_world_pos,
            march_offset: 0.0,
            returning_to_spawn: false,
        },
        CombatUnit {
            target_scan_timer: 0.0,
            auto_fire_timer: 2.0,
            current_target: None,
        },
        TurretRotatingAssembly {
            current_barrel_index: 0,
        },
    )).id();

    // Link child to parent
    commands.entity(base_entity).add_children(&[assembly_entity]);

    base_entity
}

/// Spawn a functional heavy turret (only if enabled in debug mode)
pub fn spawn_functional_turret(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    heightmap: Res<TerrainHeightmap>,
    debug_mode: Res<crate::objective::ExplosionDebugMode>,
) {
    if !debug_mode.heavy_turret_enabled {
        info!("Heavy turret disabled at startup (press 0 then H to enable)");
        return;
    }

    let x = 30.0;
    let z = 30.0;
    let terrain_height = heightmap.sample_height(x, z);

    spawn_heavy_turret_internal(&mut commands, &mut meshes, &mut materials, x, z, terrain_height);
    info!("Spawned functional turret at position ({}, {}, {})", x, terrain_height, z);
}

/// System to respawn turrets when map switches
pub fn respawn_turrets_on_map_switch(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    heightmap: Res<TerrainHeightmap>,
    mut map_switch_events: EventReader<MapSwitchEvent>,
    turret_base_query: Query<Entity, With<TurretBase>>,
) {
    // Only process if there's a map switch event
    if map_switch_events.read().next().is_none() {
        return;
    }

    // Despawn all existing turrets (both base and assembly entities)
    for base_entity in turret_base_query.iter() {
        commands.entity(base_entity).despawn();
    }

    info!("Respawning turrets for new terrain");

    // Spawn MG turret at new terrain height
    let mg_x = 10.0;
    let mg_z = 10.0;
    let mg_height = heightmap.sample_height(mg_x, mg_z);
    spawn_mg_turret_internal(&mut commands, &mut meshes, &mut materials, mg_x, mg_z, mg_height);
    info!("Respawned MG turret at ({}, {}, {})", mg_x, mg_height, mg_z);

    // Spawn heavy turret at new terrain height
    let heavy_x = 30.0;
    let heavy_z = 30.0;
    let heavy_height = heightmap.sample_height(heavy_x, heavy_z);
    spawn_heavy_turret_internal(&mut commands, &mut meshes, &mut materials, heavy_x, heavy_z, heavy_height);
    info!("Respawned heavy turret at ({}, {}, {})", heavy_x, heavy_height, heavy_z);
}

// ============================================================================
// PUBLIC API FOR SCENARIO SYSTEM
// ============================================================================

/// Spawn an MG turret at the specified world position
/// Used by scenario systems to place turrets programmatically
/// Returns the turret base entity for tracking/undo
pub fn spawn_mg_turret_at(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    position: Vec3,
) -> Entity {
    spawn_mg_turret_internal(commands, meshes, materials, position.x, position.z, position.y)
}

/// Spawn a heavy turret at the specified world position
/// Used by scenario systems to place turrets programmatically
/// Returns the turret base entity for tracking/undo
pub fn spawn_heavy_turret_at(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    position: Vec3,
) -> Entity {
    spawn_heavy_turret_internal(commands, meshes, materials, position.x, position.z, position.y)
}

/// Debug system to toggle turrets on/off (M=MG, H=Heavy) when debug mode is active
pub fn debug_turret_toggle_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    heightmap: Res<TerrainHeightmap>,
    mut debug_mode: ResMut<crate::objective::ExplosionDebugMode>,
    mg_turret_query: Query<Entity, With<MgTurret>>,
    // Heavy turret has TurretRotatingAssembly but NOT MgTurret
    heavy_turret_query: Query<Entity, (With<TurretRotatingAssembly>, Without<MgTurret>)>,
    turret_base_query: Query<(Entity, &Children), With<TurretBase>>,
) {
    // Only process when debug mode is active
    if !debug_mode.explosion_mode {
        return;
    }

    // M key: Toggle MG turret
    if keyboard_input.just_pressed(KeyCode::KeyM) {
        debug_mode.mg_turret_enabled = !debug_mode.mg_turret_enabled;

        if debug_mode.mg_turret_enabled {
            // Spawn MG turret
            let x = 10.0;
            let z = 10.0;
            let terrain_height = heightmap.sample_height(x, z);
            spawn_mg_turret_internal(&mut commands, &mut meshes, &mut materials, x, z, terrain_height);
            info!("🔫 MG turret ENABLED");
        } else {
            // Despawn MG turret - find the base that has MG turret as child
            for (base_entity, children) in turret_base_query.iter() {
                for child in children.iter() {
                    if mg_turret_query.get(child).is_ok() {
                        commands.entity(base_entity).despawn();
                        info!("🔫 MG turret DISABLED");
                        break;
                    }
                }
            }
        }
    }

    // H key: Toggle Heavy turret
    if keyboard_input.just_pressed(KeyCode::KeyH) {
        debug_mode.heavy_turret_enabled = !debug_mode.heavy_turret_enabled;

        if debug_mode.heavy_turret_enabled {
            // Spawn Heavy turret
            let x = 30.0;
            let z = 30.0;
            let terrain_height = heightmap.sample_height(x, z);
            spawn_heavy_turret_internal(&mut commands, &mut meshes, &mut materials, x, z, terrain_height);
            info!("🔫 Heavy turret ENABLED");
        } else {
            // Despawn Heavy turret - find the base that has heavy turret (no MgTurret) as child
            for (base_entity, children) in turret_base_query.iter() {
                for child in children.iter() {
                    if heavy_turret_query.get(child).is_ok() {
                        commands.entity(base_entity).despawn();
                        info!("🔫 Heavy turret DISABLED");
                        break;
                    }
                }
            }
        }
    }
}

// ============================================================================
// HEALTH BAR SYSTEMS
// ============================================================================

/// Spawn a shader-based health bar for a turret (single quad, no z-fighting)
fn spawn_health_bar_for_turret(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    health_bar_materials: &mut Assets<HealthBarMaterial>,
    turret_entity: Entity,
    turret_pos: Vec3,
) {
    // Create single quad mesh for the health bar
    let bar_mesh = meshes.add(Mesh::from(Rectangle::new(HEALTH_BAR_WIDTH, HEALTH_BAR_HEIGHT)));

    // Shader-based material - health_fraction controls green/gray split
    let bar_material = health_bar_materials.add(HealthBarMaterial {
        health_data: Vec4::new(1.0, 0.0, 0.0, 0.0), // Full health initially
    });

    let bar_y = turret_pos.y + HEALTH_BAR_Y_OFFSET;

    // Spawn single health bar quad
    commands.spawn((
        Mesh3d(bar_mesh),
        MeshMaterial3d(bar_material),
        Transform::from_translation(Vec3::new(turret_pos.x, bar_y, turret_pos.z)),
        TurretHealthBar { turret_entity },
        bevy::pbr::NotShadowCaster,
    ));
}

/// System to spawn health bars for turrets that don't have them yet
pub fn spawn_turret_health_bars(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut health_bar_materials: ResMut<Assets<HealthBarMaterial>>,
    turret_query: Query<(Entity, &Transform), (With<TurretBase>, With<Health>)>,
    health_bar_query: Query<&TurretHealthBar>,
) {
    // Find turrets without health bars
    for (turret_entity, transform) in turret_query.iter() {
        let has_bar = health_bar_query.iter().any(|bar| bar.turret_entity == turret_entity);
        if !has_bar {
            spawn_health_bar_for_turret(
                &mut commands,
                &mut meshes,
                &mut health_bar_materials,
                turret_entity,
                transform.translation,
            );
        }
    }
}

/// System to update health bar position, rotation, and shader uniform based on turret health
pub fn update_turret_health_bars(
    mut commands: Commands,
    turret_query: Query<(Entity, &Transform, &Health), With<TurretBase>>,
    mut bar_query: Query<(Entity, &TurretHealthBar, &mut Transform, &MeshMaterial3d<HealthBarMaterial>), Without<TurretBase>>,
    mut health_bar_materials: ResMut<Assets<HealthBarMaterial>>,
    camera_query: Query<&Transform, (With<crate::types::RtsCamera>, Without<TurretBase>, Without<TurretHealthBar>)>,
) {
    let Ok(camera_transform) = camera_query.single() else { return };

    // Create a lookup for turret health and position
    let turrets: std::collections::HashMap<Entity, (&Transform, &Health)> = turret_query
        .iter()
        .map(|(e, t, h)| (e, (t, h)))
        .collect();

    // Billboard rotation: use camera's rotation so bars are always parallel to camera view plane
    let billboard_rotation = camera_transform.rotation;

    // Update all health bars
    for (bar_entity, health_bar, mut bar_transform, material_handle) in bar_query.iter_mut() {
        if let Some((turret_transform, health)) = turrets.get(&health_bar.turret_entity) {
            let health_fraction = health.current / health.max;

            // Update position to follow turret
            let bar_pos = Vec3::new(
                turret_transform.translation.x,
                turret_transform.translation.y + HEALTH_BAR_Y_OFFSET,
                turret_transform.translation.z,
            );
            bar_transform.translation = bar_pos;
            bar_transform.rotation = billboard_rotation;

            // Update shader uniform with current health fraction
            if let Some(material) = health_bar_materials.get_mut(&material_handle.0) {
                material.health_data.x = health_fraction;
            }
        } else {
            // Turret no longer exists, despawn bar
            commands.entity(bar_entity).despawn();
        }
    }
}

/// System to check turret health and trigger explosion + despawn when dead
/// Uses WFX billboard explosion for better visual quality than hanabi particles
pub fn turret_death_system(
    mut commands: Commands,
    turret_query: Query<(Entity, &Transform, &Health), With<TurretBase>>,
    audio_assets: Res<crate::types::AudioAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut additive_materials: ResMut<Assets<crate::wfx_materials::AdditiveMaterial>>,
    mut smoke_materials: ResMut<Assets<crate::wfx_materials::SmokeScrollMaterial>>,
    asset_server: Res<AssetServer>,
) {
    for (entity, transform, health) in turret_query.iter() {
        if health.current <= 0.0 {
            let position = transform.translation;
            info!("Turret destroyed at {:?}", position);

            // Play explosion sound (smaller volume than tower explosions)
            commands.spawn((
                AudioPlayer::new(audio_assets.explosion_sound.clone()),
                PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(crate::constants::VOLUME_TURRET_EXPLOSION)),
            ));

            // Spawn WFX billboard explosion (28 flames + 50 dot sparkles + 5 glow sparkles + 1 center glow)
            crate::wfx_spawn::spawn_turret_wfx_explosion(
                &mut commands,
                &mut meshes,
                &mut additive_materials,
                &mut smoke_materials,
                &asset_server,
                position + Vec3::Y * 2.0,
                1.5,
            );

            // Despawn the turret
            commands.entity(entity).despawn();
        }
    }
}

/// Old turret death system using hanabi GPU particles
/// Kept for reference - hanabi particles are faster but less visually detailed than WFX billboards
#[allow(dead_code)]
pub fn turret_death_system_hanabi(
    mut commands: Commands,
    turret_query: Query<(Entity, &Transform, &Health), With<TurretBase>>,
    particle_effects: Option<Res<crate::particles::ExplosionParticleEffects>>,
    audio_assets: Res<crate::types::AudioAssets>,
    time: Res<Time>,
) {
    for (entity, transform, health) in turret_query.iter() {
        if health.current <= 0.0 {
            let position = transform.translation;
            info!("Turret destroyed at {:?}", position);

            // Play explosion sound (smaller volume than tower explosions)
            commands.spawn((
                AudioPlayer::new(audio_assets.explosion_sound.clone()),
                PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(crate::constants::VOLUME_TURRET_EXPLOSION)),
            ));

            // Spawn hanabi particle explosion for turrets (more sparks/flames)
            if let Some(ref particles) = particle_effects {
                info!("Spawning explosion particles for turret at {:?}", position);
                crate::particles::spawn_turret_explosion_particles(
                    &mut commands,
                    particles,
                    position + Vec3::Y * 2.0,
                    1.5,
                    time.elapsed_secs_f64(),
                );
            } else {
                warn!("ExplosionParticleEffects resource not available for turret explosion!");
            }

            // Despawn the turret
            commands.entity(entity).despawn();
        }
    }
}

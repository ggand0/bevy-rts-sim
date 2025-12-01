use bevy::prelude::*;
use bevy::pbr::MaterialPlugin;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::alpha::AlphaMode;
use crate::types::Team;

// Shield configuration
#[derive(Resource, Clone)]
pub struct ShieldConfig {
    pub max_hp: f32,
    pub regen_rate: f32,           // HP per second
    pub regen_delay: f32,           // Seconds after last hit before regen starts
    pub respawn_delay: f32,         // Seconds after destruction before respawn
    pub impact_flash_duration: f32, // Seconds
    pub laser_damage: f32,          // Damage per laser hit
    #[allow(dead_code)]
    pub particle_scale: f32,        // Scale for impact particles (reserved for future use)
    pub shield_impact_volume: f32,  // Audio volume for shield impacts
    pub surface_offset: f32,        // Offset for particle spawn from surface
    pub fresnel_power: f32,         // Fresnel edge glow exponent
    pub hex_scale: f32,             // Hexagonal pattern scale
    pub mesh_segments: u32,         // Hemisphere mesh detail (vertices)
}

impl Default for ShieldConfig {
    fn default() -> Self {
        Self {
            max_hp: 5000.0,
            regen_rate: 50.0,
            regen_delay: 3.0,
            respawn_delay: 10.0,
            impact_flash_duration: 0.2,
            laser_damage: 25.0,
            particle_scale: 2.0,
            shield_impact_volume: 0.4,
            surface_offset: 0.5,
            fresnel_power: 3.0,
            hex_scale: 8.0,
            mesh_segments: 32,
        }
    }
}

// Visual effect constants
const MAX_RIPPLES: usize = 8; // Maximum simultaneous ripple effects
const RIPPLE_DURATION: f32 = 1.5; // Seconds for ripple to expand and fade
const RIPPLE_SPAWN_CHANCE: f32 = 0.25; // 25% chance to spawn ripple on hit

// Shield visual alpha constants
const SHIELD_BASE_ALPHA_MIN: f32 = 0.3; // Minimum alpha multiplier at 0 HP
const SHIELD_BASE_ALPHA_MAX: f32 = 0.7; // Additional alpha multiplier at full HP
const SHIELD_IMPACT_FLASH_ALPHA: f32 = 0.5; // Alpha boost during impact flash
const SHIELD_MAX_ALPHA: f32 = 0.8; // Maximum total alpha

pub struct ShieldPlugin;

impl Plugin for ShieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ShieldMaterial>::default())
            .insert_resource(ShieldConfig::default());
        // Shield systems are registered in main.rs with proper ordering
    }
}

/// Represents a single ripple effect on the shield
#[derive(Clone, Copy)]
pub struct ShieldRipple {
    pub position: Vec3,      // World position of impact
    pub spawn_time: f32,     // When the ripple was created
}

impl Default for ShieldRipple {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            spawn_time: -999.0, // Inactive by default
        }
    }
}

/// Active shield component with full state tracking
/// When shield HP reaches 0, this component is removed and replaced with DestroyedShield
#[derive(Component)]
pub struct Shield {
    pub material_handle: Handle<ShieldMaterial>,
    pub team: Team,
    pub radius: f32,
    pub center: Vec3,
    pub current_hp: f32,
    pub max_hp: f32,
    pub last_hit_time: f32,
    pub impact_flash_timer: f32,
    pub base_alpha: f32,
    pub ripples: [ShieldRipple; MAX_RIPPLES],
}

impl Shield {
    #[allow(dead_code)]
    pub fn new(team: Team, radius: f32, center: Vec3, material_handle: Handle<ShieldMaterial>, max_hp: f32) -> Self {
        Self::with_hp(team, radius, center, material_handle, max_hp, max_hp)
    }

    pub fn with_hp(team: Team, radius: f32, center: Vec3, material_handle: Handle<ShieldMaterial>, max_hp: f32, starting_hp: f32) -> Self {
        Self {
            material_handle,
            team,
            radius,
            center,
            current_hp: starting_hp,
            max_hp,
            last_hit_time: -999.0,
            impact_flash_timer: 0.0,
            base_alpha: 0.2,
            ripples: [ShieldRipple::default(); MAX_RIPPLES],
        }
    }

    pub fn take_damage(&mut self, damage: f32, current_time: f32, flash_duration: f32) {
        self.current_hp = (self.current_hp - damage).max(0.0);
        self.last_hit_time = current_time;
        self.impact_flash_timer = flash_duration;
    }

    pub fn add_ripple(&mut self, position: Vec3, current_time: f32) {
        // Find oldest ripple slot or first inactive slot
        let mut oldest_idx = 0;
        let mut oldest_time = self.ripples[0].spawn_time;

        for (i, ripple) in self.ripples.iter().enumerate() {
            // Check if inactive (very old)
            if current_time - ripple.spawn_time > RIPPLE_DURATION {
                oldest_idx = i;
                break;
            }
            // Track oldest for replacement
            if ripple.spawn_time < oldest_time {
                oldest_time = ripple.spawn_time;
                oldest_idx = i;
            }
        }

        // Add new ripple
        self.ripples[oldest_idx] = ShieldRipple {
            position,
            spawn_time: current_time,
        };
    }

    pub fn is_destroyed(&self) -> bool {
        self.current_hp <= 0.0
    }

    pub fn health_percent(&self) -> f32 {
        self.current_hp / self.max_hp
    }
}

/// Marker component for destroyed shields waiting to respawn
/// Replaces the Shield component when HP reaches 0
/// After respawn_timer expires, this is removed and a new Shield is spawned at 0 HP
#[derive(Component)]
pub struct DestroyedShield {
    pub team: Team,
    pub position: Vec3,
    pub radius: f32,
    pub team_color: Color,
    pub respawn_timer: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ShieldMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(0)]
    pub fresnel_power: f32,
    #[uniform(0)]
    pub hex_scale: f32,
    #[uniform(0)]
    pub time: f32,
    #[uniform(0)]
    pub health_percent: f32,                   // 0.0 = dead, 1.0 = full health
    #[uniform(0)]
    pub shield_center: Vec3,                   // Shield center for ripple calculation
    #[uniform(0)]
    pub shield_radius: f32,                    // Shield radius
    #[uniform(0)]
    pub ripple_data: [Vec4; MAX_RIPPLES],      // x,y,z = position, w = time
}

impl Material for ShieldMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/shield.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        0.0
    }
}

impl Default for ShieldMaterial {
    fn default() -> Self {
        Self {
            color: LinearRgba::rgb(0.2, 0.6, 1.0), // Cyan/blue
            fresnel_power: 3.0,
            hex_scale: 8.0,
            time: 0.0,
            health_percent: 1.0,
            shield_center: Vec3::ZERO,
            shield_radius: 50.0,
            ripple_data: [Vec4::ZERO; MAX_RIPPLES],
        }
    }
}

/// Creates a hemisphere mesh (upper half of a sphere)
pub fn create_hemisphere_mesh(radius: f32, segments: u32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Generate vertices for upper hemisphere only
    for lat in 0..=segments {
        let theta = std::f32::consts::PI * 0.5 * (lat as f32) / (segments as f32); // 0 to PI/2
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for lon in 0..=segments {
            let phi = 2.0 * std::f32::consts::PI * (lon as f32) / (segments as f32);
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();

            let x = sin_theta * cos_phi;
            let y = cos_theta;
            let z = sin_theta * sin_phi;

            positions.push([x * radius, y * radius, z * radius]);
            normals.push([x, y, z]);
            uvs.push([lon as f32 / segments as f32, lat as f32 / segments as f32]);
        }
    }

    // Generate indices (reversed winding for outward-facing normals)
    for lat in 0..segments {
        for lon in 0..segments {
            let first = lat * (segments + 1) + lon;
            let second = first + segments + 1;

            // First triangle (counter-clockwise from outside)
            indices.push(first);
            indices.push(first + 1);
            indices.push(second);

            // Second triangle (counter-clockwise from outside)
            indices.push(second);
            indices.push(first + 1);
            indices.push(second + 1);
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::render::render_asset::RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// Creates a shield material with the given parameters
fn create_shield_material(
    team_color: Color,
    position: Vec3,
    radius: f32,
    health_percent: f32,
    config: &ShieldConfig,
) -> ShieldMaterial {
    ShieldMaterial {
        color: team_color.to_linear(),
        fresnel_power: config.fresnel_power,
        hex_scale: config.hex_scale,
        time: 0.0,
        health_percent,
        shield_center: position,
        shield_radius: radius,
        ripple_data: [Vec4::ZERO; MAX_RIPPLES],
    }
}

/// Spawns a shield around a position
pub fn spawn_shield(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ShieldMaterial>>,
    position: Vec3,
    radius: f32,
    team_color: Color,
    team: Team,
    config: &ShieldConfig,
) -> Entity {
    spawn_shield_with_hp(commands, meshes, materials, position, radius, team_color, team, config, config.max_hp)
}

/// Spawns a shield around a position with custom starting HP
pub fn spawn_shield_with_hp(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ShieldMaterial>>,
    position: Vec3,
    radius: f32,
    team_color: Color,
    team: Team,
    config: &ShieldConfig,
    starting_hp: f32,
) -> Entity {
    let shield_mesh = create_hemisphere_mesh(radius, config.mesh_segments);
    let health_percent = starting_hp / config.max_hp;
    let shield_material = create_shield_material(team_color, position, radius, health_percent, config);
    let material_handle = materials.add(shield_material);

    commands.spawn((
        Mesh3d(meshes.add(shield_mesh)),
        MeshMaterial3d(material_handle.clone()),
        Transform::from_translation(position),
        Shield::with_hp(team, radius, position, material_handle.clone(), config.max_hp, starting_hp),
        bevy::pbr::NotShadowCaster,
        bevy::pbr::NotShadowReceiver,
    )).id()
}

/// Animates shield time for energy pulses
pub fn animate_shields(
    time: Res<Time>,
    mut materials: ResMut<Assets<ShieldMaterial>>,
    query: Query<&Shield>,
) {
    for shield in query.iter() {
        if let Some(material) = materials.get_mut(&shield.material_handle) {
            material.time = time.elapsed_secs();
        }
    }
}

/// Detects laser collisions with shields and applies damage
pub fn shield_collision_system(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<ShieldConfig>,
    mut shield_query: Query<(Entity, &mut Shield)>,
    laser_query: Query<(Entity, &crate::types::LaserProjectile, &Transform)>,
    // particle_effects: Res<crate::particles::ExplosionParticleEffects>,  // Temporarily disabled
    audio_assets: Res<crate::types::AudioAssets>,
) {
    let current_time = time.elapsed_secs();
    let current_time_f64 = time.elapsed_secs_f64();

    for (shield_entity, mut shield) in shield_query.iter_mut() {
        for (laser_entity, laser, laser_transform) in laser_query.iter() {
            // Only check enemy lasers
            if laser.team == shield.team {
                continue;
            }

            // Check if laser is within shield radius (3D sphere check)
            let laser_pos = laser_transform.translation;
            let distance = shield.center.distance(laser_pos);

            // Simple sphere collision - laser within shield radius
            if distance < shield.radius {
                // Shield blocks the laser
                shield.take_damage(config.laser_damage, current_time, config.impact_flash_duration);

                // 25% chance to spawn ripple effect and particles
                if rand::random::<f32>() < RIPPLE_SPAWN_CHANCE {
                    shield.add_ripple(laser_pos, current_time);

                    // Calculate impact point on shield surface
                    let dir_to_laser = (laser_pos - shield.center).normalize();
                    let surface_pos = shield.center + dir_to_laser * (shield.radius + config.surface_offset);

                    // Spawn particle effect at surface impact point
                    // crate::particles::spawn_shield_impact_particles(  // Temporarily disabled
                    //     &mut commands,
                    //     &particle_effects,
                    //     surface_pos,
                    //     current_time_f64,
                    // );

                    // Play shield impact sound
                    commands.spawn((
                        AudioPlayer::new(audio_assets.shield_impact_sound.clone()),
                        PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(config.shield_impact_volume)),
                    ));
                }

                commands.entity(laser_entity).despawn();

                info!(
                    "Shield {:?} hit! HP: {:.1}/{:.1} ({:.0}%)",
                    shield.team,
                    shield.current_hp,
                    shield.max_hp,
                    shield.health_percent() * 100.0
                );

                // Destroy shield if HP depleted
                if shield.is_destroyed() {
                    info!("Shield {:?} destroyed!", shield.team);

                    // Spawn destroyed shield marker for respawn
                    commands.spawn(DestroyedShield {
                        team: shield.team,
                        position: shield.center,
                        radius: shield.radius,
                        team_color: shield.team.shield_color(),
                        respawn_timer: config.respawn_delay,
                    });

                    commands.entity(shield_entity).despawn();
                }
            }
        }
    }
}

/// Regenerates shield HP over time after damage
pub fn shield_regeneration_system(
    time: Res<Time>,
    config: Res<ShieldConfig>,
    mut query: Query<&mut Shield>,
) {
    let current_time = time.elapsed_secs();
    let delta = time.delta_secs();

    for mut shield in query.iter_mut() {
        if shield.current_hp < shield.max_hp {
            // Check if enough time has passed since last hit
            if current_time - shield.last_hit_time >= config.regen_delay {
                let regen_amount = config.regen_rate * delta;
                shield.current_hp = (shield.current_hp + regen_amount).min(shield.max_hp);
            }
        }
    }
}

/// Handles impact flash visual feedback
pub fn shield_impact_flash_system(
    time: Res<Time>,
    mut query: Query<&mut Shield>,
) {
    let delta = time.delta_secs();

    for mut shield in query.iter_mut() {
        if shield.impact_flash_timer > 0.0 {
            shield.impact_flash_timer = (shield.impact_flash_timer - delta).max(0.0);
        }
    }
}

/// Updates shield material alpha based on health and impacts
pub fn shield_health_visual_system(
    time: Res<Time>,
    config: Res<ShieldConfig>,
    mut materials: ResMut<Assets<ShieldMaterial>>,
    query: Query<&Shield>,
) {
    let current_time = time.elapsed_secs();

    for shield in query.iter() {
        if let Some(material) = materials.get_mut(&shield.material_handle) {
            // Calculate alpha based on health (fade out as HP decreases)
            let health_alpha = shield.base_alpha * (SHIELD_BASE_ALPHA_MIN + SHIELD_BASE_ALPHA_MAX * shield.health_percent());

            // Add flash effect on impact
            let flash_intensity = shield.impact_flash_timer / config.impact_flash_duration;
            let impact_alpha = flash_intensity * SHIELD_IMPACT_FLASH_ALPHA;

            // Update health percent for shader (shader will interpolate color to white)
            material.health_percent = shield.health_percent();

            // Update ripple data for shader (pack position and time into Vec4)
            for (i, ripple) in shield.ripples.iter().enumerate() {
                let ripple_age = current_time - ripple.spawn_time;
                material.ripple_data[i] = Vec4::new(
                    ripple.position.x,
                    ripple.position.y,
                    ripple.position.z,
                    ripple_age,
                );
            }

            // Set final alpha
            material.color.alpha = (health_alpha + impact_alpha).min(SHIELD_MAX_ALPHA);
        }
    }
}

/// Despawns active shields when their tower is destroyed
pub fn shield_tower_death_system(
    mut commands: Commands,
    shield_query: Query<(Entity, &Shield)>,
    tower_query: Query<(&crate::types::UplinkTower, &crate::types::Health)>,
) {
    for (shield_entity, shield) in shield_query.iter() {
        // Check if the tower for this shield's team is dead
        let tower_dead = tower_query.iter()
            .any(|(tower, health)| tower.team == shield.team && health.is_dead());

        if tower_dead {
            info!("Despawning shield for team {:?} - tower destroyed", shield.team);
            commands.entity(shield_entity).despawn();
        }
    }
}

/// Handles shield respawn after destruction
pub fn shield_respawn_system(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<ShieldConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ShieldMaterial>>,
    mut query: Query<(Entity, &mut DestroyedShield)>,
    tower_query: Query<(&crate::types::UplinkTower, &crate::types::Health)>,
) {
    let delta = time.delta_secs();

    for (entity, mut destroyed) in query.iter_mut() {
        // Check if the tower for this team is still alive
        let tower_alive = tower_query.iter()
            .any(|(tower, health)| tower.team == destroyed.team && !health.is_dead());

        if !tower_alive {
            // Tower is destroyed, don't respawn shield - just remove the marker
            info!("Shield for team {:?} will not respawn - tower is destroyed", destroyed.team);
            commands.entity(entity).despawn();
            continue;
        }

        destroyed.respawn_timer -= delta;

        if destroyed.respawn_timer <= 0.0 {
            info!("Respawning shield for team {:?} at 0 HP - will regenerate", destroyed.team);

            // Respawn the shield at 0 HP - it will gradually regenerate to full
            spawn_shield_with_hp(
                &mut commands,
                &mut meshes,
                &mut materials,
                destroyed.position,
                destroyed.radius,
                destroyed.team_color,
                destroyed.team,
                &config,
                0.0, // Start at 0 HP, will regenerate like Empire at War
            );

            // Remove the destroyed marker
            commands.entity(entity).despawn();
        }
    }
}

/// Debug system: Press 'S' (when debug mode active) to set enemy (Team B) shield HP to zero
pub fn debug_destroy_enemy_shield(
    keyboard: Res<ButtonInput<KeyCode>>,
    config: Res<ShieldConfig>,
    mut shield_query: Query<&mut Shield>,
    time: Res<Time>,
    debug_mode: Res<crate::objective::ExplosionDebugMode>,
) {
    // Only work when debug mode is active
    if !debug_mode.explosion_mode {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyS) {
        let current_time = time.elapsed_secs();

        for mut shield in shield_query.iter_mut() {
            if shield.team == Team::B {
                info!("DEBUG: Setting Team B shield HP to 0");
                shield.current_hp = 0.0;
                shield.last_hit_time = current_time;
                shield.impact_flash_timer = config.impact_flash_duration;
            }
        }
    }
}

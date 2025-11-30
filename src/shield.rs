use bevy::prelude::*;
use bevy::pbr::MaterialPlugin;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::alpha::AlphaMode;
use crate::types::Team;

// Shield configuration constants
const SHIELD_MAX_HP: f32 = 5000.0;
const SHIELD_REGEN_RATE: f32 = 50.0; // HP per second
const SHIELD_REGEN_DELAY: f32 = 3.0; // Seconds after last hit before regen starts
const SHIELD_RESPAWN_DELAY: f32 = 10.0; // Seconds after destruction before respawn
const SHIELD_IMPACT_FLASH_DURATION: f32 = 0.2; // Seconds
const MAX_RIPPLES: usize = 8; // Maximum simultaneous ripple effects
const RIPPLE_DURATION: f32 = 1.5; // Seconds for ripple to expand and fade
const RIPPLE_SPAWN_CHANCE: f32 = 0.25; // 25% chance to spawn ripple on hit

pub struct ShieldPlugin;

impl Plugin for ShieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ShieldMaterial>::default());
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
    pub fn new(team: Team, radius: f32, center: Vec3, material_handle: Handle<ShieldMaterial>) -> Self {
        Self {
            material_handle,
            team,
            radius,
            center,
            current_hp: SHIELD_MAX_HP,
            max_hp: SHIELD_MAX_HP,
            last_hit_time: -999.0,
            impact_flash_timer: 0.0,
            base_alpha: 0.2,
            ripples: [ShieldRipple::default(); MAX_RIPPLES],
        }
    }

    pub fn take_damage(&mut self, damage: f32, current_time: f32) {
        self.current_hp = (self.current_hp - damage).max(0.0);
        self.last_hit_time = current_time;
        self.impact_flash_timer = SHIELD_IMPACT_FLASH_DURATION;
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

/// Marker for destroyed shields waiting to respawn
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
    pub _padding1: f32,
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
            _padding1: 0.0,
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

/// Spawns a shield around a position
pub fn spawn_shield(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ShieldMaterial>>,
    position: Vec3,
    radius: f32,
    team_color: Color,
    team: Team,
) -> Entity {
    let shield_mesh = create_hemisphere_mesh(radius, 32);

    let shield_material = ShieldMaterial {
        color: team_color.to_linear(),
        fresnel_power: 3.0,
        hex_scale: 8.0,
        time: 0.0,
        _padding1: 0.0,
        shield_center: position,
        shield_radius: radius,
        ripple_data: [Vec4::ZERO; MAX_RIPPLES],
    };

    let material_handle = materials.add(shield_material);

    commands.spawn((
        Mesh3d(meshes.add(shield_mesh)),
        MeshMaterial3d(material_handle.clone()),
        Transform::from_translation(position),
        Shield::new(team, radius, position, material_handle.clone()),
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
    mut shield_query: Query<(Entity, &mut Shield)>,
    laser_query: Query<(Entity, &crate::types::LaserProjectile, &Transform)>,
) {
    let current_time = time.elapsed_secs();

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
                shield.take_damage(25.0, current_time);

                // 25% chance to spawn ripple effect
                if rand::random::<f32>() < RIPPLE_SPAWN_CHANCE {
                    shield.add_ripple(laser_pos, current_time);
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
                    let team_color = if shield.team == crate::types::Team::A {
                        Color::srgb(0.2, 0.6, 1.0)
                    } else {
                        Color::srgb(1.0, 0.4, 0.2)
                    };

                    commands.spawn(DestroyedShield {
                        team: shield.team,
                        position: shield.center,
                        radius: shield.radius,
                        team_color,
                        respawn_timer: SHIELD_RESPAWN_DELAY,
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
    mut query: Query<&mut Shield>,
) {
    let current_time = time.elapsed_secs();
    let delta = time.delta_secs();

    for mut shield in query.iter_mut() {
        if shield.current_hp < shield.max_hp {
            // Check if enough time has passed since last hit
            if current_time - shield.last_hit_time >= SHIELD_REGEN_DELAY {
                let regen_amount = SHIELD_REGEN_RATE * delta;
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
    mut materials: ResMut<Assets<ShieldMaterial>>,
    query: Query<&Shield>,
) {
    let current_time = time.elapsed_secs();

    for shield in query.iter() {
        if let Some(material) = materials.get_mut(&shield.material_handle) {
            // Calculate alpha based on health (fade out as HP decreases)
            let health_alpha = shield.base_alpha * (0.3 + 0.7 * shield.health_percent());

            // Add flash effect on impact
            let flash_intensity = shield.impact_flash_timer / SHIELD_IMPACT_FLASH_DURATION;
            let impact_alpha = flash_intensity * 0.5;

            // Set final alpha (clamped)
            let base_color = material.color;
            material.color = LinearRgba {
                red: base_color.red * (1.0 + flash_intensity),
                green: base_color.green * (1.0 + flash_intensity),
                blue: base_color.blue * (1.0 + flash_intensity),
                alpha: (health_alpha + impact_alpha).min(0.8),
            };

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
        }
    }
}

/// Handles shield respawn after destruction
pub fn shield_respawn_system(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ShieldMaterial>>,
    mut query: Query<(Entity, &mut DestroyedShield)>,
) {
    let delta = time.delta_secs();

    for (entity, mut destroyed) in query.iter_mut() {
        destroyed.respawn_timer -= delta;

        if destroyed.respawn_timer <= 0.0 {
            info!("Respawning shield for team {:?}", destroyed.team);

            // Respawn the shield
            spawn_shield(
                &mut commands,
                &mut meshes,
                &mut materials,
                destroyed.position,
                destroyed.radius,
                destroyed.team_color,
                destroyed.team,
            );

            // Remove the destroyed marker
            commands.entity(entity).despawn();
        }
    }
}

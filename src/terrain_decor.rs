//! Terrain decoration module - rocks, vegetation, and atmospheric effects
//!
//! This module adds visual interest to the desert terrain with:
//! - Procedural rock/boulder placement using noise-based clustering
//! - Blowing sand particles for atmospheric effect

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use noise::{NoiseFn, Perlin};
use rand::prelude::*;
use std::f32::consts::PI;

use crate::terrain::{TerrainHeightmap, MapPreset, TerrainConfig, MapSwitchEvent, TerrainMarker};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Number of rocks to scatter across the terrain
const ROCK_COUNT: usize = 150;
/// Minimum rock scale
const ROCK_MIN_SCALE: f32 = 0.5;
/// Maximum rock scale
const ROCK_MAX_SCALE: f32 = 3.5;
/// Rock placement noise threshold (higher = more clustering)
const ROCK_NOISE_THRESHOLD: f64 = 0.2;
/// Noise scale for rock clustering
const ROCK_NOISE_SCALE: f64 = 0.015;

// Sand particle constants - disabled for now
#[allow(dead_code)]
const SAND_EMITTER_COUNT: usize = 20;
#[allow(dead_code)]
const SAND_PARTICLE_LIFETIME: f32 = 4.0;
#[allow(dead_code)]
const SAND_PARTICLE_SPEED: f32 = 8.0;
#[allow(dead_code)]
const SAND_HEIGHT_OFFSET: f32 = 0.5;

// ============================================================================
// PLUGIN
// ============================================================================

pub struct TerrainDecorPlugin;

impl Plugin for TerrainDecorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            spawn_terrain_decorations,
            // Sand particles disabled - visual effect needs work
            // animate_sand_particles,
        ));
    }
}

// ============================================================================
// COMPONENTS
// ============================================================================

/// Marker for rock entities
#[derive(Component)]
pub struct Rock;

/// Marker for terrain decoration entities (separate from TerrainMarker to avoid early cleanup)
#[derive(Component)]
pub struct TerrainDecoration;

/// Marker for sand particle entities (disabled)
#[allow(dead_code)]
#[derive(Component)]
pub struct SandParticle {
    pub velocity: Vec3,
    pub lifetime: f32,
    pub age: f32,
}

/// Marker for sand emitter positions (disabled)
#[allow(dead_code)]
#[derive(Component)]
pub struct SandEmitter {
    pub spawn_timer: f32,
    pub spawn_interval: f32,
}


// ============================================================================
// ROCK MESH GENERATION
// ============================================================================

/// Create a procedural rock mesh with irregular geometry
fn create_rock_mesh(meshes: &mut ResMut<Assets<Mesh>>, seed: u32) -> Handle<Mesh> {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);

    let mut rng = StdRng::seed_from_u64(seed as u64);

    // Base shape is a deformed sphere/icosahedron
    // Using 3 levels of subdivision for decent detail
    let subdivisions = 2;
    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Start with icosahedron vertices
    let phi = (1.0 + 5.0_f32.sqrt()) / 2.0;
    let base_verts = [
        Vec3::new(-1.0, phi, 0.0).normalize(),
        Vec3::new(1.0, phi, 0.0).normalize(),
        Vec3::new(-1.0, -phi, 0.0).normalize(),
        Vec3::new(1.0, -phi, 0.0).normalize(),
        Vec3::new(0.0, -1.0, phi).normalize(),
        Vec3::new(0.0, 1.0, phi).normalize(),
        Vec3::new(0.0, -1.0, -phi).normalize(),
        Vec3::new(0.0, 1.0, -phi).normalize(),
        Vec3::new(phi, 0.0, -1.0).normalize(),
        Vec3::new(phi, 0.0, 1.0).normalize(),
        Vec3::new(-phi, 0.0, -1.0).normalize(),
        Vec3::new(-phi, 0.0, 1.0).normalize(),
    ];

    // Icosahedron faces
    let base_faces = [
        (0, 11, 5), (0, 5, 1), (0, 1, 7), (0, 7, 10), (0, 10, 11),
        (1, 5, 9), (5, 11, 4), (11, 10, 2), (10, 7, 6), (7, 1, 8),
        (3, 9, 4), (3, 4, 2), (3, 2, 6), (3, 6, 8), (3, 8, 9),
        (4, 9, 5), (2, 4, 11), (6, 2, 10), (8, 6, 7), (9, 8, 1),
    ];

    // Subdivide and deform
    let mut current_verts: Vec<Vec3> = base_verts.to_vec();
    let mut current_faces: Vec<(usize, usize, usize)> = base_faces.to_vec();

    for _ in 0..subdivisions {
        let mut new_faces = Vec::new();
        let mut edge_midpoints: std::collections::HashMap<(usize, usize), usize> = std::collections::HashMap::new();

        for &(a, b, c) in &current_faces {
            let mut get_midpoint = |i1: usize, i2: usize| -> usize {
                let key = if i1 < i2 { (i1, i2) } else { (i2, i1) };
                if let Some(&idx) = edge_midpoints.get(&key) {
                    idx
                } else {
                    let mid = ((current_verts[i1] + current_verts[i2]) / 2.0).normalize();
                    let idx = current_verts.len();
                    current_verts.push(mid);
                    edge_midpoints.insert(key, idx);
                    idx
                }
            };

            let ab = get_midpoint(a, b);
            let bc = get_midpoint(b, c);
            let ca = get_midpoint(c, a);

            new_faces.push((a, ab, ca));
            new_faces.push((b, bc, ab));
            new_faces.push((c, ca, bc));
            new_faces.push((ab, bc, ca));
        }

        current_faces = new_faces;
    }

    // Apply random deformation to make it look like a rock
    let noise = Perlin::new(seed);
    for v in &mut current_verts {
        // Multi-octave noise for natural deformation
        let deform = noise.get([v.x as f64 * 2.0, v.y as f64 * 2.0, v.z as f64 * 2.0]) as f32 * 0.3
            + noise.get([v.x as f64 * 4.0, v.y as f64 * 4.0, v.z as f64 * 4.0]) as f32 * 0.15;

        // Random per-vertex displacement
        let random_offset = rng.gen_range(-0.1..0.1);

        // Apply deformation while maintaining roughly spherical shape
        let scale = 1.0 + deform + random_offset;
        *v = *v * scale;

        // Flatten bottom slightly for more stable-looking rocks
        if v.y < 0.0 {
            v.y *= 0.7;
        }
    }

    // Build final mesh with flat shading (duplicate vertices per face)
    for &(a, b, c) in &current_faces {
        let va = current_verts[a];
        let vb = current_verts[b];
        let vc = current_verts[c];

        // Calculate face normal
        let edge1 = vb - va;
        let edge2 = vc - va;
        let normal = edge1.cross(edge2).normalize();

        let base_idx = vertices.len() as u32;
        vertices.push([va.x, va.y, va.z]);
        vertices.push([vb.x, vb.y, vb.z]);
        vertices.push([vc.x, vc.y, vc.z]);

        normals.push([normal.x, normal.y, normal.z]);
        normals.push([normal.x, normal.y, normal.z]);
        normals.push([normal.x, normal.y, normal.z]);

        indices.push(base_idx);
        indices.push(base_idx + 1);
        indices.push(base_idx + 2);
    }

    // Add UVs (simple spherical mapping)
    let uvs: Vec<[f32; 2]> = vertices.iter().map(|v| {
        let theta = v[0].atan2(v[2]);
        let phi = v[1].asin();
        [(theta / PI + 1.0) / 2.0, phi / PI + 0.5]
    }).collect();

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    meshes.add(mesh)
}

// ============================================================================
// SPAWNING SYSTEMS
// ============================================================================

/// Tracks the last heightmap state to detect when terrain is ready
#[derive(Default)]
struct DecorSpawnState {
    spawned_for_map: Option<MapPreset>,
    last_center_height: f32,
}

/// System to spawn terrain decorations when map changes
/// Waits for heightmap to be updated before spawning rocks
fn spawn_terrain_decorations(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    config: Res<TerrainConfig>,
    heightmap: Res<TerrainHeightmap>,
    mut map_switch_events: EventReader<MapSwitchEvent>,
    rock_query: Query<Entity, With<Rock>>,
    sand_emitter_query: Query<Entity, With<SandEmitter>>,
    sand_particle_query: Query<Entity, With<SandParticle>>,
    mut state: Local<DecorSpawnState>,
) {
    // Consume map switch events (just to clear them, we use heightmap changes instead)
    for _event in map_switch_events.read() {}

    let map = config.current_map;

    // Skip flat maps
    if map == MapPreset::Flat {
        // Despawn decorations when switching to flat
        if state.spawned_for_map.is_some() {
            for entity in rock_query.iter() {
                commands.entity(entity).despawn();
            }
            for entity in sand_emitter_query.iter() {
                commands.entity(entity).despawn();
            }
            for entity in sand_particle_query.iter() {
                commands.entity(entity).despawn();
            }
            state.spawned_for_map = None;
            state.last_center_height = 0.0;
        }
        return;
    }

    // Check if heightmap has been updated by sampling center height
    let center_height = heightmap.sample_height(0.0, 0.0);

    // For FirebaseDelta, center should be ~49.8, for RollingHills it varies
    // We detect terrain is ready when center height is significantly different from flat (-1.0)
    let terrain_ready = center_height > 0.0;

    if !terrain_ready {
        return;
    }

    // Skip if we already spawned for this map with this heightmap
    if state.spawned_for_map == Some(map) && (center_height - state.last_center_height).abs() < 1.0 {
        return;
    }

    // Despawn existing decorations
    for entity in rock_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in sand_emitter_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in sand_particle_query.iter() {
        commands.entity(entity).despawn();
    }

    state.spawned_for_map = Some(map);
    state.last_center_height = center_height;

    info!("Spawning terrain decorations for {:?}", map);

    // Create rock material (desert sandstone color)
    let rock_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.45, 0.35),
        perceptual_roughness: 0.95,
        metallic: 0.0,
        ..default()
    });

    // Create a few rock mesh variants for variety
    let rock_meshes: Vec<Handle<Mesh>> = (0..5)
        .map(|i| create_rock_mesh(&mut meshes, 42 + i))
        .collect();

    // Spawn rocks using noise-based placement
    let noise = Perlin::new(42);
    let mut rng = StdRng::seed_from_u64(42);
    let half_size = config.terrain_size / 2.0;

    let mut rocks_spawned = 0;
    let max_attempts = ROCK_COUNT * 10;
    let mut attempts = 0;

    while rocks_spawned < ROCK_COUNT && attempts < max_attempts {
        attempts += 1;

        // Random position
        let x = rng.gen_range(-half_size + 20.0..half_size - 20.0);
        let z = rng.gen_range(-half_size + 20.0..half_size - 20.0);

        // Check noise value for clustering
        let noise_val = noise.get([x as f64 * ROCK_NOISE_SCALE, z as f64 * ROCK_NOISE_SCALE]);
        if noise_val < ROCK_NOISE_THRESHOLD {
            continue;
        }

        // Avoid center area (where buildings/action happens)
        let dist_from_center = (x * x + z * z).sqrt();
        if dist_from_center < 80.0 {
            continue;
        }

        let y = heightmap.sample_height(x, z);

        // Random scale and rotation
        let scale = rng.gen_range(ROCK_MIN_SCALE..ROCK_MAX_SCALE);
        let rotation = Quat::from_euler(
            EulerRot::YXZ,
            rng.gen_range(0.0..PI * 2.0),
            rng.gen_range(-0.2..0.2),
            rng.gen_range(-0.2..0.2),
        );

        // Pick random rock mesh
        let mesh_idx = rng.gen_range(0..rock_meshes.len());

        // Place rock above terrain - rock mesh is centered, so offset by half the scale
        // to have rock sit on terrain surface
        let rock_y_offset = scale * 0.3; // Rocks are slightly flattened on bottom
        commands.spawn((
            Mesh3d(rock_meshes[mesh_idx].clone()),
            MeshMaterial3d(rock_material.clone()),
            Transform::from_xyz(x, y + rock_y_offset, z)
                .with_rotation(rotation)
                .with_scale(Vec3::splat(scale)),
            Rock,
            TerrainDecoration, // Separate marker for decoration cleanup
            Name::new("Rock"),
        ));

        rocks_spawned += 1;
    }

    // Debug: sample center height to verify heightmap is loaded
    let center_height = heightmap.sample_height(0.0, 0.0);
    info!("Spawned {} rocks (terrain center height: {:.1})", rocks_spawned, center_height);

    // Sand emitters disabled - visual effect needs work
    // spawn_sand_emitters(&mut commands, &heightmap, config.terrain_size);
}

/// Spawn sand particle emitters across the terrain (disabled)
#[allow(dead_code)]
fn spawn_sand_emitters(
    commands: &mut Commands,
    heightmap: &TerrainHeightmap,
    terrain_size: f32,
) {
    let mut rng = StdRng::seed_from_u64(123);
    let half_size = terrain_size / 2.0;

    for _ in 0..SAND_EMITTER_COUNT {
        let x = rng.gen_range(-half_size + 10.0..half_size - 10.0);
        let z = rng.gen_range(-half_size + 10.0..half_size - 10.0);
        let y = heightmap.sample_height(x, z);

        commands.spawn((
            Transform::from_xyz(x, y, z),
            SandEmitter {
                spawn_timer: rng.gen_range(0.0..2.0),
                spawn_interval: rng.gen_range(0.3..0.8),
            },
            TerrainMarker,
            Name::new("SandEmitter"),
        ));
    }

    info!("Spawned {} sand emitters", SAND_EMITTER_COUNT);
}

// ============================================================================
// ANIMATION SYSTEMS
// ============================================================================

/// Animate sand particles and spawn new ones from emitters (disabled)
#[allow(dead_code)]
fn animate_sand_particles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    heightmap: Res<TerrainHeightmap>,
    config: Res<TerrainConfig>,
    mut emitter_query: Query<(&Transform, &mut SandEmitter)>,
    mut particle_query: Query<(Entity, &mut Transform, &mut SandParticle, &MeshMaterial3d<StandardMaterial>), Without<SandEmitter>>,
) {
    // Skip on flat map
    if config.current_map == MapPreset::Flat {
        return;
    }

    let dt = time.delta_secs();

    // Wind direction (prevailing wind from one direction with slight variation)
    let base_wind = Vec3::new(1.0, 0.0, 0.3).normalize();

    // Update existing particles
    for (entity, mut transform, mut particle, material_handle) in particle_query.iter_mut() {
        particle.age += dt;

        if particle.age >= particle.lifetime {
            commands.entity(entity).despawn();
            continue;
        }

        // Move particle
        transform.translation += particle.velocity * dt;

        // Keep particle above terrain
        let terrain_y = heightmap.sample_height(transform.translation.x, transform.translation.z);
        if transform.translation.y < terrain_y + SAND_HEIGHT_OFFSET {
            transform.translation.y = terrain_y + SAND_HEIGHT_OFFSET;
        }

        // Fade out particle
        let alpha = 1.0 - (particle.age / particle.lifetime);
        if let Some(mat) = materials.get_mut(&material_handle.0) {
            mat.base_color = Color::srgba(0.85, 0.75, 0.55, alpha * 0.4);
        }
    }

    // Spawn new particles from emitters
    let particle_mesh = meshes.add(Rectangle::new(0.3, 0.3));

    for (emitter_transform, mut emitter) in emitter_query.iter_mut() {
        emitter.spawn_timer -= dt;

        if emitter.spawn_timer <= 0.0 {
            emitter.spawn_timer = emitter.spawn_interval;

            // Spawn a burst of particles
            let mut rng = rand::thread_rng();
            let spawn_count = rng.gen_range(2..5);

            for _ in 0..spawn_count {
                let offset = Vec3::new(
                    rng.gen_range(-5.0..5.0),
                    rng.gen_range(0.0..2.0),
                    rng.gen_range(-5.0..5.0),
                );

                let start_pos = emitter_transform.translation + offset;
                let terrain_y = heightmap.sample_height(start_pos.x, start_pos.z);
                let spawn_pos = Vec3::new(start_pos.x, terrain_y + SAND_HEIGHT_OFFSET + offset.y, start_pos.z);

                // Velocity varies around wind direction
                let wind_variation = Vec3::new(
                    rng.gen_range(-0.3..0.3),
                    rng.gen_range(-0.1..0.2),
                    rng.gen_range(-0.3..0.3),
                );
                let velocity = (base_wind + wind_variation).normalize() * SAND_PARTICLE_SPEED * rng.gen_range(0.7..1.3);

                let sand_material = materials.add(StandardMaterial {
                    base_color: Color::srgba(0.85, 0.75, 0.55, 0.4),
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    ..default()
                });

                commands.spawn((
                    Mesh3d(particle_mesh.clone()),
                    MeshMaterial3d(sand_material),
                    Transform::from_translation(spawn_pos)
                        .with_rotation(Quat::from_rotation_x(-PI / 2.0))
                        .with_scale(Vec3::splat(rng.gen_range(0.5..1.5))),
                    SandParticle {
                        velocity,
                        lifetime: SAND_PARTICLE_LIFETIME * rng.gen_range(0.8..1.2),
                        age: 0.0,
                    },
                    TerrainMarker,
                    Name::new("SandParticle"),
                ));
            }
        }
    }
}

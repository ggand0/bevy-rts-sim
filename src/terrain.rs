// Procedural terrain generation module
// Provides height-mapped terrain with multiple map presets

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::core_pipeline::Skybox;
use noise::{NoiseFn, Perlin, Fbm, MultiFractal};
use std::f32::consts::PI;
use crate::constants::*;
use crate::types::*;

/// Marker component for skybox - used to remove skybox when switching maps
#[derive(Component)]
pub struct MapSkybox;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TerrainConfig::default())
            .add_event::<MapSwitchEvent>()
            .add_systems(Startup, spawn_initial_terrain)
            .add_systems(Update, (terrain_map_switching, handle_map_switch_units));
    }
}

/// Event sent when map is switched
#[derive(Event)]
pub struct MapSwitchEvent {
    pub new_map: MapPreset,
}

/// Map preset types
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MapPreset {
    #[default]
    Flat,
    RollingHills,
}

/// Terrain configuration resource
#[derive(Resource)]
pub struct TerrainConfig {
    pub current_map: MapPreset,
    pub grid_size: usize,
    pub terrain_size: f32,
    pub max_height: f32,
    pub seed: u32,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            current_map: MapPreset::Flat,
            grid_size: TERRAIN_GRID_SIZE,
            terrain_size: TERRAIN_SIZE,
            max_height: TERRAIN_MAX_HEIGHT,
            seed: 42,
        }
    }
}

/// Marker component for terrain entity (both flat ground and procedural terrain)
#[derive(Component)]
pub struct TerrainMarker;

/// Marker for the original flat ground (Map 1)
#[derive(Component)]
pub struct FlatGroundMarker;

/// Resource storing the heightmap data for raycasting
#[derive(Resource)]
pub struct TerrainHeightmap {
    pub heights: Vec<Vec<f32>>,
    pub grid_size: usize,
    pub terrain_size: f32,
    pub cell_size: f32,
    pub base_height: f32, // Y offset for the terrain (e.g., -1.0 for flat ground)
}

impl TerrainHeightmap {
    /// Sample height at world position (x, z)
    /// Returns the interpolated height at that position
    pub fn sample_height(&self, x: f32, z: f32) -> f32 {
        // Convert world coordinates to grid coordinates
        let half_size = self.terrain_size / 2.0;
        let grid_x = ((x + half_size) / self.cell_size).clamp(0.0, (self.grid_size - 1) as f32);
        let grid_z = ((z + half_size) / self.cell_size).clamp(0.0, (self.grid_size - 1) as f32);

        // Get integer grid indices
        let x0 = grid_x.floor() as usize;
        let z0 = grid_z.floor() as usize;
        let x1 = (x0 + 1).min(self.grid_size - 1);
        let z1 = (z0 + 1).min(self.grid_size - 1);

        // Get fractional parts for interpolation
        let fx = grid_x.fract();
        let fz = grid_z.fract();

        // Bilinear interpolation
        let h00 = self.heights[z0][x0];
        let h10 = self.heights[z0][x1];
        let h01 = self.heights[z1][x0];
        let h11 = self.heights[z1][x1];

        let h0 = h00 * (1.0 - fx) + h10 * fx;
        let h1 = h01 * (1.0 - fx) + h11 * fx;

        self.base_height + h0 * (1.0 - fz) + h1 * fz
    }

    /// Create a flat heightmap at a specific base height
    pub fn flat(terrain_size: f32, base_height: f32) -> Self {
        let grid_size = 2; // Minimal grid for flat terrain
        Self {
            heights: vec![vec![0.0; grid_size]; grid_size],
            grid_size,
            terrain_size,
            cell_size: terrain_size,
            base_height,
        }
    }
}

/// Generate heightmap using Perlin noise with fractal Brownian motion
/// Creates distinct hills rather than bumpy terrain
fn generate_heightmap(config: &TerrainConfig) -> Vec<Vec<f32>> {
    let grid_size = config.grid_size;
    let mut heights = vec![vec![0.0f32; grid_size]; grid_size];

    if config.current_map == MapPreset::Flat {
        // Flat map - all heights are 0
        return heights;
    }

    // Create fractal Brownian motion noise with lower frequency for larger hills
    let fbm: Fbm<Perlin> = Fbm::new(config.seed)
        .set_octaves(2) // Fewer octaves = smoother, less bumpy
        .set_persistence(0.3) // Lower persistence = smoother transitions
        .set_lacunarity(2.0);

    let half_size = config.terrain_size / 2.0;
    let cell_size = config.terrain_size / (grid_size - 1) as f32;

    // Use much lower scale for larger, fewer hills
    let noise_scale = 0.008; // Lower = larger features

    for z in 0..grid_size {
        for x in 0..grid_size {
            // Convert grid coordinates to world coordinates
            let world_x = (x as f32 * cell_size) - half_size;
            let world_z = (z as f32 * cell_size) - half_size;

            // Sample noise (returns -1 to 1)
            let noise_val = fbm.get([
                world_x as f64 * noise_scale,
                world_z as f64 * noise_scale,
            ]);

            // Apply threshold to create distinct hills (values below threshold become flat)
            let threshold = 0.1;
            let adjusted_noise = if noise_val > threshold {
                // Smooth ramp up from threshold
                ((noise_val - threshold) / (1.0 - threshold)).powf(1.5) as f32
            } else {
                0.0
            };

            // Scale to max height (reduced for gentler hills)
            let height = adjusted_noise * config.max_height * 0.6;

            // Apply stronger edge falloff to keep edges flat
            let edge_dist_x = (half_size - world_x.abs()) / half_size;
            let edge_dist_z = (half_size - world_z.abs()) / half_size;
            let edge_factor = (edge_dist_x.min(edge_dist_z)).powf(2.0).min(1.0);

            heights[z][x] = height * edge_factor;
        }
    }

    heights
}

/// Build terrain mesh from heightmap
fn build_terrain_mesh(heights: &[Vec<f32>], config: &TerrainConfig) -> Mesh {
    let grid_size = config.grid_size;
    let half_size = config.terrain_size / 2.0;
    let cell_size = config.terrain_size / (grid_size - 1) as f32;

    let vertex_count = grid_size * grid_size;
    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = Vec::with_capacity(vertex_count);
    let mut uvs = Vec::with_capacity(vertex_count);

    // Generate vertices
    for z in 0..grid_size {
        for x in 0..grid_size {
            let world_x = (x as f32 * cell_size) - half_size;
            let world_z = (z as f32 * cell_size) - half_size;
            let height = heights[z][x];

            positions.push([world_x, height, world_z]);
            // Tile UVs for texture repetition
            uvs.push([x as f32 * 0.5, z as f32 * 0.5]);
        }
    }

    // Calculate normals using central differences
    for z in 0..grid_size {
        for x in 0..grid_size {
            let left = if x > 0 { heights[z][x - 1] } else { heights[z][x] };
            let right = if x < grid_size - 1 { heights[z][x + 1] } else { heights[z][x] };
            let down = if z > 0 { heights[z - 1][x] } else { heights[z][x] };
            let up = if z < grid_size - 1 { heights[z + 1][x] } else { heights[z][x] };

            // Normal from height differences
            let normal = Vec3::new(
                (left - right) / (2.0 * cell_size),
                1.0,
                (down - up) / (2.0 * cell_size),
            ).normalize();

            normals.push([normal.x, normal.y, normal.z]);
        }
    }

    // Generate indices for triangle strips
    let quad_count = (grid_size - 1) * (grid_size - 1);
    let mut indices = Vec::with_capacity(quad_count * 6);

    for z in 0..(grid_size - 1) {
        for x in 0..(grid_size - 1) {
            let top_left = (z * grid_size + x) as u32;
            let top_right = top_left + 1;
            let bottom_left = ((z + 1) * grid_size + x) as u32;
            let bottom_right = bottom_left + 1;

            // First triangle
            indices.push(top_left);
            indices.push(bottom_left);
            indices.push(top_right);

            // Second triangle
            indices.push(top_right);
            indices.push(bottom_left);
            indices.push(bottom_right);
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, bevy::render::render_asset::RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}

/// Create checkerboard ground texture (original Map 1 style)
fn create_ground_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let mut image = Image::new_fill(
        Extent3d {
            width: 32,
            height: 32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[100, 50, 30, 255],
        TextureFormat::Rgba8UnormSrgb,
        bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
    );

    // Create checkerboard pattern
    for y in 0..32 {
        for x in 0..32 {
            let index = (y * 32 + x) * 4;
            if (x + y) % 2 == 0 {
                image.data[index] = 120;     // R
                image.data[index + 1] = 80;  // G
                image.data[index + 2] = 40;  // B
                image.data[index + 3] = 255; // A
            } else {
                image.data[index] = 80;      // R
                image.data[index + 1] = 60;  // G
                image.data[index + 2] = 30;  // B
                image.data[index + 3] = 255; // A
            }
        }
    }

    images.add(image)
}

/// Spawn initial terrain (flat ground for Map 1)
pub fn spawn_initial_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    config: Res<TerrainConfig>,
) {
    info!("Spawning initial terrain: {:?}", config.current_map);

    // Create flat heightmap for Map 1
    // Ground is at Y=-1.0 in the original setup
    commands.insert_resource(TerrainHeightmap::flat(TERRAIN_SIZE, -1.0));

    // Create textured ground (original style)
    let ground_texture = create_ground_texture(&mut images);

    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(800.0, 800.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(ground_texture),
            perceptual_roughness: 0.8,
            metallic: 0.0,
            ..default()
        })),
        Transform::from_xyz(0.0, -1.0, 0.0)
            .with_rotation(Quat::from_rotation_x(-PI / 2.0)),
        TerrainMarker,
        FlatGroundMarker,
        Name::new("FlatGround"),
    ));

    info!("Flat ground spawned at Y=-1.0");
}

/// System to switch between map presets using 1-2 keys
fn terrain_map_switching(
    keys: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<TerrainConfig>,
    terrain_query: Query<Entity, With<TerrainMarker>>,
    skybox_entity_query: Query<Entity, With<MapSkybox>>,
    camera_query: Query<Entity, With<crate::types::RtsCamera>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    mut map_switch_events: EventWriter<MapSwitchEvent>,
) {
    let new_preset = if keys.just_pressed(KeyCode::Digit1) {
        Some(MapPreset::Flat)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(MapPreset::RollingHills)
    } else {
        None
    };

    if let Some(preset) = new_preset {
        if config.current_map != preset {
            info!("Switching terrain to: {:?}", preset);
            config.current_map = preset;

            // Despawn all terrain entities
            for entity in terrain_query.iter() {
                commands.entity(entity).despawn_recursive();
            }

            // Remove skybox from camera if present
            for entity in skybox_entity_query.iter() {
                commands.entity(entity).despawn_recursive();
            }
            // Also remove Skybox component from camera
            if let Ok(camera_entity) = camera_query.get_single() {
                commands.entity(camera_entity).remove::<Skybox>();
            }

            match preset {
                MapPreset::Flat => {
                    // Restore original flat ground (no skybox for Map 1)
                    commands.insert_resource(TerrainHeightmap::flat(TERRAIN_SIZE, -1.0));

                    let ground_texture = create_ground_texture(&mut images);

                    commands.spawn((
                        Mesh3d(meshes.add(Rectangle::new(800.0, 800.0))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color_texture: Some(ground_texture),
                            perceptual_roughness: 0.8,
                            metallic: 0.0,
                            ..default()
                        })),
                        Transform::from_xyz(0.0, -1.0, 0.0)
                            .with_rotation(Quat::from_rotation_x(-PI / 2.0)),
                        TerrainMarker,
                        FlatGroundMarker,
                        Name::new("FlatGround"),
                    ));

                    info!("Switched to flat ground at Y=-1.0");
                }
                MapPreset::RollingHills => {
                    // Generate procedural terrain
                    let heights = generate_heightmap(&config);
                    let mesh = build_terrain_mesh(&heights, &config);

                    // Update heightmap resource (base_height = 0 for procedural terrain)
                    let cell_size = config.terrain_size / (config.grid_size - 1) as f32;
                    commands.insert_resource(TerrainHeightmap {
                        heights: heights.clone(),
                        grid_size: config.grid_size,
                        terrain_size: config.terrain_size,
                        cell_size,
                        base_height: 0.0,
                    });

                    // Create terrain material (Mars-like dark reddish-brown)
                    let terrain_material = materials.add(StandardMaterial {
                        base_color: Color::srgb(0.35, 0.18, 0.12),
                        perceptual_roughness: 0.95,
                        metallic: 0.0,
                        ..default()
                    });

                    // Spawn terrain mesh
                    commands.spawn((
                        Mesh3d(meshes.add(mesh)),
                        MeshMaterial3d(terrain_material),
                        Transform::default(),
                        TerrainMarker,
                        Name::new("ProceduralTerrain"),
                    ));

                    // Add skybox to camera for Map 2
                    let skybox_handle: Handle<Image> = asset_server.load("skybox/qwantani_mid_morning_puresky_2k/skybox.ktx2");
                    if let Ok(camera_entity) = camera_query.get_single() {
                        commands.entity(camera_entity).insert(Skybox {
                            image: skybox_handle.clone(),
                            brightness: 1000.0,
                            rotation: Quat::IDENTITY,
                        });
                    }

                    info!("Switched to rolling hills terrain with skybox");
                }
            }

            // Send event to reposition units
            map_switch_events.send(MapSwitchEvent { new_map: preset });
        }
    }
}

/// Unit Y offset above terrain (mesh feet are at Y=-1.6, scaled by 0.8 = -1.28)
/// For flat ground at Y=-1.0, this gives spawn at Y=0.28
/// For procedural terrain at Y=0+, this gives spawn at terrain_y + 1.28
const UNIT_TERRAIN_OFFSET: f32 = 1.28;

/// System to reposition units, towers, and reset game state when map is switched
fn handle_map_switch_units(
    mut map_switch_events: EventReader<MapSwitchEvent>,
    heightmap: Res<TerrainHeightmap>,
    mut droid_query: Query<(&mut Transform, &mut BattleDroid, &SquadMember)>,
    mut tower_query: Query<(&mut Transform, &mut Health), (With<UplinkTower>, Without<BattleDroid>)>,
    mut squad_manager: ResMut<SquadManager>,
    mut game_state: ResMut<GameState>,
) {
    for event in map_switch_events.read() {
        info!("Repositioning units for map: {:?}", event.new_map);

        // Reset game state
        game_state.team_a_tower_destroyed = false;
        game_state.team_b_tower_destroyed = false;
        game_state.game_ended = false;
        game_state.winner = None;
        info!("Game state reset");

        // Reposition all units to terrain height
        for (mut transform, mut droid, _squad_member) in droid_query.iter_mut() {
            let x = transform.translation.x;
            let z = transform.translation.z;
            let terrain_y = heightmap.sample_height(x, z);

            // Update unit position with proper offset for feet placement
            let new_y = terrain_y + UNIT_TERRAIN_OFFSET;
            transform.translation.y = new_y;

            // Update spawn position so retreat works correctly
            droid.spawn_position.y = new_y;
            droid.target_position.y = new_y;
        }

        // Update squad center positions
        for (_squad_id, squad) in squad_manager.squads.iter_mut() {
            let terrain_y = heightmap.sample_height(squad.center_position.x, squad.center_position.z);
            squad.center_position.y = terrain_y + UNIT_TERRAIN_OFFSET;
            squad.target_position.y = terrain_y + UNIT_TERRAIN_OFFSET;
        }

        // Reposition towers and reset health
        for (mut transform, mut health) in tower_query.iter_mut() {
            let x = transform.translation.x;
            let z = transform.translation.z;
            let terrain_y = heightmap.sample_height(x, z);
            transform.translation.y = terrain_y;

            // Reset tower health
            health.current = health.max;
        }

        info!("Units and towers repositioned, game state reset");
    }
}

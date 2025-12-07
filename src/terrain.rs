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
use crate::scenario::CommandBunker;
use crate::shield::Shield;

/// Marker component for skybox - used to remove skybox when switching maps
#[derive(Component)]
pub struct MapSkybox;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TerrainConfig::default())
            .add_event::<MapSwitchEvent>()
            .add_systems(Startup, spawn_initial_terrain)
            .add_systems(Update, (
                terrain_map_switching,
                handle_pending_heightmap,
                handle_map_switch_units,
            ));
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
    FirebaseDelta,
}

/// Terrain configuration resource
#[derive(Resource)]
pub struct TerrainConfig {
    pub current_map: MapPreset,
    pub grid_size: usize,
    pub terrain_size: f32,
    pub max_height: f32,
    pub seed: u32,
    /// Handle to PNG heightmap being loaded (for async loading)
    pub pending_heightmap: Option<Handle<Image>>,
    /// Target map preset to switch to once heightmap loads
    pub pending_map: Option<MapPreset>,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            current_map: MapPreset::Flat,
            grid_size: TERRAIN_GRID_SIZE,
            terrain_size: TERRAIN_SIZE,
            max_height: TERRAIN_MAX_HEIGHT,
            seed: 42,
            pending_heightmap: None,
            pending_map: None,
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

    /// Sample terrain normal at world position (x, z) using central differences
    /// Returns normalized normal vector pointing up from terrain surface
    #[allow(dead_code)]
    pub fn sample_normal(&self, x: f32, z: f32) -> Vec3 {
        let offset = self.cell_size;

        // Sample heights at neighboring points
        let h_left = self.sample_height(x - offset, z);
        let h_right = self.sample_height(x + offset, z);
        let h_down = self.sample_height(x, z - offset);
        let h_up = self.sample_height(x, z + offset);

        // Calculate tangent vectors
        let tangent_x = Vec3::new(2.0 * offset, h_right - h_left, 0.0);
        let tangent_z = Vec3::new(0.0, h_up - h_down, 2.0 * offset);

        // Cross product gives normal (normalized)
        tangent_x.cross(tangent_z).normalize()
    }

    /// Sample both height and normal at once (more efficient than separate calls)
    #[allow(dead_code)]
    pub fn sample_height_and_normal(&self, x: f32, z: f32) -> (f32, Vec3) {
        let height = self.sample_height(x, z);
        let normal = self.sample_normal(x, z);
        (height, normal)
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

/// Load heightmap from PNG image
/// Converts grayscale pixel values (0-255) to height values (0-max_height)
fn load_heightmap_from_png(image: &Image, max_height: f32, target_grid_size: usize) -> Vec<Vec<f32>> {
    let width = image.width() as usize;
    let height = image.height() as usize;

    // Determine bytes per pixel based on format
    let bytes_per_pixel = match image.texture_descriptor.format {
        TextureFormat::R8Unorm => 1,
        TextureFormat::Rg8Unorm => 2,
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => 4,
        TextureFormat::R16Unorm => 2,
        TextureFormat::Rgba16Unorm => 8,
        _ => {
            warn!("Unsupported texture format {:?}, assuming RGBA8", image.texture_descriptor.format);
            4
        }
    };

    let data = match &image.data {
        Some(d) => d,
        None => {
            warn!("PNG heightmap has no data, returning flat heightmap");
            return vec![vec![0.0; target_grid_size]; target_grid_size];
        }
    };

    // First read the raw heights from the PNG at its native resolution
    let mut raw_heights = vec![vec![0.0f32; width]; height];

    for y in 0..height {
        for x in 0..width {
            let pixel_index = (y * width + x) * bytes_per_pixel;
            // Use first channel (R) as grayscale value
            let grayscale = if pixel_index < data.len() {
                data[pixel_index]
            } else {
                0
            };
            raw_heights[y][x] = (grayscale as f32 / 255.0) * max_height;
        }
    }

    // If the PNG matches target size, use directly
    if width == target_grid_size && height == target_grid_size {
        return raw_heights;
    }

    // Otherwise, resample to target grid size using bilinear interpolation
    let mut heights = vec![vec![0.0f32; target_grid_size]; target_grid_size];
    let scale_x = (width - 1) as f32 / (target_grid_size - 1) as f32;
    let scale_y = (height - 1) as f32 / (target_grid_size - 1) as f32;

    for gy in 0..target_grid_size {
        for gx in 0..target_grid_size {
            let src_x = gx as f32 * scale_x;
            let src_y = gy as f32 * scale_y;

            let x0 = src_x.floor() as usize;
            let y0 = src_y.floor() as usize;
            let x1 = (x0 + 1).min(width - 1);
            let y1 = (y0 + 1).min(height - 1);

            let fx = src_x.fract();
            let fy = src_y.fract();

            // Bilinear interpolation
            let h00 = raw_heights[y0][x0];
            let h10 = raw_heights[y0][x1];
            let h01 = raw_heights[y1][x0];
            let h11 = raw_heights[y1][x1];

            let h0 = h00 * (1.0 - fx) + h10 * fx;
            let h1 = h01 * (1.0 - fx) + h11 * fx;

            heights[gy][gx] = h0 * (1.0 - fy) + h1 * fy;
        }
    }

    info!("Loaded PNG heightmap {}x{} -> {}x{} grid", width, height, target_grid_size, target_grid_size);
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

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
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
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );

    // Create checkerboard pattern
    if let Some(data) = &mut image.data {
        for y in 0..32 {
            for x in 0..32 {
                let index = (y * 32 + x) * 4;
                if (x + y) % 2 == 0 {
                    data[index] = 120;     // R
                    data[index + 1] = 80;  // G
                    data[index + 2] = 40;  // B
                    data[index + 3] = 255; // A
                } else {
                    data[index] = 80;      // R
                    data[index + 1] = 60;  // G
                    data[index + 2] = 30;  // B
                    data[index + 3] = 255; // A
                }
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

/// System to switch between map presets using 1-3 keys
/// Skips map switching when explosion debug mode is active (0 -> 1/2/3)
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
    debug_mode: Res<crate::objective::ExplosionDebugMode>,
) {
    // Skip map switching when debug mode is active (digit keys used for debug spawns)
    if debug_mode.explosion_mode {
        return;
    }

    let new_preset = if keys.just_pressed(KeyCode::Digit1) {
        Some(MapPreset::Flat)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(MapPreset::RollingHills)
    } else if keys.just_pressed(KeyCode::Digit3) {
        Some(MapPreset::FirebaseDelta)
    } else {
        None
    };

    if let Some(preset) = new_preset {
        if config.current_map != preset {
            info!("Switching terrain to: {:?}", preset);
            config.current_map = preset;

            // Despawn all terrain entities
            for entity in terrain_query.iter() {
                commands.entity(entity).despawn();
            }

            // Remove skybox from camera if present
            for entity in skybox_entity_query.iter() {
                commands.entity(entity).despawn();
            }
            // Also remove Skybox component from camera
            if let Ok(camera_entity) = camera_query.single() {
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
                    if let Ok(camera_entity) = camera_query.single() {
                        commands.entity(camera_entity).insert(Skybox {
                            image: skybox_handle.clone(),
                            brightness: 1000.0,
                            rotation: Quat::IDENTITY,
                        });
                    }

                    info!("Switched to rolling hills terrain with skybox");
                }
                MapPreset::FirebaseDelta => {
                    // Start async loading of PNG heightmap
                    let heightmap_handle: Handle<Image> = asset_server.load("heightmap/rts_heightmap0.png");
                    //let heightmap_handle: Handle<Image> = asset_server.load("heightmap/wgen_x0_y0.png");
                    config.pending_heightmap = Some(heightmap_handle);
                    config.pending_map = Some(MapPreset::FirebaseDelta);
                    info!("Loading Firebase Delta heightmap...");
                    // Don't send map switch event yet - wait for async load
                    return;
                }
            }

            // Send event to reposition units
            map_switch_events.write(MapSwitchEvent { new_map: preset });
        }
    }
}

/// System to handle async PNG heightmap loading and build terrain once loaded
fn handle_pending_heightmap(
    mut config: ResMut<TerrainConfig>,
    images: Res<Assets<Image>>,
    terrain_query: Query<Entity, With<TerrainMarker>>,
    skybox_entity_query: Query<Entity, With<MapSkybox>>,
    camera_query: Query<Entity, With<crate::types::RtsCamera>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    mut map_switch_events: EventWriter<MapSwitchEvent>,
    mut heightmap: ResMut<TerrainHeightmap>,
) {
    // Check if we have a pending heightmap to load
    let Some(handle) = config.pending_heightmap.clone() else {
        return;
    };

    // Check if the image is loaded
    let Some(image) = images.get(&handle) else {
        return;
    };

    // Image is loaded - build the terrain
    let pending_map = config.pending_map.take().unwrap_or(MapPreset::FirebaseDelta);
    config.pending_heightmap = None;
    config.current_map = pending_map;

    info!("PNG heightmap loaded, building Firebase Delta terrain...");

    // Despawn old terrain
    for entity in terrain_query.iter() {
        commands.entity(entity).despawn();
    }

    // Remove skybox
    for entity in skybox_entity_query.iter() {
        commands.entity(entity).despawn();
    }
    if let Ok(camera_entity) = camera_query.single() {
        commands.entity(camera_entity).remove::<Skybox>();
    }

    // Load heightmap from PNG
    let heights = load_heightmap_from_png(image, config.max_height, config.grid_size);
    let mesh = build_terrain_mesh(&heights, &config);

    // Update heightmap resource directly (not via commands, so it's available this frame)
    let cell_size = config.terrain_size / (config.grid_size - 1) as f32;
    heightmap.heights = heights.clone();
    heightmap.grid_size = config.grid_size;
    heightmap.terrain_size = config.terrain_size;
    heightmap.cell_size = cell_size;
    heightmap.base_height = 0.0;

    // Create terrain material (military base dusty brown/tan)
    let terrain_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.38, 0.28),
        perceptual_roughness: 0.9,
        metallic: 0.0,
        ..default()
    });

    // Spawn terrain mesh
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(terrain_material),
        Transform::default(),
        TerrainMarker,
        Name::new("FirebaseDeltaTerrain"),
    ));

    // Add skybox for Firebase Delta (reuse the same skybox)
    let skybox_handle: Handle<Image> = asset_server.load("skybox/qwantani_mid_morning_puresky_2k/skybox.ktx2");
    if let Ok(camera_entity) = camera_query.single() {
        commands.entity(camera_entity).insert(Skybox {
            image: skybox_handle.clone(),
            brightness: 1000.0,
            rotation: Quat::IDENTITY,
        });
    }

    info!("Firebase Delta terrain built successfully");

    // Send event to reposition units
    map_switch_events.write(MapSwitchEvent { new_map: pending_map });
}

/// Unit Y offset above terrain (mesh feet are at Y=-1.6, scaled by 0.8 = -1.28)
/// For flat ground at Y=-1.0, this gives spawn at Y=0.28
/// For procedural terrain at Y=0+, this gives spawn at terrain_y + 1.28
const UNIT_TERRAIN_OFFSET: f32 = 1.28;

/// System to reposition units, towers, and reset game state when map is switched
pub fn handle_map_switch_units(
    mut commands: Commands,
    mut map_switch_events: EventReader<MapSwitchEvent>,
    heightmap: Res<TerrainHeightmap>,
    mut droid_query: Query<(Entity, &mut Transform, &mut BattleDroid, &SquadMember)>,
    tower_query: Query<(Entity, &UplinkTower), Without<CommandBunker>>,
    shield_query: Query<Entity, With<Shield>>,
    mut tower_mut_query: Query<(&mut Transform, &mut Health), (With<UplinkTower>, Without<BattleDroid>)>,
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

        // For FirebaseDelta, despawn all default units and towers instead of repositioning
        if event.new_map == MapPreset::FirebaseDelta {
            // Collect entities to despawn first (can't despawn while iterating with mutable query)
            let droid_entities: Vec<Entity> = droid_query.iter().map(|(e, _, _, _)| e).collect();
            let despawned_units = droid_entities.len();
            for entity in droid_entities {
                commands.entity(entity).despawn();
            }

            // Clear squad manager
            squad_manager.squads.clear();
            squad_manager.next_squad_id = 0;

            // Despawn all non-CommandBunker towers (scenario spawns its own)
            let mut despawned_towers = 0;
            for (entity, _tower) in tower_query.iter() {
                commands.entity(entity).despawn();
                despawned_towers += 1;
            }

            // Despawn all shields (scenario can spawn its own if needed)
            let mut despawned_shields = 0;
            for entity in shield_query.iter() {
                commands.entity(entity).despawn();
                despawned_shields += 1;
            }

            info!("FirebaseDelta: Despawned {} units, {} towers, {} shields", despawned_units, despawned_towers, despawned_shields);
            continue;
        }

        // For other maps, reposition all units to terrain height
        for (_entity, mut transform, mut droid, _squad_member) in droid_query.iter_mut() {
            let x = transform.translation.x;
            let z = transform.translation.z;
            let terrain_y = heightmap.sample_height(x, z);

            // Update unit position with proper offset for feet placement
            let new_y = terrain_y + UNIT_TERRAIN_OFFSET;
            transform.translation.y = new_y;

            // Update spawn position so retreat works correctly
            droid.spawn_position.y = new_y;
            droid.target_position.y = new_y;

            // Reset march animation to prevent units from appearing buried/floating
            // The animate_march system will recalculate the bob based on the new terrain height
            droid.march_speed = 1.0;
        }

        // Update squad center positions
        for (_squad_id, squad) in squad_manager.squads.iter_mut() {
            let terrain_y = heightmap.sample_height(squad.center_position.x, squad.center_position.z);
            squad.center_position.y = terrain_y + UNIT_TERRAIN_OFFSET;
            squad.target_position.y = terrain_y + UNIT_TERRAIN_OFFSET;
        }

        // Reposition towers and reset health
        for (mut transform, mut health) in tower_mut_query.iter_mut() {
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

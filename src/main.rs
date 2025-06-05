use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel, MouseMotion};
use rand::Rng;
use std::f32::consts::PI;

const ARMY_SIZE_PER_TEAM: usize = 5_000;
const FORMATION_WIDTH: f32 = 200.0;
const UNIT_SPACING: f32 = 2.0;
const MARCH_DISTANCE: f32 = 150.0;
const MARCH_SPEED: f32 = 3.0;
const BATTLEFIELD_SIZE: f32 = 400.0;

// RTS Camera settings
const CAMERA_SPEED: f32 = 50.0;
const CAMERA_ZOOM_SPEED: f32 = 10.0;
const CAMERA_MIN_HEIGHT: f32 = 20.0;
const CAMERA_MAX_HEIGHT: f32 = 200.0;
const CAMERA_ROTATION_SPEED: f32 = 0.005;

// Laser projectile settings
const LASER_SPEED: f32 = 100.0;
const LASER_LIFETIME: f32 = 3.0;
const LASER_LENGTH: f32 = 3.0;
const LASER_WIDTH: f32 = 0.2;

// Combat settings
const TARGETING_RANGE: f32 = 150.0;
const TARGET_SCAN_INTERVAL: f32 = 2.0;
const COLLISION_RADIUS: f32 = 1.0;
const AUTO_FIRE_INTERVAL: f32 = 2.0;

// Spatial partitioning settings
const GRID_CELL_SIZE: f32 = 10.0; // Size of each grid cell
const GRID_SIZE: i32 = 100; // Number of cells per side (covers 1000x1000 area)

#[derive(Component, Clone, Copy, PartialEq)]
enum Team {
    A,
    B,
}

#[derive(Component)]
struct BattleDroid {
    march_speed: f32,
    spawn_position: Vec3,
    target_position: Vec3,
    march_offset: f32,
    returning_to_spawn: bool,
    team: Team,
}

#[derive(Component)]
struct FormationUnit {
    formation_index: usize,
    row: usize,
    column: usize,
}

#[derive(Component)]
struct RtsCamera {
    focus_point: Vec3,
    yaw: f32,
    pitch: f32,
    distance: f32,
}

#[derive(Component)]
struct LaserProjectile {
    velocity: Vec3,
    lifetime: f32,
    team: Team, // Track which team fired this laser
}

#[derive(Component)]
struct CombatUnit {
    target_scan_timer: f32,
    auto_fire_timer: f32,
    current_target: Option<Entity>,
}

// Audio resources
#[derive(Resource)]
struct AudioAssets {
    laser_sounds: Vec<Handle<AudioSource>>,
}

impl AudioAssets {
    fn get_random_laser_sound(&self, rng: &mut rand::rngs::ThreadRng) -> Handle<AudioSource> {
        let index = rng.gen_range(0..self.laser_sounds.len());
        self.laser_sounds[index].clone()
    }
}

// Spatial grid for collision optimization
#[derive(Resource, Default)]
struct SpatialGrid {
    // Grid cells containing entity IDs - [x][y]
    laser_cells: Vec<Vec<Vec<Entity>>>,
    droid_cells: Vec<Vec<Vec<Entity>>>,
}

impl SpatialGrid {
    fn new() -> Self {
        let size = GRID_SIZE as usize;
        Self {
            laser_cells: vec![vec![Vec::new(); size]; size],
            droid_cells: vec![vec![Vec::new(); size]; size],
        }
    }
    
    fn clear(&mut self) {
        for row in &mut self.laser_cells {
            for cell in row {
                cell.clear();
            }
        }
        for row in &mut self.droid_cells {
            for cell in row {
                cell.clear();
            }
        }
    }
    
    fn world_to_grid(pos: Vec3) -> (i32, i32) {
        let x = ((pos.x + GRID_SIZE as f32 * GRID_CELL_SIZE * 0.5) / GRID_CELL_SIZE) as i32;
        let z = ((pos.z + GRID_SIZE as f32 * GRID_CELL_SIZE * 0.5) / GRID_CELL_SIZE) as i32;
        (x.clamp(0, GRID_SIZE - 1), z.clamp(0, GRID_SIZE - 1))
    }
    
    fn add_laser(&mut self, entity: Entity, pos: Vec3) {
        let (x, z) = Self::world_to_grid(pos);
        self.laser_cells[x as usize][z as usize].push(entity);
    }
    
    fn add_droid(&mut self, entity: Entity, pos: Vec3) {
        let (x, z) = Self::world_to_grid(pos);
        self.droid_cells[x as usize][z as usize].push(entity);
    }
    
    fn get_nearby_droids(&self, pos: Vec3) -> Vec<Entity> {
        let (center_x, center_z) = Self::world_to_grid(pos);
        let mut nearby = Vec::new();
        
        // Check 3x3 grid around the position to account for collision radius
        for dx in -1..=1 {
            for dz in -1..=1 {
                let x = center_x + dx;
                let z = center_z + dz;
                if x >= 0 && x < GRID_SIZE && z >= 0 && z < GRID_SIZE {
                    nearby.extend(&self.droid_cells[x as usize][z as usize]);
                }
            }
        }
        nearby
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin)
        .insert_resource(SpatialGrid::new())
        .add_systems(Startup, (setup_scene, spawn_army))
        .add_systems(Update, (
            animate_march,
            update_camera_info,
            rts_camera_movement,
            target_acquisition_system,
            auto_fire_system,
            volley_fire_system,
            update_projectiles,
            collision_detection_system,
        ))
        .run();
}

// Helper function to calculate proper laser orientation
fn calculate_laser_orientation(
    velocity: Vec3,
    position: Vec3,
    camera_position: Vec3,
) -> Quat {
    if velocity.length() > 0.0 {
        let velocity_dir = velocity.normalize();
        let to_camera = (camera_position - position).normalize();
        
        // Choose a stable up vector for billboarding that's not parallel to to_camera
        let up = if to_camera.dot(Vec3::Y).abs() > 0.95 {
            Vec3::X // fallback when camera is nearly vertical
        } else {
            Vec3::Y // normal case
        };
        
        // First, make the quad face the camera using stable up vector
        let base_rotation = Transform::from_translation(Vec3::ZERO)
            .looking_at(-to_camera, up)
            .rotation;
        
        // Calculate the billboard's actual "up" direction after rotation
        let billboard_up = base_rotation * Vec3::Y;
        
        // Project velocity onto the billboard plane
        let velocity_in_quad_plane = velocity_dir - velocity_dir.dot(to_camera) * to_camera;
        if velocity_in_quad_plane.length() > 0.001 {
            let velocity_in_quad_plane = velocity_in_quad_plane.normalize();
            
            // Use billboard's actual up direction instead of fixed Vec3::Y
            let angle = billboard_up.dot(velocity_in_quad_plane).acos();
            let cross = billboard_up.cross(velocity_in_quad_plane);
            let rotation_sign = if cross.dot(to_camera) > 0.0 { 1.0 } else { -1.0 };
            
            let alignment_rotation = Quat::from_axis_angle(to_camera, angle * rotation_sign);
            alignment_rotation * base_rotation
        } else {
            base_rotation
        }
    } else {
        Quat::IDENTITY
    }
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    // Create a simple checkerboard texture for the ground
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
    let ground_texture = images.add(image);

    // Ground plane (expanded for marching distance)
    commands.spawn(PbrBundle {
        mesh: meshes.add(Rectangle::new(800.0, 800.0)),
        material: materials.add(StandardMaterial {
            base_color_texture: Some(ground_texture),
            perceptual_roughness: 0.8,
            metallic: 0.0,
            ..default()
        }),
        transform: Transform::from_xyz(0.0, -1.0, 0.0)
            .with_rotation(Quat::from_rotation_x(-PI / 2.0)),
        ..default()
    });

    // Directional light (sun)
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform {
            translation: Vec3::new(0.0, 50.0, 0.0),
            rotation: Quat::from_rotation_x(-PI / 4.0),
            ..default()
        },
        ..default()
    });

    // Ambient light
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.4, 0.4, 0.6),
        brightness: 300.0,
    });

    // RTS Camera (positioned for better battlefield view)
    let focus_point = Vec3::new(0.0, 0.0, MARCH_DISTANCE / 2.0);
    let initial_distance = 200.0;
    let initial_yaw = 0.0;
    let initial_pitch = -0.5; // Looking down at battlefield
    
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 120.0, 180.0)
                .looking_at(focus_point, Vec3::Y),
            ..default()
        },
        RtsCamera {
            focus_point,
            yaw: initial_yaw,
            pitch: initial_pitch,
            distance: initial_distance,
        },
    ));

    // Load audio assets - all 5 laser sound variations
    let laser_sounds = vec![
        asset_server.load("audio/sfx/laser0.wav"),
        asset_server.load("audio/sfx/laser1.wav"),
        asset_server.load("audio/sfx/laser2.wav"),
        asset_server.load("audio/sfx/laser3.wav"),
        asset_server.load("audio/sfx/laser4.wav"),
    ];
    commands.insert_resource(AudioAssets { laser_sounds });

    // UI text for performance info
    commands.spawn(
        TextBundle::from_section(
            "5,000 vs 5,000 Units | FPS: --\nWSAD: Move | Mouse: Rotate | Scroll: Zoom | F: Volley Fire",
            TextStyle {
                font_size: 20.0,
                color: Color::WHITE,
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        }),
    );
}

fn spawn_army(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Create battle droid mesh (simple humanoid shape using cubes)
    let droid_mesh = create_battle_droid_mesh(&mut meshes);
    
    // Team A materials (current blue-gray theme)
    let team_a_body_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.7, 0.8),
        metallic: 0.3,
        perceptual_roughness: 0.5,
        ..default()
    });
    
    let team_a_head_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.6, 0.4),
        metallic: 0.2,
        perceptual_roughness: 0.6,
        ..default()
    });

    // Team B materials (white/light theme)
    let team_b_body_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.9, 0.95),
        metallic: 0.4,
        perceptual_roughness: 0.3,
        ..default()
    });
    
    let team_b_head_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.95, 1.0),
        metallic: 0.3,
        perceptual_roughness: 0.4,
        ..default()
    });

    let mut rng = rand::thread_rng();
    
    // Calculate formation parameters
    let units_per_row = (FORMATION_WIDTH / UNIT_SPACING) as usize;
    let total_rows = (ARMY_SIZE_PER_TEAM + units_per_row - 1) / units_per_row;
    
    // Spawn Team A (left side, facing right)
    spawn_team(
        &mut commands,
        &mut rng,
        &droid_mesh,
        &team_a_body_material,
        &team_a_head_material,
        Team::A,
        Vec3::new(-BATTLEFIELD_SIZE / 2.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0), // Facing right
        units_per_row,
        total_rows,
    );
    
    // Spawn Team B (right side, facing left) 
    spawn_team(
        &mut commands,
        &mut rng,
        &droid_mesh,
        &team_b_body_material,
        &team_b_head_material,
        Team::B,
        Vec3::new(BATTLEFIELD_SIZE / 2.0, 0.0, 0.0),
        Vec3::new(-1.0, 0.0, 0.0), // Facing left
        units_per_row,
        total_rows,
    );
    
    info!("Spawned {} droids per team ({} total)", ARMY_SIZE_PER_TEAM, ARMY_SIZE_PER_TEAM * 2);
}

fn spawn_team(
    commands: &mut Commands,
    rng: &mut rand::rngs::ThreadRng,
    droid_mesh: &Handle<Mesh>,
    body_material: &Handle<StandardMaterial>,
    head_material: &Handle<StandardMaterial>,
    team: Team,
    team_center: Vec3,
    facing_direction: Vec3,
    units_per_row: usize,
    total_rows: usize,
) {
    for i in 0..ARMY_SIZE_PER_TEAM {
        let row = i / units_per_row;
        let column = i % units_per_row;
        
        // Calculate position in formation relative to team center
        let x = (column as f32 - units_per_row as f32 / 2.0) * UNIT_SPACING;
        let z = (row as f32 - total_rows as f32 / 2.0) * UNIT_SPACING;
        let y = 0.0;
        
        let local_position = Vec3::new(x, y, z);
        let world_position = team_center + local_position;
        
        // Add some randomness to march timing
        let march_offset = rng.gen_range(0.0..2.0 * PI);
        let march_speed = rng.gen_range(0.8..1.2);
        
        // Calculate target position (march toward center of battlefield)
        let target_position = world_position + facing_direction * MARCH_DISTANCE;
        
        // Spawn the battle droid
        let droid_entity = commands.spawn((
            PbrBundle {
                mesh: droid_mesh.clone(),
                material: body_material.clone(),
                transform: Transform::from_translation(world_position)
                    .with_scale(Vec3::splat(0.8))
                    .looking_at(world_position + facing_direction, Vec3::Y),
                ..default()
            },
            BattleDroid {
                march_speed,
                spawn_position: world_position,
                target_position,
                march_offset,
                returning_to_spawn: false,
                team,
            },
            CombatUnit {
                target_scan_timer: rng.gen_range(0.0..TARGET_SCAN_INTERVAL),
                auto_fire_timer: rng.gen_range(0.0..AUTO_FIRE_INTERVAL),
                current_target: None,
            },
            FormationUnit {
                formation_index: i,
                row,
                column,
            },
        )).id();
        
        // Add a head (separate entity as child) - need to create mesh here
        let head_entity = commands.spawn(PbrBundle {
            mesh: droid_mesh.clone(), // Reuse droid mesh for now, can be improved later
            material: head_material.clone(),
            transform: Transform::from_xyz(0.0, 1.2, 0.0)
                .with_scale(Vec3::splat(0.3)),
            ..default()
        }).id();
        
        commands.entity(droid_entity).push_children(&[head_entity]);
    }
}

fn create_battle_droid_mesh(meshes: &mut ResMut<Assets<Mesh>>) -> Handle<Mesh> {
    // Create a simple humanoid battle droid shape
    // This creates a basic robot-like figure that resembles Trade Federation battle droids
    
    use bevy::render::mesh::{Indices, PrimitiveTopology};
    use bevy::render::render_asset::RenderAssetUsages;
    
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    
    // Define vertices for a simple humanoid robot
    // Body is taller and thinner, head is smaller and more angular
    let vertices = vec![
        // Torso (rectangular, thin)
        [-0.3, -0.8, -0.15], [0.3, -0.8, -0.15], [0.3, 0.4, -0.15], [-0.3, 0.4, -0.15], // Front
        [-0.3, -0.8, 0.15], [0.3, -0.8, 0.15], [0.3, 0.4, 0.15], [-0.3, 0.4, 0.15],   // Back
        
        // Arms (thin rectangles)
        // Left arm
        [-0.6, 0.2, -0.1], [-0.4, 0.2, -0.1], [-0.4, -0.4, -0.1], [-0.6, -0.4, -0.1], // Front
        [-0.6, 0.2, 0.1], [-0.4, 0.2, 0.1], [-0.4, -0.4, 0.1], [-0.6, -0.4, 0.1],   // Back
        
        // Right arm
        [0.4, 0.2, -0.1], [0.6, 0.2, -0.1], [0.6, -0.4, -0.1], [0.4, -0.4, -0.1],   // Front
        [0.4, 0.2, 0.1], [0.6, 0.2, 0.1], [0.6, -0.4, 0.1], [0.4, -0.4, 0.1],       // Back
        
        // Legs (thin rectangles)
        // Left leg
        [-0.15, -0.8, -0.1], [0.05, -0.8, -0.1], [0.05, -1.6, -0.1], [-0.15, -1.6, -0.1], // Front
        [-0.15, -0.8, 0.1], [0.05, -0.8, 0.1], [0.05, -1.6, 0.1], [-0.15, -1.6, 0.1],   // Back
        
        // Right leg
        [-0.05, -0.8, -0.1], [0.15, -0.8, -0.1], [0.15, -1.6, -0.1], [-0.05, -1.6, -0.1], // Front
        [-0.05, -0.8, 0.1], [0.15, -0.8, 0.1], [0.15, -1.6, 0.1], [-0.05, -1.6, 0.1],   // Back
    ];
    
    // Convert to Vec3
    let positions: Vec<[f32; 3]> = vertices;
    
    // Generate normals (simplified - pointing outward)
    let normals = vec![
        // Torso normals
        [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], // Front
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],     // Back
        
        // Arm normals (simplified)
        [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], // Left arm front
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],     // Left arm back
        [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], // Right arm front
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],     // Right arm back
        
        // Leg normals (simplified)
        [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], // Left leg front
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],     // Left leg back
        [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], // Right leg front
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],     // Right leg back
    ];
    
    // UV coordinates (basic mapping)
    let uvs: Vec<[f32; 2]> = (0..positions.len()).map(|_| [0.5, 0.5]).collect();
    
    // Define triangular faces for each cube part
    let mut indices = Vec::new();
    
    // Helper function to add cube faces
    let mut add_cube_faces = |start_idx: u32| {
        let faces = [
            // Front face
            [start_idx, start_idx + 1, start_idx + 2], [start_idx, start_idx + 2, start_idx + 3],
            // Back face
            [start_idx + 4, start_idx + 6, start_idx + 5], [start_idx + 4, start_idx + 7, start_idx + 6],
            // Left face
            [start_idx, start_idx + 4, start_idx + 7], [start_idx, start_idx + 7, start_idx + 3],
            // Right face
            [start_idx + 1, start_idx + 2, start_idx + 6], [start_idx + 1, start_idx + 6, start_idx + 5],
            // Top face
            [start_idx + 2, start_idx + 3, start_idx + 7], [start_idx + 2, start_idx + 7, start_idx + 6],
            // Bottom face
            [start_idx, start_idx + 1, start_idx + 5], [start_idx, start_idx + 5, start_idx + 4],
        ];
        
        for face in faces.iter() {
            indices.extend_from_slice(face);
        }
    };
    
    // Add faces for each body part
    add_cube_faces(0);   // Torso
    add_cube_faces(8);   // Left arm
    add_cube_faces(16);  // Right arm
    add_cube_faces(24);  // Left leg
    add_cube_faces(32);  // Right leg
    
    // Set mesh attributes
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    
    meshes.add(mesh)
}

fn animate_march(
    time: Res<Time>,
    mut query: Query<(&mut BattleDroid, &mut Transform), With<FormationUnit>>,
) {
    let time_seconds = time.elapsed_seconds();
    let delta_time = time.delta_seconds();
    
    for (mut droid, mut transform) in query.iter_mut() {
        // Determine current target
        let current_target = if droid.returning_to_spawn {
            droid.spawn_position
        } else {
            droid.target_position
        };
        
        // Calculate direction to target
        let direction = (current_target - transform.translation).normalize_or_zero();
        let distance_to_target = transform.translation.distance(current_target);
        
        // Check if we've reached the target
        if distance_to_target < 1.0 {
            // Switch direction
            droid.returning_to_spawn = !droid.returning_to_spawn;
        } else {
            // Move towards target
            let movement = direction * MARCH_SPEED * delta_time * droid.march_speed;
            transform.translation += movement;
        }
        
        // Add marching animation - slight bobbing motion
        let march_cycle = (time_seconds * droid.march_speed * 4.0 + droid.march_offset).sin();
        let bob_height = march_cycle * 0.03; // Subtle up/down movement
        transform.translation.y += bob_height;
        
        // Slight rotation for more natural look and face movement direction
        let sway = (time_seconds * droid.march_speed * 2.0 + droid.march_offset).sin() * 0.01;
        let forward_rotation = if direction.length() > 0.1 {
            Quat::from_rotation_y(direction.x.atan2(direction.z))
        } else {
            transform.rotation
        };
        transform.rotation = forward_rotation * Quat::from_rotation_y(sway);
    }
}

fn update_camera_info(
    mut query: Query<&mut Text>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
) {
    if let Ok(mut text) = query.get_single_mut() {
        let fps = diagnostics
            .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|fps| fps.smoothed())
            .unwrap_or(0.0);
            
        text.sections[0].value = format!(
            "{} vs {} Units | FPS: {:.1}\nWSAD: Move | Mouse: Rotate | Scroll: Zoom | F: Volley Fire",
            ARMY_SIZE_PER_TEAM, ARMY_SIZE_PER_TEAM, fps
        );
    }
}

fn rts_camera_movement(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut scroll_events: EventReader<MouseWheel>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    mut camera_query: Query<(&mut Transform, &mut RtsCamera)>,
) {
    if let Ok((mut transform, mut camera)) = camera_query.get_single_mut() {
        let delta_time = time.delta_seconds();
        
        // Mouse drag rotation
        if mouse_button_input.pressed(MouseButton::Left) {
            for motion in mouse_motion_events.read() {
                camera.yaw -= motion.delta.x * CAMERA_ROTATION_SPEED;
                camera.pitch = (camera.pitch - motion.delta.y * CAMERA_ROTATION_SPEED)
                    .clamp(-1.5, -0.1); // Limit pitch to reasonable RTS angles
            }
        } else {
            // Clear mouse motion events if not dragging to prevent accumulation
            mouse_motion_events.clear();
        }
        
        // WASD movement (relative to camera's view direction)
        let mut movement = Vec3::ZERO;
        
        if keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp) {
            movement.z -= 1.0; // Move North (away from camera in world space)
        }
        if keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown) {
            movement.z += 1.0; // Move South (toward camera in world space)
        }
        if keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft) {
            movement.x -= 1.0; // Move West (left from camera perspective)
        }
        if keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight) {
            movement.x += 1.0; // Move East (right from camera perspective)
        }
        
        // Apply movement relative to camera rotation
        if movement.length() > 0.0 {
            movement = movement.normalize() * CAMERA_SPEED * delta_time;
            
            // Rotate movement vector by camera yaw to make it relative to camera facing
            // Only rotate around Y axis (yaw) to keep movement on the ground plane
            let yaw_rotation = Mat3::from_rotation_y(camera.yaw);
            let rotated_movement = yaw_rotation * movement;
            
            camera.focus_point += rotated_movement;
        }
        
        // Mouse wheel zoom
        for scroll in scroll_events.read() {
            let zoom_delta = match scroll.unit {
                MouseScrollUnit::Line => scroll.y * CAMERA_ZOOM_SPEED,
                MouseScrollUnit::Pixel => scroll.y * CAMERA_ZOOM_SPEED * 0.1,
            };
            
            camera.distance = (camera.distance - zoom_delta)
                .clamp(CAMERA_MIN_HEIGHT, CAMERA_MAX_HEIGHT);
        }
        
        // Update camera transform based on focus point, yaw, pitch, and distance
        let rotation = Quat::from_euler(EulerRot::YXZ, camera.yaw, camera.pitch, 0.0);
        let offset = rotation * Vec3::new(0.0, 0.0, camera.distance);
        
        transform.translation = camera.focus_point + offset;
        transform.rotation = rotation;
    }
}

fn volley_fire_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    droid_query: Query<(&Transform, &BattleDroid), Without<LaserProjectile>>,
    camera_query: Query<&Transform, (With<RtsCamera>, Without<LaserProjectile>)>,
    audio_assets: Res<AudioAssets>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyF) {
        // Create a simple laser texture (bright center with falloff)
        let texture_size = 16;
        let mut texture_data = Vec::new();
        
        for y in 0..texture_size {
            for x in 0..texture_size {
                let center_x = texture_size as f32 / 2.0;
                let center_y = texture_size as f32 / 2.0;
                let dist = ((x as f32 - center_x).powi(2) + (y as f32 - center_y).powi(2)).sqrt();
                let max_dist = center_x;
                let intensity = (1.0 - (dist / max_dist).clamp(0.0, 1.0)) * 255.0;
                
                texture_data.extend_from_slice(&[
                    0,                    // R - no red
                    intensity as u8,      // G - green
                    0,                    // B - no blue  
                    intensity as u8,      // A - alpha based on distance
                ]);
            }
        }
        
        let laser_texture = images.add(Image::new(
            bevy::render::render_resource::Extent3d {
                width: texture_size,
                height: texture_size,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            texture_data,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
        ));
        
        // Create laser materials for both teams
        let team_a_laser_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.0, 2.0, 0.0), // Bright green for Team A
            base_color_texture: Some(laser_texture.clone()),
            emissive: Color::srgb(0.0, 1.0, 0.0).into(),
            unlit: true, // No lighting calculations
            alpha_mode: AlphaMode::Add, // Additive blending for glow
            cull_mode: None, // Visible from both sides
            ..default()
        });
        
        let team_b_laser_material = materials.add(StandardMaterial {
            base_color: Color::srgb(2.0, 0.0, 0.0), // Bright red for Team B
            base_color_texture: Some(laser_texture),
            emissive: Color::srgb(1.0, 0.0, 0.0).into(),
            unlit: true, // No lighting calculations
            alpha_mode: AlphaMode::Add, // Additive blending for glow
            cull_mode: None, // Visible from both sides
            ..default()
        });
        
        // Create laser mesh (simple quad)
        let laser_mesh = meshes.add(Rectangle::new(LASER_WIDTH, LASER_LENGTH));
        
        // Get camera position for initial orientation
        let camera_position = camera_query.get_single()
            .map(|cam_transform| cam_transform.translation)
            .unwrap_or(Vec3::new(0.0, 100.0, 100.0)); // Fallback position
        
        // Spawn laser from each droid
        for (droid_transform, droid) in droid_query.iter() {
            // Calculate firing position (slightly in front of droid)
            let firing_pos = droid_transform.translation + Vec3::new(0.0, 0.8, 0.0);
            
            // Get droid's forward direction (corrected)
            let forward = -droid_transform.forward().as_vec3(); // Negative to fix direction
            let velocity = forward * LASER_SPEED;
            
            // Calculate proper initial orientation
            let laser_rotation = calculate_laser_orientation(velocity, firing_pos, camera_position);
            let laser_transform = Transform::from_translation(firing_pos)
                .with_rotation(laser_rotation);
            
            // Choose material based on team
            let laser_material = match droid.team {
                Team::A => team_a_laser_material.clone(),
                Team::B => team_b_laser_material.clone(),
            };
            
            // Spawn laser projectile
            commands.spawn((
                PbrBundle {
                    mesh: laser_mesh.clone(),
                    material: laser_material,
                    transform: laser_transform,
                    ..default()
                },
                LaserProjectile {
                    velocity,
                    lifetime: LASER_LIFETIME,
                    team: droid.team, // Add team to track laser ownership
                },
            ));
        }
        
        // Play random laser sound effect for volley fire
        let mut rng = rand::thread_rng();
        let sound = audio_assets.get_random_laser_sound(&mut rng);
        commands.spawn(AudioBundle {
            source: sound,
            settings: PlaybackSettings::DESPAWN,
        });
        
        info!("Volley fire! {} lasers fired!", droid_query.iter().count());
    }
}

fn update_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut projectile_query: Query<(Entity, &mut Transform, &mut LaserProjectile)>,
    camera_query: Query<&Transform, (With<RtsCamera>, Without<LaserProjectile>)>,
) {
    let delta_time = time.delta_seconds();
    
    // Get camera position for billboarding
    let camera_transform = camera_query.get_single().ok();
    
    for (entity, mut transform, mut laser) in projectile_query.iter_mut() {
        // Update lifetime
        laser.lifetime -= delta_time;
        
        // Despawn if lifetime expired
        if laser.lifetime <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        
        // Move projectile
        transform.translation += laser.velocity * delta_time;
        
        // Update orientation using our improved calculation
        if let Some(cam_transform) = camera_transform {
            transform.rotation = calculate_laser_orientation(
                laser.velocity,
                transform.translation,
                cam_transform.translation,
            );
        }
    }
}

fn target_acquisition_system(
    time: Res<Time>,
    mut combat_query: Query<(Entity, &Transform, &BattleDroid, &mut CombatUnit)>,
) {
    let delta_time = time.delta_seconds();
    
    // Collect all unit data first to avoid borrowing issues
    let all_units: Vec<(Entity, Vec3, Team)> = combat_query
        .iter()
        .map(|(entity, transform, droid, _)| (entity, transform.translation, droid.team))
        .collect();
    
    for (entity, transform, droid, mut combat_unit) in combat_query.iter_mut() {
        // Update target scan timer
        combat_unit.target_scan_timer -= delta_time;
        
        if combat_unit.target_scan_timer <= 0.0 {
            combat_unit.target_scan_timer = TARGET_SCAN_INTERVAL;
            
            // Find closest enemy within range
            let mut closest_enemy: Option<Entity> = None;
            let mut closest_distance = f32::INFINITY;
            
            for &(target_entity, target_position, target_team) in &all_units {
                // Skip allies and self
                if target_team == droid.team || target_entity == entity {
                    continue;
                }
                
                let distance = transform.translation.distance(target_position);
                if distance <= TARGETING_RANGE && distance < closest_distance {
                    closest_distance = distance;
                    closest_enemy = Some(target_entity);
                }
            }
            
            combat_unit.current_target = closest_enemy;
        }
    }
}

fn auto_fire_system(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut combat_query: Query<(&Transform, &BattleDroid, &mut CombatUnit)>,
    target_query: Query<&Transform, With<BattleDroid>>,
    camera_query: Query<&Transform, (With<RtsCamera>, Without<LaserProjectile>)>,
    audio_assets: Res<AudioAssets>,
) {
    let delta_time = time.delta_seconds();
    
    // Get camera position for initial orientation
    let camera_position = camera_query.get_single()
        .map(|cam_transform| cam_transform.translation)
        .unwrap_or(Vec3::new(0.0, 100.0, 100.0)); // Fallback position
    
    // Count shots fired this frame for audio throttling
    let mut shots_fired = 0;
    const MAX_AUDIO_PER_FRAME: usize = 5; // Limit concurrent audio to prevent spam
    
    for (droid_transform, droid, mut combat_unit) in combat_query.iter_mut() {
        // Update auto fire timer
        combat_unit.auto_fire_timer -= delta_time;
        
        if combat_unit.auto_fire_timer <= 0.0 && combat_unit.current_target.is_some() {
            if let Some(target_entity) = combat_unit.current_target {
                if let Ok(target_transform) = target_query.get(target_entity) {
                    // Reset timer
                    combat_unit.auto_fire_timer = AUTO_FIRE_INTERVAL;
                    
                    // Create laser material based on team
                    let laser_material = match droid.team {
                        Team::A => materials.add(StandardMaterial {
                            base_color: Color::srgb(0.0, 2.0, 0.0), // Green for Team A
                            emissive: Color::srgb(0.0, 1.0, 0.0).into(),
                            unlit: true,
                            alpha_mode: AlphaMode::Add,
                            cull_mode: None,
                            ..default()
                        }),
                        Team::B => materials.add(StandardMaterial {
                            base_color: Color::srgb(2.0, 0.0, 0.0), // Red for Team B
                            emissive: Color::srgb(1.0, 0.0, 0.0).into(),
                            unlit: true,
                            alpha_mode: AlphaMode::Add,
                            cull_mode: None,
                            ..default()
                        }),
                    };
                    
                    let laser_mesh = meshes.add(Rectangle::new(LASER_WIDTH, LASER_LENGTH));
                    
                    // Calculate firing position and direction toward target
                    let firing_pos = droid_transform.translation + Vec3::new(0.0, 0.8, 0.0);
                    let target_pos = target_transform.translation + Vec3::new(0.0, 0.8, 0.0);
                    let direction = (target_pos - firing_pos).normalize();
                    let velocity = direction * LASER_SPEED;
                    
                    // Calculate proper initial orientation
                    let laser_rotation = calculate_laser_orientation(velocity, firing_pos, camera_position);
                    let laser_transform = Transform::from_translation(firing_pos)
                        .with_rotation(laser_rotation);
                    
                    // Spawn targeted laser
                    commands.spawn((
                        PbrBundle {
                            mesh: laser_mesh,
                            material: laser_material,
                            transform: laser_transform,
                            ..default()
                        },
                        LaserProjectile {
                            velocity,
                            lifetime: LASER_LIFETIME,
                            team: droid.team,
                        },
                    ));
                    
                    // Play random laser sound (throttled to prevent audio spam)
                    shots_fired += 1;
                    if shots_fired <= MAX_AUDIO_PER_FRAME {
                        let mut rng = rand::thread_rng();
                        let sound = audio_assets.get_random_laser_sound(&mut rng);
                        commands.spawn(AudioBundle {
                            source: sound,
                            settings: PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::new(0.3)),
                        });
                    }
                }
            }
        }
    }
}

fn collision_detection_system(
    mut commands: Commands,
    mut spatial_grid: ResMut<SpatialGrid>,
    laser_query: Query<(Entity, &Transform, &LaserProjectile)>,
    droid_query: Query<(Entity, &Transform, &BattleDroid), Without<LaserProjectile>>,
) {
    // Clear and rebuild the spatial grid each frame
    spatial_grid.clear();
    
    // Populate grid with droids
    for (droid_entity, droid_transform, _) in droid_query.iter() {
        spatial_grid.add_droid(droid_entity, droid_transform.translation);
    }
    
    let mut entities_to_despawn = std::collections::HashSet::new();
    
    // Check collisions for each laser using spatial grid
    for (laser_entity, laser_transform, laser) in laser_query.iter() {
        // Skip if laser already marked for despawn
        if entities_to_despawn.contains(&laser_entity) {
            continue;
        }
        
        // Get only nearby droids using spatial grid
        let nearby_droids = spatial_grid.get_nearby_droids(laser_transform.translation);
        
        for &droid_entity in &nearby_droids {
            // Skip if droid already marked for despawn
            if entities_to_despawn.contains(&droid_entity) {
                continue;
            }
            
            // Get droid data - we need to check if it still exists and get its data
            if let Ok((_, droid_transform, droid)) = droid_query.get(droid_entity) {
                // Skip friendly fire
                if laser.team == droid.team {
                    continue;
                }
                
                // Simple sphere collision detection
                let distance = laser_transform.translation.distance(droid_transform.translation);
                if distance <= COLLISION_RADIUS {
                    // Hit! Mark both laser and droid for despawn
                    entities_to_despawn.insert(laser_entity);
                    entities_to_despawn.insert(droid_entity);
                    break; // Laser can only hit one target
                }
            }
        }
    }
    
    // Despawn all marked entities
    for entity in entities_to_despawn {
        if let Some(entity_commands) = commands.get_entity(entity) {
            entity_commands.despawn_recursive();
        }
    }
} 
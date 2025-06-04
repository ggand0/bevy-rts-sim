use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use rand::Rng;
use std::f32::consts::PI;

const ARMY_SIZE: usize = 10_000;
const FORMATION_WIDTH: f32 = 200.0;
const UNIT_SPACING: f32 = 2.0;

#[derive(Component)]
struct BattleDroid {
    march_speed: f32,
    base_position: Vec3,
    march_offset: f32,
}

#[derive(Component)]
struct FormationUnit {
    formation_index: usize,
    row: usize,
    column: usize,
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PanOrbitCameraPlugin,
        ))
        .add_systems(Startup, (setup_scene, spawn_army))
        .add_systems(Update, (
            animate_march,
            update_camera_info,
        ))
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
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

    // Ground plane
    commands.spawn(PbrBundle {
        mesh: meshes.add(Rectangle::new(400.0, 400.0)),
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

    // Camera with pan-orbit controls
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 80.0, 120.0)
                .looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
            ..default()
        },
        PanOrbitCamera::default(),
    ));

    // UI text for performance info
    commands.spawn(
        TextBundle::from_section(
            "Battle Droid Army - 10,000 Units\nWSAD/Mouse: Camera controls\nScroll: Zoom",
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
    
    // Materials for different parts
    let body_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.7, 0.8),
        metallic: 0.3,
        perceptual_roughness: 0.5,
        ..default()
    });
    
    let head_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.6, 0.4),
        metallic: 0.2,
        perceptual_roughness: 0.6,
        ..default()
    });

    let mut rng = rand::thread_rng();
    
    // Calculate formation parameters
    let units_per_row = (FORMATION_WIDTH / UNIT_SPACING) as usize;
    let total_rows = (ARMY_SIZE + units_per_row - 1) / units_per_row;
    
    for i in 0..ARMY_SIZE {
        let row = i / units_per_row;
        let column = i % units_per_row;
        
        // Calculate position in formation
        let x = (column as f32 - units_per_row as f32 / 2.0) * UNIT_SPACING;
        let z = (row as f32 - total_rows as f32 / 2.0) * UNIT_SPACING;
        let y = 0.0;
        
        let position = Vec3::new(x, y, z);
        
        // Add some randomness to march timing
        let march_offset = rng.gen_range(0.0..2.0 * PI);
        let march_speed = rng.gen_range(0.8..1.2);
        
        // Spawn the battle droid
        let droid_entity = commands.spawn((
            PbrBundle {
                mesh: droid_mesh.clone(),
                material: body_material.clone(),
                transform: Transform::from_translation(position)
                    .with_scale(Vec3::splat(0.8)),
                ..default()
            },
            BattleDroid {
                march_speed,
                base_position: position,
                march_offset,
            },
            FormationUnit {
                formation_index: i,
                row,
                column,
            },
        )).id();
        
        // Add a head (separate entity as child)
        let head_entity = commands.spawn(PbrBundle {
            mesh: meshes.add(Cuboid::new(0.6, 0.6, 0.6)),
            material: head_material.clone(),
            transform: Transform::from_xyz(0.0, 1.2, 0.0),
            ..default()
        }).id();
        
        commands.entity(droid_entity).push_children(&[head_entity]);
    }
    
    info!("Spawned {} battle droids in formation", ARMY_SIZE);
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
    mut query: Query<(&BattleDroid, &mut Transform), With<FormationUnit>>,
) {
    let time_seconds = time.elapsed_seconds();
    
    for (droid, mut transform) in query.iter_mut() {
        // Simple marching animation - slight bobbing motion
        let march_cycle = (time_seconds * droid.march_speed + droid.march_offset).sin();
        let bob_height = march_cycle * 0.05; // Subtle up/down movement
        
        // Update position with marching bob
        transform.translation = droid.base_position + Vec3::new(0.0, bob_height, 0.0);
        
        // Slight rotation for more natural look
        let sway = (time_seconds * droid.march_speed * 2.0 + droid.march_offset).sin() * 0.02;
        transform.rotation = Quat::from_rotation_y(sway);
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
            "Battle Droid Army - {} Units\nFPS: {:.1}\nWSAD/Mouse: Camera controls\nScroll: Zoom",
            ARMY_SIZE, fps
        );
    }
} 
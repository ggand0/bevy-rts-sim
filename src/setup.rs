// Scene setup and army spawning module
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rand::Rng;
use std::f32::consts::PI;
use crate::types::*;
use crate::constants::*;
use crate::formation::*;

pub fn setup_scene(
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
            "5,000 vs 5,000 Units | FPS: --\nWSAD: Move | Mouse: Rotate | Scroll: Zoom | F: Volley Fire\nQ/E/R/T: Formations (Rect/Line/Box/Wedge) | G: Advance | H: Retreat",
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

pub fn spawn_army_with_squads(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut squad_manager: ResMut<SquadManager>,
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
    
    // Team A commander materials (yellow/gold like SW)
    let team_a_commander_body_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.8, 0.4), // Golden yellow
        metallic: 0.5,
        perceptual_roughness: 0.3,
        ..default()
    });
    
    let team_a_commander_head_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.9, 0.5), // Bright gold
        metallic: 0.4,
        perceptual_roughness: 0.4,
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
    
    // Team B commander materials (red/orange like enemy commanders)
    let team_b_commander_body_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.5, 0.3), // Orange-red
        metallic: 0.5,
        perceptual_roughness: 0.3,
        ..default()
    });
    
    let team_b_commander_head_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.6, 0.4), // Bright orange
        metallic: 0.4,
        perceptual_roughness: 0.4,
        ..default()
    });

    let mut rng = rand::thread_rng();
    
    // Calculate number of squads per team
    let squads_per_team = ARMY_SIZE_PER_TEAM / SQUAD_SIZE;
    let squads_per_row = (squads_per_team as f32).sqrt().ceil() as usize;
    
    // Spawn Team A squads (left side, facing right)
    spawn_team_squads(
        &mut commands,
        &mut squad_manager,
        &mut rng,
        &droid_mesh,
        &team_a_body_material,
        &team_a_head_material,
        &team_a_commander_body_material,
        &team_a_commander_head_material,
        &mut materials,
        Team::A,
        Vec3::new(-BATTLEFIELD_SIZE / 2.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0), // Facing right
        squads_per_team,
        squads_per_row,
    );
    
    // Spawn Team B squads (right side, facing left) 
    spawn_team_squads(
        &mut commands,
        &mut squad_manager,
        &mut rng,
        &droid_mesh,
        &team_b_body_material,
        &team_b_head_material,
        &team_b_commander_body_material,
        &team_b_commander_head_material,
        &mut materials,
        Team::B,
        Vec3::new(BATTLEFIELD_SIZE / 2.0, 0.0, 0.0),
        Vec3::new(-1.0, 0.0, 0.0), // Facing left
        squads_per_team,
        squads_per_row,
    );
    
    info!("Spawned {} squads per team ({} droids per squad, {} total units)", 
          squads_per_team, SQUAD_SIZE, squads_per_team * SQUAD_SIZE * 2);
}

fn spawn_team_squads(
    commands: &mut Commands,
    squad_manager: &mut ResMut<SquadManager>,
    rng: &mut rand::rngs::ThreadRng,
    droid_mesh: &Handle<Mesh>,
    body_material: &Handle<StandardMaterial>,
    head_material: &Handle<StandardMaterial>,
    _commander_body_material: &Handle<StandardMaterial>,
    _commander_head_material: &Handle<StandardMaterial>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    team: Team,
    team_center: Vec3,
    facing_direction: Vec3,
    total_squads: usize,
    squads_per_row: usize,
) {
    for squad_index in 0..total_squads {
        let squad_row = squad_index / squads_per_row;
        let squad_col = squad_index % squads_per_row;
        
        // Calculate squad center position - extra spacing for line formations
        let base_squad_width = SQUAD_WIDTH as f32 * SQUAD_HORIZONTAL_SPACING;
        let line_formation_width = 25.0 * (SQUAD_HORIZONTAL_SPACING * 1.5); // Account for line formation
        let max_formation_width = base_squad_width.max(line_formation_width);
        
        let squad_offset_x = (squad_col as f32 - (squads_per_row as f32 - 1.0) / 2.0) * 
                             (max_formation_width + INTER_SQUAD_SPACING);
        let squad_offset_z = (squad_row as f32 - (total_squads as f32 / squads_per_row as f32 - 1.0) / 2.0) * 
                             (SQUAD_DEPTH as f32 * SQUAD_VERTICAL_SPACING + INTER_SQUAD_SPACING);
        
        let right = Vec3::new(facing_direction.z, 0.0, -facing_direction.x).normalize();
        let squad_center = team_center + right * squad_offset_x + facing_direction * squad_offset_z;
        
        // Create the squad
        let squad_id = squad_manager.create_squad(team, squad_center, facing_direction);
        
        // Get formation positions for this squad
        let formation_positions = assign_formation_positions(FormationType::Rectangle);
        let commander_pos = get_commander_position(FormationType::Rectangle);
        
        // Spawn units for this squad
        for (unit_index, &(row, col)) in formation_positions.iter().enumerate() {
            if unit_index >= SQUAD_SIZE {
                break;
            }
            
            let is_commander = (row, col) == commander_pos;
            
            // Calculate unit position within the squad formation
            let formation_offset = calculate_formation_offset(
                FormationType::Rectangle,
                row,
                col,
                facing_direction,
            );
            let unit_position = squad_center + formation_offset;
            
            // Add some randomness to march timing but reduce speed variance
            let march_offset = rng.gen_range(0.0..2.0 * PI);
            let march_speed = rng.gen_range(0.96..1.04); // Much smaller variance for tighter formations
            
            // Units start stationary - target position same as spawn position initially
            let target_position = unit_position;
            
            // Choose materials based on commander status - create unique materials for commanders only
            let unit_body_material = if is_commander {
                // Create a unique commander material for this specific unit
                materials.add(StandardMaterial {
                    base_color: match team {
                        Team::A => Color::srgb(0.9, 0.8, 0.4), // Golden yellow
                        Team::B => Color::srgb(0.9, 0.5, 0.3), // Orange-red
                    },
                    metallic: 0.5,
                    perceptual_roughness: 0.3,
                    ..default()
                })
            } else {
                body_material.clone() // Share materials for regular units to maintain performance
            };
            let unit_head_material = if is_commander {
                // Create a unique commander head material for this specific unit
                materials.add(StandardMaterial {
                    base_color: match team {
                        Team::A => Color::srgb(1.0, 0.9, 0.5), // Bright gold
                        Team::B => Color::srgb(1.0, 0.6, 0.4), // Bright orange
                    },
                    metallic: 0.4,
                    perceptual_roughness: 0.4,
                    ..default()
                })
            } else {
                head_material.clone() // Share materials for regular units to maintain performance
            };
            
            // Spawn the battle droid
            let droid_entity = commands.spawn((
                PbrBundle {
                    mesh: droid_mesh.clone(),
                    material: unit_body_material,
                    transform: Transform::from_translation(unit_position)
                        .with_scale(if is_commander { Vec3::splat(0.9) } else { Vec3::splat(0.8) }) // Commanders slightly larger
                        .looking_at(unit_position + facing_direction, Vec3::Y),
                    ..default()
                },
                BattleDroid {
                    march_speed,
                    spawn_position: unit_position,
                    target_position,
                    march_offset,
                    returning_to_spawn: false, // Start stationary at spawn
                    team,
                },
                CombatUnit {
                    target_scan_timer: rng.gen_range(0.0..TARGET_SCAN_INTERVAL),
                    auto_fire_timer: rng.gen_range(0.0..AUTO_FIRE_INTERVAL),
                    current_target: None,
                },
                SquadMember {
                    squad_id,
                    formation_position: (row, col),
                    is_commander,
                },
                FormationOffset {
                    local_offset: formation_offset,
                    target_world_position: unit_position,
                },
            )).id();
            
            // Add unit to squad manager
            squad_manager.add_unit_to_squad(squad_id, droid_entity);
            
            // Set commander if this is the commander unit
            if is_commander {
                if let Some(squad) = squad_manager.get_squad_mut(squad_id) {
                    squad.commander = Some(droid_entity);
                }
            }
            
            // Add a head (separate entity as child)
            let head_entity = commands.spawn(PbrBundle {
                mesh: droid_mesh.clone(),
                material: unit_head_material,
                transform: Transform::from_xyz(0.0, 1.2, 0.0)
                    .with_scale(Vec3::splat(0.3)),
                ..default()
            }).id();
            
            commands.entity(droid_entity).push_children(&[head_entity]);
        }
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

// Objective system module - Uplink Tower mechanics
use bevy::prelude::*;
use rand::Rng;
use crate::types::*;
use crate::constants::*;
use crate::explosion_shader::*;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

// ===== TOWER CREATION =====

pub fn create_uplink_tower_mesh(meshes: &mut ResMut<Assets<Mesh>>) -> Handle<Mesh> {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Helper function to add a box with proper normals
    let mut add_box = |center: Vec3, size: Vec3| {
        let base = vertices.len() as u32;
        let hw = size.x / 2.0;
        let hh = size.y / 2.0;
        let hd = size.z / 2.0;

        // 8 vertices of the box
        let box_vertices = [
            [center.x - hw, center.y - hh, center.z - hd], // 0: bottom-left-back
            [center.x + hw, center.y - hh, center.z - hd], // 1: bottom-right-back
            [center.x + hw, center.y - hh, center.z + hd], // 2: bottom-right-front
            [center.x - hw, center.y - hh, center.z + hd], // 3: bottom-left-front
            [center.x - hw, center.y + hh, center.z - hd], // 4: top-left-back
            [center.x + hw, center.y + hh, center.z - hd], // 5: top-right-back
            [center.x + hw, center.y + hh, center.z + hd], // 6: top-right-front
            [center.x - hw, center.y + hh, center.z + hd], // 7: top-left-front
        ];
        
        vertices.extend_from_slice(&box_vertices);
        
        // Proper face normals for each vertex - one normal per vertex per face
        // We'll use proper per-face normals
        let _face_normals = [
            [0.0, -1.0, 0.0], // bottom face normal
            [0.0, 1.0, 0.0],  // top face normal  
            [-1.0, 0.0, 0.0], // left face normal
            [1.0, 0.0, 0.0],  // right face normal
            [0.0, 0.0, -1.0], // back face normal
            [0.0, 0.0, 1.0],  // front face normal
        ];
        
        // Add normals for each vertex (we'll use averaged normals for simplicity)
        for _ in 0..8 {
            normals.push([0.0, 1.0, 0.0]); // For now, keep simple upward normals
        }

        // Box face indices (12 triangles) - Fixed winding order
        let box_indices = [
            // Bottom face (looking up from below)
            base + 0, base + 1, base + 2, base + 0, base + 2, base + 3,
            // Top face (looking down from above)
            base + 4, base + 6, base + 5, base + 4, base + 7, base + 6,
            // Left face
            base + 0, base + 7, base + 4, base + 0, base + 3, base + 7,
            // Right face
            base + 1, base + 5, base + 6, base + 1, base + 6, base + 2,
            // Back face
            base + 0, base + 4, base + 5, base + 0, base + 5, base + 1,
            // Front face
            base + 3, base + 2, base + 6, base + 3, base + 6, base + 7,
        ];
        indices.extend_from_slice(&box_indices);
    };

    let tower_height = TOWER_HEIGHT;
    let base_width = TOWER_BASE_WIDTH;
    
    // === CENTRAL SPINE DIMENSIONS (DEFINED EARLY) ===
    let spine_width = base_width * 0.35;  // Wider dimension (increased from 0.25)
    let spine_depth = base_width * 0.25;  // Narrower dimension (increased from 0.15)
    let spine_start_y = 1.0;
    
    // === FOUNDATION SYSTEM (PROPERLY CONNECTED) ===
    // Underground foundation for proper grounding
    add_box(
        Vec3::new(0.0, -0.8, 0.0),
        Vec3::new(spine_width * 1.8, 1.6, spine_depth * 1.8)
    );
    
    // Ground-level foundation platform - directly connected to spine
    add_box(
        Vec3::new(0.0, 0.4, 0.0),
        Vec3::new(spine_width * 1.4, 0.8, spine_depth * 1.4)
    );
    
    // Direct connection to spine base - no gap
    add_box(
        Vec3::new(0.0, spine_start_y - 0.1, 0.0),
        Vec3::new(spine_width * 1.1, 0.2, spine_depth * 1.1)
    );

    // === CENTRAL SPINE (RECTANGULAR CORE) ===
    // This is the main structural element - tall, slender, rectangular but slightly wider as requested
    let spine_height = tower_height - spine_start_y - 8.0; // Leave room for pointed top
    
    // Main central spine - rectangular cross-section
    add_box(
        Vec3::new(0.0, spine_start_y + spine_height / 2.0, 0.0),
        Vec3::new(spine_width, spine_height, spine_depth)
    );

    // === INTEGRATED ARCHITECTURAL MODULES ===
    // Create modules that are much closer to the spine, like in the reference images
    let module_levels = 20;
    let module_spacing = spine_height / module_levels as f32;
    
    for level in 0..module_levels {
        let level_y = spine_start_y + (level as f32 + 0.5) * module_spacing;
        let level_factor = 1.0 - (level as f32 / module_levels as f32) * 0.2; // Very slight taper
        
        // Vary the module pattern - sometimes none, sometimes 1-3 modules
        let module_pattern = level % 7;
        let module_count = match module_pattern {
            0 | 1 => 0, // Some levels have no modules for variation
            2 | 5 => 1, // Single module
            3 | 4 => 2, // Two modules opposite each other
            _ => 3,     // Three modules
        };
        
        for module in 0..module_count {
            let angle = (module as f32 / module_count as f32) * std::f32::consts::TAU + (level as f32 * 0.3);
            
            // Much closer to spine - attached rather than floating
            let module_distance = spine_width * 0.6; // Was 1.8, now much closer
            let module_x = angle.cos() * module_distance;
            let module_z = angle.sin() * module_distance;
            
            // Rectangular modules that extend from the spine
            let module_width = 0.8 * level_factor;
            let module_height = 2.0 + (level % 3) as f32 * 0.5; // Varying heights
            let module_depth = 0.6 * level_factor;
            
            add_box(
                Vec3::new(module_x, level_y, module_z),
                Vec3::new(module_width, module_height, module_depth)
            );
            
            // Additional stacked modules for some levels (like reference image)
            if level % 5 == 0 {
                add_box(
                    Vec3::new(module_x * 1.2, level_y + module_height * 0.3, module_z * 1.2),
                    Vec3::new(module_width * 0.7, module_height * 0.6, module_depth * 0.7)
                );
            }
        }
        
        // Spine structural details at regular intervals
        if level % 4 == 0 {
            // Horizontal structural elements around the spine
            for segment in 0..4 {
                let seg_angle = (segment as f32 / 4.0) * std::f32::consts::TAU;
                let seg_x = seg_angle.cos() * spine_width * 0.52;
                let seg_z = seg_angle.sin() * spine_depth * 0.52;
                
                add_box(
                    Vec3::new(seg_x, level_y, seg_z),
                    Vec3::new(0.12, 0.4, 0.12)
                );
            }
        }
    }

    // === UPPER BUILDING SECTION (FLAT TOP) ===
    // Continue the spine upward like a normal building
    let upper_start_y = spine_start_y + spine_height;
    let upper_height = 10.0;
    
    // Main upper spine section - same width as main spine
    add_box(
        Vec3::new(0.0, upper_start_y + upper_height / 2.0, 0.0),
        Vec3::new(spine_width, upper_height, spine_depth)
    );
    
    // === REFINED ARCHITECTURAL DETAILS ===
    // Thin corner reinforcements at the top
    for corner in 0..4 {
        let angle = (corner as f32 / 4.0) * std::f32::consts::TAU + std::f32::consts::FRAC_PI_4;
        let corner_x = angle.cos() * spine_width * 0.45;
        let corner_z = angle.sin() * spine_depth * 0.45;
        
        // Thinner corner elements
        add_box(
            Vec3::new(corner_x, upper_start_y + upper_height - 1.0, corner_z),
            Vec3::new(0.15, 2.0, 0.15)
        );
    }
    
    // Thin equipment housings on the sides
    for side in 0..2 {
        let angle = side as f32 * std::f32::consts::PI; // Front and back
        let side_x = angle.cos() * spine_width * 0.52;
        let side_z = angle.sin() * spine_depth * 0.52;
        
        // Thinner equipment box
        add_box(
            Vec3::new(side_x, upper_start_y + upper_height - 2.0, side_z),
            Vec3::new(0.4, 1.5, 0.2)
        );
    }
    
    // Vertical accent lines on facades
    for facade in 0..2 {
        let angle = facade as f32 * std::f32::consts::PI;
        let facade_x = angle.cos() * spine_width * 0.51;
        let facade_z = angle.sin() * spine_depth * 0.51;
        
        // Thin vertical accent
        add_box(
            Vec3::new(facade_x, upper_start_y + upper_height / 2.0, facade_z),
            Vec3::new(0.08, upper_height * 0.8, 0.08)
        );
    }
    
    // Horizontal bands for architectural interest
    for band in 0..3 {
        let band_y = upper_start_y + (band + 1) as f32 * (upper_height / 4.0);
        
        // Thin horizontal accent band
        add_box(
            Vec3::new(0.0, band_y, spine_depth * 0.52),
            Vec3::new(spine_width * 0.8, 0.1, 0.1)
        );
    }
    
    // === ROOFTOP ANTENNA CLUSTER ===
    let roof_y = upper_start_y + upper_height;
    
    // Antenna array clustered on the northeast corner/edge
    let antenna_base_x = spine_width * 0.25;
    let antenna_base_z = spine_depth * 0.3;
    
    // Main tall antenna (tallest in the group)
    add_box(
        Vec3::new(antenna_base_x, roof_y + 6.0, antenna_base_z),
        Vec3::new(0.08, 12.0, 0.08)
    );
    
    // Secondary tall antenna
    add_box(
        Vec3::new(antenna_base_x + 0.3, roof_y + 4.5, antenna_base_z - 0.2),
        Vec3::new(0.06, 9.0, 0.06)
    );
    
    // Medium height antennas
    add_box(
        Vec3::new(antenna_base_x - 0.2, roof_y + 3.0, antenna_base_z + 0.1),
        Vec3::new(0.05, 6.0, 0.05)
    );
    
    add_box(
        Vec3::new(antenna_base_x + 0.1, roof_y + 3.5, antenna_base_z + 0.3),
        Vec3::new(0.05, 7.0, 0.05)
    );
    
    // Shorter antennas for variety
    add_box(
        Vec3::new(antenna_base_x - 0.1, roof_y + 2.0, antenna_base_z - 0.1),
        Vec3::new(0.04, 4.0, 0.04)
    );
    
    add_box(
        Vec3::new(antenna_base_x + 0.4, roof_y + 2.5, antenna_base_z + 0.1),
        Vec3::new(0.04, 5.0, 0.04)
    );
    
    // Tiny support antennas
    add_box(
        Vec3::new(antenna_base_x + 0.2, roof_y + 1.25, antenna_base_z - 0.3),
        Vec3::new(0.03, 2.5, 0.03)
    );
    
    // Antenna support platform (small)
    add_box(
        Vec3::new(antenna_base_x, roof_y + 0.15, antenna_base_z),
        Vec3::new(0.8, 0.3, 0.6)
    );
    
    // Rooftop equipment/details
    add_box(
        Vec3::new(spine_width * 0.2, roof_y + 0.3, 0.0),
        Vec3::new(0.4, 0.6, 0.3)
    );
    add_box(
        Vec3::new(-spine_width * 0.2, roof_y + 0.4, spine_depth * 0.15),
        Vec3::new(0.3, 0.8, 0.2)
    );

    // === STRUCTURAL SUPPORT ELEMENTS ===
    // Add some connecting elements between major module levels for structural integrity
    for level in (3..module_levels).step_by(6) {
        let level_y = spine_start_y + (level as f32) * module_spacing;
        
        // Cross-bracing elements
        for brace in 0..4 {
            let angle = (brace as f32 / 4.0) * std::f32::consts::TAU + std::f32::consts::FRAC_PI_4;
            let brace_distance = spine_width * 1.4;
            let brace_x = angle.cos() * brace_distance;
            let brace_z = angle.sin() * brace_distance;
            
            add_box(
                Vec3::new(brace_x, level_y, brace_z),
                Vec3::new(0.12, 2.0, 0.12)
            );
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    meshes.add(mesh)
}

pub fn spawn_uplink_towers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
        PbrBundle {
            mesh: tower_mesh.clone(),
            material: team_a_material,
            transform: Transform::from_translation(team_a_pos)
                .with_scale(Vec3::splat(1.0)),
            ..default()
        },
        UplinkTower {
            team: Team::A,
            destruction_radius: TOWER_DESTRUCTION_RADIUS,
        },
        ObjectiveTarget {
            team: Team::A,
            is_primary: true,
        },
        Health::new(TOWER_MAX_HEALTH),
    ));
    
    // Spawn Team B tower (right side, behind army)
    let team_b_pos = Vec3::new(BATTLEFIELD_SIZE / 2.0 + 30.0, 0.0, 0.0);
    commands.spawn((
        PbrBundle {
            mesh: tower_mesh,
            material: team_b_material,
            transform: Transform::from_translation(team_b_pos)
                .with_scale(Vec3::splat(1.0)),
            ..default()
        },
        UplinkTower {
            team: Team::B,
            destruction_radius: TOWER_DESTRUCTION_RADIUS,
        },
        ObjectiveTarget {
            team: Team::B,
            is_primary: true,
        },
        Health::new(TOWER_MAX_HEALTH),
    ));
    
    info!("Spawned Uplink Towers for both teams");
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    tower_query: Query<(Entity, &Transform, &UplinkTower, &Health), (With<UplinkTower>, Without<PendingExplosion>)>,
    droid_query: Query<(Entity, &Transform, &BattleDroid), With<BattleDroid>>,
    mut game_state: ResMut<GameState>,
) {
    for (tower_entity, tower_transform, tower, tower_health) in tower_query.iter() {
        if tower_health.is_dead() {
            info!("Processing tower destruction for team {:?}", tower.team);
            
            // Mark game as ended
            game_state.tower_destroyed(tower.team);
            
            // Find all friendly units within destruction radius
            let mut units_to_explode = Vec::new();
            for (droid_entity, droid_transform, droid) in droid_query.iter() {
                // Only friendly units explode (loss of command link)
                if droid.team == tower.team {
                    let distance = tower_transform.translation.distance(droid_transform.translation);
                    if distance <= tower.destruction_radius {
                        units_to_explode.push(droid_entity);
                    }
                }
            }
            
            // Add delayed explosions for dramatic effect
            let explosion_count = units_to_explode.len();
            let mut rng = rand::thread_rng();
            for unit_entity in units_to_explode {
                let delay = rng.gen_range(EXPLOSION_DELAY_MIN..EXPLOSION_DELAY_MAX);
                commands.entity(unit_entity).insert(PendingExplosion {
                    delay_timer: delay,
                    explosion_power: 1.0,
                });
            }
            
                            // Create massive explosion effect at tower location using RTS-style
            spawn_explosion_effect(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut images,
                tower_transform.translation,
                tower.destruction_radius,
                3.0, // High intensity for tower explosion
                EXPLOSION_EFFECT_DURATION * 2.0, // Tower explosion lasts longer
            );
            
            // Add PendingExplosion to tower to prevent re-processing
            commands.entity(tower_entity).insert(PendingExplosion {
                delay_timer: 0.1, // Very short delay before removing tower
                explosion_power: 3.0,
            });
            
            info!("Tower {:?} destroyed! {} friendly units scheduled for cascade explosion", 
                  tower.team, explosion_count);
        }
    }
}

// ===== DELAYED EXPLOSION SYSTEM =====

pub fn pending_explosion_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut explosion_query: Query<(Entity, &mut PendingExplosion, &Transform, Option<&UplinkTower>), With<PendingExplosion>>,
    time: Res<Time>,
) {
    for (entity, mut pending, transform, tower_component) in explosion_query.iter_mut() {
        pending.delay_timer -= time.delta_seconds();
        
        if pending.delay_timer <= 0.0 {
            // Determine explosion radius based on entity type
            let explosion_radius = if tower_component.is_some() {
                tower_component.unwrap().destruction_radius * 0.5 // Tower explosion
            } else {
                8.0 // Individual unit explosion radius (increased from 5.0)
            };
            
            // Create explosion effect using RTS-style
            spawn_explosion_effect(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut images,
                transform.translation,
                explosion_radius,
                pending.explosion_power,
                EXPLOSION_EFFECT_DURATION,
            );
            
            // Remove the entity
            commands.entity(entity).despawn_recursive();
        }
    }
}

// ===== EXPLOSION VISUAL EFFECTS =====

pub fn explosion_effect_system(
    mut commands: Commands,
    mut explosion_query: Query<(Entity, &mut ExplosionEffect, &Transform), With<ExplosionEffect>>,
    time: Res<Time>,
) {
    for (entity, mut effect, _transform) in explosion_query.iter_mut() {
        effect.timer += time.delta_seconds();
        
        // TODO: Update visual effect (scale, alpha, particle systems)
        // For now, just manage lifetime
        
        if effect.timer >= effect.max_time {
            commands.entity(entity).despawn();
        }
    }
}

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

// ===== UI SYSTEM =====

pub fn update_objective_ui_system(
    mut ui_query: Query<&mut Text, With<ObjectiveUI>>,
    tower_query: Query<(&UplinkTower, &Health), With<UplinkTower>>,
    game_state: Res<GameState>,
) {
    for mut text in ui_query.iter_mut() {
        let mut ui_text = String::new();
        
        // Tower health display
        ui_text.push_str("=== UPLINK TOWERS ===\n");
        for (tower, health) in tower_query.iter() {
            ui_text.push_str(&format!(
                "Team {:?}: {:.0}/{:.0} HP ({:.1}%)\n",
                tower.team,
                health.current,
                health.max,
                health.health_percentage() * 100.0
            ));
        }
        
        // Game status
        if game_state.game_ended {
            if let Some(winner) = game_state.winner {
                ui_text.push_str(&format!("\n🏆 VICTORY: Team {:?} Wins! 🏆", winner));
            }
        } else {
            ui_text.push_str("\n⚔️ Battle in Progress ⚔️");
        }
        
        text.sections[0].value = ui_text;
    }
}

#[derive(Component)]
pub struct ObjectiveUI;

pub fn spawn_objective_ui(mut commands: Commands) {
    commands.spawn((
        TextBundle::from_section(
            "Loading objective data...",
            TextStyle {
                font_size: 18.0,
                color: Color::WHITE,
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(120.0),
            left: Val::Px(10.0),
            ..default()
        }),
        ObjectiveUI,
    ));
}

// ===== DEBUG SYSTEMS =====

pub fn debug_explosion_hotkey_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut tower_query: Query<(Entity, &Transform, &UplinkTower, &mut Health), With<UplinkTower>>,
    droid_query: Query<(Entity, &Transform, &BattleDroid), With<BattleDroid>>,
    mut game_state: ResMut<GameState>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyE) {
        info!("🔥 DEBUG: Explosion hotkey pressed! Triggering tower destruction...");
        
        // Find Team B tower and destroy it
        for (tower_entity, tower_transform, tower, mut tower_health) in tower_query.iter_mut() {
            if tower.team == Team::B {
                info!("🔥 DEBUG: Destroying Team B tower for explosion test");
                
                // Set health to 0 to trigger destruction
                tower_health.current = 0.0;
                
                // Mark game as ended
                game_state.tower_destroyed(tower.team);
                
                // Find all friendly units within destruction radius
                let mut units_to_explode = Vec::new();
                for (droid_entity, droid_transform, droid) in droid_query.iter() {
                    if droid.team == tower.team {
                        let distance = tower_transform.translation.distance(droid_transform.translation);
                        if distance <= tower.destruction_radius {
                            units_to_explode.push(droid_entity);
                        }
                    }
                }
                
                // Add immediate explosions for testing (no delay)
                let explosion_count = units_to_explode.len();
                for (i, unit_entity) in units_to_explode.into_iter().enumerate() {
                    let delay = (i as f32) * 0.1; // Stagger explosions every 0.1 seconds
                    commands.entity(unit_entity).insert(PendingExplosion {
                        delay_timer: delay,
                        explosion_power: 1.5,
                    });
                }
                
                // Create massive explosion effect at tower location
                spawn_explosion_effect(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &mut images,
                    tower_transform.translation,
                    tower.destruction_radius,
                    4.0, // Very high intensity for debug
                    EXPLOSION_EFFECT_DURATION * 3.0, // Long duration for observation
                );
                
                // Also create some test explosions around the battlefield at ground level
                for i in 0..5 {
                    let test_pos = Vec3::new(
                        (i as f32 - 2.0) * 20.0,
                        0.0, // Ground level
                        50.0 + (i as f32 - 2.0) * 15.0, // Spread out in front of camera
                    );
                    spawn_explosion_effect(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        &mut images,
                        test_pos,
                        15.0, // Medium radius
                        2.0 + i as f32 * 0.5, // Varying intensity
                        EXPLOSION_EFFECT_DURATION * 2.0,
                    );
                }
                
                // Mark tower for destruction
                commands.entity(tower_entity).insert(PendingExplosion {
                    delay_timer: 0.5, // Half second delay
                    explosion_power: 5.0,
                });
                
                info!("🔥 DEBUG: Triggered {} unit explosions + 6 test explosions", explosion_count);
                break; // Only destroy one tower
            }
        }
    }
    
    // T key functionality removed - debug cubes no longer needed
    
    if keyboard_input.just_pressed(KeyCode::KeyR) {
        info!("🔴 DEBUG: Creating bright test material explosions to verify rendering...");
        
        // Create explosions with modified material for maximum visibility at ground level
        for i in 0..3 {
            let pos = Vec3::new((i as f32 - 1.0) * 50.0, 0.0, 60.0); // Ground level, spread out
            
            // Create explosion with maximum brightness and very simple settings
            spawn_explosion_effect(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut images,
                pos,
                40.0, // Large radius
                20.0, // Maximum intensity
                10.0, // Very long duration
            );
        }
        
        info!("🔴 DEBUG: Created 3 maximum brightness explosions");
    }
} 
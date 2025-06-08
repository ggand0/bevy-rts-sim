use bevy::prelude::*;
use rand::Rng;
use crate::types::*;
use crate::constants::*;

// Helper function to calculate proper laser orientation
pub fn calculate_laser_orientation(
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

pub fn volley_fire_system(
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
                    team: droid.team,
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

pub fn update_projectiles(
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

pub fn target_acquisition_system(
    time: Res<Time>,
    mut combat_query: Query<(Entity, &Transform, &BattleDroid, &mut CombatUnit)>,
    tower_query: Query<(Entity, &Transform, &UplinkTower), With<UplinkTower>>,
) {
    let delta_time = time.delta_seconds();
    
    // Collect all unit data first to avoid borrowing issues
    let all_units: Vec<(Entity, Vec3, Team)> = combat_query
        .iter()
        .map(|(entity, transform, droid, _)| (entity, transform.translation, droid.team))
        .collect();
    
    // Collect all tower data
    let all_towers: Vec<(Entity, Vec3, Team)> = tower_query
        .iter()
        .map(|(entity, transform, tower)| (entity, transform.translation, tower.team))
        .collect();
    
    for (entity, transform, droid, mut combat_unit) in combat_query.iter_mut() {
        // Update target scan timer
        combat_unit.target_scan_timer -= delta_time;
        
        if combat_unit.target_scan_timer <= 0.0 {
            combat_unit.target_scan_timer = TARGET_SCAN_INTERVAL;
            
            let mut closest_enemy: Option<Entity> = None;
            let mut closest_distance = f32::INFINITY;
            let mut target_priority = 0; // 0 = unit, 1 = tower
            
            // Check enemy towers first (higher priority targets)
            for &(tower_entity, tower_position, tower_team) in &all_towers {
                if tower_team != droid.team { // Enemy tower
                    let distance = transform.translation.distance(tower_position);
                    if distance <= TARGETING_RANGE * 1.5 { // Slightly longer range for towers
                        // Towers are high priority - prefer them over units
                        if target_priority < 1 || distance < closest_distance {
                            closest_distance = distance;
                            closest_enemy = Some(tower_entity);
                            target_priority = 1;
                        }
                    }
                }
            }
            
            // If no tower in range, check enemy units
            if target_priority == 0 {
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
            }
            
            combat_unit.current_target = closest_enemy;
        }
    }
}

pub fn auto_fire_system(
    time: Res<Time>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut combat_query: Query<(&Transform, &BattleDroid, &mut CombatUnit)>,
    target_query: Query<&Transform, With<BattleDroid>>,
    tower_target_query: Query<&Transform, With<UplinkTower>>,
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
                // Try to get target as either a unit or a tower
                let target_transform = target_query.get(target_entity)
                    .or_else(|_| tower_target_query.get(target_entity));
                
                if let Ok(target_transform) = target_transform {
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

pub fn collision_detection_system(
    mut commands: Commands,
    mut spatial_grid: ResMut<SpatialGrid>,
    mut squad_manager: ResMut<SquadManager>,
    laser_query: Query<(Entity, &Transform, &LaserProjectile)>,
    droid_query: Query<(Entity, &Transform, &BattleDroid, &SquadMember), Without<LaserProjectile>>,
) {
    // Clear and rebuild the spatial grid each frame
    spatial_grid.clear();
    
    // Populate grid with droids
    for (droid_entity, droid_transform, _, _) in droid_query.iter() {
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
            if let Ok((_, droid_transform, droid, _squad_member)) = droid_query.get(droid_entity) {
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
                    
                    // Handle squad casualty immediately (commander promotion, etc.)
                    squad_manager.remove_unit_from_squad(droid_entity);
                    
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
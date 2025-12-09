use bevy::prelude::*;
use crate::types::*;
use crate::constants::*;
use crate::terrain::TerrainHeightmap;
use crate::math_utils::ray_sphere_intersection;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;

/// Check if there's a clear line of sight between shooter and target
/// Returns true if the path is clear (no terrain blocking)
fn has_line_of_sight(
    shooter_pos: Vec3,
    target_pos: Vec3,
    heightmap: Option<&TerrainHeightmap>,
) -> bool {
    // If no heightmap, assume clear line of sight
    let Some(hm) = heightmap else { return true };

    // Sample terrain at multiple points along the line
    const NUM_SAMPLES: usize = 8;

    for i in 1..NUM_SAMPLES {
        let t = i as f32 / NUM_SAMPLES as f32;

        // Interpolate position along the line
        let sample_pos = shooter_pos.lerp(target_pos, t);

        // Get terrain height at this point
        let terrain_y = hm.sample_height(sample_pos.x, sample_pos.z);

        // Calculate the expected Y at this point along the straight line
        let line_y = shooter_pos.y.lerp(target_pos.y, t);

        // If terrain is above the line of sight (with small margin), line is blocked
        if terrain_y > line_y + 0.5 {
            return false;
        }
    }

    true
}

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

/// Initialize cached laser assets (materials and meshes) to avoid per-shot allocation
pub fn setup_laser_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let team_a_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 2.0, 0.0), // Green for Team A
        emissive: Color::srgb(0.0, 1.0, 0.0).into(),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        cull_mode: None,
        ..default()
    });

    let team_b_material = materials.add(StandardMaterial {
        base_color: Color::srgb(2.0, 0.0, 0.0), // Red for Team B
        emissive: Color::srgb(1.0, 0.0, 0.0).into(),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        cull_mode: None,
        ..default()
    });

    // Standard laser mesh
    let laser_mesh = meshes.add(Rectangle::new(LASER_WIDTH, LASER_LENGTH));

    // MG turret uses shorter bolts (60% length)
    let mg_laser_mesh = meshes.add(Rectangle::new(LASER_WIDTH, LASER_LENGTH * 0.6));

    // Hitscan tracer mesh (slightly larger for visibility)
    let hitscan_tracer_mesh = meshes.add(Rectangle::new(HITSCAN_TRACER_WIDTH, HITSCAN_TRACER_LENGTH));

    commands.insert_resource(LaserAssets {
        team_a_material,
        team_b_material,
        laser_mesh,
        mg_laser_mesh,
        hitscan_tracer_mesh,
    });
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
            bevy::asset::RenderAssetUsages::RENDER_WORLD,
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
        let camera_position = camera_query.single()
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
                Mesh3d(laser_mesh.clone()),
                MeshMaterial3d(laser_material),
                laser_transform,
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
        commands.spawn((
            AudioPlayer::new(sound),
            PlaybackSettings::DESPAWN,
        ));
        
        info!("Volley fire! {} lasers fired!", droid_query.iter().count());
    }
}

pub fn update_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut projectile_query: Query<(Entity, &mut Transform, &mut LaserProjectile)>,
    camera_query: Query<&Transform, (With<RtsCamera>, Without<LaserProjectile>)>,
    heightmap: Option<Res<TerrainHeightmap>>,
) {
    let delta_time = time.delta_secs();

    // Get camera position for billboarding
    let camera_transform = camera_query.single().ok();

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

        // Check terrain collision
        if let Some(ref hm) = heightmap {
            let terrain_y = hm.sample_height(transform.translation.x, transform.translation.z);
            if transform.translation.y < terrain_y {
                // Laser hit terrain - despawn it
                commands.entity(entity).despawn();
                continue;
            }
        }

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
    mut combat_query: Query<(Entity, &GlobalTransform, &BattleDroid, &mut CombatUnit)>,
    tower_query: Query<(Entity, &GlobalTransform, &UplinkTower), With<UplinkTower>>,
    turret_query: Query<(Entity, &GlobalTransform, &TurretBase), With<TurretBase>>,
    heightmap: Option<Res<TerrainHeightmap>>,
) {
    let delta_time = time.delta_secs();
    let hm = heightmap.as_ref().map(|h| h.as_ref());

    // Collect all unit data first to avoid borrowing issues
    // Use GlobalTransform to get world position (handles parent-child hierarchies like turrets)
    let all_units: Vec<(Entity, Vec3, Team)> = combat_query
        .iter()
        .map(|(entity, transform, droid, _)| (entity, transform.translation(), droid.team))
        .collect();

    // Collect all tower data
    let all_towers: Vec<(Entity, Vec3, Team)> = tower_query
        .iter()
        .map(|(entity, transform, tower)| (entity, transform.translation(), tower.team))
        .collect();

    // Collect all turret data
    let all_turrets: Vec<(Entity, Vec3, Team)> = turret_query
        .iter()
        .map(|(entity, transform, turret)| (entity, transform.translation(), turret.team))
        .collect();

    for (entity, transform, droid, mut combat_unit) in combat_query.iter_mut() {
        // Update target scan timer
        combat_unit.target_scan_timer -= delta_time;

        if combat_unit.target_scan_timer <= 0.0 {
            combat_unit.target_scan_timer = TARGET_SCAN_INTERVAL;

            let mut closest_enemy: Option<Entity> = None;
            let shooter_pos = transform.translation();

            // Check enemy units first (they're the threat)
            // Collect all enemies in range with their distances
            let mut enemies_in_range: Vec<(Entity, Vec3, f32)> = all_units.iter()
                .filter(|(target_entity, _, target_team)| {
                    *target_team != droid.team && *target_entity != entity
                })
                .filter_map(|&(target_entity, target_position, _)| {
                    let distance = shooter_pos.distance(target_position);
                    if distance <= TARGETING_RANGE {
                        Some((target_entity, target_position, distance))
                    } else {
                        None
                    }
                })
                .collect();

            // Sort by distance (closest first)
            enemies_in_range.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

            // Find the closest enemy with clear line of sight
            for (target_entity, target_position, _distance) in enemies_in_range {
                if has_line_of_sight(shooter_pos, target_position, hm) {
                    closest_enemy = Some(target_entity);
                    break;
                }
            }

            // If no enemy units in range, check turrets (high threat buildings)
            if closest_enemy.is_none() {
                let mut turrets_in_range: Vec<(Entity, Vec3, f32)> = all_turrets.iter()
                    .filter(|(_, _, turret_team)| *turret_team != droid.team)
                    .filter_map(|&(turret_entity, turret_position, _)| {
                        let distance = shooter_pos.distance(turret_position);
                        if distance <= TARGETING_RANGE {
                            Some((turret_entity, turret_position, distance))
                        } else {
                            None
                        }
                    })
                    .collect();

                turrets_in_range.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

                // Find the closest turret with clear line of sight
                for (turret_entity, turret_position, _distance) in turrets_in_range {
                    if has_line_of_sight(shooter_pos, turret_position, hm) {
                        closest_enemy = Some(turret_entity);
                        break;
                    }
                }
            }

            // If no turrets in range, check towers as last fallback
            if closest_enemy.is_none() {
                let mut towers_in_range: Vec<(Entity, Vec3, f32)> = all_towers.iter()
                    .filter(|(_, _, tower_team)| *tower_team != droid.team)
                    .filter_map(|&(tower_entity, tower_position, _)| {
                        let distance = shooter_pos.distance(tower_position);
                        if distance <= TARGETING_RANGE * 1.5 {
                            Some((tower_entity, tower_position, distance))
                        } else {
                            None
                        }
                    })
                    .collect();

                towers_in_range.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

                // Find the closest tower with clear line of sight
                for (tower_entity, tower_position, _distance) in towers_in_range {
                    if has_line_of_sight(shooter_pos, tower_position, hm) {
                        closest_enemy = Some(tower_entity);
                        break;
                    }
                }
            }

            combat_unit.current_target = closest_enemy;
        }
    }
}

/// Turret projectile fire system - turrets still use traditional projectiles
/// Infantry firing is now handled by hitscan_fire_system
pub fn auto_fire_system(
    time: Res<Time>,
    mut commands: Commands,
    laser_assets: Res<LaserAssets>,
    mut turret_query: Query<(&GlobalTransform, &Transform, &BattleDroid, &mut CombatUnit, &mut crate::types::TurretRotatingAssembly, Option<&mut crate::types::MgTurret>)>,
    target_query: Query<&GlobalTransform, With<BattleDroid>>,
    tower_target_query: Query<&GlobalTransform, With<UplinkTower>>,
    all_units_query: Query<(Entity, &GlobalTransform, &BattleDroid)>,
    all_towers_query: Query<(Entity, &GlobalTransform, &UplinkTower)>,
    camera_query: Query<&Transform, (With<RtsCamera>, Without<LaserProjectile>)>,
    audio_assets: Res<AudioAssets>,
    heightmap: Option<Res<TerrainHeightmap>>,
) {
    let delta_time = time.delta_secs();
    
    // Get camera position for initial orientation
    let camera_position = camera_query.single()
        .map(|cam_transform| cam_transform.translation)
        .unwrap_or(Vec3::new(0.0, 100.0, 100.0)); // Fallback position
    
    // Count shots fired this frame for audio throttling
    // NOTE: Infantry firing is now handled by hitscan_fire_system
    let mut shots_fired = 0;
    let mut mg_shots_fired = 0; // Separate counter for MG (prioritized)
    const MAX_AUDIO_PER_FRAME: usize = 5; // Limit concurrent audio to prevent spam
    const MAX_MG_AUDIO_PER_FRAME: usize = 3; // Prioritized limit for MG turret

    // Handle turret firing with barrel positions
    // Standard turret barrel positions
    let standard_barrel_positions = [
        Vec3::new(-1.8, 1.5, -6.0), // Left barrel muzzle
        Vec3::new(1.8, 1.5, -6.0),  // Right barrel muzzle
    ];
    
    // MG turret barrel position (single center barrel)
    let mg_barrel_positions = [
        Vec3::new(0.0, 2.0, -7.4),
    ];

    for (global_transform, local_transform, droid, mut combat_unit, mut turret, mut mg_turret_opt) in turret_query.iter_mut() {
        // Handle MG firing mode control
        let mut can_fire = true;
        if let Some(ref mut mg_turret) = mg_turret_opt {
            match mg_turret.firing_mode {
                crate::types::FiringMode::Burst => {
                    // Burst mode: fire fixed shots then cooldown
                    if mg_turret.cooldown_timer > 0.0 {
                        mg_turret.cooldown_timer -= delta_time;
                        can_fire = false;

                        // Reset burst counter when cooldown ends
                        if mg_turret.cooldown_timer <= 0.0 {
                            mg_turret.shots_in_burst = 0;
                        }
                    } else if mg_turret.shots_in_burst >= mg_turret.max_burst_shots {
                        // Start cooldown
                        mg_turret.cooldown_timer = mg_turret.cooldown_duration;
                        can_fire = false;
                    }
                }
                crate::types::FiringMode::Continuous => {
                    // Continuous mode: pause after max_burst_shots for cooling
                    if mg_turret.cooldown_timer > 0.0 {
                        mg_turret.cooldown_timer -= delta_time;
                        can_fire = false;

                        // Reset burst counter when cooldown ends
                        if mg_turret.cooldown_timer <= 0.0 {
                            mg_turret.shots_in_burst = 0;
                        }
                    } else if mg_turret.shots_in_burst >= mg_turret.max_burst_shots {
                        // Start cooldown after firing max shots
                        mg_turret.cooldown_timer = mg_turret.cooldown_duration;
                        can_fire = false;
                    }
                }
            }
        }

        // Update auto fire timer
        combat_unit.auto_fire_timer -= delta_time;

        if can_fire && combat_unit.auto_fire_timer <= 0.0 {
            // Handle target validation and rapid switching for MG turrets
            let is_continuous_mode = mg_turret_opt.as_ref()
                .map(|mg| mg.firing_mode == crate::types::FiringMode::Continuous)
                .unwrap_or(false);

            // Check if current target is dead
            if combat_unit.current_target.is_some() {
                if let Some(target_entity) = combat_unit.current_target {
                    let target_exists = target_query.get(target_entity).is_ok() ||
                                       tower_target_query.get(target_entity).is_ok();
                    if !target_exists {
                        combat_unit.current_target = None;
                    }
                }
            }

            // For continuous mode MG, immediately acquire new target if current is None
            if is_continuous_mode && combat_unit.current_target.is_none() {
                let shooter_pos = global_transform.translation();

                // Find closest enemy in range
                let mut closest_enemy: Option<(Entity, f32)> = None;

                // Check all enemy units
                for (target_entity, target_transform, target_droid) in all_units_query.iter() {
                    if target_droid.team != droid.team {
                        let distance = shooter_pos.distance(target_transform.translation());
                        if distance <= TARGETING_RANGE {
                            if let Some((_, min_dist)) = closest_enemy {
                                if distance < min_dist {
                                    closest_enemy = Some((target_entity, distance));
                                }
                            } else {
                                closest_enemy = Some((target_entity, distance));
                            }
                        }
                    }
                }

                // If no units found, check enemy towers
                if closest_enemy.is_none() {
                    for (target_entity, target_transform, target_tower) in all_towers_query.iter() {
                        if target_tower.team != droid.team {
                            let distance = shooter_pos.distance(target_transform.translation());
                            if distance <= TARGETING_RANGE {
                                if let Some((_, min_dist)) = closest_enemy {
                                    if distance < min_dist {
                                        closest_enemy = Some((target_entity, distance));
                                    }
                                } else {
                                    closest_enemy = Some((target_entity, distance));
                                }
                            }
                        }
                    }
                }

                // Assign new target immediately
                combat_unit.current_target = closest_enemy.map(|(entity, _)| entity);
            }

            // Get or keep target
            if let Some(target_entity) = combat_unit.current_target {
                // Double-check target still exists (critical for rapid-fire MG to avoid wasting shots)
                let target_transform = target_query.get(target_entity)
                    .or_else(|_| tower_target_query.get(target_entity));

                if let Ok(target_transform) = target_transform {
                    let is_mg = mg_turret_opt.is_some();

                    // Check line of sight before firing (critical for Map 2 terrain blocking)
                    let shooter_pos = global_transform.translation();
                    let target_pos = target_transform.translation();
                    if !has_line_of_sight(shooter_pos, target_pos, heightmap.as_deref()) {
                        // Clear target if line of sight is blocked
                        combat_unit.current_target = None;
                        continue;
                    }

                    // Determine barrel configuration, fire rate, and laser speed
                    let (barrel_positions, fire_interval, laser_speed) = if is_mg {
                        (&mg_barrel_positions[..], 0.05, LASER_SPEED * 3.0) // MG: 20 shots/sec, 3x speed
                    } else {
                        (&standard_barrel_positions[..], AUTO_FIRE_INTERVAL, LASER_SPEED)
                    };

                    // Reset timer
                    combat_unit.auto_fire_timer = fire_interval;

                    // Use cached laser material (turrets are Team A = green)
                    let laser_material = laser_assets.team_a_material.clone();

                    // Use appropriate cached mesh based on turret type
                    let laser_mesh = if is_mg {
                        laser_assets.mg_laser_mesh.clone()
                    } else {
                        laser_assets.laser_mesh.clone()
                    };

                    // Get current barrel position in local space
                    let local_barrel_pos = barrel_positions[turret.current_barrel_index % barrel_positions.len()];

                    // Transform barrel position to world space using turret's rotation
                    let world_barrel_offset = local_transform.rotation * local_barrel_pos;
                    let firing_pos = global_transform.translation() + world_barrel_offset;

                    // Aim at center of collision sphere (ground level, where collision detection happens)
                    let target_pos = target_transform.translation();
                    let direction = (target_pos - firing_pos).normalize();
                    let velocity = direction * laser_speed;

                    // Calculate proper initial orientation
                    let laser_rotation = calculate_laser_orientation(velocity, firing_pos, camera_position);
                    let laser_transform = Transform::from_translation(firing_pos)
                        .with_rotation(laser_rotation);

                    // Spawn laser
                    commands.spawn((
                        Mesh3d(laser_mesh),
                        MeshMaterial3d(laser_material),
                        laser_transform,
                        LaserProjectile {
                            velocity,
                            lifetime: LASER_LIFETIME,
                            team: droid.team,
                        },
                    ));

                    // Advance to next barrel
                    turret.current_barrel_index = (turret.current_barrel_index + 1) % barrel_positions.len();

                    // Increment MG burst counter (both modes track shots for pause timing)
                    if let Some(ref mut mg_turret) = mg_turret_opt {
                        mg_turret.shots_in_burst += 1;
                    }

                    // Play sound per bullet (MG sounds are prioritized with separate counter)
                    if is_mg {
                        mg_shots_fired += 1;
                        if mg_shots_fired <= MAX_MG_AUDIO_PER_FRAME {
                            // MG uses single bullet sound with distance-based volume
                            let turret_pos = global_transform.translation();
                            let distance = turret_pos.distance(camera_position);
                            let volume = proximity_volume(distance, crate::constants::VOLUME_MG_TURRET);
                            trace!("MG turret at {:?}, camera at {:?}, distance: {:.1}, volume: {:.3}",
                                turret_pos, camera_position, distance, volume);

                            commands.spawn((
                                AudioPlayer::new(audio_assets.mg_sound.clone()),
                                PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(volume)),
                            ));
                        }
                    } else {
                        shots_fired += 1;
                        if shots_fired <= MAX_AUDIO_PER_FRAME {
                            // Standard turret uses random laser sound with proximity-based volume
                            let mut rng = rand::thread_rng();
                            let sound = audio_assets.get_random_laser_sound(&mut rng);

                            let turret_pos = global_transform.translation();
                            let distance = turret_pos.distance(camera_position);
                            let volume = proximity_volume(distance, crate::constants::VOLUME_HEAVY_TURRET);
                            trace!("Heavy turret at {:?}, camera at {:?}, distance: {:.1}, volume: {:.3}",
                                turret_pos, camera_position, distance, volume);

                            commands.spawn((
                                AudioPlayer::new(sound),
                                PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(volume)),
                            ));
                        }
                    }
                }
            }
        }
    }
}

/// Hitscan fire system for infantry - instant damage with visual tracer
/// Damage is calculated immediately via raycast, tracer is purely cosmetic
pub fn hitscan_fire_system(
    time: Res<Time>,
    mut commands: Commands,
    laser_assets: Res<LaserAssets>,
    spatial_grid: Res<SpatialGrid>,
    shield_config: Res<crate::shield::ShieldConfig>,
    mut squad_manager: ResMut<SquadManager>,
    mut combat_query: Query<
        (&GlobalTransform, &BattleDroid, &mut CombatUnit),
        (Without<crate::types::TurretRotatingAssembly>, Without<HitscanTracer>)
    >,
    // all_droids_query combines target lookup + hitscan collision
    all_droids_query: Query<(Entity, &GlobalTransform, &BattleDroid), Without<HitscanTracer>>,
    tower_target_query: Query<&GlobalTransform, With<UplinkTower>>,
    mut turret_query: Query<(&GlobalTransform, &mut Health), With<crate::types::TurretBase>>,
    mut tower_health_query: Query<&mut Health, (With<UplinkTower>, Without<crate::types::TurretBase>)>,
    turret_assembly_query: Query<&ChildOf, With<crate::types::TurretRotatingAssembly>>,
    mut shield_query: Query<&mut crate::shield::Shield>,
    camera_query: Query<&Transform, (With<RtsCamera>, Without<HitscanTracer>)>,
    audio_assets: Res<AudioAssets>,
    heightmap: Option<Res<TerrainHeightmap>>,
) {
    let delta_time = time.delta_secs();

    // Get camera position for tracer orientation
    let camera_position = camera_query.single()
        .map(|cam_transform| cam_transform.translation)
        .unwrap_or(Vec3::new(0.0, 100.0, 100.0));

    // Audio throttling
    let mut shots_fired = 0;
    const MAX_AUDIO_PER_FRAME: usize = 5;

    for (droid_transform, droid, mut combat_unit) in combat_query.iter_mut() {
        // Update auto fire timer
        combat_unit.auto_fire_timer -= delta_time;

        if combat_unit.auto_fire_timer <= 0.0 && combat_unit.current_target.is_some() {
            if let Some(target_entity) = combat_unit.current_target {
                // Try to get target position (unit, tower, or turret)
                let target_pos_opt = all_droids_query.get(target_entity)
                    .map(|(_, t, _)| t.translation())
                    .or_else(|_| tower_target_query.get(target_entity).map(|t| t.translation()))
                    .or_else(|_| turret_query.get(target_entity).map(|(t, _)| t.translation()))
                    .ok();

                if let Some(target_pos) = target_pos_opt {
                    // Reset timer
                    combat_unit.auto_fire_timer = AUTO_FIRE_INTERVAL;

                    let firing_pos = droid_transform.translation() + Vec3::new(0.0, 0.8, 0.0);
                    let direction = (target_pos - firing_pos).normalize();

                    // Check line of sight
                    if !has_line_of_sight(firing_pos, target_pos, heightmap.as_deref()) {
                        combat_unit.current_target = None;
                        continue;
                    }

                    let current_time = time.elapsed_secs();
                    let ray_length = firing_pos.distance(target_pos);

                    // === CHECK SHIELD INTERSECTION FIRST ===
                    // Find closest enemy shield along the ray and damage it
                    let mut shield_hit_pos: Option<Vec3> = None;

                    for mut shield in shield_query.iter_mut() {
                        // Skip friendly shields
                        if shield.team == droid.team {
                            continue;
                        }

                        // Ray-sphere intersection test for shield
                        if let Some((hit_dist, hit_pos)) = ray_sphere_intersection(
                            firing_pos, direction, shield.center, shield.radius
                        ) {
                            // Only count if hit is between shooter and target
                            if hit_dist > 0.0 && hit_dist < ray_length {
                                // This shield blocks the shot - damage it
                                let old_hp = shield.current_hp;
                                shield.take_damage(
                                    shield_config.laser_damage,
                                    current_time,
                                    shield_config.impact_flash_duration
                                );
                                trace!("Hitscan shield hit: hp {} -> {}", old_hp, shield.current_hp);
                                // Add ripple effect occasionally
                                if rand::random::<f32>() < 0.25 {
                                    shield.add_ripple(hit_pos, current_time);
                                }
                                shield_hit_pos = Some(hit_pos);
                                break; // Only hit one shield
                            }
                        }
                    }

                    // If we hit a shield, the ray stops there
                    let impact_pos = if let Some(hit_pos) = shield_hit_pos {
                        hit_pos
                    } else {
                        // === INSTANT HIT DETECTION (hitscan) ===
                        // No shield in the way - raycast from firing position to target
                        let hit_result = perform_hitscan(
                            firing_pos,
                            direction,
                            droid.team,
                            &spatial_grid,
                            &all_droids_query,
                            target_pos,
                        );

                        match hit_result {
                            HitscanResult::HitUnit(hit_entity, hit_pos) => {
                                // Despawn hit unit (try_despawn to avoid double-despawn warnings)
                                commands.entity(hit_entity).try_despawn();
                                squad_manager.remove_unit_from_squad(hit_entity);
                                hit_pos
                            }
                            HitscanResult::HitTower(hit_pos) => {
                                // Apply damage to buildings (turrets or towers)
                                // Try turret base directly
                                if let Ok((_, mut turret_health)) = turret_query.get_mut(target_entity) {
                                    turret_health.damage(HITSCAN_DAMAGE);
                                }
                                // Try tower if turret query failed
                                else if let Ok(mut tower_health) = tower_health_query.get_mut(target_entity) {
                                    tower_health.damage(HITSCAN_DAMAGE);
                                }
                                // Target may be turret assembly (child entity) - damage parent
                                else if let Ok(child_of) = turret_assembly_query.get(target_entity) {
                                    let parent_entity = child_of.parent();
                                    if let Ok((_, mut turret_health)) = turret_query.get_mut(parent_entity) {
                                        turret_health.damage(HITSCAN_DAMAGE);
                                    }
                                }
                                hit_pos
                            }
                            HitscanResult::Miss(end_pos) => end_pos,
                        }
                    };

                    // === SPAWN VISUAL TRACER ===
                    let laser_material = match droid.team {
                        Team::A => laser_assets.team_a_material.clone(),
                        Team::B => laser_assets.team_b_material.clone(),
                    };

                    // Calculate initial orientation for the tracer
                    let velocity = direction * HITSCAN_TRACER_SPEED;
                    let tracer_rotation = calculate_laser_orientation(velocity, firing_pos, camera_position);

                    commands.spawn((
                        Mesh3d(laser_assets.hitscan_tracer_mesh.clone()),
                        MeshMaterial3d(laser_material),
                        Transform::from_translation(firing_pos).with_rotation(tracer_rotation),
                        HitscanTracer {
                            start_pos: firing_pos,
                            end_pos: impact_pos,
                            progress: 0.0,
                            speed: HITSCAN_TRACER_SPEED,
                            team: droid.team,
                        },
                    ));

                    // Play sound with proximity-based volume
                    shots_fired += 1;
                    if shots_fired <= MAX_AUDIO_PER_FRAME {
                        let mut rng = rand::thread_rng();
                        let sound = audio_assets.get_random_laser_sound(&mut rng);
                        let distance = droid_transform.translation().distance(camera_position);
                        let volume = proximity_volume(distance, 0.3);

                        commands.spawn((
                            AudioPlayer::new(sound),
                            PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(volume)),
                        ));
                    }
                }
            }
        }
    }
}

/// Result of a hitscan raycast
#[allow(dead_code)]
enum HitscanResult {
    HitUnit(Entity, Vec3),  // Hit a unit, returns entity and position
    HitTower(Vec3),         // Hit a tower, returns position
    Miss(Vec3),             // Missed, returns end position (target pos)
}

/// Perform instant raycast hit detection
fn perform_hitscan(
    start: Vec3,
    direction: Vec3,
    shooter_team: Team,
    spatial_grid: &SpatialGrid,
    droid_query: &Query<(Entity, &GlobalTransform, &BattleDroid), Without<HitscanTracer>>,
    target_pos: Vec3,
) -> HitscanResult {
    let ray_length = start.distance(target_pos);

    // Check all units along the ray path using spatial grid
    // Sample points along the ray and check nearby units
    let num_samples = (ray_length / GRID_CELL_SIZE).ceil() as usize + 1;

    let mut closest_hit: Option<(Entity, Vec3, f32)> = None;

    for i in 0..=num_samples {
        let t = i as f32 / num_samples as f32;
        let sample_pos = start.lerp(target_pos, t);

        // Get nearby droids at this sample point
        let nearby = spatial_grid.get_nearby_droids(sample_pos);

        for &entity in &nearby {
            if let Ok((_, droid_transform, droid)) = droid_query.get(entity) {
                // Skip friendly fire
                if droid.team == shooter_team {
                    continue;
                }

                let droid_pos = droid_transform.translation();

                // Ray-sphere intersection test
                // Find closest point on ray to sphere center
                let to_droid = droid_pos - start;
                let projection = to_droid.dot(direction);

                // Skip if behind the shooter
                if projection < 0.0 {
                    continue;
                }

                // Skip if beyond target
                if projection > ray_length {
                    continue;
                }

                let closest_point_on_ray = start + direction * projection;
                let distance_to_ray = closest_point_on_ray.distance(droid_pos);

                // Check if ray passes through unit's collision sphere
                if distance_to_ray <= COLLISION_RADIUS {
                    // Calculate actual hit point (entry point of sphere)
                    let hit_dist = projection - (COLLISION_RADIUS * COLLISION_RADIUS - distance_to_ray * distance_to_ray).sqrt();

                    if closest_hit.is_none() || hit_dist < closest_hit.unwrap().2 {
                        let hit_pos = start + direction * hit_dist;
                        closest_hit = Some((entity, hit_pos, hit_dist));
                    }
                }
            }
        }
    }

    if let Some((entity, hit_pos, _)) = closest_hit {
        HitscanResult::HitUnit(entity, hit_pos)
    } else {
        // Check if we hit the intended target (could be a tower)
        HitscanResult::HitTower(target_pos)
    }
}

/// Update hitscan tracers - move them along their path and despawn when done
pub fn update_hitscan_tracers(
    time: Res<Time>,
    mut commands: Commands,
    mut tracer_query: Query<(Entity, &mut Transform, &mut HitscanTracer)>,
    camera_query: Query<&Transform, (With<RtsCamera>, Without<HitscanTracer>)>,
) {
    let delta_time = time.delta_secs();
    let camera_position = camera_query.single()
        .map(|t| t.translation)
        .unwrap_or(Vec3::new(0.0, 100.0, 100.0));

    for (entity, mut transform, mut tracer) in tracer_query.iter_mut() {
        // Calculate total distance
        let total_distance = tracer.start_pos.distance(tracer.end_pos);

        // Update progress based on speed
        let progress_delta = (tracer.speed * delta_time) / total_distance.max(0.001);
        tracer.progress += progress_delta;

        if tracer.progress >= 1.0 {
            // Tracer reached end - despawn
            commands.entity(entity).despawn();
        } else {
            // Update position along path
            let current_pos = tracer.start_pos.lerp(tracer.end_pos, tracer.progress);
            transform.translation = current_pos;

            // Update rotation to face camera while aligned with travel direction
            let direction = (tracer.end_pos - tracer.start_pos).normalize();
            let velocity = direction * tracer.speed;
            transform.rotation = calculate_laser_orientation(velocity, current_pos, camera_position);
        }
    }
}

pub fn collision_detection_system(
    mut commands: Commands,
    mut spatial_grid: ResMut<SpatialGrid>,
    mut squad_manager: ResMut<SquadManager>,
    laser_query: Query<(Entity, &Transform, &LaserProjectile)>,
    droid_query: Query<(Entity, &Transform, &BattleDroid, &SquadMember), Without<LaserProjectile>>,
    building_query: Query<(Entity, &GlobalTransform, &crate::types::BuildingCollider)>,
    mut turret_health_query: Query<&mut crate::types::Health, With<crate::types::TurretBase>>,
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

        // Check building collisions first (buildings block lasers)
        // Use distance_squared to avoid sqrt overhead
        let mut hit_building = false;
        for (building_entity, building_transform, building_collider) in building_query.iter() {
            let distance_sq = laser_transform.translation.distance_squared(building_transform.translation());
            let radius_sq = building_collider.radius * building_collider.radius;
            if distance_sq <= radius_sq {
                // Hit building! Mark laser for despawn (but not the building)
                entities_to_despawn.insert(laser_entity);
                hit_building = true;

                // Apply damage to turrets if hit by enemy laser
                if let Ok(mut turret_health) = turret_health_query.get_mut(building_entity) {
                    // Only enemy lasers damage turrets (turrets are Team::A, enemies are Team::B)
                    if laser.team == crate::types::Team::B {
                        turret_health.damage(25.0); // Same damage as tower hits
                    }
                }
                break;
            }
        }

        // If laser hit a building, skip unit collision checks
        if hit_building {
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

                // Simple sphere collision detection using distance_squared to avoid sqrt
                const COLLISION_RADIUS_SQ: f32 = COLLISION_RADIUS * COLLISION_RADIUS;
                let distance_sq = laser_transform.translation.distance_squared(droid_transform.translation);
                if distance_sq <= COLLISION_RADIUS_SQ {
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
    
    // Despawn all marked entities (try_despawn to avoid double-despawn warnings with hitscan)
    for entity in entities_to_despawn {
        commands.entity(entity).try_despawn();
    }
}

/// Turret rotation system - smoothly rotates turret assembly to face current target
pub fn turret_rotation_system(
    time: Res<Time>,
    mut turret_query: Query<(&mut Transform, &GlobalTransform, &CombatUnit, Option<&crate::types::MgTurret>), With<crate::types::TurretRotatingAssembly>>,
    target_query: Query<&GlobalTransform, (With<BattleDroid>, Without<crate::types::TurretRotatingAssembly>)>,
) {
    for (mut turret_transform, turret_global_transform, combat_unit, mg_turret) in turret_query.iter_mut() {
        if let Some(target_entity) = combat_unit.current_target {
            if let Ok(target_global_transform) = target_query.get(target_entity) {
                // Calculate direction to target (horizontal plane only)
                // Use GlobalTransform to get world positions for direction calculation
                let turret_pos = turret_global_transform.translation();
                let target_pos = target_global_transform.translation();

                // Flatten target position to horizontal plane (keep Y at turret's level)
                let target_pos_flat = Vec3::new(target_pos.x, turret_pos.y, target_pos.z);

                // Create target rotation using Transform's from_translation + looking_at
                // This ensures the turret's -Z axis points at the target
                let target_rotation = Transform::from_translation(turret_pos)
                    .looking_at(target_pos_flat, Vec3::Y)
                    .rotation;

                // Smooth rotation interpolation
                let rotation_speed = if mg_turret.is_some() { 5.0 } else { 3.0 }; // Faster for MG
                turret_transform.rotation = turret_transform.rotation.slerp(
                    target_rotation,
                    rotation_speed * time.delta_secs()
                );
            }
        }
    }
}

// Marker component for debug spheres
#[derive(Component)]
pub struct DebugCollisionSphere;

/// Create a wireframe sphere mesh for debug visualization
fn create_debug_sphere_mesh(radius: f32, segments: usize) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default());
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Create sphere vertices using latitude/longitude
    let rings = segments;
    let sectors = segments * 2;

    for ring in 0..=rings {
        let theta = (ring as f32 / rings as f32) * std::f32::consts::PI;
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for sector in 0..=sectors {
            let phi = (sector as f32 / sectors as f32) * 2.0 * std::f32::consts::PI;
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();

            let x = sin_theta * cos_phi * radius;
            let y = cos_theta * radius;
            let z = sin_theta * sin_phi * radius;

            vertices.push([x, y, z]);
        }
    }

    // Create line indices for latitude circles
    for ring in 0..rings {
        for sector in 0..sectors {
            let current = ring * (sectors + 1) + sector;
            let next = current + 1;

            indices.push(current as u32);
            indices.push(next as u32);
        }
    }

    // Create line indices for longitude circles
    for sector in 0..=sectors {
        for ring in 0..rings {
            let current = ring * (sectors + 1) + sector;
            let below = (ring + 1) * (sectors + 1) + sector;

            indices.push(current as u32);
            indices.push(below as u32);
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// System to visualize collision spheres for units (toggleable with C key when debug mode active)
pub fn visualize_collision_spheres_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    droid_query: Query<(Entity, &Transform), (With<BattleDroid>, Without<DebugCollisionSphere>)>,
    building_query: Query<(Entity, &GlobalTransform, &crate::types::BuildingCollider), Without<DebugCollisionSphere>>,
    existing_spheres: Query<Entity, With<DebugCollisionSphere>>,
    debug_mode: Res<crate::objective::ExplosionDebugMode>,
) {
    // Always remove existing debug spheres
    for entity in existing_spheres.iter() {
        commands.entity(entity).despawn();
    }

    // Only create new spheres if visualization is enabled
    if !debug_mode.show_collision_spheres {
        return;
    }

    // Create sphere mesh (reuse for all visualizations)
    let unit_sphere_mesh = meshes.add(create_debug_sphere_mesh(COLLISION_RADIUS, 12));

    // Semi-transparent green material for unit collision spheres
    let unit_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.0, 1.0, 0.0, 0.3),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    });

    // Visualize unit collision spheres
    for (_entity, transform) in droid_query.iter() {
        commands.spawn((
            Mesh3d(unit_sphere_mesh.clone()),
            MeshMaterial3d(unit_material.clone()),
            Transform::from_translation(transform.translation),
            DebugCollisionSphere,
        ));
    }

    // Visualize building collision spheres (different color and size)
    let building_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.5, 0.0, 0.3),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    });

    for (_entity, global_transform, collider) in building_query.iter() {
        let building_sphere_mesh = meshes.add(create_debug_sphere_mesh(collider.radius, 16));
        commands.spawn((
            Mesh3d(building_sphere_mesh),
            MeshMaterial3d(building_material.clone()),
            Transform::from_translation(global_transform.translation()),
            DebugCollisionSphere,
        ));
    }
} 
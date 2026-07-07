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

/// Calculate hit probability based on shooter/target states and positions
/// Returns a value between ACCURACY_MIN and ACCURACY_MAX
pub fn calculate_hit_chance(
    base_accuracy: f32,        // 0.70 for infantry, 0.80 for turrets
    shooter_pos: Vec3,
    target_pos: Vec3,
    shooter_stationary: bool,  // Bonus if true (not for turrets, they're always stationary)
    target_stationary: bool,   // Penalty if false (target is moving)
) -> f32 {
    let mut accuracy = base_accuracy;

    // Stationary shooter bonus (only for infantry - turrets are always stationary and already have higher base)
    if shooter_stationary && base_accuracy < 0.75 {
        accuracy += ACCURACY_STATIONARY_BONUS;
    }

    // High ground bonus (shooter 3+ units higher than target)
    if shooter_pos.y > target_pos.y + HIGH_GROUND_HEIGHT_THRESHOLD {
        accuracy += ACCURACY_HIGH_GROUND_BONUS;
    }

    // Target moving penalty
    if !target_stationary {
        accuracy -= ACCURACY_TARGET_MOVING_PENALTY;
    }

    // Range falloff - segment-based interpolation
    let distance = shooter_pos.distance(target_pos);
    let range_penalty = calculate_range_penalty(distance);
    accuracy -= range_penalty;

    accuracy.clamp(ACCURACY_MIN, ACCURACY_MAX)
}

/// Pitch angle (radians) from the MG barrel pivot to a target, clamped to the
/// barrel's articulation range. Used by BOTH the rotation system (visual barrel
/// angle) and the firing system (muzzle position) so they can never diverge.
fn mg_barrel_pitch(pivot_world: Vec3, target_pos: Vec3) -> f32 {
    let to_target = target_pos - pivot_world;
    let horizontal_dist = Vec2::new(to_target.x, to_target.z).length();
    to_target.y.atan2(horizontal_dist).clamp(MG_BARREL_PITCH_MIN, MG_BARREL_PITCH_MAX)
}

/// Calculate range penalty using segment-based interpolation
/// Returns penalty as a positive value (0.0 to RANGE_SEGMENT_2_PENALTY)
pub fn calculate_range_penalty(distance: f32) -> f32 {
    if distance <= RANGE_SEGMENT_0_END {
        // Segment 0: no penalty
        RANGE_SEGMENT_0_PENALTY
    } else if distance <= RANGE_SEGMENT_1_END {
        // Segment 1: interpolate from 0 to segment 1 penalty
        let t = (distance - RANGE_SEGMENT_0_END) / (RANGE_SEGMENT_1_END - RANGE_SEGMENT_0_END);
        RANGE_SEGMENT_0_PENALTY + t * (RANGE_SEGMENT_1_PENALTY - RANGE_SEGMENT_0_PENALTY)
    } else if distance <= RANGE_SEGMENT_2_END {
        // Segment 2: interpolate from segment 1 to segment 2 penalty
        let t = (distance - RANGE_SEGMENT_1_END) / (RANGE_SEGMENT_2_END - RANGE_SEGMENT_1_END);
        RANGE_SEGMENT_1_PENALTY + t * (RANGE_SEGMENT_2_PENALTY - RANGE_SEGMENT_1_PENALTY)
    } else {
        // Beyond max range: cap at segment 2 penalty
        RANGE_SEGMENT_2_PENALTY
    }
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

    // MG turret uses shorter bolts (60% length)
    let mg_laser_mesh = meshes.add(Rectangle::new(LASER_WIDTH, LASER_LENGTH * 0.6));

    // Hitscan tracer mesh (slightly larger for visibility)
    let hitscan_tracer_mesh = meshes.add(Rectangle::new(HITSCAN_TRACER_WIDTH, HITSCAN_TRACER_LENGTH));

    commands.insert_resource(LaserAssets {
        team_a_material,
        team_b_material,
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
                    origin: firing_pos,
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
    mut combat_query: Query<(Entity, &GlobalTransform, &BattleDroid, &mut CombatUnit), Without<crate::types::TurretRotatingAssembly>>,
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
        (&GlobalTransform, &BattleDroid, &mut CombatUnit, &MovementTracker),
        (Without<crate::types::TurretRotatingAssembly>, Without<HitscanTracer>, Without<KnockbackState>, Without<RagdollDeath>)
    >,
    // all_droids_query combines target lookup + hitscan collision + movement tracking
    all_droids_query: Query<(Entity, &GlobalTransform, &BattleDroid, &MovementTracker), Without<HitscanTracer>>,
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

    for (droid_transform, droid, mut combat_unit, shooter_tracker) in combat_query.iter_mut() {
        // Update auto fire timer
        combat_unit.auto_fire_timer -= delta_time;

        if combat_unit.auto_fire_timer <= 0.0 && combat_unit.current_target.is_some() {
            if let Some(target_entity) = combat_unit.current_target {
                // Try to get target position and movement state (unit, tower, or turret)
                // Units have MovementTracker, buildings don't (treat as stationary)
                let target_info_opt: Option<(Vec3, bool)> = all_droids_query.get(target_entity)
                    .map(|(_, t, _, tracker)| (t.translation(), tracker.is_stationary))
                    .or_else(|_| tower_target_query.get(target_entity).map(|t| (t.translation(), true))) // Towers are stationary
                    .or_else(|_| turret_query.get(target_entity).map(|(t, _)| (t.translation(), true))) // Turrets are stationary
                    .ok();

                let target_pos_opt = target_info_opt.map(|(pos, _)| pos);
                let target_stationary = target_info_opt.map(|(_, stationary)| stationary).unwrap_or(true);

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

                    // Successfully firing - reset blocked timer (stuck prevention)
                    combat_unit.blocked_timer = 0.0;

                    // === ACCURACY CHECK ===
                    let hit_chance = calculate_hit_chance(
                        INFANTRY_BASE_ACCURACY,
                        firing_pos,
                        target_pos,
                        shooter_tracker.is_stationary,
                        target_stationary,
                    );
                    let hit_success = rand::random::<f32>() < hit_chance;

                    let current_time = time.elapsed_secs();
                    let ray_length = firing_pos.distance(target_pos);

                    // === DETERMINE IMPACT POSITION (depends on hit_success) ===
                    let impact_pos = if !hit_success {
                        // MISS - tracer goes past target (no damage)
                        // Add slight random offset for visual variety
                        let miss_offset = Vec3::new(
                            (rand::random::<f32>() - 0.5) * 4.0,
                            (rand::random::<f32>() - 0.5) * 4.0,
                            (rand::random::<f32>() - 0.5) * 4.0,
                        );
                        target_pos + miss_offset
                    } else {
                        // HIT - check shields first, then target
                        // === CHECK SHIELD INTERSECTION FIRST ===
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
                        if let Some(hit_pos) = shield_hit_pos {
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
                            crate::types::GunfireAudio { volume },
                        ));
                    }
                }
            }
        }
    }
}

/// Turret hitscan fire system - turrets use instant hitscan with visual tracers
/// Similar to infantry hitscan but with turret-specific mechanics (barrel positions, burst modes)
pub fn turret_hitscan_fire_system(
    time: Res<Time>,
    mut commands: Commands,
    laser_assets: Res<LaserAssets>,
    spatial_grid: Res<SpatialGrid>,
    shield_config: Res<crate::shield::ShieldConfig>,
    mut squad_manager: ResMut<SquadManager>,
    mut turret_query: Query<(
        Entity,
        &GlobalTransform,
        &Transform,
        &BattleDroid,
        &mut CombatUnit,
        &mut crate::types::TurretRotatingAssembly,
        Option<&mut crate::types::MgTurret>,
    )>,
    // For target lookup, validation, and hitscan collision
    all_droids_query: Query<(Entity, &GlobalTransform, &BattleDroid, &MovementTracker), Without<HitscanTracer>>,
    all_towers_query: Query<(Entity, &GlobalTransform, &UplinkTower)>,
    // For applying damage to buildings
    mut turret_health_query: Query<(&GlobalTransform, &mut Health), With<crate::types::TurretBase>>,
    mut tower_health_query: Query<&mut Health, (With<UplinkTower>, Without<crate::types::TurretBase>)>,
    turret_assembly_query: Query<&ChildOf, With<crate::types::TurretRotatingAssembly>>,
    // For shield intersection
    mut shield_query: Query<&mut crate::shield::Shield>,
    // For tracer orientation
    camera_query: Query<&Transform, (With<RtsCamera>, Without<HitscanTracer>)>,
    audio_assets: Res<AudioAssets>,
    heightmap: Option<Res<TerrainHeightmap>>,
) {
    let delta_time = time.delta_secs();

    // Get camera position for tracer orientation and audio distance
    let camera_position = camera_query.single()
        .map(|cam_transform| cam_transform.translation)
        .unwrap_or(Vec3::new(0.0, 100.0, 100.0));

    // Audio throttling for heavy turret (MG uses per-burst audio instead)
    let mut shots_fired = 0;
    const MAX_AUDIO_PER_FRAME: usize = 5;

    // Barrel positions
    let standard_barrel_positions = [
        Vec3::new(-1.8, 1.5, -6.0), // Left barrel
        Vec3::new(1.8, 1.5, -6.0),  // Right barrel
    ];
    // Note: MG barrel position is now computed dynamically with pitch rotation
    // See the firing_pos calculation for MG turrets below

    // MG turrets currently mid-burst (last frame's state), for loudness normalization:
    // concurrent clips sum acoustically, so each clip is scaled by 1/sqrt(count) to keep
    // perceived MG loudness constant whether 1 or 5 turrets are firing
    let active_mg_bursts = turret_query.iter()
        .filter(|(_, _, _, _, combat_unit, _, mg_turret_opt)| {
            mg_turret_opt.as_ref()
                .map(|mg| combat_unit.current_target.is_some()
                    && mg.cooldown_timer <= 0.0
                    && mg.shots_in_burst > 0)
                .unwrap_or(false)
        })
        .count()
        .max(1);
    let mg_burst_volume_scale = 1.0 / (active_mg_bursts as f32).sqrt();

    for (turret_entity, global_transform, local_transform, droid, mut combat_unit, mut turret, mut mg_turret_opt) in turret_query.iter_mut() {
        // === MG FIRING MODE CONTROL ===
        let mut can_fire = true;
        if let Some(ref mut mg_turret) = mg_turret_opt {
            if mg_turret.cooldown_timer > 0.0 {
                mg_turret.cooldown_timer -= delta_time;
                can_fire = false;
                if mg_turret.cooldown_timer <= 0.0 {
                    mg_turret.shots_in_burst = 0;
                }
            } else if mg_turret.shots_in_burst >= mg_turret.max_burst_shots {
                mg_turret.cooldown_timer = mg_turret.cooldown_duration;
                can_fire = false;
            }
        }

        // Update fire timer
        combat_unit.auto_fire_timer -= delta_time;

        if can_fire && combat_unit.auto_fire_timer <= 0.0 {
            // === TARGET VALIDATION & RAPID SWITCHING (MG) ===
            let is_continuous_mode = mg_turret_opt.as_ref()
                .map(|mg| mg.firing_mode == crate::types::FiringMode::Continuous)
                .unwrap_or(false);

            // Check if current target is dead
            if let Some(target_entity) = combat_unit.current_target {
                let target_exists = all_droids_query.get(target_entity).is_ok() ||
                                   all_towers_query.get(target_entity).is_ok();
                if !target_exists {
                    combat_unit.current_target = None;
                }
            }

            // MG continuous mode: immediately acquire new target
            if is_continuous_mode && combat_unit.current_target.is_none() {
                let shooter_pos = global_transform.translation();
                let mut closest_enemy: Option<(Entity, f32)> = None;

                // Check enemy units first
                for (target_entity, target_transform, target_droid, _) in all_droids_query.iter() {
                    if target_droid.team != droid.team {
                        let distance = shooter_pos.distance(target_transform.translation());
                        if distance <= TARGETING_RANGE {
                            if closest_enemy.map(|(_, d)| distance < d).unwrap_or(true) {
                                closest_enemy = Some((target_entity, distance));
                            }
                        }
                    }
                }

                // Fallback to towers
                if closest_enemy.is_none() {
                    for (target_entity, target_transform, target_tower) in all_towers_query.iter() {
                        if target_tower.team != droid.team {
                            let distance = shooter_pos.distance(target_transform.translation());
                            if distance <= TARGETING_RANGE {
                                if closest_enemy.map(|(_, d)| distance < d).unwrap_or(true) {
                                    closest_enemy = Some((target_entity, distance));
                                }
                            }
                        }
                    }
                }

                combat_unit.current_target = closest_enemy.map(|(entity, _)| entity);
            }

            // === FIRE AT TARGET ===
            if combat_unit.current_target.is_none() {
                if let Some(ref mut mg_turret) = mg_turret_opt {
                    mg_turret.shots_in_burst = 0;
                }
            }
            if let Some(target_entity) = combat_unit.current_target {
                // Get target position and movement state
                let target_info_opt: Option<(Vec3, bool)> = all_droids_query.get(target_entity)
                    .map(|(_, t, _, tracker)| (t.translation(), tracker.is_stationary))
                    .or_else(|_| all_towers_query.get(target_entity).map(|(_, t, _)| (t.translation(), true)))
                    .or_else(|_| turret_health_query.get(target_entity).map(|(t, _)| (t.translation(), true)))
                    .ok();

                if let Some((target_pos, target_stationary)) = target_info_opt {
                    let is_mg = mg_turret_opt.is_some();

                    // Determine fire interval based on turret type
                    let fire_interval = if is_mg { 0.05 } else { AUTO_FIRE_INTERVAL };
                    combat_unit.auto_fire_timer = fire_interval;

                    // Calculate firing position from barrel
                    let firing_pos = if is_mg {
                        // MG turret: compute barrel muzzle position with pitch rotation
                        let assembly_pos = global_transform.translation();
                        let barrel_pivot_world = assembly_pos + local_transform.rotation * MG_BARREL_PIVOT;
                        let pitch_angle = mg_barrel_pitch(barrel_pivot_world, target_pos);

                        // Compute muzzle position with pitch applied
                        let barrel_forward = local_transform.rotation * Vec3::NEG_Z;
                        let pitch_rotation = Quat::from_axis_angle(local_transform.rotation * Vec3::X, pitch_angle);
                        let pitched_forward = pitch_rotation * barrel_forward;
                        barrel_pivot_world + pitched_forward * MG_BARREL_MUZZLE_LENGTH
                    } else {
                        // Heavy turret: use standard barrel positions
                        let local_barrel_pos = standard_barrel_positions[turret.current_barrel_index % standard_barrel_positions.len()];
                        let world_barrel_offset = local_transform.rotation * local_barrel_pos;
                        global_transform.translation() + world_barrel_offset
                    };

                    // Check line of sight
                    if !has_line_of_sight(firing_pos, target_pos, heightmap.as_deref()) {
                        combat_unit.current_target = None;
                        // Burst is interrupted: reset so the next burst retriggers its audio
                        if let Some(ref mut mg_turret) = mg_turret_opt {
                            mg_turret.shots_in_burst = 0;
                        }
                        continue;
                    }

                    let direction = (target_pos - firing_pos).normalize();

                    // === ACCURACY CHECK ===
                    // Turrets are always stationary, skip stationary bonus (base accuracy is higher)
                    let hit_chance = calculate_hit_chance(
                        TURRET_BASE_ACCURACY,
                        firing_pos,
                        target_pos,
                        true, // Turrets always stationary
                        target_stationary,
                    );
                    let hit_success = rand::random::<f32>() < hit_chance;

                    let current_time = time.elapsed_secs();
                    let ray_length = firing_pos.distance(target_pos);

                    // === DETERMINE IMPACT POSITION ===
                    let impact_pos = if !hit_success {
                        // MISS - tracer goes past target
                        let miss_offset = Vec3::new(
                            (rand::random::<f32>() - 0.5) * 4.0,
                            (rand::random::<f32>() - 0.5) * 4.0,
                            (rand::random::<f32>() - 0.5) * 4.0,
                        );
                        target_pos + miss_offset
                    } else {
                        // HIT - check shields first
                        let mut shield_hit_pos: Option<Vec3> = None;

                        for mut shield in shield_query.iter_mut() {
                            if shield.team == droid.team {
                                continue;
                            }

                            if let Some((hit_dist, hit_pos)) = ray_sphere_intersection(
                                firing_pos, direction, shield.center, shield.radius
                            ) {
                                if hit_dist > 0.0 && hit_dist < ray_length {
                                    let old_hp = shield.current_hp;
                                    shield.take_damage(
                                        shield_config.laser_damage,
                                        current_time,
                                        shield_config.impact_flash_duration
                                    );
                                    trace!("Turret hitscan shield hit: hp {} -> {}", old_hp, shield.current_hp);
                                    if rand::random::<f32>() < 0.25 {
                                        shield.add_ripple(hit_pos, current_time);
                                    }
                                    shield_hit_pos = Some(hit_pos);
                                    break;
                                }
                            }
                        }

                        if let Some(hit_pos) = shield_hit_pos {
                            hit_pos
                        } else {
                            // === INSTANT HIT DETECTION ===
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
                                    commands.entity(hit_entity).try_despawn();
                                    squad_manager.remove_unit_from_squad(hit_entity);
                                    hit_pos
                                }
                                HitscanResult::HitTower(hit_pos) => {
                                    // Apply damage to buildings
                                    if let Ok((_, mut health)) = turret_health_query.get_mut(target_entity) {
                                        health.damage(HITSCAN_DAMAGE);
                                    } else if let Ok(mut health) = tower_health_query.get_mut(target_entity) {
                                        health.damage(HITSCAN_DAMAGE);
                                    } else if let Ok(child_of) = turret_assembly_query.get(target_entity) {
                                        let parent_entity = child_of.parent();
                                        if let Ok((_, mut health)) = turret_health_query.get_mut(parent_entity) {
                                            health.damage(HITSCAN_DAMAGE);
                                        }
                                    }
                                    hit_pos
                                }
                                HitscanResult::Miss(end_pos) => end_pos,
                            }
                        }
                    };

                    // === SPAWN VISUAL TRACER ===
                    // Turrets are Team A = green
                    let laser_material = laser_assets.team_a_material.clone();

                    // Use MG mesh for MG turrets, standard for heavy
                    let tracer_mesh = if is_mg {
                        laser_assets.mg_laser_mesh.clone()
                    } else {
                        laser_assets.hitscan_tracer_mesh.clone()
                    };

                    // Faster tracer speed for turrets (instant feel)
                    let tracer_speed = if is_mg { 600.0 } else { 500.0 };

                    let velocity = direction * tracer_speed;
                    let tracer_rotation = calculate_laser_orientation(velocity, firing_pos, camera_position);

                    commands.spawn((
                        Mesh3d(tracer_mesh),
                        MeshMaterial3d(laser_material),
                        Transform::from_translation(firing_pos).with_rotation(tracer_rotation),
                        HitscanTracer {
                            start_pos: firing_pos,
                            end_pos: impact_pos,
                            progress: 0.0,
                            speed: tracer_speed,
                            team: droid.team,
                        },
                    ));

                    // Advance barrel index (MG has 1 barrel, Heavy has 2)
                    let barrel_count = if is_mg { 1 } else { standard_barrel_positions.len() };
                    turret.current_barrel_index = (turret.current_barrel_index + 1) % barrel_count;

                    // Increment MG burst counter
                    if let Some(ref mut mg_turret) = mg_turret_opt {
                        mg_turret.shots_in_burst += 1;
                    }

                    // === AUDIO ===
                    if is_mg {
                        // Play burst sound once at start of each burst (not per-shot)
                        let burst_just_started = mg_turret_opt.as_ref()
                            .map(|mg| mg.shots_in_burst == 1)
                            .unwrap_or(false);
                        if burst_just_started {
                            let turret_pos = global_transform.translation();
                            let distance = turret_pos.distance(camera_position);
                            let volume = proximity_volume(
                                distance,
                                crate::constants::VOLUME_MG_TURRET * mg_burst_volume_scale,
                            );

                            commands.spawn((
                                AudioPlayer::new(audio_assets.mg_sound.clone()),
                                PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(volume)),
                                crate::types::MgBurstAudio {
                                    turret: turret_entity,
                                    volume,
                                },
                                crate::types::GunfireAudio { volume },
                            ));
                        }
                    } else {
                        shots_fired += 1;
                        if shots_fired <= MAX_AUDIO_PER_FRAME {
                            let mut rng = rand::thread_rng();
                            let sound = audio_assets.get_random_laser_sound(&mut rng);
                            let turret_pos = global_transform.translation();
                            let distance = turret_pos.distance(camera_position);
                            let volume = proximity_volume(distance, crate::constants::VOLUME_HEAVY_TURRET);

                            commands.spawn((
                                AudioPlayer::new(sound),
                                PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(volume)),
                                crate::types::GunfireAudio { volume },
                            ));
                        }
                    }
                }
            }
        }
    }
}

/// How long an MG burst clip takes to fade out after the last bullet.
/// Matches the fade-out baked into the tail of the clip itself.
const MG_BURST_AUDIO_FADE_SECS: f32 = 0.25;

/// Start fading out MG burst audio the moment its turret stops firing —
/// burst complete (cooldown), target lost, LOS blocked, or turret destroyed.
/// This keeps the clip's end synced to the last bullet no matter how the
/// burst ended, without the hard cut of despawning mid-playback.
pub fn mg_burst_audio_sync_system(
    mut commands: Commands,
    audio_query: Query<(Entity, &crate::types::MgBurstAudio), Without<crate::types::AudioFadeOut>>,
    turret_query: Query<(&CombatUnit, &crate::types::MgTurret)>,
) {
    for (audio_entity, burst_audio) in audio_query.iter() {
        // Turret gone (destroyed) counts as not firing
        let still_firing = turret_query.get(burst_audio.turret)
            .map(|(combat_unit, mg_turret)| {
                combat_unit.current_target.is_some()
                    && mg_turret.cooldown_timer <= 0.0
                    && mg_turret.shots_in_burst > 0
            })
            .unwrap_or(false);

        if !still_firing {
            commands.entity(audio_entity).insert(crate::types::AudioFadeOut {
                remaining: MG_BURST_AUDIO_FADE_SECS,
                duration: MG_BURST_AUDIO_FADE_SECS,
            });
        }
    }
}

/// Duck the gunfire bed while an explosion plays, then ramp it back.
/// One explosion clip can't compete with dozens of concurrent gunfire clips —
/// masking, not volume, is why explosions sound small in a firefight.
pub fn explosion_ducking_system(
    time: Res<Time>,
    mut ducking: ResMut<crate::types::ExplosionDucking>,
    mut gunfire_query: Query<(&crate::types::GunfireAudio, &mut AudioSink), Without<crate::types::AudioFadeOut>>,
) {
    if ducking.timer <= 0.0 {
        return;
    }
    ducking.timer -= time.delta_secs();

    // Full duck, then ramp back to 1.0 over the release window so there's no pop
    let factor = if ducking.timer <= 0.0 {
        1.0
    } else if ducking.timer >= crate::constants::EXPLOSION_DUCK_RELEASE {
        crate::constants::EXPLOSION_DUCK_FACTOR
    } else {
        let t = ducking.timer / crate::constants::EXPLOSION_DUCK_RELEASE;
        crate::constants::EXPLOSION_DUCK_FACTOR * t + 1.0 * (1.0 - t)
    };

    for (gunfire, mut sink) in gunfire_query.iter_mut() {
        sink.set_volume(bevy::audio::Volume::Linear(gunfire.volume * factor));
    }
}

/// Fade marked audio entities to silence, then despawn them.
/// One-way: once AudioFadeOut is inserted the clip always dies.
pub fn audio_fade_out_system(
    mut commands: Commands,
    time: Res<Time>,
    mut audio_query: Query<(Entity, &crate::types::MgBurstAudio, &mut crate::types::AudioFadeOut, &mut AudioSink)>,
) {
    for (entity, burst_audio, mut fade, mut sink) in audio_query.iter_mut() {
        fade.remaining -= time.delta_secs();
        if fade.remaining <= 0.0 {
            commands.entity(entity).try_despawn();
        } else {
            let t = fade.remaining / fade.duration;
            sink.set_volume(bevy::audio::Volume::Linear(burst_audio.volume * t));
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
    droid_query: &Query<(Entity, &GlobalTransform, &BattleDroid, &MovementTracker), Without<HitscanTracer>>,
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
            if let Ok((_, droid_transform, droid, _)) = droid_query.get(entity) {
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
    droid_query: Query<(Entity, &Transform, &BattleDroid, &SquadMember, &MovementTracker), Without<LaserProjectile>>,
    building_query: Query<(Entity, &GlobalTransform, &crate::types::BuildingCollider)>,
    mut turret_health_query: Query<&mut crate::types::Health, With<crate::types::TurretBase>>,
) {
    // Clear and rebuild the spatial grid each frame
    spatial_grid.clear();

    // Populate grid with droids
    for (droid_entity, droid_transform, _, _, _) in droid_query.iter() {
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
            if let Ok((_, droid_transform, droid, _squad_member, movement_tracker)) = droid_query.get(droid_entity) {
                // Skip friendly fire
                if laser.team == droid.team {
                    continue;
                }

                // Simple sphere collision detection using distance_squared to avoid sqrt
                const COLLISION_RADIUS_SQ: f32 = COLLISION_RADIUS * COLLISION_RADIUS;
                let distance_sq = laser_transform.translation.distance_squared(droid_transform.translation);
                if distance_sq <= COLLISION_RADIUS_SQ {
                    // Collision detected - now roll for accuracy (turrets use TURRET_BASE_ACCURACY)
                    let hit_chance = calculate_hit_chance(
                        TURRET_BASE_ACCURACY,
                        laser.origin,
                        droid_transform.translation,
                        true, // Turrets are always stationary
                        movement_tracker.is_stationary,
                    );

                    // Always despawn the laser on collision
                    entities_to_despawn.insert(laser_entity);

                    // Only despawn/kill the droid if hit succeeds
                    if rand::random::<f32>() < hit_chance {
                        entities_to_despawn.insert(droid_entity);
                        // Handle squad casualty immediately (commander promotion, etc.)
                        squad_manager.remove_unit_from_squad(droid_entity);
                    }
                    // If miss, laser still despawns but droid survives

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

/// Turret rotation system - handles yaw (assembly) and pitch (barrel) rotation
/// - TurretRotatingAssembly: horizontal rotation to face target
/// - TurretBarrel: vertical rotation for MG turrets to aim up/down
pub fn turret_rotation_system(
    time: Res<Time>,
    mut turret_query: Query<(&mut Transform, &GlobalTransform, &CombatUnit, Option<&crate::types::MgTurret>, Option<&Children>), With<crate::types::TurretRotatingAssembly>>,
    target_query: Query<&GlobalTransform, (With<BattleDroid>, Without<crate::types::TurretRotatingAssembly>)>,
    mut barrel_query: Query<&mut Transform, (With<crate::types::TurretBarrel>, Without<crate::types::TurretRotatingAssembly>)>,
) {
    for (mut turret_transform, turret_global_transform, combat_unit, mg_turret, children) in turret_query.iter_mut() {
        if let Some(target_entity) = combat_unit.current_target {
            if let Ok(target_global_transform) = target_query.get(target_entity) {
                let turret_pos = turret_global_transform.translation();
                let target_pos = target_global_transform.translation();

                // === YAW: Assembly rotation (horizontal) ===
                let target_pos_flat = Vec3::new(target_pos.x, turret_pos.y, target_pos.z);
                let target_rotation = Transform::from_translation(turret_pos)
                    .looking_at(target_pos_flat, Vec3::Y)
                    .rotation;

                let rotation_speed = if mg_turret.is_some() { 5.0 } else { 3.0 };
                turret_transform.rotation = turret_transform.rotation.slerp(
                    target_rotation,
                    rotation_speed * time.delta_secs()
                );

                // === PITCH: Barrel rotation (vertical) - MG turret only ===
                if mg_turret.is_some() {
                    if let Some(children) = children {
                        for child in children.iter() {
                            if let Ok(mut barrel_transform) = barrel_query.get_mut(child) {
                                let barrel_world_pos = turret_pos + turret_transform.rotation * MG_BARREL_PIVOT;
                                let pitch_angle = mg_barrel_pitch(barrel_world_pos, target_pos);
                                let target_pitch = Quat::from_rotation_x(pitch_angle);

                                barrel_transform.rotation = barrel_transform.rotation.slerp(
                                    target_pitch,
                                    rotation_speed * time.delta_secs()
                                );
                            }
                        }
                    }
                }
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

/// System: Clear targets that have been blocked for too long
/// Prevents AttackMove units from getting stuck when they can't shoot
pub fn clear_blocked_targets_system(
    time: Res<Time>,
    mut query: Query<&mut CombatUnit, With<BattleDroid>>,
) {
    let delta_time = time.delta_secs();

    for mut combat in query.iter_mut() {
        if combat.current_target.is_some() {
            combat.blocked_timer += delta_time;

            // If blocked for too long, give up on this target
            if combat.blocked_timer > BLOCKED_TARGET_TIMEOUT {
                combat.current_target = None;
                combat.blocked_timer = 0.0;
            }
        } else {
            // No target - reset timer
            combat.blocked_timer = 0.0;
        }
    }
} 
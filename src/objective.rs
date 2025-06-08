// Objective system module - Uplink Tower mechanics
use bevy::prelude::*;
use rand::Rng;
use crate::types::*;
use crate::constants::*;

// ===== TOWER CREATION =====

pub fn create_uplink_tower_mesh(meshes: &mut ResMut<Assets<Mesh>>) -> Handle<Mesh> {
    use bevy::render::mesh::{Indices, PrimitiveTopology};
    use bevy::render::render_asset::RenderAssetUsages;
    
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    
    // Create a tall, pointy sci-fi tower with tapered design
    let base_width = TOWER_BASE_WIDTH;
    let height = TOWER_HEIGHT;
    let top_width = base_width * 0.2; // Tapers to 20% of base width at top
    let mid_height = height * 0.7; // Transition point
    
    let vertices = vec![
        // Base (octagonal for sci-fi look)
        // Bottom vertices (y = 0)
        [base_width, 0.0, 0.0],           // 0: +X
        [base_width * 0.7, 0.0, base_width * 0.7],  // 1: +X+Z
        [0.0, 0.0, base_width],           // 2: +Z
        [-base_width * 0.7, 0.0, base_width * 0.7], // 3: -X+Z
        [-base_width, 0.0, 0.0],          // 4: -X
        [-base_width * 0.7, 0.0, -base_width * 0.7], // 5: -X-Z
        [0.0, 0.0, -base_width],          // 6: -Z
        [base_width * 0.7, 0.0, -base_width * 0.7],  // 7: +X-Z
        
        // Mid-section (y = mid_height)
        [base_width * 0.6, mid_height, 0.0],         // 8
        [base_width * 0.42, mid_height, base_width * 0.42], // 9
        [0.0, mid_height, base_width * 0.6],         // 10
        [-base_width * 0.42, mid_height, base_width * 0.42], // 11
        [-base_width * 0.6, mid_height, 0.0],        // 12
        [-base_width * 0.42, mid_height, -base_width * 0.42], // 13
        [0.0, mid_height, -base_width * 0.6],        // 14
        [base_width * 0.42, mid_height, -base_width * 0.42], // 15
        
        // Top (pointed)
        [top_width, height, 0.0],           // 16
        [top_width * 0.7, height, top_width * 0.7],  // 17
        [0.0, height, top_width],           // 18
        [-top_width * 0.7, height, top_width * 0.7], // 19
        [-top_width, height, 0.0],          // 20
        [-top_width * 0.7, height, -top_width * 0.7], // 21
        [0.0, height, -top_width],          // 22
        [top_width * 0.7, height, -top_width * 0.7],  // 23
        
        // Apex point
        [0.0, height + base_width * 0.5, 0.0], // 24: Sharp point at top
    ];
    
    // Generate triangular faces for the octagonal tower
    let mut indices = Vec::new();
    
    // Bottom to mid-section faces (8 trapezoids, 2 triangles each)
    for i in 0..8 {
        let next = (i + 1) % 8;
        let base_i = i;
        let base_next = next;
        let mid_i = i + 8;
        let mid_next = next + 8;
        
        // Triangle 1: base_i -> base_next -> mid_i
        indices.extend_from_slice(&[base_i as u32, base_next as u32, mid_i as u32]);
        // Triangle 2: base_next -> mid_next -> mid_i
        indices.extend_from_slice(&[base_next as u32, mid_next as u32, mid_i as u32]);
    }
    
    // Mid-section to top faces (8 trapezoids, 2 triangles each)
    for i in 0..8 {
        let next = (i + 1) % 8;
        let mid_i = i + 8;
        let mid_next = next + 8;
        let top_i = i + 16;
        let top_next = next + 16;
        
        // Triangle 1: mid_i -> mid_next -> top_i
        indices.extend_from_slice(&[mid_i as u32, mid_next as u32, top_i as u32]);
        // Triangle 2: mid_next -> top_next -> top_i
        indices.extend_from_slice(&[mid_next as u32, top_next as u32, top_i as u32]);
    }
    
    // Top to apex (8 triangles)
    for i in 0..8 {
        let next = (i + 1) % 8;
        let top_i = i + 16;
        let top_next = next + 16;
        
        // Triangle: top_i -> top_next -> apex
        indices.extend_from_slice(&[top_i as u32, top_next as u32, 24]);
    }
    
    // Generate normals (simplified outward-facing)
    let mut normals = Vec::new();
    for _ in &vertices {
        normals.push([0.0, 1.0, 0.0]); // Simplified upward normals
    }
    
    // Set mesh attributes
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
    tower_query: Query<(Entity, &Transform, &UplinkTower, &Health), With<UplinkTower>>,
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
            let explosion_count = units_to_explode.len(); // Get count before moving the vector
            let mut rng = rand::thread_rng();
            for unit_entity in units_to_explode {
                let delay = rng.gen_range(EXPLOSION_DELAY_MIN..EXPLOSION_DELAY_MAX);
                commands.entity(unit_entity).insert(PendingExplosion {
                    delay_timer: delay,
                    explosion_power: 1.0,
                });
            }
            
            // Create massive explosion effect at tower location
            commands.spawn((
                ExplosionEffect {
                    timer: 0.0,
                    max_time: EXPLOSION_EFFECT_DURATION * 2.0, // Tower explosion lasts longer
                    radius: tower.destruction_radius,
                    intensity: 2.0,
                },
                Transform::from_translation(tower_transform.translation),
            ));
            
            // Remove the tower
            commands.entity(tower_entity).despawn_recursive();
            
            info!("Tower {:?} destroyed! {} friendly units scheduled for cascade explosion", 
                  tower.team, explosion_count);
        }
    }
}

// ===== DELAYED EXPLOSION SYSTEM =====

pub fn pending_explosion_system(
    mut commands: Commands,
    mut explosion_query: Query<(Entity, &mut PendingExplosion, &Transform), With<PendingExplosion>>,
    time: Res<Time>,
) {
    for (entity, mut pending, transform) in explosion_query.iter_mut() {
        pending.delay_timer -= time.delta_seconds();
        
        if pending.delay_timer <= 0.0 {
            // Create explosion effect
            commands.spawn((
                ExplosionEffect {
                    timer: 0.0,
                    max_time: EXPLOSION_EFFECT_DURATION,
                    radius: 5.0, // Individual unit explosion radius
                    intensity: pending.explosion_power,
                },
                Transform::from_translation(transform.translation),
            ));
            
            // Remove the unit
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
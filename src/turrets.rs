// Turret spawn systems module
use bevy::prelude::*;
use crate::types::*;
use crate::terrain::{TerrainHeightmap, MapSwitchEvent};
use crate::procedural_meshes::*;

/// Internal helper to spawn MG turret at specified position
fn spawn_mg_turret_internal(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    x: f32,
    z: f32,
    terrain_height: f32,
) {
    let turret_world_pos = Vec3::new(x, terrain_height, z);

    // Create meshes
    let base_mesh = create_mg_turret_base_mesh(meshes);
    let assembly_mesh = create_mg_turret_assembly_mesh(meshes);

    // Create materials (Darker, more industrial)
    let base_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.2, 0.2),
        metallic: 0.8,
        perceptual_roughness: 0.6,
        ..default()
    });

    let gun_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.35, 0.3), // Slightly greenish military metal
        metallic: 0.9,
        perceptual_roughness: 0.4,
        ..default()
    });

    // Spawn base entity (parent)
    let base_entity = commands.spawn((
        Mesh3d(base_mesh),
        MeshMaterial3d(base_material),
        Transform::from_translation(turret_world_pos),
        TurretBase,
        BuildingCollider { radius: 3.0 }, // Smaller collision radius
    )).id();

    // Spawn rotating assembly entity (child)
    let assembly_entity = commands.spawn((
        Mesh3d(assembly_mesh),
        MeshMaterial3d(gun_material),
        Transform::from_xyz(0.0, 1.0, 0.0), // Mounted on top of base
        BattleDroid {
            team: Team::A,
            march_speed: 0.0,
            spawn_position: turret_world_pos,
            target_position: turret_world_pos,
            march_offset: 0.0,
            returning_to_spawn: false,
        },
        CombatUnit {
            target_scan_timer: 0.0,
            auto_fire_timer: 0.3, // Faster fire rate for MG
            current_target: None,
        },
        TurretRotatingAssembly {
            current_barrel_index: 0,
        },
        MgTurret {
            firing_mode: FiringMode::Continuous, // Start with Continuous mode for mowing down
            shots_in_burst: 0,
            max_burst_shots: 45,   // 45 shots at 0.05s = ~2.25 seconds before pause (20 shots/sec)
            cooldown_timer: 0.0,   // No cooldown initially
            cooldown_duration: 1.5, // 1.5 second pause between bursts/sweeps
        },
    )).id();

    // Link child to parent
    commands.entity(base_entity).add_children(&[assembly_entity]);
}

/// Spawn a functional MG turret (only if enabled in debug mode)
pub fn spawn_mg_turret(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    heightmap: Res<TerrainHeightmap>,
    debug_mode: Res<crate::objective::ExplosionDebugMode>,
) {
    if !debug_mode.mg_turret_enabled {
        info!("MG turret disabled at startup (press 0 then M to enable)");
        return;
    }

    let x = 10.0;
    let z = 10.0;
    let terrain_height = heightmap.sample_height(x, z);

    spawn_mg_turret_internal(&mut commands, &mut meshes, &mut materials, x, z, terrain_height);
    info!("Spawned MG turret at position ({}, {}, {})", x, terrain_height, z);
}

/// Internal helper to spawn heavy turret at specified position
fn spawn_heavy_turret_internal(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    x: f32,
    z: f32,
    terrain_height: f32,
) {
    let turret_world_pos = Vec3::new(x, terrain_height, z);

    // Create meshes
    let base_mesh = create_turret_base_mesh(meshes);
    let assembly_mesh = create_turret_rotating_assembly_mesh(meshes);

    // Create materials
    let base_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.3, 0.35),
        metallic: 0.1,
        perceptual_roughness: 0.8,
        ..default()
    });

    let gun_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.25, 0.3),
        metallic: 0.8,
        perceptual_roughness: 0.4,
        ..default()
    });

    // Spawn base entity (parent)
    let base_entity = commands.spawn((
        Mesh3d(base_mesh),
        MeshMaterial3d(base_material),
        Transform::from_translation(turret_world_pos),
        TurretBase,
        BuildingCollider { radius: 4.0 }, // Collision radius for laser blocking
    )).id();

    // Spawn rotating assembly entity (child)
    let assembly_entity = commands.spawn((
        Mesh3d(assembly_mesh),
        MeshMaterial3d(gun_material),
        Transform::from_xyz(0.0, 2.7, 0.0), // Mounted on top of base
        BattleDroid {
            team: Team::A,
            march_speed: 0.0,
            spawn_position: turret_world_pos,
            target_position: turret_world_pos,
            march_offset: 0.0,
            returning_to_spawn: false,
        },
        CombatUnit {
            target_scan_timer: 0.0,
            auto_fire_timer: 2.0,
            current_target: None,
        },
        TurretRotatingAssembly {
            current_barrel_index: 0,
        },
    )).id();

    // Link child to parent
    commands.entity(base_entity).add_children(&[assembly_entity]);
}

/// Spawn a functional heavy turret (only if enabled in debug mode)
pub fn spawn_functional_turret(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    heightmap: Res<TerrainHeightmap>,
    debug_mode: Res<crate::objective::ExplosionDebugMode>,
) {
    if !debug_mode.heavy_turret_enabled {
        info!("Heavy turret disabled at startup (press 0 then H to enable)");
        return;
    }

    let x = 30.0;
    let z = 30.0;
    let terrain_height = heightmap.sample_height(x, z);

    spawn_heavy_turret_internal(&mut commands, &mut meshes, &mut materials, x, z, terrain_height);
    info!("Spawned functional turret at position ({}, {}, {})", x, terrain_height, z);
}

/// System to respawn turrets when map switches
pub fn respawn_turrets_on_map_switch(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    heightmap: Res<TerrainHeightmap>,
    mut map_switch_events: EventReader<MapSwitchEvent>,
    turret_base_query: Query<Entity, With<TurretBase>>,
) {
    // Only process if there's a map switch event
    if map_switch_events.read().next().is_none() {
        return;
    }

    // Despawn all existing turrets (both base and assembly entities)
    for base_entity in turret_base_query.iter() {
        commands.entity(base_entity).despawn();
    }

    info!("Respawning turrets for new terrain");

    // Spawn MG turret at new terrain height
    let mg_x = 10.0;
    let mg_z = 10.0;
    let mg_height = heightmap.sample_height(mg_x, mg_z);
    spawn_mg_turret_internal(&mut commands, &mut meshes, &mut materials, mg_x, mg_z, mg_height);
    info!("Respawned MG turret at ({}, {}, {})", mg_x, mg_height, mg_z);

    // Spawn heavy turret at new terrain height
    let heavy_x = 30.0;
    let heavy_z = 30.0;
    let heavy_height = heightmap.sample_height(heavy_x, heavy_z);
    spawn_heavy_turret_internal(&mut commands, &mut meshes, &mut materials, heavy_x, heavy_z, heavy_height);
    info!("Respawned heavy turret at ({}, {}, {})", heavy_x, heavy_height, heavy_z);
}

/// Debug system to toggle turrets on/off (M=MG, H=Heavy) when debug mode is active
pub fn debug_turret_toggle_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    heightmap: Res<TerrainHeightmap>,
    mut debug_mode: ResMut<crate::objective::ExplosionDebugMode>,
    mg_turret_query: Query<Entity, With<MgTurret>>,
    // Heavy turret has TurretRotatingAssembly but NOT MgTurret
    heavy_turret_query: Query<Entity, (With<TurretRotatingAssembly>, Without<MgTurret>)>,
    turret_base_query: Query<(Entity, &Children), With<TurretBase>>,
) {
    // Only process when debug mode is active
    if !debug_mode.explosion_mode {
        return;
    }

    // M key: Toggle MG turret
    if keyboard_input.just_pressed(KeyCode::KeyM) {
        debug_mode.mg_turret_enabled = !debug_mode.mg_turret_enabled;

        if debug_mode.mg_turret_enabled {
            // Spawn MG turret
            let x = 10.0;
            let z = 10.0;
            let terrain_height = heightmap.sample_height(x, z);
            spawn_mg_turret_internal(&mut commands, &mut meshes, &mut materials, x, z, terrain_height);
            info!("🔫 MG turret ENABLED");
        } else {
            // Despawn MG turret - find the base that has MG turret as child
            for (base_entity, children) in turret_base_query.iter() {
                for child in children.iter() {
                    if mg_turret_query.get(child).is_ok() {
                        commands.entity(base_entity).despawn_recursive();
                        info!("🔫 MG turret DISABLED");
                        break;
                    }
                }
            }
        }
    }

    // H key: Toggle Heavy turret
    if keyboard_input.just_pressed(KeyCode::KeyH) {
        debug_mode.heavy_turret_enabled = !debug_mode.heavy_turret_enabled;

        if debug_mode.heavy_turret_enabled {
            // Spawn Heavy turret
            let x = 30.0;
            let z = 30.0;
            let terrain_height = heightmap.sample_height(x, z);
            spawn_heavy_turret_internal(&mut commands, &mut meshes, &mut materials, x, z, terrain_height);
            info!("🔫 Heavy turret ENABLED");
        } else {
            // Despawn Heavy turret - find the base that has heavy turret (no MgTurret) as child
            for (base_entity, children) in turret_base_query.iter() {
                for child in children.iter() {
                    if heavy_turret_query.get(child).is_ok() {
                        commands.entity(base_entity).despawn_recursive();
                        info!("🔫 Heavy turret DISABLED");
                        break;
                    }
                }
            }
        }
    }
}

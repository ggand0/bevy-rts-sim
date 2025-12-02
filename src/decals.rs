// Decal rendering system using Bevy 0.16's ClusteredDecal
use bevy::prelude::*;
use bevy::pbr::decal::clustered::ClusteredDecal;

pub struct DecalPlugin;

#[derive(Component)]
struct DecalsSpawned;

impl Plugin for DecalPlugin {
    fn build(&self, app: &mut App) {
        // ClusteredDecalPlugin is already included in DefaultPlugins
        app.add_systems(Startup, setup_decal_textures.before(crate::terrain::spawn_initial_terrain))
            .add_systems(Update, spawn_test_decals);
    }
}

#[derive(Resource)]
pub struct DecalTextures {
    pub bullet_hole: Handle<Image>,
    pub selection_ring: Handle<Image>,
}

fn setup_decal_textures(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    info!("🎨 Loading decal textures...");

    // Load textures from assets folder
    let bullet_hole_handle: Handle<Image> = asset_server.load("textures/bullet_hole_0.png");
    let selection_ring_handle: Handle<Image> = asset_server.load("textures/wfx/WFX_T_GlowCircle A8.png");

    commands.insert_resource(DecalTextures {
        bullet_hole: bullet_hole_handle,
        selection_ring: selection_ring_handle,
    });

    info!("✅ Decal textures loaded!");
}

fn spawn_test_decals(
    mut commands: Commands,
    decal_textures: Res<DecalTextures>,
    images: Res<Assets<Image>>,
    spawned_query: Query<&DecalsSpawned>,
    terrain_query: Query<&crate::terrain::TerrainMarker>,
) {
    // Only spawn once
    if !spawned_query.is_empty() {
        return;
    }

    // Wait for terrain to exist
    if terrain_query.is_empty() {
        return;
    }

    // Wait for texture to finish loading before spawning decals
    let texture = images.get(&decal_textures.bullet_hole);
    if texture.is_none() {
        debug!("Waiting for bullet hole texture to load...");
        return; // Texture not loaded yet, will try again next frame
    }

    info!("🎯 Spawning test decals around map center (texture loaded: {:?})...", texture.is_some());

    // Turret positions to avoid: (10, -1, 10) and (30, -1, 30)
    let turret_positions = [
        Vec3::new(10.0, -1.0, 10.0),
        Vec3::new(30.0, -1.0, 30.0),
    ];

    let min_distance_from_turret = 8.0; // Minimum distance to keep from turrets

    // Spawn 12 decals in a pattern around the map center (0, -1, 0)
    let positions = [
        Vec3::new(-5.0, -1.0, -5.0),
        Vec3::new(5.0, -1.0, -5.0),
        Vec3::new(-5.0, -1.0, 5.0),
        Vec3::new(5.0, -1.0, 5.0),
        Vec3::new(-10.0, -1.0, 0.0),
        Vec3::new(10.0, -1.0, 0.0),
        Vec3::new(0.0, -1.0, -10.0),
        Vec3::new(0.0, -1.0, 10.0),
        Vec3::new(-8.0, -1.0, -8.0),
        Vec3::new(8.0, -1.0, 8.0),
        Vec3::new(-8.0, -1.0, 8.0),
        Vec3::new(8.0, -1.0, -8.0),
    ];

    let mut spawned_count = 0;

    for position in positions.iter() {
        // Check if position is too close to any turret
        let too_close = turret_positions.iter().any(|turret_pos| {
            let dist = position.distance(*turret_pos);
            dist < min_distance_from_turret
        });

        if too_close {
            debug!("Skipping decal at {:?} - too close to turret", position);
            continue;
        }

        // Spawn decal projected downward onto the terrain
        // The transform scale controls the decal projection box:
        // - X: width
        // - Y: projection depth (how far it projects into geometry)
        // - Z: length
        commands.spawn((
            ClusteredDecal {
                image: decal_textures.bullet_hole.clone(),
                tag: 1,
            },
            Transform::from_translation(*position + Vec3::new(0.0, 2.0, 0.0)) // Position above ground for projection
                .with_scale(Vec3::new(4.0, 4.0, 4.0)) // Larger projection box
                .looking_to(Vec3::NEG_Y, Vec3::Z), // Project downward
            Name::new(format!("BulletHoleDecal_{}", spawned_count)),
        ));

        spawned_count += 1;
    }

    // Mark that we've spawned decals
    commands.spawn(DecalsSpawned);

    info!("✅ Spawned {} decals around map center", spawned_count);
}

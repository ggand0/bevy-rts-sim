// UE5 Niagara-style ground explosion with flipbook billboards
// Ported from NS_Explosion_Sand_5

use bevy::prelude::*;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, NotShadowCaster, NotShadowReceiver};
use bevy::render::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::render::render_resource::{AsBindGroup, BlendState, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError};
use bevy::asset::RenderAssetUsages;
use rand::Rng;

use crate::wfx_materials::AdditiveMaterial;

// ===== HELPER FUNCTIONS =====

/// Convert HSV to RGB (all values 0.0-1.0)
/// Used for UE5-style color variation on particles
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    if s <= 0.0 {
        return (v, v, v);
    }

    let h = (h % 1.0) * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

// ===== FLIPBOOK MATERIAL =====

/// Custom material for flipbook sprite sheets with non-square grid support
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct FlipbookMaterial {
    #[uniform(0)]
    pub frame_data: Vec4, // x: frame_col, y: frame_row, z: columns, w: rows
    #[uniform(1)]
    pub color_data: Vec4, // RGB: tint color, A: alpha
    #[texture(2, dimension = "2d")]
    #[sampler(3)]
    pub sprite_texture: Handle<Image>,
}

impl Material for FlipbookMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/flipbook.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        // Enable proper alpha blending
        if let Some(ref mut fragment) = descriptor.fragment {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState::ALPHA_BLENDING);
            }
        }
        Ok(())
    }
}

// ===== COMPONENTS =====

/// Flipbook sprite animation component
#[derive(Component)]
pub struct FlipbookSprite {
    pub columns: u32,
    pub rows: u32,
    pub total_frames: u32,
    pub frame_duration: f32,
    pub elapsed: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub base_alpha: f32, // Original alpha for fade calculations
    pub loop_animation: bool, // If false, animation plays once then holds last frame
}

/// Velocity-aligned billboard - sprite up-axis follows velocity direction
#[derive(Component)]
pub struct VelocityAligned {
    pub velocity: Vec3,
    pub gravity: f32,
}

/// Standard camera-facing billboard (unaligned mode)
#[derive(Component)]
pub struct CameraFacing;

/// Smoke particle physics - velocity with drag and acceleration
#[derive(Component)]
pub struct SmokePhysics {
    pub velocity: Vec3,
    pub acceleration: Vec3,
    pub drag: f32,
}

/// Smoke scale-over-life component - grows 2-3x using ease-out curve
#[derive(Component)]
pub struct SmokeScaleOverLife {
    pub initial_size: f32,
}

/// Smoke color-over-life - color darkens and alpha fades over lifetime
/// Based on typical UE5 Niagara ColorFromCurve:
/// t=0.0: RGB(0.4), A=0.6 | t=0.3: RGB(0.3), A=0.5 | t=0.7: RGB(0.25), A=0.3 | t=1.0: RGB(0.2), A=0.0
#[derive(Component)]
pub struct SmokeColorOverLife;

/// Sprite rotation around the billboard's facing axis (Z-axis in local space)
/// This rotation is applied AFTER billboarding calculation to preserve random sprite orientation
/// UE5: InitializeParticle.Sprite Rotation Angle 0-360°
#[derive(Component)]
pub struct SpriteRotation {
    pub angle: f32, // Rotation angle in radians
}

/// Marker for bottom-pivot billboards (fireballs grow upward)
#[derive(Component)]
pub struct BottomPivot;

/// Ground explosion parent entity for lifetime tracking
#[derive(Component)]
pub struct GroundExplosion {
    pub lifetime: f32,
    pub max_lifetime: f32,
}

/// Marker for ground explosion child entities
#[derive(Component)]
pub struct GroundExplosionChild;

/// Impact light component for fading point lights
#[derive(Component)]
pub struct ImpactLight {
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub base_intensity: f32,
}

// ===== DEBUG MENU =====

/// Debug menu state for ground explosion emitter testing
#[derive(Resource, Default)]
pub struct GroundExplosionDebugMenu {
    pub active: bool,
}

/// Emitter types for individual testing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitterType {
    MainFireball,      // 1
    SecondaryFireball, // 2
    Smoke,             // 3
    Wisp,              // 4
    Dust,              // 5
    Spark,             // 6
    FlashSpark,        // 7
    Impact,            // 8
    Dirt,              // 9
    // VelocityDirt not mapped - only 9 keys
}

// ===== PRELOADED ASSETS =====

#[derive(Resource)]
pub struct GroundExplosionAssets {
    // Flipbook textures
    pub main_texture: Handle<Image>,        // 8x8 (64 frames)
    pub secondary_texture: Handle<Image>,   // 8x8 (64 frames)
    pub smoke_texture: Handle<Image>,       // 8x8 (64 frames)
    pub wisp_texture: Handle<Image>,        // 8x8 (64 frames)
    pub dust_texture: Handle<Image>,        // 4x1 (4 frames)
    // Single-frame textures
    pub dirt_texture: Handle<Image>,
    pub flare_texture: Handle<Image>,
    pub impact_texture: Handle<Image>,
    pub glow_circle_texture: Handle<Image>, // WFX glow circle - works with additive shader
    // Shared meshes
    pub centered_quad: Handle<Mesh>,
    pub bottom_pivot_quad: Handle<Mesh>,
}

// ===== MESH CREATION =====

/// Create a standard centered quad mesh
fn create_centered_quad(size: f32) -> Mesh {
    let half = size / 2.0;

    // Vertices in counter-clockwise order (facing +Z)
    let vertices = vec![
        [-half, -half, 0.0],  // bottom-left  (vertex 0)
        [ half, -half, 0.0],  // bottom-right (vertex 1)
        [ half,  half, 0.0],  // top-right    (vertex 2)
        [-half,  half, 0.0],  // top-left     (vertex 3)
    ];

    let normals = vec![
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];

    // Bevy UV convention: [0,0] is TOP-LEFT, [1,1] is BOTTOM-RIGHT
    // This matches DirectX/Vulkan/Metal/WebGPU and image file formats
    let uvs = vec![
        [0.0, 1.0],  // bottom-left  -> UV bottom-left
        [1.0, 1.0],  // bottom-right -> UV bottom-right
        [1.0, 0.0],  // top-right    -> UV top-right
        [0.0, 0.0],  // top-left     -> UV top-left
    ];

    let indices = Indices::U32(vec![0, 1, 2, 0, 2, 3]);

    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(indices)
}

/// Create a bottom-pivot quad mesh (origin at bottom-center, grows upward)
fn create_bottom_pivot_quad(size: f32) -> Mesh {
    let half = size / 2.0;

    // Vertices offset so origin is at bottom-center
    let vertices = vec![
        [-half, 0.0, 0.0],      // bottom-left  (vertex 0)
        [ half, 0.0, 0.0],      // bottom-right (vertex 1)
        [ half, size, 0.0],     // top-right    (vertex 2)
        [-half, size, 0.0],     // top-left     (vertex 3)
    ];

    let normals = vec![
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];

    // Bevy UV convention: [0,0] is TOP-LEFT, [1,1] is BOTTOM-RIGHT
    let uvs = vec![
        [0.0, 1.0],  // bottom-left  -> UV bottom-left
        [1.0, 1.0],  // bottom-right -> UV bottom-right
        [1.0, 0.0],  // top-right    -> UV top-right
        [0.0, 0.0],  // top-left     -> UV top-left
    ];

    let indices = Indices::U32(vec![0, 1, 2, 0, 2, 3]);

    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(indices)
}

// ===== ASSET LOADING =====

pub fn setup_ground_explosion_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    info!("🌋 Loading ground explosion assets...");

    let assets = GroundExplosionAssets {
        // Premium assets - CC-BY licensed, not included in repo
        // See assets/textures/premium/ground_explosion/
        main_texture: asset_server.load("textures/premium/ground_explosion/main_9x9.png"),
        secondary_texture: asset_server.load("textures/premium/ground_explosion/secondary_8x8.png"),
        smoke_texture: asset_server.load("textures/premium/ground_explosion/smoke_8x8.png"),
        wisp_texture: asset_server.load("textures/premium/ground_explosion/wisp_8x8.png"),
        dust_texture: asset_server.load("textures/premium/ground_explosion/dust_4x1.png"),
        dirt_texture: asset_server.load("textures/premium/ground_explosion/dirt.png"),
        flare_texture: asset_server.load("textures/premium/ground_explosion/flare.png"),
        impact_texture: asset_server.load("textures/premium/ground_explosion/impact.png"),
        glow_circle_texture: asset_server.load("textures/wfx/WFX_T_GlowCircle A8.png"),
        centered_quad: meshes.add(create_centered_quad(1.0)),
        bottom_pivot_quad: meshes.add(create_bottom_pivot_quad(1.0)),
    };

    commands.insert_resource(assets);
    info!("✅ Ground explosion assets loaded");
}

// ===== MAIN SPAWN FUNCTION =====

/// Spawn a complete UE5-style ground explosion
pub fn spawn_ground_explosion(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    flipbook_materials: &mut ResMut<Assets<FlipbookMaterial>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    position: Vec3,
    scale: f32,
    camera_transform: Option<&GlobalTransform>,
) {
    info!("🌋 Spawning ground explosion at {:?} (scale: {})", position, scale);

    let mut rng = rand::thread_rng();

    // Main fireball (9x9 flipbook, velocity aligned, bottom pivot)
    spawn_main_fireball(commands, assets, flipbook_materials, position, scale, &mut rng);

    // Secondary fireball (8x8 flipbook, velocity aligned, bottom pivot)
    spawn_secondary_fireball(commands, assets, flipbook_materials, position, scale, &mut rng);

    // Smoke cloud (8x8 flipbook, camera facing) - uses camera-local velocity
    spawn_smoke_cloud(commands, assets, flipbook_materials, position, scale, &mut rng, camera_transform);

    // Wisp smoke puffs (8x8 flipbook, camera facing, short duration)
    spawn_wisps(commands, assets, flipbook_materials, position, scale, &mut rng);

    // Dust ring (4x1 flipbook, velocity aligned)
    spawn_dust_ring(commands, assets, flipbook_materials, position, scale, &mut rng);

    // Sparks with gravity (single texture, velocity aligned)
    spawn_sparks(commands, assets, additive_materials, position, scale, &mut rng);

    // Bright flash sparks (single texture, velocity aligned)
    spawn_flash_sparks(commands, assets, additive_materials, position, scale, &mut rng);

    // Impact ground flash - short duration for full explosion
    spawn_impact_flash(commands, assets, flipbook_materials, additive_materials, position, scale, 0.1);

    // Dirt debris (single texture, camera facing)
    spawn_dirt_debris(commands, assets, flipbook_materials, position, scale, &mut rng);

    // Velocity-stretched dirt (single texture, velocity aligned)
    spawn_velocity_dirt(commands, assets, flipbook_materials, position, scale, &mut rng);

    info!("✅ Ground explosion spawned with 10 emitters");
}

// ===== EMITTER SPAWN FUNCTIONS =====

/// Spawn a single emitter by type (for debug testing)
pub fn spawn_single_emitter(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    flipbook_materials: &mut ResMut<Assets<FlipbookMaterial>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    emitter_type: EmitterType,
    position: Vec3,
    scale: f32,
    camera_transform: Option<&GlobalTransform>,
) {
    let mut rng = rand::thread_rng();
    match emitter_type {
        EmitterType::MainFireball => spawn_main_fireball(commands, assets, flipbook_materials, position, scale, &mut rng),
        EmitterType::SecondaryFireball => spawn_secondary_fireball(commands, assets, flipbook_materials, position, scale, &mut rng),
        EmitterType::Smoke => spawn_smoke_cloud(commands, assets, flipbook_materials, position, scale, &mut rng, camera_transform),
        EmitterType::Wisp => spawn_wisps(commands, assets, flipbook_materials, position, scale, &mut rng),
        EmitterType::Dust => spawn_dust_ring(commands, assets, flipbook_materials, position, scale, &mut rng),
        EmitterType::Spark => spawn_sparks(commands, assets, additive_materials, position, scale, &mut rng),
        EmitterType::FlashSpark => spawn_flash_sparks(commands, assets, additive_materials, position, scale, &mut rng),
        EmitterType::Impact => spawn_impact_flash(commands, assets, flipbook_materials, additive_materials, position, scale, 2.0),
        EmitterType::Dirt => spawn_dirt_debris(commands, assets, flipbook_materials, position, scale, &mut rng),
    }
}

/// Main fireball - 8x8 flipbook (64 frames), 1s duration, bottom pivot, velocity aligned
/// UE5 spec says 9x9 but actual texture is 8x8 (2048/256=8)
/// UE5: 7-13 particles, cone velocity 90°, size 2500-2600 (~25m), speed 450-650
/// Spawn delay: 0.05s, HSV color variation, sprite rotation 0-360°
pub fn spawn_main_fireball(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // UE5: RandomRangeInt 7-13 particles
    let count = rng.gen_range(7..=13);
    let lifetime = 1.0;
    let total_frames = 64; // 8x8 grid (texture is 2048x2048, 256px per frame)
    let frame_duration = lifetime / total_frames as f32;

    for i in 0..count {
        // UE5: Uniform Sprite Size 2500-2600 (in cm) -> ~25-26m, scale down for Bevy
        let size = rng.gen_range(20.0..26.0) * scale;

        // UE5: SphereLocation radius 50 units -> 0.5m scaled
        // Spawn within sphere (not just XZ plane)
        let sphere_radius = 0.5 * scale;
        let spawn_theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let spawn_phi = rng.gen_range(0.0..std::f32::consts::PI);
        let spawn_r = rng.gen_range(0.0..sphere_radius);
        let spawn_offset = Vec3::new(
            spawn_r * spawn_phi.sin() * spawn_theta.cos(),
            spawn_r * spawn_phi.cos().abs() * 0.5, // Bias toward ground
            spawn_r * spawn_phi.sin() * spawn_theta.sin(),
        );

        // UE5: AddVelocityInCone - 90° cone pointing up (Z=1), speed 450-650
        // Convert to hemisphere distribution
        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi = rng.gen_range(0.0..std::f32::consts::FRAC_PI_2); // 0-90° from vertical
        let speed = rng.gen_range(4.5..6.5) * scale; // UE5 450-650 cm/s -> 4.5-6.5 m/s

        let velocity = Vec3::new(
            phi.sin() * theta.cos() * speed,
            phi.cos() * speed, // Mostly upward
            phi.sin() * theta.sin() * speed,
        );

        // UE5: HSV color variation
        // Hue shift: ±0.1 (±10%), Saturation: 0.8-1.0, Value: 0.8-1.0
        let hue_shift = rng.gen_range(-0.1..0.1);
        let saturation = rng.gen_range(0.8..1.0);
        let value = rng.gen_range(0.8..1.0);
        let (r, g, b) = hsv_to_rgb(0.08 + hue_shift, saturation, value); // Base hue ~0.08 (orange)

        // UE5: Alpha Scale Range 0.8-1.0
        let alpha = rng.gen_range(0.8..1.0);

        // UE5: Sprite Rotation Angle 0-360°
        let rotation_angle = rng.gen_range(0.0..std::f32::consts::TAU);

        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(0.0, 0.0, 8.0, 8.0), // 8x8 grid
            color_data: Vec4::new(r, g, b, alpha),
            sprite_texture: assets.main_texture.clone(),
        });

        // UE5: Spawn delay 0.05s - start with negative elapsed time
        let spawn_delay = 0.05;

        commands.spawn((
            Mesh3d(assets.bottom_pivot_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + spawn_offset).with_scale(Vec3::splat(size)),
            Visibility::Hidden, // Start hidden until spawn delay passes
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 8,
                rows: 8,
                total_frames,
                frame_duration,
                elapsed: -spawn_delay, // Negative elapsed = spawn delay
                lifetime: 0.0,
                max_lifetime: lifetime,
                base_alpha: alpha,
                loop_animation: false,
            },
            VelocityAligned { velocity, gravity: 0.0 },
            SpriteRotation { angle: rotation_angle },
            BottomPivot,
            GroundExplosionChild,
            Name::new(format!("GE_MainFireball_{}", i)),
        ));
    }
}

/// Secondary fireball - 8x8 flipbook (64 frames), 1s duration
/// UE5: 5-10 particles, cone velocity 90°, size 2500-2600, speed 450-650
/// Spawn delay: 0.05s, HSV color variation, sprite rotation 0-360°
pub fn spawn_secondary_fireball(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // UE5: RandomRangeInt 5-10 particles
    let count = rng.gen_range(5..=10);
    let lifetime = 1.0;
    let total_frames = 64; // 8x8 grid (different texture than main)
    let frame_duration = lifetime / total_frames as f32;

    for i in 0..count {
        // UE5: Uniform Sprite Size 2500-2600
        let size = rng.gen_range(20.0..26.0) * scale;

        // UE5: SphereLocation radius 50 units -> 0.5m scaled
        // Spawn within sphere (not just XZ plane)
        let sphere_radius = 0.5 * scale;
        let spawn_theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let spawn_phi = rng.gen_range(0.0..std::f32::consts::PI);
        let spawn_r = rng.gen_range(0.0..sphere_radius);
        let spawn_offset = Vec3::new(
            spawn_r * spawn_phi.sin() * spawn_theta.cos(),
            spawn_r * spawn_phi.cos().abs() * 0.5, // Bias toward ground
            spawn_r * spawn_phi.sin() * spawn_theta.sin(),
        );

        // UE5: AddVelocityInCone - 90° cone, speed 450-650
        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi = rng.gen_range(0.0..std::f32::consts::FRAC_PI_2);
        let speed = rng.gen_range(4.5..6.5) * scale;

        let velocity = Vec3::new(
            phi.sin() * theta.cos() * speed,
            phi.cos() * speed,
            phi.sin() * theta.sin() * speed,
        );

        // UE5: HSV color variation
        // Hue shift: ±0.1 (±10%), Saturation: 0.8-1.0, Value: 0.8-1.0
        let hue_shift = rng.gen_range(-0.1..0.1);
        let saturation = rng.gen_range(0.8..1.0);
        let value = rng.gen_range(0.8..1.0);
        let (r, g, b) = hsv_to_rgb(0.08 + hue_shift, saturation, value); // Base hue ~0.08 (orange)

        // UE5: Alpha Scale Range 0.8-1.0
        let alpha = rng.gen_range(0.8..1.0);

        // UE5: Sprite Rotation Angle 0-360°
        let rotation_angle = rng.gen_range(0.0..std::f32::consts::TAU);

        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(0.0, 0.0, 8.0, 8.0),
            color_data: Vec4::new(r, g, b, alpha),
            sprite_texture: assets.secondary_texture.clone(),
        });

        // UE5: Spawn delay 0.05s - start with negative elapsed time
        let spawn_delay = 0.05;

        commands.spawn((
            Mesh3d(assets.bottom_pivot_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + spawn_offset).with_scale(Vec3::splat(size)),
            Visibility::Hidden, // Start hidden until spawn delay passes
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 8,
                rows: 8,
                total_frames,
                frame_duration,
                elapsed: -spawn_delay, // Negative elapsed = spawn delay
                lifetime: 0.0,
                max_lifetime: lifetime,
                base_alpha: alpha,
                loop_animation: false,
            },
            VelocityAligned { velocity, gravity: 0.0 },
            SpriteRotation { angle: rotation_angle },
            BottomPivot,
            GroundExplosionChild,
            Name::new(format!("GE_SecondaryFireball_{}", i)),
        ));
    }
}

/// Smoke cloud - 8x8 flipbook (35 frames used), camera facing (Unaligned)
/// UE5: 10-15 particles, LOCAL SPACE velocity (spreads on camera plane)
/// Velocity: random box ±800 XY (camera plane), +10 Z (toward camera) | Drag: 2.0 | Acceleration: +50 Z
/// Scale: grows 2-3x over lifetime (ease-out curve)
/// Alpha: fades linearly over lifetime
///
/// IMPORTANT: UE5 uses bLocalSpace=True, meaning velocity XY is relative to emitter orientation.
/// Since the emitter typically faces the camera, XY spread is on the SCREEN PLANE (not world ground).
/// This creates a more view-friendly spread pattern where smoke expands left/right and up/down
/// relative to the camera view, rather than spreading on the world ground plane.
pub fn spawn_smoke_cloud(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
    camera_transform: Option<&GlobalTransform>,
) {
    // UE5: UniformRangedInt 10-15 particles
    let count = rng.gen_range(10..=15);

    // Get camera orientation for local-space velocity calculation
    // If no camera provided, fall back to world-space (spreads on XZ plane)
    let (cam_right, cam_up, cam_forward) = if let Some(cam_tf) = camera_transform {
        let forward = cam_tf.forward().as_vec3();
        let right = cam_tf.right().as_vec3();
        let up = cam_tf.up().as_vec3();
        (right, up, forward)
    } else {
        // Fallback: assume camera looking along -Z
        (Vec3::X, Vec3::Y, Vec3::NEG_Z)
    };

    for i in 0..count {
        // UE5: RandomRangeFloat002 size 50-100 cm -> 0.5-1.0m base size (will grow 3x)
        let base_size = rng.gen_range(0.5..1.0) * scale;

        // UE5: RandomRangeFloat 1-4 for lifetime variation
        let particle_lifetime: f32 = rng.gen_range(1.0..4.0);
        // Play 35 frames over the particle's lifetime
        let frame_duration = particle_lifetime / 35.0;

        // UE5: UniformRangedVector velocity in LOCAL SPACE
        // Min: (800, 800, 0), Max: (-800, -800, 10)
        // Local X = camera right (spread left/right on screen)
        // Local Y = camera up (spread up/down on screen)
        // Local Z = camera forward (toward/away from camera - minimal)
        let local_x = rng.gen_range(-8.0..8.0) * scale;  // UE5 ±800 cm -> ±8m (screen left/right)
        let local_y = rng.gen_range(-8.0..8.0) * scale;  // UE5 ±800 cm (screen up/down)
        let local_z = rng.gen_range(0.0..0.1) * scale;   // UE5 0-10 (toward camera - minimal)

        // Transform local velocity to world space
        let velocity = cam_right * local_x + cam_up * local_y + cam_forward * local_z;

        // UE5: AccelerationForce.Acceleration (0, 0, 50) - slowly rises in LOCAL Z
        // In local space, Z is camera forward, but for visual effect we want world Y (up)
        // so smoke rises upward regardless of camera orientation
        let acceleration = Vec3::new(0.0, 0.5 * scale, 0.0);  // World up

        // UE5: InitializeParticle.Sprite Rotation Angle 0-360°
        let rotation_angle = rng.gen_range(0.0..std::f32::consts::TAU);

        // Spawn at impact center
        let spawn_offset = Vec3::new(0.0, 0.1 * scale, 0.0);

        // UE5 M_Smoke material:
        // - ParticleColor.RGB × Texture.RGB = BaseColor
        // - ParticleColor.A × Texture.A = Opacity (with depth fade)
        // - Color comes from Niagara ColorFromCurve, starts at RGB(0.4), A=0.6
        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(0.0, 0.0, 8.0, 8.0),
            color_data: Vec4::new(0.4, 0.4, 0.4, 0.6),  // Initial: medium grey, 60% opacity
            sprite_texture: assets.smoke_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + spawn_offset)
                .with_scale(Vec3::splat(base_size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 8,
                rows: 8,
                total_frames: 35, // UE5 uses 35 frames
                frame_duration,
                elapsed: 0.0,
                lifetime: 0.0,
                max_lifetime: particle_lifetime,
                base_alpha: 0.6,  // Initial alpha from color curve
                loop_animation: false,
            },
            SmokePhysics {
                velocity,
                acceleration,
                drag: 2.0,  // UE5: Drag 2.0
            },
            SmokeScaleOverLife {
                initial_size: base_size,
            },
            SmokeColorOverLife,  // Animate color over lifetime
            SpriteRotation { angle: rotation_angle },  // Random rotation preserved during billboarding
            CameraFacing,
            GroundExplosionChild,
            Name::new(format!("GE_Smoke_{}", i)),
        ));
    }
}

/// Wisp - single large billboard that plays 64-frame animation once then fades
/// UE5: Single fading smoke blob, size 80-180 cm, plays once
pub fn spawn_wisps(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // Single billboard wisp
    let count = 1;
    // Animation plays once over ~1 second then fades
    let lifetime = 1.0;
    let frame_duration = lifetime / 64.0;  // Play all 64 frames once

    for i in 0..count {
        // UE5: RandomRangeFloat002 80-180 for size - make it bigger
        let size = rng.gen_range(6.0..10.0) * scale;  // Larger billboard

        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(0.0, 0.0, 8.0, 8.0),
            color_data: Vec4::new(0.85, 0.85, 0.85, 0.8),  // Light gray, slightly transparent
            sprite_texture: assets.wisp_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + Vec3::Y * 1.0 * scale).with_scale(Vec3::splat(size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 8,
                rows: 8,
                total_frames: 64,
                frame_duration,
                elapsed: 0.0,
                lifetime: 0.0,
                max_lifetime: lifetime,  // Play once then fade
                base_alpha: 0.8,
                loop_animation: false,  // Play once
            },
            // Stationary, camera-facing
            CameraFacing,
            GroundExplosionChild,
            Name::new(format!("GE_Wisp_{}", i)),
        ));
    }
}

/// Dust ring - 4x1 flipbook (4 frames), velocity aligned
/// UE5: 2-3 particles, AddVelocityInCone 35° upward, speed 500-1000, size 300-500 cm
/// Short lifetime (0.1-0.5s), animation plays once fast
/// Billboards are almost vertical (velocity-aligned pointing up) - barely see the face
pub fn spawn_dust_ring(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // UE5: UniformRangedInt 2-3 particles
    let count = rng.gen_range(2..=3);
    // UE5: RandomRangeFloat 0.1-0.5 for lifetime - animation plays once fast
    let lifetime = rng.gen_range(0.1..0.5);
    let frame_duration = lifetime / 4.0;  // 4 frames over short lifetime

    for i in 0..count {
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        // UE5: Size 300-500 cm -> 3-5m
        let size = rng.gen_range(3.0..5.0) * scale;

        // UE5: AddVelocityInCone - 35° cone pointing up, speed 500-1000
        // Almost vertical velocity so billboards are nearly edge-on to camera
        let cone_angle = 35.0_f32.to_radians();
        let phi = rng.gen_range(0.0..cone_angle);  // 0-35° from vertical
        let speed = rng.gen_range(5.0..10.0) * scale;  // 500-1000 cm/s

        let velocity = Vec3::new(
            phi.sin() * angle.cos() * speed,
            phi.cos() * speed,  // Mostly upward - makes billboard almost vertical
            phi.sin() * angle.sin() * speed,
        );

        let offset = Vec3::new(
            angle.cos() * 0.5 * scale,
            0.1 * scale,
            angle.sin() * 0.5 * scale,
        );

        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(0.0, 0.0, 4.0, 1.0), // col, row, columns (4), rows (1)
            color_data: Vec4::new(0.0, 0.0, 0.0, 1.0), // Black dust
            sprite_texture: assets.dust_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.bottom_pivot_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + offset).with_scale(Vec3::splat(size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 4,
                rows: 1,
                total_frames: 4,
                frame_duration,
                elapsed: 0.0,
                lifetime: 0.0,
                max_lifetime: lifetime,
                base_alpha: 0.8,
                loop_animation: false,  // Play once
            },
            VelocityAligned { velocity, gravity: 0.0 },  // No gravity - fast upward motion
            BottomPivot,
            GroundExplosionChild,
            Name::new(format!("GE_Dust_{}", i)),
        ));
    }
}

/// Sparks - single texture embers with gravity, velocity aligned
pub fn spawn_sparks(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<AdditiveMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = 15;
    let lifetime = 2.0;

    for i in 0..count {
        let size = rng.gen_range(0.15..0.45) * scale;  // 1.5x size

        // Random outward velocity with upward bias
        let theta: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi: f32 = rng.gen_range(0.2..1.2);
        let speed: f32 = rng.gen_range(8.0..15.0) * scale;

        let velocity = Vec3::new(
            phi.sin() * theta.cos() * speed,
            phi.cos() * speed + rng.gen_range(2.0..5.0) * scale,
            phi.sin() * theta.sin() * speed,
        );

        let material = materials.add(AdditiveMaterial {
            tint_color: Vec4::new(1.0, 0.8, 0.3, 1.0), // Orange-yellow
            soft_particles_fade: Vec4::new(1.0, 0.0, 0.0, 0.0),
            particle_texture: assets.flare_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + Vec3::Y * scale).with_scale(Vec3::splat(size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 1,
                rows: 1,
                total_frames: 1,
                frame_duration: lifetime,
                elapsed: 0.0,
                lifetime: 0.0,
                max_lifetime: lifetime,
                base_alpha: 1.0,
                loop_animation: true,  // Single frame, doesn't matter
            },
            VelocityAligned { velocity, gravity: 8.0 },
            GroundExplosionChild,
            Name::new(format!("GE_Spark_{}", i)),
        ));
    }
}

/// Flash sparks - bright quick sparks, 1s duration
pub fn spawn_flash_sparks(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<AdditiveMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = 10;
    let lifetime = 1.0;

    for i in 0..count {
        let size = rng.gen_range(0.225..0.6) * scale;  // 1.5x size

        let theta: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi: f32 = rng.gen_range(0.3..1.0);
        let speed: f32 = rng.gen_range(10.0..20.0) * scale;

        let velocity = Vec3::new(
            phi.sin() * theta.cos() * speed,
            phi.cos() * speed,
            phi.sin() * theta.sin() * speed,
        );

        let material = materials.add(AdditiveMaterial {
            tint_color: Vec4::new(1.0, 1.0, 0.9, 1.0), // Bright white-yellow
            soft_particles_fade: Vec4::new(1.0, 0.0, 0.0, 0.0),
            particle_texture: assets.flare_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + Vec3::Y * scale).with_scale(Vec3::splat(size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 1,
                rows: 1,
                total_frames: 1,
                frame_duration: lifetime,
                elapsed: 0.0,
                lifetime: 0.0,
                max_lifetime: lifetime,
                base_alpha: 1.0,
                loop_animation: true,  // Single frame, doesn't matter
            },
            VelocityAligned { velocity, gravity: 5.0 },
            GroundExplosionChild,
            Name::new(format!("GE_FlashSpark_{}", i)),
        ));
    }
}

/// Impact flash - tilted ground flash with impact texture + point light + glow ring
/// UE5: Dual renderer - sprite + light renderer with RadiusScale 35.0
/// lifetime parameter allows short flash for full explosion (0.1s) vs longer for debug (2.0s)
pub fn spawn_impact_flash(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    flipbook_materials: &mut ResMut<Assets<FlipbookMaterial>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    position: Vec3,
    scale: f32,
    lifetime: f32,
) {

    // Point light - illuminates nearby geometry
    // UE5: RadiusScale 35.0, color bound to particle (orange/yellow)
    let light_radius = 35.0 * scale;
    let light_intensity = 80000.0 * scale; // Bright initial flash

    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.7, 0.3), // Orange/yellow flame color
            intensity: light_intensity,
            range: light_radius,
            shadows_enabled: false, // Performance: no shadows for explosion lights
            ..default()
        },
        Transform::from_translation(position + Vec3::Y * 1.0 * scale),
        ImpactLight {
            lifetime: 0.0,
            max_lifetime: lifetime,
            base_intensity: light_intensity,
        },
        GroundExplosionChild,
        Name::new("GE_ImpactLight"),
    ));

    // Glow circle sprite - visible glowing ring effect (camera-facing, additive)
    // This creates the visible "ring" effect that UE5's sprite renderer produces
    let glow_size = 15.0 * scale; // Large visible glow
    let glow_material = additive_materials.add(AdditiveMaterial {
        tint_color: Vec4::new(1.0, 0.8, 0.4, 1.0), // Orange/yellow glow
        soft_particles_fade: Vec4::new(1.0, 0.0, 0.0, 0.0),
        particle_texture: assets.glow_circle_texture.clone(),
    });

    commands.spawn((
        Mesh3d(assets.centered_quad.clone()),
        MeshMaterial3d(glow_material),
        Transform::from_translation(position + Vec3::Y * 0.5 * scale)
            .with_scale(Vec3::splat(glow_size)),
        Visibility::Visible,
        NotShadowCaster,
        NotShadowReceiver,
        FlipbookSprite {
            columns: 1,
            rows: 1,
            total_frames: 1,
            frame_duration: lifetime,
            elapsed: 0.0,
            lifetime: 0.0,
            max_lifetime: lifetime,
            base_alpha: 1.0,
            loop_animation: true,  // Single frame, doesn't matter
        },
        CameraFacing, // Face the camera for visibility
        GroundExplosionChild,
        Name::new("GE_GlowCircle"),
    ));

    // Impact texture - tilted ground-facing billboard
    // UE5: Material M_Impact_3, Sprite Size 50-100, VelocityAligned
    let impact_size = 8.0 * scale; // UE5: 50-100 cm -> ~0.5-1m, scaled up
    let impact_material = flipbook_materials.add(FlipbookMaterial {
        frame_data: Vec4::new(0.0, 0.0, 1.0, 1.0),
        color_data: Vec4::new(1.0, 0.9, 0.7, 1.0), // Slight orange tint
        sprite_texture: assets.impact_texture.clone(),
    });

    // Tilt 70° from vertical (more horizontal, facing up)
    let tilt_angle = 70.0_f32.to_radians();
    let tilt_rotation = Quat::from_rotation_x(tilt_angle);

    commands.spawn((
        Mesh3d(assets.centered_quad.clone()),
        MeshMaterial3d(impact_material),
        Transform::from_translation(position + Vec3::Y * 0.15 * scale)
            .with_rotation(tilt_rotation)
            .with_scale(Vec3::splat(impact_size)),
        Visibility::Visible,
        NotShadowCaster,
        NotShadowReceiver,
        FlipbookSprite {
            columns: 1,
            rows: 1,
            total_frames: 1,
            frame_duration: lifetime,
            elapsed: 0.0,
            lifetime: 0.0,
            max_lifetime: lifetime,
            base_alpha: 1.0,
            loop_animation: true,  // Single frame, doesn't matter
        },
        GroundExplosionChild,
        Name::new("GE_ImpactFlash"),
    ));
}

/// Dirt debris - billboard dirt chunks, velocity aligned (rotated to face travel direction)
pub fn spawn_dirt_debris(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = 8;
    let lifetime = 1.0;

    for i in 0..count {
        let size = rng.gen_range(0.45..1.2) * scale;  // 1.5x size

        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let speed = rng.gen_range(4.0..8.0) * scale;

        let velocity = Vec3::new(
            theta.cos() * speed,
            rng.gen_range(5.0..10.0) * scale,
            theta.sin() * speed,
        );

        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(0.0, 0.0, 1.0, 1.0),
            color_data: Vec4::new(0.6, 0.5, 0.4, 1.0), // Brown dirt
            sprite_texture: assets.dirt_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.bottom_pivot_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position).with_scale(Vec3::splat(size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 1,
                rows: 1,
                total_frames: 1,
                frame_duration: lifetime,
                elapsed: 0.0,
                lifetime: 0.0,
                max_lifetime: lifetime,
                base_alpha: 1.0,
                loop_animation: true,  // Single frame, doesn't matter
            },
            VelocityAligned { velocity, gravity: 12.0 },
            BottomPivot,
            GroundExplosionChild,
            Name::new(format!("GE_Dirt_{}", i)),
        ));
    }
}

/// Velocity-stretched dirt - debris that stretches along velocity
pub fn spawn_velocity_dirt(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = 6;
    let lifetime = 1.0;

    for i in 0..count {
        let size = rng.gen_range(0.6..1.35) * scale;  // 1.5x size

        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let speed = rng.gen_range(6.0..12.0) * scale;

        let velocity = Vec3::new(
            theta.cos() * speed,
            rng.gen_range(8.0..15.0) * scale,
            theta.sin() * speed,
        );

        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(0.0, 0.0, 1.0, 1.0),
            color_data: Vec4::new(0.5, 0.4, 0.3, 1.0),
            sprite_texture: assets.dirt_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + Vec3::Y * 0.5 * scale).with_scale(Vec3::splat(size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 1,
                rows: 1,
                total_frames: 1,
                frame_duration: lifetime,
                elapsed: 0.0,
                lifetime: 0.0,
                max_lifetime: lifetime,
                base_alpha: 1.0,
                loop_animation: true,  // Single frame, doesn't matter
            },
            VelocityAligned { velocity, gravity: 10.0 },
            GroundExplosionChild,
            Name::new(format!("GE_VelDirt_{}", i)),
        ));
    }
}

// ===== ANIMATION SYSTEMS =====

/// Update flipbook sprite animations and lifetime
/// Handles spawn delay via negative elapsed time - particles start hidden and become visible
pub fn animate_flipbook_sprites(
    mut query: Query<(
        &mut FlipbookSprite,
        &MeshMaterial3d<FlipbookMaterial>,
        &mut Visibility,
        Option<&Name>,
        Option<&SmokeScaleOverLife>,  // Detect smoke for linear alpha fade
    )>,
    mut materials: ResMut<Assets<FlipbookMaterial>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    for (mut sprite, material_handle, mut visibility, name, is_smoke) in query.iter_mut() {
        sprite.elapsed += dt;

        // Handle spawn delay: negative elapsed means particle is waiting to spawn
        if sprite.elapsed < 0.0 {
            *visibility = Visibility::Hidden;
            continue;
        }

        // Make visible once spawn delay is over
        if *visibility == Visibility::Hidden {
            *visibility = Visibility::Visible;
        }

        // Only count lifetime after spawn delay
        sprite.lifetime += dt;

        // Calculate current frame - respect loop_animation flag
        let raw_frame = (sprite.elapsed / sprite.frame_duration) as u32;
        let frame = if sprite.loop_animation {
            raw_frame % sprite.total_frames
        } else {
            // Clamp to last frame if not looping
            raw_frame.min(sprite.total_frames - 1)
        };
        let col = frame % sprite.columns;
        let row = frame / sprite.columns;

        // Debug: log frame progression for main fireball
        if let Some(n) = name {
            if n.as_str().contains("MainFireball") && frame % 10 == 0 {
                trace!("🎞️ {} frame={} (col={}, row={}) elapsed={:.2}s", n, frame, col, row, sprite.elapsed);
            }
        }

        // Update material frame data
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.frame_data.x = col as f32;
            material.frame_data.y = row as f32;

            // Smoke color is handled by update_smoke_color system (has SmokeColorOverLife)
            // Other particles: fade out in last 20% of lifetime
            if is_smoke.is_none() {
                let progress = sprite.lifetime / sprite.max_lifetime;
                let alpha = if progress > 0.8 {
                    1.0 - (progress - 0.8) * 5.0
                } else {
                    1.0
                };
                material.color_data.w = (sprite.base_alpha * alpha).max(0.0);
            }
        }
    }
}

/// Update velocity-aligned billboards
/// Also handles SpriteRotation for velocity-aligned particles
pub fn update_velocity_aligned_billboards(
    mut query: Query<(
        &mut Transform,
        &mut VelocityAligned,
        Option<&BottomPivot>,
        Option<&SpriteRotation>,
        Option<&FlipbookSprite>,
    )>,
    camera_query: Query<&GlobalTransform, With<Camera>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    let camera_pos = camera_query
        .iter()
        .next()
        .map(|t| t.translation())
        .unwrap_or(Vec3::ZERO);

    for (mut transform, mut vel_aligned, bottom_pivot, sprite_rotation, flipbook) in query.iter_mut() {
        // Skip particles still in spawn delay
        if let Some(fb) = flipbook {
            if fb.elapsed < 0.0 {
                continue;
            }
        }

        // Apply gravity
        vel_aligned.velocity.y -= vel_aligned.gravity * dt;

        // Apply velocity to position
        transform.translation += vel_aligned.velocity * dt;

        // Calculate rotation to align with velocity
        let velocity_dir = vel_aligned.velocity.normalize_or_zero();

        let base_rotation = if velocity_dir.length_squared() > 0.001 {
            // For velocity-aligned sprites, the up-axis points along velocity
            // and the sprite faces the camera
            let up = velocity_dir;
            let to_camera = (camera_pos - transform.translation).normalize_or_zero();

            // Project to_camera onto the plane perpendicular to up
            let forward = (to_camera - up * to_camera.dot(up)).normalize_or_zero();

            if forward.length_squared() > 0.001 {
                let right = up.cross(forward).normalize();
                let corrected_forward = right.cross(up);
                Some(Quat::from_mat3(&Mat3::from_cols(right, up, corrected_forward)))
            } else {
                None
            }
        } else {
            // Fallback to camera-facing when velocity is near zero
            let to_camera = (camera_pos - transform.translation).normalize_or_zero();
            if to_camera.length_squared() > 0.001 {
                let forward = to_camera;
                let right = Vec3::Y.cross(forward).normalize_or_zero();
                if right.length_squared() > 0.001 {
                    let up = forward.cross(right);
                    Some(Quat::from_mat3(&Mat3::from_cols(right, up, forward)))
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Apply base rotation with optional sprite rotation
        if let Some(base_rot) = base_rotation {
            if let Some(sprite_rot) = sprite_rotation {
                // Apply sprite rotation around the velocity axis (local Y for velocity-aligned)
                let local_rotation = Quat::from_rotation_y(sprite_rot.angle);
                transform.rotation = base_rot * local_rotation;
            } else {
                transform.rotation = base_rot;
            }
        }
    }
}

/// Update camera-facing billboards
/// Applies SpriteRotation AFTER computing camera-facing orientation
pub fn update_camera_facing_billboards(
    mut query: Query<(&mut Transform, Option<&SpriteRotation>), (With<CameraFacing>, With<GroundExplosionChild>)>,
    camera_query: Query<&GlobalTransform, With<Camera>>,
) {
    let camera_pos = camera_query
        .iter()
        .next()
        .map(|t| t.translation())
        .unwrap_or(Vec3::ZERO);

    for (mut transform, sprite_rotation) in query.iter_mut() {
        let to_camera = (camera_pos - transform.translation).normalize_or_zero();

        if to_camera.length_squared() > 0.001 {
            let forward = to_camera;
            let right = Vec3::Y.cross(forward).normalize_or_zero();
            if right.length_squared() > 0.001 {
                let up = forward.cross(right);
                let billboard_rotation = Quat::from_mat3(&Mat3::from_cols(right, up, forward));

                // Apply sprite rotation AFTER billboarding (rotation around the forward/Z axis)
                if let Some(sprite_rot) = sprite_rotation {
                    let local_rotation = Quat::from_rotation_z(sprite_rot.angle);
                    transform.rotation = billboard_rotation * local_rotation;
                } else {
                    transform.rotation = billboard_rotation;
                }
            }
        }
    }
}

/// Cleanup expired ground explosion particles
pub fn cleanup_ground_explosions(
    mut commands: Commands,
    query: Query<(Entity, &FlipbookSprite), With<GroundExplosionChild>>,
) {
    for (entity, sprite) in query.iter() {
        if sprite.lifetime >= sprite.max_lifetime {
            commands.entity(entity).despawn();
        }
    }
}

/// Update smoke particle physics - velocity with drag and acceleration
pub fn update_smoke_physics(
    mut query: Query<(&mut Transform, &mut SmokePhysics), With<GroundExplosionChild>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    for (mut transform, mut physics) in query.iter_mut() {
        // Apply acceleration
        let accel = physics.acceleration;
        physics.velocity += accel * dt;

        // Apply drag (exponential decay)
        // UE5 drag formula: velocity *= exp(-drag * dt)
        let drag_factor = (-physics.drag * dt).exp();
        physics.velocity *= drag_factor;

        // Update position
        transform.translation += physics.velocity * dt;
    }
}

/// Update smoke scale over lifetime - grows 2-3x using ease-out curve
/// UE5: scale(t) = initial_size * (1.0 + 2.0 * ease_out(t))
/// where ease_out(t) = 1 - (1-t)²
pub fn update_smoke_scale(
    mut query: Query<(&mut Transform, &FlipbookSprite, &SmokeScaleOverLife), With<GroundExplosionChild>>,
) {
    for (mut transform, sprite, scale_over_life) in query.iter_mut() {
        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // Ease-out curve: 1 - (1-t)²
        let ease_out = 1.0 - (1.0 - t) * (1.0 - t);

        // Scale grows from 1x to 3x over lifetime
        let scale_factor = 1.0 + 2.0 * ease_out;
        let new_size = scale_over_life.initial_size * scale_factor;

        transform.scale = Vec3::splat(new_size);
    }
}

/// Update smoke color over lifetime - UE5 Niagara ColorFromCurve
/// Color curve: t=0.0: RGB(0.4), A=0.6 → t=1.0: RGB(0.2), A=0.0
/// Smoke starts medium grey, darkens slightly as it fades out
pub fn update_smoke_color(
    query: Query<(&FlipbookSprite, &MeshMaterial3d<FlipbookMaterial>), (With<GroundExplosionChild>, With<SmokeColorOverLife>)>,
    mut materials: ResMut<Assets<FlipbookMaterial>>,
) {
    for (sprite, material_handle) in query.iter() {
        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // Color curve keyframes (approximate from UE5):
        // t=0.0: RGB=0.4, A=0.6
        // t=0.3: RGB=0.3, A=0.5
        // t=0.7: RGB=0.25, A=0.3
        // t=1.0: RGB=0.2, A=0.0

        // Linear interpolation through keyframes
        let (rgb, alpha) = if t < 0.3 {
            // 0.0 -> 0.3
            let local_t = t / 0.3;
            let rgb = 0.4 - 0.1 * local_t;    // 0.4 -> 0.3
            let a = 0.6 - 0.1 * local_t;      // 0.6 -> 0.5
            (rgb, a)
        } else if t < 0.7 {
            // 0.3 -> 0.7
            let local_t = (t - 0.3) / 0.4;
            let rgb = 0.3 - 0.05 * local_t;   // 0.3 -> 0.25
            let a = 0.5 - 0.2 * local_t;      // 0.5 -> 0.3
            (rgb, a)
        } else {
            // 0.7 -> 1.0
            let local_t = (t - 0.7) / 0.3;
            let rgb = 0.25 - 0.05 * local_t;  // 0.25 -> 0.2
            let a = 0.3 * (1.0 - local_t);    // 0.3 -> 0.0
            (rgb, a)
        };

        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.color_data.x = rgb;
            material.color_data.y = rgb;
            material.color_data.z = rgb;
            material.color_data.w = alpha;
        }
    }
}

/// Update additive material alpha for sparks and glow effects
pub fn animate_additive_sprites(
    mut query: Query<(
        &mut FlipbookSprite,
        &MeshMaterial3d<AdditiveMaterial>,
    ), With<GroundExplosionChild>>,
    mut materials: ResMut<Assets<AdditiveMaterial>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    for (mut sprite, material_handle) in query.iter_mut() {
        // Update lifetime
        sprite.lifetime += dt;

        // Fade out in last 30% of lifetime
        let progress = sprite.lifetime / sprite.max_lifetime;
        let fade = if progress > 0.7 {
            1.0 - (progress - 0.7) * 3.33
        } else {
            1.0
        };

        if let Some(material) = materials.get_mut(&material_handle.0) {
            // Apply base_alpha multiplied by fade
            material.tint_color.w = (sprite.base_alpha * fade).max(0.0);
        }
    }
}

/// Update impact lights - fade intensity over lifetime
pub fn update_impact_lights(
    mut commands: Commands,
    mut query: Query<(Entity, &mut ImpactLight, &mut PointLight)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    for (entity, mut impact_light, mut point_light) in query.iter_mut() {
        impact_light.lifetime += dt;

        let progress = impact_light.lifetime / impact_light.max_lifetime;

        if progress >= 1.0 {
            // Despawn expired lights
            commands.entity(entity).despawn();
            continue;
        }

        // Fast initial fade, then slower decay
        // UE5 light curve: bright flash then quick falloff
        let fade = if progress < 0.1 {
            // First 10%: full brightness
            1.0
        } else if progress < 0.3 {
            // 10-30%: quick fade to 30%
            1.0 - (progress - 0.1) * 3.5
        } else {
            // 30-100%: slow fade to 0
            0.3 * (1.0 - (progress - 0.3) / 0.7)
        };

        point_light.intensity = impact_light.base_intensity * fade.max(0.0);
    }
}

// ===== DEBUG MENU SYSTEM =====

/// Debug menu system - P to toggle, 1-9 to spawn individual emitters
pub fn ground_explosion_debug_menu_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut debug_menu: ResMut<GroundExplosionDebugMenu>,
    mut commands: Commands,
    ground_assets: Option<Res<GroundExplosionAssets>>,
    mut flipbook_materials: ResMut<Assets<FlipbookMaterial>>,
    mut additive_materials: ResMut<Assets<AdditiveMaterial>>,
    camera_query: Query<&GlobalTransform, With<Camera>>,
) {
    // P key toggles the debug menu
    if keyboard_input.just_pressed(KeyCode::KeyP) {
        debug_menu.active = !debug_menu.active;
        if debug_menu.active {
            info!("🔧 Ground Explosion Debug Menu ACTIVE");
            info!("   F1: Main Fireball (8x8 flipbook)");
            info!("   F2: Secondary Fireball (8x8 flipbook)");
            info!("   F3: Smoke Cloud (8x8 flipbook)");
            info!("   F4: Wisps (8x8 flipbook)");
            info!("   F5: Dust Ring (4x1 flipbook)");
            info!("   F6: Sparks (additive)");
            info!("   F7: Flash Sparks (additive)");
            info!("   F8: Impact Flash (additive)");
            info!("   F9: Dirt Debris");
            info!("   F10: FULL EXPLOSION (all emitters)");
            info!("   P: Close menu");
        } else {
            info!("🔧 Ground Explosion Debug Menu CLOSED");
        }
        return;
    }

    // Only process F keys when menu is active
    if !debug_menu.active {
        return;
    }

    let Some(assets) = ground_assets else {
        if keyboard_input.any_just_pressed([
            KeyCode::F1, KeyCode::F2, KeyCode::F3,
            KeyCode::F4, KeyCode::F5, KeyCode::F6,
            KeyCode::F7, KeyCode::F8, KeyCode::F9,
        ]) {
            warn!("Ground explosion assets not loaded yet!");
        }
        return;
    };

    let position = Vec3::new(0.0, 0.0, 0.0);
    let scale = 1.0;

    let emitter = if keyboard_input.just_pressed(KeyCode::F1) {
        Some((EmitterType::MainFireball, "Main Fireball"))
    } else if keyboard_input.just_pressed(KeyCode::F2) {
        Some((EmitterType::SecondaryFireball, "Secondary Fireball"))
    } else if keyboard_input.just_pressed(KeyCode::F3) {
        Some((EmitterType::Smoke, "Smoke Cloud"))
    } else if keyboard_input.just_pressed(KeyCode::F4) {
        Some((EmitterType::Wisp, "Wisps"))
    } else if keyboard_input.just_pressed(KeyCode::F5) {
        Some((EmitterType::Dust, "Dust Ring"))
    } else if keyboard_input.just_pressed(KeyCode::F6) {
        Some((EmitterType::Spark, "Sparks"))
    } else if keyboard_input.just_pressed(KeyCode::F7) {
        Some((EmitterType::FlashSpark, "Flash Sparks"))
    } else if keyboard_input.just_pressed(KeyCode::F8) {
        Some((EmitterType::Impact, "Impact Flash"))
    } else if keyboard_input.just_pressed(KeyCode::F9) {
        Some((EmitterType::Dirt, "Dirt Debris"))
    } else {
        None
    };

    // Get camera transform for local-space velocity calculation
    let camera_transform = camera_query.iter().next();

    if let Some((emitter_type, name)) = emitter {
        spawn_single_emitter(
            &mut commands,
            &assets,
            &mut flipbook_materials,
            &mut additive_materials,
            emitter_type,
            position,
            scale,
            camera_transform,
        );
        info!("🌋 Spawned: {} at (0, 0, 0)", name);
    }

    // F10 spawns the combined full explosion effect
    if keyboard_input.just_pressed(KeyCode::F10) {
        spawn_ground_explosion(
            &mut commands,
            &assets,
            &mut flipbook_materials,
            &mut additive_materials,
            position,
            1.5,  // Use 1.5x scale for combined effect
            camera_transform,
        );
        info!("🌋 Spawned: FULL GROUND EXPLOSION at (0, 0, 0)");
    }
}

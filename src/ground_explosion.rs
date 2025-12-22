// UE5 Niagara-style ground explosion with flipbook billboards
// Ported from NS_Explosion_Sand_5

use bevy::prelude::*;
use bevy::audio::{AudioPlayer, PlaybackSettings};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, NotShadowCaster, NotShadowReceiver};
use bevy::render::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::render::render_resource::{AsBindGroup, BlendState, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError};
use bevy::asset::RenderAssetUsages;
use bevy_hanabi::{ParticleEffect, EffectMaterial};
use rand::Rng;

use crate::wfx_materials::AdditiveMaterial;
use crate::particles::{ExplosionParticleEffects, spawn_ground_explosion_gpu_sparks, spawn_ground_explosion_gpu_dirt, spawn_ground_explosion_gpu_fireballs};

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
    #[uniform(2)]
    pub uv_scale: f32, // UV zoom: 1.0 = full texture, >1 = zoomed into center (UE5: 500→1)
    #[texture(3, dimension = "2d")]
    #[sampler(4)]
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
        // Standard alpha blending
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
    #[allow(dead_code)] // Used for documentation/future use
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
    pub drag: f32,  // Velocity decay per second (0.0 = no drag)
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

/// Dirt particle physics - velocity with gravity and drag (no acceleration)
/// UE5: GravityForce -980 cm/s², Drag 2.0
#[derive(Component)]
pub struct DirtPhysics {
    pub velocity: Vec3,
    pub gravity: f32,
    pub drag: f32,
}

/// Smoke scale-over-life component - grows 2-3x using ease-out curve
#[derive(Component)]
pub struct SmokeScaleOverLife {
    pub initial_size: f32,
}

/// Fireball scale-over-life component - UE5 Value_Scale_Factor_FloatCurve
/// Actual UE5 curve data:
/// - (t=0.0, value=0.5) → (t=1.0, value=2.0)
/// - Cubic interpolation with tangent ~3.2 (fast initial growth, eases out)
/// LUT samples: t=0.0→0.5, t=0.2→1.0, t=0.5→1.55, t=0.8→1.87, t=1.0→2.0
#[derive(Component)]
pub struct FireballScaleOverLife {
    pub initial_size: f32,
}

/// UE5 UV zoom component - animates uv_scale from 500→1 over lifetime
/// This creates a "zooming out" effect where more texture becomes visible over time
/// LUT samples: t=0.0→500, t=0.2→466, t=0.4→350, t=0.6→224, t=0.8→100, t=1.0→1
#[derive(Component)]
pub struct FireballUVZoom;

/// Marker for fireball S-curve alpha fade (replaces hold-then-fade)
/// UE5 LUT: t=0→1.0, t=0.2→0.77, t=0.4→0.56, t=0.6→0.33, t=0.8→0.14, t=1.0→0.0
#[derive(Component)]
pub struct FireballAlphaCurve;

/// Dirt scale-over-life component - UE5 curves from dirt.md:
/// - Scale: Linear shrink 100→0 over lifetime
/// - Size XY: X stretches (1→2), Y compresses (1→0.5) - "flattening" effect
/// - Alpha: Fast fade-in (0→2.0 in first 10%), slow fade-out (2.0→0 in remaining 90%)
#[derive(Component)]
pub struct DirtScaleOverLife {
    pub initial_size: f32,
    pub base_scale_x: f32,  // Random initial X scale
    pub base_scale_y: f32,  // Random initial Y scale
}

/// Dirt001 scale-over-life component - UE5 curves from dirt001.md:
/// - Scale: Linear GROWTH 1→2 (opposite of dirt!)
/// - Alpha: Same as dirt (fast fade-in 10%, slow fade-out 90%)
/// - No XY flattening effect
#[derive(Component)]
pub struct Dirt001ScaleOverLife {
    pub initial_size: f32,
    pub base_scale_x: f32,
    pub base_scale_y: f32,
}

/// Dust scale-over-life component - UE5 curves from dust.md:
/// - Scale XY: Linear growth from 0 to (3×, 2×) - X grows faster than Y
/// - Alpha: 3.0→0 linear fade (starts very bright)
/// - Color: Constant dark brown (0.147, 0.114, 0.070)
#[derive(Component)]
pub struct DustScaleOverLife {
    pub initial_size: f32,
    pub base_scale_x: f32,
    pub base_scale_y: f32,
}

/// Wisp scale-over-life component - UE5 curves from wisp.md:
/// - Scale: 0→5× cubic ease-in growth
/// - Alpha: 3.0→1.0 in 10%, then 1.0→0 linear fade
/// - Color: Constant dark brown (0.105, 0.080, 0.056)
/// - Has combined velocity (up then down arc motion)
#[derive(Component)]
pub struct WispScaleOverLife {
    pub initial_size: f32,
}

/// Wisp physics - two-phase velocity creating arc motion
/// Wisp physics - upward launch with gravity pulling down
#[derive(Component)]
pub struct WispPhysics {
    pub velocity: Vec3,
    pub gravity: f32,
}

/// Smoke color-over-life - color darkens and alpha fades over lifetime
/// Based on typical UE5 Niagara ColorFromCurve:
/// t=0.0: RGB(0.4), A=0.6 | t=0.3: RGB(0.3), A=0.5 | t=0.7: RGB(0.25), A=0.3 | t=1.0: RGB(0.2), A=0.0
#[derive(Component)]
pub struct SmokeColorOverLife;

/// Spark HDR color over life - "cooling ember" effect
/// UE5: HDR emissive (50/27/7.6) → dim red (1/0.05/0) over 55%, then fades
/// Includes per-particle random phase for flickering
#[derive(Component)]
pub struct SparkColorOverLife {
    pub random_phase: f32, // 0-TAU for flickering offset
}

/// Flash spark (spark_l) constant HDR orange with alpha fade
/// UE5: Constant HDR (10/6.5/3.9), alpha fades 1→0
/// Also has "shooting star" XY scale effect
#[derive(Component)]
pub struct SparkLColorOverLife {
    pub random_phase: f32,   // 0-TAU for flickering offset
    pub initial_size: f32,   // Base size for XY scale curve
}

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
#[allow(dead_code)] // Reserved for future parent entity tracking
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

/// Parts debris physics - 3D mesh particles with gravity and ground collision
/// UE5 parts.md: 50-75 mesh debris, gravity -980, 1 bounce with friction
#[derive(Component)]
pub struct PartsPhysics {
    pub velocity: Vec3,
    pub angular_velocity: Vec3,  // Rotation speed (radians/sec per axis)
    pub gravity: f32,
    pub bounce_count: u32,       // Track number of bounces
    pub ground_y: f32,           // Ground level for collision
}

/// Parts scale over life - holds at 1.0 until 90%, then shrinks to 0
/// UE5: Scale curve 0→1 (grow), hold at 0.9, then 1→0 (shrink to nothing)
#[derive(Component)]
pub struct PartsScaleOverLife {
    pub initial_size: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
}

// ===== DEBUG MENU =====

/// Debug menu state for ground explosion emitter testing
#[derive(Resource, Default)]
pub struct GroundExplosionDebugMenu {
    pub active: bool,
}

/// Marker component for the debug menu UI text
#[derive(Component)]
pub struct GroundExplosionDebugUI;

/// Emitter types for individual testing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitterType {
    MainFireball,      // 1
    SecondaryFireball, // 2
    Dirt,              // 3
    VelocityDirt,      // 4
    Dust,              // 5
    Wisp,              // 6
    Smoke,             // 7
    Spark,             // 8 (both sparks)
    FlashSpark,        // 8 (both sparks)
    Parts,             // 9
    #[allow(dead_code)]
    Impact,            // (internal, spawned with full explosion)
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
    // Debris meshes for parts emitter (3 variants)
    pub debris_meshes: [Handle<Mesh>; 3],
    pub debris_material: Handle<StandardMaterial>,
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
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    info!("🌋 Loading ground explosion assets...");

    // Create 3 debris mesh variants using Bevy's primitive shapes
    // UE5 parts.md: 3 mesh variants randomly selected, size 5-7 units
    let debris_meshes = [
        // Variant 0: Small cube (chunky rock)
        meshes.add(Cuboid::new(1.0, 0.8, 0.6)),
        // Variant 1: Flat slab (debris piece)
        meshes.add(Cuboid::new(1.2, 0.4, 0.8)),
        // Variant 2: Elongated piece (shrapnel)
        meshes.add(Cuboid::new(0.5, 0.5, 1.4)),
    ];

    // Debris material - dark brown/grey unlit look
    // UE5: Color range (0.26, 0.17, 0.05) brown to (0.44, 0.42, 0.42) grey
    let debris_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.30, 0.24), // Mid brown-grey
        perceptual_roughness: 1.0,
        metallic: 0.0,
        ..default()
    });

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
        debris_meshes,
        debris_material,
    };

    commands.insert_resource(assets);
    info!("✅ Ground explosion assets loaded");
}

// ===== MAIN SPAWN FUNCTION =====

/// Spawn a complete UE5-style ground explosion
/// If `gpu_effects` and `current_time` are provided, uses GPU particles for sparks (better performance)
pub fn spawn_ground_explosion(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    flipbook_materials: &mut ResMut<Assets<FlipbookMaterial>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    position: Vec3,
    scale: f32,
    camera_transform: Option<&GlobalTransform>,
    audio_assets: Option<&crate::types::AudioAssets>,
    gpu_effects: Option<&ExplosionParticleEffects>,
    current_time: Option<f64>,
) {
    info!("🌋 Spawning ground explosion at {:?} (scale: {})", position, scale);

    let mut rng = rand::thread_rng();

    // Play random explosion sound
    if let Some(audio) = audio_assets {
        let sound = audio.get_random_ground_explosion_sound(&mut rng);
        commands.spawn((
            AudioPlayer::new(sound),
            PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(crate::constants::VOLUME_EXPLOSION)),
        ));
    }

    // Fireballs - use GPU if available, otherwise CPU
    if let (Some(effects), Some(time)) = (gpu_effects, current_time) {
        // GPU fireballs: 1 entity instead of ~16-30 CPU entities
        // (main: 9-17, secondary: 7-13)
        spawn_ground_explosion_gpu_fireballs(commands, effects, position, scale, time);
    } else {
        // Fallback to CPU fireballs
        // Main fireball (9x9 flipbook, velocity aligned, bottom pivot)
        spawn_main_fireball(commands, assets, flipbook_materials, position, scale, &mut rng);
        // Secondary fireball (8x8 flipbook, velocity aligned, bottom pivot)
        spawn_secondary_fireball(commands, assets, flipbook_materials, position, scale, &mut rng);
    }

    // Smoke cloud (8x8 flipbook, camera facing) - uses camera-local velocity
    spawn_smoke_cloud(commands, assets, flipbook_materials, position, scale, &mut rng, camera_transform);

    // Wisp smoke puffs (8x8 flipbook, camera facing, short duration)
    spawn_wisps(commands, assets, flipbook_materials, position, scale, &mut rng);

    // Dust ring (4x1 flipbook, velocity aligned)
    spawn_dust_ring(commands, assets, flipbook_materials, position, scale, &mut rng);

    // Sparks and parts - use GPU if available, otherwise CPU
    if let (Some(effects), Some(time)) = (gpu_effects, current_time) {
        // GPU particles: 3 entities instead of 100-185 CPU entities
        // (sparks: 30-60, flash_sparks: 20-50, parts: 50-75)
        spawn_ground_explosion_gpu_sparks(commands, effects, position, scale, time);
    } else {
        // Fallback to CPU particles
        spawn_sparks(commands, assets, additive_materials, position, scale, &mut rng);
        spawn_flash_sparks(commands, assets, additive_materials, position, scale, &mut rng);
        spawn_parts(commands, assets, position, scale, &mut rng);
    }

    // Impact ground flash - short duration for full explosion
    spawn_impact_flash(commands, assets, flipbook_materials, additive_materials, position, scale, 0.1);

    // Dirt debris - use GPU if available, otherwise CPU
    if let (Some(effects), Some(time)) = (gpu_effects, current_time) {
        // GPU dirt: 2 entities instead of ~45-50 CPU entities
        // (dirt_debris: 35, velocity_dirt: 10-15)
        spawn_ground_explosion_gpu_dirt(commands, effects, position, scale, time);
    } else {
        // Fallback to CPU dirt particles
        spawn_dirt_debris(commands, assets, flipbook_materials, position, scale, &mut rng);
        spawn_velocity_dirt(commands, assets, flipbook_materials, position, scale, &mut rng);
    }

    let gpu_type = if gpu_effects.is_some() { "GPU" } else { "CPU" };
    info!("✅ Ground explosion spawned with {} sparks/parts/dirt/fireballs", gpu_type);
}

/// ABLATION TEST: GPU particles + selected CPU emitters
/// Use this to isolate which CPU emitters cause the FPS drop
pub fn spawn_ground_explosion_gpu_only(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    flipbook_materials: &mut ResMut<Assets<FlipbookMaterial>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    gpu_effects: &ExplosionParticleEffects,
    position: Vec3,
    scale: f32,
    current_time: f64,
) {
    let mut rng = rand::thread_rng();

    // GPU particles (sparks, flash sparks, parts debris)
    spawn_ground_explosion_gpu_sparks(commands, gpu_effects, position, scale, current_time);

    // GPU dirt emitters (replaces CPU dirt_debris + velocity_dirt)
    spawn_ground_explosion_gpu_dirt(commands, gpu_effects, position, scale, current_time);

    // GPU fireballs (replaces CPU main + secondary fireball)
    spawn_ground_explosion_gpu_fireballs(commands, gpu_effects, position, scale, current_time);

    // CPU emitters for ablation test: dust + impact
    spawn_dust_ring(commands, assets, flipbook_materials, position, scale, &mut rng);
    spawn_impact_flash(commands, assets, flipbook_materials, additive_materials, position, scale, 0.1);

    info!("🧪 ABLATION: GPU sparks/parts/dirt/fireballs + dust/impact at {:?}", position);
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
        EmitterType::Parts => spawn_parts(commands, assets, position, scale, &mut rng),
        EmitterType::VelocityDirt => spawn_velocity_dirt(commands, assets, flipbook_materials, position, scale, &mut rng),
    }
}

// =============================================================================
// SIMPLE FIREBALL VARIANTS (no UV zoom - useful for other effects)
// =============================================================================
// These are the original implementations before adding UE5-accurate UV zoom.
// They're simpler and can be useful for other explosion effects.

/// Simple main fireball - 8x8 flipbook, bottom pivot, velocity aligned
/// No UV zoom effect - shows full texture from start
/// Useful for generic explosion effects
#[allow(dead_code)]
pub fn spawn_simple_main_fireball(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = rng.gen_range(9..=17);
    let lifetime = 1.5;
    let total_frames = 64;
    let frame_duration = lifetime / total_frames as f32;

    for i in 0..count {
        let size = rng.gen_range(14.0..18.0) * scale;

        // Spawn within sphere
        let sphere_radius = 0.5 * scale;
        let spawn_theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let spawn_phi = rng.gen_range(0.0..std::f32::consts::PI);
        let spawn_r = rng.gen_range(0.0..sphere_radius);
        let spawn_offset = Vec3::new(
            spawn_r * spawn_phi.sin() * spawn_theta.cos(),
            spawn_r * spawn_phi.cos().abs() * 0.5,
            spawn_r * spawn_phi.sin() * spawn_theta.sin(),
        );

        // Cone velocity
        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi = rng.gen_range(0.0..std::f32::consts::FRAC_PI_2);
        let speed = rng.gen_range(3.0..5.0) * scale;

        let velocity = Vec3::new(
            phi.sin() * theta.cos() * speed,
            phi.cos() * speed,
            phi.sin() * theta.sin() * speed,
        );

        // HSV color variation
        let hue_shift = rng.gen_range(-0.1..0.1);
        let saturation = rng.gen_range(0.8..1.0);
        let value = rng.gen_range(0.8..1.0);
        let (r, g, b) = hsv_to_rgb(0.08 + hue_shift, saturation, value);

        let alpha = rng.gen_range(0.8..1.0);
        let rotation_angle = rng.gen_range(0.0..std::f32::consts::TAU);

        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(0.0, 0.0, 8.0, 8.0),
            color_data: Vec4::new(r, g, b, alpha),
            uv_scale: 1.0, // No zoom for simple variant
            sprite_texture: assets.main_texture.clone(),
        });

        let spawn_delay = 0.05;

        commands.spawn((
            Mesh3d(assets.bottom_pivot_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + spawn_offset).with_scale(Vec3::splat(size)),
            Visibility::Hidden,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 8,
                rows: 8,
                total_frames,
                frame_duration,
                elapsed: -spawn_delay,
                lifetime: 0.0,
                max_lifetime: lifetime,
                base_alpha: alpha,
                loop_animation: false,
            },
            VelocityAligned { velocity, gravity: 0.0, drag: 0.0 },
            SpriteRotation { angle: rotation_angle },
            FireballScaleOverLife { initial_size: size },
            BottomPivot,
            GroundExplosionChild,
            Name::new(format!("GE_SimpleMainFireball_{}", i)),
        ));
    }
}

/// Simple secondary fireball - 8x8 flipbook, bottom pivot, velocity aligned
/// No UV zoom effect - shows full texture from start
/// Useful for generic explosion effects
#[allow(dead_code)]
pub fn spawn_simple_secondary_fireball(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    let count = rng.gen_range(7..=13);
    let lifetime = 1.5;
    let total_frames = 64;
    let frame_duration = lifetime / total_frames as f32;

    for i in 0..count {
        let size = rng.gen_range(14.0..18.0) * scale;

        // Spawn within sphere
        let sphere_radius = 0.5 * scale;
        let spawn_theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let spawn_phi = rng.gen_range(0.0..std::f32::consts::PI);
        let spawn_r = rng.gen_range(0.0..sphere_radius);
        let spawn_offset = Vec3::new(
            spawn_r * spawn_phi.sin() * spawn_theta.cos(),
            spawn_r * spawn_phi.cos().abs() * 0.5,
            spawn_r * spawn_phi.sin() * spawn_theta.sin(),
        );

        // Cone velocity
        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi = rng.gen_range(0.0..std::f32::consts::FRAC_PI_2);
        let speed = rng.gen_range(3.0..5.0) * scale;

        let velocity = Vec3::new(
            phi.sin() * theta.cos() * speed,
            phi.cos() * speed,
            phi.sin() * theta.sin() * speed,
        );

        // HSV color variation
        let hue_shift = rng.gen_range(-0.1..0.1);
        let saturation = rng.gen_range(0.8..1.0);
        let value = rng.gen_range(0.8..1.0);
        let (r, g, b) = hsv_to_rgb(0.08 + hue_shift, saturation, value);

        let alpha = rng.gen_range(0.8..1.0);
        let rotation_angle = rng.gen_range(0.0..std::f32::consts::TAU);

        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(0.0, 0.0, 8.0, 8.0),
            color_data: Vec4::new(r, g, b, alpha),
            uv_scale: 1.0, // No zoom for simple variant
            sprite_texture: assets.secondary_texture.clone(),
        });

        let spawn_delay = 0.05;

        commands.spawn((
            Mesh3d(assets.bottom_pivot_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position + spawn_offset).with_scale(Vec3::splat(size)),
            Visibility::Hidden,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 8,
                rows: 8,
                total_frames,
                frame_duration,
                elapsed: -spawn_delay,
                lifetime: 0.0,
                max_lifetime: lifetime,
                base_alpha: alpha,
                loop_animation: false,
            },
            VelocityAligned { velocity, gravity: 0.0, drag: 0.0 },
            SpriteRotation { angle: rotation_angle },
            FireballScaleOverLife { initial_size: size },
            BottomPivot,
            GroundExplosionChild,
            Name::new(format!("GE_SimpleSecondaryFireball_{}", i)),
        ));
    }
}

// =============================================================================
// UE5-ACCURATE FIREBALL (with UV zoom effect)
// =============================================================================

/// Main fireball - 8x8 flipbook (64 frames), 1s duration, bottom pivot, velocity aligned
/// UE5 spec says 9x9 but actual texture is 8x8 (2048/256=8)
/// UE5: 7-13 particles, cone velocity 90°, size 2500-2600 (~25m), speed 450-650
/// Spawn delay: 0.05s, HSV color variation, sprite rotation 0-360°
/// NOW WITH: UV zoom effect (500→1) and S-curve alpha fade
pub fn spawn_main_fireball(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // UE5: RandomRangeInt 7-13 particles, scaled 1.3× for fuller appearance
    let count = rng.gen_range(9..=17);
    // UE5: 1.0s, extended to 1.5s for longer linger to match SFX
    let lifetime = 1.5;
    let total_frames = 64; // 8x8 grid (texture is 2048x2048, 256px per frame)
    let frame_duration = lifetime / total_frames as f32;

    for i in 0..count {
        // UE5 spec: 2500-2600 units (25-26m base), with 0.5→2.0 scale curve = 12.5-52m final
        // PREVIOUS: 20-26m base × 0.5→2.0 = 10-52m final
        // Scaled down to better match other emitters that were scaled up for visibility
        // Now: 14-18m base × 0.5→2.0 = 7-36m final (middle ground)
        let size = rng.gen_range(14.0..18.0) * scale;

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
        // Reduced velocity for less vertical stretch (was 4.5-6.5)
        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi = rng.gen_range(0.0..std::f32::consts::FRAC_PI_2); // 0-90° from vertical
        let speed = rng.gen_range(3.0..5.0) * scale; // Reduced from 4.5-6.5 m/s

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
            uv_scale: 1.0, // UV zoom disabled - UE5's 500→1 doesn't translate directly to our shader
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
            VelocityAligned { velocity, gravity: 0.0, drag: 0.0 },
            SpriteRotation { angle: rotation_angle },
            FireballScaleOverLife { initial_size: size },
            // FireballUVZoom disabled - needs investigation of UE5's UV scale behavior
            FireballAlphaCurve, // UE5 S-curve alpha fade (not hold-then-fade)
            BottomPivot,
            GroundExplosionChild,
            Name::new(format!("GE_MainFireball_{}", i)),
        ));
    }
}

/// Secondary fireball - 8x8 flipbook (64 frames), 1s duration
/// UE5: 5-10 particles, cone velocity 90°, size 2500-2600, speed 450-650
/// Spawn delay: 0.05s, HSV color variation, sprite rotation 0-360°
/// NOW WITH: UV zoom effect (500→1) and S-curve alpha fade
pub fn spawn_secondary_fireball(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // UE5: RandomRangeInt 5-10 particles, scaled 1.3× for fuller appearance
    let count = rng.gen_range(7..=13);
    // UE5: 1.0s, extended to 1.5s for longer linger to match SFX
    let lifetime = 1.5;
    let total_frames = 64; // 8x8 grid (different texture than main)
    let frame_duration = lifetime / total_frames as f32;

    for i in 0..count {
        // UE5 spec: 2500-2600 units (25-26m base), with 0.5→2.0 scale curve = 12.5-52m final
        // PREVIOUS: 20-26m base × 0.5→2.0 = 10-52m final
        // Scaled down to better match other emitters that were scaled up for visibility
        // Now: 14-18m base × 0.5→2.0 = 7-36m final (same as main fireball)
        let size = rng.gen_range(14.0..18.0) * scale;

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
        // Reduced velocity for less vertical stretch (was 4.5-6.5)
        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi = rng.gen_range(0.0..std::f32::consts::FRAC_PI_2);
        let speed = rng.gen_range(3.0..5.0) * scale; // Reduced from 4.5-6.5 m/s

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
            uv_scale: 1.0, // UV zoom disabled - UE5's 500→1 doesn't translate directly to our shader
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
            VelocityAligned { velocity, gravity: 0.0, drag: 0.0 },
            SpriteRotation { angle: rotation_angle },
            FireballScaleOverLife { initial_size: size },
            // FireballUVZoom disabled - needs investigation of UE5's UV scale behavior
            FireballAlphaCurve, // UE5 S-curve alpha fade (not hold-then-fade)
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
        // Reduced to 0.8-2.5s for faster fade-out
        let particle_lifetime: f32 = rng.gen_range(0.8..2.5);
        // Play 35 frames over the particle's lifetime
        let frame_duration = particle_lifetime / 35.0;

        // UE5: UniformRangedVector velocity in LOCAL SPACE
        // Min: (800, 800, 0), Max: (-800, -800, 10)
        // Local X = camera right (spread left/right on screen)
        // Local Y = camera up (spread up/down on screen)
        // Local Z = camera forward (toward/away from camera - minimal)
        // Scaled up 1.5× for better spread relative to explosion size
        let local_x = rng.gen_range(-12.0..12.0) * scale;  // UE5 ±800 cm -> ±12m (1.5× for spread)
        let local_y = rng.gen_range(-12.0..12.0) * scale;  // UE5 ±800 cm -> ±12m (1.5× for spread)
        let local_z = rng.gen_range(0.0..0.1) * scale;     // UE5 0-10 (toward camera - minimal)

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
            uv_scale: 1.0,
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

/// Wisp smoke - 8x8 flipbook, camera-facing billboards with arc motion
/// UE5: 3 particles, two-phase velocity (up then down), 1-3s lifetime
/// Scale: 0→5× cubic ease-in, Alpha: 3.0→1.0 in 10% then linear fade
/// Color: Constant dark brown (0.105, 0.080, 0.056)
pub fn spawn_wisps(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // Single set of 3 wisps, scaled 1.5× for visibility
    let wisp_scale = scale * 1.5;
    let count = 3;

    for i in 0..count {
        // UE5: Shorter lifetime for punchier effect (1.0-2.0s)
        let lifetime = rng.gen_range(1.0..2.0);
        let frame_duration = lifetime / 64.0;  // Play all 64 frames once

        // UE5 spec: 80-180 units (0.8-1.8m) with 1.5× scale modifier
        let size = rng.gen_range(0.8..1.8) * wisp_scale;

        // UE5: Gentle upward launch, gravity pulls down
        // Subtle motion - goes up a little, then falls
        let velocity = Vec3::new(
            rng.gen_range(-1.0..1.0) * wisp_scale,  // Horizontal spread
            rng.gen_range(3.0..6.0) * wisp_scale,   // Gentle upward launch
            rng.gen_range(-1.0..1.0) * wisp_scale,
        );

        // UE5: Sprite Rotation Angle 0-360°
        let rotation_angle = rng.gen_range(0.0..std::f32::consts::TAU);

        // UE5: Wisp smoke - using same dark grey as dust for consistency
        // Alpha starts at 3.0 (300% brightness) as per UE5 spec
        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(0.0, 0.0, 8.0, 8.0),
            color_data: Vec4::new(0.15, 0.12, 0.10, 3.0), // Dark grey-black (same as dust), 3× brightness
            uv_scale: 1.0,
            sprite_texture: assets.wisp_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            // Start at 0 scale (grows from 0 to 5×)
            Transform::from_translation(position + Vec3::Y * 0.5 * wisp_scale).with_scale(Vec3::ZERO),
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
                max_lifetime: lifetime,
                base_alpha: 1.0,
                loop_animation: false,  // Play once
            },
            // UE5: Billboard (Unaligned) with gravity like dirt001
            WispPhysics {
                velocity,
                gravity: 9.8,  // Same gravity as dirt
            },
            WispScaleOverLife { initial_size: size },
            SpriteRotation { angle: rotation_angle },
            CameraFacing,
            GroundExplosionChild,
            Name::new(format!("GE_Wisp_{}", i)),
        ));
    }
}

/// Dust ring - 4x1 flipbook (4 frames), velocity aligned
/// UE5: 2-3 particles, AddVelocityInCone 35° upward, speed 500-1000, size 300-500 cm
/// Short lifetime (0.1-0.5s), animation plays once fast
/// UE5 curves: Scale XY growth (0→3×, 0→2×), Alpha 3.0→0 linear fade
/// Color: Constant dark brown (0.147, 0.114, 0.070)
pub fn spawn_dust_ring(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // UE5: UniformRangedInt 2-3 particles (single set, no double-spawn)
    let count = rng.gen_range(2..=3);

    for i in 0..count {
        // UE5: RandomRangeFloat 0.1-0.5 for lifetime - very short
        let lifetime = rng.gen_range(0.1..0.5);

        // Random angle for velocity direction within cone
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);

        // UE5 spec: 300-500 units (3-5m)
        // With (3×, 2×) scale curve = final ~9-15m width, ~6-10m height
        let size = rng.gen_range(3.0..5.0) * scale;

        // UE5: AddVelocityInCone - 35° cone pointing up, speed 500-1000
        // Cone axis (0,0,3) = strong upward bias
        let cone_angle = 35.0_f32.to_radians();
        let phi = rng.gen_range(0.0..cone_angle);  // 0-35° from vertical
        let speed = rng.gen_range(5.0..10.0) * scale;  // 500-1000 cm/s -> 5-10 m/s

        let velocity = Vec3::new(
            phi.sin() * angle.cos() * speed,
            phi.cos() * speed,  // Mostly upward
            phi.sin() * angle.sin() * speed,
        );

        // UE5: Spawn at exact origin - NO offset
        let spawn_pos = position;

        // UE5: SubUV Animation Mode = Random - pick ONE random frame (0-3) that stays fixed
        let random_frame = rng.gen_range(0..4) as f32;

        // UE5: Dark brown color (0.147, 0.114, 0.070) from dust.md
        // Alpha starts at 3.0 (300% brightness) as per UE5 spec
        // Texture is 4×1 grid - frame is fixed at spawn (no animation)
        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(random_frame, 0.0, 4.0, 1.0), // col=random, row=0, columns (4), rows (1)
            color_data: Vec4::new(0.147, 0.114, 0.070, 3.0), // UE5 dark brown, 3× brightness
            uv_scale: 1.0,
            sprite_texture: assets.dust_texture.clone(),
        });

        commands.spawn((
            Mesh3d(assets.bottom_pivot_quad.clone()),
            MeshMaterial3d(material),
            // UE5: Scale starts at ZERO - particles grow from nothing
            Transform::from_translation(spawn_pos).with_scale(Vec3::ZERO),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 4,  // 4×1 grid texture
                rows: 1,
                total_frames: 1,  // Only 1 frame - no animation
                frame_duration: lifetime,  // Doesn't matter since no animation
                elapsed: 0.0,
                lifetime: 0.0,
                max_lifetime: lifetime,
                base_alpha: 1.0,
                loop_animation: false,
            },
            VelocityAligned { velocity, gravity: 0.0, drag: 0.0 },  // No gravity - fast upward motion
            // UE5: Scale grows from 0 - X faster than Y (3×, 2×)
            DustScaleOverLife {
                initial_size: size,
                base_scale_x: 1.0,
                base_scale_y: 1.0,
            },
            BottomPivot,
            GroundExplosionChild,
            Name::new(format!("GE_Dust_{}", i)),
        ));
    }
}

/// Sparks - flying embers with gravity and HDR color cooling curve
/// UE5 spark.md: 250-500 count, 90° cone, gravity -980, collision (1 bounce)
/// HDR color: (50/27/7.6) → (1/0.05/0) over 55%, flickering via sin()
pub fn spawn_sparks(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<AdditiveMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // UE5: 250-500 sparks. Using lower count for performance.
    // PREVIOUS VALUE: count = 15 (kept for reference if UE5 count is too heavy)
    let count = rng.gen_range(30..60);
    // UE5: 2.0s duration, lifetime 0.1-4.0s variable
    let base_lifetime = 2.0;

    for i in 0..count {
        // UE5 spec: 1-3 units (0.01-0.03m)
        // PREVIOUS: 0.5-1.2m, increased for better visibility with blast SFX
        let size = rng.gen_range(0.8..1.8) * scale;

        // UE5: 90° cone (hemisphere), cone axis (0,0,1) = upward
        // Speed: 1000-2500 units = 10-25m in Bevy scale
        let theta: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
        // phi from 0 to PI/2 for 90° hemisphere (0 = straight up, PI/2 = horizontal)
        let phi: f32 = rng.gen_range(0.0..std::f32::consts::FRAC_PI_2);
        // UE5: 1000-2500 units = 10-25m, scaled 1.5× for higher launch
        let speed: f32 = rng.gen_range(15.0..37.5) * scale;

        // Velocity falloff: faster toward center (lower phi)
        let falloff = 1.0 - (phi / std::f32::consts::FRAC_PI_2) * 0.5;
        let adjusted_speed = speed * falloff;

        let velocity = Vec3::new(
            phi.sin() * theta.cos() * adjusted_speed,
            phi.cos() * adjusted_speed,  // Y is up in Bevy
            phi.sin() * theta.sin() * adjusted_speed,
        );

        // Variable lifetime: 0.1-4.0s (UE5)
        let lifetime = rng.gen_range(0.5..base_lifetime);

        // Initial HDR color: (50, 27, 7.6) normalized for shader's 4× brightness
        // Shader does: tex.rgb * tint_color.rgb * 4.0
        // So we pass: HDR_value / 4.0 to get final HDR output
        // Initial: (50/4, 27/4, 7.6/4) = (12.5, 6.75, 1.9)
        let material = materials.add(AdditiveMaterial {
            tint_color: Vec4::new(12.5, 6.75, 1.9, 1.0), // HDR orange-yellow (pre-divided by shader's 4×)
            soft_particles_fade: Vec4::new(1.0, 0.0, 0.0, 0.0),
            particle_texture: assets.flare_texture.clone(),
        });

        // Random phase for flickering (0 to 2π)
        let random_phase = rng.gen_range(0.0..std::f32::consts::TAU);

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            // Spawn at explosion core
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
            // UE5: Gravity -980 cm/s² = 9.8 m/s²
            VelocityAligned { velocity, gravity: 9.8, drag: 0.0 },
            SparkColorOverLife { random_phase },
            GroundExplosionChild,
            Name::new(format!("GE_Spark_{}", i)),
        ));
    }
}

/// Flash sparks (spark_l) - bright ring burst with "shooting star" XY scale effect
/// UE5 spark_l.md: 100-250 count, ring spawn at sphere equator, 100° cone
/// Constant HDR orange (10/6.5/3.9), deceleration physics, XY scale curve
pub fn spawn_flash_sparks(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<AdditiveMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // UE5: 100-250 sparks. Using lower count for performance.
    // PREVIOUS VALUE: count = 10 (kept for reference)
    let count = rng.gen_range(20..50);
    // UE5: 1.0s duration, lifetime 0.1-2.0s variable
    let base_lifetime = 1.0;

    // Ring spawn radius (UE5: 5 units at sphere equator, scaled)
    // Actually UE5 uses V=0.5 which is equator of spawn sphere
    let ring_radius = 0.5 * scale;

    for i in 0..count {
        // UE5 spec: 0.05-1.0 units (very small)
        // PREVIOUS VALUE: size = rng.gen_range(0.15..0.4) * scale
        // Increased to 0.4-1.0m to better match fireball visibility
        let size = rng.gen_range(0.4..1.0) * scale;

        // Ring spawn: spawn at equator of small sphere
        // theta = angle around the ring (0 to 2π)
        let spawn_theta: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
        let spawn_offset = Vec3::new(
            spawn_theta.cos() * ring_radius,
            0.0,  // Equator = Y=0
            spawn_theta.sin() * ring_radius,
        );

        // UE5: 100° cone (wider than hemisphere), with upward bias
        // phi from 0 to ~100° (0 = straight up, 100° = slightly past horizontal)
        let max_phi = 100.0_f32.to_radians();
        let phi: f32 = rng.gen_range(0.0..max_phi);
        // Velocity direction - mostly outward from ring with upward component
        let vel_theta: f32 = rng.gen_range(0.0..std::f32::consts::TAU);

        // UE5: 400-5500 units = 4-55m, very wide range
        // PREVIOUS VALUE: speed = rng.gen_range(10.0..20.0) * scale
        let speed: f32 = rng.gen_range(4.0..55.0) * scale;

        // Velocity with falloff toward edges
        let falloff = 1.0 - (phi / max_phi) * 0.5;
        let adjusted_speed = speed * falloff;

        let velocity = Vec3::new(
            phi.sin() * vel_theta.cos() * adjusted_speed,
            phi.cos() * adjusted_speed,  // Y is up
            phi.sin() * vel_theta.sin() * adjusted_speed,
        );

        // Variable lifetime: 0.1-2.0s (UE5)
        let lifetime = rng.gen_range(0.3..base_lifetime);

        // Constant HDR orange: (10, 6.5, 3.9) normalized for shader's 4× brightness
        // (10/4, 6.5/4, 3.9/4) = (2.5, 1.625, 0.975)
        let material = materials.add(AdditiveMaterial {
            tint_color: Vec4::new(2.5, 1.625, 0.975, 1.0), // HDR orange (pre-divided by shader's 4×)
            soft_particles_fade: Vec4::new(1.0, 0.0, 0.0, 0.0),
            particle_texture: assets.flare_texture.clone(),
        });

        // Random phase for flickering (0 to 2π)
        let random_phase = rng.gen_range(0.0..std::f32::consts::TAU);

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            // Spawn at explosion core with ring offset
            Transform::from_translation(position + spawn_offset)
                .with_scale(Vec3::splat(size)),
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
            // UE5: No gravity, uses deceleration instead
            // Deceleration is handled by SparkLColorOverLife update system
            VelocityAligned { velocity, gravity: 0.0, drag: 0.0 },
            SparkLColorOverLife { random_phase, initial_size: size },
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
        uv_scale: 1.0,
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

/// Dirt debris - billboard dirt chunks (Unaligned), camera-facing with gravity
/// UE5: 35 particles, 0.1s spawn delay, velocity box XY:±500 Z:1000-1500
/// Drag: 2.0, Gravity: -980, Size: 50-100, Lifetime: 1-4s, Sprite rotation 0-360°
pub fn spawn_dirt_debris(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // UE5: 35 fixed count
    let count = 35;

    for i in 0..count {
        // UE5 spec: 50-100 units (0.5-1.0m), scaled 2× for visibility
        let size = rng.gen_range(1.0..2.0) * scale;

        // UE5: RandomRangeVector2D for non-uniform size
        // Min: (30, 200), Max: (100, 500) -> normalized to multipliers
        let base_scale_x = rng.gen_range(0.3..1.0);  // Width variation
        let base_scale_y = rng.gen_range(0.4..1.0);  // Height variation

        // UE5: UniformRangedVector - box velocity
        // X/Z: ±500 (±5m/s scaled), Y: increased to 15-25m/s for higher arc
        let velocity = Vec3::new(
            rng.gen_range(-5.0..5.0) * scale,
            rng.gen_range(15.0..25.0) * scale,  // Increased upward launch (was 10-15)
            rng.gen_range(-5.0..5.0) * scale,
        );

        // UE5: RandomRangeFloat 1.0-4.0s lifetime
        let lifetime = rng.gen_range(1.0..4.0);

        // UE5: Sprite Rotation Angle 0-360°
        let rotation_angle = rng.gen_range(0.0..std::f32::consts::TAU);

        // UE5 ColorCurve: Dark brown (0.082, 0.063, 0.050) at t=0
        // Alpha starts at 0 (handled by alpha curve in update system)
        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(0.0, 0.0, 1.0, 1.0),
            color_data: Vec4::new(0.082, 0.063, 0.050, 0.0), // Dark brown, alpha=0 (fade-in)
            uv_scale: 1.0,
            sprite_texture: assets.dirt_texture.clone(),
        });

        // UE5: Spawn delay 0.1s
        let spawn_delay = 0.1;

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),  // Billboard, not bottom-pivot
            MeshMaterial3d(material),
            Transform::from_translation(position)
                .with_scale(Vec3::new(size * base_scale_x, size * base_scale_y, size)),
            Visibility::Hidden,  // Start hidden until spawn delay passes
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 1,
                rows: 1,
                total_frames: 1,
                frame_duration: lifetime,
                elapsed: -spawn_delay,  // Negative elapsed = spawn delay
                lifetime: 0.0,
                max_lifetime: lifetime,
                base_alpha: 1.0,
                loop_animation: true,
            },
            // UE5: Billboard (Unaligned) with gravity and drag
            DirtPhysics {
                velocity,
                gravity: 9.8,  // Earth gravity (scaled from 980 cm/s²)
                drag: 2.0,
            },
            // UE5 curves: shrink, XY flattening, alpha fade-in/out
            DirtScaleOverLife {
                initial_size: size,
                base_scale_x,
                base_scale_y,
            },
            SpriteRotation { angle: rotation_angle },
            CameraFacing,
            GroundExplosionChild,
            Name::new(format!("GE_Dirt_{}", i)),
        ));
    }
}

/// Velocity-stretched dirt - debris that stretches along velocity
/// UE5 dirt001 emitter - velocity-aligned debris (streaking)
/// Unlike dirt (billboard, gravity), dirt001 creates fast streaking debris
/// that shoots outward and fades without falling
pub fn spawn_velocity_dirt(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    materials: &mut ResMut<Assets<FlipbookMaterial>>,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // UE5: 10-15 (RandomRangeInt)
    let count = rng.gen_range(10..=15);

    for i in 0..count {
        // UE5 spec: 50-100 units (0.5-1.0m), scaled 2× for visibility
        let size = rng.gen_range(1.0..2.0) * scale;

        // UE5: RandomRangeVector2D for non-uniform size
        // Min: (200, 350), Max: (400, 600) - elongated shapes
        let base_scale_x = rng.gen_range(0.5..1.0);  // Width
        let base_scale_y = rng.gen_range(0.6..1.2);  // Height (taller for velocity stretch)

        // UE5: Cone velocity - 90° cone pointing upward
        // Speed: 250-1000 (2.5-10m/s)
        let speed = rng.gen_range(2.5..10.0) * scale;

        // Generate random direction in upward cone (90° = hemisphere)
        let theta = rng.gen_range(0.0..std::f32::consts::TAU);  // Azimuth
        let phi = rng.gen_range(0.0..(std::f32::consts::PI * 0.5));  // Elevation (0-90°)

        // Cone velocity with falloff toward center (faster at center)
        let falloff = (1.0 - phi / (std::f32::consts::PI * 0.5)).powf(2.0);
        let adjusted_speed = speed * (0.5 + 0.5 * falloff);

        let velocity = Vec3::new(
            theta.cos() * phi.sin() * adjusted_speed,
            phi.cos() * adjusted_speed,  // Upward (Y in Bevy)
            theta.sin() * phi.sin() * adjusted_speed,
        );

        // UE5: RandomRangeFloat 0.8-1.7s lifetime (shorter than dirt)
        let lifetime = rng.gen_range(0.8..1.7);

        // UE5: Sprite Rotation Angle 0-360°
        let rotation_angle = rng.gen_range(0.0..std::f32::consts::TAU);

        // UE5 ColorCurve: Same dark brown as dirt (0.082, 0.063, 0.050)
        // Alpha starts at 0 (fade-in handled by Dirt001ScaleOverLife)
        let material = materials.add(FlipbookMaterial {
            frame_data: Vec4::new(0.0, 0.0, 1.0, 1.0),
            color_data: Vec4::new(0.082, 0.063, 0.050, 0.0), // Dark brown, alpha=0
            uv_scale: 1.0,
            sprite_texture: assets.dirt_texture.clone(),
        });

        // UE5: Spawn delay 0.1s (same as dirt)
        let spawn_delay = 0.1;

        commands.spawn((
            Mesh3d(assets.centered_quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position)
                .with_scale(Vec3::new(size * base_scale_x, size * base_scale_y, size)),
            Visibility::Hidden,  // Start hidden until spawn delay passes
            NotShadowCaster,
            NotShadowReceiver,
            FlipbookSprite {
                columns: 1,
                rows: 1,
                total_frames: 1,
                frame_duration: lifetime,
                elapsed: -spawn_delay,  // Negative = spawn delay
                lifetime: 0.0,
                max_lifetime: lifetime,
                base_alpha: 1.0,
                loop_animation: false,
            },
            // VelocityAligned with NO gravity (key difference from dirt)
            // UE5: High drag (2.0) decelerates quickly
            VelocityAligned { velocity, gravity: 0.0, drag: 2.0 },
            // Track scale-over-life (same alpha curve as dirt)
            Dirt001ScaleOverLife {
                initial_size: size,
                base_scale_x,
                base_scale_y,
            },
            SpriteRotation { angle: rotation_angle },
            GroundExplosionChild,
            Name::new(format!("GE_Dirt001_{}", i)),
        ));
    }
}

/// Parts debris - 3D mesh shrapnel/rock pieces flying outward with gravity
/// UE5 parts.md: 50-75 mesh debris, box velocity, gravity, 1 bounce collision
pub fn spawn_parts(
    commands: &mut Commands,
    assets: &GroundExplosionAssets,
    position: Vec3,
    scale: f32,
    rng: &mut impl Rng,
) {
    // UE5: 50-75 (RandomRangeInt)
    let count = rng.gen_range(50..75);

    for i in 0..count {
        // UE5: Size 5-7 units (0.05-0.07m)
        // Scaled to 0.3-0.5m - small enough that despawn isn't noticeable
        let size = rng.gen_range(0.3..0.5) * scale;

        // UE5: UniformRangedVector - box velocity
        // Min: (800, 800, 500), Max: (-800, -800, 2500)
        // This means X/Y: ±800, Z: 500-2500 (UE5 Z-up)
        // In Bevy (Y-up): X/Z: ±8m/s, Y: 5-25m/s
        let velocity = Vec3::new(
            rng.gen_range(-8.0..8.0) * scale,
            rng.gen_range(5.0..25.0) * scale,   // Strong upward launch
            rng.gen_range(-8.0..8.0) * scale,
        );

        // Random angular velocity for tumbling (radians/sec)
        let angular_velocity = Vec3::new(
            rng.gen_range(-10.0..10.0),
            rng.gen_range(-10.0..10.0),
            rng.gen_range(-10.0..10.0),
        );

        // UE5: RandomRangeFloat 0.5-1.5s lifetime
        let lifetime = rng.gen_range(0.5..1.5);

        // Random initial rotation
        let initial_rotation = Quat::from_euler(
            EulerRot::XYZ,
            rng.gen_range(0.0..std::f32::consts::TAU),
            rng.gen_range(0.0..std::f32::consts::TAU),
            rng.gen_range(0.0..std::f32::consts::TAU),
        );

        // Select one of 3 mesh variants randomly
        let mesh_index = rng.gen_range(0..3);

        commands.spawn((
            Mesh3d(assets.debris_meshes[mesh_index].clone()),
            MeshMaterial3d(assets.debris_material.clone()),
            Transform::from_translation(position)
                .with_rotation(initial_rotation)
                .with_scale(Vec3::splat(size)),
            Visibility::Visible,
            NotShadowCaster,
            NotShadowReceiver,
            PartsPhysics {
                velocity,
                angular_velocity,
                gravity: 9.8,   // Earth gravity (UE5: -980 cm/s²)
                bounce_count: 0,
                ground_y: position.y, // Ground level at spawn position
            },
            PartsScaleOverLife {
                initial_size: size,
                lifetime: 0.0,
                max_lifetime: lifetime,
            },
            GroundExplosionChild,
            Name::new(format!("GE_Parts_{}", i)),
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
        Option<&FireballAlphaCurve>,  // Detect fireball for S-curve alpha fade
        Option<&DustScaleOverLife>,   // Detect dust - alpha handled by update_dust_alpha
    )>,
    mut materials: ResMut<Assets<FlipbookMaterial>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    for (mut sprite, material_handle, mut visibility, name, is_smoke, is_fireball, is_dust) in query.iter_mut() {
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

            // Alpha curves differ by particle type:
            // - Smoke: handled by update_smoke_color system (has SmokeColorOverLife)
            // - Dust: handled by update_dust_alpha system (has DustScaleOverLife)
            // - Fireball: S-curve fade
            // - Others: hold then fade
            if is_smoke.is_none() && is_dust.is_none() {
                let progress = sprite.lifetime / sprite.max_lifetime;
                let alpha = if is_fireball.is_some() {
                    // UE5 fireball: S-curve fade throughout entire lifetime
                    // LUT: t=0→1.0, t=0.2→0.77, t=0.4→0.56, t=0.6→0.33, t=0.8→0.14, t=1.0→0.0
                    let s = progress * progress * (3.0 - 2.0 * progress); // smoothstep
                    1.0 - s
                } else {
                    // Other particles: hold at 1.0 until t=0.8, then cubic fade
                    if progress > 0.8 {
                        let local_t = (progress - 0.8) / 0.2;  // 0.0 → 1.0
                        1.0 - local_t * local_t  // Quadratic approximation of cubic
                    } else {
                        1.0
                    }
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

    for (mut transform, mut vel_aligned, _bottom_pivot, sprite_rotation, flipbook) in query.iter_mut() {
        // Skip particles still in spawn delay
        if let Some(fb) = flipbook {
            if fb.elapsed < 0.0 {
                continue;
            }
        }

        // Apply gravity
        vel_aligned.velocity.y -= vel_aligned.gravity * dt;

        // Apply drag (exponential decay: v = v * e^(-drag * dt))
        if vel_aligned.drag > 0.0 {
            let drag_factor = (-vel_aligned.drag * dt).exp();
            vel_aligned.velocity *= drag_factor;
        }

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

/// Update dirt particle physics - velocity with gravity and drag
/// UE5: GravityForce -980 cm/s², Drag 2.0
pub fn update_dirt_physics(
    mut query: Query<(&mut Transform, &mut DirtPhysics, Option<&FlipbookSprite>), With<GroundExplosionChild>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    for (mut transform, mut physics, flipbook) in query.iter_mut() {
        // Skip particles still in spawn delay
        if let Some(fb) = flipbook {
            if fb.elapsed < 0.0 {
                continue;
            }
        }

        // Apply gravity
        physics.velocity.y -= physics.gravity * dt;

        // Apply drag (exponential decay)
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

/// Update fireball scale over lifetime - UE5 Value_Scale_Factor_FloatCurve
/// Actual UE5 curve: (t=0.0, 0.5) → (t=1.0, 2.0) with cubic ease-out (tangent ~3.2)
/// LUT: t=0.0→0.5, t=0.1→0.76, t=0.2→1.0, t=0.3→1.21, t=0.5→1.55, t=0.8→1.87, t=1.0→2.0
pub fn update_fireball_scale(
    mut query: Query<(&mut Transform, &FlipbookSprite, &FireballScaleOverLife), With<GroundExplosionChild>>,
) {
    for (mut transform, sprite, scale_over_life) in query.iter_mut() {
        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // UE5 cubic interpolation: 0.5 → 2.0 with ease-out (tangent 3.2)
        // Using cubic ease-out: 1 - (1-t)³
        // Adjusted to 0.5 → 1.3 for less vertical stretch
        let ease = 1.0 - (1.0 - t).powi(3);
        let scale_factor = 0.5 + ease * 0.8;  // 0.5 → 1.3

        let new_size = scale_over_life.initial_size * scale_factor;
        transform.scale = Vec3::splat(new_size);
    }
}

/// Update fireball UV zoom over lifetime - UE5 UV scale curve
/// UV scale 500→1 over lifetime (smoothstep ease)
/// Creates a "zooming out" effect where more texture becomes visible
pub fn update_fireball_uv_zoom(
    mut query: Query<(&FlipbookSprite, &MeshMaterial3d<FlipbookMaterial>, &FireballUVZoom), With<GroundExplosionChild>>,
    mut materials: ResMut<Assets<FlipbookMaterial>>,
) {
    for (sprite, material_handle, _) in query.iter_mut() {
        // Skip particles in spawn delay
        if sprite.elapsed < 0.0 {
            continue;
        }

        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // UE5 UV scale: 500 → 1 with smoothstep ease
        // LUT: t=0→500, t=0.2→466, t=0.4→350, t=0.6→224, t=0.8→100, t=1.0→1
        let ease = t * t * (3.0 - 2.0 * t); // smoothstep
        let uv_scale = 500.0 - ease * 499.0; // 500 → 1

        if let Some(material) = materials.get_mut(material_handle) {
            material.uv_scale = uv_scale;
        }
    }
}

/// Update dirt scale over lifetime - UE5 curves from dirt.md:
/// - Scale: Linear shrink 100→0 over lifetime
/// - Size XY: X stretches (1→2), Y compresses (1→0.5) - "flattening" effect
pub fn update_dirt_scale(
    mut query: Query<(&mut Transform, &FlipbookSprite, &DirtScaleOverLife), With<GroundExplosionChild>>,
) {
    for (mut transform, sprite, scale_data) in query.iter_mut() {
        // Skip particles in spawn delay
        if sprite.elapsed < 0.0 {
            continue;
        }

        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // UE5: Linear shrink from 100% to 0%
        let shrink_factor = 1.0 - t;

        // UE5 Size XY curve: X stretches (1→2), Y compresses (1→0.5)
        let x_stretch = 1.0 + t;          // 1.0 → 2.0
        let y_compress = 1.0 - t * 0.5;   // 1.0 → 0.5

        // Apply all factors
        let final_size = scale_data.initial_size * shrink_factor;
        transform.scale = Vec3::new(
            final_size * scale_data.base_scale_x * x_stretch,
            final_size * scale_data.base_scale_y * y_compress,
            final_size,
        );
    }
}

/// Update dirt color/alpha over lifetime - UE5 curves from dirt.md:
/// - Alpha: Fast fade-in (0→2.0 in first 10%), slow cubic fade-out (2.0→0 in remaining 90%)
/// - Color: Very subtle shift from dark brown (0.082, 0.063, 0.050) to slightly lighter (0.109, 0.084, 0.066)
pub fn update_dirt_alpha(
    query: Query<(&FlipbookSprite, &MeshMaterial3d<FlipbookMaterial>), (With<GroundExplosionChild>, With<DirtScaleOverLife>)>,
    mut materials: ResMut<Assets<FlipbookMaterial>>,
) {
    for (sprite, material_handle) in query.iter() {
        // Skip particles in spawn delay
        if sprite.elapsed < 0.0 {
            continue;
        }

        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // UE5 Alpha curve: Fast fade-in (0→2.0 in first 10%), slow cubic fade-out (2.0→0 in remaining 90%)
        let alpha = if t < 0.1 {
            // Fast fade-in: 0 → 2.0 (clamped to 1.0 for rendering, but stored as multiplier)
            (t / 0.1 * 2.0).min(1.0)
        } else {
            // Slow cubic fade-out: peak → 0
            let local_t = (t - 0.1) / 0.9;
            (1.0 - local_t * local_t).max(0.0)
        };

        // UE5 Color curve: Very subtle brown shift
        // t=0.0: (0.082, 0.063, 0.050) → t=1.0: (0.109, 0.084, 0.066)
        let r = 0.082 + t * (0.109 - 0.082);
        let g = 0.063 + t * (0.084 - 0.063);
        let b = 0.050 + t * (0.066 - 0.050);

        if let Some(material) = materials.get_mut(material_handle.id()) {
            material.color_data = Vec4::new(r, g, b, alpha);
        }
    }
}

/// Update dirt001 scale over lifetime - UE5 curves from dirt001.md:
/// - Scale: Linear GROWTH 1→2 (opposite of dirt which shrinks!)
/// - No XY flattening effect
pub fn update_dirt001_scale(
    mut query: Query<(&mut Transform, &FlipbookSprite, &Dirt001ScaleOverLife), With<GroundExplosionChild>>,
) {
    for (mut transform, sprite, scale_data) in query.iter_mut() {
        // Skip particles in spawn delay
        if sprite.elapsed < 0.0 {
            continue;
        }

        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // UE5: Linear growth from 1.0 to 2.0 (opposite of dirt!)
        let growth_factor = 1.0 + t;

        // Apply growth factor
        let final_size = scale_data.initial_size * growth_factor;
        transform.scale = Vec3::new(
            final_size * scale_data.base_scale_x,
            final_size * scale_data.base_scale_y,
            final_size,
        );
    }
}

/// Update dirt001 color/alpha over lifetime - same alpha curve as dirt
/// - Alpha: Fast fade-in (0→2.0 in first 10%), slow cubic fade-out (2.0→0 in remaining 90%)
/// - Color: Same dark brown as dirt
pub fn update_dirt001_alpha(
    query: Query<(&FlipbookSprite, &MeshMaterial3d<FlipbookMaterial>), (With<GroundExplosionChild>, With<Dirt001ScaleOverLife>)>,
    mut materials: ResMut<Assets<FlipbookMaterial>>,
) {
    for (sprite, material_handle) in query.iter() {
        // Skip particles in spawn delay
        if sprite.elapsed < 0.0 {
            continue;
        }

        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // UE5 Alpha curve: Same as dirt - fast fade-in, slow cubic fade-out
        let alpha = if t < 0.1 {
            (t / 0.1 * 2.0).min(1.0)
        } else {
            let local_t = (t - 0.1) / 0.9;
            (1.0 - local_t * local_t).max(0.0)
        };

        // UE5 Color curve: Same dark brown as dirt
        let r = 0.082 + t * (0.109 - 0.082);
        let g = 0.063 + t * (0.084 - 0.063);
        let b = 0.050 + t * (0.066 - 0.050);

        if let Some(material) = materials.get_mut(material_handle.id()) {
            material.color_data = Vec4::new(r, g, b, alpha);
        }
    }
}

/// Update smoke color over lifetime - UE5 Niagara ColorFromCurve
/// T3D-verified smoke color and alpha curves:
/// - Color: Dark brown (0.147, 0.117, 0.089) → Tan (0.328, 0.235, 0.156)
/// - Alpha: Smoothstep bell curve 0→0.5→0 (ease-in to peak, ease-out from peak)
pub fn update_smoke_color(
    query: Query<(&FlipbookSprite, &MeshMaterial3d<FlipbookMaterial>), (With<GroundExplosionChild>, With<SmokeColorOverLife>)>,
    mut materials: ResMut<Assets<FlipbookMaterial>>,
) {
    for (sprite, material_handle) in query.iter() {
        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // T3D-verified color curve: dark brown → tan
        // Start: RGB(0.147, 0.117, 0.089) = #261D17
        // End:   RGB(0.328, 0.235, 0.156) = #543C28
        let start_color = Vec3::new(0.147, 0.117, 0.089);
        let end_color = Vec3::new(0.328, 0.235, 0.156);
        let color = start_color.lerp(end_color, t);

        // T3D-verified alpha curve: smoothstep bell curve 0→0.5→0
        // NOT a simple parabola - uses cubic S-curve for gradual fade-in/out
        // LUT: t=0→0.0, t=0.25→0.24, t=0.5→0.5, t=0.75→0.27, t=1.0→0.0
        let smoothstep = |x: f32| x * x * (3.0 - 2.0 * x);
        let alpha = if t <= 0.5 {
            // Ease-in to peak at t=0.5
            0.5 * smoothstep(t * 2.0)
        } else {
            // Ease-out from peak
            0.5 * smoothstep((1.0 - t) * 2.0)
        };

        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.color_data.x = color.x;
            material.color_data.y = color.y;
            material.color_data.z = color.z;
            material.color_data.w = alpha;
        }
    }
}

/// Update dust scale over lifetime - UE5 curves from dust.md:
/// - Scale XY: Linear growth from 0 to (3×, 2×) - X grows faster than Y
/// - Alpha: 3.0→0 linear fade (starts very bright)
/// - Color: Constant dark brown (0.147, 0.114, 0.070)
pub fn update_dust_scale(
    mut query: Query<(&mut Transform, &FlipbookSprite, &DustScaleOverLife), With<GroundExplosionChild>>,
) {
    for (mut transform, sprite, scale_data) in query.iter_mut() {
        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // UE5: Linear growth from 0 to final size
        // X grows to 3×, Y grows to 2× - starts at ZERO (intentional)
        let x_scale = t * 3.0;
        let y_scale = t * 2.0;

        transform.scale = Vec3::new(
            scale_data.initial_size * scale_data.base_scale_x * x_scale,
            scale_data.initial_size * scale_data.base_scale_y * y_scale,
            scale_data.initial_size,
        );
    }
}

/// Update dust alpha over lifetime - UE5 accurate: 3.0→0 S-curve fade
/// UE5 uses alpha > 1.0 as brightness multiplier, shader now handles this
/// LUT: t=0→3.0, t=0.2→2.59, t=0.5→1.5, t=0.8→0.34, t=1.0→0
pub fn update_dust_alpha(
    query: Query<(&FlipbookSprite, &MeshMaterial3d<FlipbookMaterial>), (With<GroundExplosionChild>, With<DustScaleOverLife>)>,
    mut materials: ResMut<Assets<FlipbookMaterial>>,
) {
    for (sprite, material_handle) in query.iter() {
        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // UE5: Alpha starts at 3.0 (300% brightness!), S-curve fade to 0
        // Smoothstep: slower at start/end, faster in middle
        let s = t * t * (3.0 - 2.0 * t); // smoothstep
        let alpha = 3.0 * (1.0 - s);

        if let Some(material) = materials.get_mut(material_handle.id()) {
            material.color_data.w = alpha;
        }
    }
}

/// Update wisp physics - apply velocity to position
pub fn update_wisp_physics(
    mut query: Query<(&mut Transform, &mut WispPhysics, Option<&FlipbookSprite>), With<GroundExplosionChild>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    for (mut transform, mut physics, flipbook) in query.iter_mut() {
        // Skip particles still in spawn delay
        if let Some(fb) = flipbook {
            if fb.elapsed < 0.0 {
                continue;
            }
        }

        // Apply gravity (like dirt001)
        physics.velocity.y -= physics.gravity * dt;

        // Apply velocity to position
        transform.translation += physics.velocity * dt;
    }
}

/// Update wisp scale over lifetime - UE5 curves from wisp.md:
/// - Scale: 0→5× cubic ease-in growth
pub fn update_wisp_scale(
    mut query: Query<(&mut Transform, &FlipbookSprite, &WispScaleOverLife), With<GroundExplosionChild>>,
) {
    for (mut transform, sprite, scale_data) in query.iter_mut() {
        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // UE5: Cubic ease-in (smoothstep variant) - faster initial growth
        let ease = t * t * (3.0 - 2.0 * t);
        let scale_factor = ease * 5.0;  // Grows to 5× base size

        let new_size = scale_data.initial_size * scale_factor;
        transform.scale = Vec3::splat(new_size.max(0.001));  // Prevent zero scale
    }
}

/// Update wisp alpha over lifetime - 4.0→1.0 fast drop, then 1.0→0 linear fade
/// Alpha > 1.0 acts as brightness multiplier in shader
pub fn update_wisp_alpha(
    query: Query<(&FlipbookSprite, &MeshMaterial3d<FlipbookMaterial>), (With<GroundExplosionChild>, With<WispScaleOverLife>)>,
    mut materials: ResMut<Assets<FlipbookMaterial>>,
) {
    for (sprite, material_handle) in query.iter() {
        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // Alpha starts at 4.0 (400% brightness), drops to 1.0 at t=0.2
        // Then fades linearly 1.0→0 over remaining 80%
        let alpha = if t < 0.2 {
            // Fast drop: 4.0 → 1.0 in first 20%
            4.0 - (t / 0.2) * 3.0
        } else {
            // Linear fade: 1.0 → 0.0 over remaining 80%
            let local_t = (t - 0.2) / 0.8;
            1.0 - local_t
        };

        if let Some(material) = materials.get_mut(material_handle.id()) {
            material.color_data.w = alpha.max(0.0);
        }
    }
}

/// Update spark HDR color over lifetime - "cooling ember" effect with flickering
/// UE5: HDR (50/27/7.6) → (1/0.05/0) over 55%, then continues fading
/// Flickering via sin(time + random_phase)
pub fn update_spark_color(
    mut query: Query<(
        &mut FlipbookSprite,
        &SparkColorOverLife,
        &MeshMaterial3d<AdditiveMaterial>,
    ), With<GroundExplosionChild>>,
    mut materials: ResMut<Assets<AdditiveMaterial>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();

    for (mut sprite, spark_color, material_handle) in query.iter_mut() {
        // Update lifetime (since animate_additive_sprites filters out sparks)
        sprite.lifetime += dt;
        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // HDR color curve (pre-divided by shader's 4× brightness)
        // UE5: (50, 27, 7.6) → (1, 0.05, 0) over 55% of lifetime
        let (r, g, b, alpha) = if t < 0.55 {
            let local_t = t / 0.55;
            // Interpolate from bright to dim
            // Starting: (50/4, 27/4, 7.6/4) = (12.5, 6.75, 1.9)
            // Ending: (1/4, 0.05/4, 0) = (0.25, 0.0125, 0)
            let r = 12.5 * (1.0 - local_t) + 0.25 * local_t;
            let g = 6.75 * (1.0 - local_t) + 0.0125 * local_t;
            let b = 1.9 * (1.0 - local_t);
            let alpha = 1.0 - local_t * 0.5;  // Slight fade during cooling
            (r, g, b, alpha)
        } else {
            // Continue fading from dim red to invisible
            let local_t = (t - 0.55) / 0.45;
            let r = 0.25 * (1.0 - local_t * 0.5);
            let g = 0.0125 * (1.0 - local_t * 0.6);
            let b = 0.0;
            let alpha = 0.5 * (1.0 - local_t);
            (r, g, b, alpha)
        };

        // Flickering: sin(time + random_phase) oscillates 0-2, normalized to 0.5-1.5
        let flicker = (elapsed * 8.0 + spark_color.random_phase).sin() * 0.25 + 1.0;

        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.tint_color = Vec4::new(
                r * flicker,
                g * flicker,
                b * flicker,
                alpha.max(0.0),
            );
        }
    }
}

/// Update flash spark (spark_l) color and "shooting star" XY scale
/// UE5: Constant HDR (10/6.5/3.9), alpha fades, XY scale creates elongation
pub fn update_spark_l_color(
    mut query: Query<(
        &mut FlipbookSprite,
        &SparkLColorOverLife,
        &MeshMaterial3d<AdditiveMaterial>,
        &mut Transform,
        &VelocityAligned,
    ), With<GroundExplosionChild>>,
    mut materials: ResMut<Assets<AdditiveMaterial>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();

    for (mut sprite, spark_l, material_handle, mut transform, _velocity_aligned) in query.iter_mut() {
        // Update lifetime (since animate_additive_sprites filters out sparks)
        sprite.lifetime += dt;
        let t = (sprite.lifetime / sprite.max_lifetime).clamp(0.0, 1.0);

        // Constant HDR color (pre-divided by shader's 4×)
        // UE5: (10/4, 6.5/4, 3.9/4) = (2.5, 1.625, 0.975)
        let (r, g, b) = (2.5, 1.625, 0.975);

        // Alpha: linear fade 1→0
        let alpha = 1.0 - t;

        // Flickering: sin(time + random_phase)
        let flicker = (elapsed * 10.0 + spark_l.random_phase).sin() * 0.2 + 1.0;

        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.tint_color = Vec4::new(
                r * flicker,
                g * flicker,
                b * flicker,
                alpha.max(0.0),
            );
        }

        // "Shooting star" XY scale effect
        // UE5: t=0: 0×0 → t=0.05: 0.3×50 → t=0.5: 5×3 → t=1.0: 5×3
        // X = width perpendicular to velocity, Y = length along velocity
        let (scale_x, scale_y) = if t < 0.05 {
            // Fast stretch to elongated shape
            let local_t = t / 0.05;
            (local_t * 0.3, local_t * 50.0)
        } else if t < 0.5 {
            // Transition to normal shape
            let local_t = (t - 0.05) / 0.45;
            let x = 0.3 + local_t * 4.7;    // 0.3 → 5.0
            let y = 50.0 - local_t * 47.0;  // 50.0 → 3.0
            (x, y)
        } else {
            // Hold final shape
            (5.0, 3.0)
        };

        // Apply XY scale - since VelocityAligned stretches along Y (velocity direction),
        // we apply the elongation along local Y
        // Scale relative to initial size, normalized so final (5, 3) gives reasonable result
        let base_size = spark_l.initial_size;
        let normalized_x = scale_x / 5.0;  // Normalize to 0-1 range at end
        let normalized_y = scale_y / 5.0;  // Normalize (50 at peak is intentionally huge)

        transform.scale = Vec3::new(
            base_size * normalized_x.max(0.01),
            base_size * normalized_y.max(0.01),
            base_size,
        );
    }
}

/// Apply deceleration to flash sparks (spark_l)
/// UE5: Acceleration (-25, -50, -100) instead of gravity
pub fn update_spark_l_physics(
    mut query: Query<&mut VelocityAligned, (With<GroundExplosionChild>, With<SparkLColorOverLife>)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    // UE5 deceleration: (-25, -50, -100) cm/s² = (-0.25, -0.5, -1.0) m/s²
    let deceleration = Vec3::new(-0.25, -1.0, -0.5);  // Y is up in Bevy

    for mut velocity_aligned in query.iter_mut() {
        // Apply constant deceleration
        velocity_aligned.velocity += deceleration * dt * 10.0;  // Scale up for visibility
    }
}

/// Update parts debris physics - gravity, rotation, and ground collision with bounce
/// UE5 parts.md: gravity -980, 1 bounce with friction 0.25, rotational drag
pub fn update_parts_physics(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &mut PartsPhysics, &PartsScaleOverLife), With<GroundExplosionChild>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    for (entity, mut transform, mut physics, scale_life) in query.iter_mut() {
        // Check if particle should despawn
        if scale_life.lifetime >= scale_life.max_lifetime {
            commands.entity(entity).despawn();
            continue;
        }

        // Apply gravity
        physics.velocity.y -= physics.gravity * dt;

        // Update position
        transform.translation += physics.velocity * dt;

        // Apply rotation (tumbling)
        let rotation_delta = Quat::from_euler(
            EulerRot::XYZ,
            physics.angular_velocity.x * dt,
            physics.angular_velocity.y * dt,
            physics.angular_velocity.z * dt,
        );
        transform.rotation = rotation_delta * transform.rotation;

        // Apply rotational drag (slow down spinning over time)
        physics.angular_velocity *= 0.99;

        // Ground collision
        if transform.translation.y < physics.ground_y && physics.bounce_count < 1 {
            // Bounce off ground
            transform.translation.y = physics.ground_y;
            physics.velocity.y = -physics.velocity.y * 0.25;  // UE5: friction 0.25
            physics.velocity.x *= 0.5;  // Horizontal friction
            physics.velocity.z *= 0.5;
            physics.bounce_count += 1;

            // Add some spin on bounce
            physics.angular_velocity *= 0.5;
        }
    }
}

/// Update parts scale over lifetime - holds at 1.0 until 90%, then shrinks to 0
/// UE5 parts.md: Scale curve 0→1→1→0 (grow, hold, shrink)
pub fn update_parts_scale(
    mut query: Query<(&mut Transform, &mut PartsScaleOverLife), With<GroundExplosionChild>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    for (mut transform, mut scale_life) in query.iter_mut() {
        scale_life.lifetime += dt;

        let t = (scale_life.lifetime / scale_life.max_lifetime).clamp(0.0, 1.0);

        // UE5 scale curve:
        // t=0.0→0.1: quick grow from 0→1
        // t=0.1→0.9: hold at 1.0
        // t=0.9→1.0: fast shrink from 1→0
        let scale_factor = if t < 0.1 {
            // Quick grow-in (0→1 in first 10%)
            t / 0.1
        } else if t < 0.9 {
            // Hold at 1.0
            1.0
        } else {
            // Fast shrink (1→0 in last 10%)
            1.0 - (t - 0.9) / 0.1
        };

        let size = scale_life.initial_size * scale_factor;
        transform.scale = Vec3::splat(size);
    }
}

/// Update additive material alpha for sparks and glow effects
/// Note: This is the generic handler for additive sprites without specific color components
pub fn animate_additive_sprites(
    mut query: Query<(
        &mut FlipbookSprite,
        &MeshMaterial3d<AdditiveMaterial>,
    ), (With<GroundExplosionChild>, Without<SparkColorOverLife>, Without<SparkLColorOverLife>)>,
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
    terrain_config: Res<crate::terrain::TerrainConfig>,
    heightmap: Res<crate::terrain::TerrainHeightmap>,
    audio_assets: Res<crate::types::AudioAssets>,
    gpu_effects: Option<Res<crate::particles::ExplosionParticleEffects>>,
    time: Res<Time>,
) {
    // P key toggles the debug menu
    if keyboard_input.just_pressed(KeyCode::KeyP) {
        debug_menu.active = !debug_menu.active;
        if debug_menu.active {
            info!("═══════════════════════════════════════");
            info!("  GROUND EXPLOSION DEBUG [P]");
            info!("═══════════════════════════════════════");
            info!("  1: main       2: main001    3: dirt");
            info!("  4: dirt001    5: dust       6: wisp");
            info!("  7: smoke      8: spark      9: spark_l");
            info!("  0: parts");
            info!("  Shift+3/4/8/9/0: GPU versions");
            info!("  J: group 1-6  K: full explosion");
            info!("  P: close");
            info!("═══════════════════════════════════════");
        } else {
            info!("Ground Explosion Debug CLOSED");
        }
        return;
    }

    // Only process keys when menu is active
    if !debug_menu.active {
        return;
    }

    let Some(assets) = ground_assets else {
        if keyboard_input.any_just_pressed([
            KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3,
            KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6,
            KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
        ]) {
            warn!("Ground explosion assets not loaded yet!");
        }
        return;
    };

    // For FirebaseDelta (map3), spawn at terrain height at offset position around the tower
    // For other maps, keep spawning at origin
    let position = if terrain_config.current_map == crate::terrain::MapPreset::FirebaseDelta {
        // Offset from center (tower is at origin) - spawn ~30-50m away
        let offset_x = 40.0;
        let offset_z = 30.0;
        let terrain_y = heightmap.sample_height(offset_x, offset_z);
        Vec3::new(offset_x, terrain_y, offset_z)
    } else {
        Vec3::new(0.0, 0.0, 0.0)
    };
    let scale = 1.0;

    // Get camera transform for local-space velocity calculation
    let camera_transform = camera_query.iter().next();

    let shift_held = keyboard_input.pressed(KeyCode::ShiftLeft) || keyboard_input.pressed(KeyCode::ShiftRight);

    // Individual emitter keys (1-7, 0 without shift for CPU parts)
    // 1, 2, 3, 4, 8, 9, 0 with shift are handled separately for GPU versions
    let emitter = if keyboard_input.just_pressed(KeyCode::Digit1) && !shift_held {
        Some((EmitterType::MainFireball, "main"))
    } else if keyboard_input.just_pressed(KeyCode::Digit2) && !shift_held {
        Some((EmitterType::SecondaryFireball, "main001"))
    } else if keyboard_input.just_pressed(KeyCode::Digit3) && !shift_held {
        Some((EmitterType::Dirt, "dirt"))
    } else if keyboard_input.just_pressed(KeyCode::Digit4) && !shift_held {
        Some((EmitterType::VelocityDirt, "dirt001"))
    } else if keyboard_input.just_pressed(KeyCode::Digit5) {
        Some((EmitterType::Dust, "dust"))
    } else if keyboard_input.just_pressed(KeyCode::Digit6) {
        Some((EmitterType::Wisp, "wisp"))
    } else if keyboard_input.just_pressed(KeyCode::Digit7) {
        Some((EmitterType::Smoke, "smoke"))
    } else if keyboard_input.just_pressed(KeyCode::Digit0) && !shift_held {
        // CPU parts (only when shift is NOT held)
        Some((EmitterType::Parts, "parts"))
    } else {
        None
    };

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
        info!("[P] Spawned: {}", name);
    }

    // 3: dirt (CPU) or Shift+3: dirt (GPU)
    if keyboard_input.just_pressed(KeyCode::Digit3) && shift_held {
        if let Some(effects) = gpu_effects.as_ref() {
            let current_time = time.elapsed_secs_f64();
            let seed = (current_time * 1000000.0) as u32;
            commands.spawn((
                ParticleEffect {
                    handle: effects.ground_dirt_effect.clone(),
                    prng_seed: Some(seed),
                },
                EffectMaterial {
                    images: vec![effects.ground_dirt_texture.clone()],
                },
                Transform::from_translation(position).with_scale(Vec3::splat(scale)),
                Visibility::Visible,
                crate::particles::ParticleEffectLifetime {
                    spawn_time: current_time,
                    duration: 5.0,
                },
                Name::new("GE_GPU_Dirt_Debug"),
            ));
            info!("[P] Spawned: dirt (GPU)");
        }
    }

    // 4: velocity dirt (CPU) or Shift+4: velocity dirt (GPU)
    if keyboard_input.just_pressed(KeyCode::Digit4) && shift_held {
        if let Some(effects) = gpu_effects.as_ref() {
            let current_time = time.elapsed_secs_f64();
            let seed = (current_time * 1000000.0) as u32;
            commands.spawn((
                ParticleEffect {
                    handle: effects.ground_vdirt_effect.clone(),
                    prng_seed: Some(seed),
                },
                EffectMaterial {
                    images: vec![effects.ground_dirt_texture.clone()],
                },
                Transform::from_translation(position).with_scale(Vec3::splat(scale)),
                Visibility::Visible,
                crate::particles::ParticleEffectLifetime {
                    spawn_time: current_time,
                    duration: 3.0,
                },
                Name::new("GE_GPU_VDirt_Debug"),
            ));
            info!("[P] Spawned: dirt001 (GPU)");
        }
    }

    // 1: main fireball (CPU) or Shift+1: fireball (GPU)
    if keyboard_input.just_pressed(KeyCode::Digit1) && shift_held {
        if let Some(effects) = gpu_effects.as_ref() {
            let current_time = time.elapsed_secs_f64();
            let seed = (current_time * 1000000.0) as u32;
            commands.spawn((
                ParticleEffect {
                    handle: effects.ground_fireball_effect.clone(),
                    prng_seed: Some(seed),
                },
                EffectMaterial {
                    images: vec![effects.ground_fireball_texture.clone()],
                },
                Transform::from_translation(position).with_scale(Vec3::splat(scale)),
                Visibility::Visible,
                crate::particles::ParticleEffectLifetime {
                    spawn_time: current_time,
                    duration: 2.0,
                },
                Name::new("GE_GPU_Fireball_Debug"),
            ));
            info!("[P] Spawned: fireball (GPU) - combined main+secondary");
        }
    }

    // 2: secondary fireball (CPU) or Shift+2: secondary fireball (GPU)
    if keyboard_input.just_pressed(KeyCode::Digit2) && shift_held {
        if let Some(effects) = gpu_effects.as_ref() {
            let current_time = time.elapsed_secs_f64();
            let seed = (current_time * 1000000.0) as u32;
            commands.spawn((
                ParticleEffect {
                    handle: effects.ground_fireball_effect.clone(),
                    prng_seed: Some(seed),
                },
                EffectMaterial {
                    images: vec![effects.ground_fireball_secondary_texture.clone()],
                },
                Transform::from_translation(position).with_scale(Vec3::splat(scale)),
                Visibility::Visible,
                crate::particles::ParticleEffectLifetime {
                    spawn_time: current_time,
                    duration: 2.0,
                },
                Name::new("GE_GPU_Secondary_Fireball"),
            ));
            info!("[P] Spawned: secondary fireball (GPU)");
        }
    }

    // 8: spark (CPU) or Shift+8: spark (GPU)
    if keyboard_input.just_pressed(KeyCode::Digit8) {
        if shift_held {
            // GPU spark (replaces CPU spawn_sparks / SparkColorOverLife)
            if let Some(effects) = gpu_effects.as_ref() {
                let current_time = time.elapsed_secs_f64();
                // Generate unique seed from current time to ensure randomization
                let seed = (current_time * 1000000.0) as u32;
                commands.spawn((
                    bevy_hanabi::ParticleEffect {
                        handle: effects.ground_sparks_effect.clone(),
                        prng_seed: Some(seed),
                    },
                    bevy_hanabi::EffectMaterial {
                        images: vec![effects.ground_sparks_texture.clone()],
                    },
                    Transform::from_translation(position).with_scale(Vec3::splat(scale)),
                    Visibility::Visible,
                    crate::particles::ParticleEffectLifetime {
                        spawn_time: current_time,
                        duration: 3.0,
                    },
                    Name::new("GE_GPU_Spark_Debug"),
                ));
                info!("[P] Spawned: spark (GPU)");
            } else {
                warn!("[P] GPU effects not available!");
            }
        } else {
            // CPU spark
            spawn_single_emitter(
                &mut commands,
                &assets,
                &mut flipbook_materials,
                &mut additive_materials,
                EmitterType::Spark,
                position,
                scale,
                camera_transform,
            );
            info!("[P] Spawned: spark (CPU)");
        }
    }

    // 9: spark_l (CPU) or Shift+9: spark_l (GPU)
    if keyboard_input.just_pressed(KeyCode::Digit9) {
        if shift_held {
            // GPU spark_l (replaces CPU spawn_flash_sparks / SparkLColorOverLife)
            if let Some(effects) = gpu_effects.as_ref() {
                let current_time = time.elapsed_secs_f64();
                // Generate unique seed from current time to ensure randomization
                let seed = (current_time * 1000000.0) as u32;
                commands.spawn((
                    bevy_hanabi::ParticleEffect {
                        handle: effects.ground_flash_sparks_effect.clone(),
                        prng_seed: Some(seed),
                    },
                    bevy_hanabi::EffectMaterial {
                        images: vec![effects.ground_sparks_texture.clone()],
                    },
                    Transform::from_translation(position).with_scale(Vec3::splat(scale)),
                    Visibility::Visible,
                    crate::particles::ParticleEffectLifetime {
                        spawn_time: current_time,
                        duration: 2.0,
                    },
                    Name::new("GE_GPU_SparkL_Debug"),
                ));
                info!("[P] Spawned: spark_l (GPU)");
            } else {
                warn!("[P] GPU effects not available!");
            }
        } else {
            // CPU spark_l
            spawn_single_emitter(
                &mut commands,
                &assets,
                &mut flipbook_materials,
                &mut additive_materials,
                EmitterType::FlashSpark,
                position,
                scale,
                camera_transform,
            );
            info!("[P] Spawned: spark_l (CPU)");
        }
    }

    // 0: parts (CPU) or Shift+0: parts (GPU)
    if keyboard_input.just_pressed(KeyCode::Digit0) && shift_held {
        // GPU parts (replaces CPU spawn_parts / PartsPhysics)
        if let Some(effects) = gpu_effects.as_ref() {
            let current_time = time.elapsed_secs_f64();
            let seed = (current_time * 1000000.0) as u32;
            commands.spawn((
                bevy_hanabi::ParticleEffect {
                    handle: effects.ground_parts_effect.clone(),
                    prng_seed: Some(seed),
                },
                bevy_hanabi::EffectMaterial {
                    images: vec![effects.ground_parts_texture.clone()],
                },
                Transform::from_translation(position).with_scale(Vec3::splat(scale)),
                Visibility::Visible,
                crate::particles::ParticleEffectLifetime {
                    spawn_time: current_time,
                    duration: 2.0,
                },
                Name::new("GE_GPU_Parts_Debug"),
            ));
            info!("[P] Spawned: parts (GPU)");
        } else {
            warn!("[P] GPU effects not available!");
        }
    }

    // J: Emitter group 1-6 (main, main001, dirt, dirt001, dust, wisp)
    if keyboard_input.just_pressed(KeyCode::KeyJ) {
        for emitter_type in [
            EmitterType::MainFireball,
            EmitterType::SecondaryFireball,
            EmitterType::Dirt,
            EmitterType::VelocityDirt,
            EmitterType::Dust,
            EmitterType::Wisp,
        ] {
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
        }
        info!("[P] Spawned: group 1-6 (main, main001, dirt, dirt001, dust, wisp)");
    }

    // K: Full explosion (all emitters)
    if keyboard_input.just_pressed(KeyCode::KeyK) {
        let current_time = time.elapsed_secs_f64();
        spawn_ground_explosion(
            &mut commands,
            &assets,
            &mut flipbook_materials,
            &mut additive_materials,
            position,
            1.0,  // Default scale
            camera_transform,
            Some(&audio_assets),
            gpu_effects.as_deref(),
            Some(current_time),
        );
        info!("[P] Spawned: FULL EXPLOSION");
    }

    // Shift+L: ABLATION - GPU + fireballs/dust/impact scatter barrage (8 explosions)
    // Check shift first to avoid triggering single explosion
    if keyboard_input.just_pressed(KeyCode::KeyL) && shift_held {
        if let Some(effects) = gpu_effects.as_ref() {
            let current_time = time.elapsed_secs_f64();
            let mut rng = rand::thread_rng();
            let scatter_radius = 30.0; // Same as artillery scatter

            for i in 0..8 {
                // Random offset within scatter radius
                let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                let dist = rng.gen_range(0.0..scatter_radius);
                let offset = Vec3::new(angle.cos() * dist, 0.0, angle.sin() * dist);
                let spawn_pos = position + offset;

                spawn_ground_explosion_gpu_only(
                    &mut commands,
                    &assets,
                    &mut flipbook_materials,
                    &mut additive_materials,
                    effects,
                    spawn_pos,
                    1.0,
                    current_time + (i as f64 * 0.001), // Tiny offset for unique seeds
                );
            }
            info!("[P] ABLATION: GPU + fireballs/dust/impact barrage (8 explosions)");
        } else {
            info!("[P] ABLATION: GPU effects not available");
        }
    } else if keyboard_input.just_pressed(KeyCode::KeyL) {
        // L: ABLATION - GPU + fireballs/dust/impact single explosion
        if let Some(effects) = gpu_effects.as_ref() {
            let current_time = time.elapsed_secs_f64();
            spawn_ground_explosion_gpu_only(
                &mut commands,
                &assets,
                &mut flipbook_materials,
                &mut additive_materials,
                effects,
                position,
                1.0,
                current_time,
            );
            info!("[P] ABLATION: GPU + fireballs/dust/impact explosion");
        } else {
            info!("[P] ABLATION: GPU effects not available");
        }
    }

    // X: Debug GPU fireball (velocity colors, no texture, small quads)
    if keyboard_input.just_pressed(KeyCode::KeyX) {
        if let Some(effects) = gpu_effects.as_ref() {
            let current_time = time.elapsed_secs_f64();
            let seed = (current_time * 1000000.0) as u32;
            commands.spawn((
                ParticleEffect {
                    handle: effects.debug_fireball_effect.clone(),
                    prng_seed: Some(seed),
                },
                // No EffectMaterial - uses vertex colors only
                Transform::from_translation(position),  // No scale - use raw 5m hemisphere
                Visibility::Visible,
                crate::particles::ParticleEffectLifetime {
                    spawn_time: current_time,
                    duration: 6.0,
                },
                Name::new("GE_GPU_Debug_Fireball"),
            ));
            info!("[P] DEBUG: velocity-colored fireball (5m hemisphere, 0.3m quads)");
        }
    }
}

/// Spawn the debug menu UI (hidden by default)
pub fn setup_ground_explosion_debug_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("GROUND EXPLOSION [P]\n─────────────────────\n1: main    2: main001\n3: dirt    4: dirt001\n5: dust    6: wisp\n7: smoke   8: spark\n9: spark_l 0: parts\n─────────────────────\nShift+3/4/8/9/0: GPU\nJ: group 1-6\nK: full explosion\nL: GPU-only (ablation)\nShift+L: GPU barrage\nX: debug velocity\nP: close"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.8, 0.2)),  // Orange/gold color
        Node {
            position_type: PositionType::Absolute,
            // WFX debug UI is at bottom: 10px with font_size 16px (~20px line height)
            // Position this just above it: 10 + 20 + 10 gap = 40px
            bottom: Val::Px(40.0),
            left: Val::Px(10.0),
            ..default()
        },
        Visibility::Hidden,
        GroundExplosionDebugUI,
    ));
}

/// Update debug menu UI visibility based on menu state
pub fn update_ground_explosion_debug_ui(
    debug_menu: Res<GroundExplosionDebugMenu>,
    mut query: Query<&mut Visibility, With<GroundExplosionDebugUI>>,
) {
    for mut visibility in query.iter_mut() {
        *visibility = if debug_menu.active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

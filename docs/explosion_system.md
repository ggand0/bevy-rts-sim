# Explosion System - Technical Documentation

**Last Updated:** November 29, 2025
**System Version:** Dual System (v5 - Flipbook + War FX)
**Bevy Version:** 0.14.2

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [War FX System (Tower Explosions)](#war-fx-system-tower-explosions)
4. [Legacy Flipbook System (Unit Deaths)](#legacy-flipbook-system-unit-deaths)
5. [Explosion Orchestration](#explosion-orchestration)
6. [Custom Materials](#custom-materials)
7. [Debug Controls](#debug-controls)
8. [Troubleshooting](#troubleshooting)
9. [Performance](#performance)
10. [Future Enhancements](#future-enhancements)

---

## Overview

The explosion system uses a **dual-system approach** combining Unity's War FX particle system with a legacy flipbook system to create dramatic, performant explosions.

### Design Philosophy
- **Dual System:** War FX for tower explosions, flipbook for unit deaths
- **Multi-Emitter:** Combined effects with glow, flames, smoke, and sparkles
- **Billboard Rendering:** All particles face the camera
- **Custom Materials:** Scrolling UV, additive blending, alpha smoke
- **No Shadows:** Particles don't cast or receive shadows
- **Orchestrated Timing:** Cascading delayed explosions for dramatic effect

### System Comparison

| Feature | War FX (Towers) | Flipbook (Units) |
|---------|-----------------|------------------|
| Particles | 6 emitter types, 180+ particles | Single billboard |
| Duration | 5+ seconds (lingering smoke) | 2 seconds |
| Materials | 3 custom materials | 1 custom shader |
| Audio | Explosion sound effect | Silent |
| Scale | 4.0× (dramatic) | 0.8× (subtle) |
| Use Case | High-impact events | Mass casualties |

### Evolution History

| Version | Approach | Outcome |
|---------|----------|---------|
| v1 | Procedural noise-based shader | "Super ugly" - abandoned |
| v2 | Smoke-only sprite + normal maps | "Didn't work well" - too complex |
| v3 | 8×8 grid flipbook (64 frames) | Good but oversized texture |
| v4 | 5×5 grid flipbook (25 frames) | Optimized for units |
| v5 | **War FX + Flipbook** | **Current - dual system** |

---

## Architecture

### File Structure

```
src/
├── explosion_system.rs      # Explosion orchestration (pending, timing)
├── explosion_shader.rs      # Legacy flipbook system (489 lines)
├── wfx_spawn.rs             # War FX particle spawning (1,436 lines)
├── wfx_materials.rs         # Custom materials (200 lines)
├── particles.rs             # Particle plugin setup
└── objective.rs             # Debug controls, tower destruction

assets/
├── shaders/
│   └── explosion.wgsl       # Flipbook shader (41 lines)
├── textures/
│   ├── Explosion02HD_5x5.tga    # Flipbook sprite sheet
│   └── wfx_explosivesmoke_big/  # War FX textures
│       ├── Center_glow.tga
│       ├── FireFlameB_00.tga - FireFlameB_14.tga (15 frames)
│       ├── smoke.tga
│       ├── GlowCircle.tga
│       └── SmallDots.tga
└── audio/
    └── explosion.ogg        # Explosion sound effect
```

### Plugin Registration

```rust
// In main.rs
.add_plugins(ExplosionShaderPlugin)      // Legacy flipbook
.add_plugins(ParticleEffectsPlugin)      // Particle setup
.add_plugins(MaterialPlugin::<SmokeScrollMaterial>::default())
.add_plugins(MaterialPlugin::<AdditiveMaterial>::default())
.add_plugins(MaterialPlugin::<SmokeOnlyMaterial>::default())
.insert_resource(ExplosionDebugMode::default())
```

### System Registration

**Explosion Systems** (run every frame):
- `pending_explosion_system` - Processes delayed explosions
- `explosion_effect_system` - Updates visual effects
- `update_debug_mode_ui` - Shows/hides debug indicator
- `debug_explosion_hotkey_system` - E key tower destruction
- `debug_warfx_test_system` - 0→1-6 key emitter spawning

**War FX Animation** (run every frame):
- `update_warfx_explosions` - Manages lifetimes
- `animate_explosion_flames` - Animates 57 flame particles
- `animate_warfx_billboards` - Animates center glow (2 billboards)
- `animate_warfx_smoke_billboards` - Animates smoke particles
- `animate_explosion_billboards` - Animates explosion billboards
- `animate_smoke_only_billboards` - Animates lingering smoke (6 particles)
- `animate_glow_sparkles` - Animates 25 sparkles with gravity

---

## War FX System (Tower Explosions)

### Overview

Tower explosions use a multi-emitter particle system ported from Unity's [War FX](https://assetstore.unity.com/packages/vfx/particles/war-fx-5669) asset (free, no license restrictions).

### Combined Explosion Structure

**Function:** `spawn_combined_explosion()` in `wfx_spawn.rs`

**Components:**
1. **Center Glow** (2 billboards)
   - Bright orange/yellow expanding sphere
   - Additive blending, no transparency
   - Duration: 1.5s, fades out smoothly

2. **Flame Particles** (57 particles)
   - Fire/smoke texture with 15-frame animation
   - Spherical emission (full 4π distribution)
   - Scrolling UV for animated flames
   - Duration: 3.0s, randomized lifetimes

3. **Smoke Emitter** (6 particles)
   - Delayed start (0.5s)
   - Lingering smoke trail
   - Alpha-blended, rises slowly
   - Duration: 5.0s (outlasts other emitters)

4. **Glow Sparkles** (25 particles)
   - Fast-moving embers
   - Gravity-affected, fall and fade
   - Additive blending
   - Duration: 2.0s

5. **Dot Sparkles** (90 particles total)
   - 75 falling particles (gravity)
   - 15 floating particles (rise upward)
   - Small circular dots
   - Duration: 2.5s

**Total:** 180 particles, 5+ second effect duration

### Spawning War FX Explosions

```rust
use crate::wfx_spawn::spawn_combined_explosion;

// Tower destruction
spawn_combined_explosion(
    &mut commands,
    &mut meshes,
    &mut additive_materials,
    &mut smoke_materials,
    &mut smoke_only_materials,
    &asset_server,
    tower_position,
    4.0,  // Scale multiplier (large tower explosion)
);
```

### War FX Components

```rust
#[derive(Component)]
pub struct WarfxExplosion {
    pub lifetime: f32,
    pub max_lifetime: f32,
}

#[derive(Component)]
pub struct ExplosionFlame {
    pub frame_timer: f32,
    pub current_frame: usize,
    pub velocity: Vec3,
    pub lifetime: f32,
}

#[derive(Component)]
pub struct WarfxSmokeBillboard {
    pub velocity: Vec3,
    pub lifetime: f32,
}

#[derive(Component)]
pub struct GlowSparkle {
    pub velocity: Vec3,
    pub lifetime: f32,
    pub initial_scale: f32,
}
```

### Billboard Calculation

All War FX particles face the camera using custom transform logic:

```rust
// In animate_* systems
let to_camera = (camera_position - particle_position).normalize();
let right = Vec3::Y.cross(to_camera).normalize();
let up = to_camera.cross(right);
transform.rotation = Quat::from_mat3(&Mat3::from_cols(right, up, to_camera));
```

---

## Legacy Flipbook System (Unit Deaths)

### Overview

Unit deaths use a simple 5×5 sprite sheet with custom shader for frame-by-frame animation.

**File:** `assets/textures/Explosion02HD_5x5.tga`
- **Grid:** 5×5 layout (25 frames)
- **Frame Order:** Left to right, top to bottom
- **Duration:** 2.0 seconds (0.08s per frame)
- **Content:** Complete explosion lifecycle baked in

### Spawning Flipbook Explosions

```rust
use crate::explosion_shader::{spawn_custom_shader_explosion, ExplosionAssets};

if let Some(assets) = explosion_assets.as_ref() {
    spawn_custom_shader_explosion(
        &mut commands,
        &mut meshes,
        &mut explosion_materials,
        &assets,
        particle_effects.as_ref().map(|p| p.as_ref()),
        unit_position,
        0.8,  // Radius (small for units)
        1.0,  // Intensity
        2.0,  // Duration
        false, // Not a tower
        time.elapsed_seconds_f64(),
    );
}
```

### Custom Shader (explosion.wgsl)

**41 lines** - calculates UV offsets for sprite sheet frames:

```wgsl
let frame_size = 1.0 / grid_size;  // 1/5 = 0.2
let frame_offset = vec2<f32>(
    frame_x * frame_size,
    frame_y * frame_size
);
let frame_uv = in.uv * frame_size + frame_offset;
let sprite_sample = textureSample(sprite_texture, sprite_sampler, frame_uv);
```

---

## Explosion Orchestration

### PendingExplosion Component

```rust
#[derive(Component)]
pub struct PendingExplosion {
    pub delay_timer: f32,
    pub explosion_power: f32,
}
```

Used in `tower_destruction_system` to create cascading explosions:

```rust
// Quantize delays to discrete time slots for multiple explosions per frame
let raw_delay = rng.gen_range(EXPLOSION_DELAY_MIN..EXPLOSION_DELAY_MAX);
let delay = (raw_delay / EXPLOSION_TIME_QUANTUM).round() * EXPLOSION_TIME_QUANTUM;

commands.get_entity(unit_entity).unwrap()
    .try_insert(PendingExplosion {
        delay_timer: delay,
        explosion_power: 1.0,
    });
```

### Constants

```rust
const EXPLOSION_DELAY_MIN: f32 = 0.5;      // Min delay in seconds
const EXPLOSION_DELAY_MAX: f32 = 3.0;      // Max delay in seconds
const EXPLOSION_TIME_QUANTUM: f32 = 0.1;   // Time slot size (100ms)
const MAX_EXPLOSIONS_PER_FRAME: usize = 20; // Frame limit to prevent lag
```

### Pending Explosion System

**File:** `explosion_system.rs`

```rust
pub fn pending_explosion_system(
    mut commands: Commands,
    // ... resources for spawning
    mut explosion_query: Query<(Entity, &mut PendingExplosion, &Transform, Option<&UplinkTower>)>,
    time: Res<Time>,
) {
    // Update all timers
    for (entity, mut pending, transform, tower_component) in explosion_query.iter_mut() {
        pending.delay_timer -= time.delta_seconds();

        if pending.delay_timer <= 0.0 {
            let is_tower = tower_component.is_some();

            if is_tower {
                // Spawn War FX combined explosion
                spawn_combined_explosion(/* ... */);
            } else {
                // Spawn flipbook explosion
                spawn_custom_shader_explosion(/* ... */);
            }

            commands.entity(entity).despawn_recursive();
        }
    }
}
```

---

## Custom Materials

### 1. SmokeScrollMaterial

**File:** `wfx_materials.rs`

Scrolling UV animation for smoke/flame textures:

```rust
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct SmokeScrollMaterial {
    #[uniform(0)]
    pub scroll_speed: Vec2,  // UV scroll velocity
    #[uniform(1)]
    pub time: f32,           // Elapsed time
    #[texture(2)]
    #[sampler(3)]
    pub texture: Handle<Image>,
    pub alpha: f32,
}
```

**Shader:** Offsets UVs based on time to create scrolling effect

### 2. AdditiveMaterial

Additive blending for glow/fire particles:

```rust
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct AdditiveMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Handle<Image>,
    pub color: LinearRgba,
    pub alpha: f32,
}
```

**Alpha Mode:** `AlphaMode::Add` - particle colors add to background

### 3. SmokeOnlyMaterial

Alpha-blended smoke particles:

```rust
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct SmokeOnlyMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Handle<Image>,
    pub alpha: f32,
    pub color: LinearRgba,
}
```

**Alpha Mode:** `AlphaMode::Blend` - standard transparency

---

## Debug Controls

### Nested Debug Mode

**Key 0:** Toggle explosion debug mode
- Shows/hides UI indicator at bottom-left
- Text: `[0] EXPLOSION DEBUG: 1=glow 2=flames 3=smoke 4=sparkles 5=combined 6=dots`

**Keys 1-6** (when debug mode active):
- **1:** Spawn center glow only (2 billboards)
- **2:** Spawn flame particles only (57 particles)
- **3:** Spawn smoke emitter only (6 particles)
- **4:** Spawn glow sparkles only (25 particles)
- **5:** Spawn combined explosion (all emitters, 180 particles)
- **6:** Spawn dot sparkles (75 + 15 particles)

All spawn at `Vec3::new(0.0, 10.0, 0.0)` with scale 2.0

### Gameplay Trigger

**Key E:** Destroy Team B tower
- Triggers tower explosion (War FX combined)
- Cascades to ~1,000-2,000 unit explosions (flipbook)
- Quantized delays (0.5-3.0s) for dramatic effect
- Plays explosion audio

**Implementation:** `debug_explosion_hotkey_system` in `objective.rs`

---

## Troubleshooting

### Common Issues

#### 1. Glow Hard-Cut

**Symptom:** Center glow disappears abruptly instead of fading.

**Solution:** ✅ Fixed - Proper fade-out curve in animation system:
```rust
let fade_factor = if progress < 0.7 {
    1.0
} else {
    1.0 - ((progress - 0.7) / 0.3).powf(2.0)
};
```

#### 2. Explosion Lag Spikes

**Symptom:** Game freezes when tower explodes.

**Solution:** ✅ Fixed with two optimizations:
1. **Frame Limit:** Max 20 explosions per frame
2. **Quantized Timing:** Delays rounded to 100ms slots ensures multiple explosions per frame

#### 3. Particles Not Facing Camera

**Symptom:** Billboards render as thin lines from certain angles.

**Solution:** Ensure billboard calculation uses correct cross products:
```rust
let to_camera = (camera_pos - particle_pos).normalize();
let right = Vec3::Y.cross(to_camera).normalize();
let up = to_camera.cross(right);
```

#### 4. Missing Textures

**Symptom:** Pink/magenta particles or white quads.

**Solution:** Ensure War FX textures are in `assets/textures/wfx_explosivesmoke_big/` with correct filenames.

---

## Performance

### Benchmarks

**Tower Destruction Cascade:**
- 1 tower explosion: 180 particles
- ~1,500 unit explosions: 1,500 billboards
- Peak concurrent: ~200 explosions (frame limit)
- Frame time impact: < 2ms

**Per-Particle Cost:**
- War FX particle: ~0.01ms (billboarding + animation)
- Flipbook explosion: ~0.005ms (shader-based)

### Optimization Features

1. **Frame Limiting:** Max 20 explosions spawn per frame
2. **Quantized Timing:** Delays rounded to 100ms slots
3. **Billboard Caching:** Camera position queried once
4. **No Shadows:** Particles excluded from shadow passes
5. **Automatic Cleanup:** Expired particles despawned immediately
6. **Simple Shaders:** UV offset only, no complex procedural noise

### Scalability

For > 500 concurrent explosions, consider:
- Object pooling (reuse entities)
- GPU instancing (single draw call)
- Compute shaders (parallel updates)
- LOD system (reduce particles at distance)

---

## Future Enhancements

### Planned Features

1. **Unified System**
   - Replace flipbook with War FX for all explosions
   - Scale parameter for unit vs tower
   - Single animation pipeline

2. **Screen-Space Effects**
   - Camera shake on large explosions
   - Heat wave distortion
   - Chromatic aberration
   - Enhanced bloom

3. **Audio Enhancements**
   - Per-explosion sound variations
   - 3D spatial audio
   - Distance attenuation
   - Randomized pitch/volume

4. **Visual Improvements**
   - Light emission (dynamic point lights)
   - Ground scorch marks (decals)
   - Debris physics
   - Randomized rotation/scale

5. **Performance**
   - GPU instancing for War FX particles
   - Compute shader animation updates
   - Particle pooling system
   - Distance-based LOD

---

## API Reference

### Key Functions

#### spawn_combined_explosion
```rust
pub fn spawn_combined_explosion(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    additive_materials: &mut ResMut<Assets<AdditiveMaterial>>,
    smoke_materials: &mut ResMut<Assets<SmokeScrollMaterial>>,
    smoke_only_materials: &mut ResMut<Assets<SmokeOnlyMaterial>>,
    asset_server: &Res<AssetServer>,
    position: Vec3,
    scale: f32,
)
```

Spawns complete War FX explosion with all emitters.

#### spawn_custom_shader_explosion
```rust
pub fn spawn_custom_shader_explosion(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    explosion_materials: &mut ResMut<Assets<ExplosionMaterial>>,
    assets: &ExplosionAssets,
    particle_effects: Option<&ExplosionParticleEffects>,
    position: Vec3,
    radius: f32,
    intensity: f32,
    duration: f32,
    is_tower: bool,
    spawn_time: f64,
)
```

Spawns flipbook explosion for unit deaths.

---

**End of Explosion System Documentation**

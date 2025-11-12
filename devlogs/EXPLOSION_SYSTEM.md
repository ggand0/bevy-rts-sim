# Explosion System - Technical Documentation

**Last Updated:** November 12, 2025  
**System Version:** Sprite Sheet-Based (v4)  
**Bevy Version:** 0.14.2

---

## 📋 Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Asset Configuration](#asset-configuration)
4. [Component Structure](#component-structure)
5. [System Flow](#system-flow)
6. [Spawning Explosions](#spawning-explosions)
7. [Animation Details](#animation-details)
8. [Shader Implementation](#shader-implementation)
9. [Troubleshooting](#troubleshooting)
10. [Performance](#performance)
11. [Future Enhancements](#future-enhancements)

---

## 🎯 Overview

The explosion system uses **sprite sheet flipbook animation** to create visually appealing explosions for the RTS game. The system went through multiple iterations before arriving at the current implementation.

### Design Philosophy
- **Self-Contained Sprite Sheet:** All explosion phases (bright core → expanding fireball → trailing smoke) baked into a single texture
- **Billboard Rendering:** Explosions always face the camera
- **No Shadows:** Explosions don't cast or receive shadows
- **Frame-Based Animation:** Simple frame counter, no complex state machines
- **Full Intensity:** Explosions maintain full brightness for 90% of their duration

### Evolution History

| Version | Approach | Outcome |
|---------|----------|---------|
| v1 | Procedural noise-based shader | "Super ugly" - abandoned |
| v2 | Smoke-only sprite + normal maps | "Didn't work well" - too complex |
| v3 | 8×8 grid flipbook (64 frames) | Good but oversized texture |
| v4 | **5×5 grid flipbook (25 frames)** | **Current - optimal** |

---

## 🏗️ Architecture

### File Structure

```
src/explosion_shader.rs      # Main explosion system (781 lines)
assets/shaders/explosion.wgsl # Custom shader for flipbook (41 lines)
assets/textures/Explosion02HD_5x5.tga # 5×5 sprite sheet texture
```

### Plugin Registration

```rust
// In main.rs
.add_plugins(ExplosionShaderPlugin)
```

The `ExplosionShaderPlugin` registers:
1. Custom `MaterialPlugin::<ExplosionMaterial>` for shader-based explosions
2. `setup_explosion_assets` startup system (loads texture, creates materials)
3. Multiple update systems for animation and cleanup

---

## 📦 Asset Configuration

### Texture Specification

**File:** `assets/textures/Explosion02HD_5x5.tga`
- **Format:** TGA (enabled via `tga` feature in Cargo.toml)
- **Grid:** 5×5 layout
- **Total Frames:** 25 frames
- **Frame Order:** Left to right, top to bottom (row-major)
- **Content:** Complete explosion lifecycle in each frame
  - Frames 0-8: Bright initial fireball
  - Frames 9-16: Expanding orange flames
  - Frames 17-24: Dissipating smoke

### ExplosionAssets Resource

```rust
pub struct ExplosionAssets {
    pub explosion_flipbook_texture: Handle<Image>,
    pub explosion_atlas: Handle<TextureAtlasLayout>,
    pub explosion_bright_material: Handle<StandardMaterial>,
    pub explosion_dim_material: Handle<StandardMaterial>,
    pub smoke_material: Handle<StandardMaterial>,
}
```

**Loaded During Startup:**
- `explosion_flipbook_texture`: The main 5×5 TGA texture
- `explosion_atlas`: TextureAtlasLayout for 5×5 grid
- Three pre-configured StandardMaterials (currently unused, kept for backward compatibility)

---

## 🧩 Component Structure

### Core Components

#### 1. ExplosionTimer
```rust
#[derive(Component)]
pub struct ExplosionTimer {
    timer: Timer,  // Manages explosion lifetime
}
```
- Controls how long the explosion lasts
- When timer finishes, explosion is despawned

#### 2. SpriteExplosion (StandardMaterial-based)
```rust
#[derive(Component)]
pub struct SpriteExplosion {
    pub explosion_type: ExplosionType,     // Fire, Smoke, Nuclear, Impact
    pub current_phase: ExplosionPhase,     // Currently unused (legacy)
    pub frame_count: usize,                // Always 25
    pub current_frame: usize,              // 0-24
    pub frame_duration: f32,               // Time per frame
    pub frame_timer: f32,                  // Accumulator
    pub scale: f32,                        // Base scale
    pub fade_alpha: f32,                   // Alpha multiplier
    pub phase_transition_timer: f32,       // Legacy, unused
}
```

#### 3. CustomShaderExplosion (Custom shader-based)
```rust
#[derive(Component)]
pub struct CustomShaderExplosion {
    pub explosion_type: ExplosionType,
    pub current_phase: ExplosionPhase,     // Legacy, unused
    pub frame_count: usize,                // Always 25
    pub current_frame: usize,              // 0-24
    pub frame_duration: f32,
    pub frame_timer: f32,
    pub scale: f32,
    pub fade_alpha: f32,
}
```

### Supporting Types

```rust
#[derive(PartialEq, Clone)]
pub enum ExplosionType {
    Fire,     // Standard fire explosion
    Smoke,    // Smoke-only (legacy)
    Nuclear,  // High-intensity explosion
    Impact,   // Low-intensity explosion
}

#[derive(PartialEq, Clone)]
pub enum ExplosionPhase {
    Initial,    // Bright phase (legacy, unused)
    Secondary,  // Dimmer phase (legacy, unused)
    Smoke,      // Smoke phase (legacy, unused)
}
```

**Note:** `ExplosionPhase` is kept for backward compatibility but no longer affects rendering. The sprite sheet contains all phases pre-rendered.

---

## 🔄 System Flow

### Startup Phase

```
Game Start
    ↓
setup_explosion_assets()
    ↓
Load TGA texture
Create TextureAtlas layout
Create StandardMaterials
Insert ExplosionAssets resource
```

### Runtime Phase

```
Tower Destroyed / Unit Explodes
    ↓
spawn_animated_sprite_explosion()
    ↓
Create PbrBundle with:
  - Quad mesh (scaled to explosion radius)
  - StandardMaterial with sprite texture
  - Billboard transform
  - ExplosionTimer
  - SpriteExplosion component
  - NotShadowCaster
  - NotShadowReceiver
    ↓
Every Frame:
  update_explosion_timers()        # Tick timers
  animate_sprite_explosions()      # Update frames, billboard, alpha
  cleanup_finished_explosions()    # Remove expired
```

### Animation Loop (per explosion)

```
Frame Update:
  1. Increment frame_timer by delta_time
  2. If frame_timer >= frame_duration:
     - Reset frame_timer to 0
     - Increment current_frame
     - Clamp to frame_count - 1 (hold last frame)
  
Billboard Update:
  1. Get camera position
  2. Calculate direction to camera
  3. Create rotation matrix to face camera
  4. Apply rotation to transform
  
Alpha/Emissive Update:
  1. Calculate progress (0.0 to 1.0)
  2. If progress > 0.9: Begin fade out
  3. Otherwise: Full intensity
  4. Update material properties
```

---

## 🚀 Spawning Explosions

### Primary Function: spawn_animated_sprite_explosion

```rust
pub fn spawn_animated_sprite_explosion(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    explosion_assets: &ExplosionAssets,
    position: Vec3,           // World position
    radius: f32,              // Visual size (quad size = radius * 2.0)
    intensity: f32,           // Determines explosion type
    duration: f32,            // Total animation time
)
```

**Usage Example:**
```rust
if let Some(assets) = explosion_assets.as_ref() {
    spawn_animated_sprite_explosion(
        &mut commands,
        &mut meshes,
        &mut materials,
        &assets,
        Vec3::new(0.0, 5.0, 0.0),  // Position
        10.0,                       // 20-unit wide quad
        2.0,                        // Fire intensity
        2.5,                        // 2.5 second duration
    );
}
```

### Intensity Mapping

```rust
if intensity > 2.5 {
    ExplosionType::Nuclear   // Very large explosions
} else if intensity > 1.5 {
    ExplosionType::Fire      // Standard explosions
} else {
    ExplosionType::Impact    // Small explosions
}
```

**Note:** Currently, `ExplosionType` doesn't affect rendering (legacy system), but kept for potential future differentiation.

### Animation Timing

```rust
timer: Timer::new(
    Duration::from_secs_f32(duration * 0.8),  // 20% faster than specified
    TimerMode::Once
)

frame_duration: (duration * 0.8) / 25.0  // Time per frame
```

**Example:** For `duration = 2.5` seconds:
- Actual duration: 2.0 seconds
- Frame duration: 0.08 seconds (12.5 FPS)
- Total animation: 25 frames × 0.08s = 2.0s

---

## 🎬 Animation Details

### Frame Update System

**System:** `animate_sprite_explosions`

**Per Explosion, Each Frame:**

1. **Frame Advancement**
   ```rust
   sprite_explosion.frame_timer += time.delta_seconds();
   if sprite_explosion.frame_timer >= sprite_explosion.frame_duration {
       sprite_explosion.frame_timer = 0.0;
       sprite_explosion.current_frame += 1;
       
       if sprite_explosion.current_frame >= sprite_explosion.frame_count {
           sprite_explosion.current_frame = sprite_explosion.frame_count - 1;
       }
   }
   ```

2. **Billboard Rotation**
   ```rust
   let to_camera = (camera_position - explosion_position).normalize();
   let forward = to_camera;
   let right = Vec3::Y.cross(forward).normalize();
   let up = forward.cross(right).normalize();
   transform.rotation = Quat::from_mat3(&Mat3::from_cols(right, up, forward));
   ```

3. **Alpha & Emissive Control**
   ```rust
   let progress = elapsed / duration;
   
   let alpha_fade = if progress > 0.9 {
       fade_alpha * (1.0 - (progress - 0.9) * 10.0)  // Fade last 10%
   } else {
       fade_alpha  // Full alpha for 90%
   };
   
   let emissive_strength = if progress > 0.9 {
       2.0 * (1.0 - (progress - 0.9) * 5.0)
   } else {
       2.0  // Full emissive
   };
   ```

4. **Material Update**
   ```rust
   material.base_color.set_alpha(alpha_fade);
   material.emissive = LinearRgba::new(
       current.red * emissive_strength,
       current.green * emissive_strength,
       current.blue * emissive_strength,
       current.alpha
   );
   ```

### Scale Behavior

**Constant Scale:** `transform.scale = Vec3::splat(1.0)`

The explosion does **not** scale over time. All expansion/contraction is baked into the sprite sheet frames. This simplifies animation and improves performance.

---

## 🎨 Shader Implementation

### Custom Material: ExplosionMaterial

```rust
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ExplosionMaterial {
    #[uniform(0)]
    pub frame_data: Vec4,  // x: frame_x, y: frame_y, z: grid_size, w: alpha
    
    #[uniform(1)]
    pub color_data: Vec4,  // RGB: tint, A: emissive_strength
    
    #[texture(2, dimension = "2d")]
    #[sampler(3)]
    pub sprite_texture: Handle<Image>,
}
```

### WGSL Shader (explosion.wgsl)

**Total:** 41 lines (simplified from 332 lines of procedural code)

**Key Operations:**

1. **Frame UV Calculation**
   ```wgsl
   let frame_size = 1.0 / grid_size;  // 1/5 = 0.2
   let frame_offset = vec2<f32>(
       frame_x * frame_size,
       frame_y * frame_size
   );
   let frame_uv = in.uv * frame_size + frame_offset;
   ```

2. **Texture Sampling**
   ```wgsl
   let sprite_sample = textureSample(
       sprite_texture,
       sprite_sampler,
       frame_uv
   );
   ```

3. **Color Enhancement**
   ```wgsl
   let tinted_color = sprite_sample.rgb * color_data.rgb;
   let emissive_strength = color_data.a;
   let enhanced_rgb = tinted_color + (tinted_color * emissive_strength);
   let final_alpha = sprite_sample.a * alpha;  // From frame_data.w
   ```

### Material Trait Implementation

```rust
impl Material for ExplosionMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/explosion.wgsl".into()
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
        descriptor.primitive.cull_mode = None;  // Disable backface culling
        Ok(())
    }
}
```

---

## 🐛 Troubleshooting

### Common Issues

#### 1. White Quads Instead of Explosions

**Symptom:** Explosions appear as plain white rectangles.

**Cause:** Materials don't have the sprite sheet texture applied.

**Solution:** Ensure you're using `spawn_animated_sprite_explosion()` or `spawn_custom_shader_explosion()`, **not** `spawn_explosion_effect()` (which is now a stub).

**Fix Applied:** Tower destruction and unit explosions now use the proper functions with `ExplosionAssets` parameter.

#### 2. Shadows Under Explosions

**Symptom:** Dark shadows visible beneath explosion quads.

**Cause:** Explosions participating in shadow system.

**Solution:** Already fixed - all explosions spawn with:
```rust
NotShadowCaster,   // Don't cast shadows
NotShadowReceiver, // Don't receive shadows
```

Also, all materials have `unlit: true`.

#### 3. "Three Explosions" Effect

**Symptom:** Explosion appears to flash/restart three times with decreasing transparency.

**Cause:** Legacy phase transition system (Initial → Secondary → Smoke).

**Solution:** Already fixed - phase transition code is disabled:
```rust
// DISABLED: Phase transitions to prevent "three explosions" effect
// The self-contained sprite sheet already has all phases baked in
```

#### 4. Overlapping Explosions

**Symptom:** Multiple explosions spawning at once, hard to see individual animations.

**Cause:** Automatic smoke spawning for large explosions + multiple debug spawns.

**Solution:** Already fixed:
- Removed automatic smoke explosion spawning
- Changed debug keys to spawn single explosions
- Removed T key debug cubes

#### 5. Entity Despawn Panic

**Symptom:** `error[B0003]: Could not insert a bundle ... because it doesn't exist`

**Cause:** Trying to add `PendingExplosion` to units already killed in combat.

**Solution:** Already fixed - use safe entity checks:
```rust
if let Some(mut entity_commands) = commands.get_entity(unit_entity) {
    entity_commands.insert(PendingExplosion { ... });
}
```

#### 6. Explosion Not Animating

**Symptom:** Explosion shows full sprite sheet grid or doesn't change frames.

**Status:** **CURRENT ISSUE** - Being investigated.

**Potential Causes:**
- Material not updating each frame
- Frame calculation incorrect
- UV coordinates not being applied properly
- Shader not receiving frame data

**Debug Steps:**
1. Verify `animate_sprite_explosions` is running (check with `info!()`)
2. Check if `current_frame` is incrementing
3. Verify material handle is valid and mutable
4. Confirm texture is loaded (check asset server state)

---

## ⚡ Performance

### Optimization Features

1. **No Scaling Animation:** Constant scale reduces transform updates
2. **Frame Hold:** Last frame held instead of looping
3. **Billboard Caching:** Camera position queried once per frame, not per explosion
4. **Simple Shader:** Texture lookup only, no procedural noise
5. **Shadow Exclusion:** Explosions don't participate in shadow calculations
6. **Automatic Cleanup:** Expired explosions removed immediately

### Performance Characteristics

**Per Explosion:**
- 1 PbrBundle entity
- 1 quad mesh (4 vertices, 6 indices)
- 1 StandardMaterial or ExplosionMaterial
- 2-3 component queries per frame
- 1 billboard rotation calculation
- 1 alpha/emissive update
- ~0.001ms CPU time (estimated)

**Typical Scenario (Tower Destruction):**
- 1 tower explosion (large)
- ~1,000-2,000 unit explosions (staggered over 3 seconds)
- Peak: ~200 concurrent explosions
- Impact: Minimal (< 1ms frame time)

### Scalability Notes

For truly massive explosion counts (>500 concurrent), consider:
- Object pooling instead of spawn/despawn
- GPU instancing for explosion quads
- Compute shader for animation updates
- LOD system (fewer frames for distant explosions)

---

## 🚀 Future Enhancements

### Planned Features

1. **Particle Systems**
   - GPU-based particle emitters
   - Debris, sparks, embers
   - Smoke trails
   - Integration with sprite sheet base

2. **Audio Integration**
   - Explosion sound effects
   - 3D spatial audio
   - Randomized variations
   - Synchronized with visual

3. **Screen-Space Effects**
   - Camera shake
   - Heat wave distortion
   - Chromatic aberration
   - Bloom enhancement

4. **Physics Integration**
   - Ragdoll units on death
   - Debris with physics
   - Explosion force impulses
   - Ground scorch marks

5. **Visual Enhancements**
   - Multiple sprite sheet variants
   - Color tinting based on type
   - Randomized rotation
   - Size variation
   - Light emission (point lights)

### Potential Optimizations

1. **Pooling System**
   ```rust
   pub struct ExplosionPool {
       inactive: Vec<Entity>,
       active: Vec<Entity>,
   }
   ```

2. **LOD System**
   - Distance-based frame rate reduction
   - Smaller textures for distant explosions
   - Culling for off-screen explosions

3. **Instanced Rendering**
   - Single draw call for all explosions
   - Per-instance frame data
   - Compute shader for updates

---

## 📊 Debug Controls Reference

### Test Explosion Spawning

| Key | Function | Type | Notes |
|-----|----------|------|-------|
| **Y** | Animated Sprite | StandardMaterial | Single explosion at center |
| **U** | Custom Shader | ExplosionMaterial | Primary method, uses custom shader |
| **I** | Solid Color | StandardMaterial | Positioning test (no texture) |

**Location:** All spawn at `Vec3::new(0.0, 8.0, 0.0)` (battlefield center, elevated)  
**Parameters:** radius=8.0, intensity=2.0, duration=3.0

### Gameplay Triggers

| Key | Function | Explosions | Notes |
|-----|----------|------------|-------|
| **E** | Destroy Team B Tower | 1 tower + ~1000-2000 units | Cascade effect |

**Tower Explosion:**
- Position: Tower location
- Radius: 40.0 (80% of destruction radius)
- Duration: 4.0 seconds (2× normal)

**Unit Explosions:**
- Position: Unit locations
- Radius: 0.8 (scaled down)
- Duration: 2.0 seconds
- Delay: Random 0.5-3.0s per unit

---

## 🔍 Code Examples

### Example 1: Basic Explosion Spawning

```rust
fn spawn_explosion_on_death(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    explosion_assets: Option<Res<ExplosionAssets>>,
    query: Query<(Entity, &Transform, &Health)>,
) {
    for (entity, transform, health) in query.iter() {
        if health.is_dead() {
            if let Some(assets) = explosion_assets.as_ref() {
                spawn_animated_sprite_explosion(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &assets,
                    transform.translation,
                    5.0,    // Medium size
                    1.5,    // Fire intensity
                    2.0,    // Standard duration
                );
            }
            
            commands.entity(entity).despawn_recursive();
        }
    }
}
```

### Example 2: Custom Shader Explosion

```rust
fn spawn_custom_explosion(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    explosion_materials: &mut ResMut<Assets<ExplosionMaterial>>,
    explosion_assets: &ExplosionAssets,
    position: Vec3,
) {
    let quad_mesh = meshes.add(Rectangle::new(10.0, 10.0));
    
    let material = explosion_materials.add(ExplosionMaterial {
        frame_data: Vec4::new(0.0, 0.0, 5.0, 1.0),  // Start at frame 0
        color_data: Vec4::new(1.0, 1.0, 1.0, 2.0),  // White, 2× emissive
        sprite_texture: explosion_assets.explosion_flipbook_texture.clone(),
    });
    
    commands.spawn((
        MaterialMeshBundle {
            mesh: quad_mesh,
            material,
            transform: Transform::from_translation(position),
            ..default()
        },
        ExplosionTimer {
            timer: Timer::new(Duration::from_secs_f32(2.0), TimerMode::Once),
        },
        CustomShaderExplosion {
            explosion_type: ExplosionType::Fire,
            current_phase: ExplosionPhase::Initial,
            frame_count: 25,
            current_frame: 0,
            frame_duration: 2.0 / 25.0,
            frame_timer: 0.0,
            scale: 5.0,
            fade_alpha: 1.0,
        },
        NotShadowCaster,
        NotShadowReceiver,
        Name::new("CustomExplosion"),
    ));
}
```

### Example 3: Delayed Explosion

```rust
// Add pending explosion component
if let Some(mut entity_commands) = commands.get_entity(unit_entity) {
    entity_commands.insert(PendingExplosion {
        delay_timer: 1.5,  // Wait 1.5 seconds
        explosion_power: 2.0,
    });
}

// The pending_explosion_system will handle spawning after delay
```

---

## 📚 API Reference

### Public Functions

#### spawn_animated_sprite_explosion
Creates a StandardMaterial-based explosion with sprite sheet.

**Parameters:**
- `commands`: Bevy command buffer
- `meshes`: Mesh asset storage
- `materials`: StandardMaterial asset storage
- `explosion_assets`: Reference to loaded explosion resources
- `position`: World space position (Vec3)
- `radius`: Visual size (quad width/height = radius × 2)
- `intensity`: Determines explosion type (f32)
- `duration`: Total animation time in seconds (f32)

**Returns:** None (spawns entity directly)

#### spawn_custom_shader_explosion
Creates a custom shader-based explosion.

**Parameters:** Same as `spawn_animated_sprite_explosion`, plus:
- `explosion_materials`: ExplosionMaterial asset storage (instead of StandardMaterial)

**Returns:** None

#### spawn_debug_explosion_effect
**DEPRECATED** - Creates colored quad without texture (debug only).

#### spawn_explosion_effect
**STUB** - Backward compatibility, logs warning. Use `spawn_animated_sprite_explosion` instead.

---

## 🎓 Learning Resources

### Understanding the System

1. **Start Here:** Read `spawn_animated_sprite_explosion()` function
2. **Animation Logic:** Study `animate_sprite_explosions()` system
3. **Shader Basics:** Review `explosion.wgsl` (only 41 lines)
4. **Integration:** Check `tower_destruction_system()` usage example

### Key Concepts

- **Billboard Rendering:** Quads that always face camera
- **Sprite Sheet Animation:** UV coordinate offsetting per frame
- **Material vs Shader:** StandardMaterial vs custom ExplosionMaterial
- **Component-System Pattern:** ECS architecture for explosion lifecycle

### Bevy-Specific Knowledge

- **Asset Loading:** `asset_server.load()` and `Handle<T>`
- **Material Plugin:** Registering custom materials
- **Query Filters:** `With<T>`, `Without<T>` patterns
- **Command Buffers:** Deferred entity spawning/despawning

---

**End of Explosion System Documentation**


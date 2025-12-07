# Bevy Mass Render - RTS Game Project Overview

**Last Updated:** December 4, 2025
**Bevy Version:** 0.16.0
**Rust Version:** 1.90.0
**Platform:** Linux (tested on Ubuntu with AMD Radeon RX 7900 XTX)

---

## Game Concept

A 3D real-time strategy (RTS) game inspired by **Star Wars: Empire at War**, featuring massive-scale battles with thousands of units. The game simulates epic confrontations between two armies of battle droids fighting to destroy each other's command uplink towers.

### Core Gameplay
- **10,000 Unit Battles:** 5,000 droids per team engaging in large-scale warfare
- **Objective-Based:** Destroy the enemy's uplink tower to win
- **Shield Defense System:** Empire at War-style shields protect towers with regeneration mechanics
- **Tower Cascade System:** When a tower is destroyed, all friendly units within range explode dramatically
- **Defensive Turrets:** Two procedural turrets (Heavy and MG) provide Team A defensive fire
- **Formation Combat:** Squad-based movement with tactical formations
- **Real-time Combat:** Automatic targeting and projectile-based weapon systems
- **RTS Controls:** Click/box selection, group formations, orientation control

---

## Architecture Overview

### Module Structure

```
src/
├── main.rs              # App initialization, plugin registration
├── types.rs             # Core data structures and components
├── constants.rs         # Game configuration constants
├── setup.rs             # Scene setup, army spawning
├── formation.rs         # Squad formations and management
├── movement.rs          # Unit animation, camera controls
├── combat.rs            # Targeting, firing, collision detection
├── commander.rs         # Commander promotion and visual markers
├── objective.rs         # Tower mechanics, destruction cascade, debug systems
├── shield.rs            # Shield system, regeneration, respawn mechanics
├── procedural_meshes.rs # Procedural mesh generation (units, towers, turrets)
├── turrets.rs           # Turret spawn systems and map respawn
├── explosion_system.rs  # Explosion orchestration (pending explosions, effects)
├── explosion_shader.rs  # Legacy flipbook explosion (unit deaths)
├── particles.rs         # Particle system plugin (debris, sparks)
├── wfx_materials.rs     # Custom materials (smoke scroll, additive, etc.)
├── wfx_spawn.rs         # War FX explosion spawning and animation
├── terrain.rs           # Terrain generation and map switching
├── decals.rs            # Decal rendering system (bullet holes, ClusteredDecal)
└── selection/           # Selection and grouping system
    ├── mod.rs           # Module exports, system registration
    ├── state.rs         # SelectionState resource, marker components
    ├── groups.rs        # Squad grouping logic, formation preservation
    ├── input.rs         # Click and box selection input handling
    ├── movement.rs      # Move commands with orientation drag
    ├── obb.rs           # Oriented Bounding Box calculations
    ├── utils.rs         # Shared utility functions
    └── visuals/         # Visual feedback systems
        ├── mod.rs       # Re-exports for clean API
        ├── selection.rs # Selection rings, box selection visuals
        ├── movement.rs  # Move indicators, path arrows
        └── group.rs     # Group orientation markers, OBB debug

assets/
├── shaders/
│   ├── explosion.wgsl   # Custom shader for flipbook animation
│   └── shield.wgsl      # Shield effect shader (hexagonal grid, fresnel)
├── textures/
│   ├── Explosion02HD_5x5.tga      # 5x5 sprite sheet (unit deaths)
│   ├── bullet_hole_0.png          # Bullet hole decal texture (alpha transparency)
│   └── wfx_explosivesmoke_big/    # War FX textures (tower explosions)
│       ├── Center_glow.tga        # Center glow billboard
│       ├── FireFlameB_00.tga      # Flame animation frames
│       ├── smoke.tga              # Smoke particles
│       ├── GlowCircle.tga         # Sparkle particles
│       └── SmallDots.tga          # Dot particles
└── audio/
    ├── sfx/                       # Laser sound effects
    └── explosion.ogg              # Explosion sound
```

### Key Systems (Update Order)

1. **Formation & Squad Management**
   - `squad_formation_system` - Maintains squad formations
   - `squad_casualty_management_system` - Reorganizes squads when units die
   - `squad_movement_system` - Moves squads in formation
   - `commander_promotion_system` - Promotes new commanders
   - `commander_visual_update_system` - Updates commander visuals
   - `commander_visual_marker_system` - Creates debug markers
   - `update_commander_markers_system` - Updates marker positions

2. **Animation & Camera**
   - `animate_march` - Animates marching units
   - `update_camera_info` - Updates camera information display
   - `rts_camera_movement` - RTS-style camera controls

3. **Combat Systems**
   - `target_acquisition_system` - Finds enemies in range
   - `auto_fire_system` - Automatic firing at targets (handles MG turret rapid fire modes)
   - `volley_fire_system` - Coordinated volley attacks (F key)
   - `update_projectiles` - Moves projectiles
   - `collision_detection_system` - Detects hits and applies damage
   - `turret_rotation_system` - Smoothly rotates turret assemblies toward targets

3a. **Shield Systems**
   - `shield_collision_system` - Laser impact detection & damage
   - `shield_regeneration_system` - HP recovery over time
   - `shield_impact_flash_system` - Flash timer countdown
   - `shield_health_visual_system` - Material alpha & ripple updates
   - `shield_tower_death_system` - Despawn when tower dies
   - `shield_respawn_system` - Handle respawn countdown
   - `animate_shields` - Shader time animation

4. **Objective & Explosion Systems**
   - `tower_targeting_system` - Units target towers when in range
   - `tower_destruction_system` - Handles tower death and cascade
   - `pending_explosion_system` - Manages delayed explosions (from explosion_system.rs)
   - `explosion_effect_system` - Updates visual effects (from explosion_system.rs)
   - `win_condition_system` - Checks for game end
   - `update_objective_ui_system` - Updates tower health UI
   - `update_debug_mode_ui` - Shows/hides debug mode indicator
   - `debug_explosion_hotkey_system` - Debug controls (E key triggers tower death)
   - `debug_warfx_test_system` - WFX debug spawning (0 to toggle, then 1-6)

5. **Selection & Grouping Systems**
   - `selection_input_system` - Mouse click selection
   - `box_selection_system` - Drag-to-select multiple squads
   - `move_command_system` - Right-click move orders with orientation
   - `group_selection_system` - G/U key grouping controls
   - `squad_rotation_system` - Smooth facing rotation
   - `selection_visual_system` - Cyan/yellow selection rings
   - `box_selection_visual_system` - Box selection rectangle
   - `orientation_arrow_system` - Green arrow during right-click drag
   - `update_squad_path_arrows` - Persistent path arrows for moving squads
   - `move_visual_cleanup_system` - Fade-out for move indicators
   - `update_group_orientation_markers` - Yellow triangle for group facing
   - `update_group_bounding_box_debug` - Magenta OBB wireframe (optional)

6. **War FX Explosion System** (from wfx_spawn.rs)
   - `update_warfx_explosions` - Manages combined explosion lifetimes
   - `animate_explosion_flames` - Animates flame particles
   - `animate_warfx_billboards` - Animates center glow billboards
   - `animate_warfx_smoke_billboards` - Animates smoke particles
   - `animate_explosion_billboards` - Animates explosion billboards
   - `animate_smoke_only_billboards` - Animates lingering smoke
   - `animate_glow_sparkles` - Animates sparkle particles with gravity

7. **Legacy Explosion System** (from ExplosionShaderPlugin)
   - `setup_explosion_assets` - Loads sprite sheet and creates materials
   - `update_explosion_timers` - Manages explosion lifetimes
   - `animate_custom_shader_explosions` - Animates flipbook explosions (unit deaths)
   - `cleanup_finished_explosions` - Removes expired explosions

---

## Game Configuration (constants.rs)

### Army & Formation
- `ARMY_SIZE_PER_TEAM`: 5,000 units per team
- `SQUAD_SIZE`: 50 droids per squad (100 squads per team)
- `SQUAD_WIDTH`: 10 units wide
- `SQUAD_DEPTH`: 5 units deep
- `SQUAD_HORIZONTAL_SPACING`: 2.0
- `SQUAD_VERTICAL_SPACING`: 2.5
- `INTER_SQUAD_SPACING`: 12.0 (tactical spacing between squads)

### Combat
- `TARGETING_RANGE`: 150.0 units
- `TARGET_SCAN_INTERVAL`: 2.0 seconds between target updates
- `AUTO_FIRE_INTERVAL`: 2.0 seconds between shots
- `COLLISION_RADIUS`: 1.0 unit
- `LASER_SPEED`: 100.0 units/sec
- `LASER_LIFETIME`: 3.0 seconds
- `LASER_LENGTH`: 3.0 units
- `LASER_WIDTH`: 0.2 units

### Objectives
- `TOWER_HEIGHT`: 35.0 units (tall, slender design)
- `TOWER_BASE_WIDTH`: 9.0 units (rectangular base, wider than deep)
- `TOWER_MAX_HEALTH`: 1,000.0 HP
- `TOWER_DESTRUCTION_RADIUS`: 80.0 units (explosion cascade range)
- `EXPLOSION_DELAY_MIN`: 0.5 seconds
- `EXPLOSION_DELAY_MAX`: 3.0 seconds (dramatic cascade timing)
- `EXPLOSION_EFFECT_DURATION`: 2.0 seconds

### Camera
- `CAMERA_SPEED`: 50.0 units/sec
- `CAMERA_ZOOM_SPEED`: 10.0 units/sec
- `CAMERA_MIN_HEIGHT`: 20.0 units
- `CAMERA_MAX_HEIGHT`: 200.0 units
- `CAMERA_ROTATION_SPEED`: 0.005 radians

### World
- `BATTLEFIELD_SIZE`: 400.0 units (total battlefield)
- `FORMATION_WIDTH`: 200.0 units
- `MARCH_DISTANCE`: 150.0 units (distance teams march toward center)
- `MARCH_SPEED`: 3.0 units/sec

### Spatial Partitioning
- `GRID_CELL_SIZE`: 10.0 units per cell
- `GRID_SIZE`: 100 cells per side (covers 1000×1000 area)

### Selection & Grouping
- `SELECTION_CLICK_RADIUS`: 15.0 units (generous for usability)
- `SELECTION_RING_INNER_RADIUS`: 8.0 units
- `SELECTION_RING_OUTER_RADIUS`: 10.0 units
- `BOX_SELECT_DRAG_THRESHOLD`: 8.0 pixels
- `MOVE_INDICATOR_RADIUS`: 3.0 units
- `MOVE_INDICATOR_LIFETIME`: 1.5 seconds
- `SQUAD_ROTATION_SPEED`: 2.0 radians/sec
- `MULTI_SQUAD_SPACING`: 25.0 units
- `SQUAD_ARRIVAL_THRESHOLD`: 5.0 units (arrival detection)

---

## Visual Design

### Unit Design - Battle Droids
Procedurally generated using simple boxes:
- **Body:** Tall rectangular torso
- **Head:** Smaller rectangular head offset forward
- **Arms:** Two thin rectangular arms at sides
- **Legs:** Two rectangular legs
- **Materials:** Color-coded by team
  - Team A: Blue-gray body, tan head
  - Team B: White body, bright white head
  - Commanders: Golden/orange highlights

### Tower Design - Uplink Towers
Procedurally generated futuristic data towers:
- **Foundation:** Underground grounding system for stability
- **Base Platform:** Rectangular landing pad
- **Central Spine:** Tall, slender rectangular core (wider than deep)
- **Architectural Modules:** Modular pods attached close to spine
- **Rooftop:** Flat top with realistic antenna cluster
- **Design Philosophy:** Tall, functional military communications structure
- **Face Winding:** Proper front-facing geometry, back-face culling enabled
- **Team Colors:** Distinct materials for Team A vs Team B

### Explosion Effects

**Dual System Approach:**
- **Unit Deaths:** Legacy flipbook system (5×5 sprite sheet, 25 frames)
- **Tower Explosions:** War FX particle system (multi-emitter combined effect)

**War FX Explosion Components** (from Unity Asset Store - War FX):
1. **Center Glow:** Bright orange/yellow expanding sphere (2 billboards)
2. **Flame Particles:** 57 fire/smoke particles with spherical emission
3. **Smoke Emitter:** Lingering smoke trail (6 particles, delayed start)
4. **Glow Sparkles:** 25 fast-moving embers with gravity
5. **Dot Sparkles:** 75 + 15 small particles (falling + floating)

**Custom Materials:**
- `SmokeScrollMaterial`: Scrolling UV animation for smoke
- `AdditiveMaterial`: Additive blending for glow/fire
- `SmokeOnlyMaterial`: Alpha-blended smoke

**Scale Parameter:** Tower explosions use 4.0× scale for dramatic impact

---

## Controls

### Camera Controls (RTS-Style)
- **WASD:** Pan camera horizontally
- **Mouse Right-Click + Drag:** Rotate camera
- **Scroll Wheel:** Zoom in/out
- **Focus:** Auto-centers on battlefield

### Selection & Movement (RTS Controls)
- **Left-Click:** Select squad at cursor (15-unit radius, player team only)
- **Shift+Click:** Add/remove squad from selection
- **Left-Click Drag:** Box selection (8-pixel threshold before activating)
- **Right-Click:** Move selected squads to cursor position
- **Right-Click Drag:** Move + set orientation (CoH1-style)
- **G Key:** Group 2+ selected squads with formation preservation
- **U Key:** Ungroup selected squads

### Combat Commands
- **F Key:** Volley Fire - coordinated attack from all units
- **T Key:** Advance formation (changed from G to avoid grouping conflict)
- **H Key:** Retreat formation

### Formation Commands
- **Q Key:** Rectangular formation
- **E Key:** Line formation
- **R Key:** Box formation

### Debug Controls
- **E Key:** Trigger Team B tower destruction (cascade explosion test)
- **0 Key:** Toggle explosion debug mode (shows UI indicator)
  - When active, keys 1-6 spawn individual WFX emitters:
  - **1:** Center glow only
  - **2:** Flame particles only
  - **3:** Smoke emitter only
  - **4:** Glow sparkles only
  - **5:** Combined explosion (all emitters)
  - **6:** Dot sparkles (falling + floating)
  - **M:** Toggle MG turret spawn
  - **H:** Toggle Heavy turret spawn
  - **S:** Instantly destroy enemy (Team B) shield

---

## Technical Details

### Performance Optimizations
1. **Spatial Partitioning:** 10×10 grid system for efficient collision detection
2. **Squad System:** Groups of 50 units managed together, reducing individual queries
3. **Target Caching:** Targets updated every 2 seconds, not every frame
4. **Instancing:** Single mesh shared across all units of same type
5. **Conditional Systems:** Systems only run when needed (e.g., formation changes)
6. **Release Profile:** LTO enabled, optimized compilation settings
7. **Cached Laser Assets:** Laser materials/meshes pre-created at startup to avoid per-shot allocation
8. **Proximity-based Audio:** `proximity_volume()` helper for distance-based volume attenuation

### Rendering
- **Backend:** Vulkan (with AMD GPU workarounds required)
- **Materials:** PBR StandardMaterial for units/towers
- **Custom Shaders:**
  - Flipbook animation (legacy unit explosions)
  - Scrolling UV (smoke particles)
  - Additive blending (glow/fire)
  - Shield effects (hexagonal grid, fresnel edge glow, impact ripples)
- **Billboard Quads:** All particles face camera using custom Transform calculations
- **Shadow Control:** Units cast/receive shadows; particles do not
- **Material Plugins:** Custom material plugins for specialized effects (smoke, additive, shields, etc.)

### AMD GPU Compatibility

**Required Launch Command:**
```bash
VK_LOADER_DEBUG=error VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json cargo run --profile opt-dev
```

This works around segfault issues with AMD drivers on Linux.

---

## Known Issues & Limitations

### Current Limitations
1. **Dual Explosion Systems:** Unit deaths use legacy flipbook; towers use War FX (intentional)
2. **2D Particles:** Billboard quads, no 3D volumetric effects
3. **Limited Audio:** Explosion sounds only on tower destruction
4. **Fixed Formations:** Formations are preset, not customizable in-game
5. **No Pathfinding:** Units march in straight lines, no obstacle avoidance
6. **Simple AI:** Units auto-target nearest enemy, no tactics

---

## Design Goals

### Achieved
- Massive unit counts (10,000 concurrent units)
- Performant rendering and simulation
- Squad-based formation system
- Objective-based gameplay (tower destruction)
- Empire at War-style shield defense system
- Dramatic cascade explosion system
- RTS-style camera controls
- Procedural unit and tower generation
- War FX particle system (multi-emitter explosions)
- Custom material system (scrolling smoke, additive glow, shields)
- Explosion audio integration
- Billboard particle rendering
- Squad selection and grouping system
- Total War-style formation preservation
- CoH1-style orientation control
- Visual feedback (selection rings, path arrows, group markers)

### Potential Enhancements
- Screen-space distortion effects (heat waves)
- Camera shake on explosions
- Unit ragdoll/death animations
- Physics-based debris
- LOD system for distant units
- Control groups (number keys 1-9 for saved selections)
- Formation templates (line, column, wedge, box)
- Pathfinding system
- Tactical AI behaviors
- Minimap with selection indicators
- Merge dual explosion systems into unified approach

---

## Project History

### Evolution of Explosion System
1. **Procedural Approach (v1):** Complex noise-based shaders - "super ugly", abandoned
2. **Smoke-Only Sprites (v2):** Sprite sheet with normal maps - "didn't work well"
3. **8×8 Flipbook (v3):** Initial sprite sheet approach with 64 frames
4. **5×5 Flipbook (v4):** Self-contained explosion with 25 frames, all phases baked in
5. **War FX Integration (v5, Current):** Multi-emitter particle system for towers, flipbook for units

### Major Milestones
- **Initial Development:** 10,000 unit simulation with basic combat
- **June 2025:** Implemented sprite sheet explosion system
- **June 15, 2025:** Fixed shadow artifacts and overlapping explosions
- **November 12, 2025:** Updated to Bevy 0.14.2, fixed AMD GPU compatibility
- **November 29, 2025:**
  - War FX particle system integration (6 emitter types)
  - Custom material system (smoke scroll, additive blending)
  - Module refactoring (explosion_system.rs extraction)
  - Nested debug shortcuts with UI indicator
  - Selection and grouping system (Total War-style formation preservation)
  - Visual feedback overhaul (rings, arrows, orientation markers)
  - Oriented Bounding Box (OBB) implementation
- **December 1, 2025:**
  - Migrated to Bevy 0.16 (from 0.15)
  - Resolved bevy_hanabi shader crash with Bevy 0.17
  - Decal rendering system using ClusteredDecal
  - Bullet hole texture projections on terrain
- **December 4, 2025:**
  - Fixed post-0.16 bugs (terrain positioning, animation, audio)
  - Optimized tower explosion visuals with shared assets
  - Cached laser materials/meshes to reduce per-shot allocations
  - Added proximity-based volume attenuation for spatial audio
  - Added debug menu toggle for turrets (M/H keys)
  - Extracted `proximity_volume()` helper for audio distance falloff

---

## Build & Run

### Development Build
```bash
cargo run
```

### Release Build (Recommended for Performance)
```bash
# AMD GPU (Linux)
VK_LOADER_DEBUG=error VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json cargo run --release

# Other GPUs
cargo run --release
```

### Build Profile Settings
```toml
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3

[profile.release]
lto = true
codegen-units = 1
```

### Running the Binary Directly (Without Cargo)

When running the compiled binary directly (instead of `cargo run`), you need to set several environment variables because the project uses dynamic linking for faster development builds:

```bash
# From the project root directory:
BEVY_ASSET_ROOT=$(pwd) \
LD_LIBRARY_PATH=./target/opt-dev/deps:~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/lib \
VK_LOADER_DEBUG=error VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json \
./target/opt-dev/bevy-mass-render
```

**Why these are needed:**
- `BEVY_ASSET_ROOT`: Tells Bevy where to find the `assets/` folder (dynamic linking changes the default lookup path)
- `LD_LIBRARY_PATH`: Points to Bevy's dynamic library and Rust's stdlib (required for dynamic linking)
- `VK_*`: AMD GPU Vulkan workarounds

**Note:** For release/shipping builds, disable `dynamic_linking` in `Cargo.toml` to produce a standalone binary that doesn't require these environment variables.

---

## Code Style & Conventions

### Naming Conventions
- **Components:** PascalCase (e.g., `BattleDroid`, `UplinkTower`)
- **Systems:** snake_case with `_system` suffix (e.g., `combat_system`)
- **Resources:** PascalCase (e.g., `SquadManager`, `GameState`)
- **Constants:** SCREAMING_SNAKE_CASE (e.g., `ARMY_SIZE_PER_TEAM`)

### System Organization
- Systems grouped by functionality in separate files
- Update systems ordered by logical dependencies
- Startup systems run in defined order for initialization

### Component Patterns
- Marker components for entity types (e.g., `BattleDroid`, `Commander`)
- Data components for state (e.g., `Health`, `Target`, `Weapon`)
- Relationship components for hierarchy (e.g., squad membership)

### Query Patterns
- Use `With<T>` for filtering by marker components
- Use `Without<T>` to avoid query conflicts
- Use `Option<&T>` for optional components
- Use `Changed<T>` for change detection

---

## For Future AI Agents

### Key Files to Understand
1. **types.rs** - Core data structures, understand these first
2. **constants.rs** - All tunable parameters
3. **main.rs** - System registration order (important!)
4. **selection/** - Selection and grouping system (1,716 lines across submodules)
5. **shield.rs** - Shield defense system (regeneration, respawn, visual effects)
6. **wfx_spawn.rs** - War FX explosion system (tower explosions)
7. **explosion_system.rs** - Explosion orchestration (pending, timing)
8. **explosion_shader.rs** - Legacy flipbook system (unit deaths)

### Common Tasks
- **Adding new unit types:** Modify `setup.rs`, add to `types.rs`
- **Tweaking gameplay:** Edit `constants.rs` values
- **New formations:** Add to `formation.rs`
- **Combat changes:** Modify `combat.rs` systems
- **Selection/grouping changes:**
  - Input handling: `selection/input.rs`
  - Move commands: `selection/movement.rs`
  - Visual feedback: `selection/visuals/*.rs`
  - Formation preservation: `selection/groups.rs`
- **Visual effects:**
  - New particles: Add to `wfx_spawn.rs`
  - New materials: Add to `wfx_materials.rs`
  - Explosion timing: Modify `explosion_system.rs`

### Debugging Tips
- Use `info!()` macros for logging (already extensively used)
- Check entity counts with queries in debug systems
- Monitor FPS with bevy's diagnostic plugin (already integrated)
- Use the debug keys:
  - **E:** Trigger tower destruction cascade
  - **0:** Toggle explosion debug mode
  - **1-6:** Spawn individual WFX emitters (when debug mode active)

### Performance Considerations
- This project pushes Bevy's limits with 10,000+ entities
- Spatial partitioning is critical for collision detection
- Squad system reduces query overhead
- Any new systems should batch operations when possible

---

## Support & Resources

### Bevy Resources
- **Bevy Documentation:** https://bevyengine.org/learn/
- **Bevy Discord:** For engine-specific questions
- **Error Codes:** https://bevyengine.org/learn/errors/

### Project-Specific Documentation
- **explosion_system.md** - Detailed explosion implementation
- **selection_grouping.md** - Selection and grouping system details
- **turrets.md** - Turret systems, firing modes, and targeting behavior
- **shield_system.md** - Shield defense mechanics and configuration
- **devlogs/** - Historical development notes and fixes

---

**End of Overview**


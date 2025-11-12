# Bevy Mass Render - RTS Game Project Overview

**Last Updated:** November 12, 2025  
**Bevy Version:** 0.14.2  
**Rust Version:** 1.90.0  
**Platform:** Linux (tested on Ubuntu with AMD Radeon RX 7900 XTX)

---

## 🎮 Game Concept

A 3D real-time strategy (RTS) game inspired by **Star Wars: Empire at War**, featuring massive-scale battles with thousands of units. The game simulates epic confrontations between two armies of battle droids fighting to destroy each other's command uplink towers.

### Core Gameplay
- **10,000 Unit Battles:** 5,000 droids per team engaging in large-scale warfare
- **Objective-Based:** Destroy the enemy's uplink tower to win
- **Tower Cascade System:** When a tower is destroyed, all friendly units within range explode dramatically
- **Formation Combat:** Squad-based movement with tactical formations
- **Real-time Combat:** Automatic targeting and projectile-based weapon systems

---

## 🏗️ Architecture Overview

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
├── objective.rs         # Tower mechanics, destruction cascade
└── explosion_shader.rs  # Sprite sheet explosion system

assets/
├── shaders/
│   └── explosion.wgsl   # Custom shader for flipbook animation
├── textures/
│   └── Explosion02HD_5x5.tga  # 5x5 sprite sheet (25 frames)
└── audio/
    └── sfx/             # Laser sound effects
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
   - `auto_fire_system` - Automatic firing at targets
   - `volley_fire_system` - Coordinated volley attacks (F key)
   - `update_projectiles` - Moves projectiles
   - `collision_detection_system` - Detects hits and applies damage

4. **Objective System**
   - `tower_targeting_system` - Units target towers when in range
   - `tower_destruction_system` - Handles tower death and cascade
   - `pending_explosion_system` - Manages delayed explosions
   - `explosion_effect_system` - Updates visual effects
   - `win_condition_system` - Checks for game end
   - `update_objective_ui_system` - Updates tower health UI
   - `debug_explosion_hotkey_system` - Debug controls (E key)

5. **Explosion System** (from ExplosionShaderPlugin)
   - `setup_explosion_assets` - Loads sprite sheet and creates materials
   - `update_explosion_timers` - Manages explosion lifetimes
   - `animate_sprite_explosions` - Animates StandardMaterial explosions
   - `animate_custom_shader_explosions` - Animates custom shader explosions
   - `cleanup_finished_explosions` - Removes expired explosions
   - `debug_test_explosions` - Debug spawning (Y/U/I keys)

---

## 📊 Game Configuration (constants.rs)

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

---

## 🎨 Visual Design

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
See `EXPLOSION_SYSTEM.md` for detailed information.

---

## 🎮 Controls

### Camera Controls (RTS-Style)
- **WASD:** Pan camera horizontally
- **Mouse Right-Click + Drag:** Rotate camera
- **Scroll Wheel:** Zoom in/out
- **Focus:** Auto-centers on battlefield

### Combat Commands
- **F Key:** Volley Fire - coordinated attack from all units
- **G Key:** Advance formation
- **H Key:** Retreat formation

### Formation Commands
- **Q Key:** Rectangular formation
- **E Key:** Line formation
- **R Key:** Box formation
- **T Key:** Wedge formation

### Debug Controls
- **E Key:** Trigger Team B tower destruction (for testing explosions)
- **Y Key:** Spawn test animated sprite explosion (StandardMaterial)
- **U Key:** Spawn test custom shader explosion (primary method)
- **I Key:** Spawn test solid color explosion (positioning test)

---

## 🔧 Technical Details

### Performance Optimizations
1. **Spatial Partitioning:** 10×10 grid system for efficient collision detection
2. **Squad System:** Groups of 50 units managed together, reducing individual queries
3. **Target Caching:** Targets updated every 2 seconds, not every frame
4. **Instancing:** Single mesh shared across all units of same type
5. **Conditional Systems:** Systems only run when needed (e.g., formation changes)
6. **Release Profile:** LTO enabled, optimized compilation settings

### Rendering
- **Backend:** Vulkan (with AMD GPU workarounds required)
- **Materials:** PBR StandardMaterial for units/towers
- **Custom Shaders:** Used for explosion sprite sheet animation
- **Billboard Quads:** Explosions always face camera
- **Shadow Control:** Units cast/receive shadows; explosions do not

### AMD GPU Compatibility

**Required Launch Command:**
```bash
VK_LOADER_DEBUG=error VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json cargo run --release
```

This works around segfault issues with AMD drivers on Linux.

---

## 🐛 Known Issues & Limitations

### Current Limitations
1. **Static Explosions:** Billboarded sprite quads, no 3D volume
2. **No Particle Systems:** Pure sprite sheet approach (planned enhancement)
3. **Limited Audio:** Only laser sound effects, no explosion sounds yet
4. **No Unit Selection:** No RTS-style unit selection/commands (planned)
5. **Fixed Formations:** Formations are preset, not customizable in-game
6. **No Pathfinding:** Units march in straight lines, no obstacle avoidance
7. **Simple AI:** Units auto-target nearest enemy, no tactics

### Known Bugs
- ✅ Fixed: Explosion shadows (added NotShadowCaster/NotShadowReceiver)
- ✅ Fixed: Overlapping explosions (disabled phase transitions)
- ✅ Fixed: White quad explosions (updated to use sprite sheet)
- ✅ Fixed: Entity despawn panic (added safe entity checks)
- ⚠️ In Progress: Explosion animation not playing properly

---

## 🎯 Design Goals

### Achieved
- ✅ Massive unit counts (10,000 concurrent units)
- ✅ Performant rendering and simulation
- ✅ Squad-based formation system
- ✅ Objective-based gameplay (tower destruction)
- ✅ Dramatic cascade explosion system
- ✅ RTS-style camera controls
- ✅ Procedural unit and tower generation
- ✅ Sprite sheet explosion system

### Planned Enhancements
- 🔲 GPU-based particle systems for debris/sparks
- 🔲 Explosion audio integration
- 🔲 Screen-space distortion effects (heat waves)
- 🔲 Camera shake on explosions
- 🔲 Unit ragdoll/death animations
- 🔲 Physics-based debris
- 🔲 LOD system for distant units
- 🔲 Explosion pooling for performance
- 🔲 More formation types
- 🔲 Unit selection and manual commands
- 🔲 Pathfinding system
- 🔲 Tactical AI behaviors

---

## 📚 Project History

### Evolution of Explosion System
1. **Procedural Approach (v1):** Complex noise-based shaders - "super ugly", abandoned
2. **Smoke-Only Sprites (v2):** Sprite sheet with normal maps - "didn't work well"
3. **8×8 Flipbook (v3):** Initial sprite sheet approach with 64 frames
4. **5×5 Flipbook (v4, Current):** Self-contained explosion with 25 frames, all phases baked in

### Major Milestones
- **Initial Development:** 10,000 unit simulation with basic combat
- **June 2025:** Implemented sprite sheet explosion system
- **June 15, 2025:** Fixed shadow artifacts and overlapping explosions
- **November 12, 2025:** Updated to Bevy 0.14.2, fixed AMD GPU compatibility

---

## 🛠️ Build & Run

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

---

## 📝 Code Style & Conventions

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

## 🤝 For Future AI Agents

### Key Files to Understand
1. **types.rs** - Core data structures, understand these first
2. **constants.rs** - All tunable parameters
3. **main.rs** - System registration order (important!)
4. **explosion_shader.rs** - See EXPLOSION_SYSTEM.md for details

### Common Tasks
- **Adding new unit types:** Modify `setup.rs`, add to `types.rs`
- **Tweaking gameplay:** Edit `constants.rs` values
- **New formations:** Add to `formation.rs`
- **Combat changes:** Modify `combat.rs` systems
- **Visual effects:** Update `explosion_shader.rs` or materials

### Debugging Tips
- Use `info!()` macros for logging (already extensively used)
- Check entity counts with queries in debug systems
- Monitor FPS with bevy's diagnostic plugin (already integrated)
- Use the debug keys (E, Y, U, I) to test specific features

### Performance Considerations
- This project pushes Bevy's limits with 10,000+ entities
- Spatial partitioning is critical for collision detection
- Squad system reduces query overhead
- Any new systems should batch operations when possible

---

## 📞 Support & Resources

### Bevy Resources
- **Bevy Documentation:** https://bevyengine.org/learn/
- **Bevy Discord:** For engine-specific questions
- **Error Codes:** https://bevyengine.org/learn/errors/

### Project-Specific Documentation
- **EXPLOSION_SYSTEM.md** - Detailed explosion implementation
- **devlogs/** - Historical development notes and fixes

---

**End of Overview**


# Turrets

## Overview
Two procedural turret types provide defensive fire for Team A, featuring different firing characteristics and targeting behaviors.

## Turret Types

### Heavy Turret
- **Location**: (30, 30) on terrain
- **Fire Rate**: 2.0s interval
- **Barrels**: Dual barrels (alternating fire)
- **Design**: Reinforced bunker base with armored housing
- **Role**: Anti-vehicle, sustained fire

### MG Turret
- **Location**: (10, 10) on terrain
- **Fire Rate**: 0.08s interval (12.5 shots/sec)
- **Barrels**: Single barrel
- **Projectile Speed**: 3x standard (300 units/sec)
- **Firing Modes**:
  - **Burst**: Fixed 40 shots, 1.0s cooldown
  - **Continuous**: 45 shots with instant target switching, 1.5s cooldown
- **Role**: Anti-infantry suppression fire

## Technical Details

### Architecture
- **Parent-child entity structure**: Static base (parent) + rotating assembly (child)
- **Rotation**: Y-axis only, smooth interpolation with `Quat::slerp()`
- **Targeting**: Integrates with existing `target_acquisition_system` and `auto_fire_system`
- **Line-of-Sight**: Respects terrain blocking via heightmap sampling

### Rapid Target Switching (MG Turret)
When in Continuous mode, the MG turret:
- Instantly retargets when current enemy dies mid-burst
- Scans for closest enemy in 150-unit range
- Maintains burst counter across target switches
- Creates suppression fire effect

### Map Switching
Turrets despawn and respawn at correct terrain heights when switching between Map 1 and Map 2 via the `respawn_turrets_on_map_switch` system.

## Audio
- MG turret has dedicated audio channel (`mg_sound`) at 0.25 volume
- Separate audio counter (`MAX_MG_AUDIO_PER_FRAME: 3`) prevents throttling
- Standard turret uses regular combat audio

## Debug Features
- **Collision Sphere Visualization**: Press `0` then `C` to toggle
- Shows green wireframe spheres (units) and orange spheres (buildings)
- Helps diagnose aiming and hit detection issues

## Known Limitations
- **Projectile waste**: 2-3 wasted shots per kill due to in-flight projectiles (inherent to projectile-based combat at high fire rates)
- **Collision tunneling**: Projectiles faster than 8x speed skip through 1.0 radius collision spheres
- **Current compromise**: 3x speed balances hit detection reliability with MG feel

## Code Organization
- **src/turrets.rs**: Spawn systems and respawn logic
- **src/procedural_meshes.rs**: Mesh generation (bases, assemblies, barrels)
- **src/combat.rs**: MG firing modes, rapid targeting, rotation system
- **src/types.rs**: `MgTurret`, `FiringMode`, `TurretRotatingAssembly` components

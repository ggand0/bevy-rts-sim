# Shield System

**Overview:** Empire at War-style shield defense system protecting uplink towers with regeneration mechanics and visual effects.

## Core Mechanics

**Defense:**
- Hemisphere shields (50-unit radius, 5000 HP max)
- Block all enemy laser projectiles (25 damage per hit)
- Team-colored: Cyan (Team A), Orange (Team B)

**Regeneration:**
- 3-second delay after last hit
- 50 HP/second regen rate
- Shields respawn at 0 HP after 10s destruction delay
- Gradual regeneration from 0 → full (100s total)

## Visual Effects

**Shader (WGSL):**
- Hexagonal energy grid pattern
- Fresnel edge glow (brighter at angles)
- Animated energy pulse waves
- Health-based alpha fade and color shift to white
- Expanding impact ripples (up to 8 simultaneous)

**Particles (Bevy Hanabi):**
- Cyan energy burst on 25% of impacts
- Shield impact sound effect (0.4 volume)

## Configuration

All parameters centralized in `ShieldConfig` resource:
```rust
max_hp: 5000.0
regen_rate: 50.0          // HP/s
regen_delay: 3.0          // Seconds
respawn_delay: 10.0       // Seconds
laser_damage: 25.0
fresnel_power: 3.0
hex_scale: 8.0
mesh_segments: 32
```

## State Machine

```
Active Shield (visible, blocks lasers, regenerates)
    ↓ HP reaches 0
DestroyedShield (marker, 10s countdown)
    ↓ Timer expires (if tower alive)
Respawned Shield (0 HP, regenerating to full)
```

## Systems

1. `shield_collision_system` - Laser impact detection & damage
2. `shield_regeneration_system` - HP recovery over time
3. `shield_impact_flash_system` - Flash timer countdown
4. `shield_health_visual_system` - Material alpha & ripple updates
5. `shield_tower_death_system` - Despawn when tower dies
6. `shield_respawn_system` - Handle respawn countdown
7. `animate_shields` - Shader time animation

## Files

- `src/shield.rs` - Core system implementation
- `assets/shaders/shield.wgsl` - Custom WGSL shader
- `src/particles.rs` - Shield impact particle effects
- `src/types.rs` - `Team::shield_color()` method

## Debug

- Press `0` to toggle debug menu
- Press `S` to instantly destroy Team B shield
- Tower HP: 2000 (increased for testing)

See `devlogs/021_shield-system.md` for detailed implementation notes.

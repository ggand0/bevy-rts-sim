pub const ARMY_SIZE_PER_TEAM: usize = 5_000;
#[allow(dead_code)]
pub const FORMATION_WIDTH: f32 = 200.0;
#[allow(dead_code)]
pub const UNIT_SPACING: f32 = 2.0;
pub const MARCH_DISTANCE: f32 = 150.0;
pub const MARCH_SPEED: f32 = 3.0;
pub const BATTLEFIELD_SIZE: f32 = 400.0;

// Squad and formation constants
pub const SQUAD_SIZE: usize = 50;
pub const SQUAD_WIDTH: usize = 10;  // 10 units wide
pub const SQUAD_DEPTH: usize = 5;   // 5 units deep
pub const SQUAD_HORIZONTAL_SPACING: f32 = 2.0;
pub const SQUAD_VERTICAL_SPACING: f32 = 2.5;
pub const INTER_SQUAD_SPACING: f32 = 12.0; // Tactical spacing for combined arms formations

// RTS Camera settings
pub const CAMERA_SPEED: f32 = 50.0;
pub const CAMERA_ZOOM_SPEED: f32 = 10.0;
pub const CAMERA_MIN_HEIGHT: f32 = 20.0;
pub const CAMERA_MAX_HEIGHT: f32 = 500.0;  // Increased for terrain overview
pub const CAMERA_ROTATION_SPEED: f32 = 0.005;
#[allow(dead_code)]
pub const CAMERA_INITIAL_HEIGHT: f32 = 250.0;  // Higher starting position for terrain

// Laser projectile settings
pub const LASER_SPEED: f32 = 100.0;
pub const LASER_LIFETIME: f32 = 3.0;
pub const LASER_LENGTH: f32 = 3.0;
pub const LASER_WIDTH: f32 = 0.2;

// Combat settings
pub const TARGETING_RANGE: f32 = 150.0;
pub const TARGET_SCAN_INTERVAL: f32 = 2.0;
pub const COLLISION_RADIUS: f32 = 1.0;
pub const AUTO_FIRE_INTERVAL: f32 = 2.0;

// Hitscan weapon settings
pub const HITSCAN_TRACER_SPEED: f32 = 400.0;  // Visual tracer speed (faster than projectiles for snappy feel)
pub const HITSCAN_TRACER_LENGTH: f32 = 4.0;   // Length of the tracer bolt visual
pub const HITSCAN_TRACER_WIDTH: f32 = 0.25;   // Slightly wider than projectiles for visibility
pub const HITSCAN_DAMAGE: f32 = 25.0;         // Damage per hitscan hit (buildings)

// Spatial partitioning settings
pub const GRID_CELL_SIZE: f32 = 10.0; // Size of each grid cell
pub const GRID_SIZE: i32 = 100; // Number of cells per side (covers 1000x1000 area)

// Objective system settings
pub const TOWER_HEIGHT: f32 = 35.0;
pub const TOWER_BASE_WIDTH: f32 = 9.0;
pub const TOWER_MAX_HEALTH: f32 = 2000.0;
pub const TOWER_DESTRUCTION_RADIUS: f32 = 80.0; // Units within this range explode when tower dies
#[allow(dead_code)]
pub const EXPLOSION_DELAY_MIN: f32 = 0.1; // Minimum delay before unit explodes
#[allow(dead_code)]
pub const EXPLOSION_DELAY_MAX: f32 = 2.0; // Maximum delay for dramatic cascade effect
#[allow(dead_code)]
pub const EXPLOSION_TIME_QUANTUM: f32 = 0.05; // Quantize delays to 50ms slots for burst clustering
pub const EXPLOSION_EFFECT_DURATION: f32 = 2.0; // Visual explosion duration
#[allow(dead_code)]
pub const MAX_EXPLOSIONS_PER_FRAME: usize = 50; // Limit explosions per frame to prevent stutter
#[allow(dead_code)]
pub const PARTICLE_SPAWN_PROBABILITY: f32 = 0.3; // Probability (0.0-1.0) that an explosion spawns particles

// Selection system settings
pub const SELECTION_CLICK_RADIUS: f32 = 15.0;       // How close to squad center to select (generous for usability)
pub const SELECTION_RING_INNER_RADIUS: f32 = 8.0;   // Inner radius of selection ring
pub const SELECTION_RING_OUTER_RADIUS: f32 = 10.0;  // Outer radius of selection ring
pub const SELECTION_RING_COLOR: bevy::prelude::Color = bevy::prelude::Color::srgba(0.2, 0.9, 1.0, 0.7); // Cyan
pub const BOX_SELECT_DRAG_THRESHOLD: f32 = 8.0;     // Pixels before box select starts
pub const MOVE_INDICATOR_RADIUS: f32 = 3.0;         // Radius of move destination indicator
pub const MOVE_INDICATOR_LIFETIME: f32 = 1.5;       // Seconds before move indicator fades
pub const SQUAD_ROTATION_SPEED: f32 = 2.0;          // Radians per second for squad rotation
pub const MULTI_SQUAD_SPACING: f32 = 25.0;          // Spacing between squads when moving multiple
pub const SQUAD_ARRIVAL_THRESHOLD: f32 = 5.0;       // Distance at which squads are considered "arrived" at destination

// Terrain generation settings
pub const TERRAIN_GRID_SIZE: usize = 100;           // 100x100 vertices for terrain mesh
pub const TERRAIN_SIZE: f32 = 800.0;                // Match current ground size
pub const TERRAIN_MAX_HEIGHT: f32 = 50.0;           // Maximum hill height
// Perlin noise parameters - currently hardcoded in terrain.rs, kept here for future configurability
#[allow(dead_code)]
pub const PERLIN_SCALE: f64 = 0.02;                 // Noise frequency (lower = larger hills)
#[allow(dead_code)]
pub const PERLIN_OCTAVES: usize = 4;                // Detail levels for fractal noise
#[allow(dead_code)]
pub const PERLIN_PERSISTENCE: f64 = 0.5;            // How much each octave contributes
#[allow(dead_code)]
pub const PERLIN_LACUNARITY: f64 = 2.0;             // Frequency multiplier per octave

// Audio volume settings
pub const VOLUME_EXPLOSION: f32 = 0.5;              // Tower/unit explosion volume
pub const VOLUME_TURRET_EXPLOSION: f32 = 0.3;      // Turret explosion volume (smaller than tower)
#[allow(dead_code)]
pub const VOLUME_LASER: f32 = 0.3;                  // Laser fire volume (droids and turrets)
#[allow(dead_code)]
pub const VOLUME_SHIELD_IMPACT: f32 = 0.4;          // Shield impact volume (moved to ShieldConfig, kept for reference)
pub const VOLUME_MG_TURRET: f32 = 0.1;             // MG turret max volume (proximity-based)
pub const VOLUME_HEAVY_TURRET: f32 = 0.015;         // Heavy turret max volume (proximity-based)

// Proximity-based audio attenuation
// Note: RTS camera sits at ~150-200 units height, so we need larger distances
pub const AUDIO_MIN_DISTANCE: f32 = 100.0;  // Full volume below this distance
pub const AUDIO_MAX_DISTANCE: f32 = 400.0;  // Minimum volume above this distance
pub const AUDIO_MIN_VOLUME: f32 = 0.005;    // Volume at max distance

/// Calculate distance-based volume attenuation for spatial audio
/// Returns a volume multiplier between min_volume and max_volume based on distance
#[inline]
pub fn proximity_volume(distance: f32, max_volume: f32) -> f32 {
    if distance <= AUDIO_MIN_DISTANCE {
        max_volume
    } else if distance >= AUDIO_MAX_DISTANCE {
        AUDIO_MIN_VOLUME
    } else {
        let t = (distance - AUDIO_MIN_DISTANCE) / (AUDIO_MAX_DISTANCE - AUDIO_MIN_DISTANCE);
        max_volume - t * (max_volume - AUDIO_MIN_VOLUME)
    }
}

// ===== AREA DAMAGE SYSTEM =====

/// Area damage zones (base radii, scaled by explosion scale parameter)
pub const AREA_DAMAGE_CORE_RADIUS: f32 = 5.0;   // Instant death zone
pub const AREA_DAMAGE_MID_RADIUS: f32 = 12.0;   // RNG death zone (probability decreases with distance)
pub const AREA_DAMAGE_RIM_RADIUS: f32 = 20.0;   // Knockback only zone

/// Knockback physics (for units in rim zone)
pub const KNOCKBACK_BASE_SPEED: f32 = 15.0;     // Base launch velocity
pub const KNOCKBACK_GRAVITY: f32 = -30.0;       // Gravity acceleration
pub const KNOCKBACK_STUN_DURATION: f32 = 1.0;   // Post-landing stun (no move/shoot)
pub const KNOCKBACK_TILT_MAX: f32 = 0.6;        // Max tilt angle in radians (~35 degrees)
pub const KNOCKBACK_TILT_SPEED: f32 = 8.0;      // How fast to reach max tilt while airborne
pub const KNOCKBACK_RECOVER_SPEED: f32 = 3.0;   // How fast to recover to upright during stun

/// Ragdoll death physics (50% of deaths in core/mid zones)
pub const RAGDOLL_MIN_SPEED: f32 = 20.0;        // Min launch velocity
pub const RAGDOLL_MAX_SPEED: f32 = 35.0;        // Max launch velocity
pub const RAGDOLL_GRAVITY: f32 = -25.0;         // Slightly slower fall for visual effect

// ===== ARTILLERY SYSTEM =====

pub const ARTILLERY_SCATTER_RADIUS: f32 = 25.0;       // XZ scatter for scatter barrage
pub const ARTILLERY_SHELL_COUNT_MIN: usize = 6;       // Min shells per scatter barrage
pub const ARTILLERY_SHELL_COUNT_MAX: usize = 10;      // Max shells per scatter barrage
pub const ARTILLERY_SHELL_DELAY_MIN: f32 = 0.3;       // Min delay between shells
pub const ARTILLERY_SHELL_DELAY_MAX: f32 = 1.5;       // Max delay between shells
pub const ARTILLERY_LINE_MAX_LENGTH: f32 = 100.0;     // Max line barrage length
pub const ARTILLERY_LINE_SHELL_SPACING: f32 = 15.0;   // Spacing between shells on line

// ===== UNIT COLLISION SYSTEM =====

/// Physical collision radius for unit-unit collision (slightly less than spacing/2)
pub const UNIT_COLLISION_RADIUS: f32 = 0.8;
/// Soft avoidance starts slowing units at this distance
pub const SOFT_AVOIDANCE_RADIUS: f32 = 2.0;
/// Soft avoidance strength: 0.0 = off, 1.0 = full. Adjustable for tuning.
pub const SOFT_AVOIDANCE_STRENGTH: f32 = 1.0;
/// Push force multiplier for hard collision resolution
pub const COLLISION_PUSH_STRENGTH: f32 = 8.0;
/// Default mass for battle droids (future: heavy platforms = 5.0, drones = 0.3)
pub const DEFAULT_UNIT_MASS: f32 = 1.0;
pub const ARMY_SIZE_PER_TEAM: usize = 5_000;
pub const FORMATION_WIDTH: f32 = 200.0;
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
pub const CAMERA_MAX_HEIGHT: f32 = 200.0;
pub const CAMERA_ROTATION_SPEED: f32 = 0.005;

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

// Spatial partitioning settings
pub const GRID_CELL_SIZE: f32 = 10.0; // Size of each grid cell
pub const GRID_SIZE: i32 = 100; // Number of cells per side (covers 1000x1000 area) 
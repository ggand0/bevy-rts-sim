use bevy::prelude::*;
use std::collections::HashMap;
use rand::Rng;

#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub enum Team {
    A,
    B,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FormationType {
    Rectangle,
}

#[derive(Component)]
pub struct Squad {
    #[allow(dead_code)]
    pub id: u32,
    pub team: Team,
    pub formation_type: FormationType,
    pub commander: Option<Entity>,
    pub members: Vec<Entity>,
    pub center_position: Vec3,
    pub facing_direction: Vec3,
    pub target_facing_direction: Vec3,  // Direction to rotate toward (for smooth rotation)
    pub target_position: Vec3,
}

impl Squad {
    pub fn new(id: u32, team: Team, center_position: Vec3, facing_direction: Vec3) -> Self {
        Self {
            id,
            team,
            formation_type: FormationType::Rectangle,
            commander: None,
            members: Vec::new(),
            center_position,
            facing_direction,
            target_facing_direction: facing_direction,  // Initially same as facing
            target_position: center_position,
        }
    }
    
    pub fn add_member(&mut self, entity: Entity) {
        if self.members.len() < crate::constants::SQUAD_SIZE {
            self.members.push(entity);
        }
    }
    
    pub fn remove_member(&mut self, entity: Entity) {
        self.members.retain(|&e| e != entity);
    }
    
    pub fn promote_new_commander(&mut self) -> Option<Entity> {
        // Find the unit closest to the rear-center position for promotion
        if !self.members.is_empty() {
            // For now, just promote the first available member
            // In a more sophisticated system, we'd consider position and rank
            let new_commander = self.members[0];
            self.commander = Some(new_commander);
            Some(new_commander)
        } else {
            self.commander = None;
            None
        }
    }
}

#[derive(Component)]
pub struct SquadMember {
    pub squad_id: u32,
    pub formation_position: (usize, usize), // (row, column) in formation
    pub is_commander: bool,
}

#[derive(Component)]
pub struct FormationOffset {
    pub local_offset: Vec3, // Relative position to squad center
    pub target_world_position: Vec3, // Where this unit should be
}

#[derive(Component)]
pub struct BattleDroid {
    pub march_speed: f32,
    pub spawn_position: Vec3,
    pub target_position: Vec3,
    pub march_offset: f32,
    pub returning_to_spawn: bool,
    pub team: Team,
}

#[allow(dead_code)]
#[derive(Component)]
pub struct FormationUnit {
    pub formation_index: usize,
    pub row: usize,
    pub column: usize,
}

#[derive(Component)]
pub struct RtsCamera {
    pub focus_point: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

#[derive(Component)]
pub struct LaserProjectile {
    pub velocity: Vec3,
    pub lifetime: f32,
    pub team: Team, // Track which team fired this laser
}

#[derive(Component)]
pub struct CombatUnit {
    pub target_scan_timer: f32,
    pub auto_fire_timer: f32,
    pub current_target: Option<Entity>,
}

// Audio resources
#[derive(Resource)]
pub struct AudioAssets {
    pub laser_sounds: Vec<Handle<AudioSource>>,
    pub explosion_sound: Handle<AudioSource>,
}

impl AudioAssets {
    pub fn get_random_laser_sound(&self, rng: &mut rand::rngs::ThreadRng) -> Handle<AudioSource> {
        let index = rng.gen_range(0..self.laser_sounds.len());
        self.laser_sounds[index].clone()
    }
}

// Spatial grid for collision optimization
#[derive(Resource, Default)]
pub struct SpatialGrid {
    // Grid cells containing entity IDs - [x][y]
    pub laser_cells: Vec<Vec<Vec<Entity>>>,
    pub droid_cells: Vec<Vec<Vec<Entity>>>,
}

impl SpatialGrid {
    pub fn new() -> Self {
        let size = crate::constants::GRID_SIZE as usize;
        Self {
            laser_cells: vec![vec![Vec::new(); size]; size],
            droid_cells: vec![vec![Vec::new(); size]; size],
        }
    }
    
    pub fn clear(&mut self) {
        for row in &mut self.laser_cells {
            for cell in row {
                cell.clear();
            }
        }
        for row in &mut self.droid_cells {
            for cell in row {
                cell.clear();
            }
        }
    }
    
    pub fn world_to_grid(pos: Vec3) -> (i32, i32) {
        let x = ((pos.x + crate::constants::GRID_SIZE as f32 * crate::constants::GRID_CELL_SIZE * 0.5) / crate::constants::GRID_CELL_SIZE) as i32;
        let z = ((pos.z + crate::constants::GRID_SIZE as f32 * crate::constants::GRID_CELL_SIZE * 0.5) / crate::constants::GRID_CELL_SIZE) as i32;
        (x.clamp(0, crate::constants::GRID_SIZE - 1), z.clamp(0, crate::constants::GRID_SIZE - 1))
    }
    
    #[allow(dead_code)]
    pub fn add_laser(&mut self, entity: Entity, pos: Vec3) {
        let (x, z) = Self::world_to_grid(pos);
        self.laser_cells[x as usize][z as usize].push(entity);
    }
    
    pub fn add_droid(&mut self, entity: Entity, pos: Vec3) {
        let (x, z) = Self::world_to_grid(pos);
        self.droid_cells[x as usize][z as usize].push(entity);
    }
    
    pub fn get_nearby_droids(&self, pos: Vec3) -> Vec<Entity> {
        let (center_x, center_z) = Self::world_to_grid(pos);
        let mut nearby = Vec::new();
        
        // Check 3x3 grid around the position to account for collision radius
        for dx in -1..=1 {
            for dz in -1..=1 {
                let x = center_x + dx;
                let z = center_z + dz;
                if x >= 0 && x < crate::constants::GRID_SIZE && z >= 0 && z < crate::constants::GRID_SIZE {
                    nearby.extend(&self.droid_cells[x as usize][z as usize]);
                }
            }
        }
        nearby
    }
}

// Squad management resource
#[derive(Resource, Default)]
pub struct SquadManager {
    pub squads: HashMap<u32, Squad>,
    pub next_squad_id: u32,
    pub entity_to_squad: HashMap<Entity, u32>, // Quick lookup: entity -> squad_id
}

impl SquadManager {
    pub fn new() -> Self {
        Self {
            squads: HashMap::new(),
            next_squad_id: 0,
            entity_to_squad: HashMap::new(),
        }
    }
    
    pub fn create_squad(&mut self, team: Team, center_position: Vec3, facing_direction: Vec3) -> u32 {
        let squad_id = self.next_squad_id;
        self.next_squad_id += 1;
        
        let squad = Squad::new(squad_id, team, center_position, facing_direction);
        self.squads.insert(squad_id, squad);
        squad_id
    }
    
    pub fn add_unit_to_squad(&mut self, squad_id: u32, entity: Entity) {
        if let Some(squad) = self.squads.get_mut(&squad_id) {
            squad.add_member(entity);
            self.entity_to_squad.insert(entity, squad_id);
        }
    }
    
    pub fn remove_unit_from_squad(&mut self, entity: Entity) -> Option<u32> {
        if let Some(&squad_id) = self.entity_to_squad.get(&entity) {
            if let Some(squad) = self.squads.get_mut(&squad_id) {
                squad.remove_member(entity);
                
                // If this was the commander, promote someone else
                if squad.commander == Some(entity) {
                    squad.promote_new_commander();
                }
            }
            self.entity_to_squad.remove(&entity);
            Some(squad_id)
        } else {
            None
        }
    }
    
    pub fn get_squad(&self, squad_id: u32) -> Option<&Squad> {
        self.squads.get(&squad_id)
    }
    
    pub fn get_squad_mut(&mut self, squad_id: u32) -> Option<&mut Squad> {
        self.squads.get_mut(&squad_id)
    }
    
    #[allow(dead_code)]
    pub fn get_unit_squad_id(&self, entity: Entity) -> Option<u32> {
        self.entity_to_squad.get(&entity).copied()
    }
}

// ===== OBJECTIVE SYSTEM COMPONENTS =====

#[derive(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max_health: f32) -> Self {
        Self {
            current: max_health,
            max: max_health,
        }
    }
    
    pub fn damage(&mut self, amount: f32) {
        self.current = (self.current - amount).max(0.0);
    }
    
    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }
    
    pub fn health_percentage(&self) -> f32 {
        self.current / self.max
    }
}

#[derive(Component)]
pub struct UplinkTower {
    pub team: Team,
    pub destruction_radius: f32, // Radius for chain reaction
}

#[allow(dead_code)]
#[derive(Component)]
pub struct ObjectiveTarget {
    pub team: Team,
    pub is_primary: bool, // Primary objectives end the game when destroyed
}

// Turret components
#[derive(Component)]
pub struct TurretBase; // Marker for static turret base

#[derive(Component)]
pub struct TurretRotatingAssembly {
    pub current_barrel_index: usize, // 0-3 for four barrels in 2x2 arrangement
}

// Building collision component
#[derive(Component)]
pub struct BuildingCollider {
    pub radius: f32, // Collision radius for laser blocking
}

// PendingExplosion and ExplosionEffect moved to src/explosion_system.rs

// Game state management
#[derive(Resource, Default)]
pub struct GameState {
    pub team_a_tower_destroyed: bool,
    pub team_b_tower_destroyed: bool,
    pub game_ended: bool,
    pub winner: Option<Team>,
}

impl GameState {
    pub fn tower_destroyed(&mut self, team: Team) {
        match team {
            Team::A => self.team_a_tower_destroyed = true,
            Team::B => self.team_b_tower_destroyed = true,
        }
        
        if !self.game_ended {
            self.game_ended = true;
            // Winner is the opposing team
            self.winner = Some(match team {
                Team::A => Team::B,
                Team::B => Team::A,
            });
        }
    }
} 
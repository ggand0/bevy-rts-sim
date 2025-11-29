// Selection state and shared types
use bevy::prelude::*;
use std::collections::HashMap;

use super::groups::SquadGroup;

/// Selection state resource - tracks which squads are selected (Vec preserves selection order)
#[derive(Resource)]
pub struct SelectionState {
    pub selected_squads: Vec<u32>,  // First element is primary selection
    pub box_select_start: Option<Vec2>,  // Screen-space start position for box selection
    pub is_box_selecting: bool,
    pub drag_start_world: Option<Vec3>,  // World position where drag started
    // Right-click drag for orientation (CoH1-style)
    pub move_drag_start: Option<Vec3>,   // World position where right-click started
    pub move_drag_current: Option<Vec3>, // Current drag position (for arrow visual)
    pub is_orientation_dragging: bool,   // True when drag exceeds threshold
    // Squad grouping
    pub groups: HashMap<u32, SquadGroup>,
    pub squad_to_group: HashMap<u32, u32>,
    pub next_group_id: u32,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            selected_squads: Vec::new(),
            box_select_start: None,
            is_box_selecting: false,
            drag_start_world: None,
            move_drag_start: None,
            move_drag_current: None,
            is_orientation_dragging: false,
            groups: HashMap::new(),
            squad_to_group: HashMap::new(),
            next_group_id: 1,
        }
    }
}

// Marker component for selection ring visuals
#[derive(Component)]
pub struct SelectionVisual {
    pub squad_id: u32,
    pub is_grouped: bool,  // Track if currently showing grouped color
}

// Marker component for move order destination indicator (circle at destination)
#[derive(Component)]
pub struct MoveOrderVisual {
    pub timer: Timer,
    pub base_color: Color,  // Original color for fade-out
}

// Marker component for path line connecting squad to destination
#[derive(Component)]
pub struct MovePathVisual {
    pub timer: Timer,
}

// Marker component for orientation arrow during right-click drag
#[derive(Component)]
pub struct OrientationArrowVisual;

// Marker component for box selection rectangle visual (UI element)
#[derive(Component)]
pub struct BoxSelectionVisual;

// Marker component for group orientation indicator
#[derive(Component)]
pub struct GroupOrientationMarker {
    pub group_id: u32,
}

// Marker component for debug bounding rectangle
#[derive(Component)]
pub struct GroupBoundingBoxDebug {
    pub group_id: u32,
}

// Threshold for orientation drag (in world units)
pub const ORIENTATION_DRAG_THRESHOLD: f32 = 3.0;

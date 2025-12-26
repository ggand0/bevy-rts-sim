// Selection module - RTS-style squad selection and movement controls
//
// Submodules:
// - state: SelectionState resource and marker components
// - groups: Squad grouping logic (Total War-style formation preservation)
// - input: Selection input handling (click, box select)
// - movement: Move command handling (right-click with orientation drag)
// - visuals: Visual feedback systems (rings, arrows, path lines)
// - obb: Oriented Bounding Box calculations
// - utils: Shared utility functions

mod state;
mod groups;
mod input;
mod movement;
pub mod visuals;
mod obb;
pub mod utils;
mod ui;

// Re-export main types for external use
pub use state::SelectionState;
pub use utils::screen_to_ground_with_heightmap;

// Re-export systems for main.rs
pub use input::{selection_input_system, box_selection_update_system, hold_command_system};
pub use movement::move_command_system;
pub use groups::group_command_system;
pub use visuals::{
    selection_visual_system,
    move_visual_cleanup_system,
    orientation_arrow_system,
    box_selection_visual_system,
    update_group_orientation_markers,
    update_group_bounding_box_debug,
    update_squad_path_arrows,
};
pub use ui::{spawn_squad_details_ui, update_squad_details_ui};

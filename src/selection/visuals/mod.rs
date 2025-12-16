// Visual systems for selection feedback
//
// Submodules:
// - selection: Selection ring visuals (cyan rings under selected squads)
// - movement: Move indicators, path lines, orientation arrows, path arrows
// - group: Group orientation markers and bounding box debug

mod selection;
pub mod movement;
mod group;

// Re-export systems
pub use selection::{selection_visual_system, box_selection_visual_system};
pub use movement::{
    move_visual_cleanup_system,
    orientation_arrow_system,
    update_squad_path_arrows,
    spawn_move_indicator,
    spawn_move_indicator_with_color,
    spawn_path_line,
};
pub use group::{update_group_orientation_markers, update_group_bounding_box_debug};

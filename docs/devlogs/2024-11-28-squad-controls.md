# Squad Selection and Movement Controls

**Date:** 2024-11-28
**Branch:** `feat/squad-controls`

## Overview

Implemented Total War / Company of Heroes style RTS controls for squad selection and movement. Players can now select their squads and issue move commands with orientation control.

## Features Implemented

### 1. Squad Selection System
- **Left-click selection**: Click near a squad to select it
- **Shift+click**: Add/remove squads from selection (toggle)
- **Box selection**: Left-click drag to draw selection rectangle, selects all squads within on release
- **Selection visuals**: Cyan ring appears under selected squads, follows unit positions in real-time
- **Team filtering**: Only player's team (Team::A) can be selected

### 2. Movement Commands
- **Right-click to move**: Selected squads move to clicked destination
- **Multi-squad line formation**: When multiple squads are selected, they spread into a line perpendicular to facing direction
- **Closest-destination assignment**: Each squad picks the closest available slot in the formation line (prevents crossing paths)
- **Path visuals**: Green destination circles and connecting path lines show where each squad is heading

### 3. Orientation Control (CoH1-style)
- **Right-click drag**: Hold right-click and drag to set squad facing direction
- **Visual arrow**: Green arrow shows the orientation during drag
- **Unified facing**: All selected squads face the same direction after moving

### 4. Formation Alignment
- **Target-position anchoring**: Squads align to their target position for clean formations
- **Smooth arrival blend**: Formation correction gradually increases as units approach destination (5→1 unit transition zone)
- **Stationary rotation**: Units smoothly rotate to face squad's orientation when stationary

### 5. Bug Fixes
- **Dead squad cleanup**: Selection markers removed when all units in a squad die
- **Actual position tracking**: Selection rings and path visuals use real unit positions, not lagging squad center
- **Front-row facing**: Fixed formation so front row faces movement direction, commander at rear

## Technical Details

### New Files
- `src/selection.rs` (~1000 lines) - All selection and command systems

### Key Systems
| System | Purpose |
|--------|---------|
| `selection_input_system` | Handle click selection |
| `box_selection_update_system` | Handle drag box selection |
| `box_selection_visual_system` | Render selection rectangle UI |
| `move_command_system` | Process right-click move commands |
| `selection_visual_system` | Manage selection ring visuals |
| `move_visual_cleanup_system` | Fade and despawn path visuals |
| `orientation_arrow_system` | Show orientation arrow during drag |

### Key Design Decisions
1. **SelectionState as Resource** - Squads are data in HashMap, not entities, so marker components weren't suitable
2. **Screen-to-ground raycast** - Manual ray-plane math for flat ground at Y=-1.0
3. **Greedy slot assignment** - Squads sorted by distance, each picks closest available destination slot
4. **Smooth formation correction** - Uses `arrival_blend` factor to prevent snapping

### Constants Added (`src/constants.rs`)
```rust
pub const SELECTION_CLICK_RADIUS: f32 = 15.0;
pub const SELECTION_RING_INNER_RADIUS: f32 = 8.0;
pub const SELECTION_RING_OUTER_RADIUS: f32 = 10.0;
pub const BOX_SELECT_DRAG_THRESHOLD: f32 = 8.0;
pub const MOVE_INDICATOR_RADIUS: f32 = 3.0;
pub const MOVE_INDICATOR_LIFETIME: f32 = 1.5;
pub const SQUAD_ROTATION_SPEED: f32 = 2.0;
pub const MULTI_SQUAD_SPACING: f32 = 25.0;
```

## Controls Summary

| Input | Action |
|-------|--------|
| Left-click | Select squad |
| Shift + Left-click | Add/remove from selection |
| Left-click drag | Box select multiple squads |
| Right-click | Move selected squads |
| Right-click drag | Move with custom orientation |
| Middle-mouse drag | Rotate camera |
| WASD / Arrows | Pan camera |
| Scroll wheel | Zoom camera |
| G | Advance all squads (debug) |
| H | Retreat all squads (debug) |

## Commits

1. `e9f5ae0` - Add squad selection and move commands
2. `c0c9fc1` - Fix selection marker using actual unit positions
3. `ce3d638` - Fix squad orientation: front row faces movement direction
4. `39dff62` - WIP: Multi-squad movement with spacing
5. `0812dae` - Multi-squad movement: unified facing and orthogonal line formation
6. `8d8e1e7` - Multi-squad movement: closest-destination assignment
7. `af3d75b` - Add right-click drag to set squad orientation (CoH1-style)
8. `bc251b2` - Add path visuals: destination circles and connecting lines
9. `f675d3a` - Fix path visuals to use actual unit positions
10. `3eff564` - Units face squad orientation when stationary
11. `6cc7588` - WIP: Fix squad alignment at destination
12. `095632f` - Smooth formation correction with gradual arrival blend
13. `edfbfb9` - Add visual box selection for multi-squad control
14. `315e19f` - Fix selection persisting on dead squads
15. `9cc74ea` - Restrict selection to player team only

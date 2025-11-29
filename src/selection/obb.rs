// Oriented Bounding Box calculations for group visualization
use bevy::prelude::*;

/// Oriented Bounding Box (OBB) result for group visualization
pub struct OrientedBoundingBox {
    pub center: Vec3,           // Center of the OBB in world space
    pub half_extents: Vec2,     // Half-width (right) and half-depth (forward) in local space
    pub facing: Vec3,           // Forward direction (normalized)
    pub right: Vec3,            // Right direction (normalized)
}

impl OrientedBoundingBox {
    /// Calculate OBB from squad positions aligned to a facing direction
    pub fn from_squads(squad_positions: &[Vec3], facing: Vec3, padding: f32) -> Option<Self> {
        if squad_positions.is_empty() {
            return None;
        }

        // Calculate center
        let center: Vec3 = squad_positions.iter().copied().sum::<Vec3>() / squad_positions.len() as f32;

        // Calculate right vector (perpendicular to facing in XZ plane)
        let right = Vec3::new(facing.z, 0.0, -facing.x).normalize_or_zero();

        // If facing is zero, fall back to axis-aligned
        let (facing, right) = if facing.length() < 0.1 {
            (Vec3::Z, Vec3::X)
        } else {
            (facing.normalize(), right)
        };

        // Transform squad positions to local space (relative to center, aligned to facing)
        let mut min_right = f32::MAX;
        let mut max_right = f32::MIN;
        let mut min_forward = f32::MAX;
        let mut max_forward = f32::MIN;

        for &pos in squad_positions {
            let relative = pos - center;
            let local_right = relative.dot(right);
            let local_forward = relative.dot(facing);

            min_right = min_right.min(local_right);
            max_right = max_right.max(local_right);
            min_forward = min_forward.min(local_forward);
            max_forward = max_forward.max(local_forward);
        }

        // Add padding and ensure minimum size
        let min_dimension = 5.0;
        let half_width = ((max_right - min_right) / 2.0 + padding / 2.0).max(min_dimension);
        let half_depth = ((max_forward - min_forward) / 2.0 + padding / 2.0).max(min_dimension);

        // Adjust center to be at the actual center of the OBB (not just average of squad positions)
        let center_offset_right = (min_right + max_right) / 2.0;
        let center_offset_forward = (min_forward + max_forward) / 2.0;
        let adjusted_center = center + right * center_offset_right + facing * center_offset_forward;

        Some(Self {
            center: adjusted_center,
            half_extents: Vec2::new(half_width, half_depth),
            facing,
            right,
        })
    }

    /// Get the 4 corners of the OBB in world space (bottom-left, bottom-right, top-right, top-left)
    /// where "top" is the front (facing direction)
    pub fn corners(&self, y: f32) -> [Vec3; 4] {
        let hw = self.half_extents.x;
        let hd = self.half_extents.y;

        [
            self.center + self.right * (-hw) + self.facing * (-hd) + Vec3::Y * y, // back-left
            self.center + self.right * hw + self.facing * (-hd) + Vec3::Y * y,    // back-right
            self.center + self.right * hw + self.facing * hd + Vec3::Y * y,       // front-right
            self.center + self.right * (-hw) + self.facing * hd + Vec3::Y * y,    // front-left
        ]
    }

    /// Get the front edge center position
    pub fn front_edge_center(&self, y: f32) -> Vec3 {
        self.center + self.facing * self.half_extents.y + Vec3::Y * y
    }
}

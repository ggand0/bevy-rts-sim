//! Procedural mesh generation for game structures
//!
//! This module contains all procedural mesh generation functions for:
//! - Uplink towers
//! - Heavy turrets (base and rotating assembly)
//! - MG turrets (base and rotating assembly)

use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use std::f32::consts::PI;

use crate::constants::*;

// ============================================================================
// UPLINK TOWER MESH
// ============================================================================

pub fn create_uplink_tower_mesh(meshes: &mut ResMut<Assets<Mesh>>) -> Handle<Mesh> {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Helper function to add a box with proper normals
    let mut add_box = |center: Vec3, size: Vec3| {
        let base = vertices.len() as u32;
        let hw = size.x / 2.0;
        let hh = size.y / 2.0;
        let hd = size.z / 2.0;

        // 8 vertices of the box
        let box_vertices = [
            [center.x - hw, center.y - hh, center.z - hd], // 0: bottom-left-back
            [center.x + hw, center.y - hh, center.z - hd], // 1: bottom-right-back
            [center.x + hw, center.y - hh, center.z + hd], // 2: bottom-right-front
            [center.x - hw, center.y - hh, center.z + hd], // 3: bottom-left-front
            [center.x - hw, center.y + hh, center.z - hd], // 4: top-left-back
            [center.x + hw, center.y + hh, center.z - hd], // 5: top-right-back
            [center.x + hw, center.y + hh, center.z + hd], // 6: top-right-front
            [center.x - hw, center.y + hh, center.z + hd], // 7: top-left-front
        ];

        vertices.extend_from_slice(&box_vertices);

        // Proper face normals for each vertex - one normal per vertex per face
        // We'll use proper per-face normals
        let _face_normals = [
            [0.0, -1.0, 0.0], // bottom face normal
            [0.0, 1.0, 0.0],  // top face normal
            [-1.0, 0.0, 0.0], // left face normal
            [1.0, 0.0, 0.0],  // right face normal
            [0.0, 0.0, -1.0], // back face normal
            [0.0, 0.0, 1.0],  // front face normal
        ];

        // Add normals for each vertex (we'll use averaged normals for simplicity)
        for _ in 0..8 {
            normals.push([0.0, 1.0, 0.0]); // For now, keep simple upward normals
        }

        // Box face indices (12 triangles) - Fixed winding order
        let box_indices = [
            // Bottom face (looking up from below)
            base + 0, base + 1, base + 2, base + 0, base + 2, base + 3,
            // Top face (looking down from above)
            base + 4, base + 6, base + 5, base + 4, base + 7, base + 6,
            // Left face
            base + 0, base + 7, base + 4, base + 0, base + 3, base + 7,
            // Right face
            base + 1, base + 5, base + 6, base + 1, base + 6, base + 2,
            // Back face
            base + 0, base + 4, base + 5, base + 0, base + 5, base + 1,
            // Front face
            base + 3, base + 2, base + 6, base + 3, base + 6, base + 7,
        ];
        indices.extend_from_slice(&box_indices);
    };

    let tower_height = TOWER_HEIGHT;
    let base_width = TOWER_BASE_WIDTH;

    // === CENTRAL SPINE DIMENSIONS (DEFINED EARLY) ===
    let spine_width = base_width * 0.35;  // Wider dimension (increased from 0.25)
    let spine_depth = base_width * 0.25;  // Narrower dimension (increased from 0.15)
    let spine_start_y = 1.0;

    // === FOUNDATION SYSTEM (PROPERLY CONNECTED) ===
    // Underground foundation for proper grounding
    add_box(
        Vec3::new(0.0, -0.8, 0.0),
        Vec3::new(spine_width * 1.8, 1.6, spine_depth * 1.8)
    );

    // Ground-level foundation platform - directly connected to spine
    add_box(
        Vec3::new(0.0, 0.4, 0.0),
        Vec3::new(spine_width * 1.4, 0.8, spine_depth * 1.4)
    );

    // Direct connection to spine base - no gap
    add_box(
        Vec3::new(0.0, spine_start_y - 0.1, 0.0),
        Vec3::new(spine_width * 1.1, 0.2, spine_depth * 1.1)
    );

    // === CENTRAL SPINE (RECTANGULAR CORE) ===
    // This is the main structural element - tall, slender, rectangular but slightly wider as requested
    let spine_height = tower_height - spine_start_y - 8.0; // Leave room for pointed top

    // Main central spine - rectangular cross-section
    add_box(
        Vec3::new(0.0, spine_start_y + spine_height / 2.0, 0.0),
        Vec3::new(spine_width, spine_height, spine_depth)
    );

    // === INTEGRATED ARCHITECTURAL MODULES ===
    // Create modules that are much closer to the spine, like in the reference images
    let module_levels = 20;
    let module_spacing = spine_height / module_levels as f32;

    for level in 0..module_levels {
        let level_y = spine_start_y + (level as f32 + 0.5) * module_spacing;
        let level_factor = 1.0 - (level as f32 / module_levels as f32) * 0.2; // Very slight taper

        // Vary the module pattern - sometimes none, sometimes 1-3 modules
        let module_pattern = level % 7;
        let module_count = match module_pattern {
            0 | 1 => 0, // Some levels have no modules for variation
            2 | 5 => 1, // Single module
            3 | 4 => 2, // Two modules opposite each other
            _ => 3,     // Three modules
        };

        for module in 0..module_count {
            let angle = (module as f32 / module_count as f32) * std::f32::consts::TAU + (level as f32 * 0.3);

            // Much closer to spine - attached rather than floating
            let module_distance = spine_width * 0.6; // Was 1.8, now much closer
            let module_x = angle.cos() * module_distance;
            let module_z = angle.sin() * module_distance;

            // Rectangular modules that extend from the spine
            let module_width = 0.8 * level_factor;
            let module_height = 2.0 + (level % 3) as f32 * 0.5; // Varying heights
            let module_depth = 0.6 * level_factor;

            add_box(
                Vec3::new(module_x, level_y, module_z),
                Vec3::new(module_width, module_height, module_depth)
            );

            // Additional stacked modules for some levels (like reference image)
            if level % 5 == 0 {
                add_box(
                    Vec3::new(module_x * 1.2, level_y + module_height * 0.3, module_z * 1.2),
                    Vec3::new(module_width * 0.7, module_height * 0.6, module_depth * 0.7)
                );
            }
        }

        // Spine structural details at regular intervals
        if level % 4 == 0 {
            // Horizontal structural elements around the spine
            for segment in 0..4 {
                let seg_angle = (segment as f32 / 4.0) * std::f32::consts::TAU;
                let seg_x = seg_angle.cos() * spine_width * 0.52;
                let seg_z = seg_angle.sin() * spine_depth * 0.52;

                add_box(
                    Vec3::new(seg_x, level_y, seg_z),
                    Vec3::new(0.12, 0.4, 0.12)
                );
            }
        }
    }

    // === UPPER BUILDING SECTION (FLAT TOP) ===
    // Continue the spine upward like a normal building
    let upper_start_y = spine_start_y + spine_height;
    let upper_height = 10.0;

    // Main upper spine section - same width as main spine
    add_box(
        Vec3::new(0.0, upper_start_y + upper_height / 2.0, 0.0),
        Vec3::new(spine_width, upper_height, spine_depth)
    );

    // === REFINED ARCHITECTURAL DETAILS ===
    // Thin corner reinforcements at the top
    for corner in 0..4 {
        let angle = (corner as f32 / 4.0) * std::f32::consts::TAU + std::f32::consts::FRAC_PI_4;
        let corner_x = angle.cos() * spine_width * 0.45;
        let corner_z = angle.sin() * spine_depth * 0.45;

        // Thinner corner elements
        add_box(
            Vec3::new(corner_x, upper_start_y + upper_height - 1.0, corner_z),
            Vec3::new(0.15, 2.0, 0.15)
        );
    }

    // Thin equipment housings on the sides
    for side in 0..2 {
        let angle = side as f32 * std::f32::consts::PI; // Front and back
        let side_x = angle.cos() * spine_width * 0.52;
        let side_z = angle.sin() * spine_depth * 0.52;

        // Thinner equipment box
        add_box(
            Vec3::new(side_x, upper_start_y + upper_height - 2.0, side_z),
            Vec3::new(0.4, 1.5, 0.2)
        );
    }

    // Vertical accent lines on facades
    for facade in 0..2 {
        let angle = facade as f32 * std::f32::consts::PI;
        let facade_x = angle.cos() * spine_width * 0.51;
        let facade_z = angle.sin() * spine_depth * 0.51;

        // Thin vertical accent
        add_box(
            Vec3::new(facade_x, upper_start_y + upper_height / 2.0, facade_z),
            Vec3::new(0.08, upper_height * 0.8, 0.08)
        );
    }

    // Horizontal bands for architectural interest
    for band in 0..3 {
        let band_y = upper_start_y + (band + 1) as f32 * (upper_height / 4.0);

        // Thin horizontal accent band
        add_box(
            Vec3::new(0.0, band_y, spine_depth * 0.52),
            Vec3::new(spine_width * 0.8, 0.1, 0.1)
        );
    }

    // === ROOFTOP ANTENNA CLUSTER ===
    let roof_y = upper_start_y + upper_height;

    // Antenna array clustered on the northeast corner/edge
    let antenna_base_x = spine_width * 0.25;
    let antenna_base_z = spine_depth * 0.3;

    // Main tall antenna (tallest in the group)
    add_box(
        Vec3::new(antenna_base_x, roof_y + 6.0, antenna_base_z),
        Vec3::new(0.08, 12.0, 0.08)
    );

    // Secondary tall antenna
    add_box(
        Vec3::new(antenna_base_x + 0.3, roof_y + 4.5, antenna_base_z - 0.2),
        Vec3::new(0.06, 9.0, 0.06)
    );

    // Medium height antennas
    add_box(
        Vec3::new(antenna_base_x - 0.2, roof_y + 3.0, antenna_base_z + 0.1),
        Vec3::new(0.05, 6.0, 0.05)
    );

    add_box(
        Vec3::new(antenna_base_x + 0.1, roof_y + 3.5, antenna_base_z + 0.3),
        Vec3::new(0.05, 7.0, 0.05)
    );

    // Shorter antennas for variety
    add_box(
        Vec3::new(antenna_base_x - 0.1, roof_y + 2.0, antenna_base_z - 0.1),
        Vec3::new(0.04, 4.0, 0.04)
    );

    add_box(
        Vec3::new(antenna_base_x + 0.4, roof_y + 2.5, antenna_base_z + 0.1),
        Vec3::new(0.04, 5.0, 0.04)
    );

    // Tiny support antennas
    add_box(
        Vec3::new(antenna_base_x + 0.2, roof_y + 1.25, antenna_base_z - 0.3),
        Vec3::new(0.03, 2.5, 0.03)
    );

    // Antenna support platform (small)
    add_box(
        Vec3::new(antenna_base_x, roof_y + 0.15, antenna_base_z),
        Vec3::new(0.8, 0.3, 0.6)
    );

    // Rooftop equipment/details
    add_box(
        Vec3::new(spine_width * 0.2, roof_y + 0.3, 0.0),
        Vec3::new(0.4, 0.6, 0.3)
    );
    add_box(
        Vec3::new(-spine_width * 0.2, roof_y + 0.4, spine_depth * 0.15),
        Vec3::new(0.3, 0.8, 0.2)
    );

    // === STRUCTURAL SUPPORT ELEMENTS ===
    // Add some connecting elements between major module levels for structural integrity
    for level in (3..module_levels).step_by(6) {
        let level_y = spine_start_y + (level as f32) * module_spacing;

        // Cross-bracing elements
        for brace in 0..4 {
            let angle = (brace as f32 / 4.0) * std::f32::consts::TAU + std::f32::consts::FRAC_PI_4;
            let brace_distance = spine_width * 1.4;
            let brace_x = angle.cos() * brace_distance;
            let brace_z = angle.sin() * brace_distance;

            add_box(
                Vec3::new(brace_x, level_y, brace_z),
                Vec3::new(0.12, 2.0, 0.12)
            );
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    meshes.add(mesh)
}

// ============================================================================
// HEAVY TURRET MESHES
// ============================================================================

/// Create procedural mesh for the heavy turret base (Bunker style)
pub fn create_turret_base_mesh(meshes: &mut ResMut<Assets<Mesh>>) -> Handle<Mesh> {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Helper: Add Frustum (Tapered Cylinder) with flat caps and correct winding
    fn add_frustum(
        vertices: &mut Vec<[f32; 3]>,
        normals: &mut Vec<[f32; 3]>,
        indices: &mut Vec<u32>,
        center: Vec3,
        bottom_radius: f32,
        top_radius: f32,
        height: f32,
        segments: u32,
    ) {
        let half_height = height / 2.0;

        // Calculate slope normal for side faces
        let dr = bottom_radius - top_radius;
        let side_len = (dr * dr + height * height).sqrt();
        let ny = dr / side_len;
        let nr = height / side_len; // radial component

        // 1. Side Faces
        let side_base = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let normal = [cos_a * nr, ny, sin_a * nr];

            // Bottom ring vertex
            vertices.push([
                center.x + cos_a * bottom_radius,
                center.y - half_height,
                center.z + sin_a * bottom_radius,
            ]);
            normals.push(normal);

            // Top ring vertex
            vertices.push([
                center.x + cos_a * top_radius,
                center.y + half_height,
                center.z + sin_a * top_radius,
            ]);
            normals.push(normal);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            let b = side_base + i * 2;
            let t = side_base + i * 2 + 1;
            let bn = side_base + next * 2;
            let tn = side_base + next * 2 + 1;

            // CCW Winding for Sides
            indices.push(b); indices.push(t); indices.push(tn);
            indices.push(b); indices.push(tn); indices.push(bn);
        }

        // 2. Bottom Cap
        let bot_center_idx = vertices.len() as u32;
        vertices.push([center.x, center.y - half_height, center.z]);
        normals.push([0.0, -1.0, 0.0]);

        let bot_ring_start = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let x = angle.cos() * bottom_radius;
            let z = angle.sin() * bottom_radius;
            vertices.push([center.x + x, center.y - half_height, center.z + z]);
            normals.push([0.0, -1.0, 0.0]);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            indices.push(bot_center_idx);
            indices.push(bot_ring_start + next);
            indices.push(bot_ring_start + i);
        }

        // 3. Top Cap
        let top_center_idx = vertices.len() as u32;
        vertices.push([center.x, center.y + half_height, center.z]);
        normals.push([0.0, 1.0, 0.0]);

        let top_ring_start = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let x = angle.cos() * top_radius;
            let z = angle.sin() * top_radius;
            vertices.push([center.x + x, center.y + half_height, center.z + z]);
            normals.push([0.0, 1.0, 0.0]);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            indices.push(top_center_idx);
            // FIXED: Reversed winding for Top Cap (next, i) to be CCW/Visible
            indices.push(top_ring_start + next);
            indices.push(top_ring_start + i);
        }
    }

    // 1. Concrete Foundation Slab (Octagonal)
    add_frustum(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 0.25, 0.0),
        6.0, 6.0, 0.5, 8);

    // 2. Main Sloped Bunker Body (Frustum)
    add_frustum(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 1.5, 0.0),
        5.0, 3.5, 2.0, 8);

    // 3. Turret Ring (Top Detail)
    add_frustum(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 2.6, 0.0),
        3.8, 3.8, 0.2, 16);

    // Add UVs
    let uvs: Vec<[f32; 2]> = (0..vertices.len()).map(|_| [0.5, 0.5]).collect();

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    meshes.add(mesh)
}

/// Create procedural mesh for the rotating turret assembly (Armored head)
pub fn create_turret_rotating_assembly_mesh(meshes: &mut ResMut<Assets<Mesh>>) -> Handle<Mesh> {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Helper: Add Box with sharp edges
    fn add_box(
        vertices: &mut Vec<[f32; 3]>,
        normals: &mut Vec<[f32; 3]>,
        indices: &mut Vec<u32>,
        center: Vec3,
        size: Vec3,
    ) {
        let hw = size.x / 2.0;
        let hh = size.y / 2.0;
        let hd = size.z / 2.0;

        let raw_verts = [
            [center.x - hw, center.y - hh, center.z - hd],
            [center.x + hw, center.y - hh, center.z - hd],
            [center.x + hw, center.y - hh, center.z + hd],
            [center.x - hw, center.y - hh, center.z + hd],
            [center.x - hw, center.y + hh, center.z - hd],
            [center.x + hw, center.y + hh, center.z - hd],
            [center.x + hw, center.y + hh, center.z + hd],
            [center.x - hw, center.y + hh, center.z + hd],
        ];

        // Corrected winding for all faces (CCW)
        let faces = [
            ([0.0, -1.0, 0.0], [0, 1, 2, 3]), // Bottom
            ([0.0, 1.0, 0.0], [4, 7, 6, 5]),  // Top - FLIPPED
            ([-1.0, 0.0, 0.0], [0, 3, 7, 4]), // Left
            ([1.0, 0.0, 0.0], [1, 5, 6, 2]),  // Right
            ([0.0, 0.0, -1.0], [0, 4, 5, 1]), // Back
            ([0.0, 0.0, 1.0], [3, 2, 6, 7]),  // Front
        ];

        for (normal, vert_indices) in faces {
            let face_base = vertices.len() as u32;
            for &idx in &vert_indices {
                vertices.push(raw_verts[idx]);
                normals.push(normal);
            }
            indices.push(face_base); indices.push(face_base + 1); indices.push(face_base + 2);
            indices.push(face_base); indices.push(face_base + 2); indices.push(face_base + 3);
        }
    }

    // Helper: Add Cylinder (Y-aligned)
    fn add_cylinder(
        vertices: &mut Vec<[f32; 3]>,
        normals: &mut Vec<[f32; 3]>,
        indices: &mut Vec<u32>,
        center: Vec3,
        radius: f32,
        height: f32,
        segments: u32,
    ) {
        let base = vertices.len() as u32;
        let half_height = height / 2.0;

        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let n = [angle.cos(), 0.0, angle.sin()];

            vertices.push([center.x + n[0] * radius, center.y - half_height, center.z + n[2] * radius]);
            normals.push(n);
            vertices.push([center.x + n[0] * radius, center.y + half_height, center.z + n[2] * radius]);
            normals.push(n);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            let b = base + i * 2;
            let t = base + i * 2 + 1;
            let bn = base + next * 2;
            let tn = base + next * 2 + 1;

            // CCW Winding for Sides
            indices.push(b); indices.push(t); indices.push(tn);
            indices.push(b); indices.push(tn); indices.push(bn);
        }

        // 2. Bottom Cap
        let bot_center_idx = vertices.len() as u32;
        vertices.push([center.x, center.y - half_height, center.z]);
        normals.push([0.0, -1.0, 0.0]);

        let bot_ring_start = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let x = angle.cos() * radius;
            let z = angle.sin() * radius;
            vertices.push([center.x + x, center.y - half_height, center.z + z]);
            normals.push([0.0, -1.0, 0.0]);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            indices.push(bot_center_idx);
            indices.push(bot_ring_start + next);
            indices.push(bot_ring_start + i);
        }

        // 3. Top Cap
        let top_center_idx = vertices.len() as u32;
        vertices.push([center.x, center.y + half_height, center.z]);
        normals.push([0.0, 1.0, 0.0]);

        let top_ring_start = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let x = angle.cos() * radius;
            let z = angle.sin() * radius;
            vertices.push([center.x + x, center.y + half_height, center.z + z]);
            normals.push([0.0, 1.0, 0.0]);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            indices.push(top_center_idx);
            // Reversed winding for Top Cap visibility (next, i)
            indices.push(top_ring_start + next);
            indices.push(top_ring_start + i);
        }
    }

    // Helper: Add Cylinder (Z-aligned for barrels)
    fn add_z_cylinder(
        vertices: &mut Vec<[f32; 3]>,
        normals: &mut Vec<[f32; 3]>,
        indices: &mut Vec<u32>,
        center: Vec3,
        radius: f32,
        length: f32,
        segments: u32,
    ) {
        let base = vertices.len() as u32;
        let half_len = length / 2.0;

        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let n = [angle.cos(), angle.sin(), 0.0];

            vertices.push([center.x + n[0] * radius, center.y + n[1] * radius, center.z - half_len]);
            normals.push(n);
            vertices.push([center.x + n[0] * radius, center.y + n[1] * radius, center.z + half_len]);
            normals.push(n);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            let b = base + i * 2;
            let t = base + i * 2 + 1;
            let bn = base + next * 2;
            let tn = base + next * 2 + 1;

            // Reverted winding order (b, tn, t) for Outward faces
            indices.push(b); indices.push(tn); indices.push(t);
            indices.push(b); indices.push(bn); indices.push(tn);
        }

        // 2. Back Cap
        let back_center_idx = vertices.len() as u32;
        vertices.push([center.x, center.y, center.z - half_len]);
        normals.push([0.0, 0.0, -1.0]);

        let back_ring_start = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let x = angle.cos() * radius;
            let y = angle.sin() * radius;
            vertices.push([center.x + x, center.y + y, center.z - half_len]);
            normals.push([0.0, 0.0, -1.0]);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            indices.push(back_center_idx);
            indices.push(back_ring_start + next);
            indices.push(back_ring_start + i);
        }

        // 3. Front Cap
        let front_center_idx = vertices.len() as u32;
        vertices.push([center.x, center.y, center.z + half_len]);
        normals.push([0.0, 0.0, 1.0]);

        let front_ring_start = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let x = angle.cos() * radius;
            let y = angle.sin() * radius;
            vertices.push([center.x + x, center.y + y, center.z + half_len]);
            normals.push([0.0, 0.0, 1.0]);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            indices.push(front_center_idx);
            indices.push(front_ring_start + i);
            indices.push(front_ring_start + next);
        }
    }

    // 1. Swivel Mount
    add_cylinder(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 0.25, 0.0), 2.5, 0.5, 16);

    // 2. Main Gun Housing (Flipped Z)
    add_box(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 1.5, 0.5), // Was -0.5
        Vec3::new(2.5, 2.0, 4.0));

    // 3. Side Armor Pods
    add_box(&mut vertices, &mut normals, &mut indices,
        Vec3::new(1.8, 1.5, 0.0),
        Vec3::new(1.2, 1.6, 3.5));
    add_box(&mut vertices, &mut normals, &mut indices,
        Vec3::new(-1.8, 1.5, 0.0),
        Vec3::new(1.2, 1.6, 3.5));

    // 4. Barrels (Dual Heavy) - Extending Forward (-Z)
    // Left
    add_z_cylinder(&mut vertices, &mut normals, &mut indices,
        Vec3::new(-1.8, 1.5, -3.5), 0.35, 5.0, 12); // Was 3.5
    // Right
    add_z_cylinder(&mut vertices, &mut normals, &mut indices,
        Vec3::new(1.8, 1.5, -3.5), 0.35, 5.0, 12);

    // 5. Muzzle Brakes
    add_z_cylinder(&mut vertices, &mut normals, &mut indices,
        Vec3::new(-1.8, 1.5, -6.0), 0.5, 0.8, 12); // Was 6.0
    add_z_cylinder(&mut vertices, &mut normals, &mut indices,
        Vec3::new(1.8, 1.5, -6.0), 0.5, 0.8, 12);

    // 6. Sensor Pod
    add_box(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.8, 2.8, -0.5), // Was 0.5
        Vec3::new(0.8, 0.6, 1.2));

    // Add UVs
    let uvs: Vec<[f32; 2]> = (0..vertices.len()).map(|_| [0.5, 0.5]).collect();

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    meshes.add(mesh)
}

// ============================================================================
// MG TURRET MESHES
// ============================================================================

/// Create procedural mesh for the MG turret base (Hexagonal platform)
pub fn create_mg_turret_base_mesh(meshes: &mut ResMut<Assets<Mesh>>) -> Handle<Mesh> {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Helper: Add Cylinder (Y-aligned) with flat shading for caps
    fn add_cylinder(
        vertices: &mut Vec<[f32; 3]>,
        normals: &mut Vec<[f32; 3]>,
        indices: &mut Vec<u32>,
        center: Vec3,
        radius: f32,
        height: f32,
        segments: u32,
    ) {
        let half_height = height / 2.0;

        // 1. Side Faces
        let side_base = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let n = [angle.cos(), 0.0, angle.sin()];

            // Bottom ring vertex
            vertices.push([center.x + n[0] * radius, center.y - half_height, center.z + n[2] * radius]);
            normals.push(n);

            // Top ring vertex
            vertices.push([center.x + n[0] * radius, center.y + half_height, center.z + n[2] * radius]);
            normals.push(n);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            let b = side_base + i * 2;
            let t = side_base + i * 2 + 1;
            let bn = side_base + next * 2;
            let tn = side_base + next * 2 + 1;

            // Fixed winding order (CCW) for Y-Axis Cylinder
            indices.push(b); indices.push(t); indices.push(tn);
            indices.push(b); indices.push(tn); indices.push(bn);
        }

        // 2. Bottom Cap
        let bot_center_idx = vertices.len() as u32;
        vertices.push([center.x, center.y - half_height, center.z]);
        normals.push([0.0, -1.0, 0.0]);

        let bot_ring_start = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let x = angle.cos() * radius;
            let z = angle.sin() * radius;
            vertices.push([center.x + x, center.y - half_height, center.z + z]);
            normals.push([0.0, -1.0, 0.0]);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            indices.push(bot_center_idx);
            indices.push(bot_ring_start + next);
            indices.push(bot_ring_start + i);
        }

        // 3. Top Cap
        let top_center_idx = vertices.len() as u32;
        vertices.push([center.x, center.y + half_height, center.z]);
        normals.push([0.0, 1.0, 0.0]);

        let top_ring_start = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let x = angle.cos() * radius;
            let z = angle.sin() * radius;
            vertices.push([center.x + x, center.y + half_height, center.z + z]);
            normals.push([0.0, 1.0, 0.0]);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            indices.push(top_center_idx);
            // FLIPPED winding for Top Cap to be visible (+Y)
            indices.push(top_ring_start + next);
            indices.push(top_ring_start + i);
        }
    }

    // 1. Hexagonal Base Platform (Reduced size)
    add_cylinder(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 0.5, 0.0),
        2.8, 1.0, 6); // Radius reduced to 2.8

    // 2. Upper Mount Point (Smaller Hexagon)
    add_cylinder(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 1.2, 0.0),
        2.0, 0.4, 6);

    // Add UVs
    let uvs: Vec<[f32; 2]> = (0..vertices.len()).map(|_| [0.5, 0.5]).collect();

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    meshes.add(mesh)
}

/// Create procedural mesh for the MG turret assembly (Articulated arm with detailed barrel)
pub fn create_mg_turret_assembly_mesh(meshes: &mut ResMut<Assets<Mesh>>) -> Handle<Mesh> {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Helper: Add Box
    fn add_box(
        vertices: &mut Vec<[f32; 3]>,
        normals: &mut Vec<[f32; 3]>,
        indices: &mut Vec<u32>,
        center: Vec3,
        size: Vec3,
    ) {
        let hw = size.x / 2.0;
        let hh = size.y / 2.0;
        let hd = size.z / 2.0;

        let raw_verts = [
            [center.x - hw, center.y - hh, center.z - hd],
            [center.x + hw, center.y - hh, center.z - hd],
            [center.x + hw, center.y - hh, center.z + hd],
            [center.x - hw, center.y - hh, center.z + hd],
            [center.x - hw, center.y + hh, center.z - hd],
            [center.x + hw, center.y + hh, center.z - hd],
            [center.x + hw, center.y + hh, center.z + hd],
            [center.x - hw, center.y + hh, center.z + hd],
        ];

        let faces = [
            ([0.0, -1.0, 0.0], [0, 1, 2, 3]), // Bottom
            ([0.0, 1.0, 0.0], [4, 7, 6, 5]),  // Top - FLIPPED INDICES for CCW
            ([-1.0, 0.0, 0.0], [0, 3, 7, 4]), // Left
            ([1.0, 0.0, 0.0], [1, 5, 6, 2]),  // Right
            ([0.0, 0.0, -1.0], [0, 4, 5, 1]), // Back
            ([0.0, 0.0, 1.0], [3, 2, 6, 7]),  // Front
        ];

        for (normal, vert_indices) in faces {
            let face_base = vertices.len() as u32;
            for &idx in &vert_indices {
                vertices.push(raw_verts[idx]);
                normals.push(normal);
            }
            indices.push(face_base); indices.push(face_base + 1); indices.push(face_base + 2);
            indices.push(face_base); indices.push(face_base + 2); indices.push(face_base + 3);
        }
    }

    // Helper: Add Cylinder (Y-aligned)
    fn add_cylinder(
        vertices: &mut Vec<[f32; 3]>,
        normals: &mut Vec<[f32; 3]>,
        indices: &mut Vec<u32>,
        center: Vec3,
        radius: f32,
        height: f32,
        segments: u32,
    ) {
        let half_height = height / 2.0;

        // 1. Side Faces
        let side_base = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let n = [angle.cos(), 0.0, angle.sin()];

            vertices.push([center.x + n[0] * radius, center.y - half_height, center.z + n[2] * radius]);
            normals.push(n);

            vertices.push([center.x + n[0] * radius, center.y + half_height, center.z + n[2] * radius]);
            normals.push(n);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            let b = side_base + i * 2;
            let t = side_base + i * 2 + 1;
            let bn = side_base + next * 2;
            let tn = side_base + next * 2 + 1;

            // Fixed winding order (CCW)
            indices.push(b); indices.push(t); indices.push(tn);
            indices.push(b); indices.push(tn); indices.push(bn);
        }

        // 2. Bottom Cap
        let bot_center_idx = vertices.len() as u32;
        vertices.push([center.x, center.y - half_height, center.z]);
        normals.push([0.0, -1.0, 0.0]);

        let bot_ring_start = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let x = angle.cos() * radius;
            let z = angle.sin() * radius;
            vertices.push([center.x + x, center.y - half_height, center.z + z]);
            normals.push([0.0, -1.0, 0.0]);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            indices.push(bot_center_idx);
            indices.push(bot_ring_start + next);
            indices.push(bot_ring_start + i);
        }

        // 3. Top Cap
        let top_center_idx = vertices.len() as u32;
        vertices.push([center.x, center.y + half_height, center.z]);
        normals.push([0.0, 1.0, 0.0]);

        let top_ring_start = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let x = angle.cos() * radius;
            let z = angle.sin() * radius;
            vertices.push([center.x + x, center.y + half_height, center.z + z]);
            normals.push([0.0, 1.0, 0.0]);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            indices.push(top_center_idx);
            // FLIPPED winding for Top Cap
            indices.push(top_ring_start + next);
            indices.push(top_ring_start + i);
        }
    }

    // Helper: Add Cylinder (Z-aligned for barrels)
    fn add_z_cylinder(
        vertices: &mut Vec<[f32; 3]>,
        normals: &mut Vec<[f32; 3]>,
        indices: &mut Vec<u32>,
        center: Vec3,
        radius: f32,
        length: f32,
        segments: u32,
    ) {
        let half_len = length / 2.0;

        // 1. Side Faces
        let side_base = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let n = [angle.cos(), angle.sin(), 0.0];

            // Back ring
            vertices.push([center.x + n[0] * radius, center.y + n[1] * radius, center.z - half_len]);
            normals.push(n);

            // Front ring
            vertices.push([center.x + n[0] * radius, center.y + n[1] * radius, center.z + half_len]);
            normals.push(n);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            let b = side_base + i * 2;
            let t = side_base + i * 2 + 1;
            let bn = side_base + next * 2;
            let tn = side_base + next * 2 + 1;

            // REVERTED winding order to (b, tn, t) for Z-Axis Cylinder Outward faces
            indices.push(b); indices.push(tn); indices.push(t);
            indices.push(b); indices.push(bn); indices.push(tn);
        }

        // 2. Back Cap
        let back_center_idx = vertices.len() as u32;
        vertices.push([center.x, center.y, center.z - half_len]);
        normals.push([0.0, 0.0, -1.0]);

        let back_ring_start = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let x = angle.cos() * radius;
            let y = angle.sin() * radius;
            vertices.push([center.x + x, center.y + y, center.z - half_len]);
            normals.push([0.0, 0.0, -1.0]);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            indices.push(back_center_idx);
            indices.push(back_ring_start + next);
            indices.push(back_ring_start + i);
        }

        // 3. Front Cap
        let front_center_idx = vertices.len() as u32;
        vertices.push([center.x, center.y, center.z + half_len]);
        normals.push([0.0, 0.0, 1.0]);

        let front_ring_start = vertices.len() as u32;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * 2.0 * PI;
            let x = angle.cos() * radius;
            let y = angle.sin() * radius;
            vertices.push([center.x + x, center.y + y, center.z + half_len]);
            normals.push([0.0, 0.0, 1.0]);
        }

        for i in 0..segments {
            let next = (i + 1) % segments;
            indices.push(front_center_idx);
            indices.push(front_ring_start + i);
            indices.push(front_ring_start + next);
        }
    }

    // 1. Shoulder Swivel (Improved Connector)
    // Turret ring (wide base)
    add_cylinder(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 0.2, 0.0), 1.2, 0.4, 16);

    // Hinge block (Box shape for pivot)
    add_box(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 0.6, 0.0),
        Vec3::new(1.0, 0.8, 1.0));

    // 2. Arm Segment (Angled connection)
    add_box(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 1.4, 0.5),
        Vec3::new(0.6, 1.5, 0.6));

    // 3. Weapon Housing (Main body)
    add_box(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(0.8, 0.8, 2.0));

    // 4. Barrel (Oerlikon 20mm Style - Elongated)
    // Stage 1: Base Connector/Shroud
    add_z_cylinder(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 2.0, -1.2),
        0.25, 0.4, 12);

    // Stage 2: Recoil Spring/Mechanism (Thicker section)
    add_z_cylinder(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 2.0, -2.2),
        0.18, 1.6, 12);

    // Stage 3: Main Long Barrel (Thin)
    add_z_cylinder(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 2.0, -5.0),
        0.10, 4.0, 12);

    // Stage 4: Muzzle Brake/Flash Hider
    add_z_cylinder(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.0, 2.0, -7.2),
        0.14, 0.4, 12);

    // 5. Cooling Fins (Thin boxes along housing sides)
    // Moved slightly outward to avoid z-fighting with housing
    let fin_count = 6;
    for i in 0..fin_count {
        let z_pos = 0.5 - (i as f32 * 0.25);
        // Left fins
        add_box(&mut vertices, &mut normals, &mut indices,
            Vec3::new(-0.52, 2.0, z_pos),
            Vec3::new(0.2, 0.6, 0.1));
        // Right fins
        add_box(&mut vertices, &mut normals, &mut indices,
            Vec3::new(0.52, 2.0, z_pos),
            Vec3::new(0.2, 0.6, 0.1));
    }

    // 7. Ammo Magazines (Side boxes)
    // Left Magazine
    add_box(&mut vertices, &mut normals, &mut indices,
        Vec3::new(-0.95, 2.1, 0.0),
        Vec3::new(0.5, 1.0, 1.4));
    // Left Strut
    add_box(&mut vertices, &mut normals, &mut indices,
        Vec3::new(-0.6, 2.1, 0.0),
        Vec3::new(0.3, 0.4, 0.8));

    // Right Magazine
    add_box(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.95, 2.1, 0.0),
        Vec3::new(0.5, 1.0, 1.4));
    // Right Strut
    add_box(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.6, 2.1, 0.0),
        Vec3::new(0.3, 0.4, 0.8));

    // 6. Hydraulic Details (Optional small cylinders)
    add_cylinder(&mut vertices, &mut normals, &mut indices,
        Vec3::new(0.6, 1.0, 0.0), 0.1, 0.8, 8); // Right piston
    add_cylinder(&mut vertices, &mut normals, &mut indices,
        Vec3::new(-0.6, 1.0, 0.0), 0.1, 0.8, 8); // Left piston


    // Add UVs
    let uvs: Vec<[f32; 2]> = (0..vertices.len()).map(|_| [0.5, 0.5]).collect();

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    meshes.add(mesh)
}

use bevy::prelude::*;

/// Ray-sphere intersection test
/// Returns Some((distance, hit_point)) if ray intersects sphere, None otherwise
pub fn ray_sphere_intersection(
    ray_origin: Vec3,
    ray_direction: Vec3,
    sphere_center: Vec3,
    sphere_radius: f32,
) -> Option<(f32, Vec3)> {
    let oc = ray_origin - sphere_center;
    let a = ray_direction.dot(ray_direction);
    let b = 2.0 * oc.dot(ray_direction);
    let c = oc.dot(oc) - sphere_radius * sphere_radius;
    let discriminant = b * b - 4.0 * a * c;

    if discriminant < 0.0 {
        return None;
    }

    // Find nearest intersection point (entry point into sphere)
    let t = (-b - discriminant.sqrt()) / (2.0 * a);
    if t > 0.0 {
        let hit_point = ray_origin + ray_direction * t;
        return Some((t, hit_point));
    }

    // Check far intersection (exit point, in case we're inside the sphere)
    let t2 = (-b + discriminant.sqrt()) / (2.0 * a);
    if t2 > 0.0 {
        let hit_point = ray_origin + ray_direction * t2;
        return Some((t2, hit_point));
    }

    None
}

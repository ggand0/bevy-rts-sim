#import bevy_pbr::{
    mesh_view_bindings::view,
    mesh_bindings::mesh,
    forward_io::VertexOutput,
}

const MAX_RIPPLES: u32 = 8u;
const RIPPLE_DURATION: f32 = 1.5;

struct ShieldMaterial {
    color: vec4<f32>,
    fresnel_power: f32,
    hex_scale: f32,
    time: f32,
    health_percent: f32,
    shield_center: vec3<f32>,
    shield_radius: f32,
    ripple_data: array<vec4<f32>, MAX_RIPPLES>, // xyz = position, w = age
}

@group(2) @binding(0)
var<uniform> material: ShieldMaterial;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate view direction
    let world_pos = in.world_position.xyz;
    let view_pos = view.world_position.xyz;
    let view_dir = normalize(view_pos - world_pos);

    // Fresnel effect (edge glow)
    let normal = normalize(in.world_normal);
    let fresnel = pow(1.0 - abs(dot(normal, view_dir)), material.fresnel_power);

    // Hexagonal pattern
    let hex_uv = in.uv * material.hex_scale;
    let hex_pattern = hexagonal_grid(hex_uv);

    // Energy pulse animation
    let pulse = sin(material.time * 2.0 + world_pos.y * 0.5) * 0.5 + 0.5;

    // Calculate ripple effects
    var ripple_intensity = 0.0;
    for (var i = 0u; i < MAX_RIPPLES; i = i + 1u) {
        let ripple_info = material.ripple_data[i];
        let impact_pos = ripple_info.xyz;
        let ripple_age = ripple_info.w;

        // Skip inactive ripples
        if ripple_age < 0.0 || ripple_age > RIPPLE_DURATION {
            continue;
        }

        // Distance from world position to impact point
        let dist_to_impact = distance(world_pos, impact_pos);

        // Ripple expands outward from impact
        let ripple_radius = ripple_age * 30.0; // Expansion speed
        let ripple_width = 4.0;

        // Create expanding ring
        let ring_dist = abs(dist_to_impact - ripple_radius);
        let ring_falloff = 1.0 - smoothstep(0.0, ripple_width, ring_dist);

        // Fade out over time
        let fade = 1.0 - (ripple_age / RIPPLE_DURATION);
        let fade_curve = fade * fade; // Quadratic fadeout

        ripple_intensity += ring_falloff * fade_curve * 0.5;
    }

    // Combine effects
    let hex_intensity = hex_pattern * 0.3;
    let base_alpha = material.color.a * 0.2;
    let edge_alpha = fresnel * 0.6;
    let ripple_alpha = ripple_intensity * 0.4;
    let total_alpha = base_alpha + edge_alpha + hex_intensity * pulse * 0.2 + ripple_alpha;

    // Gradual color shift to white as health decreases
    // damage_ratio: 0.0 at full health, 1.0 at zero health
    let damage_ratio = 1.0 - material.health_percent;
    let white_shift = damage_ratio * 0.7; // Scale the shift intensity
    let white = vec3<f32>(1.0, 1.0, 1.0);
    let damaged_color = material.color.rgb + (white - material.color.rgb) * white_shift;

    // Final color with emissive glow and ripple brightness
    let ripple_glow = ripple_intensity * 2.0;
    let final_color = damaged_color * (1.0 + fresnel * 2.0 + hex_pattern * pulse * 0.5 + ripple_glow);

    return vec4<f32>(final_color, total_alpha);
}

// Hexagonal grid pattern
fn hexagonal_grid(uv: vec2<f32>) -> f32 {
    // Hexagonal tiling math
    let q = vec2<f32>(
        uv.x * 1.15470054,  // sqrt(4/3)
        uv.y + uv.x * 0.57735027  // 1/sqrt(3)
    );

    let pi = vec2<f32>(floor(q.x), floor(q.y));
    let pf = fract(q);

    // Determine which hex cell we're in
    let a = fract((pi.x + pi.y) / 3.0);
    var dx = 0.0;
    var dy = 0.0;

    if a < 0.33333 {
        dx = 0.0;
        dy = 0.0;
    } else if a < 0.66667 {
        dx = 1.0;
        dy = 0.0;
        if pf.x + pf.y > 1.0 {
            dx = 0.0;
            dy = 1.0;
        }
    } else {
        dx = 0.0;
        dy = 1.0;
        if pf.x + pf.y < 1.0 {
            dx = 1.0;
            dy = 0.0;
        }
    }

    // Distance to hex center
    let hex_center = vec2<f32>(pi.x + dx, pi.y + dy);
    let diff = q - hex_center;
    let dist = length(diff);

    // Create hex outline
    let hex_size = 0.4;
    let line_width = 0.05;
    let edge = smoothstep(hex_size - line_width, hex_size, dist) -
               smoothstep(hex_size, hex_size + line_width, dist);

    return edge;
}

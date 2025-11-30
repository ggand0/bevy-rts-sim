#import bevy_pbr::{
    mesh_view_bindings::view,
    mesh_bindings::mesh,
    forward_io::VertexOutput,
}

struct ShieldMaterial {
    color: vec4<f32>,
    fresnel_power: f32,
    hex_scale: f32,
    time: f32,
    _padding: f32,
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

    // Combine effects
    let hex_intensity = hex_pattern * 0.3;
    let base_alpha = material.color.a * 0.2;
    let edge_alpha = fresnel * 0.6;
    let total_alpha = base_alpha + edge_alpha + hex_intensity * pulse * 0.2;

    // Final color with emissive glow
    let final_color = material.color.rgb * (1.0 + fresnel * 2.0 + hex_pattern * pulse * 0.5);

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

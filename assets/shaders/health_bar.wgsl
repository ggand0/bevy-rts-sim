#import bevy_pbr::forward_io::VertexOutput

@group(2) @binding(0)
var<uniform> health_data: vec4<f32>; // x: health_fraction (0.0-1.0), yzw: unused

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // UV.x goes from 0.0 (left) to 1.0 (right)
    let health_fraction = health_data.x;

    // Colors
    let green = vec3<f32>(0.4, 1.0, 0.4);  // Bright green for health
    let gray = vec3<f32>(0.2, 0.2, 0.2);   // Dark gray for missing health

    // Choose color based on UV position vs health fraction
    // Shrink from right side: filled portion is on the left (low UV values)
    // Note: Billboard rotation mirrors the quad, so we use < instead of >
    var color: vec3<f32>;
    if in.uv.x < health_fraction {
        color = green;
    } else {
        color = gray;
    }

    return vec4<f32>(color, 1.0);
}

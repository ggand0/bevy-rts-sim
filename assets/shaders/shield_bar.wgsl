#import bevy_pbr::forward_io::VertexOutput

@group(2) @binding(0)
var<uniform> health_data: vec4<f32>; // x: health_fraction (0.0-1.0), yzw: unused

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // UV.x goes from 0.0 (left) to 1.0 (right)
    let health_fraction = health_data.x;

    // Colors - cyan for shields
    let cyan = vec3<f32>(0.3, 0.8, 1.0);   // Bright cyan for shield
    let gray = vec3<f32>(0.15, 0.2, 0.25); // Dark blue-gray for missing shield

    // Choose color based on UV position vs health fraction
    // Shrink from right side: filled portion is on the right (high UV values)
    var color: vec3<f32>;
    if in.uv.x > (1.0 - health_fraction) {
        color = cyan;
    } else {
        color = gray;
    }

    return vec4<f32>(color, 1.0);
}

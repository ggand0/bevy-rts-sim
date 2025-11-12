#import bevy_pbr::forward_io::VertexOutput

@group(2) @binding(0)
var<uniform> frame_data: vec4<f32>;
@group(2) @binding(1)
var<uniform> color_data: vec4<f32>;
@group(2) @binding(2)
var sprite_texture: texture_2d<f32>;
@group(2) @binding(3)
var sprite_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Extract frame data
    let frame_x = frame_data.x;
    let frame_y = frame_data.y;
    let grid_size = frame_data.z;
    let alpha = frame_data.w;
    
    // Calculate UV coordinates for the specific frame in 5x5 grid
    let frame_size = 1.0 / grid_size;
    let frame_offset = vec2<f32>(frame_x * frame_size, frame_y * frame_size);
    
    // Scale UV coordinates to fit within the frame
    let frame_uv = in.uv * frame_size + frame_offset;
    
    // Sample the flipbook texture directly
    let sprite_sample = textureSample(sprite_texture, sprite_sampler, frame_uv);
    
    // Apply base color tint and emissive strength from material
    let tinted_color = sprite_sample.rgb * color_data.rgb;
    let emissive_strength = color_data.a;
    
    // Add emissive glow for bright explosion effects
    let enhanced_rgb = tinted_color + (tinted_color * emissive_strength);
    
    // Use the original alpha from the flipbook texture, modulated by our fade alpha
    let final_alpha = sprite_sample.a * alpha;
    
    return vec4<f32>(enhanced_rgb, final_alpha);
} 
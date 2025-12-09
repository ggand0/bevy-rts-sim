#import bevy_pbr::forward_io::VertexOutput

@group(2) @binding(0)
var<uniform> frame_data: vec4<f32>;  // x: frame_col, y: frame_row, z: columns, w: rows
@group(2) @binding(1)
var<uniform> color_data: vec4<f32>;  // RGB: tint color, A: alpha (overall fade)
@group(2) @binding(2)
var sprite_texture: texture_2d<f32>;
@group(2) @binding(3)
var sprite_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Extract frame data - supports non-square grids
    let frame_col = frame_data.x;
    let frame_row = frame_data.y;
    let columns = frame_data.z;
    let rows = frame_data.w;

    // Calculate frame size (different for X and Y if non-square grid)
    let frame_size_x = 1.0 / columns;
    let frame_size_y = 1.0 / rows;

    // Simple UV calculation matching explosion.wgsl approach
    // Scale UV to frame size, then offset to correct frame position
    let frame_offset = vec2<f32>(frame_col * frame_size_x, frame_row * frame_size_y);
    let frame_uv = in.uv * vec2<f32>(frame_size_x, frame_size_y) + frame_offset;

    // Sample the flipbook texture
    let sprite_sample = textureSample(sprite_texture, sprite_sampler, frame_uv);

    // These textures store smoke/explosion in alpha channel
    // RGB contains the color, A contains the opacity mask
    // Tint the texture color with our color_data
    let tinted_color = sprite_sample.rgb * color_data.rgb;

    // Use texture alpha multiplied by our fade alpha
    let final_alpha = sprite_sample.a * color_data.a;

    return vec4<f32>(tinted_color, final_alpha);
}

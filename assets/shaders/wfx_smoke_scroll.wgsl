// War FX Scrolling Smoke Shader
// Converted from Unity shader: WFX_S Smoke Scroll
// Creates rising smoke effect via UV scrolling

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

struct SmokeScrollMaterial {
    tint_color_and_speed: vec4<f32>,  // RGB = tint, A = scroll_speed
}

@group(2) @binding(0)
var<uniform> material: SmokeScrollMaterial;
@group(2) @binding(1)
var smoke_texture: texture_2d<f32>;
@group(2) @binding(2)
var smoke_sampler: sampler;

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    var uv = in.uv;

    // Extract tint color and scroll speed from uniform
    let tint_color = material.tint_color_and_speed.rgb;
    let scroll_speed = material.tint_color_and_speed.a;

    // Apply UV scrolling (UPWARD - add to y instead of subtract)
    var scrolled_uv = uv;
    scrolled_uv.y += (globals.time * scroll_speed) % 1.0;

    // Sample scrolled texture
    var tex = textureSample(smoke_texture, smoke_sampler, scrolled_uv);

    // Use the texture's alpha directly for transparency
    // Apply tint to color
    let final_color = vec4<f32>(
        tex.rgb * tint_color,
        tex.a  // Use texture alpha directly
    );

    return final_color;
}

// War FX Scrolling Smoke Shader
// Converted from Unity shader: WFX_S Smoke Scroll
// Creates rising smoke effect via UV scrolling

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

struct SmokeScrollMaterial {
    tint_color: vec4<f32>,
    scroll_speed: f32,
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
    // DEBUG: Output UVs as color to verify shader execution
    return vec4<f32>(in.uv.x, in.uv.y, 0.0, 1.0);

    // var uv = in.uv;

    // // Get alpha mask from original UV (before scrolling)
    // let mask = textureSample(smoke_texture, smoke_sampler, uv).a;

    // // Apply UV scrolling (moves texture upward over time)
    // uv.y -= (globals.time * material.scroll_speed) % 1.0;

    // // Sample scrolled texture
    // var tex = textureSample(smoke_texture, smoke_sampler, uv);

    // // Apply tint and use mask for alpha
    // let final_color = vec4<f32>(
    //     tex.rgb * material.tint_color.rgb,
    //     tex.a * mask
    // );

    // // Lerp to gray based on mask (creates soft edges)
    // return mix(vec4<f32>(0.5, 0.5, 0.5, 0.5), final_color, mask);
}

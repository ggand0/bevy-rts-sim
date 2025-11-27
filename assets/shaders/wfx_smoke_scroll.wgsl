// War FX Scrolling Smoke Shader
// Converted from Unity shader: WFX_S Smoke Scroll
// Blend mode: DstColor SrcAlpha (multiply blend)
// Creates volumetric smoke effect via UV scrolling

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

    // Extract tint color from uniform RGB
    let tint_color = material.tint_color_and_speed.rgb;

    // Extract scroll_speed (integer part) and particle_alpha (decimal part) from w component
    let packed_w = material.tint_color_and_speed.a;
    let scroll_speed = floor(packed_w);
    let particle_alpha = fract(packed_w);

    // Sample texture alpha
    let tex_alpha = textureSample(smoke_texture, smoke_sampler, uv).a;

    // INVERT the texture alpha - this gave us bright core, dark rim
    let mask = (1.0 - tex_alpha) * particle_alpha;

    // For multiply blend: values > 0.5 brighten, < 0.5 darken
    // Boost tint toward white to ensure values > 0.5
    let white_amount = mask * 0.7;
    var bright_color = mix(tint_color, vec3<f32>(1.0), white_amount);
    bright_color = max(bright_color, vec3<f32>(0.6));

    // Unity lerp formula: mix(0.5, color, mask)
    let final_rgb = mix(vec3<f32>(0.5), bright_color, mask);

    return vec4<f32>(final_rgb, mask);
}

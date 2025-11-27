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

    // Sample texture alpha - it's actually LOW at center, HIGH at edges
    let tex_alpha = textureSample(smoke_texture, smoke_sampler, uv).a;

    // INVERT to get spatial mask: HIGH at center, LOW at edges
    let spatial_mask = 1.0 - tex_alpha;

    // particle_alpha controls the flame→smoke transition over lifetime:
    // High alpha (start, ~1.0) = bright flame (values > 0.5 brighten)
    // Low alpha (end, ~0.0) = dark smoke (values < 0.5 darken)

    // Bright flame color for the start
    var bright_color = mix(tint_color, vec3<f32>(1.0), 0.8);
    bright_color = max(bright_color, vec3<f32>(0.85));

    // Dark smoke color for the end (below 0.5 causes darkening in multiply blend)
    let smoke_color = vec3<f32>(0.3);

    // Transition from bright flame to dark smoke based on particle lifetime
    let lifetime_color = mix(smoke_color, bright_color, particle_alpha);

    // Apply spatial mask: center gets the color, edges get neutral 0.5
    let final_rgb = mix(vec3<f32>(0.5), lifetime_color, spatial_mask);

    // For multiply blend, we want the effect to stay visible as smoke
    // Keep alpha relatively high but fade at edges
    // Use particle_alpha^0.3 to keep it visible longer (slower fade)
    let fade_alpha = pow(particle_alpha, 0.3);
    let final_alpha = spatial_mask * fade_alpha;

    return vec4<f32>(final_rgb, final_alpha);
}

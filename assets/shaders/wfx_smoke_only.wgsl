// War FX Smoke-Only Shader
// Simplified version for pure smoke (no flame blending)
// Used by "Smoke" emitter - gray lingering smoke trail
// Blend mode: DstColor SrcAlpha (Unity's exact blend equation)

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

struct SmokeOnlyMaterial {
    tint_color_and_speed: vec4<f32>,  // RGB = tint (gray), A = packed(scroll_speed + particle_alpha)
}

@group(2) @binding(0)
var<uniform> material: SmokeOnlyMaterial;
@group(2) @binding(1)
var smoke_texture: texture_2d<f32>;
@group(2) @binding(2)
var smoke_sampler: sampler;

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    var uv = in.uv;

    // Extract tint color from uniform RGB (should be gray for smoke)
    let tint_color = material.tint_color_and_speed.rgb;

    // Extract scroll_speed (integer part) and particle_alpha (decimal part) from w component
    let packed_w = material.tint_color_and_speed.a;
    let scroll_speed = floor(packed_w);
    let particle_alpha = fract(packed_w);

    // Sample texture alpha (HIGH at center, LOW at edges)
    let tex_alpha = textureSample(smoke_texture, smoke_sampler, uv).a;

    // Unity formula: mask = tex_alpha * vertex_alpha (for visibility)
    let mask = tex_alpha * particle_alpha;

    // Lifetime-based darkening (similar to explosion shader)
    // particle_alpha: 1.0 at spawn → 0.0 at death
    // lifetime_t: 0.0 at spawn → 1.0 at death
    let lifetime_t = 1.0 - particle_alpha;

    // Darkening curve: starts at 1.0, gradually darkens to ~0.3 at end
    // This makes smoke get darker as it dissipates
    let lifetime_mult = mix(1.0, 0.3, lifetime_t);

    // Apply lifetime darkening to tint color
    let darkened_smoke = tint_color * lifetime_mult;

    // Unity's key formula: lerp(0.5, color, mask) for RGB
    // At mask=0 (edges): output 0.5 (neutral for multiply blend)
    // At mask=1 (center): output darkened_smoke
    let final_rgb = mix(vec3<f32>(0.5), darkened_smoke, mask);

    // Alpha for multiply blend: rgb + alpha should equal desired factor
    // For darkening smoke: we want rgb < 0.5, so use similar alpha
    let final_alpha = mix(0.5, darkened_smoke.r, mask);

    return vec4<f32>(final_rgb, final_alpha);
}

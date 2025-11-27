// War FX Scrolling Smoke Shader
// Converted from Unity shader: WFX_S Smoke Scroll
// Blend mode: DstColor SrcAlpha (Unity's exact blend equation)
// Creates volumetric smoke effect via UV scrolling

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

struct SmokeScrollMaterial {
    tint_color_and_speed: vec4<f32>,  // RGB = tint, A = packed(scroll_speed + particle_alpha)
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

    // Sample texture alpha directly (HIGH at center, LOW at edges in the file)
    // Try WITHOUT inversion first - use exactly like Unity
    let tex_alpha = textureSample(smoke_texture, smoke_sampler, uv).a;

    // Unity formula: mask = tex_alpha * vertex_alpha
    let mask = tex_alpha * particle_alpha;

    // Unity blend equation: result = dst * (src.rgb + src.a)
    // For neutral (no change to scene): src.rgb + src.a = 1.0
    // Unity's lerp(0.5, color, mask) gives:
    //   At edges (mask=0): rgb=0.5, alpha=0.5 → factor = 0.5 + 0.5 = 1.0 (neutral!)
    //   At center (mask=1): rgb=color, alpha=color.a → factor = color + alpha

    // Bright flame color for the start, dark smoke for the end
    var flame_color = mix(tint_color, vec3<f32>(1.0), 0.8);
    flame_color = max(flame_color, vec3<f32>(0.85));
    let smoke_color = vec3<f32>(0.3);

    // Transition from bright flame to dark smoke based on particle lifetime
    let lifetime_color = mix(smoke_color, flame_color, particle_alpha);

    // Unity's key formula: lerp(0.5, color, mask) for BOTH rgb AND alpha
    // This ensures edges are perfectly neutral (0.5 + 0.5 = 1.0)
    let final_rgb = mix(vec3<f32>(0.5), lifetime_color, mask);
    let final_alpha = mix(0.5, 1.0, mask);

    return vec4<f32>(final_rgb, final_alpha);
}

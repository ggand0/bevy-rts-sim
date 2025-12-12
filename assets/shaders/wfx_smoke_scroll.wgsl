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

    // Unity formula: mask = tex_alpha * vertex_alpha (for visibility)
    let mask = tex_alpha * particle_alpha;

    // Unity Start Color Gradient colors (from EXPLOSION_EMITTER_DETAILS.md):
    // - Bright orange flame: rgb(1.0, 0.522, 0.2)
    // - Dark red/brown smoke: rgb(0.663, 0.235, 0.184)
    // NOTE: tint_color now contains the Color Over Lifetime result from Rust,
    // so we apply it directly instead of calculating lifetime_mult here
    let base_flame_color = vec3<f32>(1.0, 0.522, 0.2) * tint_color;
    let base_smoke_color = vec3<f32>(0.663, 0.235, 0.184) * tint_color;

    // For bright fusing cores: blend toward white at center (high tex_alpha)
    // Multiply blend needs rgb + alpha > 1.0 to brighten
    // Orange (1.0, 0.522, 0.2) only brightens red, so we boost toward white at centers
    let center_boost = tex_alpha * particle_alpha;  // strongest at center + start of life
    let bright_flame = mix(base_flame_color, vec3<f32>(1.0, 0.95, 0.85), center_boost * 0.7);

    // Spatial blend: center=bright flame, edges=dark smoke
    let spatial_blend = tex_alpha * (0.5 + 0.5 * particle_alpha);
    let pixel_color = mix(base_smoke_color, bright_flame, spatial_blend);

    // Unity's key formula: lerp(0.5, color, mask) for RGB
    let final_rgb = mix(vec3<f32>(0.5), pixel_color, mask);

    // Alpha tied to color for proper brighten/darken blend math
    let target_alpha = pixel_color.r;
    let final_alpha = mix(0.5, target_alpha, mask);

    return vec4<f32>(final_rgb, final_alpha);
}

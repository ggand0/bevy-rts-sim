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
    // Packed as: scroll_speed.floor() + particle_alpha (where alpha is 0.0-1.0)
    let packed_w = material.tint_color_and_speed.a;
    let scroll_speed = floor(packed_w);
    let particle_alpha = fract(packed_w);

    // Unity shader: float mask = tex2D(_MainTex, i.uv).a * i.color.a;
    // Get alpha mask BEFORE scrolling, modulated by particle alpha
    let tex_alpha = textureSample(smoke_texture, smoke_sampler, uv).a;
    let mask = tex_alpha * particle_alpha;

    // Unity shader: i.uv.y -= fmod(_Time*_ScrollSpeed,1);
    // Apply UV scrolling (creates morphing/billowing effect)
    // TEMPORARILY DISABLED to debug twitching
    var scrolled_uv = uv;
    // scrolled_uv.y -= fract(globals.time * scroll_speed);

    // Unity shader: fixed4 tex = tex2D(_MainTex, i.uv);
    // Sample scrolled texture for RGB
    let tex = textureSample(smoke_texture, smoke_sampler, scrolled_uv);

    // Unity shader logic for multiply blend (Blend DstColor SrcAlpha):
    // tex.rgb *= i.color.rgb * _TintColor.rgb;
    // tex = lerp(fixed4(0.5,0.5,0.5,0.5), tex, mask);
    //
    // For multiply blend: output > 0.5 brightens the scene, output < 0.5 darkens
    // Overlapping particles with high values "fuse" and create bright white cores

    // Texture RGB is grayscale ~0.52-0.69, we need values > 0.5 to brighten
    // Unity tints the texture by particle color (orange for flames, brown for smoke)
    // The key is that the FINAL color after tinting must be > 0.5 for bright areas

    // Boost base texture brightness first (0.52 * 1.6 = 0.83)
    let boosted_tex = tex.rgb * 1.6;

    // Then apply tint color
    var tinted_rgb = boosted_tex * tint_color;

    // Clamp to valid range
    tinted_rgb = clamp(tinted_rgb, vec3<f32>(0.0), vec3<f32>(1.0));

    // Unity: tex = lerp(fixed4(0.5,0.5,0.5,0.5), tex, mask);
    // Blend between neutral gray and tinted color based on mask
    let final_rgb = mix(vec3<f32>(0.5), tinted_rgb, mask);

    return vec4<f32>(final_rgb, mask);
}

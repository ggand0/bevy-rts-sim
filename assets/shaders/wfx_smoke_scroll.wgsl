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

    // Extract tint color and scroll speed from uniform
    let tint_color = material.tint_color_and_speed.rgb;
    let scroll_speed = material.tint_color_and_speed.a;

    // Unity shader: float mask = tex2D(_MainTex, i.uv).a * i.color.a;
    // Get alpha mask BEFORE scrolling (this gives the particle shape)
    let mask = textureSample(smoke_texture, smoke_sampler, uv).a;

    // Unity shader: i.uv.y -= fmod(_Time*_ScrollSpeed,1);
    // Apply UV scrolling (creates morphing/billowing effect)
    var scrolled_uv = uv;
    scrolled_uv.y -= (globals.time * scroll_speed) % 1.0;

    // Unity shader: fixed4 tex = tex2D(_MainTex, i.uv);
    // Sample scrolled texture for RGB
    let tex = textureSample(smoke_texture, smoke_sampler, scrolled_uv);

    // Unity shader: tex.rgb *= i.color.rgb * _TintColor.rgb;
    // Tint the color
    var tinted_rgb = tex.rgb * tint_color;

    // Unity shader: tex = lerp(fixed4(0.5,0.5,0.5,0.5), tex, mask);
    // Lerp to gray (0.5) based on mask - this is critical for multiply blend
    // Where mask=0: output gray (neutral for multiply)
    // Where mask=1: output tinted color
    let gray = vec3<f32>(0.5, 0.5, 0.5);
    let final_rgb = mix(gray, tinted_rgb, mask);
    let final_alpha = mix(0.5, mask, mask);

    return vec4<f32>(final_rgb, final_alpha);
}

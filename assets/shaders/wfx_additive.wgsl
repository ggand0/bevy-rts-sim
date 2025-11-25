// War FX Additive Shader
// Converted from Unity shader: WFX_S Particle Add A8
// Creates bright glowing particles with additive blending
//
// NOTE: Soft particles (depth-based fade) are not currently supported.
// Bevy's depth prepass is not available for transparent/additive materials (AlphaMode::Add).
// This is a fundamental limitation of Bevy 0.15's rendering architecture.
// To implement soft particles would require a custom render pipeline or post-processing effect.

#import bevy_pbr::forward_io::VertexOutput

@group(2) @binding(0)
var<uniform> tint_color: vec4<f32>;
@group(2) @binding(1)
var<uniform> soft_particles_fade: vec4<f32>;  // Reserved for future soft particle implementation
@group(2) @binding(2)
var particle_texture: texture_2d<f32>;
@group(2) @binding(3)
var particle_sampler: sampler;

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    // Sample texture
    var tex = textureSample(particle_texture, particle_sampler, in.uv);

    // Extract tint color
    let tint = tint_color;

    // Unity "A8" textures use RGB luminance as alpha mask
    // Calculate luminance (perceived brightness) as alpha
    let luminance = dot(tex.rgb, vec3<f32>(0.299, 0.587, 0.114));

    // Discard only completely black pixels to avoid hard edges
    // Use a very low threshold to preserve soft falloff
    if (luminance < 0.001) {
        discard;
    }

    // Apply soft falloff to the edges for smoother blending
    // Remap luminance to enhance the soft gradient
    let soft_alpha = smoothstep(0.0, 0.1, luminance);

    // Apply 4x brightness multiplier (Unity shader does 2.0 * 2.0)
    // Use smoothed luminance for softer edges
    let brightness = 4.0;
    let final_color = vec4<f32>(
        tex.rgb * tint.rgb * brightness,
        soft_alpha * tint.a
    );

    return final_color;
}

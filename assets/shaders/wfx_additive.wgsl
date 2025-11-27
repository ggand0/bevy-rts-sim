// War FX Additive Shader
// Converted from Unity shader: WFX_S Particle Add A8
// Creates bright glowing particles with additive blending

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
    let tex = textureSample(particle_texture, particle_sampler, in.uv);

    // Unity "A8" textures use RGB luminance as alpha mask
    let luminance = dot(tex.rgb, vec3<f32>(0.299, 0.587, 0.114));

    // Soft falloff instead of hard discard - preserves gaussian-like edges
    // Use smoothstep for gradual fade at low luminance values
    let soft_alpha = smoothstep(0.0, 0.3, luminance);

    // Apply brightness multiplier (Unity shader does 2.0 * 2.0)
    let brightness = 4.0;

    // Combine luminance-based alpha with soft falloff and tint alpha
    let final_alpha = luminance * soft_alpha * tint_color.a;

    // RGB must also fade with tint_color.a for additive blending to work correctly
    // Otherwise the glow remains visible even when alpha is 0
    let final_color = vec4<f32>(
        tex.rgb * tint_color.rgb * brightness * soft_alpha * tint_color.a,
        final_alpha
    );

    return final_color;
}

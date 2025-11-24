// War FX Additive Shader
// Converted from Unity shader: WFX_S Particle Add A8
// Creates bright glowing particles with additive blending

#import bevy_pbr::forward_io::VertexOutput

struct AdditiveMaterial {
    tint_color: vec4<f32>,
}

@group(2) @binding(0)
var<uniform> material: AdditiveMaterial;
@group(2) @binding(1)
var particle_texture: texture_2d<f32>;
@group(2) @binding(2)
var particle_sampler: sampler;

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    // Sample texture
    var tex = textureSample(particle_texture, particle_sampler, in.uv);

    // Extract tint color
    let tint = material.tint_color;

    // Unity "A8" textures use RGB luminance as alpha mask
    // Calculate luminance (perceived brightness) as alpha
    let luminance = dot(tex.rgb, vec3<f32>(0.299, 0.587, 0.114));

    // Discard very dark pixels (black background)
    if (luminance < 0.05) {
        discard;
    }

    // Apply 4x brightness multiplier (Unity shader does 2.0 * 2.0)
    // Use luminance as alpha for proper masking
    let brightness = 4.0;
    let final_color = vec4<f32>(
        tex.rgb * tint.rgb * brightness,
        luminance * tint.a
    );

    return final_color;
}

// War FX custom materials with UV scrolling
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, ShaderRef, BlendState, BlendComponent, BlendFactor, BlendOperation,
};
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::render::render_resource::SpecializedMeshPipelineError;

/// Scrolling smoke material with UV animation
/// Based on Unity shader: WFX_S Smoke Scroll
/// Note: tint_color.w (alpha) stores the scroll_speed to keep uniforms in one binding
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SmokeScrollMaterial {
    /// RGB = tint color, A = scroll speed
    #[uniform(0)]
    pub tint_color_and_speed: Vec4,

    #[texture(1)]
    #[sampler(2)]
    pub smoke_texture: Handle<Image>,
}

/// Additive material for bright flames, glow, and sparks
/// Based on Unity shader: WFX_S Particle Add A8
/// Uses additive blending (ONE + ONE) with 4x brightness multiplier
/// Supports soft particles with depth-based fade
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct AdditiveMaterial {
    #[uniform(0)]
    pub tint_color: Vec4,

    #[uniform(1)]
    pub soft_particles_fade: Vec4, // x = inv_fade, yzw = padding

    #[texture(2)]
    #[sampler(3)]
    pub particle_texture: Handle<Image>,
}

impl Material for SmokeScrollMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/wfx_smoke_scroll.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        // We use Multiply as the base, but override the blend state in specialize()
        AlphaMode::Multiply
    }

    fn opaque_render_method(&self) -> bevy::pbr::OpaqueRendererMethod {
        bevy::pbr::OpaqueRendererMethod::Forward
    }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Override the blend state to match Unity's "Blend DstColor SrcAlpha"
        // Unity blend: result = dst * src.rgb + dst * src.a = dst * (src.rgb + src.a)
        //
        // Bevy's default Multiply is: dst * src.rgb + dst * (1 - src.a)
        // which is different!
        //
        // Unity's formula makes edges (rgb=0.5, a=0) darken: factor = 0.5
        // This is why lerp(0.5, tex, mask) works - at mask=0, output is 0.5
        // and the blend multiplies dst by 0.5 + 0 = 0.5 (darken slightly)
        // But visually it appears neutral because the texture detail is lost.
        //
        // For truly neutral edges, we need: dst * 1.0 = dst
        // So we need: src.rgb + src.a = 1.0, meaning src.rgb = 1 - src.a
        // Or equivalently, output (1 - mask, mask) at edges for perfect neutrality.
        if let Some(ref mut fragment) = descriptor.fragment {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState {
                    color: BlendComponent {
                        // Unity: Blend DstColor SrcAlpha
                        // result.rgb = src_factor * src.rgb + dst_factor * dst.rgb
                        // For DstColor SrcAlpha:
                        //   src_factor = DstColor (dst.rgb)
                        //   dst_factor = SrcAlpha (src.a)
                        // result.rgb = dst.rgb * src.rgb + src.a * dst.rgb
                        //            = dst.rgb * (src.rgb + src.a)
                        src_factor: BlendFactor::Dst,
                        dst_factor: BlendFactor::SrcAlpha,
                        operation: BlendOperation::Add,
                    },
                    alpha: BlendComponent {
                        // For alpha, use standard over blending
                        src_factor: BlendFactor::One,
                        dst_factor: BlendFactor::OneMinusSrcAlpha,
                        operation: BlendOperation::Add,
                    },
                });
            }
        }
        Ok(())
    }
}

impl Default for SmokeScrollMaterial {
    fn default() -> Self {
        Self {
            // RGB = white tint, A = scroll speed of 2.0
            tint_color_and_speed: Vec4::new(1.0, 1.0, 1.0, 2.0),
            smoke_texture: Handle::default(),
        }
    }
}

impl Material for AdditiveMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/wfx_additive.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add  // ONE + ONE additive blending
    }

    fn opaque_render_method(&self) -> bevy::pbr::OpaqueRendererMethod {
        bevy::pbr::OpaqueRendererMethod::Forward
    }
}

impl Default for AdditiveMaterial {
    fn default() -> Self {
        Self {
            tint_color: Vec4::ONE,
            soft_particles_fade: Vec4::new(1.0, 0.0, 0.0, 0.0), // x = inv_fade
            particle_texture: Handle::default(),
        }
    }
}

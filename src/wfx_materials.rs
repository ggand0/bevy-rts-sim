// War FX custom materials with UV scrolling
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use bevy::pbr::Material;

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
        // Unity uses: Blend DstColor SrcAlpha (multiply blend)
        // Bevy's Multiply is: Dst * Src
        AlphaMode::Multiply
    }

    fn opaque_render_method(&self) -> bevy::pbr::OpaqueRendererMethod {
        bevy::pbr::OpaqueRendererMethod::Forward
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

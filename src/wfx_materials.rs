// War FX custom materials with UV scrolling
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use bevy::pbr::Material;

/// Scrolling smoke material with UV animation
/// Based on Unity shader: WFX_S Smoke Scroll
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SmokeScrollMaterial {
    #[uniform(0)]
    pub tint_color: Vec4,
    #[uniform(0)]
    pub scroll_speed: f32,

    #[texture(1)]
    #[sampler(2)]
    pub smoke_texture: Handle<Image>,
}

impl Material for SmokeScrollMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/wfx_smoke_scroll.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn opaque_render_method(&self) -> bevy::pbr::OpaqueRendererMethod {
        bevy::pbr::OpaqueRendererMethod::Forward
    }
}

impl Default for SmokeScrollMaterial {
    fn default() -> Self {
        Self {
            tint_color: Vec4::ONE,
            scroll_speed: 2.0,
            smoke_texture: Handle::default(),
        }
    }
}

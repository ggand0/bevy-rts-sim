use bevy::prelude::*;
use bevy::pbr::MaterialPlugin;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::alpha::AlphaMode;

pub struct ShieldPlugin;

impl Plugin for ShieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ShieldMaterial>::default())
            .add_systems(Update, animate_shields);
    }
}

#[derive(Component)]
pub struct Shield {
    pub material_handle: Handle<ShieldMaterial>,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ShieldMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(0)]
    pub fresnel_power: f32,
    #[uniform(0)]
    pub hex_scale: f32,
    #[uniform(0)]
    pub time: f32,
    #[uniform(0)]
    pub _padding: f32,
}

impl Material for ShieldMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/shield.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        0.0
    }
}

impl Default for ShieldMaterial {
    fn default() -> Self {
        Self {
            color: LinearRgba::rgb(0.2, 0.6, 1.0), // Cyan/blue
            fresnel_power: 3.0,
            hex_scale: 8.0,
            time: 0.0,
            _padding: 0.0,
        }
    }
}

/// Creates a hemisphere mesh (upper half of a sphere)
pub fn create_hemisphere_mesh(radius: f32, segments: u32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Generate vertices for upper hemisphere only
    for lat in 0..=segments {
        let theta = std::f32::consts::PI * 0.5 * (lat as f32) / (segments as f32); // 0 to PI/2
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for lon in 0..=segments {
            let phi = 2.0 * std::f32::consts::PI * (lon as f32) / (segments as f32);
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();

            let x = sin_theta * cos_phi;
            let y = cos_theta;
            let z = sin_theta * sin_phi;

            positions.push([x * radius, y * radius, z * radius]);
            normals.push([x, y, z]);
            uvs.push([lon as f32 / segments as f32, lat as f32 / segments as f32]);
        }
    }

    // Generate indices
    for lat in 0..segments {
        for lon in 0..segments {
            let first = lat * (segments + 1) + lon;
            let second = first + segments + 1;

            indices.push(first);
            indices.push(second);
            indices.push(first + 1);

            indices.push(second);
            indices.push(second + 1);
            indices.push(first + 1);
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::render::render_asset::RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// Spawns a shield around a position
pub fn spawn_shield(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ShieldMaterial>>,
    position: Vec3,
    radius: f32,
    team_color: Color,
) -> Entity {
    let shield_mesh = create_hemisphere_mesh(radius, 32);

    let shield_material = ShieldMaterial {
        color: team_color.to_linear(),
        fresnel_power: 3.0,
        hex_scale: 8.0,
        time: 0.0,
        _padding: 0.0,
    };

    let material_handle = materials.add(shield_material);

    commands.spawn((
        Mesh3d(meshes.add(shield_mesh)),
        MeshMaterial3d(material_handle.clone()),
        Transform::from_translation(position),
        Shield {
            material_handle: material_handle.clone(),
        },
    )).id()
}

/// Animates shield time for energy pulses
fn animate_shields(
    time: Res<Time>,
    mut materials: ResMut<Assets<ShieldMaterial>>,
    query: Query<&Shield>,
) {
    for shield in query.iter() {
        if let Some(material) = materials.get_mut(&shield.material_handle) {
            material.time = time.elapsed_secs();
        }
    }
}

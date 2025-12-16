//! Utility to render debris meshes from multiple angles into a sprite sheet
//! Run with: cargo run --bin render_debris_sprites
//!
//! Generates: assets/textures/generated/debris_v{variant}_a{angle}.png
//! Layout: 3 variants x 8 rotation angles = 24 files
//! Each image: 64x64 pixels

use bevy::prelude::*;
use bevy::render::camera::ClearColorConfig;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::f32::consts::PI;

const CELL_SIZE: u32 = 64;
const NUM_ANGLES: u32 = 8;
const NUM_VARIANTS: u32 = 3;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Debris Sprite Renderer".to_string(),
                resolution: (CELL_SIZE as f32, CELL_SIZE as f32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, (render_frame, check_exit))
        .insert_resource(RenderState::default())
        .run();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderPhase {
    Setup,      // Set up mesh visibility/rotation
    Wait,       // Wait for render
    Capture,    // Take screenshot
}

#[derive(Resource)]
struct RenderState {
    current_variant: usize,
    current_angle: usize,
    frame_delay: u32,
    phase: RenderPhase,
    done: bool,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            current_variant: 0,
            current_angle: 0,
            frame_delay: 10,  // Wait 10 frames at startup for rendering pipeline to initialize
            phase: RenderPhase::Setup,  // Start with Setup to position the first mesh
            done: false,
        }
    }
}

#[derive(Component)]
struct DebrisMesh;

#[derive(Component)]
struct DebrisCamera;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Create the 3 debris mesh variants (same as ground_explosion.rs)
    let debris_meshes = [
        // Variant 0: Small cube (chunky rock)
        meshes.add(Cuboid::new(1.0, 0.8, 0.6)),
        // Variant 1: Flat slab (debris piece)
        meshes.add(Cuboid::new(1.2, 0.4, 0.8)),
        // Variant 2: Elongated piece (shrapnel)
        meshes.add(Cuboid::new(0.5, 0.5, 1.4)),
    ];

    // Debris material - dark brown/grey
    let debris_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.30, 0.24),
        perceptual_roughness: 1.0,
        metallic: 0.0,
        unlit: false,
        ..default()
    });

    // Spawn all meshes (we'll show one at a time by toggling visibility)
    for (i, mesh) in debris_meshes.iter().enumerate() {
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(debris_material.clone()),
            Transform::from_translation(Vec3::ZERO),
            Visibility::Hidden,
            DebrisMesh,
            Name::new(format!("Debris_{}", i)),
        ));
    }

    // Camera looking at origin - position for good framing
    // Use transparent clear color for alpha background
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        Transform::from_xyz(0.0, 0.8, 2.5).looking_at(Vec3::ZERO, Vec3::Y),
        DebrisCamera,
    ));

    // Directional light
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, 0.5, 0.0)),
    ));

    // Ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 500.0,
        affects_lightmapped_meshes: false,
    });

    info!("Setup complete. Will render {} variants x {} angles = {} frames",
          NUM_VARIANTS, NUM_ANGLES, NUM_VARIANTS * NUM_ANGLES);
}

fn render_frame(
    mut state: ResMut<RenderState>,
    mut meshes: Query<(&mut Visibility, &mut Transform, &Name), With<DebrisMesh>>,
    mut commands: Commands,
) {
    if state.done {
        return;
    }

    // Handle delay countdown
    if state.frame_delay > 0 {
        state.frame_delay -= 1;
        return;
    }

    match state.phase {
        RenderPhase::Setup => {
            // Set up mesh visibility and rotation for current frame
            let variant = state.current_variant;
            let angle_idx = state.current_angle;
            let angle = (angle_idx as f32 / NUM_ANGLES as f32) * 2.0 * PI;

            for (mut vis, mut transform, name) in meshes.iter_mut() {
                let mesh_idx: usize = name.as_str().strip_prefix("Debris_")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(99);

                if mesh_idx == variant {
                    *vis = Visibility::Visible;
                    transform.rotation = Quat::from_rotation_y(angle);
                } else {
                    *vis = Visibility::Hidden;
                }
            }

            // Wait 3 frames for the mesh to render
            state.frame_delay = 3;
            state.phase = RenderPhase::Wait;
        }

        RenderPhase::Wait => {
            // After waiting, capture
            state.phase = RenderPhase::Capture;
        }

        RenderPhase::Capture => {
            let variant = state.current_variant;
            let angle_idx = state.current_angle;
            let angle = (angle_idx as f32 / NUM_ANGLES as f32) * 2.0 * PI;

            let filename = format!("debris_v{}_a{}.png", variant, angle_idx);
            info!("Capturing: variant={}, angle={} ({}°) -> {}",
                  variant, angle_idx, (angle * 180.0 / PI) as i32, filename);

            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!("assets/textures/generated/{}", filename)));

            // Advance to next frame
            state.current_angle += 1;
            if state.current_angle >= NUM_ANGLES as usize {
                state.current_angle = 0;
                state.current_variant += 1;
            }

            // Check if done
            if state.current_variant >= NUM_VARIANTS as usize {
                info!("All {} frames rendered! Check assets/textures/generated/",
                      NUM_VARIANTS * NUM_ANGLES);
                state.done = true;
                // Wait a bit for last screenshot to save
                state.frame_delay = 10;
                return;
            }

            // Go back to setup for next frame
            state.phase = RenderPhase::Setup;
        }
    }
}

fn check_exit(
    mut state: ResMut<RenderState>,
    mut exit: EventWriter<AppExit>,
) {
    if state.done {
        if state.frame_delay > 0 {
            state.frame_delay -= 1;
        } else {
            exit.write(AppExit::Success);
        }
    }
}

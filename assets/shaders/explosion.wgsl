#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::mesh_bindings::mesh
#import bevy_pbr::mesh_functions

// ===== EXPLOSION MATERIAL UNIFORMS =====

@group(2) @binding(0) var<uniform> explosion_data: ExplosionData;
@group(2) @binding(1) var noise_texture: texture_2d<f32>;
@group(2) @binding(2) var noise_sampler: sampler;

struct ExplosionData {
    time: f32,                    // Time since explosion start
    intensity: f32,               // Explosion brightness multiplier  
    max_radius: f32,              // Maximum explosion radius
    center: vec3<f32>,            // World space center of explosion
    fade_start: f32,              // When to start fading (0.0-1.0)
    fade_end: f32,                // When explosion ends (0.0-1.0)
    noise_scale: f32,             // Scale for procedural noise
    explosion_type: f32,          // 0=fireball, 1=smoke, 2=shockwave
}

// ===== VERTEX SHADER =====

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vertex(vertex: Vertex, @builtin(instance_index) instance_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    // Transform vertex to world space using mesh model matrix
    let model = bevy_pbr::mesh_functions::get_world_from_local(instance_index);
    out.world_position = model * vec4<f32>(vertex.position, 1.0);
    out.position = bevy_pbr::mesh_view_bindings::view.clip_from_world * out.world_position;
    out.uv = vertex.uv;
    
    return out;
}

// ===== NOISE FUNCTIONS =====

// Hash function for procedural noise
fn hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

// 2D noise function
fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    
    let u = f * f * (3.0 - 2.0 * f);
    
    return mix(a, b, u.x) + (c - a) * u.y * (1.0 - u.x) + (d - b) * u.x * u.y;
}

// Fractal Brownian Motion (multiple octaves of noise)
fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;
    
    for (var i = 0; i < octaves; i++) {
        value += amplitude * noise(pos);
        pos *= 2.0;
        amplitude *= 0.5;
    }
    
    return value;
}

// 3D noise using texture sampling
fn texture_noise_3d(pos: vec3<f32>) -> f32 {
    let xy_noise = textureSample(noise_texture, noise_sampler, pos.xy * 0.1).r;
    let xz_noise = textureSample(noise_texture, noise_sampler, pos.xz * 0.1).g;
    let yz_noise = textureSample(noise_texture, noise_sampler, pos.yz * 0.1).b;
    
    return (xy_noise + xz_noise + yz_noise) / 3.0;
}

// ===== FRAGMENT SHADER =====

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_position.xyz;
    let uv = in.uv;
    
    // Distance from explosion center
    let dist_from_center = distance(world_pos, explosion_data.center);
    let normalized_dist = dist_from_center / explosion_data.max_radius;
    
    // Time progression (0.0 to 1.0)
    let time_progress = explosion_data.time / 2.0; // 2 second explosion duration
    let clamped_time = clamp(time_progress, 0.0, 1.0);
    
    // UV coordinates centered around explosion
    let centered_uv = (uv - 0.5) * 2.0;
    let radial_dist = length(centered_uv);
    
    // === EXPLOSION TYPE ROUTING ===
    
    if (explosion_data.explosion_type < 0.5) {
        // FIREBALL TYPE
        return render_fireball(world_pos, uv, centered_uv, radial_dist, normalized_dist, clamped_time);
    } else if (explosion_data.explosion_type < 1.5) {
        // SMOKE TYPE  
        return render_smoke(world_pos, uv, centered_uv, radial_dist, normalized_dist, clamped_time);
    } else {
        // SHOCKWAVE TYPE
        return render_shockwave(world_pos, uv, centered_uv, radial_dist, normalized_dist, clamped_time);
    }
}

// ===== FIREBALL RENDERER =====

fn render_fireball(
    world_pos: vec3<f32>,
    uv: vec2<f32>, 
    centered_uv: vec2<f32>,
    radial_dist: f32,
    normalized_dist: f32,
    time_progress: f32
) -> vec4<f32> {
    
    // Expanding fireball radius based on time
    let expansion_curve = 1.0 - exp(-time_progress * 4.0); // Fast initial expansion
    let current_radius = expansion_curve;
    
    // Early exit if outside current explosion radius
    if (radial_dist > current_radius) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    
    // === VOLUMETRIC CORE ===
    
    // Multiple noise layers for complex fire patterns
    let noise_time = explosion_data.time * 2.0;
    let noise_pos = world_pos * explosion_data.noise_scale + vec3<f32>(noise_time * 0.3);
    
    // High frequency detail noise
    let detail_noise = fbm(noise_pos.xy + noise_time * 0.5, 4);
    
    // Low frequency shape noise  
    let shape_noise = fbm(noise_pos.xz * 0.3 + noise_time * 0.2, 3);
    
    // Texture-based noise for additional detail
    let texture_noise = texture_noise_3d(noise_pos);
    
    // Combine noise layers
    let combined_noise = (detail_noise * 0.4 + shape_noise * 0.4 + texture_noise * 0.2);
    
    // === PLASMA FLICKERING ===
    
    // Create plasma flickering effect
    let flicker_freq = explosion_data.time * 8.0 + radial_dist * 6.0;
    let flicker = sin(flicker_freq) * 0.1 + sin(flicker_freq * 1.7) * 0.05;
    
    // === CORE INTENSITY ===
    
    // Distance-based intensity (bright center, softer edges)
    let core_intensity = (1.0 - radial_dist) * (1.0 - radial_dist);
    
    // Add noise variation to intensity
    let noisy_intensity = core_intensity * (0.7 + combined_noise * 0.6 + flicker);
    
    // === EDGE DISSOLUTION ===
    
    // Create organic, dissolving edges using noise
    let edge_threshold = 0.3 + time_progress * 0.4; // Dissolve over time
    let edge_noise = fbm(uv * 8.0 + vec2<f32>(noise_time * 0.4), 3);
    
    // Soft edge falloff with noise-based dissolution
    let edge_factor = smoothstep(edge_threshold - 0.1, edge_threshold + 0.1, edge_noise + (1.0 - radial_dist));
    
    // === COLOR TEMPERATURE ===
    
    // Temperature cooling over time (white → yellow → orange → red)
    let heat = (1.0 - time_progress) * noisy_intensity;
    
    var fireball_color: vec3<f32>;
    if (heat > 0.8) {
        // White hot core
        fireball_color = vec3<f32>(1.0, 1.0, 0.9);
    } else if (heat > 0.6) {
        // Yellow flame
        fireball_color = mix(vec3<f32>(1.0, 0.8, 0.3), vec3<f32>(1.0, 1.0, 0.9), (heat - 0.6) / 0.2);
    } else if (heat > 0.3) {
        // Orange flame  
        fireball_color = mix(vec3<f32>(1.0, 0.4, 0.1), vec3<f32>(1.0, 0.8, 0.3), (heat - 0.3) / 0.3);
    } else {
        // Red embers
        fireball_color = mix(vec3<f32>(0.8, 0.1, 0.0), vec3<f32>(1.0, 0.4, 0.1), heat / 0.3);
    }
    
    // === EMISSIVE BOOST ===
    
    // Boost emissive for bloom compatibility
    let emissive_boost = explosion_data.intensity * 2.0;
    fireball_color *= emissive_boost;
    
    // === FINAL ALPHA ===
    
    // Combine all factors for final alpha
    let final_alpha = noisy_intensity * edge_factor * (1.0 - time_progress * 0.5);
    
    return vec4<f32>(fireball_color, clamp(final_alpha, 0.0, 1.0));
}

// ===== SMOKE RENDERER =====

fn render_smoke(
    world_pos: vec3<f32>,
    uv: vec2<f32>,
    centered_uv: vec2<f32>, 
    radial_dist: f32,
    normalized_dist: f32,
    time_progress: f32
) -> vec4<f32> {
    
    // Smoke appears after fireball starts fading
    let smoke_start_time = 0.3;
    if (time_progress < smoke_start_time) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    
    let smoke_progress = (time_progress - smoke_start_time) / (1.0 - smoke_start_time);
    
    // === SMOKE EXPANSION ===
    
    // Smoke expands and rises
    let expansion = smoke_progress * 1.5;
    let rise_offset = smoke_progress * explosion_data.max_radius * 0.8;
    
    // Adjust UV for rising motion
    let rising_uv = uv + vec2<f32>(0.0, -rise_offset / explosion_data.max_radius);
    
    // === SMOKE NOISE ===
    
    let noise_time = explosion_data.time * 1.5;
    let smoke_noise_pos = world_pos * explosion_data.noise_scale * 0.5 + vec3<f32>(0.0, noise_time * 0.4, 0.0);
    
    // Large billowing patterns
    let billow_noise = fbm(smoke_noise_pos.xy * 0.3, 3);
    
    // Fine detail wisps
    let wisp_noise = fbm(smoke_noise_pos.xz * 2.0 + vec2<f32>(noise_time * 0.6), 4);
    
    // Texture noise for additional complexity
    let texture_smoke_noise = texture_noise_3d(smoke_noise_pos);
    
    let combined_smoke_noise = billow_noise * 0.5 + wisp_noise * 0.3 + texture_smoke_noise * 0.2;
    
    // === SMOKE DENSITY ===
    
    // Density falls off with distance and time
    let density_falloff = (1.0 - radial_dist) * (1.0 - smoke_progress * 0.7);
    let noisy_density = density_falloff * (0.6 + combined_smoke_noise * 0.8);
    
    // === SMOKE COLOR ===
    
    // Dark gray smoke with slight color variation
    let base_smoke_color = vec3<f32>(0.1, 0.1, 0.1);
    let smoke_variation = vec3<f32>(0.05, 0.03, 0.02) * combined_smoke_noise;
    let final_smoke_color = base_smoke_color + smoke_variation;
    
    // === SMOKE ALPHA ===
    
    let smoke_alpha = noisy_density * 0.8 * (1.0 - smoke_progress * 0.6);
    
    return vec4<f32>(final_smoke_color, clamp(smoke_alpha, 0.0, 1.0));
}

// ===== SHOCKWAVE RENDERER =====

fn render_shockwave(
    world_pos: vec3<f32>,
    uv: vec2<f32>,
    centered_uv: vec2<f32>,
    radial_dist: f32, 
    normalized_dist: f32,
    time_progress: f32
) -> vec4<f32> {
    
    // Shockwave is a fast expanding ring
    let wave_speed = 3.0; // Very fast expansion
    let current_wave_radius = time_progress * wave_speed;
    
    // Ring thickness
    let ring_thickness = 0.1;
    let ring_inner = current_wave_radius - ring_thickness;
    let ring_outer = current_wave_radius + ring_thickness;
    
    // Check if we're in the ring
    if (radial_dist < ring_inner || radial_dist > ring_outer) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    
    // === RING INTENSITY ===
    
    // Distance from ring center line
    let ring_center_dist = abs(radial_dist - current_wave_radius) / ring_thickness;
    let ring_intensity = 1.0 - ring_center_dist;
    
    // === SHOCKWAVE NOISE ===
    
    let wave_noise_pos = centered_uv * 8.0 + vec2<f32>(explosion_data.time * 2.0);
    let wave_noise = fbm(wave_noise_pos, 3);
    
    // Add noise variation to ring
    let noisy_ring_intensity = ring_intensity * (0.7 + wave_noise * 0.6);
    
    // === SHOCKWAVE COLOR ===
    
    // Bright yellow-white with energy glow
    let wave_color = vec3<f32>(1.0, 0.9, 0.6) * explosion_data.intensity * 3.0;
    
    // === SHOCKWAVE ALPHA ===
    
    // Fast fade based on time
    let wave_alpha = noisy_ring_intensity * (1.0 - time_progress * time_progress) * 0.8;
    
    return vec4<f32>(wave_color, clamp(wave_alpha, 0.0, 1.0));
} 
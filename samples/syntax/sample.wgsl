// WGSL (WebGPU Shading Language) Syntax Highlighting Test
// A compute-based particle system with rendering pipeline.

// Struct definitions
struct Particle {
    position: vec3<f32>,
    velocity: vec3<f32>,
    color: vec4<f32>,
    life: f32,
    size: f32,
    _padding: vec2<f32>,  // Align to 16 bytes
}

struct SimParams {
    delta_time: f32,
    gravity: f32,
    damping: f32,
    noise_scale: f32,
    bounds_min: vec3<f32>,
    _pad0: f32,
    bounds_max: vec3<f32>,
    _pad1: f32,
    emitter_pos: vec3<f32>,
    emit_rate: f32,
    time: f32,
    particle_count: u32,
    _pad2: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) world_pos: vec3<f32>,
}

struct Camera {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    view_projection: mat4x4<f32>,
    position: vec3<f32>,
    _pad: f32,
}

// Bind group layouts
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<uniform> params: SimParams;

@group(1) @binding(0) var<storage, read> particles_in: array<Particle>;
@group(1) @binding(1) var<storage, read_write> particles_out: array<Particle>;

@group(2) @binding(0) var particle_texture: texture_2d<f32>;
@group(2) @binding(1) var particle_sampler: sampler;

// Constants
const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 6.28318530718;
const WORKGROUP_SIZE: u32 = 256;

// Noise functions for organic motion
fn hash(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.zyx + 31.32);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise3d(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    return mix(
        mix(
            mix(hash(i + vec3(0.0, 0.0, 0.0)), hash(i + vec3(1.0, 0.0, 0.0)), u.x),
            mix(hash(i + vec3(0.0, 1.0, 0.0)), hash(i + vec3(1.0, 1.0, 0.0)), u.x),
            u.y
        ),
        mix(
            mix(hash(i + vec3(0.0, 0.0, 1.0)), hash(i + vec3(1.0, 0.0, 1.0)), u.x),
            mix(hash(i + vec3(0.0, 1.0, 1.0)), hash(i + vec3(1.0, 1.0, 1.0)), u.x),
            u.y
        ),
        u.z
    );
}

fn curl_noise(p: vec3<f32>) -> vec3<f32> {
    let e = 0.01;
    let dx = vec3(e, 0.0, 0.0);
    let dy = vec3(0.0, e, 0.0);
    let dz = vec3(0.0, 0.0, e);

    let px = noise3d(p + dy) - noise3d(p - dy) - noise3d(p + dz) + noise3d(p - dz);
    let py = noise3d(p + dz) - noise3d(p - dz) - noise3d(p + dx) + noise3d(p - dx);
    let pz = noise3d(p + dx) - noise3d(p - dx) - noise3d(p + dy) + noise3d(p - dy);

    return normalize(vec3(px, py, pz)) / (2.0 * e);
}

// Compute shader: particle simulation
@compute @workgroup_size(WORKGROUP_SIZE)
fn simulate(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= params.particle_count) {
        return;
    }

    var p = particles_in[index];

    // Skip dead particles
    if (p.life <= 0.0) {
        // Respawn at emitter
        p.position = params.emitter_pos;
        p.velocity = vec3(
            (hash(vec3(f32(index), params.time, 0.0)) - 0.5) * 2.0,
            hash(vec3(f32(index), params.time, 1.0)) * 3.0 + 1.0,
            (hash(vec3(f32(index), params.time, 2.0)) - 0.5) * 2.0
        );
        p.life = 1.0 + hash(vec3(f32(index), params.time, 3.0)) * 2.0;
        p.size = 0.05 + hash(vec3(f32(index), params.time, 4.0)) * 0.1;
        p.color = vec4(
            0.8 + hash(vec3(f32(index), 0.0, 5.0)) * 0.2,
            0.3 + hash(vec3(f32(index), 0.0, 6.0)) * 0.4,
            0.1,
            1.0
        );
    }

    // Apply forces
    let gravity_force = vec3(0.0, -params.gravity, 0.0);
    let noise_force = curl_noise(p.position * params.noise_scale + params.time * 0.5) * 2.0;
    let total_force = gravity_force + noise_force;

    // Integrate velocity and position
    p.velocity += total_force * params.delta_time;
    p.velocity *= params.damping;
    p.position += p.velocity * params.delta_time;

    // Bounce off bounds
    for (var i = 0u; i < 3u; i++) {
        if (p.position[i] < params.bounds_min[i]) {
            p.position[i] = params.bounds_min[i];
            p.velocity[i] *= -0.5;
        }
        if (p.position[i] > params.bounds_max[i]) {
            p.position[i] = params.bounds_max[i];
            p.velocity[i] *= -0.5;
        }
    }

    // Age particle
    p.life -= params.delta_time;
    p.color.a = smoothstep(0.0, 0.3, p.life);

    particles_out[index] = p;
}

// Vertex shader: billboard particles
@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    let particle = particles_in[instance_index];

    // Quad vertices (triangle strip)
    let quad_pos = array<vec2<f32>, 4>(
        vec2(-0.5, -0.5),
        vec2( 0.5, -0.5),
        vec2(-0.5,  0.5),
        vec2( 0.5,  0.5),
    );

    let uv_coords = array<vec2<f32>, 4>(
        vec2(0.0, 1.0),
        vec2(1.0, 1.0),
        vec2(0.0, 0.0),
        vec2(1.0, 0.0),
    );

    let pos = quad_pos[vertex_index];
    let uv = uv_coords[vertex_index];

    // Billboard: align quad to face camera
    let camera_right = vec3(camera.view[0][0], camera.view[1][0], camera.view[2][0]);
    let camera_up = vec3(camera.view[0][1], camera.view[1][1], camera.view[2][1]);

    let world_pos = particle.position
        + camera_right * pos.x * particle.size
        + camera_up * pos.y * particle.size;

    var output: VertexOutput;
    output.clip_position = camera.view_projection * vec4(world_pos, 1.0);
    output.color = particle.color;
    output.uv = uv;
    output.world_pos = world_pos;

    return output;
}

// Fragment shader: textured particle with soft edges
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(particle_texture, particle_sampler, input.uv);

    // Soft circular falloff
    let dist = length(input.uv - vec2(0.5));
    let alpha = 1.0 - smoothstep(0.3, 0.5, dist);

    // Distance fade
    let camera_dist = length(camera.position - input.world_pos);
    let distance_fade = 1.0 - smoothstep(50.0, 100.0, camera_dist);

    var final_color = input.color * tex_color;
    final_color.a *= alpha * distance_fade;

    // Premultiplied alpha
    return vec4(final_color.rgb * final_color.a, final_color.a);
}

// Fullscreen post-processing pass
struct FullscreenOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> FullscreenOutput {
    // Full-screen triangle trick (3 vertices, no buffer needed)
    let uv = vec2(f32((index << 1u) & 2u), f32(index & 2u));
    var output: FullscreenOutput;
    output.position = vec4(uv * 2.0 - 1.0, 0.0, 1.0);
    output.uv = vec2(uv.x, 1.0 - uv.y);
    return output;
}

@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;

@fragment
fn fs_tonemap(input: FullscreenOutput) -> @location(0) vec4<f32> {
    let color = textureSample(scene_texture, scene_sampler, input.uv).rgb;

    // ACES tonemapping
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    let mapped = saturate((color * (a * color + b)) / (color * (c * color + d) + e));

    // Gamma correction
    let gamma = pow(mapped, vec3(1.0 / 2.2));

    return vec4(gamma, 1.0);
}

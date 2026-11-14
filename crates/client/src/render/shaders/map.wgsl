struct CameraUniform {
    view_proj: mat4x4<f32>,
    internal_border_alpha: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var<storage, read> province_colors: array<vec4<f32>>;

struct OceanOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec2<f32>,
};

@vertex
fn vs_ocean(@builtin(vertex_index) vertex_index: u32) -> OceanOutput {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-10800.0, -3200.0),
        vec2<f32>( 10800.0, -3200.0),
        vec2<f32>(-10800.0,  3200.0),
        vec2<f32>(-10800.0,  3200.0),
        vec2<f32>( 10800.0, -3200.0),
        vec2<f32>( 10800.0,  3200.0)
    );
    let world_pos = positions[vertex_index];
    var out: OceanOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    out.world_pos = world_pos;
    return out;
}

fn hash2(p: vec2<f32>) -> f32 {
    let p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    let p3_dot = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3_dot.x + p3_dot.y) * p3_dot.z);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash2(i + vec2<f32>(0.0, 0.0)), hash2(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash2(i + vec2<f32>(0.0, 1.0)), hash2(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var pos = p;
    let rot = mat2x2<f32>(0.8, -0.6, 0.6, 0.8);
    for (var i = 0; i < 3; i = i + 1) {
        v += a * value_noise(pos);
        pos = rot * pos * 2.1 + vec2<f32>(100.0, 100.0);
        a *= 0.5;
    }
    return v;
}

@fragment
fn fs_ocean(in: OceanOutput) -> @location(0) vec4<f32> {
    let wrapped_x = (fract((in.world_pos.x + 2700.0) / 5400.0) - 0.5) * 5400.0;

    let lat_factor = clamp(abs(in.world_pos.y) / 2200.0, 0.0, 1.0);
    let deep_ocean = vec3<f32>(0.07, 0.11, 0.18);
    let polar_ocean = vec3<f32>(0.09, 0.13, 0.19);
    var col = mix(deep_ocean, polar_ocean, lat_factor * 0.5);

    let wave_macro = fbm(in.world_pos * 0.0035);
    let wave_micro = fbm(in.world_pos * 0.012 + vec2<f32>(0.15, -0.15));
    let wave = (wave_macro - 0.5) * 0.04 + (wave_micro - 0.5) * 0.02;
    col = col + vec3<f32>(wave * 0.7, wave * 0.85, wave * 1.1);

    let eq_dist = abs(in.world_pos.y);
    let eq_line = smoothstep(1.8, 0.0, eq_dist) * 0.15;
    col = col + vec3<f32>(eq_line * 0.85, eq_line * 0.75, eq_line * 0.45);

    let tropic_dist = min(abs(in.world_pos.y - 363.0), abs(in.world_pos.y + 363.0));
    let tropic_dash = step(0.0, sin(in.world_pos.x * 0.04));
    let tropic_line = smoothstep(1.5, 0.0, tropic_dist) * tropic_dash * 0.07;
    col = col + vec3<f32>(tropic_line * 0.7, tropic_line * 0.75, tropic_line * 0.85);

    let polar_circle_dist = min(abs(in.world_pos.y - 1420.0), abs(in.world_pos.y + 1420.0));
    let polar_dash = step(0.0, sin(in.world_pos.x * 0.03));
    let polar_line = smoothstep(1.5, 0.0, polar_circle_dist) * polar_dash * 0.05;
    col = col + vec3<f32>(polar_line);

    let mx = abs(fract(in.world_pos.x / 450.0) - 0.5);
    let m_line = smoothstep(0.496, 0.50, mx) * 0.035;
    col = col + vec3<f32>(m_line * 0.7, m_line * 0.8, m_line * 1.0);

    let pm_dist = abs(wrapped_x);
    let pm_line = smoothstep(2.0, 0.0, pm_dist) * 0.10;
    col = col + vec3<f32>(pm_line * 0.75, pm_line * 0.75, pm_line * 0.6);

    let ice_fade = smoothstep(2200.0, 2600.0, abs(in.world_pos.y));
    col = mix(col, vec3<f32>(0.14, 0.18, 0.23), ice_fade * 0.75);

    return vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) province_id: u32,
    @location(2) is_border: u32,
    @location(3) edge_distance: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) is_border: u32,
    @location(2) edge_distance: f32,
    @location(3) world_pos: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput, @builtin(instance_index) instance_idx: u32) -> VertexOutput {
    let world_offsets = array<f32, 3>(-5400.0, 0.0, 5400.0);
    let offset_x = world_offsets[instance_idx];
    let world_pos = vec2<f32>(in.position.x + offset_x, in.position.y);

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    out.is_border = in.is_border;
    out.edge_distance = in.edge_distance;
    out.world_pos = world_pos;

    if (in.province_id < arrayLength(&province_colors)) {
        out.color = province_colors[in.province_id];
    } else {
        out.color = vec4<f32>(0.26, 0.28, 0.30, 1.0);
    }

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (in.is_border == 1u) {
        return vec4<f32>(0.06, 0.07, 0.09, 0.95);
    }

    if (in.is_border == 2u) {
        let alpha = 0.45 * camera.internal_border_alpha;
        if (alpha <= 0.01) {
            discard;
        }
        return vec4<f32>(0.12, 0.14, 0.18, alpha);
    }

    if (in.is_border == 3u) {
        let alpha = pow(clamp(in.edge_distance, 0.0, 1.0), 1.8) * 0.40;
        let highlight = in.color.rgb * 1.30 + vec3<f32>(0.05);
        return vec4<f32>(clamp(highlight, vec3<f32>(0.0), vec3<f32>(1.0)), alpha);
    }

    if (in.is_border == 4u) {
        return vec4<f32>(1.0, 0.88, 0.22, 1.0);
    }

    if (in.is_border == 5u) {
        return vec4<f32>(0.05, 0.08, 0.12, 0.85);
    }

    let luma = dot(in.color.rgb, vec3<f32>(0.299, 0.587, 0.114));
    let paper_tint = vec3<f32>(luma * 0.95, luma * 0.93, luma * 0.88);
    var base_rgb = mix(in.color.rgb, paper_tint, 0.20);

    let uv_macro = in.world_pos * 0.005;
    let uv_micro = in.world_pos * 0.015;
    let h_center = fbm(uv_macro) * 0.7 + fbm(uv_micro) * 0.3;
    let h_offset = fbm(uv_macro + vec2<f32>(0.04, -0.04)) * 0.7 + fbm(uv_micro + vec2<f32>(0.04, -0.04)) * 0.3;
    let slope = (h_center - h_offset) * 1.6;

    let macro_var = (h_center - 0.5) * 0.06;
    let grain = (hash2(floor(in.world_pos * 0.8)) - 0.5) * 0.012;

    let lighting = 1.0 + slope * 0.22 + macro_var + grain;
    var final_rgb = base_rgb * lighting;

    let mx = abs(fract(in.world_pos.x / 450.0) - 0.5);
    let m_line = smoothstep(0.496, 0.50, mx) * 0.035;
    final_rgb = final_rgb - vec3<f32>(m_line);

    let eq_line = smoothstep(1.8, 0.0, abs(in.world_pos.y)) * 0.05;
    final_rgb = final_rgb - vec3<f32>(eq_line);

    return vec4<f32>(clamp(final_rgb, vec3<f32>(0.0), vec3<f32>(1.0)), in.color.a);
}

struct Uniforms {
    proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) quad_size: vec2<f32>,
    @location(3) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) quad_size: vec2<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.proj * vec4<f32>(in.position, 0.0, 1.0);
    out.color = in.color;
    out.quad_size = in.quad_size;
    out.uv = in.uv;
    return out;
}

const TARGET_THICKNESS: f32 = 2.0;
const FILL_ALPHA_MULT: f32 = 0.15;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel_pos = in.uv * in.quad_size;

    let dist_to_left   = pixel_pos.x;
    let dist_to_right  = in.quad_size.x - pixel_pos.x;
    let dist_to_top    = pixel_pos.y;
    let dist_to_bottom = in.quad_size.y - pixel_pos.y;

    let min_dist = min(min(dist_to_left, dist_to_right), min(dist_to_top, dist_to_bottom));

    var final_color = in.color;
    if (min_dist < TARGET_THICKNESS) {
        final_color.a = in.color.a; 
    } else {
        final_color.a = in.color.a * FILL_ALPHA_MULT; 
    }

    return final_color;
}
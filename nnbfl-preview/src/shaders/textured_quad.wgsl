struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) tint: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) tint: vec4<f32>,
}

struct MaterialUniforms {
    tev_mode: u32,
    source_a: u32,
    source_b: u32,
    source_c: u32,

    color_op: u32,
    alpha_op: u32,
    has_indirect: u32,
    indirect_scale_x: f32,
    indirect_scale_y: f32,

    constant_color0: vec4<f32>,
    constant_color1: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u_projection: mat4x4<f32>;
@group(1) @binding(0) var t_texture0: texture_2d<f32>;
@group(1) @binding(1) var s_sampler0: sampler;
@group(1) @binding(2) var t_texture1: texture_2d<f32>;
@group(1) @binding(3) var<uniform> u_material: MaterialUniforms;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = u_projection * vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.uv;
    out.tint = in.tint;
    return out;
}

fn get_source_color(
    source: u32, 
    tev_mode: u32,
    tex0: vec4<f32>, 
    tex1: vec4<f32>, 
    primary: vec4<f32>, 
    previous: vec4<f32>,
    constant_color: vec4<f32>
) -> vec4<f32> {
    switch (source) {
        case 0u:  { return primary; }
        case 3u:  { return tex0; }
        case 4u:  { return tex1; }
        case 14u: { return constant_color; }
        case 15u: { return previous; }
        default:  { return vec4<f32>(1.0); }
    }
}

fn apply_color_op(op: u32, color: vec4<f32>) -> vec3<f32> {
    switch (op) {
        case 1u:  { return 1.0 - color.rgb; } // InvRGB
        case 2u:  { return vec3<f32>(color.a); } // Alpha
        case 3u:  { return vec3<f32>(1.0 - color.a); } // InvAlpha
        case 4u:  { return vec3<f32>(color.r); } // RRR
        case 5u:  { return vec3<f32>(1.0 - color.r); } // InvRRR
        default:  { return color.rgb; } // RGB
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var final_uv = in.uv;

    if (u_material.has_indirect == 1u) {
        let raw_offset = textureSample(t_texture1, s_sampler0, in.uv);
        let offset_x = raw_offset.r - 0.5;
        let offset_y = raw_offset.g - 0.5;
        
        final_uv.x += offset_x * u_material.indirect_scale_x;
        final_uv.y += offset_y * u_material.indirect_scale_y;
    }

    let raw_tex0 = textureSample(t_texture0, s_sampler0, final_uv);
    let tex1_color = textureSample(t_texture1, s_sampler0, in.uv);
    
    var tex0_color = raw_tex0;
    
    if (u_material.tev_mode == 11u) {
        tex0_color = vec4<f32>(
            u_material.constant_color1.rgb * raw_tex0.r, 
            raw_tex0.r * u_material.constant_color1.a
        );
    }

    let fallback_prev = tex0_color * in.tint;

    var src_a = get_source_color(u_material.source_a, u_material.tev_mode, tex0_color, tex1_color, in.tint, fallback_prev, u_material.constant_color1);
    var src_b = get_source_color(u_material.source_b, u_material.tev_mode, tex0_color, tex1_color, in.tint, fallback_prev, u_material.constant_color1);
    var src_c = get_source_color(u_material.source_c, u_material.tev_mode, tex0_color, tex1_color, in.tint, fallback_prev, u_material.constant_color0);

    let color_a = apply_color_op(u_material.color_op, src_a);
    let color_b = apply_color_op(u_material.color_op, src_b);
    let color_c = apply_color_op(u_material.color_op, src_c);

    var out_rgb = vec3<f32>(0.0);
    
    switch (u_material.tev_mode) {
        case 0u: { // Replace
            out_rgb = color_a;
        }
        case 1u: { // Modulate
            out_rgb = color_a * color_b;
        }
        case 4u: { // Interpolate
            out_rgb = mix(color_a, color_b, color_c);
        }
        case 11u: { // Indirect
            out_rgb = color_a;
        }
        default: {
            out_rgb = tex0_color.rgb * in.tint.rgb;
        }
    }

    let out_alpha = tex0_color.a * in.tint.a;
    return vec4<f32>(out_rgb, out_alpha);
}
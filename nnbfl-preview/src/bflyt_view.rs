use std::{collections::HashMap, path::Path};

use nnbfl::{
    bflyt::{
        file::{Bflyt, BflytNode, BflytSection},
        flags::{BflytOrigin, BflytParentOrigin, TexFilter, TexWrapMode},
        list::TevSource,
        pane::BflytPane,
    },
    core::ReadWriteable,
    sarc::file::Sarc,
};

use crate::renderer::textured_quad::TexturedQuad;
use crate::renderer::{quad::Quad, textured_quad::MaterialUniforms};

#[derive(Debug, Clone)]
pub struct PaneInfo {
    pub label: String,
    pub kind: String,
    pub section: BflytSection,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub depth: usize,
    /// Which top-level PartsPane spawned this (None = root layout pane)
    pub parts_source: Option<String>,
}

pub struct BflytView {
    pub quads: Vec<Quad>,
    pub textured_quads: Vec<TexturedQuad>,
    pub panes: Vec<PaneInfo>,
    pub layout_width: f32,
    pub layout_height: f32,

    pub discovered_bntx_buffers: Vec<Vec<u8>>,
}

fn section_color(section: &BflytSection) -> [f32; 4] {
    match section {
        BflytSection::Pane(_) => [0.55, 0.76, 0.98, 0.55],
        BflytSection::PicturePane(_) => [0.40, 0.85, 0.55, 0.55],
        BflytSection::TextBoxPane(_) => [0.98, 0.82, 0.35, 0.55],
        BflytSection::WindowPane(_) => [0.85, 0.45, 0.85, 0.55],
        BflytSection::PartsPane(_) => [0.98, 0.55, 0.35, 0.55],
        BflytSection::AlignmentPane(_) => [0.35, 0.90, 0.90, 0.55],
        BflytSection::CapturePane(_) => [0.90, 0.35, 0.50, 0.55],
        BflytSection::BoundingPane(_) => [0.75, 0.75, 0.75, 0.30],
        BflytSection::ScissorPane(_) => [0.95, 0.95, 0.35, 0.40],
        _ => [0.60, 0.60, 0.60, 0.30],
    }
}

fn section_kind_name(section: &BflytSection) -> &'static str {
    match section {
        BflytSection::UserData(_) => "UserData",
        BflytSection::Layout(_) => "Layout",
        BflytSection::TextureList(_) => "TextureList",
        BflytSection::FontList(_) => "FontList",
        BflytSection::MaterialList(_) => "MaterialList",
        BflytSection::CaptureTextureList(_) => "CaptureTextureList",
        BflytSection::VectorGraphicsList(_) => "VectorGraphicsList",
        BflytSection::Pane(_) => "Pane",
        BflytSection::PicturePane(_) => "PicturePane",
        BflytSection::TextBoxPane(_) => "TextBoxPane",
        BflytSection::WindowPane(_) => "WindowPane",
        BflytSection::PartsPane(_) => "PartsPane",
        BflytSection::AlignmentPane(_) => "AlignmentPane",
        BflytSection::CapturePane(_) => "CapturePane",
        BflytSection::BoundingPane(_) => "BoundingPane",
        BflytSection::ScissorPane(_) => "ScissorPane",
        BflytSection::Group(_) => "Group",
        BflytSection::ControlSource(_) => "ControlSource",
        BflytSection::PaneStart => "PaneStart",
        BflytSection::PaneEnd => "PaneEnd",
        BflytSection::GroupStart => "GroupStart",
        BflytSection::GroupEnd => "GroupEnd",
        BflytSection::Unknown(_, _) => "Unknown",
    }
}

fn base_pane(section: &BflytSection) -> Option<&BflytPane> {
    match section {
        BflytSection::Pane(p) | BflytSection::BoundingPane(p) | BflytSection::ScissorPane(p) => {
            Some(p)
        }
        BflytSection::PicturePane(p) => Some(&p.base),
        BflytSection::TextBoxPane(p) => Some(&p.base),
        BflytSection::WindowPane(p) => Some(&p.base),
        BflytSection::PartsPane(p) => Some(&p.base),
        BflytSection::AlignmentPane(p) => Some(&p.base),
        BflytSection::CapturePane(p) => Some(&p.base),
        _ => None,
    }
}

fn pane_name(section: &BflytSection) -> String {
    if let Some(p) = base_pane(section) {
        let name = p.pane_name.trim_end_matches('\0');
        if !name.is_empty() {
            return name.to_string();
        }
    }
    section_kind_name(section).to_string()
}

fn resolve_rect(
    pane: &BflytPane,
    parent_x: f32,
    parent_y: f32,
    parent_w: f32,
    parent_h: f32,
) -> (f32, f32, f32, f32) {
    let anchor_x = match pane.origin.parent_origin_x {
        BflytParentOrigin::None => parent_x + parent_w * 0.5,
        BflytParentOrigin::LeftTop => parent_x,
        BflytParentOrigin::RightBottom => parent_x + parent_w,
    };
    let anchor_y = match pane.origin.parent_origin_y {
        BflytParentOrigin::None => parent_y + parent_h * 0.5,
        BflytParentOrigin::LeftTop => parent_y,
        BflytParentOrigin::RightBottom => parent_y + parent_h,
    };

    let cx = anchor_x + pane.translation.x;
    let cy = anchor_y + pane.translation.y;

    let w = pane.size.x * pane.scale.x;
    let h = pane.size.y * pane.scale.y;

    let tl_x = match pane.origin.origin_x {
        BflytOrigin::Center => cx - w * 0.5,
        BflytOrigin::LeftTop => cx,
        BflytOrigin::RightBottom => cx - w,
    };
    let tl_y = match pane.origin.origin_y {
        BflytOrigin::Center => cy - h * 0.5,
        BflytOrigin::LeftTop => cy,
        BflytOrigin::RightBottom => cy - h,
    };

    (tl_x, tl_y, w.abs().max(1.0), h.abs().max(1.0))
}

fn resolve_rect_in_parts(
    pane: &BflytPane,
    parent_x: f32,
    parent_y: f32,
    parent_w: f32,
    parent_h: f32,
    parts_center_x: f32,
    parts_center_y: f32,
    parts_scale_x: f32,
    parts_scale_y: f32,
) -> (f32, f32, f32, f32) {
    let (lx, ly, lw, lh) = resolve_rect(pane, parent_x, parent_y, parent_w, parent_h);
    let x = parts_center_x + lx * parts_scale_x;
    let y = parts_center_y + ly * parts_scale_y;
    let w = (lw * parts_scale_x).abs().max(1.0);
    let h = (lh * parts_scale_y).abs().max(1.0);
    (x, y, w, h)
}

pub struct ResolvedBlarc {
    pub bflyt_bytes: Vec<u8>,
    pub bntx_bytes: Option<Vec<u8>>,
}

pub fn load_bflyt_from_blarc_dir(blarc_dir: &Path, layout_name: &str) -> Option<ResolvedBlarc> {
    let entry = std::fs::read_dir(blarc_dir).ok()?.find_map(|e| {
        let e = e.ok()?;
        let fname = e.file_name();
        let fname = fname.to_string_lossy();
        if fname.starts_with(layout_name) && (fname.ends_with(".blarc") || fname.ends_with(".sarc"))
        {
            Some(e.path())
        } else {
            None
        }
    })?;

    let bytes = std::fs::read(&entry).ok()?;
    let sarc = Sarc::parse(&bytes).ok()?;

    let mut bflyt_bytes = None;
    let mut bntx_bytes = None;

    for file in sarc.files {
        if let Some(name) = &file.name {
            if name.ends_with(".bflyt") {
                bflyt_bytes = Some(file.data);
            } else if name.ends_with(".bntx") || name.contains("__Combined") {
                bntx_bytes = Some(file.data);
            }
        }
    }

    Some(ResolvedBlarc {
        bflyt_bytes: bflyt_bytes?,
        bntx_bytes,
    })
}

struct Walker<'a> {
    layout_w: f32,
    layout_h: f32,
    quads: &'a mut Vec<Quad>,
    textured_quads: &'a mut Vec<TexturedQuad>,
    panes: &'a mut Vec<PaneInfo>,
    material_list: Option<&'a nnbfl::bflyt::list::BflytMaterialList>,
    texture_list: Option<&'a nnbfl::bflyt::list::BflytTextureList>,
    blarc_dir: Option<&'a Path>,
    blarc_cache: &'a mut HashMap<String, Option<Bflyt>>,
    parts_depth: usize,
    parts_source: Option<String>,

    discovered_bntx_buffers: Vec<Vec<u8>>,
}

const MAX_PARTS_DEPTH: usize = 8;

impl<'a> Walker<'a> {
    fn walk_nodes(
        &mut self,
        nodes: &[BflytNode],
        parent_x: f32,
        parent_y: f32,
        parent_w: f32,
        parent_h: f32,
        depth: usize,
    ) {
        for node in nodes {
            self.walk_node(node, parent_x, parent_y, parent_w, parent_h, depth);
        }
    }

    fn walk_node(
        &mut self,
        node: &BflytNode,
        parent_x: f32,
        parent_y: f32,
        parent_w: f32,
        parent_h: f32,
        depth: usize,
    ) {
        match node {
            BflytNode::Section(section) => {
                let Some(base) = base_pane(section) else {
                    return;
                };

                let (x, y, w, h) = resolve_rect(base, parent_x, parent_y, parent_w, parent_h);

                let label = pane_name(section);
                let kind = section_kind_name(section).to_string();

                self.quads.push(Quad {
                    x,
                    y,
                    width: w,
                    height: h,
                    color: section_color(section),
                });

                if let BflytSection::PicturePane(pic) = section {
                    if let Some(tq) = self.make_textured_quad(pic, x, y, w, h) {
                        self.textured_quads.push(tq);
                    }
                }

                self.panes.push(PaneInfo {
                    label: label.clone(),
                    kind,
                    x,
                    y,
                    section: section.clone(),
                    width: w,
                    height: h,
                    depth,
                    parts_source: self.parts_source.clone(),
                });

                if let BflytSection::PartsPane(parts) = section {
                    self.maybe_resolve_parts(parts, x, y, w, h, depth);
                }
            }

            BflytNode::Panes(children) => {
                self.walk_nodes(children, parent_x, parent_y, parent_w, parent_h, depth + 1);
            }

            BflytNode::Groups(_) => {}
        }
    }

    fn make_textured_quad(
        &self,
        pic: &nnbfl::bflyt::pane::BflytPicturePane,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) -> Option<TexturedQuad> {
        let mat_list = self.material_list?;
        let mat = mat_list.materials.get(pic.material_index as usize)?;
        let tex_map = mat.tex_maps.first()?;
        let tex_name = tex_map.texture_name.trim_end();

        if tex_name.is_empty() {
            return None;
        }

        let mut tint = [1.0, 1.0, 1.0, 1.0];

        if let Some(color_entry) = mat.colors.first() {
            if let Some(c8) = &color_entry.color_u8 {
                tint = [
                    c8.r as f32 / 255.0,
                    c8.g as f32 / 255.0,
                    c8.b as f32 / 255.0,
                    c8.a as f32 / 255.0,
                ];
            } else if let Some(cf) = &color_entry.color_f32 {
                tint = [cf.r, cf.g, cf.b, cf.a];
            }

            if tint[3] == 0.0 {
                tint = [1.0, 1.0, 1.0, 1.0];
            }
        }

        let mut uvs = if let Some(uv_set) = pic.texture_uvs.first() {
            [
                [uv_set.top_left.x, uv_set.top_left.y],
                [uv_set.top_right.x, uv_set.top_right.y],
                [uv_set.bottom_left.x, uv_set.bottom_left.y],
                [uv_set.bottom_right.x, uv_set.bottom_right.y],
            ]
        } else {
            [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]
        };

        if let Some(srt) = mat.tex_srts.first() {
            for uv in uvs.iter_mut() {
                uv[0] = uv[0] * srt.scale_x + srt.translation_x;
                uv[1] = uv[1] * srt.scale_z + srt.translation_y;
            }
        }

        let address_mode_u = match tex_map.u_options.wrap_mode {
            TexWrapMode::Repeat => wgpu::AddressMode::Repeat,
            TexWrapMode::Mirror => wgpu::AddressMode::MirrorRepeat,
            TexWrapMode::Clamp => wgpu::AddressMode::ClampToEdge,
        };

        let address_mode_v = match tex_map.v_options.wrap_mode {
            TexWrapMode::Repeat => wgpu::AddressMode::Repeat,
            TexWrapMode::Mirror => wgpu::AddressMode::MirrorRepeat,
            TexWrapMode::Clamp => wgpu::AddressMode::ClampToEdge,
        };

        let min_filter = match tex_map.u_options.filter {
            TexFilter::Linear => wgpu::FilterMode::Linear,
            TexFilter::Near => wgpu::FilterMode::Nearest,
        };

        let mag_filter = match tex_map.v_options.filter {
            TexFilter::Linear => wgpu::FilterMode::Linear,
            TexFilter::Near => wgpu::FilterMode::Nearest,
        };

        let (tev_mode, source_a, source_b, source_c, color_op, alpha_op) =
            if let Some(detailed) = &mat.detailed_combiner {
                if let Some(stage) = detailed.entries.first() {
                    (
                        stage.color_config.mode as u32,
                        stage.color_config.sources[0] as u32,
                        stage.color_config.sources[1] as u32,
                        stage.color_config.sources[2] as u32,
                        stage.color_config.operands[0] as u32,
                        stage.alpha_config.operands[0] as u32,
                    )
                } else {
                    (0, 3, 0, 14, 0, 0)
                }
            } else if let Some(tev) = mat.tev_combiners.first() {
                (
                    tev.rgb_mode as u32,
                    TevSource::Texture0 as u32,
                    TevSource::Primary as u32,
                    TevSource::Constant as u32,
                    0,
                    0,
                )
            } else {
                (0, 3, 0, 14, 0, 0)
            };

        let (has_indirect, indirect_scale_x, indirect_scale_y) =
            if let Some(matrix) = &mat.indirect_matrix {
                (1, matrix.scale.x, matrix.scale.y)
            } else {
                (0, 0.0, 0.0)
            };

        let constant_color0 = mat
            .colors
            .get(0)
            .and_then(|c| c.color_u8.as_ref())
            .map(|c| {
                [
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                    c.a as f32 / 255.0,
                ]
            })
            .unwrap_or([0.0, 0.0, 0.0, 0.0]);

        let constant_color1 = mat
            .colors
            .get(1)
            .and_then(|c| c.color_u8.as_ref())
            .map(|c| {
                [
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                    c.a as f32 / 255.0,
                ]
            })
            .unwrap_or([1.0, 1.0, 1.0, 1.0]);

        Some(TexturedQuad {
            x,
            y,
            width: w,
            height: h,
            uvs,
            tint,
            texture_name: tex_name.to_string(),
            secondary_texture_name: mat
                .tex_maps
                .get(1)
                .map(|m| m.texture_name.trim_end().to_string()),
            address_mode_u,
            address_mode_v,
            min_filter,
            mag_filter,
            material_uniforms: MaterialUniforms {
                tev_mode,
                source_a,
                source_b,
                source_c,
                color_op,
                alpha_op,
                has_indirect,
                _padding: 0,
                indirect_scale_x,
                indirect_scale_y,
                _padding2: [0.0; 2],
                constant_color0,
                constant_color1,
            },
        })
    }

    fn maybe_resolve_parts(
        &mut self,
        parts: &nnbfl::bflyt::pane::BflytPartsPane,
        parts_x: f32,
        parts_y: f32,
        parts_w: f32,
        parts_h: f32,
        depth: usize,
    ) {
        if self.parts_depth >= MAX_PARTS_DEPTH {
            return;
        }
        let Some(blarc_dir) = self.blarc_dir else {
            return;
        };
        let layout_name = parts.o_layout_name.trim_end_matches('\0');
        if layout_name.is_empty() {
            return;
        }

        if !self.blarc_cache.contains_key(layout_name) {
            if let Some(assets) = load_bflyt_from_blarc_dir(blarc_dir, layout_name) {
                if let Ok(sub_bflyt) = Bflyt::parse(&assets.bflyt_bytes) {
                    if let Some(bntx_data) = assets.bntx_bytes {
                        self.discovered_bntx_buffers.push(bntx_data);
                    }

                    self.blarc_cache
                        .insert(layout_name.to_string(), Some(sub_bflyt));
                }
            }
        }

        let Some(sub_bflyt) = self.blarc_cache[layout_name].as_ref() else {
            log::warn!("PartsPane: could not load '{layout_name}'");
            return;
        };

        let overrides: HashMap<String, &BflytSection> = parts
            .properties
            .iter()
            .filter_map(|prop| {
                let name = prop.property_name.trim_end_matches('\0');
                if name.is_empty() {
                    return None;
                }
                prop.o_section.as_ref().map(|s| (name.to_string(), s))
            })
            .collect();

        let scale_x = parts.base.scale.x * parts.magnify_x;
        let scale_y = parts.base.scale.y * parts.magnify_y;

        let center_x = parts_x + parts_w * 0.5;
        let center_y = parts_y + parts_h * 0.5;

        let sub_w = sub_bflyt.layout.width;
        let sub_h = sub_bflyt.layout.height;

        let sub_parent_x = -sub_w * 0.5;
        let sub_parent_y = -sub_h * 0.5;

        let parts_source_label = parts.base.pane_name.trim_end_matches('\0').to_string();

        self.walk_nodes_in_parts(
            &sub_bflyt.nodes.clone(),
            sub_parent_x,
            sub_parent_y,
            sub_w,
            sub_h,
            center_x,
            center_y,
            scale_x,
            scale_y,
            depth + 1,
            &overrides,
            &parts_source_label,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_nodes_in_parts(
        &mut self,
        nodes: &[BflytNode],
        parent_x: f32,
        parent_y: f32,
        parent_w: f32,
        parent_h: f32,
        parts_origin_x: f32,
        parts_origin_y: f32,
        parts_scale_x: f32,
        parts_scale_y: f32,
        depth: usize,
        overrides: &HashMap<String, &BflytSection>,
        parts_source: &str,
    ) {
        for node in nodes {
            self.walk_node_in_parts(
                node,
                parent_x,
                parent_y,
                parent_w,
                parent_h,
                parts_origin_x,
                parts_origin_y,
                parts_scale_x,
                parts_scale_y,
                depth,
                overrides,
                parts_source,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_node_in_parts(
        &mut self,
        node: &BflytNode,
        parent_x: f32,
        parent_y: f32,
        parent_w: f32,
        parent_h: f32,
        parts_origin_x: f32,
        parts_origin_y: f32,
        parts_scale_x: f32,
        parts_scale_y: f32,
        depth: usize,
        overrides: &HashMap<String, &BflytSection>,
        parts_source: &str,
    ) {
        match node {
            BflytNode::Section(section) => {
                let Some(base) = base_pane(section) else {
                    return;
                };

                let pname = pane_name(section);

                let effective_section: &BflytSection =
                    overrides.get(&pname).copied().unwrap_or(section);
                let effective_base = base_pane(effective_section).unwrap_or(base);

                let (x, y, w, h) = resolve_rect_in_parts(
                    effective_base,
                    parent_x,
                    parent_y,
                    parent_w,
                    parent_h,
                    parts_origin_x,
                    parts_origin_y,
                    parts_scale_x,
                    parts_scale_y,
                );

                self.quads.push(Quad {
                    x,
                    y,
                    width: w,
                    height: h,
                    color: section_color(effective_section),
                });

                if let BflytSection::PicturePane(pic) = effective_section {
                    if let Some(tq) = self.make_textured_quad(pic, x, y, w, h) {
                        self.textured_quads.push(tq);
                    }
                }

                self.panes.push(PaneInfo {
                    label: pname,
                    kind: section_kind_name(effective_section).to_string(),
                    x,
                    y,
                    width: w,
                    height: h,
                    section: effective_section.clone(),
                    depth,
                    parts_source: Some(parts_source.to_string()),
                });

                if let BflytSection::PartsPane(nested_parts) = effective_section {
                    self.parts_depth += 1;
                    self.maybe_resolve_parts(nested_parts, x, y, w, h, depth);
                    self.parts_depth -= 1;
                }
            }

            BflytNode::Panes(children) => {
                self.walk_nodes_in_parts(
                    children,
                    parent_x,
                    parent_y,
                    parent_w,
                    parent_h,
                    parts_origin_x,
                    parts_origin_y,
                    parts_scale_x,
                    parts_scale_y,
                    depth + 1,
                    overrides,
                    parts_source,
                );
            }

            BflytNode::Groups(_) => {}
        }
    }
}

pub fn build_view(file: &Bflyt, blarc_dir: Option<&Path>) -> BflytView {
    let layout_w = file.layout.width;
    let layout_h = file.layout.height;

    let mut quads = Vec::new();
    let mut textured_quads = Vec::new();
    let mut panes = Vec::new();
    let mut blarc_cache: HashMap<String, Option<Bflyt>> = HashMap::new();

    let mut walker = Walker {
        layout_w,
        layout_h,
        quads: &mut quads,
        textured_quads: &mut textured_quads,
        panes: &mut panes,
        blarc_dir,
        blarc_cache: &mut blarc_cache,
        parts_depth: 0,
        parts_source: None,
        material_list: file.material_list.as_ref(),
        texture_list: file.texture_list.as_ref(),
        discovered_bntx_buffers: Vec::new(),
    };

    walker.walk_nodes(&file.nodes, 0.0, 0.0, layout_w, layout_h, 0);
    let discovered_bntx_buffers = walker.discovered_bntx_buffers;

    BflytView {
        quads,
        textured_quads,
        panes,
        layout_width: layout_w,
        layout_height: layout_h,
        discovered_bntx_buffers,
    }
}

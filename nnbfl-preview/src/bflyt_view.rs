use std::{collections::HashMap, path::Path};

use nnbfl::{
    bflyt::{
        file::{Bflyt, BflytNode, BflytSection},
        flags::{BflytOrigin, BflytParentOrigin, TexFilter, TexWrapMode},
        list::{BflytMaterialList, MaterialColorEntry, TexGenSrc},
        pane::{BflytPane, BflytPicturePane, Color4u8},
    },
    core::ReadWriteable,
};

use crate::{decompress_if_needed, extract_all_files_recursive, renderer::quad::Quad};
use crate::{
    renderer::textured_quad::{DetailedCombinerMaterial, StandardMaterial, TexturedQuad},
    ui::SUPPORTED_SARC_EXTENSIONS,
};

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
    pub parent_idx: Option<usize>,

    pub parent_anchor_x: f32,
    pub parent_anchor_y: f32,
    pub parts_source: Option<String>,
    pub visible: bool,
}

pub struct BflytView {
    pub quads: Vec<Quad>,
    pub textured_quads: Vec<TexturedQuad>,
    pub material_list: Option<BflytMaterialList>,
    pub panes: Vec<PaneInfo>,
    pub layout_width: f32,
    pub layout_height: f32,
    pub children: Vec<Vec<usize>>,

    pub discovered_bntx_buffers: Vec<Vec<u8>>,
    pub base_panes: Vec<PaneInfo>,
    pub base_textured_quads: Vec<TexturedQuad>,
    pub file_name: String,
}

impl BflytView {
    pub fn reset_to_base(&mut self) {
        self.panes = self.base_panes.clone();
        self.textured_quads = self.base_textured_quads.clone();
        for (i, p) in self.base_panes.iter().enumerate() {
            if let Some(q) = self.quads.get_mut(i) {
                q.x = p.x;
                q.y = p.y;
                q.color = if p.visible {
                    section_color(&p.section)
                } else {
                    [0.0; 4]
                };
            }
        }
    }

    pub fn descendants(&self, pane_idx: usize) -> Vec<usize> {
        let mut result = Vec::new();
        let mut stack = vec![pane_idx];
        while let Some(idx) = stack.pop() {
            if let Some(kids) = self.children.get(idx) {
                for &child in kids {
                    result.push(child);
                    stack.push(child);
                }
            }
        }
        result
    }
}

pub fn section_color_for_pane(pane_idx: usize, view: &BflytView) -> [f32; 4] {
    view.panes
        .get(pane_idx)
        .map(|p| section_color(&p.section))
        .unwrap_or([0.0; 4])
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
        BflytSection::CapturePane(p) => Some(p),
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

/// Compute the four world-space corner positions [TL, TR, BL, BR] for a pane
/// after applying Z rotation around the pane's pivot point (cx, cy).
/// Matches Nintendo's MakeMatrixSrt Z-only path.
pub fn compute_corners(
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
    origin_x: &BflytOrigin,
    origin_y: &BflytOrigin,
    rotate_z: f32,
) -> [[f32; 2]; 4] {
    let lx = match origin_x {
        BflytOrigin::Center => -w * 0.5,
        BflytOrigin::LeftTop => 0.0,
        BflytOrigin::RightBottom => -w,
    };
    let ly = match origin_y {
        BflytOrigin::Center => -h * 0.5,
        BflytOrigin::LeftTop => 0.0,
        BflytOrigin::RightBottom => -h,
    };

    let local = [
        [lx, ly],         // TL
        [lx + w, ly],     // TR
        [lx, ly + h],     // BL
        [lx + w, ly + h], // BR
    ];

    if rotate_z == 0.0 {
        return local.map(|[x, y]| [cx + x, cy + y]);
    }

    let rad = -rotate_z.to_radians();
    let (sin_r, cos_r) = rad.sin_cos();
    local.map(|[x, y]| [cx + x * cos_r - y * sin_r, cy + x * sin_r + y * cos_r])
}

fn resolve_rect(
    pane: &BflytPane,
    parent_x: f32,
    parent_y: f32,
    parent_w: f32,
    parent_h: f32,
) -> (f32, f32, f32, f32, f32, f32, f32, f32) {
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
    let cy = anchor_y - pane.translation.y;

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

    (
        tl_x,
        tl_y,
        w.abs().max(1.0),
        h.abs().max(1.0),
        anchor_x,
        anchor_y,
        cx,
        cy,
    )
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
    let (lx, ly, lw, lh, _, _, _, _) = resolve_rect(pane, parent_x, parent_y, parent_w, parent_h);

    let x = parts_center_x + lx * parts_scale_x;
    let y = parts_center_y + ly * parts_scale_y;
    let w = (lw * parts_scale_x).abs().max(1.0);
    let h = (lh * parts_scale_y).abs().max(1.0);
    (x, y, w, h)
}

pub struct ResolvedBlarc {
    pub bflyt_bytes: Vec<u8>,
    pub bntx_bytes: Option<Vec<u8>>,
    pub bflan_bytes: Vec<(String, Vec<u8>)>,
}

pub fn load_bflyt_from_blarc_dir(blarc_dir: &Path, layout_name: &str) -> Option<ResolvedBlarc> {
    let entry_path = std::fs::read_dir(blarc_dir).ok()?.find_map(|e| {
        let e = e.ok()?;
        let path = e.path();
        let fname = path.file_name()?.to_string_lossy().to_lowercase();

        if !fname.starts_with(&layout_name.to_lowercase()) {
            return None;
        }

        let is_valid_sarc = SUPPORTED_SARC_EXTENSIONS
            .iter()
            .any(|ext| fname.ends_with(&format!(".{}", ext.to_lowercase())));

        if is_valid_sarc { Some(path) } else { None }
    })?;

    let mut bytes = std::fs::read(&entry_path).ok()?;
    let filename = entry_path.file_name()?.to_string_lossy();

    bytes = decompress_if_needed(bytes, &filename);

    let all_files = extract_all_files_recursive(&bytes);

    let mut resolved = ResolvedBlarc {
        bflyt_bytes: Vec::new(),
        bntx_bytes: None,
        bflan_bytes: Vec::new(),
    };

    for (name, data) in &all_files {
        let name_lower = name.to_lowercase();

        if name_lower.ends_with(".bflyt") {
            if resolved.bflyt_bytes.is_empty() {
                resolved.bflyt_bytes = data.clone();
            }
        } else if name_lower.ends_with(".bntx") || name_lower.contains("__combined") {
            if resolved.bntx_bytes.is_none() {
                resolved.bntx_bytes = Some(data.clone());
            }
        } else if name_lower.ends_with(".bflan") {
            let anim_name = std::path::Path::new(name)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| name.clone());

            resolved.bflan_bytes.push((anim_name, data.clone()));
        }
    }

    if resolved.bflyt_bytes.is_empty() {
        return None;
    }

    Some(resolved)
}

struct Walker<'a> {
    quads: &'a mut Vec<Quad>,
    textured_quads: &'a mut Vec<TexturedQuad>,
    panes: &'a mut Vec<PaneInfo>,
    children: &'a mut Vec<Vec<usize>>,
    material_list: Option<&'a BflytMaterialList>,
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
        parent_visible: bool,
        parent_idx: Option<usize>,
    ) {
        let mut last_rect = (parent_x, parent_y, parent_w, parent_h);
        let mut last_visible = parent_visible;
        let mut last_pane_idx = parent_idx;

        for node in nodes {
            match node {
                BflytNode::Section(section) => {
                    self.walk_node(
                        node,
                        parent_x,
                        parent_y,
                        parent_w,
                        parent_h,
                        depth,
                        parent_visible,
                        parent_idx,
                    );
                    if let Some(p) = self.panes.last() {
                        last_rect = (p.x, p.y, p.width, p.height);
                        last_pane_idx = Some(self.panes.len() - 1);
                        if let Some(base) = base_pane(section) {
                            last_visible = parent_visible && base.pane_flags.is_visible;
                        }
                    }
                }
                BflytNode::Panes(children) => {
                    let (px, py, pw, ph) = last_rect;
                    self.walk_nodes(
                        children,
                        px,
                        py,
                        pw,
                        ph,
                        depth + 1,
                        last_visible,
                        last_pane_idx,
                    );
                }
                BflytNode::Groups(_) => {}
            }
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
        parent_visible: bool,
        parent_idx: Option<usize>,
    ) {
        match node {
            BflytNode::Section(section) => {
                let Some(base) = base_pane(section) else {
                    return;
                };

                let is_visible = parent_visible && base.pane_flags.is_visible;
                let (x, y, w, h, anchor_x, anchor_y, cx, cy) =
                    resolve_rect(base, parent_x, parent_y, parent_w, parent_h);
                let label = pane_name(section);
                let kind = section_kind_name(section).to_string();
                let pane_idx = self.panes.len();

                while self.children.len() <= pane_idx {
                    self.children.push(Vec::new());
                }
                if let Some(pidx) = parent_idx {
                    while self.children.len() <= pidx {
                        self.children.push(Vec::new());
                    }
                    self.children[pidx].push(pane_idx);
                }

                let mut has_textured = false;
                if let BflytSection::PicturePane(pic) = section
                    && let Some(mat_list) = self.material_list
                    && let Some(tq) = self.make_textured_quad(
                        mat_list,
                        pic,
                        x,
                        y,
                        w,
                        h,
                        cx,
                        cy,
                        base.rotation.z,
                        pane_idx,
                        is_visible,
                    )
                {
                    self.textured_quads.push(tq);
                    has_textured = true;
                }

                self.quads.push(Quad {
                    x,
                    y,
                    width: w,
                    height: h,
                    color: if is_visible {
                        section_color(section)
                    } else {
                        [0.0; 4]
                    },
                    has_textured,
                });

                self.panes.push(PaneInfo {
                    label: label.clone(),
                    kind,
                    x,
                    y,
                    section: section.clone(),
                    width: w,
                    height: h,
                    depth,
                    parent_idx,
                    parent_anchor_x: anchor_x,
                    parent_anchor_y: anchor_y,
                    parts_source: self.parts_source.clone(),
                    visible: is_visible,
                });

                if let BflytSection::PartsPane(parts) = section {
                    self.maybe_resolve_parts(parts, x, y, w, h, depth, is_visible, pane_idx);
                }
            }
            BflytNode::Panes(_) | BflytNode::Groups(_) => {}
        }
    }

    fn make_textured_quad(
        &self,
        mat_list: &BflytMaterialList,
        pic: &BflytPicturePane,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        cx: f32,
        cy: f32,
        rotate_z: f32,
        pane_idx: usize,
        parent_visible: bool,
    ) -> Option<TexturedQuad> {
        let mat = mat_list.materials.get(pic.material_index as usize)?;
        let tex_map = mat.tex_maps.first()?;
        let tex_name = tex_map.texture_name.trim_end();

        if tex_name.is_empty() {
            return None;
        }

        let vertex_color_to_f32 = |c: &nnbfl::bflyt::pane::Color4u8| -> [f32; 4] {
            [
                c.r as f32 / 255.0,
                c.g as f32 / 255.0,
                c.b as f32 / 255.0,
                c.a as f32 / 255.0,
            ]
        };

        let tl = vertex_color_to_f32(&pic.top_left_vertex_color);
        let tint = if parent_visible {
            if tl[3] > 0.0 { tl } else { [1.0; 4] }
        } else {
            [0.0; 4]
        };

        let get_uv_set = |layer: usize| -> [[f32; 2]; 4] {
            if let Some(uv_set) = pic.texture_uvs.get(layer) {
                [
                    [uv_set.top_left.x, uv_set.top_left.y],
                    [uv_set.top_right.x, uv_set.top_right.y],
                    [uv_set.bottom_left.x, uv_set.bottom_left.y],
                    [uv_set.bottom_right.x, uv_set.bottom_right.y],
                ]
            } else {
                [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]
            }
        };

        let uvs0_base = get_uv_set(0);
        let uvs1_base = get_uv_set(1);
        let uvs2_base = get_uv_set(2);
        let base_uvs: [[[f32; 2]; 3]; 4] =
            std::array::from_fn(|i| [uvs0_base[i], uvs1_base[i], uvs2_base[i]]);

        let mut uvs = base_uvs;

        for layer in 0..3 {
            if let Some(srt) = mat.tex_srts.get(layer) {
                for v_idx in 0..4 {
                    uvs[v_idx][layer][0] =
                        base_uvs[v_idx][layer][0] * srt.scale_u + srt.translate_u;
                    uvs[v_idx][layer][1] =
                        base_uvs[v_idx][layer][1] * srt.scale_v + srt.translate_v;
                }
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

        let wrap_to_address = |w: &TexWrapMode| match w {
            TexWrapMode::Repeat => wgpu::AddressMode::Repeat,
            TexWrapMode::Mirror => wgpu::AddressMode::MirrorRepeat,
            TexWrapMode::Clamp => wgpu::AddressMode::ClampToEdge,
        };

        let filter_to_mode = |f: &TexFilter| match f {
            TexFilter::Linear => wgpu::FilterMode::Linear,
            TexFilter::Near => wgpu::FilterMode::Nearest,
        };

        let tex_map1 = mat.tex_maps.get(1);
        let tex_map2 = mat.tex_maps.get(2);

        let address_mode_u1 = tex_map1
            .map(|m| wrap_to_address(&m.u_options.wrap_mode))
            .unwrap_or(wgpu::AddressMode::ClampToEdge);

        let address_mode_v1 = tex_map1
            .map(|m| wrap_to_address(&m.v_options.wrap_mode))
            .unwrap_or(wgpu::AddressMode::ClampToEdge);

        let min_filter1 = tex_map1
            .map(|m| filter_to_mode(&m.u_options.filter))
            .unwrap_or(wgpu::FilterMode::Linear);

        let mag_filter1 = tex_map1
            .map(|m| filter_to_mode(&m.v_options.filter))
            .unwrap_or(wgpu::FilterMode::Linear);

        let address_mode_u2 = tex_map2
            .map(|m| wrap_to_address(&m.u_options.wrap_mode))
            .unwrap_or(wgpu::AddressMode::ClampToEdge);

        let address_mode_v2 = tex_map2
            .map(|m| wrap_to_address(&m.v_options.wrap_mode))
            .unwrap_or(wgpu::AddressMode::ClampToEdge);

        let min_filter2 = tex_map2
            .map(|m| filter_to_mode(&m.u_options.filter))
            .unwrap_or(wgpu::FilterMode::Linear);

        let mag_filter2 = tex_map2
            .map(|m| filter_to_mode(&m.v_options.filter))
            .unwrap_or(wgpu::FilterMode::Linear);

        let texture_name1 = mat
            .tex_maps
            .get(1)
            .map(|m| m.texture_name.trim_end().to_string())
            .filter(|s| !s.is_empty());
        let texture_name2 = mat
            .tex_maps
            .get(2)
            .map(|m| m.texture_name.trim_end().to_string())
            .filter(|s| !s.is_empty());

        let texture_count = mat.tex_maps.len().min(3) as u32;

        let mut tex_gen_flags = [0u32; 3];

        for (flag, coord_gen) in tex_gen_flags
            .iter_mut()
            .zip(mat.tex_coord_gens.iter().take(texture_count as usize))
        {
            *flag = match coord_gen.tex_gen_source {
                TexGenSrc::PaneBasedProjection
                | TexGenSrc::OrthogonalProjection
                | TexGenSrc::PerspectiveProjection
                | TexGenSrc::PaneBasedPerspectiveProjection => 1,
                TexGenSrc::BrickRepeat => 2,
                _ => 0,
            };
        }

        let mut proj_scales = [[1.0f32; 2]; 3];
        let mut proj_translations = [[0.0f32; 2]; 3];

        let mut target_layer = 0;
        for tex_gen in mat.projection_tex_gens.iter().take(texture_count as usize) {
            while target_layer < 3 && tex_gen_flags[target_layer] != 1 {
                target_layer += 1;
            }

            if target_layer >= 3 {
                break;
            }

            proj_scales[target_layer] = [tex_gen.scale.x, tex_gen.scale.y];
            proj_translations[target_layer] = [tex_gen.translation.x, tex_gen.translation.y];

            if tex_gen.flags.fitting_layout_size {
                tex_gen_flags[target_layer] |= 1 << 2;
            }

            if tex_gen.flags.fitting_pane_size {
                tex_gen_flags[target_layer] |= 1 << 3;
            }

            if tex_gen.flags.adjust_projection_scale_rotate {
                tex_gen_flags[target_layer] |= 1 << 4;
            }

            target_layer += 1;
        }

        let tex_gen_mode_packed =
            tex_gen_flags[0] | (tex_gen_flags[1] << 8) | (tex_gen_flags[2] << 16);

        let color_f32 = |entry: &MaterialColorEntry| -> [f32; 4] {
            if let Some(c) = &entry.color_u8 {
                [
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                    c.a as f32 / 255.0,
                ]
            } else if let Some(c) = &entry.color_f32 {
                [c.r, c.g, c.b, c.a]
            } else {
                [0.0; 4]
            }
        };

        let black_color = mat
            .colors
            .first()
            .map(color_f32)
            .unwrap_or([0.0, 0.0, 0.0, 0.0]);
        let white_color = mat
            .colors
            .get(1)
            .map(color_f32)
            .unwrap_or([1.0, 1.0, 1.0, 1.0]);

        let interpolate_offset = black_color;
        let interpolate_width = [
            white_color[0] - black_color[0],
            white_color[1] - black_color[1],
            white_color[2] - black_color[2],
            white_color[3] - black_color[3],
        ];

        let is_detailed = mat.detailed_combiner.is_some();
        let mut detailed_combiner_material = DetailedCombinerMaterial::default();

        if let Some(dc) = &mat.detailed_combiner {
            detailed_combiner_material.stage_count = dc.entries.len().min(6) as u32;
            detailed_combiner_material.texture_count = texture_count;

            let color_f32 = |c: &Color4u8| {
                [
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                    c.a as f32 / 255.0,
                ]
            };

            detailed_combiner_material.constant_colors[0] = color_f32(&dc.color1);
            detailed_combiner_material.constant_colors[1] = color_f32(&dc.color2);
            detailed_combiner_material.constant_colors[2] = color_f32(&dc.color3);
            detailed_combiner_material.constant_colors[3] = color_f32(&dc.color4);
            detailed_combiner_material.constant_colors[4] = color_f32(&dc.color5);
            detailed_combiner_material.constant_colors[5] = [0.0, 0.0, 0.0, 0.0];
            detailed_combiner_material.constant_colors[6] = [0.0, 0.0, 0.0, 0.0];

            for (idx, entry) in dc.entries.iter().enumerate().take(6) {
                let (color_flags, alpha_flags, constant_selectors, _) = entry.pack_flags();

                let stage_active = 1i32;

                detailed_combiner_material.stage_bits[idx] = [
                    color_flags as i32,
                    alpha_flags as i32,
                    constant_selectors as i32,
                    stage_active,
                ];
            }
        }

        let (combine_mode, combine_mode2) = if let Some(tev0) = mat.tev_combiners.first() {
            let m0 = tev0.rgb_mode as u32;
            let m1 = mat
                .tev_combiners
                .get(1)
                .map(|t| t.rgb_mode as u32)
                .unwrap_or(0);
            (m0, m1)
        } else {
            (0, 0)
        };

        // currently using default, but how can i make it not use default because i have no idea what
        // this corresponds to
        let alpha_select = 0;

        let (indirect_mtx0, indirect_mtx1) = if let Some(im) = &mat.indirect_matrix {
            let rad = im.rotation.to_radians();
            let cos_r = rad.cos();
            let sin_r = rad.sin();
            (
                [cos_r * im.scale.x, -sin_r * im.scale.x, 0.0, 0.0],
                [sin_r * im.scale.y, cos_r * im.scale.y, 0.0, 0.0],
            )
        } else {
            ([0.0f32; 4], [0.0f32; 4])
        };

        let standard_material = StandardMaterial {
            interpolate_width,
            interpolate_offset,
            combine_mode,
            combine_mode2,
            texture_count,
            alpha_select,
            tex_gen_mode: tex_gen_mode_packed,
            use_texture_only: mat.use_texture_only as u32,
            use_thresholding_alpha_interpolation: mat.use_thresholding_alpha_interpolation as u32,
            visible: parent_visible as u32,
            indirect_mtx0,
            indirect_mtx1,
            ..Default::default()
        };

        let corners = compute_corners(
            cx,
            cy,
            w,
            h,
            &pic.base.origin.origin_x,
            &pic.base.origin.origin_y,
            rotate_z,
        );

        Some(TexturedQuad {
            x,
            y,
            width: w,
            height: h,
            corners,
            uvs,
            base_uvs,
            tint,
            corner_tints: [tint; 4],
            texture_name: tex_name.to_string(),
            texture_name1,
            texture_name2,
            address_mode_u,
            address_mode_v,
            min_filter,
            mag_filter,
            address_mode_u1,
            address_mode_v1,
            min_filter1,
            mag_filter1,
            address_mode_u2,
            address_mode_v2,
            min_filter2,
            mag_filter2,
            standard_material,
            detailed_combiner_material,
            is_detailed,
            pane_idx,
            tex_srts: mat.tex_srts.clone(),
            proj_scales,
            proj_translations,
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
        parent_visible: bool,
        parent_idx: usize,
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

        if !self.blarc_cache.contains_key(layout_name)
            && let Some(assets) = load_bflyt_from_blarc_dir(blarc_dir, layout_name)
            && let Ok(sub_bflyt) = Bflyt::parse(&assets.bflyt_bytes)
        {
            if let Some(bntx_data) = assets.bntx_bytes {
                self.discovered_bntx_buffers.push(bntx_data);
            }

            self.blarc_cache
                .insert(layout_name.to_string(), Some(sub_bflyt));
        }

        let Some(Some(sub_bflyt)) = self.blarc_cache.get(layout_name) else {
            log::warn!("PartsPane: could not load '{layout_name}'");
            return;
        };

        let mut overrides: HashMap<String, &BflytSection> = HashMap::new();
        let mut override_use_root: HashMap<String, bool> = HashMap::new();
        for prop in &parts.properties {
            let name = prop.property_name.trim_end_matches('\0');
            if name.is_empty() {
                continue;
            }
            if let Some(sec) = &prop.o_section {
                overrides.insert(name.to_string(), sec);
                override_use_root.insert(name.to_string(), prop.material_usage_flag.use_texture);
            }
        }

        let scale_x = parts.base.scale.x * parts.magnify_x;
        let scale_y = parts.base.scale.y * parts.magnify_y;

        let center_x = parts_x + parts_w * 0.5;
        let center_y = parts_y + parts_h * 0.5;

        let sub_w = sub_bflyt.layout.width;
        let sub_h = sub_bflyt.layout.height;

        let sub_parent_x = -sub_w * 0.5;
        let sub_parent_y = -sub_h * 0.5;

        let sub_nodes = sub_bflyt.nodes.clone();
        let sub_mat_list = sub_bflyt.material_list.clone();
        let root_mat_list = self.material_list;

        let parts_source_label = parts.base.pane_name.trim_end_matches('\0').to_string();

        self.walk_nodes_in_parts(
            &sub_nodes,
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
            sub_mat_list.as_ref(),
            root_mat_list,
            &parts_source_label,
            parent_visible,
            Some(parent_idx),
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
        sub_mat_list: Option<&nnbfl::bflyt::list::BflytMaterialList>,
        root_mat_list: Option<&nnbfl::bflyt::list::BflytMaterialList>,
        parts_source: &str,
        parent_visible: bool,
        parent_idx: Option<usize>,
    ) {
        let mut last_rect = (parent_x, parent_y, parent_w, parent_h);
        let mut last_visible = parent_visible;
        let mut last_pane_idx = parent_idx;

        for node in nodes {
            match node {
                BflytNode::Section(section) => {
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
                        sub_mat_list,
                        root_mat_list,
                        parts_source,
                        parent_visible,
                        parent_idx,
                    );

                    if let Some(base) = base_pane(section) {
                        let pname = pane_name(section);
                        let effective_section = overrides.get(&pname).copied().unwrap_or(section);
                        let effective_base = base_pane(effective_section).unwrap_or(base);

                        let (rx, ry, rw, rh, _, _, _, _) =
                            resolve_rect(effective_base, parent_x, parent_y, parent_w, parent_h);
                        last_rect = (rx, ry, rw, rh);
                        last_visible = parent_visible && effective_base.pane_flags.is_visible;
                        last_pane_idx = Some(self.panes.len() - 1);
                    }
                }
                BflytNode::Panes(children) => {
                    let (px, py, pw, ph) = last_rect;
                    self.walk_nodes_in_parts(
                        children,
                        px,
                        py,
                        pw,
                        ph,
                        parts_origin_x,
                        parts_origin_y,
                        parts_scale_x,
                        parts_scale_y,
                        depth + 1,
                        overrides,
                        sub_mat_list,
                        root_mat_list,
                        parts_source,
                        last_visible,
                        last_pane_idx,
                    );
                }
                BflytNode::Groups(_) => {}
            }
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
        sub_mat_list: Option<&nnbfl::bflyt::list::BflytMaterialList>,
        root_mat_list: Option<&nnbfl::bflyt::list::BflytMaterialList>,
        parts_source: &str,
        parent_visible: bool,
        parent_idx: Option<usize>,
    ) {
        match node {
            BflytNode::Section(section) => {
                let Some(base) = base_pane(section) else {
                    return;
                };

                let is_visible = parent_visible && base.pane_flags.is_visible;
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

                let (_, _, _, _, raw_anchor_x, raw_anchor_y, _, _) =
                    resolve_rect(effective_base, parent_x, parent_y, parent_w, parent_h);

                let anchor_x = parts_origin_x + raw_anchor_x * parts_scale_x;
                let anchor_y = parts_origin_y + raw_anchor_y * parts_scale_y;

                // Pivot for rotation: center of pane based on origin setting
                let cx = x + match effective_base.origin.origin_x {
                    BflytOrigin::Center => w * 0.5,
                    BflytOrigin::LeftTop => 0.0,
                    BflytOrigin::RightBottom => w,
                };
                let cy = y + match effective_base.origin.origin_y {
                    BflytOrigin::Center => h * 0.5,
                    BflytOrigin::LeftTop => 0.0,
                    BflytOrigin::RightBottom => h,
                };

                let pane_idx = self.panes.len();

                while self.children.len() <= pane_idx {
                    self.children.push(Vec::new());
                }

                if let Some(pidx) = parent_idx {
                    while self.children.len() <= pidx {
                        self.children.push(Vec::new());
                    }
                    self.children[pidx].push(pane_idx);
                }

                let mut has_textured = false;
                if let BflytSection::PicturePane(pic) = effective_section {
                    let was_overridden = overrides.contains_key(&pname);
                    let chosen_mat_list = if was_overridden {
                        root_mat_list
                    } else {
                        sub_mat_list
                    };

                    if let Some(mat_list) = chosen_mat_list
                        && let Some(tq) = self.make_textured_quad(
                            mat_list,
                            pic,
                            x,
                            y,
                            w,
                            h,
                            cx,
                            cy,
                            effective_base.rotation.z,
                            pane_idx,
                            is_visible,
                        )
                    {
                        self.textured_quads.push(tq);
                        has_textured = true;
                    }
                }

                self.quads.push(Quad {
                    x,
                    y,
                    width: w,
                    height: h,
                    color: if is_visible {
                        section_color(section)
                    } else {
                        [0.0; 4]
                    },
                    has_textured,
                });

                self.panes.push(PaneInfo {
                    label: pname,
                    kind: section_kind_name(effective_section).to_string(),
                    x,
                    y,
                    width: w,
                    height: h,
                    section: effective_section.clone(),
                    depth,
                    parent_idx,
                    parent_anchor_x: anchor_x,
                    parent_anchor_y: anchor_y,
                    parts_source: Some(parts_source.to_string()),
                    visible: is_visible,
                });

                if let BflytSection::PartsPane(nested_parts) = effective_section {
                    self.parts_depth += 1;
                    self.maybe_resolve_parts(nested_parts, x, y, w, h, depth, is_visible, pane_idx);
                    self.parts_depth -= 1;
                }
            }
            BflytNode::Panes(_) | BflytNode::Groups(_) => {}
        }
    }
}

pub fn build_view(file: Bflyt, blarc_dir: Option<&Path>, file_name: String) -> BflytView {
    let layout_w = file.layout.width;
    let layout_h = file.layout.height;

    let mut quads = Vec::new();
    let mut textured_quads = Vec::new();
    let mut panes = Vec::new();
    let mut children: Vec<Vec<usize>> = Vec::new();
    let mut blarc_cache: HashMap<String, Option<Bflyt>> = HashMap::new();

    let mut walker = Walker {
        quads: &mut quads,
        textured_quads: &mut textured_quads,
        panes: &mut panes,
        children: &mut children,
        blarc_dir,
        blarc_cache: &mut blarc_cache,
        parts_depth: 0,
        parts_source: None,
        material_list: file.material_list.as_ref(),
        discovered_bntx_buffers: Vec::new(),
    };

    walker.walk_nodes(&file.nodes, 0.0, 0.0, layout_w, layout_h, 0, true, None);
    let discovered_bntx_buffers = walker.discovered_bntx_buffers;

    children.resize(panes.len(), Vec::new());

    BflytView {
        quads,
        children,
        base_panes: panes.clone(),
        base_textured_quads: textured_quads.clone(),
        material_list: file.material_list,
        textured_quads,
        panes,
        layout_width: layout_w,
        layout_height: layout_h,
        discovered_bntx_buffers,
        file_name,
    }
}

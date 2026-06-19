use std::{collections::HashMap, path::Path};

use nnbfl::{
    bflyt::{
        file::{Bflyt, BflytNode, BflytSection},
        flags::{BflytOrigin, BflytParentOrigin},
        pane::BflytPane,
    },
    core::ReadWriteable,
    sarc::file::Sarc,
};

use crate::renderer::quad::Quad;

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
    pub parts_source: Option<String>,
}

pub struct BflytView {
    pub quads: Vec<Quad>,
    pub panes: Vec<PaneInfo>,
    pub layout_width: f32,
    pub layout_height: f32,
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

pub fn load_bflyt_from_blarc_dir(blarc_dir: &Path, layout_name: &str) -> Option<Vec<u8>> {
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

    sarc.files
        .into_iter()
        .find(|f| f.name.as_deref().is_some_and(|n| n.ends_with(".bflyt")))
        .map(|f| f.data)
}

struct Walker<'a> {
    layout_w: f32,
    layout_h: f32,
    quads: &'a mut Vec<Quad>,
    panes: &'a mut Vec<PaneInfo>,
    blarc_dir: Option<&'a Path>,
    blarc_cache: &'a mut HashMap<String, Option<Bflyt>>,
    parts_depth: usize,
    parts_source: Option<String>,
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
            let loaded = load_bflyt_from_blarc_dir(blarc_dir, layout_name)
                .and_then(|bytes| Bflyt::parse(&bytes).ok());
            self.blarc_cache.insert(layout_name.to_string(), loaded);
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
    let mut panes = Vec::new();
    let mut blarc_cache: HashMap<String, Option<Bflyt>> = HashMap::new();

    let mut walker = Walker {
        layout_w,
        layout_h,
        quads: &mut quads,
        panes: &mut panes,
        blarc_dir,
        blarc_cache: &mut blarc_cache,
        parts_depth: 0,
        parts_source: None,
    };

    walker.walk_nodes(&file.nodes, 0.0, 0.0, layout_w, layout_h, 0);

    BflytView {
        quads,
        panes,
        layout_width: layout_w,
        layout_height: layout_h,
    }
}

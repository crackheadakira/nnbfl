use nnbfl::bflyt::{
    file::{Bflyt, BflytNode, BflytSection},
    flags::{BflytOrigin, BflytParentOrigin},
    pane::BflytPane,
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

fn resolve_rect(
    pane: &BflytPane,
    parent_x: f32,
    parent_y: f32,
    parent_w: f32,
    parent_h: f32,
    layout_w: f32,
    layout_h: f32,
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

    let x = tl_x.max(-layout_w).min(layout_w * 2.0);
    let y = tl_y.max(-layout_h).min(layout_h * 2.0);

    (x, y, w.abs().max(1.0), h.abs().max(1.0))
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

struct Walker<'a> {
    layout_w: f32,
    layout_h: f32,
    quads: &'a mut Vec<Quad>,
    panes: &'a mut Vec<PaneInfo>,
}

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
                if let Some(pane) = base_pane(section) {
                    let (x, y, w, h) = resolve_rect(
                        pane,
                        parent_x,
                        parent_y,
                        parent_w,
                        parent_h,
                        self.layout_w,
                        self.layout_h,
                    );

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
                        label,
                        kind,
                        x,
                        y,
                        section: section.clone(),
                        width: w,
                        height: h,
                        depth,
                    });
                }
            }

            BflytNode::Panes(children) => {
                self.walk_nodes(children, parent_x, parent_y, parent_w, parent_h, depth + 1);
            }

            BflytNode::Groups(_) => {}
        }
    }
}

pub fn build_view(file: &Bflyt) -> BflytView {
    let layout_w = file.layout.width;
    let layout_h = file.layout.height;

    let mut quads = Vec::new();
    let mut panes = Vec::new();

    let mut walker = Walker {
        layout_w,
        layout_h,
        quads: &mut quads,
        panes: &mut panes,
    };

    walker.walk_nodes(&file.nodes, 0.0, 0.0, layout_w, layout_h, 0);

    BflytView {
        quads,
        panes,
        layout_width: layout_w,
        layout_height: layout_h,
    }
}

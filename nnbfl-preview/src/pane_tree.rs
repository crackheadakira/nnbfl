use crate::renderer::textured_quad::BatchKey;
use crate::{archive::file_stem, pane_factory::NewPaneKind};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use bitflags::bitflags;
use nnbfl::{
    bflyt::{
        file::{Bflyt, BflytNode, BflytSection, ControlSourceElement, GroupElement, PaneElement},
        flags::WindowKind,
        list::{CaptureTextureList, FontList, MaterialList, MaterialTextureSrt, TextureList},
        pane::{
            BasePaneUsageFlags, HorizontalPosition, PANE_NAME_LEN, Pane, PartsPane,
            PartsPaneBasicInfo, PicturePane, TextAlignment, TextBoxPane, TextureUv,
            VerticalPosition, WindowPane,
        },
    },
    core::FileReadWriteable,
    sarc::file::MagicFiles,
    ui2d::{
        types::{Vector2f, Vector3f},
        userdata::UserDataArray,
    },
};
use rayon::iter::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};
use tomolib::formats::bntx::Bntx;

use crate::{
    anim_state::{
        AnimationState, MaterialAnimation, PaneAnimation, ResolvedPaneTransform, transform_uv_srt,
    },
    archive_browser::{ArchiveEntry, resolve_nested_package_bytes},
    decompress_if_needed, extract_all_files_recursive,
    font::glyph_atlas::{FontFace, GLYPH_ATLAS_TEXTURE_NAME, GlyphData},
    renderer::{
        quad::Quad,
        selection::HandleCapability,
        textured_quad::{
            GlyphQuadOverride, MaterialPaneData, PaneQuadData, TexturedQuad,
            vertex_corners_color_to_corner_tints,
        },
        window_quad::{calculate_scaled_frame_uvs, calculate_window_layout, derive_from_window},
    },
    traits::Displaying,
    ui::general::SUPPORTED_SARC_EXTENSIONS,
};

pub type UvMatrix4x3x2 = [[[f32; 2]; 3]; 4];

bitflags! {
    #[derive(Clone, Copy, Debug, Default)]
    pub struct DirtyFlags: u8 {
        /// Need to recalculate transforms
        const TRANSFORM = 0x01;

        /// Need to reupload materials to GPU
        const MATERIAL = 0x02;

        /// Need to reupload vertices to GPU
        const VERTICES = 0x04;

        /// Need to rebuild bind group
        const TEXTURE = 0x08;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Corners {
    pub top_left: Vector2f,
    pub top_right: Vector2f,
    pub bottom_left: Vector2f,
    pub bottom_right: Vector2f,
}

impl Corners {
    /// Compute the four world-space corner positions [TL, TR, BL, BR] for a pane
    /// after applying rotation around the pane's pivot point (cx, cy).
    pub fn compute(
        center: Vector2f,
        size: Vector2f,
        origin_x: &HorizontalPosition,
        origin_y: &VerticalPosition,
        rotation: Vector3f,
    ) -> Self {
        let lx = match origin_x {
            HorizontalPosition::Center => -size.x * 0.5,
            HorizontalPosition::Left => 0.0,
            HorizontalPosition::Right => -size.x,
        };

        let ly = match origin_y {
            VerticalPosition::Center => -size.y * 0.5,
            VerticalPosition::Top => 0.0,
            VerticalPosition::Bottom => -size.y,
        };

        let tl = Vector2f::new(lx, ly);
        let tr = Vector2f::new(lx + size.x, ly);
        let bl = Vector2f::new(lx, ly + size.y);
        let br = Vector2f::new(lx + size.x, ly + size.y);

        Self {
            top_left: Self::rotate_point(tl, center, rotation),
            top_right: Self::rotate_point(tr, center, rotation),
            bottom_left: Self::rotate_point(bl, center, rotation),
            bottom_right: Self::rotate_point(br, center, rotation),
        }
    }

    pub fn to_array(self) -> [[f32; 2]; 4] {
        [
            [self.top_left.x, self.top_left.y],
            [self.top_right.x, self.top_right.y],
            [self.bottom_left.x, self.bottom_left.y],
            [self.bottom_right.x, self.bottom_right.y],
        ]
    }

    pub fn translate(&self, delta: Vector2f) -> Self {
        Self {
            top_left: Vector2f {
                x: self.top_left.x + delta.x,
                y: self.top_left.y + delta.y,
            },
            top_right: Vector2f {
                x: self.top_right.x + delta.x,
                y: self.top_right.y + delta.y,
            },
            bottom_left: Vector2f {
                x: self.bottom_left.x + delta.x,
                y: self.bottom_left.y + delta.y,
            },
            bottom_right: Vector2f {
                x: self.bottom_right.x + delta.x,
                y: self.bottom_right.y + delta.y,
            },
        }
    }

    pub fn rotate_point(point: Vector2f, center: Vector2f, rotation: Vector3f) -> Vector2f {
        if rotation.x == 0.0 && rotation.y == 0.0 && rotation.z == 0.0 {
            return center + point;
        }

        let (point_x, point_y, point_z) = (point.x, point.y, 0.0f32);

        let rotation_x = -rotation.x.to_radians();
        let rotation_y = -rotation.y.to_radians();
        let rotation_z = -rotation.z.to_radians();

        let (rot_x_sin, rot_x_cos) = rotation_x.sin_cos();
        let (rot_y_sin, rot_y_cos) = rotation_y.sin_cos();
        let (rot_z_sin, rot_z_cos) = rotation_z.sin_cos();

        let x1 = point_x * rot_z_cos - point_y * rot_z_sin;
        let y1 = point_x * rot_z_sin + point_y * rot_z_cos;

        let x2 = x1 * rot_y_cos + point_z * rot_y_sin;
        let z2 = -x1 * rot_y_sin + point_z * rot_y_cos;

        let y3 = y1 * rot_x_cos - z2 * rot_x_sin;

        Vector2f {
            x: center.x + x2,
            y: center.y + y3,
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PaneId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Anchor {
    First,
    Last,
    Before(PaneId),
    After(PaneId),
}

impl std::fmt::Display for PaneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug)]
pub struct PaneNode {
    pub section: BflytSection,
    pub kind: String,
    pub label: String,
    pub depth: usize,
    pub visible: bool,
    pub parts_source: Option<String>,
    pub is_parts_overridden: bool,
    pub pane_idx: PaneId,

    pub world_pos: Vector2f,
    pub world_size: Vector2f,
    pub world_center: Vector2f,
    pub world_rotation: Vector3f,
    pub world_corners: Corners,
    pub parent_anchor: Vector2f,

    pub plain_quad: Quad,
    pub textured_quad: Option<TexturedQuad>,
    pub animation: PaneAnimation,
    pub window_quads: Vec<TexturedQuad>,
    pub glyph_quads: Vec<TexturedQuad>,

    pub dirty: DirtyFlags,
    pub children: Vec<PaneNode>,
    pub parts_children: Vec<PaneNode>,
    pub user_data: Option<UserDataArray>,
    pub handle_capabilities: Vec<HandleCapability>,
}

impl PaneNode {
    pub fn flatten_to_bflyt_nodes(&self, out: &mut Vec<BflytNode>) {
        puffin::profile_function!();
        if self.plain_quad.is_parts_root || self.parts_source.is_some() {
            return;
        }

        let mut baked_section = self.section.clone();
        if let Some(base) = baked_section.get_base_pane_mut() {
            base.pane_name = self.label.clone();
        }

        let mut child_nodes = Vec::new();
        for child in &self.children {
            child.flatten_to_bflyt_nodes(&mut child_nodes);
        }

        out.push(BflytNode::Pane(PaneElement {
            data: baked_section,
            user_data: self.user_data.clone(),
            children: child_nodes,
        }));
    }

    pub fn iter(&self) -> impl Iterator<Item = &PaneNode> {
        PaneIter { stack: vec![self] }
    }

    pub fn all_children(&self) -> impl DoubleEndedIterator<Item = &PaneNode> {
        self.parts_children.iter().chain(&self.children)
    }

    pub fn all_children_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut PaneNode> {
        self.parts_children.iter_mut().chain(&mut self.children)
    }

    /// The child at `index` in [`Self::all_children`] order
    pub fn child_at(&self, index: usize) -> Option<&PaneNode> {
        match index.checked_sub(self.parts_children.len()) {
            Some(own) => self.children.get(own),
            None => self.parts_children.get(index),
        }
    }

    pub fn child_at_mut(&mut self, index: usize) -> Option<&mut PaneNode> {
        match index.checked_sub(self.parts_children.len()) {
            Some(own) => self.children.get_mut(own),
            None => self.parts_children.get_mut(index),
        }
    }

    pub fn material_indices(&self) -> Vec<u16> {
        match &self.section {
            BflytSection::PicturePane(pic) => vec![pic.material_index],
            BflytSection::TextBoxPane(txt) => vec![txt.material_index],
            BflytSection::WindowPane(win) => std::iter::once(win.content.material_index)
                .chain(win.frames.iter().map(|f| f.material_index))
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn descendants(&self) -> Vec<PaneId> {
        fn collect_all(node: &PaneNode, out: &mut Vec<PaneId>) {
            for child in node.all_children() {
                out.push(child.pane_idx);
                collect_all(child, out);
            }
        }

        let mut out = Vec::new();
        collect_all(self, &mut out);
        out
    }

    pub fn mark_transform_dirty(&mut self) {
        self.dirty
            .insert(DirtyFlags::TRANSFORM | DirtyFlags::VERTICES);

        for child in self.all_children_mut() {
            child.mark_transform_dirty();
        }
    }

    pub fn recompute_glyphs(
        &mut self,
        font_list: Option<&FontList>,
        material_list: Option<&MaterialList>,
        glyphs: &mut GlyphData,
    ) {
        self.glyph_quads = if let BflytSection::TextBoxPane(txt) = &self.section {
            build_text_glyph_quads(
                txt,
                self.world_center,
                self.world_size,
                self.world_rotation,
                self.visible,
                self.pane_idx,
                font_list,
                material_list,
                glyphs,
            )
        } else {
            Vec::new()
        };

        for child in self.all_children_mut() {
            child.recompute_glyphs(font_list, material_list, glyphs);
        }
    }

    fn walk_mut<F>(&mut self, f: &mut F)
    where
        F: FnMut(&mut PaneNode),
    {
        f(self);

        for child in self.all_children_mut() {
            child.walk_mut(f);
        }
    }

    pub fn find_descendant_by_label_mut(&mut self, label: &str) -> Option<&mut PaneNode> {
        for child in &mut self.children {
            if child.label.trim_end_matches('\0') == label {
                return Some(child);
            }

            if let Some(found) = child.find_descendant_by_label_mut(label) {
                return Some(found);
            }
        }

        None
    }

    pub fn recompute(
        &mut self,
        parent_pos: Vector2f,
        parent_size: Vector2f,
        parent_scale: Vector2f,
        parent_rotation: Vector3f,
        bntxs: &[&Bntx],
    ) {
        let transform = self
            .section
            .get_base_pane()
            .map(|base| self.animation.resolve_transform(base));
        let child_scale;

        if self.dirty.contains(DirtyFlags::TRANSFORM) {
            if let Some(base) = self.section.get_base_pane()
                && let Some(transform) = transform
            {
                let (pos, size, anchor, center) =
                    resolve_rect(base, &transform, parent_pos, parent_size, parent_scale);

                self.world_pos = pos;
                self.world_size = size;
                self.world_center = center;
                self.parent_anchor = anchor;
                self.world_rotation = transform.rotation + parent_rotation;

                let corners = Corners::compute(
                    center,
                    size,
                    &base.position.position_x,
                    &base.position.position_y,
                    self.world_rotation,
                );

                self.world_corners = corners;

                self.plain_quad.corners = corners.to_array();
                self.plain_quad.width = size.x;
                self.plain_quad.height = size.y;

                if let Some(tq) = &mut self.textured_quad {
                    tq.x = pos.x;
                    tq.y = pos.y;

                    tq.width = size.x;
                    tq.height = size.y;

                    tq.corners = corners.to_array();
                    tq.rotation = self.world_rotation;
                }

                if !self.window_quads.is_empty()
                    && let BflytSection::WindowPane(win) = &self.section
                {
                    let layout = calculate_window_layout(win, corners.to_array());

                    let mut quad_idx = 0;

                    if layout.content.is_some()
                        && let Some(geom) = layout.content
                    {
                        if let Some(tq) = self.window_quads.get_mut(quad_idx) {
                            tq.x = geom.x;
                            tq.y = geom.y;
                            tq.width = geom.width;
                            tq.height = geom.height;
                            tq.corners = geom.corners;
                            tq.rotation = self.world_rotation;
                        }

                        quad_idx += 1;
                    }

                    for geom in layout.frames {
                        let Some(kind) = geom.frame_kind else {
                            continue;
                        };

                        let Some((config_idx, flip_override)) =
                            kind.to_binary_index(win.frames.len())
                        else {
                            continue;
                        };

                        let Some(frame_data) = win.frames.get(config_idx) else {
                            continue;
                        };

                        if let Some(tq) = self.window_quads.get_mut(quad_idx) {
                            tq.x = geom.x;
                            tq.y = geom.y;
                            tq.width = geom.width;
                            tq.height = geom.height;
                            tq.corners = geom.corners;
                            tq.rotation = self.world_rotation;

                            let (tex_w, tex_h) = {
                                bntxs
                                    .iter()
                                    .flat_map(|b| &b.textures)
                                    .find(|t| t.name == *tq.texture_name)
                                    .map(|t| (t.info.width as f32, t.info.height as f32))
                                    .unwrap_or((1.0, 1.0))
                            };

                            let effective_flip =
                                flip_override.unwrap_or(frame_data.texture_flip_mode);

                            let texture_uvs = calculate_scaled_frame_uvs(
                                geom.width,
                                geom.height,
                                tex_w,
                                tex_h,
                                kind,
                                win.flag.window_kind,
                                effective_flip,
                                tq.standard_material.texture_count as usize,
                                win.frames.len(),
                            );

                            let base_uvs = Self::compute_uvs(&texture_uvs);
                            let uvs = Self::apply_srt_to_uvs(base_uvs, &tq.tex_srts);

                            tq.base_uvs = base_uvs;
                            tq.uvs = uvs;
                        }

                        quad_idx += 1;
                    }
                }

                self.dirty.remove(DirtyFlags::TRANSFORM);
                child_scale = transform.scale * parent_scale;
            } else {
                child_scale = parent_scale;
            }
        } else {
            child_scale = transform
                .map(|t| t.scale * parent_scale)
                .unwrap_or(parent_scale);
        }

        let (child_rotation, child_size) = transform
            .map(|t| (t.rotation + parent_rotation, self.world_size))
            .unwrap_or((parent_rotation, parent_size));

        let center = self.world_center;
        self.parts_children
            .par_iter_mut()
            .chain(self.children.par_iter_mut())
            .for_each(|child| {
                child.recompute(center, child_size, child_scale, child_rotation, bntxs);
            });
    }

    pub fn compute_uvs(texture_uvs: &[TextureUv]) -> UvMatrix4x3x2 {
        let get_uv_set = |layer: usize| -> [[f32; 2]; 4] {
            if let Some(uv_set) = texture_uvs.get(layer) {
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

        base_uvs
    }

    pub fn apply_srt_to_uvs(
        base_uvs: UvMatrix4x3x2,
        tex_srts: &[MaterialTextureSrt],
    ) -> UvMatrix4x3x2 {
        let mut uvs = base_uvs;

        for layer in 0..3 {
            if let Some(srt) = tex_srts.get(layer) {
                for v_idx in 0..4 {
                    uvs[v_idx][layer] = transform_uv_srt(srt, base_uvs[v_idx][layer]);
                }
            }
        }

        uvs
    }
}

pub struct PaneIter<'a> {
    stack: Vec<&'a PaneNode>,
}
impl<'a> Iterator for PaneIter<'a> {
    type Item = &'a PaneNode;
    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;

        for child in node.all_children().rev() {
            self.stack.push(child);
        }

        Some(node)
    }
}

pub struct PaneTree {
    pub roots: Vec<PaneNode>,

    pub layout_size: Vector2f,
    pub material_list: Option<MaterialList>,
    pub material_animations: HashMap<String, MaterialAnimation>,
    pub animation_binding_revision: u64,
    pub user_data: Option<UserDataArray>,
    pub texture_list: Option<TextureList>,
    pub font_list: Option<FontList>,
    pub capture_texture_list: Option<CaptureTextureList>,
    pub control_source: Option<ControlSourceElement>,
    pub group: GroupElement,

    pub file_name: String,
    pub main_bntx: Option<Bntx>,
    pub sub_bntxs: Vec<Bntx>,

    pub parent_map: HashMap<PaneId, Option<PaneId>>,
    pub label_map: HashMap<String, PaneId>,

    /// Indices in [`PaneNode::child_at`] order, from the roots
    pub path_map: HashMap<PaneId, Vec<usize>>,
    /// The panes drawing with each material of this layout's list
    pub material_users: HashMap<u16, Vec<PaneId>>,
    material_dirty: HashSet<PaneId>,
    needs_rebatch: bool,

    pub next_id: u32,
}

impl PaneTree {
    pub fn set_animation(&mut self, mut state: AnimationState) -> DirtyFlags {
        let mut changes = DirtyFlags::empty();
        self.for_each_mut(|node| {
            let next = state.panes.remove(&node.pane_idx).unwrap_or_default();

            if let Some(base) = node.section.get_base_pane()
                && node.animation.resolve_transform(base) != next.resolve_transform(base)
            {
                node.mark_transform_dirty();
                changes.insert(DirtyFlags::TRANSFORM);
            }

            if node.animation != next {
                node.animation = next;
                changes.insert(DirtyFlags::VERTICES);
            }
        });

        if self.material_animations != state.materials {
            self.material_animations = state.materials;
            changes.insert(DirtyFlags::MATERIAL);
        }
        changes
    }

    pub fn update_animation_visuals(&mut self) {
        fn update(
            node: &mut PaneNode,
            parent_visible: bool,
            parent_alpha: f32,
            materials: &HashMap<String, MaterialAnimation>,
        ) {
            let (visible, alpha) = if let Some(base) = node.section.get_base_pane() {
                let visible =
                    parent_visible && node.animation.visible.unwrap_or(base.pane_flags.is_visible);
                let local_alpha = node.animation.alpha.unwrap_or(base.alpha as f32 / 255.0);
                let alpha = if base.pane_flags.influenced_alpha {
                    local_alpha * parent_alpha
                } else {
                    local_alpha
                };
                (visible, alpha)
            } else {
                (parent_visible, parent_alpha)
            };

            node.visible = visible;
            node.plain_quad.color = if visible {
                node.section.section_color()
            } else {
                [0.0; 4]
            };

            let animation = &node.animation;
            for quad in node
                .textured_quad
                .iter_mut()
                .chain(node.window_quads.iter_mut())
                .chain(node.glyph_quads.iter_mut())
            {
                let material = if node.parts_source.is_none() {
                    materials.get(&quad.base_appearance.material_name)
                } else {
                    None
                };
                quad.apply_animation(animation, material, alpha, visible);
            }

            for child in node.all_children_mut() {
                update(child, visible, alpha, materials);
            }
        }

        for root in &mut self.roots {
            update(root, true, 1.0, &self.material_animations);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &PaneNode> {
        self.roots.iter().flat_map(|r| r.iter())
    }

    pub fn flatten(&self) -> Vec<&PaneNode> {
        puffin::profile_function!();
        self.iter().collect()
    }

    pub fn for_each_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut PaneNode),
    {
        for root in &mut self.roots {
            root.walk_mut(&mut f);
        }
    }

    pub fn for_each_descendant_mut<F>(&mut self, target_idx: PaneId, mut f: F)
    where
        F: FnMut(&mut PaneNode),
    {
        if let Some(target_node) = self.find_by_idx_mut(target_idx) {
            target_node.walk_mut(&mut f);
        }
    }

    pub fn all_bntxs(&self) -> impl Iterator<Item = &Bntx> {
        self.main_bntx.iter().chain(self.sub_bntxs.iter())
    }

    pub fn recompute_dirty(&mut self /*fonts: &mut FontCache, atlas: &mut GlyphAtlas*/) {
        puffin::profile_function!();
        let layout_center = Vector2f {
            x: self.layout_size.x * 0.5,
            y: self.layout_size.y * 0.5,
        };

        let mut roots = std::mem::take(&mut self.roots);
        let bntx_refs: Vec<_> = self.all_bntxs().collect();

        roots.par_iter_mut().for_each(|root: &mut PaneNode| {
            root.recompute(
                layout_center,
                self.layout_size,
                Vector2f { x: 1.0, y: 1.0 },
                Vector3f::default(),
                &bntx_refs,
            );
        });

        self.roots = roots;

        /*
        let font_list = self.font_list.as_ref();
        let material_list = self.material_list.as_ref();

        for root in &mut self.roots {
            root.recompute_glyphs(font_list, material_list, fonts, atlas);
        }
        */
    }

    pub fn mark_material_dirty(&mut self, material: u16) {
        if let Some(users) = self.material_users.get(&material) {
            self.material_dirty.extend(users);
        }
    }

    pub fn mark_pane_dirty(&mut self, pane: PaneId) {
        self.material_dirty.insert(pane);
    }

    pub fn recompute_dirty_materials(&mut self, glyphs: &mut GlyphData) {
        puffin::profile_function!();
        for pane in std::mem::take(&mut self.material_dirty) {
            if let Some(rebatch) = self.rebuild_node_visuals(pane, glyphs) {
                self.needs_rebatch |= rebatch;
            }
        }
    }

    pub fn take_rebatch(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebatch)
    }

    pub fn rebuild_node_visuals(&mut self, pane: PaneId, glyphs: &mut GlyphData) -> Option<bool> {
        let node = self.find_by_idx(pane)?;

        if node.parts_source.is_some() && !node.is_parts_overridden {
            return Some(false);
        }

        let bntxs: Vec<&Bntx> = self.all_bntxs().collect();
        let sources = VisualSources {
            material_list: self.material_list.as_ref(),
            font_list: self.font_list.as_ref(),
            bntxs: &bntxs,
        };

        let placement = PanePlacement {
            position: node.world_pos,
            size: node.world_size,
            center: node.world_center,
            rotation: node.world_rotation,
            visible: node.visible,
        };

        let visuals = build_pane_visuals(&node.section, &placement, pane, &sources, glyphs);

        let node = self.find_by_idx_mut(pane)?;
        let keys = |quads: &mut dyn Iterator<Item = &TexturedQuad>| -> Vec<BatchKey> {
            quads.map(BatchKey::of).collect()
        };

        let rebatch = keys(
            &mut node
                .textured_quad
                .iter()
                .chain(&node.window_quads)
                .chain(&node.glyph_quads),
        ) != keys(
            &mut visuals
                .textured_quad
                .iter()
                .chain(&visuals.window_quads)
                .chain(&visuals.glyph_quads),
        );

        node.textured_quad = visuals.textured_quad;
        node.window_quads = visuals.window_quads;
        node.glyph_quads = visuals.glyph_quads;

        Some(rebatch)
    }

    pub fn collect_render_quads<'a>(&'a mut self) -> Vec<PaneQuadData<'a>> {
        puffin::profile_function!();

        fn collect_recursive<'a>(node: &'a mut PaneNode, out: &mut Vec<PaneQuadData<'a>>) {
            if let Some(tq) = &mut node.textured_quad {
                out.push(PaneQuadData::Textured(tq));
            } else if !node.window_quads.is_empty() {
                for tq in &mut node.window_quads {
                    out.push(PaneQuadData::Textured(tq));
                }
            } else if !node.glyph_quads.is_empty() {
                for tq in &mut node.glyph_quads {
                    out.push(PaneQuadData::Textured(tq));
                }
            }

            if !node.plain_quad.is_parts_root {
                out.push(PaneQuadData::Plain(&node.plain_quad));
            }

            for child in node.parts_children.iter_mut().chain(&mut node.children) {
                collect_recursive(child, out);
            }
        }

        let mut out = Vec::with_capacity(self.next_id as usize);
        for root in &mut self.roots {
            collect_recursive(root, &mut out);
        }

        out
    }

    pub fn find_by_label(&self, label: &str) -> Option<&PaneNode> {
        let idx = *self.label_map.get(label)?;
        self.find_by_idx(idx)
    }

    pub fn find_by_idx(&self, target_idx: PaneId) -> Option<&PaneNode> {
        puffin::profile_function!();
        let path = self.path_map.get(&target_idx)?;
        let mut path_iter = path.iter();

        let root_sibling_idx = *path_iter.next()?;
        let mut current_node = self.roots.get(root_sibling_idx)?;

        for &sibling_idx in path_iter {
            current_node = current_node.child_at(sibling_idx)?;
        }

        Some(current_node)
    }

    pub fn find_by_idx_mut(&mut self, target_idx: PaneId) -> Option<&mut PaneNode> {
        puffin::profile_function!();
        let path = self.path_map.get(&target_idx)?;
        let mut path_iter = path.iter();

        let root_sibling_idx = *path_iter.next()?;
        let mut current_node = self.roots.get_mut(root_sibling_idx)?;

        for &sibling_idx in path_iter {
            current_node = current_node.child_at_mut(sibling_idx)?;
        }

        Some(current_node)
    }

    pub fn label_to_idx(&self) -> HashMap<String, PaneId> {
        self.label_map.clone()
    }

    pub fn descendants(&self, pane_idx: PaneId) -> Vec<PaneId> {
        self.find_by_idx(pane_idx)
            .map(|node| node.descendants())
            .unwrap_or_default()
    }

    pub fn insert_node(&mut self, parent_idx: Option<PaneId>, node: PaneNode) -> PaneId {
        self.insert_node_at(parent_idx, usize::MAX, node)
    }

    pub fn insert_node_at(
        &mut self,
        parent_idx: Option<PaneId>,
        position: usize,
        node: PaneNode,
    ) -> PaneId {
        let old_idx = node.pane_idx;

        match parent_idx.and_then(|pid| self.find_by_idx_mut(pid)) {
            Some(parent_node) => {
                let pos = position.min(parent_node.children.len());
                parent_node.children.insert(pos, node);
            }
            None => {
                let pos = if parent_idx.is_some() {
                    self.roots.len()
                } else {
                    position.min(self.roots.len())
                };

                self.roots.insert(pos, node);
            }
        }

        self.rebuild_maps();

        old_idx
    }

    pub fn sibling_position(&self, target_idx: PaneId) -> Option<(Option<PaneId>, usize)> {
        let parent_idx = *self.parent_map.get(&target_idx)?;
        let path = self.path_map.get(&target_idx)?;
        let position = *path.last()?;

        let parts_len = match parent_idx {
            Some(parent) => self.find_by_idx(parent)?.parts_children.len(),
            None => 0,
        };

        Some((parent_idx, position.checked_sub(parts_len)?))
    }

    pub fn remove_node(&mut self, target_idx: PaneId) -> Option<PaneNode> {
        let parent_idx = *self.parent_map.get(&target_idx)?;

        let removed_node = match parent_idx {
            Some(pid) => {
                let parent_node = self.find_by_idx_mut(pid)?;
                let pos = parent_node
                    .children
                    .iter()
                    .position(|n| n.pane_idx == target_idx)?;

                Some(parent_node.children.remove(pos))
            }
            None => {
                let pos = self.roots.iter().position(|n| n.pane_idx == target_idx)?;

                Some(self.roots.remove(pos))
            }
        };

        if removed_node.is_some() {
            self.rebuild_maps();
        }

        removed_node
    }

    pub fn is_ancestor_or_self(&self, node_idx: PaneId, candidate: PaneId) -> bool {
        if node_idx == candidate {
            return true;
        }

        let mut current = self.parent_map.get(&node_idx).copied().flatten();
        while let Some(idx) = current {
            if idx == candidate {
                return true;
            }

            current = self.parent_map.get(&idx).copied().flatten();
        }

        false
    }

    pub fn resolve_drop_position(
        &self,
        target: PaneId,
        position: &egui_ltreeview::DirPosition<PaneId>,
    ) -> Option<(Option<PaneId>, Anchor)> {
        use egui_ltreeview::DirPosition;

        let parent = self.find_by_idx(target)?;

        if parent.parts_source.is_some() {
            return None;
        }

        let is_child = |id: PaneId| parent.children.iter().any(|c| c.pane_idx == id);

        let anchor = match *position {
            DirPosition::First => Anchor::First,
            DirPosition::Last => Anchor::Last,
            DirPosition::Before(sibling) if is_child(sibling) => Anchor::Before(sibling),
            DirPosition::After(sibling) if is_child(sibling) => Anchor::After(sibling),
            DirPosition::Before(_) | DirPosition::After(_) => Anchor::First,
        };

        Some((Some(target), anchor))
    }

    fn siblings(&self, parent: Option<PaneId>) -> Option<&[PaneNode]> {
        match parent {
            Some(pid) => Some(&self.find_by_idx(pid)?.children),
            None => Some(&self.roots),
        }
    }

    pub fn anchor_of(&self, pane_idx: PaneId) -> Option<(Option<PaneId>, Anchor)> {
        let parent = *self.parent_map.get(&pane_idx)?;
        let siblings = self.siblings(parent)?;
        let position = siblings.iter().position(|n| n.pane_idx == pane_idx)?;

        let anchor = match position {
            0 => Anchor::First,
            _ => Anchor::After(siblings[position - 1].pane_idx),
        };

        Some((parent, anchor))
    }

    pub fn anchor_position(&self, parent: Option<PaneId>, anchor: Anchor) -> Option<usize> {
        let siblings = self.siblings(parent)?;
        let index_of = |id| siblings.iter().position(|n| n.pane_idx == id);

        Some(match anchor {
            Anchor::First => 0,
            Anchor::Last => siblings.len(),
            Anchor::Before(id) => index_of(id).unwrap_or(siblings.len()),
            Anchor::After(id) => index_of(id).map_or(siblings.len(), |i| i + 1),
        })
    }

    pub fn selection_roots(&self, panes: &[PaneId]) -> Vec<PaneId> {
        let listed: HashSet<PaneId> = panes.iter().copied().collect();

        let has_listed_ancestor = |pane: PaneId| {
            let mut current = self.parent_map.get(&pane).copied().flatten();

            while let Some(parent) = current {
                if listed.contains(&parent) {
                    return true;
                }
                current = self.parent_map.get(&parent).copied().flatten();
            }

            false
        };

        self.iter()
            .filter(|n| listed.contains(&n.pane_idx) && n.parts_source.is_none())
            .map(|n| n.pane_idx)
            .filter(|&pane| !has_listed_ancestor(pane))
            .collect()
    }

    pub fn can_add(&self, parent: Option<PaneId>, kind: NewPaneKind) -> bool {
        let fonts = self.font_list.as_ref().is_some_and(|f| !f.fonts.is_empty());

        (fonts || !kind.needs_font())
            && parent.is_some_and(|p| {
                self.find_by_idx(p)
                    .is_some_and(|n| n.parts_source.is_none())
            })
    }

    pub fn can_move(&self, source_idx: PaneId, new_parent: Option<PaneId>, anchor: Anchor) -> bool {
        if matches!(anchor, Anchor::Before(id) | Anchor::After(id) if id == source_idx) {
            return false;
        }

        if self
            .find_by_idx(source_idx)
            .is_none_or(|n| n.parts_source.is_some())
        {
            return false;
        }

        match new_parent {
            Some(parent) => {
                !self.is_ancestor_or_self(parent, source_idx)
                    && self
                        .find_by_idx(parent)
                        .is_some_and(|n| n.parts_source.is_none())
            }
            None => true,
        }
    }

    pub fn move_node(
        &mut self,
        source_idx: PaneId,
        new_parent: Option<PaneId>,
        anchor: Anchor,
    ) -> Option<(Option<PaneId>, Anchor)> {
        if !self.can_move(source_idx, new_parent, anchor) {
            return None;
        }

        let old = self.anchor_of(source_idx)?;

        let mut node = self.remove_node(source_idx)?;
        node.mark_transform_dirty();

        let position = self
            .anchor_position(new_parent, anchor)
            .unwrap_or(usize::MAX);
        self.insert_node_at(new_parent, position, node);

        (self.anchor_of(source_idx)? != old).then_some(old)
    }

    pub fn rebuild_maps(&mut self) {
        puffin::profile_function!();
        self.animation_binding_revision = self.animation_binding_revision.wrapping_add(1);
        self.parent_map.clear();
        self.label_map.clear();
        self.path_map.clear();
        self.material_users.clear();

        fn walk(
            node: &PaneNode,
            parent: Option<PaneId>,
            path: &mut Vec<usize>,
            parent_map: &mut HashMap<PaneId, Option<PaneId>>,
            label_map: &mut HashMap<String, PaneId>,
            path_map: &mut HashMap<PaneId, Vec<usize>>,
        ) {
            parent_map.insert(node.pane_idx, parent);
            path_map.insert(node.pane_idx, path.clone());

            if node.parts_source.is_none() {
                label_map.insert(node.label.trim_end_matches('\0').to_string(), node.pane_idx);
            }

            for (i, child) in node.all_children().enumerate() {
                path.push(i);
                walk(
                    child,
                    Some(node.pane_idx),
                    path,
                    parent_map,
                    label_map,
                    path_map,
                );
                path.pop();
            }
        }

        for (i, root) in self.roots.iter().enumerate() {
            walk(
                root,
                None,
                &mut vec![i],
                &mut self.parent_map,
                &mut self.label_map,
                &mut self.path_map,
            );
        }

        for node in self.roots.iter().flat_map(|root| root.iter()) {
            if node.parts_source.is_none() || node.is_parts_overridden {
                for material in node.material_indices() {
                    self.material_users
                        .entry(material)
                        .or_default()
                        .push(node.pane_idx);
                }
            }
        }
    }

    pub fn pane_name_error(&self, name: &str, pane: Option<PaneId>) -> Option<String> {
        if name.is_empty() {
            return Some("The name is empty".into());
        }

        if !name.bytes().all(|b| b.is_ascii_graphic()) {
            return Some("Names are ASCII without spaces".into());
        }

        if name.len() > PANE_NAME_LEN {
            return Some(format!("Names are at most {PANE_NAME_LEN} bytes"));
        }

        match self.label_map.get(name) {
            Some(other) if Some(*other) != pane => Some(format!("Another pane is named {name}")),
            _ => None,
        }
    }

    pub fn unique_name(&self, base: &str, taken: &HashSet<String>) -> String {
        let base = base.trim_end_matches('\0');
        let stem = match base.rsplit_once('_') {
            Some((stem, digits))
                if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) =>
            {
                stem
            }
            _ => base,
        };

        (0..)
            .map(|n| {
                let suffix = format!("_{n:02}");
                let mut end = stem.len().min(PANE_NAME_LEN - suffix.len());
                while !stem.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}{suffix}", &stem[..end])
            })
            .find(|name| !self.label_map.contains_key(name) && !taken.contains(name))
            .unwrap()
    }

    pub fn rename_references(&mut self, old: &str, new: &str) {
        for name in self.group_child_names_mut() {
            if name.trim_end_matches('\0') == old {
                *name = new.to_string();
            }
        }

        if let Some(control) = &mut self.control_source {
            for binding in &mut control.data.pane_bindings {
                if binding.trim_end_matches('\0') == old {
                    *binding = new.to_string();
                }
            }
        }
    }

    pub fn drop_references(&mut self, names: &HashSet<String>) {
        let named = |name: &String| names.contains(name.trim_end_matches('\0'));

        self.group.data.child_names.retain(|n| !named(n));
        for group in &mut self.group.children {
            group.child_names.retain(|n| !named(n));
        }

        if let Some(control) = &mut self.control_source {
            for binding in &mut control.data.pane_bindings {
                if named(binding) {
                    binding.clear();
                }
            }
        }
    }

    fn group_child_names_mut(&mut self) -> impl Iterator<Item = &mut String> {
        self.group.data.child_names.iter_mut().chain(
            self.group
                .children
                .iter_mut()
                .flat_map(|g| &mut g.child_names),
        )
    }

    pub fn new_node(&mut self, section: BflytSection, parent: Option<PaneId>) -> PaneNode {
        let pane_idx = self.alloc_id();
        let depth = parent
            .and_then(|p| self.find_by_idx(p))
            .map_or(0, |p| p.depth + 1);

        let placement = PanePlacement {
            position: Vector2f::default(),
            size: section
                .get_base_pane()
                .map_or_else(Vector2f::default, |b| b.size),
            center: Vector2f::default(),
            rotation: Vector3f::default(),
            visible: true,
        };

        let mut node = assemble_node(
            &section,
            &placement,
            Vector2f::default(),
            pane_idx,
            PaneVisuals::default(),
            NodeOrigin {
                depth,
                parts_source: None,
                is_parts_root: false,
            },
        );

        node.dirty = DirtyFlags::all();
        node
    }

    pub fn alloc_id(&mut self) -> PaneId {
        let id = PaneId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn duplicate_node(&mut self, target_idx: PaneId) -> Option<PaneId> {
        let parent_idx = self.parent_map.get(&target_idx).copied().flatten();
        let mut clone = self.find_by_idx(target_idx)?.clone();

        fn reindex(node: &mut PaneNode, tree: &mut PaneTree) {
            node.pane_idx = tree.alloc_id();

            node.plain_quad.pane_idx = node.pane_idx;
            for quad in node
                .textured_quad
                .iter_mut()
                .chain(&mut node.window_quads)
                .chain(&mut node.glyph_quads)
            {
                quad.pane_idx = node.pane_idx;
            }
            node.dirty = DirtyFlags::all();

            for child in node.all_children_mut() {
                reindex(child, tree);
            }
        }

        reindex(&mut clone, self);

        fn rename(node: &mut PaneNode, tree: &PaneTree, taken: &mut HashSet<String>) {
            node.label = tree.unique_name(&node.label, taken);
            taken.insert(node.label.clone());

            for child in &mut node.children {
                rename(child, tree, taken);
            }
        }
        rename(&mut clone, self, &mut HashSet::new());

        let position = self
            .sibling_position(target_idx)
            .map(|(_, pos)| pos + 1)
            .unwrap_or(usize::MAX);

        Some(self.insert_node_at(parent_idx, position, clone))
    }

    pub fn from_bflyt(
        file: Bflyt,
        blarc_dir: Option<&Path>,
        file_name: String,
        archive_entries: Option<&[ArchiveEntry]>,
        archive_layouts: &[(String, Vec<u8>)],
        discovered_bntxs: Vec<Bntx>,
        glyphs: &mut GlyphData,
    ) -> Self {
        puffin::profile_function!();
        let layout_size = Vector2f {
            x: file.layout.width,
            y: file.layout.height,
        };

        let mut root_bntxs = discovered_bntxs.into_iter();
        let main_bntx = root_bntxs.next();
        let mut sub_bntxs: Vec<Bntx> = root_bntxs.collect();

        if !sub_bntxs.is_empty() {
            log::warn!(
                "'{file_name}' bundles {} extra BNTX beyond the first, only the first is treated as the editable main texture file.",
                sub_bntxs.len()
            );
        }

        let mut blarc_cache: HashMap<String, Option<Bflyt>> = HashMap::new();

        let mut builder = Builder {
            material_list: file.material_list.as_ref(),
            sub_material_list: None,
            font_list: file.font_list.as_ref(),
            archive_entries,
            archive_layouts,
            blarc_dir,
            blarc_cache: &mut blarc_cache,
            main_bntx: main_bntx.as_ref(),
            sub_bntxs: &mut sub_bntxs,
            parts_chain: Vec::new(),
            next_pane_idx: 0,
            active_all_files: Vec::new(),
            active_sarc_key: None,
            active_sarc_bntxs_parsed: false,
            glyphs,
        };

        let layout_center = Vector2f {
            x: file.layout.width * 0.5,
            y: file.layout.height * 0.5,
        };

        let roots = builder.build_nodes(
            &file.nodes,
            layout_center,
            layout_size,
            Vector2f { x: 1.0, y: 1.0 },
            true,
            0,
        );

        let next_id = builder.next_pane_idx;

        let mut tree = PaneTree {
            roots,
            layout_size,
            material_list: file.material_list,
            material_animations: HashMap::new(),
            animation_binding_revision: 0,
            file_name,
            main_bntx,
            sub_bntxs,
            parent_map: HashMap::new(),
            label_map: HashMap::new(),
            material_users: HashMap::new(),
            material_dirty: HashSet::new(),
            needs_rebatch: false,
            next_id,
            user_data: file.user_data,
            texture_list: file.texture_list,
            font_list: file.font_list,
            capture_texture_list: file.capture_texture_list,
            group: file.root_group,
            control_source: file.control_source,
            path_map: HashMap::new(),
        };
        tree.rebuild_maps();
        tree
    }
}

struct Builder<'a> {
    material_list: Option<&'a MaterialList>,
    sub_material_list: Option<MaterialList>,
    font_list: Option<&'a FontList>,
    blarc_dir: Option<&'a Path>,
    archive_entries: Option<&'a [ArchiveEntry]>,
    archive_layouts: &'a [(String, Vec<u8>)],
    blarc_cache: &'a mut HashMap<String, Option<Bflyt>>,
    main_bntx: Option<&'a Bntx>,
    sub_bntxs: &'a mut Vec<Bntx>,
    parts_chain: Vec<String>,
    next_pane_idx: u32,

    active_sarc_key: Option<(std::path::PathBuf, Vec<usize>)>,
    active_all_files: Vec<MagicFiles>,
    active_sarc_bntxs_parsed: bool,

    glyphs: &'a mut GlyphData,
}

const MAX_PARTS_DEPTH: usize = 8;

impl<'a> Builder<'a> {
    pub fn build_nodes(
        &mut self,
        nodes: &[BflytNode],
        parent_center: Vector2f,
        parent_size: Vector2f,
        parent_scale: Vector2f,
        parent_visible: bool,
        depth: usize,
    ) -> Vec<PaneNode> {
        let mut out = Vec::new();

        for node in nodes.iter() {
            match node {
                BflytNode::Pane(pane_el) => {
                    if let Some(mut pane_node) = self.build_node(
                        &pane_el.data,
                        parent_center,
                        parent_size,
                        parent_scale,
                        parent_visible,
                        depth,
                    ) {
                        pane_node.user_data = pane_el.user_data.clone();

                        let mut current_visible = parent_visible;
                        let mut current_scale = parent_scale;

                        if let Some(base) = pane_el.data.get_base_pane() {
                            current_visible = parent_visible && base.pane_flags.is_visible;
                            current_scale = Vector2f {
                                x: base.scale.x * parent_scale.x,
                                y: base.scale.y * parent_scale.y,
                            };
                        }

                        if !pane_el.children.is_empty() {
                            pane_node.children = self.build_nodes(
                                &pane_el.children,
                                pane_node.world_center,
                                pane_node.world_size,
                                current_scale,
                                current_visible,
                                depth + 1,
                            );
                        }

                        out.push(pane_node);
                    }
                }

                BflytNode::Group(_) => {}
                BflytNode::ControlSource(_) => {}
                BflytNode::RootSection(_) => {}
            }
        }

        out
    }

    fn build_node(
        &mut self,
        section: &BflytSection,
        parent_center: Vector2f,
        parent_size: Vector2f,
        parent_scale: Vector2f,
        parent_visible: bool,
        depth: usize,
    ) -> Option<PaneNode> {
        puffin::profile_function!();
        let base = section.get_base_pane()?;

        let is_visible = parent_visible && base.pane_flags.is_visible;
        let (pos, size, anchor, center) = resolve_rect(
            base,
            &PaneAnimation::default().resolve_transform(base),
            parent_center,
            parent_size,
            parent_scale,
        );

        let pane_idx = PaneId(self.next_pane_idx);
        self.next_pane_idx += 1;

        let bntxs: Vec<&Bntx> = self
            .main_bntx
            .into_iter()
            .chain(self.sub_bntxs.iter())
            .collect();

        let sources = VisualSources {
            material_list: self.sub_material_list.as_ref().or(self.material_list),
            font_list: self.font_list,
            bntxs: &bntxs,
        };

        let placement = PanePlacement {
            position: pos,
            size,
            center,
            rotation: base.rotation,
            visible: is_visible,
        };

        let visuals = build_pane_visuals(section, &placement, pane_idx, &sources, self.glyphs);

        let is_parts_root = base.pane_name == "RootPane" && !self.parts_chain.is_empty();

        let mut node = assemble_node(
            section,
            &placement,
            anchor,
            pane_idx,
            visuals,
            NodeOrigin {
                depth,
                parts_source: self.parts_chain.last().cloned(),
                is_parts_root,
            },
        );

        if let BflytSection::PartsPane(parts) = section {
            self.resolve_parts(parts, &mut node, is_visible);
        }

        Some(node)
    }

    fn load_bflyt_from_archive_index(&mut self, layout_name: &str) -> Option<Vec<MagicFiles>> {
        puffin::profile_function!();
        let entries = self.archive_entries.as_ref()?;
        let entry = entries
            .iter()
            .find(|e| e.matches_layout_name(layout_name))?;

        let current_key = (entry.path.clone(), entry.nested_path.clone());

        if self.active_sarc_key.as_ref() != Some(&current_key) || self.active_all_files.is_empty() {
            puffin::profile_scope!("sarc_heavy_extraction_once");

            let bytes = std::fs::read(&entry.path).ok()?;
            let parent_package_bytes = resolve_nested_package_bytes(&bytes, &entry.nested_path)?;

            let mut all_files = Vec::new();

            let file_name = entry
                .path
                .file_name()
                .map(|f| f.to_string_lossy().to_string());

            extract_all_files_recursive(parent_package_bytes, &mut all_files, file_name);

            self.active_sarc_key = Some(current_key);
            self.active_all_files = all_files;
            self.active_sarc_bntxs_parsed = false;
        }

        let target_bflyt = match self.active_all_files.get(entry.file_idx)? {
            MagicFiles::Bflyt(_, b) => b.clone(),
            _ => return None,
        };

        let mut final_files = vec![MagicFiles::Bflyt(
            Some(layout_name.to_string()),
            target_bflyt,
        )];

        if !self.active_sarc_bntxs_parsed {
            for file in &self.active_all_files {
                if !matches!(file, MagicFiles::Bflyt(_, _)) {
                    final_files.push(file.clone());
                }
            }
        }

        Some(final_files)
    }

    fn resolve_parts(
        &mut self,
        parts: &PartsPane,
        parent_node: &mut PaneNode,
        parent_visible: bool,
    ) {
        puffin::profile_function!();
        let layout_name = parts.o_layout_name.trim_end_matches('\0');
        if layout_name.is_empty() {
            return;
        }

        if self.parts_chain.len() >= MAX_PARTS_DEPTH {
            log::warn!("PartsPane: Depth went beyond max");
            return;
        }

        if self
            .parts_chain
            .iter()
            .any(|l| l.eq_ignore_ascii_case(layout_name))
        {
            log::warn!("PartsPane: '{layout_name}' includes itself, not expanded");
            return;
        }

        if !self.blarc_cache.contains_key(layout_name) {
            let in_archive = self
                .archive_layouts
                .iter()
                .find(|(stem, _)| stem.eq_ignore_ascii_case(layout_name))
                .map(|(stem, bytes)| vec![MagicFiles::Bflyt(Some(stem.clone()), bytes.clone())]);

            let assets = in_archive
                .or_else(|| {
                    self.blarc_dir
                        .and_then(|dir| load_bflyt_from_blarc_dir(dir, layout_name))
                })
                .or_else(|| self.load_bflyt_from_archive_index(layout_name));

            if let Some(assets) = assets {
                let bflyts: Vec<_> = assets
                    .iter()
                    .filter_map(|f| match f {
                        MagicFiles::Bflyt(name, bytes) => Some((name.as_deref(), bytes)),
                        _ => None,
                    })
                    .collect();

                let bflyt_res = bflyts
                    .iter()
                    .find(|(name, _)| {
                        name.is_some_and(|n| file_stem(n).eq_ignore_ascii_case(layout_name))
                    })
                    .or(bflyts.first())
                    .and_then(|(_, bytes)| Bflyt::parse_file(bytes).ok());

                if let Some(sub_bflyt) = bflyt_res {
                    // TODO: make this work properly
                    // if !self.active_sarc_bntxs_parsed {
                    puffin::profile_scope!("parts_parse_bntxs");

                    let parsed_bntxs: Vec<_> = assets
                        .par_iter()
                        .filter_map(|asset| {
                            if let MagicFiles::Bntx(_, bntx_data) = asset {
                                match Bntx::parse(bntx_data) {
                                    Ok(bntx) => Some(bntx),
                                    Err(e) => {
                                        log::error!("TextureCache: failed to parse BNTX: {e}");
                                        None
                                    }
                                }
                            } else {
                                None
                            }
                        })
                        .collect();

                    self.sub_bntxs.extend(parsed_bntxs);
                    self.active_sarc_bntxs_parsed = true;
                    // }

                    self.blarc_cache
                        .insert(layout_name.to_string(), Some(sub_bflyt));
                }
            }
        }

        let Some(Some(sub_bflyt)) = self.blarc_cache.get(layout_name) else {
            log::warn!("PartsPane: could not load '{layout_name}'");
            return;
        };

        let scale = Vector2f {
            x: parts.base.scale.x * parts.magnify_x,
            y: parts.base.scale.y * parts.magnify_y,
        };

        let sub_size = Vector2f {
            x: sub_bflyt.layout.width,
            y: sub_bflyt.layout.height,
        };

        let old_sub_material_list = self.sub_material_list.take();
        self.parts_chain.push(layout_name.to_string());

        let sub_nodes = sub_bflyt.nodes.clone();
        self.sub_material_list = sub_bflyt.material_list.clone();

        let mut sub_children = self.build_nodes(
            &sub_nodes,
            parent_node.world_center,
            sub_size * scale,
            scale,
            parent_visible,
            parent_node.depth + 1,
        );

        self.sub_material_list = old_sub_material_list;

        for prop in &parts.properties {
            let prop_name = prop.property_name.trim_end_matches('\0');
            if prop_name.is_empty() {
                continue;
            }

            if let Some(info) = &prop.o_basic_info {
                fn find_mut<'a>(nodes: &'a mut [PaneNode], name: &str) -> Option<&'a mut PaneNode> {
                    for node in nodes.iter_mut() {
                        if node.label.trim_end_matches('\0') == name {
                            return Some(node);
                        }

                        if let Some(found) = find_mut(&mut node.children, name) {
                            return Some(found);
                        }
                    }

                    None
                }

                if let Some(target) = find_mut(&mut sub_children, prop_name) {
                    apply_basic_info_override(target, &prop.basic_usage_flag, info);
                } else {
                    log::warn!(
                        "PartsPane '{layout_name}': override target '{prop_name}' not found"
                    );
                }
            }

            let Some(override_section) = &prop.o_section else {
                continue;
            };

            fn apply_override(
                nodes: &mut [PaneNode],
                prop_name: &str,
                override_section: &BflytSection,
                builder: &Builder,
            ) {
                for node in nodes.iter_mut() {
                    if node.label.trim_end_matches('\0') == prop_name {
                        // How should the section actually be overriden, as I don't think it's a full overwrite?
                        if let BflytSection::PicturePane(override_pic) = override_section
                            && let BflytSection::PicturePane(pic) = &mut node.section
                        {
                            let base = pic.base.clone();

                            *pic = (*override_pic).clone();
                            pic.base = base;
                        };

                        if let BflytSection::PicturePane(pic) = override_section {
                            let bntxs: Vec<&Bntx> = builder
                                .main_bntx
                                .into_iter()
                                .chain(builder.sub_bntxs.iter())
                                .collect();

                            let sources = VisualSources {
                                material_list: builder
                                    .sub_material_list
                                    .as_ref()
                                    .or(builder.material_list),
                                font_list: builder.font_list,
                                bntxs: &bntxs,
                            };

                            let placement = PanePlacement {
                                position: node.world_pos,
                                size: node.world_size,
                                center: node.world_center,
                                rotation: node
                                    .section
                                    .get_base_pane()
                                    .map(|b| b.rotation)
                                    .unwrap_or_default(),
                                visible: node.visible,
                            };

                            node.textured_quad =
                                picture_quad(pic, &placement, node.pane_idx, &sources);
                        }

                        return;
                    }

                    apply_override(&mut node.children, prop_name, override_section, builder);
                }
            }

            apply_override(&mut sub_children, prop_name, override_section, self);
        }

        parent_node.parts_children.extend(sub_children);

        self.parts_chain.pop();
    }
}

pub struct VisualSources<'a> {
    pub material_list: Option<&'a MaterialList>,
    pub font_list: Option<&'a FontList>,
    pub bntxs: &'a [&'a Bntx],
}

pub struct PanePlacement {
    pub position: Vector2f,
    pub size: Vector2f,
    pub center: Vector2f,
    pub rotation: Vector3f,
    pub visible: bool,
}

pub struct NodeOrigin {
    pub depth: usize,
    pub parts_source: Option<String>,
    pub is_parts_root: bool,
}

pub fn assemble_node(
    section: &BflytSection,
    placement: &PanePlacement,
    anchor: Vector2f,
    pane_idx: PaneId,
    visuals: PaneVisuals,
    origin: NodeOrigin,
) -> PaneNode {
    let default_base = Pane::default();
    let base = section.get_base_pane().unwrap_or(&default_base);

    let corners = Corners::compute(
        placement.center,
        placement.size,
        &base.position.position_x,
        &base.position.position_y,
        placement.rotation,
    );

    let plain_quad = Quad {
        corners: corners.to_array(),
        width: placement.size.x,
        height: placement.size.y,
        color: if placement.visible {
            section.section_color()
        } else {
            [0.0; 4]
        },
        has_textured: matches!(
            section,
            BflytSection::PicturePane(_)
                | BflytSection::WindowPane(_)
                | BflytSection::TextBoxPane(_)
        ),
        is_parts_root: origin.is_parts_root,
        pane_idx,
    };

    let handle_capabilities = match section {
        BflytSection::WindowPane(win)
            if matches!(
                win.flag.window_kind,
                WindowKind::HorizontalNoContent | WindowKind::Horizontal
            ) =>
        {
            vec![HandleCapability::ScaleHorizontal, HandleCapability::Rotate]
        }
        _ => vec![
            HandleCapability::Rotate,
            HandleCapability::ScaleHorizontal,
            HandleCapability::ScaleVertical,
        ],
    };

    PaneNode {
        section: section.clone(),
        kind: section.kind_name().to_string(),
        label: section.pane_name(),
        depth: origin.depth,
        visible: placement.visible,
        parts_source: origin.parts_source,
        is_parts_overridden: false,
        pane_idx,
        world_pos: placement.position,
        world_size: placement.size,
        world_center: placement.center,
        parent_anchor: anchor,
        world_corners: corners,
        world_rotation: placement.rotation,
        textured_quad: visuals.textured_quad,
        animation: PaneAnimation::default(),
        window_quads: visuals.window_quads,
        glyph_quads: visuals.glyph_quads,
        plain_quad,
        dirty: DirtyFlags::empty(),
        children: Vec::new(),
        parts_children: Vec::new(),
        user_data: None,
        handle_capabilities,
    }
}

#[derive(Default)]
pub struct PaneVisuals {
    pub textured_quad: Option<TexturedQuad>,
    pub window_quads: Vec<TexturedQuad>,
    pub glyph_quads: Vec<TexturedQuad>,
}

pub fn build_pane_visuals(
    section: &BflytSection,
    placement: &PanePlacement,
    pane_idx: PaneId,
    sources: &VisualSources,
    glyphs: &mut GlyphData,
) -> PaneVisuals {
    puffin::profile_function!();
    match section {
        BflytSection::PicturePane(pic) => PaneVisuals {
            textured_quad: picture_quad(pic, placement, pane_idx, sources),
            ..Default::default()
        },
        BflytSection::WindowPane(win) => PaneVisuals {
            window_quads: window_quads(win, placement, pane_idx, sources),
            ..Default::default()
        },
        BflytSection::TextBoxPane(txt) => PaneVisuals {
            glyph_quads: build_text_glyph_quads(
                txt,
                placement.center,
                placement.size,
                placement.rotation,
                placement.visible,
                pane_idx,
                sources.font_list,
                sources.material_list,
                glyphs,
            ),
            ..Default::default()
        },
        _ => PaneVisuals::default(),
    }
}

fn picture_quad(
    pic: &PicturePane,
    placement: &PanePlacement,
    pane_idx: PaneId,
    sources: &VisualSources,
) -> Option<TexturedQuad> {
    if sources.bntxs.is_empty() {
        return None;
    }

    let mat = sources
        .material_list?
        .materials
        .get(pic.material_index as usize)?;

    let corners = Corners::compute(
        placement.center,
        placement.size,
        &pic.base.position.position_x,
        &pic.base.position.position_y,
        placement.rotation,
    );

    TexturedQuad::derive_from_material(
        MaterialPaneData {
            base_section: &pic.base,
            corner_tints: vertex_corners_color_to_corner_tints(
                &pic.top_left_vertex_color,
                &pic.top_right_vertex_color,
                &pic.bottom_left_vertex_color,
                &pic.bottom_right_vertex_color,
            ),
            piece_id: 0,
            material_idx: pic.material_index,
            texture_uvs: &pic.texture_uvs,
            rotation: placement.rotation,
        },
        mat,
        placement.position,
        placement.size,
        corners.to_array(),
        placement.visible,
        pane_idx,
        None,
    )
}

fn window_quads(
    win: &WindowPane,
    placement: &PanePlacement,
    pane_idx: PaneId,
    sources: &VisualSources,
) -> Vec<TexturedQuad> {
    let (false, Some(material_list)) = (sources.bntxs.is_empty(), sources.material_list) else {
        return Vec::new();
    };

    let corners = Corners::compute(
        placement.center,
        placement.size,
        &win.base.position.position_x,
        &win.base.position.position_y,
        placement.rotation,
    );

    derive_from_window(
        win,
        material_list,
        corners.to_array(),
        placement.visible,
        pane_idx,
        sources.bntxs,
    )
}

fn resolve_rect(
    pane: &Pane,
    transform: &ResolvedPaneTransform,
    parent_center: Vector2f,
    parent_size: Vector2f,
    parent_scale: Vector2f,
) -> (Vector2f, Vector2f, Vector2f, Vector2f) {
    let anchor_x = match pane.position.parent_relative_position_x {
        HorizontalPosition::Center => parent_center.x,
        HorizontalPosition::Left => parent_center.x - parent_size.x * 0.5,
        HorizontalPosition::Right => parent_center.x + parent_size.x * 0.5,
    };

    let anchor_y = match pane.position.parent_relative_position_y {
        VerticalPosition::Center => parent_center.y,
        VerticalPosition::Top => parent_center.y - parent_size.y * 0.5,
        VerticalPosition::Bottom => parent_center.y + parent_size.y * 0.5,
    };

    let cx = anchor_x + transform.translation.x * parent_scale.x;
    let cy = anchor_y - transform.translation.y * parent_scale.y;

    let w = transform.size.x * transform.scale.x * parent_scale.x;
    let h = transform.size.y * transform.scale.y * parent_scale.y;

    let tl_x = match pane.position.position_x {
        HorizontalPosition::Center => cx - w * 0.5,
        HorizontalPosition::Left => cx,
        HorizontalPosition::Right => cx - w,
    };

    let tl_y = match pane.position.position_y {
        VerticalPosition::Center => cy - h * 0.5,
        VerticalPosition::Top => cy,
        VerticalPosition::Bottom => cy - h,
    };

    (
        Vector2f { x: tl_x, y: tl_y },
        Vector2f {
            x: w.abs().max(1.0),
            y: h.abs().max(1.0),
        },
        Vector2f {
            x: anchor_x,
            y: anchor_y,
        },
        Vector2f { x: cx, y: cy },
    )
}

fn load_bflyt_from_blarc_dir(blarc_dir: &Path, layout_name: &str) -> Option<Vec<MagicFiles>> {
    puffin::profile_function!();
    let entry_path = std::fs::read_dir(blarc_dir).ok()?.find_map(|e| {
        let e = e.ok()?;
        let path = e.path();
        let fname = path.file_name()?.to_string_lossy().to_lowercase();

        if fname.split('.').next() != Some(layout_name.to_lowercase().as_str()) {
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

    let mut all_files = Vec::new();
    extract_all_files_recursive(bytes, &mut all_files, Some(filename.to_string()));

    let has_bflyt = all_files
        .iter()
        .any(|f| matches!(f, MagicFiles::Bflyt(_, _)));
    if !has_bflyt {
        return None;
    }

    Some(all_files)
}

pub fn apply_basic_info_override(
    node: &mut PaneNode,
    flags: &BasePaneUsageFlags,
    info: &PartsPaneBasicInfo,
) {
    node.is_parts_overridden = true;

    if flags.is_visible_set {
        node.visible = flags.is_visible_true;
    }

    if let Some(base) = node.section.get_base_pane_mut() {
        if flags.is_visible_set {
            base.pane_flags.is_visible = flags.is_visible_true;
        }

        if flags.has_translate {
            base.translation.x = info.translation_x;
            base.translation.y = info.translation_y;
            base.translation.z = info.translation_z;
        }

        if flags.has_size {
            base.size.x = info.size_x;
            base.size.y = info.size_y;
        }

        if flags.has_scale {
            base.scale.x = info.scale_x;
            base.scale.y = info.scale_y;
        }

        if flags.has_rotate {
            base.rotation.x = info.rotation_x;
            base.rotation.y = info.rotation_y;
            base.rotation.z = info.rotation_z;
        }

        if flags.has_alpha {
            base.alpha = info.pane_alpha;
        }
    }

    node.mark_transform_dirty();
}

#[allow(clippy::too_many_arguments)]
fn build_text_glyph_quads(
    txt: &TextBoxPane,
    world_center: Vector2f,
    world_size: Vector2f,
    world_rotation: Vector3f,
    is_visible: bool,
    pane_idx: PaneId,
    font_list: Option<&FontList>,
    material_list: Option<&MaterialList>,
    glyphs: &mut GlyphData,
) -> Vec<TexturedQuad> {
    puffin::profile_function!();
    let mut quads = Vec::new();

    let Some(text) = txt.text.as_deref().filter(|t| !t.is_empty()) else {
        return quads;
    };

    let Some(mat) = material_list.and_then(|list| list.materials.get(txt.material_index as usize))
    else {
        return quads;
    };

    let font_name = font_list
        .and_then(|list| list.fonts.get(txt.font_index as usize))
        .map(String::as_str)
        .unwrap_or("");

    let Some((font_id, font)) = glyphs.fonts.get_or_load(font_name) else {
        return quads;
    };

    const FONT_SCALE_BASE_SIZE: f32 = 36.0;
    let font_size_px = if txt.text_flags.is_keeping_font_scale {
        FONT_SCALE_BASE_SIZE * txt.font_size.y
    } else {
        txt.font_size.y
    };

    let scale = font_size_px / font.units_per_em.max(1.0);
    let line_height =
        (font.ascender - font.descender + font.line_gap) * font_size_px + txt.line_space;

    let box_w = world_size.x;
    let box_h = world_size.y;
    let box_lx = match txt.base.position.position_x {
        HorizontalPosition::Center => -box_w * 0.5,
        HorizontalPosition::Left => 0.0,
        HorizontalPosition::Right => -box_w,
    };
    let box_ly = match txt.base.position.position_y {
        VerticalPosition::Center => -box_h * 0.5,
        VerticalPosition::Top => 0.0,
        VerticalPosition::Bottom => -box_h,
    };

    let lines: Vec<_> = text.split('\n').collect();
    let line_widths: Vec<_> = lines
        .iter()
        .map(|line| measure_line_width(font, line, font_size_px, txt.character_space))
        .collect();

    let total_text_height = line_height * lines.len() as f32;
    let start_y = box_ly
        + match txt.text_origin.vertical {
            VerticalPosition::Top => 0.0,
            VerticalPosition::Center => (box_h - total_text_height) * 0.5,
            VerticalPosition::Bottom => box_h - total_text_height,
        };

    let corner_tints = vertex_corners_color_to_corner_tints(
        &txt.font_top_color,
        &txt.font_top_color,
        &txt.font_bottom_color,
        &txt.font_bottom_color,
    );

    let mut next_piece_id: usize = 1;

    for (line_idx, line) in lines.iter().enumerate() {
        let start_x = box_lx
            + match txt.text_alignment {
                TextAlignment::Left | TextAlignment::Synchronous => 0.0,
                TextAlignment::Center => (box_w - line_widths[line_idx]) * 0.5,
                TextAlignment::Right => box_w - line_widths[line_idx],
            };

        let mut pen_x = start_x;
        let pen_y = start_y + line_idx as f32 * line_height + font.ascender * scale;
        let mut prev_glyph = None;

        for ch in line.chars() {
            let Some(glyph_id) = font.glyph_id_for_char(ch) else {
                prev_glyph = None;
                continue;
            };

            if ch != ' '
                && let Some(atlas_glyph) = glyphs.atlas.glyph(font, font_id, glyph_id, font_size_px)
                && atlas_glyph.width > 0.0
                && atlas_glyph.height > 0.0
            {
                let local_left = pen_x + atlas_glyph.bearing_x;
                let local_top = pen_y - atlas_glyph.bearing_y - atlas_glyph.height;
                let local_right = local_left + atlas_glyph.width;
                let local_bottom = local_top + atlas_glyph.height;

                let corners = [
                    Corners::rotate_point(
                        Vector2f::new(local_left, local_top),
                        world_center,
                        world_rotation,
                    ),
                    Corners::rotate_point(
                        Vector2f::new(local_right, local_top),
                        world_center,
                        world_rotation,
                    ),
                    Corners::rotate_point(
                        Vector2f::new(local_left, local_bottom),
                        world_center,
                        world_rotation,
                    ),
                    Corners::rotate_point(
                        Vector2f::new(local_right, local_bottom),
                        world_center,
                        world_rotation,
                    ),
                ];

                let piece_id = next_piece_id;
                next_piece_id += 1;

                if let Some(tq) = TexturedQuad::derive_from_material(
                    MaterialPaneData {
                        base_section: &txt.base,
                        corner_tints,
                        piece_id,
                        material_idx: txt.material_index,
                        texture_uvs: &[],
                        rotation: world_rotation,
                    },
                    mat,
                    Vector2f::new(local_left, local_top),
                    Vector2f::new(atlas_glyph.width, atlas_glyph.height),
                    corners.map(|c| c.into()),
                    is_visible,
                    pane_idx,
                    Some(GlyphQuadOverride {
                        texture_name: GLYPH_ATLAS_TEXTURE_NAME,
                        uvs: [
                            atlas_glyph.uv_min,
                            [atlas_glyph.uv_max[0], atlas_glyph.uv_min[1]],
                            [atlas_glyph.uv_min[0], atlas_glyph.uv_max[1]],
                            atlas_glyph.uv_max,
                        ],
                    }),
                ) {
                    quads.push(tq);
                }
            }

            let advance = font.advance_width(glyph_id, font_size_px)
                + prev_glyph
                    .map(|prev| font.kerning(prev, glyph_id, font_size_px))
                    .unwrap_or(0.0)
                + txt.character_space;

            pen_x += advance;
            prev_glyph = Some(glyph_id);
        }
    }

    quads
}

fn measure_line_width(font: &FontFace, line: &str, font_size_px: f32, character_space: f32) -> f32 {
    let mut width = 0.0;
    let mut prev_glyph = None;
    let mut any_glyph = false;

    for ch in line.chars() {
        let Some(glyph_id) = font.glyph_id_for_char(ch) else {
            prev_glyph = None;
            continue;
        };

        width += font.advance_width(glyph_id, font_size_px)
            + prev_glyph
                .map(|prev| font.kerning(prev, glyph_id, font_size_px))
                .unwrap_or(0.0)
            + character_space;

        prev_glyph = Some(glyph_id);
        any_glyph = true;
    }

    if any_glyph {
        width -= character_space;
    }

    width.max(0.0)
}

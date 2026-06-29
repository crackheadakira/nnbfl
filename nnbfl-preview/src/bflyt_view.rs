use std::path::Path;

use nnbfl::bflyt::file::Bflyt;

use crate::pane_tree::{DirtyFlags, PaneTree};

pub struct BflytView {
    pub tree: PaneTree,
    pub layout_width: f32,
    pub layout_height: f32,
    pub file_name: String,
}

impl BflytView {
    pub fn reset_to_base(&mut self) {
        for node in self.tree.iter_mut() {
            node.textured_quad = node.base_textured_quad.clone();
            node.dirty
                .insert(DirtyFlags::TRANSFORM | DirtyFlags::MATERIAL | DirtyFlags::VERTICES);
        }

        self.tree.recompute_dirty();
    }

    pub fn descendants(&self, pane_idx: usize) -> Vec<usize> {
        self.tree.descendants(pane_idx)
    }
}

pub fn build_view(
    file: Bflyt,
    blarc_dir: Option<&Path>,
    file_name: String,
    has_bntx: bool,
) -> BflytView {
    let layout_width = file.layout.width;
    let layout_height = file.layout.height;

    let tree = PaneTree::from_bflyt(file, blarc_dir, file_name.clone(), has_bntx);

    BflytView {
        tree,
        layout_width,
        layout_height,
        file_name,
    }
}

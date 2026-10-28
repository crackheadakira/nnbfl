use std::collections::HashSet;

use crate::{
    LayoutData,
    pane_tree::PaneId,
    renderer::{texture::TextureCache, textured_quad::PaneRenderer},
    ui::general::PaneVisibilityFlags,
};

pub struct FrameView<'a> {
    pub selected: Option<PaneId>,
    pub hidden: &'a HashSet<PaneId>,
    pub flags: PaneVisibilityFlags,
}

pub fn prepare_frame(
    renderer: &mut PaneRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    cache: &TextureCache,
    layout: &mut LayoutData,
    view: &FrameView,
) {
    puffin::profile_function!();
    layout.timeline.anim_player.apply(&mut layout.tree);

    let layout_size = layout.tree.layout_size;

    if layout.tree.take_rebatch() {
        let quads = layout.tree.collect_render_quads();
        renderer.upload_quads(device, &quads, cache, layout_size, &view.flags);
    }

    let mut quads = layout.tree.collect_render_quads();

    renderer.update_visuals(
        device,
        queue,
        &mut quads,
        view.selected,
        view.hidden,
        view.flags,
        cache,
        layout_size,
    );
}

use std::collections::HashMap;

use nnbfl::ui2d::userdata::ResUi2dUserDataInner;
use nnbfl::{
    bflan::{
        anim_info::{AnimContent, AnimInfo, AnimInfoType, AnimType, PaneAnimInfo},
        curves::Curve,
        file::{Bflan, BflanSections},
        targets::{
            IndirectSrtTarget, PaneSrtTarget, TargetIndex, TextureSrtTarget, VertexColorTarget,
        },
    },
    bflyt::{file::BflytSection, flags::BflytOrigin},
};

use crate::bflyt_view::BflytView;

fn eval_hermite(keys: &[nnbfl::bflan::curves::HermiteKey], frame: f32) -> f32 {
    if keys.is_empty() {
        return 0.0;
    }
    if frame <= keys[0].frame {
        return keys[0].value;
    }
    if frame >= keys[keys.len() - 1].frame {
        return keys[keys.len() - 1].value;
    }
    let idx = keys.partition_point(|k| k.frame <= frame) - 1;
    let k0 = &keys[idx];
    let k1 = &keys[idx + 1];
    let dt = k1.frame - k0.frame;
    let t = (frame - k0.frame) / dt;
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    h00 * k0.value + h10 * dt * k0.slope + h01 * k1.value + h11 * dt * k1.slope
}

pub fn eval_curve(curve: &Curve, frame: f32) -> f32 {
    match curve {
        Curve::Constant(keys) => {
            let idx = (frame as usize).min(keys.len().saturating_sub(1));
            keys.get(idx).copied().unwrap_or(0.0)
        }
        Curve::Step(keys) => {
            if keys.is_empty() {
                return 0.0;
            }
            let idx = keys.partition_point(|k| k.frame <= frame).saturating_sub(1);
            keys[idx].value as f32
        }
        Curve::Hermite(keys) => eval_hermite(keys, frame),
    }
}

pub fn eval_curve_step_u16(curve: &Curve, frame: f32) -> u16 {
    match curve {
        Curve::Step(keys) => {
            if keys.is_empty() {
                return 0;
            }
            let idx = keys.partition_point(|k| k.frame <= frame).saturating_sub(1);
            keys[idx].value
        }
        _ => eval_curve(curve, frame) as u16,
    }
}

pub struct AnimInstance {
    pub bflan: Bflan,
    pub name: String,
    pub frame: f32,
    pub playing: bool,
    pub next_anim: Option<String>,
}

impl AnimInstance {
    pub fn new(file_name: String, bflan: Bflan) -> Self {
        let next_anim = find_next_anim(&bflan);
        let name = bflan
            .sections
            .iter()
            .find_map(|s| {
                if let BflanSections::PaneAnimTag(tag) = s {
                    Some(tag.o_name.clone())
                } else {
                    None
                }
            })
            .unwrap_or(file_name);
        Self {
            bflan,
            name,
            frame: 0.0,
            playing: false,
            next_anim,
        }
    }

    pub fn frame_count(&self) -> f32 {
        self.bflan
            .sections
            .iter()
            .find_map(|s| {
                if let BflanSections::PaneAnimInfo(pai) = s {
                    Some(pai.frame_count as f32)
                } else {
                    None
                }
            })
            .unwrap_or(1.0)
    }

    pub fn is_looping(&self) -> bool {
        self.bflan.sections.iter().any(|s| {
            if let BflanSections::PaneAnimInfo(pai) = s {
                pai.is_looping
            } else {
                false
            }
        })
    }

    pub fn pane_anim_info(&self) -> Option<&PaneAnimInfo> {
        self.bflan.sections.iter().find_map(|s| {
            if let BflanSections::PaneAnimInfo(pai) = s {
                Some(pai)
            } else {
                None
            }
        })
    }
}

fn find_next_anim(bflan: &Bflan) -> Option<String> {
    for section in &bflan.sections {
        if let BflanSections::PaneAnimTag(tag) = section
            && let Some(ud) = &tag.user_data
        {
            for entry in &ud.user_data {
                if entry.o_name == "CommandPlayEnd_Play"
                    && let Some(ResUi2dUserDataInner::String(value)) = entry.data_array.first()
                {
                    return Some(value.clone());
                }
            }
        }
    }

    None
}

pub struct AnimPlayer {
    pub anims: Vec<AnimInstance>,
    pub active: Option<usize>,
}

impl AnimPlayer {
    pub fn new() -> Self {
        Self {
            anims: Vec::new(),
            active: None,
        }
    }

    pub fn load(&mut self, name: String, bflan: Bflan) {
        self.anims.push(AnimInstance::new(name, bflan));
    }

    pub fn play(&mut self, name: &str) {
        if let Some(idx) = self.anims.iter().position(|a| a.name == name) {
            if let Some(prev) = self.active
                && prev < self.anims.len()
            {
                self.anims[prev].playing = false;
            }

            self.anims[idx].frame = 0.0;
            self.anims[idx].playing = true;
            self.active = Some(idx);
        }
    }

    pub fn tick(&mut self, dt: f32, fps: f32) -> Option<String> {
        let idx = self.active?;
        let anim = &mut self.anims[idx];
        if !anim.playing {
            return None;
        }

        anim.frame += dt * fps;
        let frame_count = anim.frame_count();
        if anim.frame >= frame_count {
            if anim.is_looping() {
                anim.frame %= frame_count;
            } else {
                anim.frame = frame_count;
                anim.playing = false;
                return anim.next_anim.clone();
            }
        }
        None
    }

    pub fn apply(&self, view: &mut BflytView) {
        let Some(idx) = self.active else { return };
        let anim = &self.anims[idx];
        let Some(pai) = anim.pane_anim_info() else {
            return;
        };
        apply_anim(pai, anim.frame, view);
    }

    pub fn is_playing(&self) -> bool {
        self.active.map(|i| self.anims[i].playing).unwrap_or(false)
    }
}

fn apply_anim(pai: &PaneAnimInfo, frame: f32, view: &mut BflytView) {
    let pane_by_name: HashMap<String, usize> = view
        .panes
        .iter()
        .enumerate()
        .map(|(i, p)| (p.label.trim_end_matches('\0').to_string(), i))
        .collect();

    for content in &pai.contents {
        let name = content.name.trim_end_matches('\0');
        let Some(&pane_idx) = pane_by_name.get(name) else {
            continue;
        };

        match content.anim_type {
            AnimType::Pane | AnimType::PaneExt => {
                apply_pane_content(content, frame, pane_idx, view);
            }
            AnimType::Material => {
                apply_material_content(content, frame, pane_idx, view, pai);
            }
            _ => {}
        }
    }
}

fn pane_screen_pos(
    pane_info: &crate::bflyt_view::PaneInfo,
    trans_x: f32,
    trans_y: f32,
) -> (f32, f32) {
    let base = match &pane_info.section {
        BflytSection::Pane(p) => Some(p),
        BflytSection::PicturePane(p) => Some(&p.base),
        BflytSection::TextBoxPane(p) => Some(&p.base),
        BflytSection::PartsPane(p) => Some(&p.base),
        BflytSection::WindowPane(p) => Some(&p.base),
        BflytSection::AlignmentPane(p) => Some(&p.base),
        _ => None,
    };

    let (ox, oy, w, h) = base
        .map(|b| {
            (
                b.origin.origin_x,
                b.origin.origin_y,
                b.size.x * b.scale.x,
                b.size.y * b.scale.y,
            )
        })
        .unwrap_or((
            BflytOrigin::Center,
            BflytOrigin::Center,
            pane_info.width,
            pane_info.height,
        ));

    let cx = pane_info.parent_anchor_x + trans_x;
    let cy = pane_info.parent_anchor_y - trans_y;

    let tl_x = match ox {
        BflytOrigin::Center => cx - w * 0.5,
        BflytOrigin::LeftTop => cx,
        BflytOrigin::RightBottom => cx - w,
    };
    let tl_y = match oy {
        BflytOrigin::Center => cy - h * 0.5,
        BflytOrigin::LeftTop => cy,
        BflytOrigin::RightBottom => cy - h,
    };

    (tl_x, tl_y)
}

fn cascade_translate(view: &mut BflytView, pane_idx: usize, new_trans_x: f32, new_trans_y: f32) {
    let base_info = view.base_panes.get(pane_idx).cloned();
    let Some(base_info) = base_info else { return };

    let base_trans = match &base_info.section {
        BflytSection::Pane(p) => (p.translation.x, p.translation.y),
        BflytSection::PicturePane(p) => (p.base.translation.x, p.base.translation.y),
        BflytSection::TextBoxPane(p) => (p.base.translation.x, p.base.translation.y),
        BflytSection::PartsPane(p) => (p.base.translation.x, p.base.translation.y),
        BflytSection::WindowPane(p) => (p.base.translation.x, p.base.translation.y),
        BflytSection::AlignmentPane(p) => (p.base.translation.x, p.base.translation.y),
        _ => return,
    };

    let (new_x, new_y) = pane_screen_pos(&base_info, new_trans_x, new_trans_y);
    let (base_x, base_y) = pane_screen_pos(&base_info, base_trans.0, base_trans.1);
    let dx = new_x - base_x;
    let dy = new_y - base_y;

    let mut affected = vec![pane_idx];
    affected.extend(view.descendants(pane_idx));

    for idx in affected {
        let base_screen_x = view.base_panes.get(idx).map(|p| p.x).unwrap_or(0.0);
        let base_screen_y = view.base_panes.get(idx).map(|p| p.y).unwrap_or(0.0);

        let sx = base_screen_x + dx;
        let sy = base_screen_y + dy;

        if let Some(p) = view.panes.get_mut(idx) {
            p.x = sx;
            p.y = sy;
        }

        if let Some(q) = view.quads.get_mut(idx) {
            q.x = sx;
            q.y = sy;
        }

        if let Some(tq) = view.textured_quads.iter_mut().find(|tq| tq.pane_idx == idx) {
            tq.x = sx;
            tq.y = sy;

            let base_corners = view
                .base_textured_quads
                .iter()
                .find(|btq| btq.pane_idx == idx)
                .map(|btq| btq.corners)
                .unwrap_or(tq.corners);

            for (i, bc) in base_corners.iter().enumerate() {
                tq.corners[i] = [bc[0] + dx, bc[1] + dy];
            }
        }
    }
}

fn cascade_visibility(view: &mut BflytView, pane_idx: usize, visible: bool) {
    let mut affected = vec![pane_idx];
    affected.extend(view.descendants(pane_idx));

    let colors: Vec<[f32; 4]> = affected
        .iter()
        .map(|&idx| crate::bflyt_view::section_color_for_pane(idx, view))
        .collect();

    for (i, &idx) in affected.iter().enumerate() {
        if let Some(pane) = view.panes.get_mut(idx) {
            pane.visible = visible;
        }

        if let Some(q) = view.quads.get_mut(idx) {
            q.color = if visible { colors[i] } else { [0.0; 4] };
        }

        if let Some(tq) = view.textured_quads.iter_mut().find(|tq| tq.pane_idx == idx) {
            tq.standard_material.visible = if visible { 1 } else { 0 };
        }
    }
}

fn apply_pane_content(content: &AnimContent, frame: f32, pane_idx: usize, view: &mut BflytView) {
    for info in &content.infos {
        let AnimInfo::Standard { anim_type, targets } = info else {
            continue;
        };

        match anim_type {
            AnimInfoType::PaneSrtAnim => {
                let base = view.base_panes.get(pane_idx).cloned();
                let base_w = base.as_ref().map(|p| p.width).unwrap_or(0.0);
                let base_h = base.as_ref().map(|p| p.height).unwrap_or(0.0);

                let (base_tx, base_ty) = base
                    .as_ref()
                    .and_then(|p| match &p.section {
                        BflytSection::Pane(b) => Some((b.translation.x, b.translation.y)),
                        BflytSection::PicturePane(b) => {
                            Some((b.base.translation.x, b.base.translation.y))
                        }
                        BflytSection::TextBoxPane(b) => {
                            Some((b.base.translation.x, b.base.translation.y))
                        }
                        BflytSection::PartsPane(b) => {
                            Some((b.base.translation.x, b.base.translation.y))
                        }
                        BflytSection::WindowPane(b) => {
                            Some((b.base.translation.x, b.base.translation.y))
                        }
                        BflytSection::AlignmentPane(b) => {
                            Some((b.base.translation.x, b.base.translation.y))
                        }
                        _ => None,
                    })
                    .unwrap_or((0.0, 0.0));

                let mut new_tx = base_tx;
                let mut new_ty = base_ty;
                let mut scale_x = 1.0f32;
                let mut scale_y = 1.0f32;
                let mut new_w = base_w;
                let mut new_h = base_h;

                for t in targets {
                    let v = eval_curve(&t.curve, frame);
                    match &t.target {
                        TargetIndex::PaneSrt(PaneSrtTarget::TranslateX) => new_tx = v,
                        TargetIndex::PaneSrt(PaneSrtTarget::TranslateY) => new_ty = v,
                        TargetIndex::PaneSrt(PaneSrtTarget::ScaleX) => scale_x = v,
                        TargetIndex::PaneSrt(PaneSrtTarget::ScaleY) => scale_y = v,
                        TargetIndex::PaneSrt(PaneSrtTarget::SizeX) => new_w = v,
                        TargetIndex::PaneSrt(PaneSrtTarget::SizeY) => new_h = v,
                        _ => {}
                    }
                }

                cascade_translate(view, pane_idx, new_tx, new_ty);

                let final_w = new_w * scale_x;
                let final_h = new_h * scale_y;
                if (final_w - base_w).abs() > f32::EPSILON
                    || (final_h - base_h).abs() > f32::EPSILON
                {
                    if let Some(p) = view.panes.get_mut(pane_idx) {
                        p.width = final_w;
                        p.height = final_h;
                    }

                    if let Some(q) = view.quads.get_mut(pane_idx) {
                        q.width = final_w;
                        q.height = final_h;
                    }

                    if let Some(tq) = view
                        .textured_quads
                        .iter_mut()
                        .find(|tq| tq.pane_idx == pane_idx)
                    {
                        tq.width = final_w;
                        tq.height = final_h;
                    }
                }
            }

            AnimInfoType::VisibilityAnim => {
                for t in targets {
                    let visible = eval_curve_step_u16(&t.curve, frame) != 0;
                    cascade_visibility(view, pane_idx, visible);
                }
            }

            AnimInfoType::VertexColorAnim => {
                let has_own_tq = view.textured_quads.iter().any(|tq| tq.pane_idx == pane_idx);
                let apply_to: Vec<usize> = if has_own_tq {
                    vec![pane_idx]
                } else {
                    let mut v = vec![pane_idx];
                    v.extend(view.descendants(pane_idx));
                    v
                };

                for t in targets {
                    let v = eval_curve(&t.curve, frame) / 255.0;
                    for &idx in &apply_to {
                        let Some(tq) = view.textured_quads.iter_mut().find(|tq| tq.pane_idx == idx)
                        else {
                            continue;
                        };
                        match &t.target {
                            TargetIndex::VertexColor(VertexColorTarget::PaneAlpha) => {
                                tq.tint[3] = v;

                                for c in tq.corner_tints.iter_mut() {
                                    c[3] = v;
                                }
                            }
                            TargetIndex::VertexColor(VertexColorTarget::LeftTopRed) => {
                                tq.corner_tints[0][0] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::LeftTopGreen) => {
                                tq.corner_tints[0][1] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::LeftTopBlue) => {
                                tq.corner_tints[0][2] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::LeftTopAlpha) => {
                                tq.corner_tints[0][3] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::RightTopRed) => {
                                tq.corner_tints[1][0] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::RightTopGreen) => {
                                tq.corner_tints[1][1] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::RightTopBlue) => {
                                tq.corner_tints[1][2] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::RightTopAlpha) => {
                                tq.corner_tints[1][3] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::LeftBottomRed) => {
                                tq.corner_tints[2][0] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::LeftBottomGreen) => {
                                tq.corner_tints[2][1] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::LeftBottomBlue) => {
                                tq.corner_tints[2][2] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::LeftBottomAlpha) => {
                                tq.corner_tints[2][3] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::RightBottomRed) => {
                                tq.corner_tints[3][0] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::RightBottomGreen) => {
                                tq.corner_tints[3][1] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::RightBottomBlue) => {
                                tq.corner_tints[3][2] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::RightBottomAlpha) => {
                                tq.corner_tints[3][3] = v
                            }
                            _ => {}
                        }
                    }
                }
            }

            _ => {}
        }
    }
}

fn apply_tex_srts(tq: &mut crate::renderer::textured_quad::TexturedQuad) {
    for (i, srt) in tq.tex_srts.iter().enumerate() {
        let rad = srt.rotate.to_radians();
        let cos_r = rad.cos();
        let sin_r = rad.sin();

        for v_idx in 0..4 {
            let [base_u, base_v] = tq.base_uvs[v_idx][i];
            let centered_u = base_u - 0.5;
            let centered_v = base_v - 0.5;
            let scaled_u = centered_u * srt.scale_u;
            let scaled_v = centered_v * srt.scale_v;
            let rotated_u = scaled_u * cos_r - scaled_v * sin_r;
            let rotated_v = scaled_u * sin_r + scaled_v * cos_r;

            tq.uvs[v_idx][i][0] = rotated_u + 0.5 + srt.translate_u;
            tq.uvs[v_idx][i][1] = rotated_v + 0.5 + srt.translate_v;
        }
    }
}

fn apply_material_content(
    content: &AnimContent,
    frame: f32,
    pane_idx: usize,
    view: &mut BflytView,
    pai: &PaneAnimInfo,
) {
    for info in &content.infos {
        let AnimInfo::Standard { anim_type, targets } = info else {
            continue;
        };

        match anim_type {
            AnimInfoType::TextureSrtAnim => {
                let Some(tq) = view
                    .textured_quads
                    .iter_mut()
                    .find(|tq| tq.pane_idx == pane_idx)
                else {
                    continue;
                };

                for t in targets {
                    let v = eval_curve(&t.curve, frame);
                    let layer = t.layer as usize;
                    if layer >= tq.tex_srts.len() {
                        continue;
                    }

                    match &t.target {
                        TargetIndex::TextureSrt(TextureSrtTarget::TranslateU) => {
                            tq.tex_srts[layer].translate_u = v
                        }
                        TargetIndex::TextureSrt(TextureSrtTarget::TranslateV) => {
                            tq.tex_srts[layer].translate_v = v
                        }
                        TargetIndex::TextureSrt(TextureSrtTarget::Rotate) => {
                            tq.tex_srts[layer].rotate = v
                        }
                        TargetIndex::TextureSrt(TextureSrtTarget::ScaleU) => {
                            tq.tex_srts[layer].scale_u = v
                        }
                        TargetIndex::TextureSrt(TextureSrtTarget::ScaleV) => {
                            tq.tex_srts[layer].scale_v = v
                        }
                        _ => {}
                    }
                }
                apply_tex_srts(tq);
            }

            AnimInfoType::IndirectSrtAnim => {
                let Some(tq) = view
                    .textured_quads
                    .iter_mut()
                    .find(|tq| tq.pane_idx == pane_idx)
                else {
                    continue;
                };
                for t in targets {
                    let v = eval_curve(&t.curve, frame);

                    match &t.target {
                        TargetIndex::IndirectSrt(IndirectSrtTarget::Rotate) => {
                            let rad = v.to_radians();
                            tq.standard_material.indirect_mtx0[1] = -rad.sin();
                            tq.standard_material.indirect_mtx1[0] = rad.sin();
                        }
                        TargetIndex::IndirectSrt(IndirectSrtTarget::ScaleU) => {
                            tq.standard_material.indirect_mtx0[0] = v;
                        }
                        TargetIndex::IndirectSrt(IndirectSrtTarget::ScaleV) => {
                            tq.standard_material.indirect_mtx1[1] = v;
                        }
                        _ => {}
                    }
                }
            }

            AnimInfoType::TexturePatternAnim => {
                let Some(tq) = view
                    .textured_quads
                    .iter_mut()
                    .find(|tq| tq.pane_idx == pane_idx)
                else {
                    continue;
                };
                for t in targets {
                    let file_idx = eval_curve_step_u16(&t.curve, frame) as usize;
                    let Some(tex_name) = pai.textures.get(file_idx).cloned() else {
                        continue;
                    };

                    match t.layer {
                        0 => tq.texture_name = tex_name,
                        1 => tq.texture_name1 = Some(tex_name),
                        2 => tq.texture_name2 = Some(tex_name),
                        _ => {}
                    }
                }
            }

            AnimInfoType::MaterialColorAnim => {
                let Some(tq) = view
                    .textured_quads
                    .iter_mut()
                    .find(|tq| tq.pane_idx == pane_idx)
                else {
                    continue;
                };

                for t in targets {
                    let v = eval_curve(&t.curve, frame) / 255.0;
                    if let TargetIndex::MaterialColor(c) = &t.target {
                        use nnbfl::bflan::targets::MaterialColorTarget::*;
                        match c {
                            BufferRed => tq.standard_material.interpolate_offset[0] = v,
                            BufferGreen => tq.standard_material.interpolate_offset[1] = v,
                            BufferBlue => tq.standard_material.interpolate_offset[2] = v,
                            BufferAlpha => tq.standard_material.interpolate_offset[3] = v,
                            Constant0Red | Color0Red | Color1Red | Color2Red | Color3Red
                            | Color4Red => tq.standard_material.interpolate_width[0] = v,
                            Constant0Green | Color0Green | Color1Green | Color2Green
                            | Color3Green | Color4Green => {
                                tq.standard_material.interpolate_width[1] = v
                            }
                            Constant0Blue | Color0Blue | Color1Blue | Color2Blue | Color3Blue
                            | Color4Blue => tq.standard_material.interpolate_width[2] = v,
                            Constant0Alpha | Color0Alpha | Color1Alpha | Color2Alpha
                            | Color3Alpha | Color4Alpha => {
                                tq.standard_material.interpolate_width[3] = v
                            }
                        }
                    }
                }
            }

            AnimInfoType::VertexColorAnim => {
                let Some(tq) = view
                    .textured_quads
                    .iter_mut()
                    .find(|tq| tq.pane_idx == pane_idx)
                else {
                    continue;
                };

                for t in targets {
                    let v = eval_curve(&t.curve, frame);
                    match &t.target {
                        TargetIndex::VertexColor(VertexColorTarget::LeftTopRed) => {
                            tq.tint[0] = v / 255.0
                        }
                        TargetIndex::VertexColor(VertexColorTarget::LeftTopGreen) => {
                            tq.tint[1] = v / 255.0
                        }
                        TargetIndex::VertexColor(VertexColorTarget::LeftTopBlue) => {
                            tq.tint[2] = v / 255.0
                        }
                        TargetIndex::VertexColor(VertexColorTarget::LeftTopAlpha) => {
                            tq.tint[3] = v / 255.0
                        }
                        TargetIndex::VertexColor(VertexColorTarget::PaneAlpha) => {
                            tq.tint[3] = v / 255.0
                        }
                        _ => {}
                    }
                }
            }

            _ => {}
        }
    }
}

use std::collections::HashMap;

use nnbfl::bflan::{
    anim_info::{AnimContent, AnimInfo, AnimInfoType, AnimType, PaneAnimInfo},
    curves::Curve,
    file::{Bflan, BflanSections},
    targets::{PaneSrtTarget, TargetIndex, TextureSrtTarget, VertexColorTarget},
};
use nnbfl::ui2d::userdata::ResUi2dUserDataInner;

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

fn eval_step_f32(keys: &[nnbfl::bflan::curves::StepKey], frame: f32) -> f32 {
    if keys.is_empty() {
        return 0.0;
    }
    let idx = keys.partition_point(|k| k.frame <= frame).saturating_sub(1);
    keys[idx].value as f32
}

pub fn eval_curve(curve: &Curve, frame: f32) -> f32 {
    match curve {
        Curve::Constant(keys) => {
            let idx = (frame as usize).min(keys.len().saturating_sub(1));
            keys.get(idx).copied().unwrap_or(0.0)
        }
        Curve::Step(keys) => eval_step_f32(keys, frame),
        Curve::Hermite(keys) => eval_hermite(keys, frame),
    }
}

// TODO: MAKE CHILDREN FUCKING FOLLOW THE PARENT QUAD

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
        if let BflanSections::PaneAnimTag(tag) = section {
            if let Some(ud) = &tag.user_data {
                for entry in &ud.user_data {
                    if entry.o_name == "CommandPlayEnd_Play" {
                        if let Some(ResUi2dUserDataInner::String(value)) = entry.data_array.first()
                        {
                            return Some(value.clone());
                        }
                    }
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
            if let Some(prev) = self.active {
                if prev < self.anims.len() {
                    self.anims[prev].playing = false;
                }
            }
            self.anims[idx].frame = 0.0;
            self.anims[idx].playing = true;
            self.active = Some(idx);
        }
    }

    pub fn tick(&mut self, dt: f32, fps: f32) -> Option<String> {
        let Some(idx) = self.active else { return None };
        let anim = &mut self.anims[idx];
        if !anim.playing {
            return None;
        }

        anim.frame += dt * fps;

        let frame_count = anim.frame_count();
        if anim.frame >= frame_count {
            if anim.is_looping() {
                anim.frame = anim.frame % frame_count;
            } else {
                anim.frame = frame_count - 1.0;
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
        if let Some(idx) = self.active {
            self.anims[idx].playing
        } else {
            false
        }
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
        match content.anim_type {
            AnimType::Pane | AnimType::PaneExt => {
                let Some(&pane_idx) = pane_by_name.get(name) else {
                    continue;
                };
                apply_pane_content(content, frame, pane_idx, view);
            }
            AnimType::Material => {
                let Some(&pane_idx) = pane_by_name.get(name) else {
                    continue;
                };
                apply_material_content(content, frame, pane_idx, view, pai);
            }
            _ => {}
        }
    }
}

fn apply_pane_content(content: &AnimContent, frame: f32, pane_idx: usize, view: &mut BflytView) {
    for info in &content.infos {
        let AnimInfo::Standard { magic, targets } = info else {
            continue;
        };
        match magic {
            AnimInfoType::PaneSrtAnim => {
                for t in targets {
                    let v = eval_curve(&t.curve, frame);
                    let Some(_) = view.panes.get_mut(pane_idx) else {
                        continue;
                    };

                    let base_x = view.base_panes.get(pane_idx).map(|p| p.x).unwrap_or(0.0);
                    let base_y = view.base_panes.get(pane_idx).map(|p| p.y).unwrap_or(0.0);
                    match &t.target {
                        TargetIndex::PaneSrt(PaneSrtTarget::TranslateX) => {
                            if let Some(p) = view.panes.get_mut(pane_idx) {
                                p.x = base_x + v;
                            }
                            if let Some(q) = view.quads.get_mut(pane_idx) {
                                q.x = base_x + v;
                            }
                            if let Some(tq) = view
                                .textured_quads
                                .iter_mut()
                                .find(|tq| tq.pane_idx == pane_idx)
                            {
                                tq.x = base_x + v;
                            }
                        }
                        TargetIndex::PaneSrt(PaneSrtTarget::TranslateY) => {
                            if let Some(p) = view.panes.get_mut(pane_idx) {
                                p.y = base_y - v;
                            }
                            if let Some(q) = view.quads.get_mut(pane_idx) {
                                q.y = base_y - v;
                            }
                            if let Some(tq) = view
                                .textured_quads
                                .iter_mut()
                                .find(|tq| tq.pane_idx == pane_idx)
                            {
                                tq.y = base_y - v;
                            }
                        }
                        _ => {}
                    }
                }
            }
            AnimInfoType::VisibilityAnim => {
                for t in targets {
                    let v = eval_curve_step_u16(&t.curve, frame);
                    let col = crate::bflyt_view::section_color_for_pane(pane_idx, view);

                    if let Some(pane) = view.panes.get_mut(pane_idx) {
                        pane.visible = v != 0;
                    }

                    if let Some(q) = view.quads.get_mut(pane_idx) {
                        q.color = if v != 0 { col } else { [0.0; 4] };
                    }
                }
            }
            AnimInfoType::VertexColorAnim => {
                for t in targets {
                    let v = eval_curve(&t.curve, frame);
                    if let Some(tq) = view
                        .textured_quads
                        .iter_mut()
                        .find(|tq| tq.pane_idx == pane_idx)
                    {
                        match &t.target {
                            TargetIndex::VertexColor(VertexColorTarget::LeftTopRed) => {
                                tq.tint[0] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::LeftTopGreen) => {
                                tq.tint[1] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::LeftTopBlue) => {
                                tq.tint[2] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::LeftTopAlpha) => {
                                tq.tint[3] = v
                            }
                            TargetIndex::VertexColor(VertexColorTarget::PaneAlpha) => {
                                tq.tint[3] = v / 255.0
                            }
                            TargetIndex::Raw(16) => tq.tint[3] = v / 255.0,
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
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
        let AnimInfo::Standard { magic, targets } = info else {
            continue;
        };

        match magic {
            AnimInfoType::TextureSrtAnim => {
                if let Some(tq) = view
                    .textured_quads
                    .iter_mut()
                    .find(|tq| tq.pane_idx == pane_idx)
                {
                    for t in targets {
                        let v = eval_curve(&t.curve, frame);

                        let layer = t.layer as usize;
                        if layer >= tq.tex_srts.len() {
                            continue;
                        }

                        match &t.target {
                            TargetIndex::TextureSrt(TextureSrtTarget::TranslateU) => {
                                tq.tex_srts[layer].translate_u = v;
                            }
                            TargetIndex::TextureSrt(TextureSrtTarget::TranslateV) => {
                                tq.tex_srts[layer].translate_v = v;
                            }
                            TargetIndex::TextureSrt(TextureSrtTarget::Rotate) => {
                                tq.tex_srts[layer].rotate = v;
                            }
                            TargetIndex::TextureSrt(TextureSrtTarget::ScaleU) => {
                                tq.tex_srts[layer].scale_u = v;
                            }
                            TargetIndex::TextureSrt(TextureSrtTarget::ScaleV) => {
                                tq.tex_srts[layer].scale_v = v;
                            }
                            _ => {}
                        }
                    }

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
            }
            AnimInfoType::IndirectSrtAnim => {
                for t in targets {
                    let v = eval_curve(&t.curve, frame);
                    if let Some(tq) = view
                        .textured_quads
                        .iter_mut()
                        .find(|tq| tq.pane_idx == pane_idx)
                    {
                        match t.target {
                            TargetIndex::Raw(0) => tq.standard_material.indirect_mtx0[0] = v,
                            TargetIndex::Raw(1) => tq.standard_material.indirect_mtx0[1] = v,
                            TargetIndex::Raw(2) => tq.standard_material.indirect_mtx1[0] = v,
                            TargetIndex::Raw(3) => tq.standard_material.indirect_mtx1[1] = v,
                            _ => {}
                        }
                    }
                }
            }
            AnimInfoType::TexturePatternAnim => {
                for t in targets {
                    let file_idx = eval_curve_step_u16(&t.curve, frame) as usize;

                    if let Some(tex_name) = pai.textures.get(file_idx).cloned() {
                        if let Some(tq) = view
                            .textured_quads
                            .iter_mut()
                            .find(|tq| tq.pane_idx == pane_idx)
                        {
                            match t.layer {
                                0 => tq.texture_name = tex_name,
                                1 => tq.texture_name1 = Some(tex_name),
                                2 => tq.texture_name2 = Some(tex_name),
                                _ => {}
                            }
                        }
                    }
                }
            }
            AnimInfoType::MaterialColorAnim | AnimInfoType::VertexColorAnim => {
                for t in targets {
                    let v = eval_curve(&t.curve, frame);
                    if let Some(tq) = view
                        .textured_quads
                        .iter_mut()
                        .find(|tq| tq.pane_idx == pane_idx)
                    {
                        match &t.target {
                            TargetIndex::VertexColor(VertexColorTarget::PaneAlpha)
                            | TargetIndex::Raw(16) => tq.tint[3] = v / 255.0,
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

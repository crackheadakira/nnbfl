use std::collections::{HashMap, HashSet};

use nnbfl::bflan::anim_info::{AnimType, PaneAnimInfo};
use nnbfl::bflan::targets::{PaneSrtTarget, TargetIndex, VertexColorTarget};
use nnbfl::bflan::{anim_info::AnimInfo, curves::Curve, file::Bflan};
use nnbfl::bflyt::list::MaterialTextureSrt;
use nnbfl::bflyt::pane::Pane;
use nnbfl::ui2d::types::{Vector2f, Vector3f};
use nnbfl::ui2d::userdata::UserDataContent;

use crate::pane_tree::{DirtyFlags, PaneTree};
use crate::renderer::textured_quad::TexturedQuad;
use crate::ui::timeline::{PendingKeyEdit, PendingSlopeEdit, TimelineTrack};

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
    pub source: Option<crate::archive::EntryPath>,
    pub name: String,
    pub frame: f32,
    pub playing: bool,
    pub autoplay: bool,
    pub next_anim: Option<String>,
}

impl AnimInstance {
    pub fn new(bflan: Bflan, source: Option<crate::archive::EntryPath>) -> Self {
        let next_anim = find_next_anim(&bflan);
        let name = bflan.anim_tag.o_name.clone();

        Self {
            bflan,
            source,
            name,
            frame: 0.0,
            playing: false,
            autoplay: false,
            next_anim,
        }
    }

    pub fn frame_count(&self) -> f32 {
        self.bflan.anim_info.frame_count as f32
    }

    pub fn is_looping(&self) -> bool {
        self.bflan.anim_info.is_looping
    }

    pub fn toggle_looping(&mut self) {
        self.bflan.anim_info.is_looping = !self.bflan.anim_info.is_looping
    }

    pub fn curve(&self, track: &TimelineTrack) -> Option<&Curve> {
        let content = self.bflan.anim_info.contents.get(track.content_idx)?;
        let info = content.infos.get(track.info_idx)?;
        let AnimInfo::Standard { targets, .. } = info else {
            return None;
        };

        Some(&targets.get(track.target_idx)?.curve)
    }

    pub fn curve_mut(&mut self, track: &TimelineTrack) -> Option<&mut Curve> {
        let content = self.bflan.anim_info.contents.get_mut(track.content_idx)?;
        let info = content.infos.get_mut(track.info_idx)?;
        let AnimInfo::Standard { targets, .. } = info else {
            return None;
        };

        Some(&mut targets.get_mut(track.target_idx)?.curve)
    }

    pub fn get_active_group_pane_indices(&self, pane_tree: &PaneTree) -> HashSet<usize> {
        let mut active_indices = HashSet::new();

        if self.bflan.anim_tag.groups.is_empty() {
            return active_indices;
        }

        let label_to_idx = pane_tree.label_to_idx();

        for group_name in &self.bflan.anim_tag.groups {
            if let Some(layout_group) = pane_tree
                .group
                .children
                .iter()
                .find(|g| g.group_name == *group_name)
            {
                for child_name in &layout_group.child_names {
                    let clean_name = child_name.trim_end_matches('\0');

                    if let Some(&pane_idx) = label_to_idx.get(clean_name) {
                        active_indices.insert(pane_idx);
                    }
                }
            }
        }

        active_indices
    }

    pub fn resolve_group_visibility(
        &self,
        pane_tree: &PaneTree,
        hidden_panes: &mut HashSet<usize>,
    ) {
        if self.bflan.anim_tag.groups.is_empty() {
            return;
        }

        let explicit_active_indices = self.get_active_group_pane_indices(pane_tree);
        let label_to_idx = pane_tree.label_to_idx();

        let mut fully_allowed_indices = explicit_active_indices.clone();
        for &pane_idx in &explicit_active_indices {
            fully_allowed_indices.extend(pane_tree.descendants(pane_idx));
        }

        let active_groups: HashSet<&str> = self
            .bflan
            .anim_tag
            .groups
            .iter()
            .map(|s| s.as_str())
            .collect();

        for group in pane_tree.group.children.iter() {
            if active_groups.contains(group.group_name.as_str()) {
                continue;
            }

            for child_name in &group.child_names {
                let clean_name = child_name.trim_end_matches('\0');
                if let Some(&pane_idx) = label_to_idx.get(clean_name)
                    && !fully_allowed_indices.contains(&pane_idx)
                {
                    hidden_panes.insert(pane_idx);
                }
            }
        }
    }
}

fn find_next_anim(bflan: &Bflan) -> Option<String> {
    if let Some(ud) = &bflan.anim_tag.user_data {
        for entry in &ud.user_data {
            if entry.o_name == "CommandPlayEnd_Play"
                && let UserDataContent::String(value) = &entry.content
            {
                return Some(value.clone());
            }
        }
    }

    None
}

pub struct AnimPlayer {
    pub anims: Vec<AnimInstance>,
    pub active: Option<usize>,
    pub limit_to_group: bool,
    pub preview_enabled: bool,
    last_applied: Option<(Option<usize>, u32, bool, u64)>,
}

impl AnimPlayer {
    pub fn new() -> Self {
        Self {
            anims: Vec::new(),
            active: None,
            limit_to_group: false,
            preview_enabled: false,
            last_applied: None,
        }
    }

    pub fn load(&mut self, bflan: Bflan, source: Option<crate::archive::EntryPath>) {
        self.anims.push(AnimInstance::new(bflan, source));
    }

    pub fn play(
        &mut self,
        anim_idx: Option<usize>,
        pane_tree: &PaneTree,
        hidden_panes: &mut HashSet<usize>,
    ) {
        if let Some(idx) = anim_idx {
            if let Some(prev) = self.active
                && prev < self.anims.len()
            {
                self.anims[prev].playing = false;
            }

            self.anims[idx].frame = 0.0;
            self.anims[idx].playing = true;
            self.anims[idx].autoplay = true;
            self.active = Some(idx);
            self.preview_enabled = true;

            if self.limit_to_group {
                self.anims[idx].resolve_group_visibility(pane_tree, hidden_panes);
            }
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
            if anim.is_looping() && frame_count > 0.0 {
                anim.frame %= frame_count;
            } else {
                anim.frame = frame_count;
                anim.playing = false;
                return anim.next_anim.clone();
            }
        }
        None
    }

    pub fn apply(&mut self, pane_tree: &mut PaneTree) -> bool {
        let frame = self
            .active
            .and_then(|idx| self.anims.get(idx))
            .map(|anim| anim.frame.to_bits())
            .unwrap_or_default();
        let key = (
            self.active,
            frame,
            self.preview_enabled,
            pane_tree.animation_binding_revision,
        );
        if self.last_applied == Some(key) {
            return false;
        }

        let state = if self.preview_enabled {
            self.active
                .and_then(|idx| self.anims.get(idx))
                .map(|anim| {
                    AnimationState::evaluate(
                        &anim.bflan.anim_info,
                        anim.frame,
                        &pane_tree.label_map,
                    )
                })
                .unwrap_or_default()
        } else {
            AnimationState::default()
        };
        let changes = pane_tree.set_animation(state);
        if changes.contains(DirtyFlags::TRANSFORM) {
            pane_tree.recompute_dirty();
        }
        let bindings_changed = self
            .last_applied
            .is_none_or(|previous| previous.3 != pane_tree.animation_binding_revision);
        if !changes.is_empty() || bindings_changed {
            pane_tree.update_animation_visuals();
        }
        self.last_applied = Some(key);
        true
    }

    pub fn is_playing(&self) -> bool {
        self.active.map(|i| self.anims[i].playing).unwrap_or(false)
    }

    pub fn apply_key_edit(&mut self, edit: &PendingKeyEdit) {
        let Some(idx) = self.active else { return };
        let Some(anim) = self.anims.get_mut(idx) else {
            return;
        };

        let Some(curve) = anim.curve_mut(&edit.track) else {
            return;
        };

        self.last_applied = None;

        match curve {
            Curve::Constant(keys) => {
                if let Some(v) = keys.get_mut(edit.key_idx) {
                    *v = edit.value;
                }
            }

            Curve::Step(keys) => {
                let min_frame = edit
                    .key_idx
                    .checked_sub(1)
                    .and_then(|i| keys.get(i))
                    .map(|k| k.frame + 0.01)
                    .unwrap_or(0.0);

                let max_frame = keys
                    .get(edit.key_idx + 1)
                    .map(|k| k.frame - 0.01)
                    .unwrap_or(f32::MAX)
                    .max(min_frame);

                if let Some(k) = keys.get_mut(edit.key_idx) {
                    k.frame = edit.frame.clamp(min_frame, max_frame);
                    k.value = edit.value.round().clamp(0.0, u16::MAX as f32) as u16;
                }
            }

            Curve::Hermite(keys) => {
                let min_frame = edit
                    .key_idx
                    .checked_sub(1)
                    .and_then(|i| keys.get(i))
                    .map(|k| k.frame + 0.01)
                    .unwrap_or(0.0);

                let max_frame = keys
                    .get(edit.key_idx + 1)
                    .map(|k| k.frame - 0.01)
                    .unwrap_or(f32::MAX)
                    .max(min_frame);

                if let Some(k) = keys.get_mut(edit.key_idx) {
                    k.frame = edit.frame.clamp(min_frame, max_frame);
                    k.value = edit.value;
                }
            }
        }
    }

    pub fn apply_slope_edit(&mut self, edit: &PendingSlopeEdit) {
        let Some(idx) = self.active else { return };
        let Some(anim) = self.anims.get_mut(idx) else {
            return;
        };

        let Some(curve) = anim.curve_mut(&edit.track) else {
            return;
        };

        if let Curve::Hermite(keys) = curve
            && let Some(k) = keys.get_mut(edit.key_idx)
        {
            k.slope = edit.slope;
            self.last_applied = None;
        }
    }
}

#[inline]
pub fn transform_uv_srt(srt: &MaterialTextureSrt, uv: [f32; 2]) -> [f32; 2] {
    let rad = srt.rotate.to_radians();
    let cos_r = rad.cos();
    let sin_r = rad.sin();

    let centered_u = uv[0] - 0.5;
    let centered_v = uv[1] - 0.5;

    let scaled_u = centered_u * srt.scale_u;
    let scaled_v = centered_v * srt.scale_v;

    let rotated_u = scaled_u * cos_r - scaled_v * sin_r;
    let rotated_v = scaled_u * sin_r + scaled_v * cos_r;

    [
        rotated_u + srt.translate_u + 0.5,
        rotated_v + srt.translate_v + 0.5,
    ]
}

pub fn all_quads_mut(
    node: &mut crate::pane_tree::PaneNode,
) -> impl Iterator<Item = &mut TexturedQuad> {
    node.textured_quad
        .iter_mut()
        .chain(node.window_quads.iter_mut())
        .chain(node.glyph_quads.iter_mut())
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaneAnimation {
    pub translation: [Option<f32>; 3],
    pub rotation: [Option<f32>; 3],
    pub scale: [Option<f32>; 2],
    pub size: [Option<f32>; 2],
    pub visible: Option<bool>,
    pub alpha: Option<f32>,
    pub corner_colors: [[Option<f32>; 4]; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedPaneTransform {
    pub translation: Vector3f,
    pub rotation: Vector3f,
    pub scale: Vector2f,
    pub size: Vector2f,
}

impl PaneAnimation {
    pub fn resolve_transform(&self, base: &Pane) -> ResolvedPaneTransform {
        ResolvedPaneTransform {
            translation: Vector3f {
                x: self.translation[0].unwrap_or(base.translation.x),
                y: self.translation[1].unwrap_or(base.translation.y),
                z: self.translation[2].unwrap_or(base.translation.z),
            },

            rotation: Vector3f {
                x: self.rotation[0].unwrap_or(base.rotation.x),
                y: self.rotation[1].unwrap_or(base.rotation.y),
                z: self.rotation[2].unwrap_or(base.rotation.z),
            },

            scale: Vector2f::new(
                self.scale[0].unwrap_or(base.scale.x),
                self.scale[1].unwrap_or(base.scale.y),
            ),

            size: Vector2f::new(
                self.size[0].unwrap_or(base.size.x),
                self.size[1].unwrap_or(base.size.y),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextureSrtAnimation {
    pub translate_u: Option<f32>,
    pub translate_v: Option<f32>,
    pub rotate: Option<f32>,
    pub scale_u: Option<f32>,
    pub scale_v: Option<f32>,
}

impl TextureSrtAnimation {
    pub fn resolve(&self, base: &MaterialTextureSrt) -> MaterialTextureSrt {
        MaterialTextureSrt {
            translate_u: self.translate_u.unwrap_or(base.translate_u),
            translate_v: self.translate_v.unwrap_or(base.translate_v),
            rotate: self.rotate.unwrap_or(base.rotate),
            scale_u: self.scale_u.unwrap_or(base.scale_u),
            scale_v: self.scale_v.unwrap_or(base.scale_v),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialAnimation {
    pub texture_srts: [TextureSrtAnimation; 3],
    pub texture_names: [Option<String>; 3],
    pub indirect_rotation: Option<f32>,
    pub indirect_scale: [Option<f32>; 2],
    pub alpha_reference: Option<f32>,
    pub black_color: [Option<u8>; 4],
    pub white_color: [Option<u8>; 4],
}

#[derive(Default)]
pub struct AnimationState {
    pub panes: HashMap<usize, PaneAnimation>,
    pub materials: HashMap<String, MaterialAnimation>,
}

impl AnimationState {
    pub fn evaluate(pai: &PaneAnimInfo, frame: f32, pane_by_name: &HashMap<String, usize>) -> Self {
        let mut state = Self::default();

        for content in &pai.contents {
            let name = content.name.trim_end_matches('\0');

            match content.anim_type {
                AnimType::Pane | AnimType::PaneExt => {
                    let Some(&pane_idx) = pane_by_name.get(name) else {
                        continue;
                    };

                    let pane = state.panes.entry(pane_idx).or_default();

                    for info in &content.infos {
                        let AnimInfo::Standard { targets, .. } = info else {
                            continue;
                        };

                        for target in targets {
                            let value = eval_curve(&target.curve, frame);

                            match &target.target {
                                TargetIndex::PaneSrt(channel) => match channel {
                                    PaneSrtTarget::TranslateX => pane.translation[0] = Some(value),
                                    PaneSrtTarget::TranslateY => pane.translation[1] = Some(value),
                                    PaneSrtTarget::TranslateZ => pane.translation[2] = Some(value),
                                    PaneSrtTarget::RotateX => pane.rotation[0] = Some(value),
                                    PaneSrtTarget::RotateY => pane.rotation[1] = Some(value),
                                    PaneSrtTarget::RotateZ => pane.rotation[2] = Some(value),
                                    PaneSrtTarget::ScaleX => pane.scale[0] = Some(value),
                                    PaneSrtTarget::ScaleY => pane.scale[1] = Some(value),
                                    PaneSrtTarget::SizeX => pane.size[0] = Some(value),
                                    PaneSrtTarget::SizeY => pane.size[1] = Some(value),
                                },
                                TargetIndex::Visibility(_) => {
                                    pane.visible =
                                        Some(eval_curve_step_u16(&target.curve, frame) != 0);
                                }

                                TargetIndex::VertexColor(VertexColorTarget::PaneAlpha) => {
                                    pane.alpha = Some(value / 255.0);
                                }

                                TargetIndex::VertexColor(channel) => {
                                    let index = channel.clone() as usize;
                                    pane.corner_colors[index / 4][index % 4] = Some(value / 255.0);
                                }

                                _ => {}
                            }
                        }
                    }
                }

                AnimType::Material => {
                    let material = state.materials.entry(name.to_string()).or_default();

                    for info in &content.infos {
                        let AnimInfo::Standard { targets, .. } = info else {
                            continue;
                        };

                        for target in targets {
                            let value = eval_curve(&target.curve, frame);

                            match &target.target {
                                TargetIndex::TextureSrt(channel) => {
                                    use nnbfl::bflan::targets::TextureSrtTarget::*;

                                    let Some(srt) =
                                        material.texture_srts.get_mut(target.layer as usize)
                                    else {
                                        continue;
                                    };

                                    match channel {
                                        TranslateU => srt.translate_u = Some(value),
                                        TranslateV => srt.translate_v = Some(value),
                                        Rotate => srt.rotate = Some(value),
                                        ScaleU => srt.scale_u = Some(value),
                                        ScaleV => srt.scale_v = Some(value),
                                    }
                                }

                                TargetIndex::IndirectSrt(channel) => {
                                    use nnbfl::bflan::targets::IndirectSrtTarget::*;

                                    match channel {
                                        Rotate => material.indirect_rotation = Some(value),
                                        ScaleU => material.indirect_scale[0] = Some(value),
                                        ScaleV => material.indirect_scale[1] = Some(value),
                                    }
                                }

                                TargetIndex::TexturePattern(_) => {
                                    let index = eval_curve_step_u16(&target.curve, frame) as usize;
                                    if let Some(slot) =
                                        material.texture_names.get_mut(target.layer as usize)
                                    {
                                        *slot = pai.textures.get(index).cloned();
                                    }
                                }

                                TargetIndex::AlphaCompare(_) => {
                                    material.alpha_reference = Some(value)
                                }

                                TargetIndex::MaterialColor(channel) => {
                                    let index = channel.clone() as usize;

                                    let color = if index < 4 {
                                        &mut material.black_color
                                    } else {
                                        &mut material.white_color
                                    };

                                    color[index % 4] = Some(value.round().clamp(0.0, 255.0) as u8);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        state
    }
}

use nnbfl::bflan::{
    anim_info::{AnimContent, AnimInfo, AnimInfoType, AnimType, PaneAnimInfo},
    curves::{Curve, HermiteKey, StepKey},
    targets::{AnimTarget, TargetIndex},
};

use crate::{anim_state::eval_curve, ui::timeline::TimelineTrack};

pub struct TrackKind {
    pub info_type: AnimInfoType,
    pub label: &'static str,
    pub targets: u8,
    pub step: bool,
    pub layered: bool,
}

const fn kind(
    info_type: AnimInfoType,
    label: &'static str,
    targets: u8,
    step: bool,
    layered: bool,
) -> TrackKind {
    TrackKind {
        info_type,
        label,
        targets,
        step,
        layered,
    }
}

pub const PANE_KINDS: &[TrackKind] = &[
    kind(AnimInfoType::PaneSrtAnim, "Pane SRT", 10, false, false),
    kind(AnimInfoType::VisibilityAnim, "Visibility", 1, true, false),
    kind(
        AnimInfoType::VertexColorAnim,
        "Vertex Color",
        17,
        false,
        false,
    ),
    kind(AnimInfoType::WindowAnim, "Window", 4, false, false),
];

pub const MATERIAL_KINDS: &[TrackKind] = &[
    kind(
        AnimInfoType::MaterialColorAnim,
        "Material Color",
        28,
        false,
        false,
    ),
    kind(AnimInfoType::TextureSrtAnim, "Texture SRT", 5, false, true),
    kind(
        AnimInfoType::TexturePatternAnim,
        "Texture Pattern",
        1,
        true,
        true,
    ),
    kind(
        AnimInfoType::IndirectSrtAnim,
        "Indirect SRT",
        3,
        false,
        false,
    ),
    kind(
        AnimInfoType::AlphaCompareAnim,
        "Alpha Compare",
        1,
        false,
        false,
    ),
    kind(AnimInfoType::FontShadowAnim, "Font Shadow", 7, false, false),
];

pub fn kinds_for(anim_type: &AnimType) -> &'static [TrackKind] {
    match anim_type {
        AnimType::Pane => PANE_KINDS,
        AnimType::Material => MATERIAL_KINDS,
        _ => &[],
    }
}

#[derive(Clone, Debug, Default)]
pub struct PaneBase {
    pub translate: [f32; 3],
    pub rotate: [f32; 3],
    pub scale: [f32; 2],
    pub size: [f32; 2],
    pub alpha: f32,
    pub visible: bool,
}

#[derive(Clone, Debug)]
pub struct Animatable {
    pub name: String,
    pub anim_type: AnimType,
    pub base: Option<PaneBase>,
    pub layers: usize,
}

pub fn target_name(info_type: AnimInfoType, raw: u8) -> String {
    match TargetIndex::resolve(&info_type, raw) {
        TargetIndex::PaneSrt(t) => format!("{t:?}"),
        TargetIndex::Visibility(t) => format!("{t:?}"),
        TargetIndex::VertexColor(t) => format!("{t:?}"),
        TargetIndex::Window(t) => format!("{t:?}"),
        TargetIndex::MaterialColor(t) => format!("{t:?}"),
        TargetIndex::TextureSrt(t) => format!("{t:?}"),
        TargetIndex::TexturePattern(t) => format!("{t:?}"),
        TargetIndex::IndirectSrt(t) => format!("{t:?}"),
        TargetIndex::AlphaCompare(t) => format!("{t:?}"),
        TargetIndex::FontShadow(t) => format!("{t:?}"),
        other => format!("{other:?}"),
    }
}

fn starting_value(info_type: AnimInfoType, raw: u8, base: Option<&PaneBase>) -> f32 {
    let base = base.cloned().unwrap_or(PaneBase {
        scale: [1.0, 1.0],
        alpha: 255.0,
        visible: true,
        ..Default::default()
    });

    match info_type {
        AnimInfoType::PaneSrtAnim => match raw {
            0..=2 => base.translate[raw as usize],
            3..=5 => base.rotate[raw as usize - 3],
            6 | 7 => base.scale[raw as usize - 6],
            _ => base.size[raw as usize - 8],
        },
        AnimInfoType::VisibilityAnim => base.visible as u8 as f32,
        AnimInfoType::VertexColorAnim if raw == 16 => base.alpha,
        AnimInfoType::VertexColorAnim | AnimInfoType::MaterialColorAnim => 255.0,
        AnimInfoType::TextureSrtAnim => matches!(raw, 3 | 4) as u8 as f32,
        AnimInfoType::IndirectSrtAnim => matches!(raw, 1 | 2) as u8 as f32,
        AnimInfoType::FontShadowAnim => (raw >= 3) as u8 as f32 * 255.0,
        _ => 0.0,
    }
}

pub fn add_track(
    info: &mut PaneAnimInfo,
    target: &Animatable,
    kind: &TrackKind,
    raw: u8,
    layer: u8,
) -> Result<(), String> {
    let value = starting_value(kind.info_type, raw, target.base.as_ref());
    let curve = if kind.step {
        Curve::Step(vec![StepKey {
            frame: 0.0,
            value: value as u16,
        }])
    } else {
        Curve::Hermite(vec![HermiteKey {
            frame: 0.0,
            value,
            slope: 0.0,
        }])
    };

    let content = match info.contents.iter().position(|c| {
        c.name.trim_end_matches('\0') == target.name && c.anim_type == target.anim_type
    }) {
        Some(index) => &mut info.contents[index],
        None => {
            info.contents.push(AnimContent {
                name: target.name.clone(),
                anim_type: target.anim_type.clone(),
                infos: Vec::new(),
            });
            info.contents.last_mut().unwrap()
        }
    };

    let targets = match content.infos.iter().position(
        |i| matches!(i, AnimInfo::Standard { anim_type, .. } if *anim_type == kind.info_type),
    ) {
        Some(index) => match &mut content.infos[index] {
            AnimInfo::Standard { targets, .. } => targets,
            AnimInfo::ExtendedUserData { .. } => unreachable!(),
        },
        None => {
            content.infos.push(AnimInfo::Standard {
                anim_type: kind.info_type,
                targets: Vec::new(),
            });

            match content.infos.last_mut().unwrap() {
                AnimInfo::Standard { targets, .. } => targets,
                AnimInfo::ExtendedUserData { .. } => unreachable!(),
            }
        }
    };

    let index = TargetIndex::resolve(&kind.info_type, raw);

    if targets
        .iter()
        .any(|t| t.target == index && t.layer == layer)
    {
        return Err(format!(
            "{} already has a {} track",
            target.name,
            target_name(kind.info_type, raw)
        ));
    }

    targets.push(AnimTarget {
        layer,
        target: index,
        curve,
    });

    Ok(())
}

fn targets_mut<'a>(
    info: &'a mut PaneAnimInfo,
    track: &TimelineTrack,
) -> Option<&'a mut Vec<AnimTarget>> {
    match info
        .contents
        .get_mut(track.content_idx)?
        .infos
        .get_mut(track.info_idx)?
    {
        AnimInfo::Standard { targets, .. } => Some(targets),
        AnimInfo::ExtendedUserData { .. } => None,
    }
}

pub fn delete_track(info: &mut PaneAnimInfo, track: &TimelineTrack) -> Option<()> {
    let targets = targets_mut(info, track)?;

    if track.target_idx >= targets.len() {
        return None;
    }

    targets.remove(track.target_idx);

    let content = &mut info.contents[track.content_idx];

    if matches!(&content.infos[track.info_idx], AnimInfo::Standard { targets, .. } if targets.is_empty())
    {
        content.infos.remove(track.info_idx);
    }

    if content.infos.is_empty() {
        info.contents.remove(track.content_idx);
    }

    Some(())
}

pub fn remove_content(info: &mut PaneAnimInfo, content_idx: usize) -> Option<()> {
    (content_idx < info.contents.len()).then(|| {
        info.contents.remove(content_idx);
    })
}

pub fn add_key(
    info: &mut PaneAnimInfo,
    track: &TimelineTrack,
    frame: f32,
) -> Result<usize, String> {
    let curve = &mut targets_mut(info, track)
        .and_then(|t| t.get_mut(track.target_idx))
        .ok_or("The track no longer exists")?
        .curve;

    let value = eval_curve(curve, frame);
    let taken = |f: f32| (f - frame).abs() < 0.001;

    match curve {
        Curve::Hermite(keys) => {
            if keys.iter().any(|k| taken(k.frame)) {
                return Err(format!("There is already a key at frame {frame}"));
            }

            let at = keys.partition_point(|k| k.frame < frame);
            keys.insert(
                at,
                HermiteKey {
                    frame,
                    value,
                    slope: 0.0,
                },
            );

            Ok(at)
        }
        Curve::Step(keys) => {
            if keys.iter().any(|k| taken(k.frame)) {
                return Err(format!("There is already a key at frame {frame}"));
            }

            let at = keys.partition_point(|k| k.frame < frame);
            keys.insert(
                at,
                StepKey {
                    frame,
                    value: value.round().max(0.0) as u16,
                },
            );

            Ok(at)
        }
        Curve::Constant(_) => Err("Constant curves have one value per frame".into()),
    }
}

pub fn delete_key(
    info: &mut PaneAnimInfo,
    track: &TimelineTrack,
    key_idx: usize,
) -> Result<(), String> {
    let curve = &mut targets_mut(info, track)
        .and_then(|t| t.get_mut(track.target_idx))
        .ok_or("The track no longer exists")?
        .curve;

    let len = match curve {
        Curve::Hermite(keys) => keys.len(),
        Curve::Step(keys) => keys.len(),
        Curve::Constant(values) => values.len(),
    };

    if len <= 1 {
        return Err("A track keeps at least one key; delete the track instead".into());
    }

    match curve {
        Curve::Hermite(keys) if key_idx < keys.len() => {
            keys.remove(key_idx);
        }
        Curve::Step(keys) if key_idx < keys.len() => {
            keys.remove(key_idx);
        }
        Curve::Constant(_) => return Err("Constant curves have one value per frame".into()),
        _ => return Err("The key no longer exists".into()),
    }

    Ok(())
}

pub fn track_info_type(info: &PaneAnimInfo, track: &TimelineTrack) -> Option<AnimInfoType> {
    match info
        .contents
        .get(track.content_idx)?
        .infos
        .get(track.info_idx)?
    {
        AnimInfo::Standard { anim_type, .. } => Some(*anim_type),
        AnimInfo::ExtendedUserData { .. } => None,
    }
}

use nnbfl::{
    bflan::{
        anim_info::AnimInfo,
        curves::Curve,
        targets::{PerCharacterTransformCurveTarget as CurveTarget, TargetIndex},
    },
    bflyt::{
        flags::TextPaneFlags,
        pane::{
            HorizontalAlignment, PerCharacterTransform, PerCharacterTransformLoopType,
            PerCharacterTransformOrigin,
        },
    },
    ui2d::types::Color4u8,
};

use super::layout::TextLayout;
use crate::anim_state::eval_curve;

pub fn char_times(
    pct: &PerCharacterTransform,
    [offset, width]: [f32; 2],
    flags: &TextPaneFlags,
    layout: &TextLayout,
    advances: &[f32],
) -> Vec<f32> {
    let count = advances.len();
    let character_space = layout.character_space;
    let to_center = flags.is_per_character_transform_origin_to_center;

    let begin = |step_total: f32| match pct.fix_space_or_insert_space_origin {
        HorizontalAlignment::Left => offset,
        HorizontalAlignment::Center => offset + (width - step_total) / 2.0,
        HorizontalAlignment::Right => offset + width - step_total,
    };

    if flags.is_per_character_transform_split_insert_space && to_center {
        let widths: Vec<f32> = advances
            .iter()
            .map(|advance| (advance + character_space) * layout.scale_x)
            .collect();

        let step = if layout.box_width == 0.0 {
            0.0
        } else {
            width / layout.box_width
        };

        let mut time = begin(step * widths.iter().sum::<f32>());

        return widths
            .iter()
            .map(|w| {
                time += w * step / 2.0;
                let at = time;
                time += w * step / 2.0;
                at
            })
            .collect();
    }

    if flags.is_per_character_transform_fix_space && to_center {
        let step = pct.fix_space_width;
        let mut time = begin(step * count as f32);

        return (0..count)
            .map(|_| {
                time += step / 2.0;
                let at = time;
                time += step / 2.0;
                at
            })
            .collect();
    }

    if flags.is_per_character_transform_split_by_char_width {
        let widths: Vec<f32> = advances
            .iter()
            .map(|advance| advance + character_space)
            .collect();

        let total: f32 = widths.iter().skip(usize::from(!to_center)).sum();
        let step = if total == 0.0 { 0.0 } else { width / total };
        let mut time = offset;

        return widths
            .iter()
            .enumerate()
            .map(|(i, w)| {
                if to_center {
                    time += w * step / 2.0;
                } else if i > 0 {
                    time += w * step;
                }

                let at = time;

                if to_center {
                    time += w * step / 2.0;
                }

                at
            })
            .collect();
    }

    let step = if count <= 1 {
        0.0
    } else {
        width / (count - 1) as f32
    };

    (0..count).map(|i| offset + step * i as f32).collect()
}

#[derive(Debug, Clone, Copy)]
pub struct CharValues {
    pub translate: [f32; 3],

    /// In degrees
    pub rotate: [f32; 3],

    pub scale: [f32; 2],
    pub top: [u8; 4],
    pub bottom: [u8; 4],
}

impl Default for CharValues {
    fn default() -> Self {
        Self {
            translate: [0.0; 3],
            rotate: [0.0; 3],
            scale: [1.0; 2],
            top: [255; 4],
            bottom: [255; 4],
        }
    }
}

impl CharValues {
    pub fn evaluate(pct: &PerCharacterTransform, time: f32) -> Self {
        let mut values = Self::default();

        let Some(AnimInfo::Standard { targets, .. }) = &pct.anim_info else {
            return values;
        };

        for target in targets {
            let TargetIndex::PerCharacterTransformCurve(kind) = &target.target else {
                continue;
            };

            let frame = match (pct.loop_type, key_range(&target.curve)) {
                (PerCharacterTransformLoopType::Loop, Some((first, last))) => {
                    loop_frame(time, first, last)
                }
                _ => time,
            };

            let value = eval_curve(&target.curve, frame);

            match kind {
                CurveTarget::TranslateX => values.translate[0] = value,
                CurveTarget::TranslateY => values.translate[1] = -value,
                CurveTarget::TranslateZ => values.translate[2] = value,
                CurveTarget::RotateX => values.rotate[0] = -value,
                CurveTarget::RotateY => values.rotate[1] = value,
                CurveTarget::RotateZ => values.rotate[2] = -value,
                CurveTarget::LeftTopRed => values.top[0] = value as u8,
                CurveTarget::LeftTopGreen => values.top[1] = value as u8,
                CurveTarget::LeftTopBlue => values.top[2] = value as u8,
                CurveTarget::LeftTopAlpha => values.top[3] = value as u8,
                CurveTarget::LeftBottomRed => values.bottom[0] = value as u8,
                CurveTarget::LeftBottomGreen => values.bottom[1] = value as u8,
                CurveTarget::LeftBottomBlue => values.bottom[2] = value as u8,
                CurveTarget::LeftBottomAlpha => values.bottom[3] = value as u8,
                CurveTarget::ScaleX => values.scale[0] = value,
                CurveTarget::ScaleY => values.scale[1] = value,
            }
        }

        values
    }
}

fn key_range(curve: &Curve) -> Option<(f32, f32)> {
    match curve {
        Curve::Hermite(keys) if keys.len() >= 2 => {
            Some((keys[0].frame, keys[keys.len() - 1].frame))
        }
        Curve::Step(keys) if keys.len() >= 2 => Some((keys[0].frame, keys[keys.len() - 1].frame)),
        _ => None,
    }
}

fn loop_frame(frame: f32, begin: f32, end: f32) -> f32 {
    if frame > end {
        (frame - begin) % (end - begin) + begin
    } else if frame < begin {
        end - (begin - frame) % (end - begin)
    } else {
        frame
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CharTransform {
    columns: [[f32; 2]; 2],
    center: [f32; 2],
    translate: [f32; 2],
    shift_x: f32,
    top: [u8; 4],
    bottom: [u8; 4],
}

impl CharTransform {
    pub fn new(
        values: &CharValues,
        cell: [f32; 4],
        pct: &PerCharacterTransform,
        origin_to_center: bool,
        anchor: [f32; 2],
    ) -> Self {
        let [x, y, width, height] = cell;
        let [rx, ry, rz] = values.rotate.map(f32::to_radians);
        let (sin_x, cos_x) = rx.sin_cos();
        let (sin_y, cos_y) = ry.sin_cos();
        let (sin_z, cos_z) = rz.sin_cos();

        let [scale_x, scale_y] = values.scale;
        let columns = [
            [cos_z * cos_y * scale_x, sin_z * cos_y * scale_x],
            [
                (cos_z * sin_y * sin_x - sin_z * cos_x) * scale_y,
                (cos_z * cos_x + sin_z * sin_y * sin_x) * scale_y,
            ],
        ];

        let [tx, ty, _] = values.translate;

        let (center, translate, shift_x) = if origin_to_center {
            (
                [anchor[0], y],
                [tx, ty + anchor[1] - y],
                anchor[0] - width / 2.0 - x,
            )
        } else {
            let center_y = match pct.origin_v {
                PerCharacterTransformOrigin::Center => y + height / 2.0,
                PerCharacterTransformOrigin::Bottom => y + height,
            };

            (
                [x + width / 2.0, center_y - pct.origin_v_offset],
                [tx, ty],
                0.0,
            )
        };

        Self {
            columns,
            center,
            translate,
            shift_x,
            top: values.top,
            bottom: values.bottom,
        }
    }

    pub fn apply(&self, point: [f32; 2]) -> [f32; 2] {
        let local = [
            point[0] + self.shift_x - self.center[0],
            point[1] - self.center[1],
        ];

        [
            self.columns[0][0] * local[0]
                + self.columns[1][0] * local[1]
                + self.center[0]
                + self.translate[0],
            self.columns[0][1] * local[0]
                + self.columns[1][1] * local[1]
                + self.center[1]
                + self.translate[1],
        ]
    }

    pub fn tint(&self, top: &Color4u8, bottom: &Color4u8) -> (Color4u8, Color4u8) {
        let mul = |c: &Color4u8, m: [u8; 4]| Color4u8 {
            r: (u32::from(c.r) * u32::from(m[0]) / 255) as u8,
            g: (u32::from(c.g) * u32::from(m[1]) / 255) as u8,
            b: (u32::from(c.b) * u32::from(m[2]) / 255) as u8,
            a: (u32::from(c.a) * u32::from(m[3]) / 255) as u8,
        };

        (mul(top, self.top), mul(bottom, self.bottom))
    }
}

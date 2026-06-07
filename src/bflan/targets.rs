use serde::{Deserialize, Serialize};

use crate::{
    bflan::curves::Curve,
    core::{Cursor, Writer, tchar_code32},
};

#[derive(Debug, Serialize, Clone, Deserialize)]
pub struct AnimTarget {
    pub reserve0: u8,
    pub target: TargetIndex,
    pub curve: Curve,
}

impl AnimTarget {
    pub fn parse(cursor: &mut Cursor, base_offset: usize, parent_magic: u32) -> Self {
        cursor.seek(base_offset);

        let reserve0 = cursor.read_u8();
        let target_raw = cursor.read_u8();
        let curve_type = cursor.read_u8();
        let _reserve1 = cursor.read_u8();
        let frame_count = cursor.read_u16();
        let _reserve2 = cursor.read_u16();
        let key_array_offset = cursor.read_u32();

        let target = TargetIndex::resolve(parent_magic, target_raw);

        cursor.seek(base_offset + key_array_offset as usize);

        let curve = Curve::parse(cursor, curve_type, frame_count as usize);

        Self {
            reserve0,
            target,
            curve,
        }
    }

    pub fn serialize(&self, writer: &mut Writer, base_offset: usize) {
        writer.mark("AnimTarget");
        writer.write_u8(self.reserve0);
        writer.write_u8(self.target.to_raw());

        let (curve_type, frame_count) = match &self.curve {
            Curve::Constant(keys) => (0, keys.len()),
            Curve::Step(keys) => (1, keys.len()),
            Curve::Hermite(keys) => (2, keys.len()),
        };

        writer.write_u8(curve_type);
        writer.write_u8(0);
        writer.write_u16(frame_count as u16);
        writer.write_u16(0);

        let key_array_offset_pos = writer.write_placeholder_u32();

        let keys_base = writer.pos();
        writer.patch_u32(key_array_offset_pos, (keys_base - base_offset) as u32);

        self.curve.serialize(writer);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TargetIndex {
    PerCharacterTransformCurve(PerCharacterTransformCurveTarget),
    PerCharacterTransform(PerCharacterTransformTarget),
    PaneSrt(PaneSrtTarget),
    VertexColor(VertexColorTarget),
    Visibility,
    MaterialColor(MaterialColorTarget),
    TextureSrt(TextureSrtTarget),
    TexturePattern(TexturePatternTarget),
    AlphaCompare,
    FontShadow,
    Raw(u8),
}

impl TargetIndex {
    pub fn to_raw(&self) -> u8 {
        match self {
            TargetIndex::PaneSrt(PaneSrtTarget::TranslateX) => 0,
            TargetIndex::PaneSrt(PaneSrtTarget::TranslateY) => 1,
            TargetIndex::PaneSrt(PaneSrtTarget::TranslateZ) => 2,
            TargetIndex::PaneSrt(PaneSrtTarget::RotateX) => 3,
            TargetIndex::PaneSrt(PaneSrtTarget::RotateY) => 4,
            TargetIndex::PaneSrt(PaneSrtTarget::RotateZ) => 5,
            TargetIndex::PaneSrt(PaneSrtTarget::ScaleX) => 6,
            TargetIndex::PaneSrt(PaneSrtTarget::ScaleY) => 7,
            TargetIndex::PaneSrt(PaneSrtTarget::SizeX) => 8,
            TargetIndex::PaneSrt(PaneSrtTarget::SizeY) => 9,
            TargetIndex::Raw(r) => *r,
            _ => 0,
        }
    }

    pub fn resolve(magic: u32, raw: u8) -> Self {
        match magic {
            m if m == tchar_code32(b"FLPA") => match raw {
                0 => Self::PaneSrt(PaneSrtTarget::TranslateX),
                1 => Self::PaneSrt(PaneSrtTarget::TranslateY),
                2 => Self::PaneSrt(PaneSrtTarget::TranslateZ),
                3 => Self::PaneSrt(PaneSrtTarget::RotateX),
                4 => Self::PaneSrt(PaneSrtTarget::RotateY),
                5 => Self::PaneSrt(PaneSrtTarget::RotateZ),
                6 => Self::PaneSrt(PaneSrtTarget::ScaleX),
                7 => Self::PaneSrt(PaneSrtTarget::ScaleY),
                8 => Self::PaneSrt(PaneSrtTarget::SizeX),
                9 => Self::PaneSrt(PaneSrtTarget::SizeY),
                _ => Self::Raw(raw),
            },

            _ => Self::Raw(raw),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PaneSrtTarget {
    TranslateX = 0,
    TranslateY = 1,
    TranslateZ = 2,
    RotateX = 3,
    RotateY = 4,
    RotateZ = 5,
    ScaleX = 6,
    ScaleY = 7,
    SizeX = 8,
    SizeY = 9,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PerCharacterTransformCurveTarget {
    TranslateX = 0,
    TranslateY = 1,
    TranslateZ = 2,
    RotateX = 3,
    RotateY = 4,
    RotateZ = 5,
    LeftTopRed = 6,
    LeftTopGreen = 7,
    LeftTopBlue = 8,
    LeftTopAlpha = 9,
    LeftBottomRed = 10,
    LeftBottomGreen = 11,
    LeftBottomBlue = 12,
    LeftBottomAlpha = 13,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PerCharacterTransformTarget {
    EvalTypeOffset,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TexturePatternTarget {
    Image = 0,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum VertexColorTarget {
    LeftTopRed = 0,
    LeftTopGreen = 1,
    LeftTopBlue = 2,
    LeftTopAlpha = 3,
    RightTopRed = 4,
    RightTopGreen = 5,
    RightTopBlue = 6,
    RightTopAlpha = 7,
    LeftBottomRed = 8,
    LeftBottomGreen = 9,
    LeftBottomBlue = 10,
    LeftBottomAlpha = 11,
    RightBottomRed = 12,
    RightBottomGreen = 13,
    RightBottomBlue = 14,
    RightBottomAlpha = 15,
    PaneAlpha = 16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MaterialColorTarget {
    BufferRed = 0,
    BufferGreen = 1,
    BufferBlue = 2,
    BufferAlpha = 3,
    Constant0Red = 4,
    Constant0Green = 5,
    Constant0Blue = 6,
    Constant0Alpha = 7,
    Color0Red = 8,
    Color0Green = 9,
    Color0Blue = 10,
    Color0Alpha = 11,
    Color1Red = 12,
    Color1Green = 13,
    Color1Blue = 14,
    Color1Alpha = 15,
    Color2Red = 16,
    Color2Green = 17,
    Color2Blue = 18,
    Color2Alpha = 19,
    Color3Red = 20,
    Color3Green = 21,
    Color3Blue = 22,
    Color3Alpha = 23,
    Color4Red = 24,
    Color4Green = 25,
    Color4Blue = 26,
    Color4Alpha = 27,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TextureSrtTarget {
    TranslateU = 0,
    TranslateV = 1,
    Rotate = 2,
    ScaleU = 3,
    ScaleV = 4,
}

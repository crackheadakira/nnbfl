use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};

use crate::{
    bflyt::flags::{DropShadowFlags, TexOptions},
    core::{Cursor, FormatError, Writer},
    ui2d::types::{Color4f, VertexPos},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResUi2dSystemDataArray {
    pub reserve0: u16,

    pub data_array: Vec<ResUi2dSystemDataInner>,
}

impl ResUi2dSystemDataArray {
    pub fn parse(cursor: &mut Cursor, is_pane: bool) -> Result<Self, FormatError> {
        let base_offset = cursor.pos;

        let reserve0 = cursor.read_u16()?;
        let count = cursor.read_u16()?;
        let offset = cursor.read_u32()?;

        let post_header_point = cursor.pos;

        cursor.seek(base_offset + offset as usize);

        let mut data_array = Vec::new();

        for _ in 0..count {
            let data = if is_pane {
                ResUi2dSystemDataInner::Pane(ResUi2dPaneData::parse(cursor)?)
            } else {
                ResUi2dSystemDataInner::Layout(ResUi2dLayoutData::parse(cursor, post_header_point)?)
            };
            data_array.push(data);
        }

        Ok(Self {
            reserve0,
            data_array,
        })
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("Ui2dSystemDataArray");

        writer.write_u16(self.reserve0);
        writer.write_u16(self.data_array.len() as u16);

        let count = self.data_array.len();

        let offset: u32 = if count > 1 { 0xC } else { 0x8 };
        writer.write_u32(offset);

        let size_ph = if count > 1 {
            Some(writer.write_placeholder_u32())
        } else {
            None
        };

        let items_start = writer.pos();
        for item in &self.data_array {
            match item {
                ResUi2dSystemDataInner::Pane(pane) => pane.serialize(writer),
                ResUi2dSystemDataInner::Layout(layout) => layout.serialize(writer),
            }
        }

        if let Some(ph) = size_ph {
            let items_written = writer.pos() - items_start;

            // rounding up to next 8 byte boundary
            let block_size = (items_written + 7) & !7;
            writer.patch_u32(ph, block_size as u32);

            let padding = block_size - items_written;
            for _ in 0..padding {
                writer.write_u8(0);
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ResUi2dSystemDataInner {
    Layout(ResUi2dLayoutData),
    Pane(ResUi2dPaneData),
}

#[derive(Debug, FromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Ui2dLayoutSystemDataType {
    AnimTagName = 0,
    #[num_enum(default)]
    Unknown = 1,
}

#[derive(Debug, FromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Ui2dPaneSystemDataType {
    VertexPos0 = 0,
    VertexPos1 = 1,
    Alignment = 2,
    MaskTexture = 3,
    DropShadow = 4,
    ProceduralShape = 6,
    #[num_enum(default)]
    Invalid,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ResUi2dPaneData {
    VertexPos0(VertexPos),
    VertexPos1(VertexPos),
    ProceduralShape(ResUi2dSystemDataProceduralShape),
    Alignment(ResUi2dSystemDataAlignment),
    DropShadow(ResUi2dSystemDataDropShadow),
    MaskTexture(ResUi2dSystemDataMaskTexture),
}

impl ResUi2dPaneData {
    pub fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let offset = cursor.pos;
        let data_type: Ui2dPaneSystemDataType = cursor.read_u32()?.into();

        let res = match data_type {
            Ui2dPaneSystemDataType::VertexPos0 => Self::VertexPos0(VertexPos::parse(cursor)?),
            Ui2dPaneSystemDataType::VertexPos1 => Self::VertexPos1(VertexPos::parse(cursor)?),
            Ui2dPaneSystemDataType::MaskTexture => {
                Self::MaskTexture(ResUi2dSystemDataMaskTexture::parse(cursor)?)
            }
            Ui2dPaneSystemDataType::DropShadow => {
                Self::DropShadow(ResUi2dSystemDataDropShadow::parse(cursor)?)
            }
            Ui2dPaneSystemDataType::Alignment => {
                Self::Alignment(ResUi2dSystemDataAlignment::parse(cursor)?)
            }
            Ui2dPaneSystemDataType::ProceduralShape => {
                Self::ProceduralShape(ResUi2dSystemDataProceduralShape::parse(cursor)?)
            }
            _ => {
                return Err(FormatError::UnknownTag {
                    enum_name: "Ui2dPaneSystemDataType",
                    tag: data_type.into(),
                    offset: offset,
                });
            }
        };

        Ok(res)
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("Ui2dPaneData");

        let type_id: u32 = match self {
            ResUi2dPaneData::VertexPos0(_) => 0,
            ResUi2dPaneData::VertexPos1(_) => 1,
            ResUi2dPaneData::Alignment(_) => 2,
            ResUi2dPaneData::MaskTexture(_) => 3,
            ResUi2dPaneData::DropShadow(_) => 4,
            ResUi2dPaneData::ProceduralShape(_) => 6,
        };

        writer.write_u32(type_id);

        match self {
            ResUi2dPaneData::VertexPos0(v) | ResUi2dPaneData::VertexPos1(v) => v.serialize(writer),
            ResUi2dPaneData::Alignment(a) => a.serialize(writer),
            ResUi2dPaneData::MaskTexture(m) => m.serialize(writer),
            ResUi2dPaneData::DropShadow(d) => d.serialize(writer),
            ResUi2dPaneData::ProceduralShape(p) => p.serialize(writer),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ResUi2dLayoutData {
    AnimTagName(Vec<String>),
    Unknown,
}

impl ResUi2dLayoutData {
    pub fn parse(cursor: &mut Cursor, base_offset: usize) -> Result<Self, FormatError> {
        let data_type: Ui2dLayoutSystemDataType = cursor.read_u32()?.into();

        let res = match data_type {
            Ui2dLayoutSystemDataType::AnimTagName => {
                let string_count = cursor.read_u32()?;
                let mut strings = Vec::new();

                for _ in 0..string_count {
                    let string_offset = cursor.read_u32()?;
                    let restore_point = cursor.pos;

                    cursor.seek(base_offset + string_offset as usize);
                    let string = cursor.read_null_terminated_string()?;

                    cursor.seek(restore_point);

                    strings.push(string)
                }

                Self::AnimTagName(strings)
            }
            _ => Self::Unknown,
        };

        Ok(res)
    }

    pub fn serialize(&self, writer: &mut Writer) {
        match self {
            ResUi2dLayoutData::AnimTagName(strings) => {
                let base_offset = writer.pos();

                writer.write_u32(Ui2dLayoutSystemDataType::AnimTagName as u32);
                writer.write_u32(strings.len() as u32);

                let mut offset_positions = Vec::with_capacity(strings.len());
                for _ in strings {
                    offset_positions.push(writer.write_placeholder_u32());
                }

                let string_pool_start = writer.pos();
                for (i, string) in strings.iter().enumerate() {
                    let relative_offset = (writer.pos() - base_offset) as u32;
                    writer.patch_u32(offset_positions[i], relative_offset);
                    writer.write_null_terminated_string(string);
                }

                let bytes_written = writer.pos() - string_pool_start;

                const ALIGNMENT: usize = 64;
                let padding_needed = (ALIGNMENT - (bytes_written % ALIGNMENT)) % ALIGNMENT;
                for _ in 0..padding_needed {
                    writer.write_u8(0);
                }
            }

            ResUi2dLayoutData::Unknown => {
                writer.write_u32(0xFFFFFFFF);
                writer.write_u32(0);
            }
        }

        writer.align(4);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct ResUi2dSystemDataAlignment {
    pub options: u32,
    pub margin: f32,
}

impl ResUi2dSystemDataAlignment {
    pub fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            options: cursor.read_u32()?,
            margin: cursor.read_f32()?,
        })
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("System Data Alignment");
        writer.write_u32(self.options);
        writer.write_f32(self.margin);
    }
}

#[derive(
    Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, IntoPrimitive, FromPrimitive,
)]
#[repr(u8)]
pub enum DropShadowBlendMode {
    #[num_enum(default)]
    Normal = 0,
    Multiply = 1,
    Addition = 2,
    Subtraction = 3,
    NormalMaxAlpha = 4,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct ResUi2dSystemDataDropShadow {
    pub texture_id: u16,
    pub u_options: TexOptions,
    pub v_options: TexOptions,
    pub flags: DropShadowFlags,
    pub reserve0: [u8; 3],

    pub max_size: u8,
    pub stroke_blend_mode: DropShadowBlendMode,
    pub outer_glow_blend_mode: DropShadowBlendMode,
    pub drop_shadow_blend_mode: DropShadowBlendMode,

    pub reserve5: [u32; 4],

    pub stroke_size: f32,
    pub stroke_color: Color4f,

    pub outer_glow_color: Color4f,
    pub outer_glow_spread: f32,
    pub outer_glow_size: f32,

    pub drop_shadow_color: Color4f,
    pub drop_shadow_angle: f32,
    pub drop_shadow_distance: f32,
    pub drop_shadow_spread: f32,
    pub drop_shadow_size: f32,

    pub reserve15: u32,
    pub reserve16: u32,
    pub reserve17: u32,
    pub reserve18: u32,
}

impl ResUi2dSystemDataDropShadow {
    pub fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            texture_id: cursor.read_u16()?,
            u_options: TexOptions::decode(cursor.read_u8()?),
            v_options: TexOptions::decode(cursor.read_u8()?),
            flags: DropShadowFlags::decode(cursor.read_u8()?),
            reserve0: [cursor.read_u8()?, cursor.read_u8()?, cursor.read_u8()?],
            max_size: cursor.read_u8()?,
            stroke_blend_mode: cursor.read_u8()?.into(),
            outer_glow_blend_mode: cursor.read_u8()?.into(),
            drop_shadow_blend_mode: cursor.read_u8()?.into(),
            reserve5: [
                cursor.read_u32()?,
                cursor.read_u32()?,
                cursor.read_u32()?,
                cursor.read_u32()?,
            ],
            stroke_size: cursor.read_f32()?,
            stroke_color: Color4f::parse(cursor)?,

            outer_glow_color: Color4f::parse(cursor)?,
            outer_glow_spread: cursor.read_f32()?,
            outer_glow_size: cursor.read_f32()?,

            drop_shadow_color: Color4f::parse(cursor)?,
            drop_shadow_angle: cursor.read_f32()?,
            drop_shadow_distance: cursor.read_f32()?,
            drop_shadow_spread: cursor.read_f32()?,
            drop_shadow_size: cursor.read_f32()?,

            reserve15: cursor.read_u32()?,
            reserve16: cursor.read_u32()?,
            reserve17: cursor.read_u32()?,
            reserve18: cursor.read_u32()?,
        })
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("Drop Shadow");
        writer.write_u16(self.texture_id);
        writer.write_u8(self.u_options.encode());
        writer.write_u8(self.v_options.encode());
        writer.write_u8(self.flags.encode());

        for &b in &self.reserve0 {
            writer.write_u8(b);
        }

        writer.write_u8(self.max_size);
        writer.write_u8(self.stroke_blend_mode as u8);
        writer.write_u8(self.outer_glow_blend_mode as u8);
        writer.write_u8(self.drop_shadow_blend_mode as u8);

        for &v in &self.reserve5 {
            writer.write_u32(v);
        }

        writer.write_f32(self.stroke_size);
        self.stroke_color.serialize(writer);

        self.outer_glow_color.serialize(writer);
        writer.write_f32(self.outer_glow_spread);
        writer.write_f32(self.outer_glow_size);

        self.drop_shadow_color.serialize(writer);
        writer.write_f32(self.drop_shadow_angle);
        writer.write_f32(self.drop_shadow_distance);
        writer.write_f32(self.drop_shadow_spread);
        writer.write_f32(self.drop_shadow_size);

        writer.write_u32(self.reserve15);
        writer.write_u32(self.reserve16);
        writer.write_u32(self.reserve17);
        writer.write_u32(self.reserve18);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct ResUi2dSystemDataMaskTexture {
    pub flags: u8,
    pub reserve0: [u8; 3],
    pub texture_id: u16,
    pub u_options: u8,
    pub v_options: u8,
    pub tex_ext_flags: u32,
    pub capture_texture_id: u16,
    pub capture_u_options: u8,
    pub capture_v_options: u8,
    pub is_use_capture_mask: bool,
    pub reserve1: [u8; 3],
    pub translation: [f32; 2],
    pub rotation: f32,
    pub scale: [f32; 2],
}

impl ResUi2dSystemDataMaskTexture {
    pub fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            flags: cursor.read_u8()?,
            reserve0: [cursor.read_u8()?, cursor.read_u8()?, cursor.read_u8()?],
            texture_id: cursor.read_u16()?,
            u_options: cursor.read_u8()?,
            v_options: cursor.read_u8()?,
            tex_ext_flags: cursor.read_u32()?,
            capture_texture_id: cursor.read_u16()?,
            capture_u_options: cursor.read_u8()?,
            capture_v_options: cursor.read_u8()?,
            is_use_capture_mask: cursor.read_u8()? != 0,
            reserve1: [cursor.read_u8()?, cursor.read_u8()?, cursor.read_u8()?],
            translation: [cursor.read_f32()?, cursor.read_f32()?],
            rotation: cursor.read_f32()?,
            scale: [cursor.read_f32()?, cursor.read_f32()?],
        })
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("Mask Texture");

        writer.write_u8(self.flags);

        for &b in &self.reserve0 {
            writer.write_u8(b);
        }

        writer.write_u16(self.texture_id);
        writer.write_u8(self.u_options);
        writer.write_u8(self.v_options);
        writer.write_u32(self.tex_ext_flags);
        writer.write_u16(self.capture_texture_id);
        writer.write_u8(self.capture_u_options);
        writer.write_u8(self.capture_v_options);
        writer.write_u8(self.is_use_capture_mask.into());

        for &b in &self.reserve1 {
            writer.write_u8(b);
        }

        for &f in &self.translation {
            writer.write_f32(f);
        }

        writer.write_f32(self.rotation);
        for &f in &self.scale {
            writer.write_f32(f);
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct ResUi2dSystemDataProceduralShape {
    pub options: u8,
    pub color0_options: u8,
    pub inner_shadow_options: u8,
    pub inner_shadow_base_comp: u8,
    pub color_overlay_options: u8,
    pub gradation_overlay_options: u8,
    pub drop_shadow_blend_mode: u8,
    pub drop_shadow_base_comp: u8,
    pub reserve0: [u32; 4],
    pub rounded_corner0: [f32; 4],
    pub rounded_corner1: [f32; 4],
    pub reserve1: f32,
    pub color0: Color4f,
    pub inner_shadow_color: Color4f,
    pub inner_shadow_transform: [f32; 3],
    pub color_overlay: Color4f,
    pub gradation_weights: [f32; 4],
    pub gradation_color_array: [Color4f; 4],
    pub gradation_rotation: f32,
    pub drop_shadow_color: Color4f,
    pub drop_shadow_transform: [f32; 3],
    pub reserve2: [u32; 4],
}

impl ResUi2dSystemDataProceduralShape {
    pub fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            options: cursor.read_u8()?,
            color0_options: cursor.read_u8()?,
            inner_shadow_options: cursor.read_u8()?,
            inner_shadow_base_comp: cursor.read_u8()?,
            color_overlay_options: cursor.read_u8()?,
            gradation_overlay_options: cursor.read_u8()?,
            drop_shadow_blend_mode: cursor.read_u8()?,
            drop_shadow_base_comp: cursor.read_u8()?,
            reserve0: [
                cursor.read_u32()?,
                cursor.read_u32()?,
                cursor.read_u32()?,
                cursor.read_u32()?,
            ],
            rounded_corner0: [
                cursor.read_f32()?,
                cursor.read_f32()?,
                cursor.read_f32()?,
                cursor.read_f32()?,
            ],
            rounded_corner1: [
                cursor.read_f32()?,
                cursor.read_f32()?,
                cursor.read_f32()?,
                cursor.read_f32()?,
            ],
            reserve1: cursor.read_f32()?,
            color0: Color4f::parse(cursor)?,
            inner_shadow_color: Color4f::parse(cursor)?,
            inner_shadow_transform: [cursor.read_f32()?, cursor.read_f32()?, cursor.read_f32()?],
            color_overlay: Color4f::parse(cursor)?,
            gradation_weights: [
                cursor.read_f32()?,
                cursor.read_f32()?,
                cursor.read_f32()?,
                cursor.read_f32()?,
            ],
            gradation_color_array: [
                Color4f::parse(cursor)?,
                Color4f::parse(cursor)?,
                Color4f::parse(cursor)?,
                Color4f::parse(cursor)?,
            ],
            gradation_rotation: cursor.read_f32()?,
            drop_shadow_color: Color4f::parse(cursor)?,
            drop_shadow_transform: [cursor.read_f32()?, cursor.read_f32()?, cursor.read_f32()?],
            reserve2: [
                cursor.read_u32()?,
                cursor.read_u32()?,
                cursor.read_u32()?,
                cursor.read_u32()?,
            ],
        })
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("Procedural Shape");
        writer.write_u8(self.options);
        writer.write_u8(self.color0_options);
        writer.write_u8(self.inner_shadow_options);
        writer.write_u8(self.inner_shadow_base_comp);
        writer.write_u8(self.color_overlay_options);
        writer.write_u8(self.gradation_overlay_options);
        writer.write_u8(self.drop_shadow_blend_mode);
        writer.write_u8(self.drop_shadow_base_comp);

        for &b in &self.reserve0 {
            writer.write_u32(b);
        }

        for &f in &self.rounded_corner0 {
            writer.write_f32(f);
        }

        for &f in &self.rounded_corner1 {
            writer.write_f32(f);
        }

        writer.write_f32(self.reserve1);

        self.color0.serialize(writer);
        self.inner_shadow_color.serialize(writer);

        for &f in &self.inner_shadow_transform {
            writer.write_f32(f);
        }

        self.color_overlay.serialize(writer);

        for &f in &self.gradation_weights {
            writer.write_f32(f);
        }

        for c in &self.gradation_color_array {
            c.serialize(writer);
        }

        writer.write_f32(self.gradation_rotation);
        self.drop_shadow_color.serialize(writer);

        for &f in &self.drop_shadow_transform {
            writer.write_f32(f);
        }

        for &v in &self.reserve2 {
            writer.write_u32(v);
        }
    }
}

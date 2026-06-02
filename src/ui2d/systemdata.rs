use serde::{Deserialize, Serialize};

use crate::{
    core::{Cursor, Writer},
    ui2d::types::{Color4f, VertexPos},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResUi2dSystemDataArray {
    pub reserve0: u16,
    pub count: u16,
    pub offset: u32,
    pub data_array: Vec<ResUi2dSystemDataInner>,
}

impl ResUi2dSystemDataArray {
    pub fn parse(cursor: &mut Cursor, is_pane: bool) -> Self {
        let base_offset = cursor.pos;
        let reserve0 = cursor.read_u16();
        let count = cursor.read_u16();
        let offset = cursor.read_u32();
        let mut data_array = Vec::new();

        if offset > 0 {
            let post_header_point = cursor.pos;
            cursor.seek(base_offset + offset as usize);

            let mut item_offsets = Vec::new();
            for _ in 0..count {
                item_offsets.push(cursor.read_u32());
            }

            for i in 0..count as usize {
                cursor.seek(base_offset + offset as usize + item_offsets[i] as usize);

                if is_pane {
                    data_array.push(ResUi2dSystemDataInner::Pane(ResUi2dPaneData::parse(cursor)));
                } else {
                    data_array.push(ResUi2dSystemDataInner::Layout(ResUi2dLayoutData::parse(
                        cursor,
                        post_header_point,
                    )));
                }
            }

            /*if cursor.pos < (base_offset + offset as usize) {
                cursor.seek(post_header_point);
            }*/
        };

        Self {
            reserve0,
            count,
            offset,
            data_array,
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        let base_offset = writer.pos();

        writer.write_u16(self.reserve0);
        writer.write_u16(self.data_array.len() as u16);
        let offset_pos = writer.write_placeholder_u32();

        if !self.data_array.is_empty() {
            writer.patch_u32(offset_pos, (writer.pos() - base_offset) as u32);

            for item in self.data_array.iter() {
                match item {
                    ResUi2dSystemDataInner::Pane(pane) => {
                        pane.serialize(writer);
                    }
                    ResUi2dSystemDataInner::Layout(layout) => {
                        layout.serialize(writer);
                    }
                }

                writer.align(4);
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ResUi2dSystemDataInner {
    Layout(ResUi2dLayoutData),
    Pane(ResUi2dPaneData),
}

#[repr(u32)]
pub enum Ui2dLayoutSystemDataType {
    AnimTagName = 0,
    Unknown = 1,
}

impl From<u32> for Ui2dLayoutSystemDataType {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::AnimTagName,
            _ => Self::Unknown,
        }
    }
}

#[repr(u32)]
pub enum Ui2dPaneSystemDataType {
    VertexPos0 = 0,
    VertexPos1 = 1,
    Alignment = 2,
    MaskTexture = 3,
    DropShadow = 4,
    ProceduralShape = 6,
}

impl From<u32> for Ui2dPaneSystemDataType {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::VertexPos0,
            1 => Self::VertexPos1,
            2 => Self::Alignment,
            3 => Self::MaskTexture,
            4 => Self::DropShadow,
            6 => Self::ProceduralShape,
            _ => Self::VertexPos0,
        }
    }
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
    pub fn parse(cursor: &mut Cursor) -> Self {
        let data_type: Ui2dPaneSystemDataType = cursor.read_u32().into();
        let _size = cursor.read_u32();

        match data_type {
            Ui2dPaneSystemDataType::VertexPos0 => Self::VertexPos0(VertexPos::parse(cursor)),
            Ui2dPaneSystemDataType::VertexPos1 => Self::VertexPos1(VertexPos::parse(cursor)),
            Ui2dPaneSystemDataType::MaskTexture => {
                Self::MaskTexture(ResUi2dSystemDataMaskTexture::parse(cursor))
            }
            Ui2dPaneSystemDataType::DropShadow => {
                Self::DropShadow(ResUi2dSystemDataDropShadow::parse(cursor))
            }
            Ui2dPaneSystemDataType::Alignment => {
                Self::Alignment(ResUi2dSystemDataAlignment::parse(cursor))
            }
            Ui2dPaneSystemDataType::ProceduralShape => {
                Self::ProceduralShape(ResUi2dSystemDataProceduralShape::parse(cursor))
            }
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        let type_id = match self {
            ResUi2dPaneData::VertexPos0(_) => 0,
            ResUi2dPaneData::VertexPos1(_) => 1,
            ResUi2dPaneData::Alignment(_) => 2,
            ResUi2dPaneData::MaskTexture(_) => 3,
            ResUi2dPaneData::DropShadow(_) => 4,
            ResUi2dPaneData::ProceduralShape(_) => 6,
        };

        writer.write_u32(type_id);
        let size_pos = writer.write_placeholder_u32();

        let payload_start = writer.pos();

        match self {
            ResUi2dPaneData::VertexPos0(v) | ResUi2dPaneData::VertexPos1(v) => v.serialize(writer),
            ResUi2dPaneData::Alignment(a) => a.serialize(writer),
            ResUi2dPaneData::MaskTexture(m) => m.serialize(writer),
            ResUi2dPaneData::DropShadow(d) => d.serialize(writer),
            ResUi2dPaneData::ProceduralShape(p) => p.serialize(writer),
        }

        let payload_size = (writer.pos() - payload_start) as u32;
        writer.patch_u32(size_pos, payload_size);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ResUi2dLayoutData {
    AnimTagName(Vec<String>),
    Unknown,
}

impl ResUi2dLayoutData {
    pub fn parse(cursor: &mut Cursor, base_offset: usize) -> Self {
        let data_type: Ui2dLayoutSystemDataType = cursor.read_u32().into();

        match data_type {
            Ui2dLayoutSystemDataType::AnimTagName => {
                let string_count = cursor.read_u32();
                let mut strings = Vec::new();

                for _ in 0..string_count {
                    let string_offset = cursor.read_u32();
                    let restore_point = cursor.pos;

                    cursor.seek(base_offset + string_offset as usize);
                    let string = cursor.read_null_terminated_string();

                    cursor.seek(restore_point);

                    strings.push(string)
                }

                Self::AnimTagName(strings)
            }
            _ => Self::Unknown,
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        match self {
            ResUi2dLayoutData::AnimTagName(strings) => {
                let type_val = Ui2dLayoutSystemDataType::AnimTagName as u32;
                let base_offset = writer.pos();

                writer.write_u32(type_val);
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

                const FIXED_BUFFER_SIZE: usize = 128;
                if bytes_written < FIXED_BUFFER_SIZE {
                    let padding_needed = FIXED_BUFFER_SIZE - bytes_written;
                    for _ in 0..padding_needed {
                        writer.write_u8(0);
                    }
                } else if bytes_written > FIXED_BUFFER_SIZE {
                    panic!("Strings exceed the fixed 128-byte layout editor buffer!");
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
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            options: cursor.read_u32(),
            margin: cursor.read_f32(),
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_u32(self.options);
        writer.write_f32(self.margin);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct ResUi2dSystemDataDropShadow {
    pub texture_id: u16,
    pub u_options: u8,
    pub v_options: u8,
    pub flags: u8,
    pub reserve0: [u8; 3],
    pub reserve1: u8,
    pub reserve2: u8,
    pub reserve3: u8,
    pub reserve4: u8,
    pub reserve5: [u32; 5],
    pub reserve6: [f32; 2],
    pub reserve7: [f32; 2],
    pub reserve8: [f32; 2],
    pub reserve9: [f32; 2],
    pub reserve10: [f32; 2],
    pub reserve11: [f32; 2],
    pub reserve12: [f32; 2],
    pub reserve13: [f32; 2],
    pub reserve14: [f32; 2],
    pub reserve15: u32,
}

impl ResUi2dSystemDataDropShadow {
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            texture_id: cursor.read_u16(),
            u_options: cursor.read_u8(),
            v_options: cursor.read_u8(),
            flags: cursor.read_u8(),
            reserve0: [cursor.read_u8(), cursor.read_u8(), cursor.read_u8()],
            reserve1: cursor.read_u8(),
            reserve2: cursor.read_u8(),
            reserve3: cursor.read_u8(),
            reserve4: cursor.read_u8(),
            reserve5: [
                cursor.read_u32(),
                cursor.read_u32(),
                cursor.read_u32(),
                cursor.read_u32(),
                cursor.read_u32(),
            ],
            reserve6: [cursor.read_f32(), cursor.read_f32()],
            reserve7: [cursor.read_f32(), cursor.read_f32()],
            reserve8: [cursor.read_f32(), cursor.read_f32()],
            reserve9: [cursor.read_f32(), cursor.read_f32()],
            reserve10: [cursor.read_f32(), cursor.read_f32()],
            reserve11: [cursor.read_f32(), cursor.read_f32()],
            reserve12: [cursor.read_f32(), cursor.read_f32()],
            reserve13: [cursor.read_f32(), cursor.read_f32()],
            reserve14: [cursor.read_f32(), cursor.read_f32()],
            reserve15: cursor.read_u32(),
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_u16(self.texture_id);
        writer.write_u8(self.u_options);
        writer.write_u8(self.v_options);
        writer.write_u8(self.flags);

        for &byte in &self.reserve0 {
            writer.write_u8(byte);
        }

        writer.write_u8(self.reserve1);
        writer.write_u8(self.reserve2);
        writer.write_u8(self.reserve3);
        writer.write_u8(self.reserve4);

        for &val in &self.reserve5 {
            writer.write_u32(val);
        }

        for &f in &self.reserve6 {
            writer.write_f32(f);
        }

        for &f in &self.reserve7 {
            writer.write_f32(f);
        }

        for &f in &self.reserve8 {
            writer.write_f32(f);
        }

        for &f in &self.reserve9 {
            writer.write_f32(f);
        }

        for &f in &self.reserve10 {
            writer.write_f32(f);
        }

        for &f in &self.reserve11 {
            writer.write_f32(f);
        }

        for &f in &self.reserve12 {
            writer.write_f32(f);
        }

        for &f in &self.reserve13 {
            writer.write_f32(f);
        }

        for &f in &self.reserve14 {
            writer.write_f32(f);
        }
        writer.write_u32(self.reserve15);
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
    pub is_use_capture_mask: u8,
    pub reserve1: [u8; 3],
    pub translation: [f32; 2],
    pub rotation: f32,
    pub scale: [f32; 2],
}

impl ResUi2dSystemDataMaskTexture {
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            flags: cursor.read_u8(),
            reserve0: [cursor.read_u8(), cursor.read_u8(), cursor.read_u8()],
            texture_id: cursor.read_u16(),
            u_options: cursor.read_u8(),
            v_options: cursor.read_u8(),
            tex_ext_flags: cursor.read_u32(),
            capture_texture_id: cursor.read_u16(),
            capture_u_options: cursor.read_u8(),
            capture_v_options: cursor.read_u8(),
            is_use_capture_mask: cursor.read_u8(),
            reserve1: [cursor.read_u8(), cursor.read_u8(), cursor.read_u8()],
            translation: [cursor.read_f32(), cursor.read_f32()],
            rotation: cursor.read_f32(),
            scale: [cursor.read_f32(), cursor.read_f32()],
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_u8(self.flags);
        for &byte in &self.reserve0 {
            writer.write_u8(byte);
        }
        writer.write_u16(self.texture_id);
        writer.write_u8(self.u_options);
        writer.write_u8(self.v_options);
        writer.write_u32(self.tex_ext_flags);
        writer.write_u16(self.capture_texture_id);
        writer.write_u8(self.capture_u_options);
        writer.write_u8(self.capture_v_options);
        writer.write_u8(self.is_use_capture_mask);
        for &byte in &self.reserve1 {
            writer.write_u8(byte);
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
    pub reserve0: [u8; 4],
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
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            options: cursor.read_u8(),
            color0_options: cursor.read_u8(),
            inner_shadow_options: cursor.read_u8(),
            inner_shadow_base_comp: cursor.read_u8(),
            color_overlay_options: cursor.read_u8(),
            gradation_overlay_options: cursor.read_u8(),
            drop_shadow_blend_mode: cursor.read_u8(),
            drop_shadow_base_comp: cursor.read_u8(),
            reserve0: [
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
            ],
            rounded_corner0: [
                cursor.read_f32(),
                cursor.read_f32(),
                cursor.read_f32(),
                cursor.read_f32(),
            ],
            rounded_corner1: [
                cursor.read_f32(),
                cursor.read_f32(),
                cursor.read_f32(),
                cursor.read_f32(),
            ],
            reserve1: cursor.read_f32(),
            color0: Color4f::parse(cursor),
            inner_shadow_color: Color4f::parse(cursor),
            inner_shadow_transform: [cursor.read_f32(), cursor.read_f32(), cursor.read_f32()],
            color_overlay: Color4f::parse(cursor),
            gradation_weights: [
                cursor.read_f32(),
                cursor.read_f32(),
                cursor.read_f32(),
                cursor.read_f32(),
            ],
            gradation_color_array: [
                Color4f::parse(cursor),
                Color4f::parse(cursor),
                Color4f::parse(cursor),
                Color4f::parse(cursor),
            ],
            gradation_rotation: cursor.read_f32(),
            drop_shadow_color: Color4f::parse(cursor),
            drop_shadow_transform: [cursor.read_f32(), cursor.read_f32(), cursor.read_f32()],
            reserve2: [
                cursor.read_u32(),
                cursor.read_u32(),
                cursor.read_u32(),
                cursor.read_u32(),
            ],
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_u8(self.options);
        writer.write_u8(self.color0_options);
        writer.write_u8(self.inner_shadow_options);
        writer.write_u8(self.inner_shadow_base_comp);
        writer.write_u8(self.color_overlay_options);
        writer.write_u8(self.gradation_overlay_options);
        writer.write_u8(self.drop_shadow_blend_mode);
        writer.write_u8(self.drop_shadow_base_comp);

        for &byte in &self.reserve0 {
            writer.write_u8(byte);
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

        for color in &self.gradation_color_array {
            color.serialize(writer);
        }

        writer.write_f32(self.gradation_rotation);
        self.drop_shadow_color.serialize(writer);

        for &f in &self.drop_shadow_transform {
            writer.write_f32(f);
        }

        for &val in &self.reserve2 {
            writer.write_u32(val);
        }
    }
}

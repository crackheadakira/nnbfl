use serde::{Deserialize, Serialize};

use crate::core::{Cursor, Writer};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Color4u8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color4u8 {
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            r: cursor.read_u8(),
            g: cursor.read_u8(),
            b: cursor.read_u8(),
            a: cursor.read_u8(),
        }
    }
    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_u8(self.r);
        writer.write_u8(self.g);
        writer.write_u8(self.b);
        writer.write_u8(self.a);
    }
}

pub const PANE_NAME_LEN: usize = 0x18;
pub const USER_NAME_LEN: usize = 0x08;

#[derive(Debug, Serialize, Deserialize)]
pub struct BflytPane {
    pub pane_flags: u8,
    pub origin: u8,
    pub alpha: u8,
    pub flag_ex: u8,
    pub pane_name: String,
    pub user_name: String,
    pub translation_x: f32,
    pub translation_y: f32,
    pub translation_z: f32,
    pub rotation_x: f32,
    pub rotation_y: f32,
    pub rotation_z: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub size_x: f32,
    pub size_y: f32,
}

impl BflytPane {
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            pane_flags: cursor.read_u8(),
            origin: cursor.read_u8(),
            alpha: cursor.read_u8(),
            flag_ex: cursor.read_u8(),
            pane_name: cursor.read_fixed_string(PANE_NAME_LEN),
            user_name: cursor.read_fixed_string(USER_NAME_LEN),
            translation_x: cursor.read_f32(),
            translation_y: cursor.read_f32(),
            translation_z: cursor.read_f32(),
            rotation_x: cursor.read_f32(),
            rotation_y: cursor.read_f32(),
            rotation_z: cursor.read_f32(),
            scale_x: cursor.read_f32(),
            scale_y: cursor.read_f32(),
            size_x: cursor.read_f32(),
            size_y: cursor.read_f32(),
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_u8(self.pane_flags);
        writer.write_u8(self.origin);
        writer.write_u8(self.alpha);
        writer.write_u8(self.flag_ex);
        writer.write_fixed_string(&self.pane_name, PANE_NAME_LEN);
        writer.write_fixed_string(&self.user_name, USER_NAME_LEN);
        writer.write_f32(self.translation_x);
        writer.write_f32(self.translation_y);
        writer.write_f32(self.translation_z);
        writer.write_f32(self.rotation_x);
        writer.write_f32(self.rotation_y);
        writer.write_f32(self.rotation_z);
        writer.write_f32(self.scale_x);
        writer.write_f32(self.scale_y);
        writer.write_f32(self.size_x);
        writer.write_f32(self.size_y);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextureUv {
    pub top_left_x: f32,
    pub top_left_y: f32,
    pub top_right_x: f32,
    pub top_right_y: f32,
    pub bottom_left_x: f32,
    pub bottom_left_y: f32,
    pub bottom_right_x: f32,
    pub bottom_right_y: f32,
}

impl TextureUv {
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            top_left_x: cursor.read_f32(),
            top_left_y: cursor.read_f32(),
            top_right_x: cursor.read_f32(),
            top_right_y: cursor.read_f32(),
            bottom_left_x: cursor.read_f32(),
            bottom_left_y: cursor.read_f32(),
            bottom_right_x: cursor.read_f32(),
            bottom_right_y: cursor.read_f32(),
        }
    }
    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_f32(self.top_left_x);
        writer.write_f32(self.top_left_y);
        writer.write_f32(self.top_right_x);
        writer.write_f32(self.top_right_y);
        writer.write_f32(self.bottom_left_x);
        writer.write_f32(self.bottom_left_y);
        writer.write_f32(self.bottom_right_x);
        writer.write_f32(self.bottom_right_y);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BflytPicturePane {
    pub base: BflytPane,
    pub top_left_vertex_color: Color4u8,
    pub top_right_vertex_color: Color4u8,
    pub bottom_left_vertex_color: Color4u8,
    pub bottom_right_vertex_color: Color4u8,
    pub material_index: u16,
    pub texture_count: u8,
    pub is_shape: u8,
    pub texture_uvs: Vec<TextureUv>,
}

impl BflytPicturePane {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let base = BflytPane::parse(cursor);
        let top_left_vertex_color = Color4u8::parse(cursor);
        let top_right_vertex_color = Color4u8::parse(cursor);
        let bottom_left_vertex_color = Color4u8::parse(cursor);
        let bottom_right_vertex_color = Color4u8::parse(cursor);
        let material_index = cursor.read_u16();
        let texture_count = cursor.read_u8();
        let is_shape = cursor.read_u8();
        let mut texture_uvs = Vec::new();
        for _ in 0..texture_count {
            texture_uvs.push(TextureUv::parse(cursor));
        }
        Self {
            base,
            top_left_vertex_color,
            top_right_vertex_color,
            bottom_left_vertex_color,
            bottom_right_vertex_color,
            material_index,
            texture_count,
            is_shape,
            texture_uvs,
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        self.base.serialize(writer);
        self.top_left_vertex_color.serialize(writer);
        self.top_right_vertex_color.serialize(writer);
        self.bottom_left_vertex_color.serialize(writer);
        self.bottom_right_vertex_color.serialize(writer);
        writer.write_u16(self.material_index);
        writer.write_u8(self.texture_uvs.len() as u8);
        writer.write_u8(self.is_shape);
        for uv in &self.texture_uvs {
            uv.serialize(writer);
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PerCharacterTransform {
    pub eval_time_offset: f32,
    pub eval_time_width: f32,
    pub loop_type: u8,
    pub origin_v: u8,
    pub has_anim_info: u8,
    pub reserve0: u8,
    pub reserve1: u32,
}

impl PerCharacterTransform {
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            eval_time_offset: cursor.read_f32(),
            eval_time_width: cursor.read_f32(),
            loop_type: cursor.read_u8(),
            origin_v: cursor.read_u8(),
            has_anim_info: cursor.read_u8(),
            reserve0: cursor.read_u8(),
            reserve1: cursor.read_u32(),
        }
    }
    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_f32(self.eval_time_offset);
        writer.write_f32(self.eval_time_width);
        writer.write_u8(self.loop_type);
        writer.write_u8(self.origin_v);
        writer.write_u8(self.has_anim_info);
        writer.write_u8(self.reserve0);
        writer.write_u32(self.reserve1);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BflytTextBoxPane {
    pub base: BflytPane,
    pub text_buffer_size: u16,
    pub text_length: u16,
    pub material_index: u16,
    pub font_index: u16,
    pub text_origin: u8,
    pub line_alignment: u8,
    pub text_flags: u16,
    pub italic_tilt: f32,
    pub text_offset: u32,
    pub font_top_color: Color4u8,
    pub font_bottom_color: Color4u8,
    pub font_size_x: f32,
    pub font_size_y: f32,
    pub character_space: f32,
    pub line_space: f32,
    pub label_offset: u32,
    pub shadow_translation_x: f32,
    pub shadow_translation_y: f32,
    pub shadow_size_x: f32,
    pub shadow_size_y: f32,
    pub shadow_top_color: Color4u8,
    pub shadow_bottom_color: Color4u8,
    pub shadow_italic_tilt: f32,
    pub line_transform_offset: u32,
    pub per_character_transform_offset: u32,
    pub text: Option<Vec<u16>>,
    pub label: Option<String>,
    pub per_character_transforms: Option<Vec<PerCharacterTransform>>,
}

impl BflytTextBoxPane {
    pub fn parse(cursor: &mut Cursor, section_start: usize, section_end: usize) -> Self {
        let base = BflytPane::parse(cursor);
        let txt1_base = section_start + 8;

        let text_buffer_size = cursor.read_u16();
        let text_length = cursor.read_u16();
        let material_index = cursor.read_u16();
        let font_index = cursor.read_u16();
        let text_origin = cursor.read_u8();
        let line_alignment = cursor.read_u8();
        let text_flags = cursor.read_u16();
        let italic_tilt = cursor.read_f32();
        let text_offset = cursor.read_u32();
        let font_top_color = Color4u8::parse(cursor);
        let font_bottom_color = Color4u8::parse(cursor);
        let font_size_x = cursor.read_f32();
        let font_size_y = cursor.read_f32();
        let character_space = cursor.read_f32();
        let line_space = cursor.read_f32();
        let label_offset = cursor.read_u32();
        let shadow_translation_x = cursor.read_f32();
        let shadow_translation_y = cursor.read_f32();
        let shadow_size_x = cursor.read_f32();
        let shadow_size_y = cursor.read_f32();
        let shadow_top_color = Color4u8::parse(cursor);
        let shadow_bottom_color = Color4u8::parse(cursor);
        let shadow_italic_tilt = cursor.read_f32();
        let line_transform_offset = cursor.read_u32();
        let per_character_transform_offset = cursor.read_u32();

        let is_per_character = (text_flags & (1 << 4)) != 0;

        let text = if text_offset != 0 {
            let addr = txt1_base + text_offset as usize - 8;
            let saved = cursor.pos;
            cursor.seek(addr);
            let char_count = (text_length / 2) as usize;
            let mut chars = Vec::with_capacity(char_count);
            for _ in 0..char_count {
                chars.push(cursor.read_u16());
            }
            cursor.seek(saved);
            Some(chars)
        } else {
            None
        };

        let label = if label_offset != 0 {
            let addr = txt1_base + label_offset as usize - 8;
            let saved = cursor.pos;
            cursor.seek(addr);
            let s = cursor.read_null_terminated_string();
            cursor.seek(saved);
            Some(s)
        } else {
            None
        };

        let per_character_transforms = if is_per_character && per_character_transform_offset != 0 {
            let addr = txt1_base + per_character_transform_offset as usize - 8;
            let saved = cursor.pos;
            cursor.seek(addr);

            let transform_size = std::mem::size_of::<PerCharacterTransform>();

            let mut transforms = Vec::new();
            while cursor.pos + transform_size <= section_end {
                transforms.push(PerCharacterTransform::parse(cursor));
            }

            cursor.seek(saved);
            Some(transforms)
        } else {
            None
        };

        Self {
            base,
            text_buffer_size,
            text_length,
            material_index,
            font_index,
            text_origin,
            line_alignment,
            text_flags,
            italic_tilt,
            text_offset,
            font_top_color,
            font_bottom_color,
            font_size_x,
            font_size_y,
            character_space,
            line_space,
            label_offset,
            shadow_translation_x,
            shadow_translation_y,
            shadow_size_x,
            shadow_size_y,
            shadow_top_color,
            shadow_bottom_color,
            shadow_italic_tilt,
            line_transform_offset,
            per_character_transform_offset,
            text,
            label,
            per_character_transforms,
        }
    }

    pub fn serialize(&self, writer: &mut Writer, section_start: usize) {
        let txt1_base = section_start + 8;
        self.base.serialize(writer);

        writer.write_u16(self.text_buffer_size);
        writer.write_u16(self.text_length);
        writer.write_u16(self.material_index);
        writer.write_u16(self.font_index);
        writer.write_u8(self.text_origin);
        writer.write_u8(self.line_alignment);
        writer.write_u16(self.text_flags);
        writer.write_f32(self.italic_tilt);

        let text_offset_pos = writer.write_placeholder_u32();
        self.font_top_color.serialize(writer);
        self.font_bottom_color.serialize(writer);
        writer.write_f32(self.font_size_x);
        writer.write_f32(self.font_size_y);
        writer.write_f32(self.character_space);
        writer.write_f32(self.line_space);
        let label_offset_pos = writer.write_placeholder_u32();
        writer.write_f32(self.shadow_translation_x);
        writer.write_f32(self.shadow_translation_y);
        writer.write_f32(self.shadow_size_x);
        writer.write_f32(self.shadow_size_y);
        self.shadow_top_color.serialize(writer);
        self.shadow_bottom_color.serialize(writer);
        writer.write_f32(self.shadow_italic_tilt);
        writer.write_u32(self.line_transform_offset);
        let per_char_offset_pos = writer.write_placeholder_u32();

        if let Some(text) = &self.text {
            let offset = writer.pos() - txt1_base + 8;
            writer.patch_u32(text_offset_pos, offset as u32);
            for ch in text {
                writer.write_u16(*ch);
            }
        } else {
            writer.patch_u32(text_offset_pos, 0);
        }

        if let Some(label) = &self.label {
            writer.align(4);
            let offset = writer.pos() - txt1_base + 8;
            writer.patch_u32(label_offset_pos, offset as u32);
            writer.write_null_terminated_string(label);
        } else {
            writer.patch_u32(label_offset_pos, 0);
        }

        if let Some(transforms) = &self.per_character_transforms {
            writer.align(4);
            let offset = writer.pos() - txt1_base + 8;
            writer.patch_u32(per_char_offset_pos, offset as u32);
            for t in transforms {
                t.serialize(writer);
            }
        } else {
            writer.patch_u32(per_char_offset_pos, 0);
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WindowContent {
    pub top_left_vertex_color: Color4u8,
    pub top_right_vertex_color: Color4u8,
    pub bottom_left_vertex_color: Color4u8,
    pub bottom_right_vertex_color: Color4u8,
    pub material_index: u16,
    pub uv_coordinate_count: u8,
    pub reserve0: u8,
    pub uvs: Vec<[f32; 2]>,
}

impl WindowContent {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let top_left_vertex_color = Color4u8::parse(cursor);
        let top_right_vertex_color = Color4u8::parse(cursor);
        let bottom_left_vertex_color = Color4u8::parse(cursor);
        let bottom_right_vertex_color = Color4u8::parse(cursor);
        let material_index = cursor.read_u16();
        let uv_coordinate_count = cursor.read_u8();
        let reserve0 = cursor.read_u8();
        let mut uvs = Vec::new();
        for _ in 0..uv_coordinate_count {
            uvs.push([cursor.read_f32(), cursor.read_f32()]);
        }
        Self {
            top_left_vertex_color,
            top_right_vertex_color,
            bottom_left_vertex_color,
            bottom_right_vertex_color,
            material_index,
            uv_coordinate_count,
            reserve0,
            uvs,
        }
    }
    pub fn serialize(&self, writer: &mut Writer) {
        self.top_left_vertex_color.serialize(writer);
        self.top_right_vertex_color.serialize(writer);
        self.bottom_left_vertex_color.serialize(writer);
        self.bottom_right_vertex_color.serialize(writer);
        writer.write_u16(self.material_index);
        writer.write_u8(self.uvs.len() as u8);
        writer.write_u8(self.reserve0);
        for uv in &self.uvs {
            writer.write_f32(uv[0]);
            writer.write_f32(uv[1]);
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WindowFrame {
    pub material_index: u16,
    pub texture_flip_mode: u8,
    pub reserve0: u8,
}

impl WindowFrame {
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            material_index: cursor.read_u16(),
            texture_flip_mode: cursor.read_u8(),
            reserve0: cursor.read_u8(),
        }
    }
    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_u16(self.material_index);
        writer.write_u8(self.texture_flip_mode);
        writer.write_u8(self.reserve0);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BflytWindowPane {
    pub base: BflytPane,
    pub inflation_left: i16,
    pub inflation_right: i16,
    pub inflation_top: i16,
    pub inflation_bottom: i16,
    pub frame_size_left: i16,
    pub frame_size_right: i16,
    pub frame_size_top: i16,
    pub frame_size_bottom: i16,
    pub frame_count: u8,
    pub flag: u8,
    pub reserve0: u16,
    pub content_offset: u32,
    pub frame_offset_array_offset: u32,
    pub content: WindowContent,
    pub frames: Vec<WindowFrame>,
}

impl BflytWindowPane {
    pub fn parse(cursor: &mut Cursor, section_start: usize) -> Self {
        let base = BflytPane::parse(cursor);
        let wnd_base = section_start + 8;

        let inflation_left = cursor.read_u16() as i16;
        let inflation_right = cursor.read_u16() as i16;
        let inflation_top = cursor.read_u16() as i16;
        let inflation_bottom = cursor.read_u16() as i16;
        let frame_size_left = cursor.read_u16() as i16;
        let frame_size_right = cursor.read_u16() as i16;
        let frame_size_top = cursor.read_u16() as i16;
        let frame_size_bottom = cursor.read_u16() as i16;
        let frame_count = cursor.read_u8();
        let flag = cursor.read_u8();
        let reserve0 = cursor.read_u16();
        let content_offset = cursor.read_u32();
        let frame_offset_array_offset = cursor.read_u32();

        let saved = cursor.pos;

        cursor.seek(wnd_base + content_offset as usize);
        let content = WindowContent::parse(cursor);

        cursor.seek(wnd_base + frame_offset_array_offset as usize);
        let mut frame_offsets = Vec::new();
        for _ in 0..frame_count {
            frame_offsets.push(cursor.read_u32());
        }

        let mut frames = Vec::new();
        for offset in frame_offsets {
            cursor.seek(wnd_base + offset as usize);
            frames.push(WindowFrame::parse(cursor));
        }

        cursor.seek(saved);

        Self {
            base,
            inflation_left,
            inflation_right,
            inflation_top,
            inflation_bottom,
            frame_size_left,
            frame_size_right,
            frame_size_top,
            frame_size_bottom,
            frame_count,
            flag,
            reserve0,
            content_offset,
            frame_offset_array_offset,
            content,
            frames,
        }
    }

    pub fn serialize(&self, writer: &mut Writer, section_start: usize) {
        let wnd_base = section_start + 8;
        self.base.serialize(writer);

        writer.write_u16(self.inflation_left as u16);
        writer.write_u16(self.inflation_right as u16);
        writer.write_u16(self.inflation_top as u16);
        writer.write_u16(self.inflation_bottom as u16);
        writer.write_u16(self.frame_size_left as u16);
        writer.write_u16(self.frame_size_right as u16);
        writer.write_u16(self.frame_size_top as u16);
        writer.write_u16(self.frame_size_bottom as u16);
        writer.write_u8(self.frames.len() as u8);
        writer.write_u8(self.flag);
        writer.write_u16(self.reserve0);

        let content_offset_pos = writer.write_placeholder_u32();
        let frame_offsets_pos = writer.write_placeholder_u32();

        let content_off = writer.pos() - wnd_base;
        writer.patch_u32(content_offset_pos, content_off as u32);
        self.content.serialize(writer);
        writer.align(4);

        let frame_table_off = writer.pos() - wnd_base;
        writer.patch_u32(frame_offsets_pos, frame_table_off as u32);

        let mut frame_off_placeholders = Vec::new();
        for _ in &self.frames {
            frame_off_placeholders.push(writer.write_placeholder_u32());
        }

        for (i, frame) in self.frames.iter().enumerate() {
            let frame_off = writer.pos() - wnd_base;
            writer.patch_u32(frame_off_placeholders[i], frame_off as u32);
            frame.serialize(writer);
        }
    }
}

pub const PARTS_PANE_NAME_LEN: usize = 0x18;

#[derive(Debug, Serialize, Deserialize)]
pub struct PartsPaneBasicInfo {
    pub user_name: String,
    pub translation_x: f32,
    pub translation_y: f32,
    pub translation_z: f32,
    pub rotation_x: f32,
    pub rotation_y: f32,
    pub rotation_z: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub size_x: f32,
    pub size_y: f32,
    pub pane_alpha: u8,
    pub reserve0: u8,
    pub reserve1: u8,
}

impl PartsPaneBasicInfo {
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            user_name: cursor.read_fixed_string(USER_NAME_LEN),
            translation_x: cursor.read_f32(),
            translation_y: cursor.read_f32(),
            translation_z: cursor.read_f32(),
            rotation_x: cursor.read_f32(),
            rotation_y: cursor.read_f32(),
            rotation_z: cursor.read_f32(),
            scale_x: cursor.read_f32(),
            scale_y: cursor.read_f32(),
            size_x: cursor.read_f32(),
            size_y: cursor.read_f32(),
            pane_alpha: cursor.read_u8(),
            reserve0: cursor.read_u8(),
            reserve1: cursor.read_u8(),
        }
    }
    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_fixed_string(&self.user_name, USER_NAME_LEN);
        writer.write_f32(self.translation_x);
        writer.write_f32(self.translation_y);
        writer.write_f32(self.translation_z);
        writer.write_f32(self.rotation_x);
        writer.write_f32(self.rotation_y);
        writer.write_f32(self.rotation_z);
        writer.write_f32(self.scale_x);
        writer.write_f32(self.scale_y);
        writer.write_f32(self.size_x);
        writer.write_f32(self.size_y);
        writer.write_u8(self.pane_alpha);
        writer.write_u8(self.reserve0);
        writer.write_u8(self.reserve1);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PartsProperty {
    pub property_name: String,
    pub usage_flag: u8,
    pub basic_usage_flag: u8,
    pub material_usage_flag: u8,
    pub user_data_type: u8,
    pub pane_offset: u32,
    pub user_data_offset: u32,
    pub pane_basic_info_offset: u32,
    pub basic_info: Option<PartsPaneBasicInfo>,
    pub pane_data: Vec<u8>,
    pub user_data_data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BflytPartsPane {
    pub base: BflytPane,
    pub magnify_x: f32,
    pub magnify_y: f32,
    pub properties: Vec<PartsProperty>,
    pub layout_name: String,
}

impl BflytPartsPane {
    pub fn parse(cursor: &mut Cursor, section_start: usize, section_size: u32) -> Self {
        let prt1_base = section_start + 8;
        let base = BflytPane::parse(cursor);

        let property_count = cursor.read_u32();
        let magnify_x = cursor.read_f32();
        let magnify_y = cursor.read_f32();

        let props_start = cursor.pos;

        let mut raw_properties: Vec<(String, u8, u8, u8, u8, u32, u32, u32)> = Vec::new();
        for _ in 0..property_count {
            let property_name = cursor.read_fixed_string(PANE_NAME_LEN);
            let usage_flag = cursor.read_u8();
            let basic_usage_flag = cursor.read_u8();
            let material_usage_flag = cursor.read_u8();
            let user_data_type = cursor.read_u8();
            let pane_offset = cursor.read_u32();
            let user_data_offset = cursor.read_u32();
            let pane_basic_info_offset = cursor.read_u32();
            raw_properties.push((
                property_name,
                usage_flag,
                basic_usage_flag,
                material_usage_flag,
                user_data_type,
                pane_offset,
                user_data_offset,
                pane_basic_info_offset,
            ));
        }

        let layout_name = cursor.read_null_terminated_string();
        cursor.seek(props_start + 0x28 * property_count as usize + layout_name.len() + 1);

        let saved = cursor.pos;

        let mut properties = Vec::new();
        for (
            property_name,
            usage_flag,
            basic_usage_flag,
            material_usage_flag,
            user_data_type,
            pane_offset,
            user_data_offset,
            pane_basic_info_offset,
        ) in raw_properties
        {
            let basic_info = if pane_basic_info_offset != 0 {
                cursor.seek(prt1_base + pane_basic_info_offset as usize);
                Some(PartsPaneBasicInfo::parse(cursor))
            } else {
                None
            };

            properties.push(PartsProperty {
                property_name,
                usage_flag,
                basic_usage_flag,
                material_usage_flag,
                user_data_type,
                pane_offset,
                user_data_offset,
                pane_basic_info_offset,
                basic_info,
                pane_data: Vec::new(),
                user_data_data: Vec::new(),
            });
        }

        cursor.seek(saved);

        Self {
            base,
            magnify_x,
            magnify_y,
            properties,
            layout_name,
        }
    }

    pub fn serialize(&self, writer: &mut Writer, section_start: usize) {
        let prt1_base = section_start + 8;
        self.base.serialize(writer);

        writer.write_u32(self.properties.len() as u32);
        writer.write_f32(self.magnify_x);
        writer.write_f32(self.magnify_y);

        let mut basic_info_offset_placeholders = Vec::new();
        for prop in &self.properties {
            writer.write_fixed_string(&prop.property_name, PANE_NAME_LEN);
            writer.write_u8(prop.usage_flag);
            writer.write_u8(prop.basic_usage_flag);
            writer.write_u8(prop.material_usage_flag);
            writer.write_u8(prop.user_data_type);
            writer.write_u32(prop.pane_offset);
            writer.write_u32(prop.user_data_offset);
            basic_info_offset_placeholders
                .push((prop.basic_info.is_some(), writer.write_placeholder_u32()));
        }

        writer.write_null_terminated_string(&self.layout_name);
        writer.align(4);

        for (i, prop) in self.properties.iter().enumerate() {
            let (has_info, placeholder) = basic_info_offset_placeholders[i];
            if has_info {
                if let Some(info) = &prop.basic_info {
                    let offset = writer.pos() - prt1_base;
                    writer.patch_u32(placeholder, offset as u32);
                    info.serialize(writer);
                }
            } else {
                writer.patch_u32(placeholder, 0);
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BflytAlignmentPane {
    pub base: BflytPane,
    pub direction: u32,
    pub default_margin: f32,
    pub is_align_last_pane: u8,
    pub is_vertical_alignment: u8,
    pub reserve0: u16,
}

impl BflytAlignmentPane {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let base = BflytPane::parse(cursor);
        Self {
            base,
            direction: cursor.read_u32(),
            default_margin: cursor.read_f32(),
            is_align_last_pane: cursor.read_u8(),
            is_vertical_alignment: cursor.read_u8(),
            reserve0: cursor.read_u16(),
        }
    }
    pub fn serialize(&self, writer: &mut Writer) {
        self.base.serialize(writer);
        writer.write_u32(self.direction);
        writer.write_f32(self.default_margin);
        writer.write_u8(self.is_align_last_pane);
        writer.write_u8(self.is_vertical_alignment);
        writer.write_u16(self.reserve0);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BflytCapturePane {
    pub base: BflytPane,
    pub reserve0: [u32; 4],
    pub clear_color: [f32; 4],
    pub image_format: u16,
    pub is_copy_framebuffer: u8,
    pub is_create_resource: u8,
    pub reserve1: u16,
    pub reserve2: [u8; 3],
    pub reserve3: [u8; 3],
    pub scale: f32,
}

impl BflytCapturePane {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let base = BflytPane::parse(cursor);
        let mut reserve0 = [0u32; 4];
        for v in &mut reserve0 {
            *v = cursor.read_u32();
        }
        let clear_color = [
            cursor.read_f32(),
            cursor.read_f32(),
            cursor.read_f32(),
            cursor.read_f32(),
        ];
        let image_format = cursor.read_u16();
        let is_copy_framebuffer = cursor.read_u8();
        let is_create_resource = cursor.read_u8();
        let reserve1 = cursor.read_u16();
        let reserve2 = [cursor.read_u8(), cursor.read_u8(), cursor.read_u8()];
        let reserve3 = [cursor.read_u8(), cursor.read_u8(), cursor.read_u8()];
        let scale = cursor.read_f32();
        Self {
            base,
            reserve0,
            clear_color,
            image_format,
            is_copy_framebuffer,
            is_create_resource,
            reserve1,
            reserve2,
            reserve3,
            scale,
        }
    }
    pub fn serialize(&self, writer: &mut Writer) {
        self.base.serialize(writer);
        for v in &self.reserve0 {
            writer.write_u32(*v);
        }
        for v in &self.clear_color {
            writer.write_f32(*v);
        }
        writer.write_u16(self.image_format);
        writer.write_u8(self.is_copy_framebuffer);
        writer.write_u8(self.is_create_resource);
        writer.write_u16(self.reserve1);
        for v in &self.reserve2 {
            writer.write_u8(*v);
        }
        for v in &self.reserve3 {
            writer.write_u8(*v);
        }
        writer.write_f32(self.scale);
    }
}

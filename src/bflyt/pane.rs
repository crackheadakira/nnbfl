use serde::{Deserialize, Serialize};

use crate::{
    bflan::anim_info::AnimInfo,
    bflyt::file::BflytSection,
    core::{Cursor, Writer},
    ui2d::types::Vector2f,
};

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
        writer.mark("Pane (generic)");

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
        writer.mark("TextureUv");

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
    pub is_shape: bool,
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
        let is_shape = cursor.read_u8() != 0;
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
            is_shape,
            texture_uvs,
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        self.base.serialize(writer);
        writer.mark("PicturePane");

        self.top_left_vertex_color.serialize(writer);
        self.top_right_vertex_color.serialize(writer);
        self.bottom_left_vertex_color.serialize(writer);
        self.bottom_right_vertex_color.serialize(writer);
        writer.write_u16(self.material_index);
        writer.write_u8(self.texture_uvs.len() as u8);
        writer.write_u8(self.is_shape.into());
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
    pub char_list: [u8; 16],

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub anim_info: Option<AnimInfo>,
}

impl PerCharacterTransform {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let mut transform = Self {
            eval_time_offset: cursor.read_f32(),
            eval_time_width: cursor.read_f32(),
            loop_type: cursor.read_u8(),
            origin_v: cursor.read_u8(),
            has_anim_info: cursor.read_u8(),
            reserve0: cursor.read_u8(),
            reserve1: cursor.read_u32(),
            char_list: [
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
                cursor.read_u8(),
            ],
            anim_info: None,
        };

        if transform.has_anim_info != 0 {
            transform.anim_info = Some(AnimInfo::parse(cursor, cursor.pos));
        }

        transform
    }
    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("PerCharacterTransform");

        writer.write_f32(self.eval_time_offset);
        writer.write_f32(self.eval_time_width);
        writer.write_u8(self.loop_type);
        writer.write_u8(self.origin_v);
        writer.write_u8(self.has_anim_info);
        writer.write_u8(self.reserve0);
        writer.write_u32(self.reserve1);

        for char in &self.char_list {
            writer.write_u8(*char);
        }

        if let Some(anim_info) = &self.anim_info {
            anim_info.serialize(writer, writer.pos());
        }
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
    pub font_top_color: Color4u8,
    pub font_bottom_color: Color4u8,
    pub font_size_x: f32,
    pub font_size_y: f32,
    pub character_space: f32,
    pub line_space: f32,
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
    pub per_character_transform: Option<PerCharacterTransform>,
}

impl BflytTextBoxPane {
    pub fn parse(cursor: &mut Cursor, section_start: usize) -> Self {
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

        let per_character_transform = if is_per_character && per_character_transform_offset != 0 {
            let addr = txt1_base + per_character_transform_offset as usize - 8;
            cursor.seek(addr);

            Some(PerCharacterTransform::parse(cursor))
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
            font_top_color,
            font_bottom_color,
            font_size_x,
            font_size_y,
            character_space,
            line_space,
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
            per_character_transform,
        }
    }

    pub fn serialize(&self, writer: &mut Writer, section_start: usize) {
        let txt1_base = section_start + 8;
        self.base.serialize(writer);
        writer.mark("TextBoxPane");

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

        if let Some(transform) = &self.per_character_transform {
            writer.align(4);
            let offset = writer.pos() - txt1_base + 8;

            writer.patch_u32(per_char_offset_pos, offset as u32);
            transform.serialize(writer);
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
    pub picture_uvs: Vec<Vector2f>,
    pub unk_1: Vec<Vector2f>,
    pub unk_2: Vec<Vector2f>,
    pub unk_3: Vec<Vector2f>,
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
        let mut picture_uvs = Vec::new();
        let mut unk_1 = Vec::new();
        let mut unk_2 = Vec::new();
        let mut unk_3 = Vec::new();

        for _ in 0..uv_coordinate_count {
            picture_uvs.push(Vector2f::parse(cursor));
        }

        for _ in 0..uv_coordinate_count {
            unk_1.push(Vector2f::parse(cursor));
        }

        for _ in 0..uv_coordinate_count {
            unk_2.push(Vector2f::parse(cursor));
        }

        for _ in 0..uv_coordinate_count {
            unk_3.push(Vector2f::parse(cursor));
        }

        Self {
            top_left_vertex_color,
            top_right_vertex_color,
            bottom_left_vertex_color,
            bottom_right_vertex_color,
            material_index,
            uv_coordinate_count,
            reserve0,
            picture_uvs,
            unk_1,
            unk_2,
            unk_3,
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("WindowContent");

        self.top_left_vertex_color.serialize(writer);
        self.top_right_vertex_color.serialize(writer);
        self.bottom_left_vertex_color.serialize(writer);
        self.bottom_right_vertex_color.serialize(writer);
        writer.write_u16(self.material_index);
        writer.write_u8(self.picture_uvs.len() as u8);
        writer.write_u8(self.reserve0);

        for uv in &self.picture_uvs {
            uv.serialize(writer);
        }

        for unk in &self.unk_1 {
            unk.serialize(writer);
        }

        for unk in &self.unk_2 {
            unk.serialize(writer);
        }

        for unk in &self.unk_3 {
            unk.serialize(writer);
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
        writer.mark("WindowFrame");
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
    pub content: WindowContent,
    pub frames: Vec<WindowFrame>,
}

impl BflytWindowPane {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let wnd_base = cursor.pos - 8;
        let base = BflytPane::parse(cursor);

        let inflation_left = cursor.read_i16();
        let inflation_right = cursor.read_i16();
        let inflation_top = cursor.read_i16();
        let inflation_bottom = cursor.read_i16();
        let frame_size_left = cursor.read_i16();
        let frame_size_right = cursor.read_i16();
        let frame_size_top = cursor.read_i16();
        let frame_size_bottom = cursor.read_i16();

        let frame_count = cursor.read_u8();
        let flag = cursor.read_u8();
        let reserve0 = cursor.read_u16();
        let content_offset = cursor.read_u32();
        let frame_offset_array_offset = cursor.read_u32();

        let restore_point = cursor.pos;
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

        cursor.seek(restore_point);

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
            content,
            frames,
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        let wnd_base = writer.pos() - 8;
        self.base.serialize(writer);
        writer.mark("WindowPane");

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
        writer.mark("PaneBasicInfo");
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

        writer.align(4);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PartsProperty {
    pub property_name: String,
    pub usage_flag: u8,
    pub basic_usage_flag: u8,
    pub material_usage_flag: u8,
    pub user_data_type: u8,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub o_section: Option<BflytSection>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub o_user_data: Option<BflytSection>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub o_basic_info: Option<PartsPaneBasicInfo>,
}

impl PartsProperty {
    pub fn parse(cursor: &mut Cursor, last_parts_pane: usize) -> Self {
        let mut property = Self {
            property_name: cursor.read_fixed_string(PANE_NAME_LEN),
            usage_flag: cursor.read_u8(),
            basic_usage_flag: cursor.read_u8(),
            material_usage_flag: cursor.read_u8(),
            user_data_type: cursor.read_u8(),
            o_section: None,
            o_user_data: None,
            o_basic_info: None,
        };

        let pane_offset = cursor.read_u32();
        let user_data_offset = cursor.read_u32();
        let pane_basic_info_offset = cursor.read_u32();

        let restore_point = cursor.pos;

        if pane_offset > 0 {
            cursor.seek(last_parts_pane + pane_offset as usize);
            let pane = BflytSection::parse(cursor, &mut false);

            property.o_section = Some(pane);
            cursor.seek(restore_point);
        }

        if user_data_offset > 0 {
            cursor.seek(last_parts_pane + user_data_offset as usize);
            let user_data = BflytSection::parse(cursor, &mut false);

            property.o_user_data = Some(user_data);
            cursor.seek(restore_point);
        }

        if pane_basic_info_offset > 0 {
            cursor.seek(last_parts_pane + pane_basic_info_offset as usize);
            let basic_info = PartsPaneBasicInfo::parse(cursor);

            property.o_basic_info = Some(basic_info);
            cursor.seek(restore_point);
        }

        property
    }

    pub fn serialize_header(&self, writer: &mut Writer) -> (usize, usize, usize) {
        writer.write_fixed_string(&self.property_name, PANE_NAME_LEN);
        writer.write_u8(self.usage_flag);
        writer.write_u8(self.basic_usage_flag);
        writer.write_u8(self.material_usage_flag);
        writer.write_u8(self.user_data_type);

        let pane_pos = writer.write_placeholder_u32();
        let user_data_pos = writer.write_placeholder_u32();
        let basic_info_pos = writer.write_placeholder_u32();

        (pane_pos, user_data_pos, basic_info_pos)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BflytPartsPane {
    pub base: BflytPane,
    pub magnify_x: f32,
    pub magnify_y: f32,
    pub properties: Vec<PartsProperty>,
    pub o_layout_name: String,
}

impl BflytPartsPane {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let base_offset = cursor.pos;
        let base = BflytPane::parse(cursor);

        let property_count = cursor.read_u32();
        let magnify_x = cursor.read_f32();
        let magnify_y = cursor.read_f32();

        let props_start = cursor.pos;

        let mut properties = Vec::new();

        for _ in 0..property_count {
            let property = PartsProperty::parse(cursor, base_offset - 8);
            properties.push(property);
        }

        let o_layout_name = cursor.read_null_terminated_string();
        cursor.seek(props_start + 0x28 * property_count as usize + o_layout_name.len() + 1);

        Self {
            base,
            magnify_x,
            magnify_y,
            properties,
            o_layout_name,
        }
    }

    pub fn serialize(&self, writer: &mut Writer, section_start: usize) {
        self.base.serialize(writer);
        writer.mark("PartsPane");

        writer.write_u32(self.properties.len() as u32);
        writer.write_f32(self.magnify_x);
        writer.write_f32(self.magnify_y);

        let mut patch_offsets = Vec::new();
        for prop in &self.properties {
            let (p, u, b) = prop.serialize_header(writer);
            patch_offsets.push((p, u, b, prop));
        }

        writer.write_null_terminated_string(&self.o_layout_name);
        writer.align(4);

        for (p_pos, u_pos, b_pos, prop) in patch_offsets {
            if let Some(section) = &prop.o_section {
                let start = writer.pos();

                section.serialize(writer);
                writer.patch_u32(p_pos, (start - section_start) as u32);
            }

            if let Some(data) = &prop.o_user_data {
                let start = writer.pos();
                data.serialize(writer);
                writer.patch_u32(u_pos, (start - section_start) as u32);
            }

            if let Some(info) = &prop.o_basic_info {
                let start = writer.pos();
                info.serialize(writer);
                writer.patch_u32(b_pos, (start - section_start) as u32);
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BflytAlignmentPane {
    pub base: BflytPane,
    pub direction: u32,
    pub default_margin: f32,
    pub is_align_last_pane: bool,
    pub is_vertical_alignment: bool,
    pub reserve0: u16,
}

impl BflytAlignmentPane {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let base = BflytPane::parse(cursor);
        Self {
            base,
            direction: cursor.read_u32(),
            default_margin: cursor.read_f32(),
            is_align_last_pane: cursor.read_u8() != 0,
            is_vertical_alignment: cursor.read_u8() != 0,
            reserve0: cursor.read_u16(),
        }
    }
    pub fn serialize(&self, writer: &mut Writer) {
        self.base.serialize(writer);
        writer.mark("AlignmentPane");

        writer.write_u32(self.direction);
        writer.write_f32(self.default_margin);
        writer.write_u8(self.is_align_last_pane.into());
        writer.write_u8(self.is_vertical_alignment.into());
        writer.write_u16(self.reserve0);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BflytCapturePane {
    pub base: BflytPane,
    pub reserve0: [u32; 4],
    pub clear_color: [f32; 4],
    pub image_format: u16,
    pub is_copy_framebuffer: bool,
    pub is_create_resource: bool,
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
        let is_copy_framebuffer = cursor.read_u8() != 0;
        let is_create_resource = cursor.read_u8() != 0;
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
        writer.mark("CapturePane");

        for v in &self.reserve0 {
            writer.write_u32(*v);
        }
        for v in &self.clear_color {
            writer.write_f32(*v);
        }
        writer.write_u16(self.image_format);
        writer.write_u8(self.is_copy_framebuffer.into());
        writer.write_u8(self.is_create_resource.into());
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

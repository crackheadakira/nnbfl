use serde::{Deserialize, Serialize};

use crate::{
    bflan::targets::AnimTarget,
    core::{Cursor, Writer, tchar_code32},
};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u32)]
pub enum AnimInfoType {
    Invalid = 0,
    PerCharacterTransformCurveAnim = tchar_code32(b"FLCC"),
    ExtendedUserDataAnim = tchar_code32(b"FLEU"),

    PerCharacterTransformAnim = tchar_code32(b"FLCT"),
    PaneSrtAnim = tchar_code32(b"FLPA"),
    VertexColorAnim = tchar_code32(b"FLVC"),
    VisibilityAnim = tchar_code32(b"FLVI"),
    DropShadowAnim = tchar_code32(b"FLDS"),
    MaskTextureAnim = tchar_code32(b"FLMT"),
    ProceduralShapeAnim = tchar_code32(b"FLPS"),
    WindowAnim = tchar_code32(b"FLWN"),
    StateMachineAnim = tchar_code32(b"FSMA"),

    AlphaCompareAnim = tchar_code32(b"FLAC"),
    FontShadowAnim = tchar_code32(b"FLFS"),
    IndirectSrtAnim = tchar_code32(b"FLIM"),
    MaterialColorAnim = tchar_code32(b"FLMC"),
    TextureSrtAnim = tchar_code32(b"FLTS"),
    TexturePatternAnim = tchar_code32(b"FLTP"),
    BrickRepeatAnim = tchar_code32(b"FTBR"),
    VectorGraphicsAnim = tchar_code32(b"FVGA"),
}

impl From<u32> for AnimInfoType {
    fn from(v: u32) -> Self {
        match v {
            x if x == tchar_code32(b"FLCC") => Self::PerCharacterTransformCurveAnim,
            x if x == tchar_code32(b"FLEU") => Self::ExtendedUserDataAnim,
            x if x == tchar_code32(b"FLCT") => Self::PerCharacterTransformAnim,
            x if x == tchar_code32(b"FLPA") => Self::PaneSrtAnim,
            x if x == tchar_code32(b"FLVC") => Self::VertexColorAnim,
            x if x == tchar_code32(b"FLVI") => Self::VisibilityAnim,
            x if x == tchar_code32(b"FLDS") => Self::DropShadowAnim,
            x if x == tchar_code32(b"FLMT") => Self::MaskTextureAnim,
            x if x == tchar_code32(b"FLPS") => Self::ProceduralShapeAnim,
            x if x == tchar_code32(b"FLWN") => Self::WindowAnim,
            x if x == tchar_code32(b"FSMA") => Self::StateMachineAnim,
            x if x == tchar_code32(b"FLAC") => Self::AlphaCompareAnim,
            x if x == tchar_code32(b"FLFS") => Self::FontShadowAnim,
            x if x == tchar_code32(b"FLIM") => Self::IndirectSrtAnim,
            x if x == tchar_code32(b"FLMC") => Self::MaterialColorAnim,
            x if x == tchar_code32(b"FLTS") => Self::TextureSrtAnim,
            x if x == tchar_code32(b"FLTP") => Self::TexturePatternAnim,
            x if x == tchar_code32(b"FTBR") => Self::BrickRepeatAnim,
            x if x == tchar_code32(b"FVGA") => Self::VectorGraphicsAnim,
            _ => Self::Invalid,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AnimType {
    Pane = 0,
    Material = 1,
    User = 2,
    PaneExt = 3,
    StateMachine = 4,
}

impl From<u8> for AnimType {
    fn from(value: u8) -> Self {
        match value {
            0 => AnimType::Pane,
            1 => AnimType::Material,
            2 => AnimType::User,
            3 => AnimType::PaneExt,
            4 => AnimType::StateMachine,
            // fallback
            _ => AnimType::Pane,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AnimInfo {
    Standard {
        magic: AnimInfoType,
        targets: Vec<AnimTarget>,
    },
    ExtendedUserData {
        magic: AnimInfoType,
        data: Vec<ExtendedUserDataAnim>,
    },
}

impl AnimInfo {
    pub fn parse(cursor: &mut Cursor, base_offset: usize) -> Self {
        cursor.seek(base_offset);

        let magic_val = cursor.read_u32();
        let magic: AnimInfoType = magic_val.into();

        let anim_target_count = cursor.read_u8();
        let _reserve0 = cursor.read_u8();
        let _reserve1 = cursor.read_u16();

        match magic {
            AnimInfoType::ExtendedUserDataAnim => {
                let mut data_array = Vec::new();
                for _ in 0..anim_target_count {
                    let data = ExtendedUserDataAnim::parse(cursor);
                    data_array.push(data);
                }

                Self::ExtendedUserData {
                    magic,
                    data: data_array,
                }
            }
            _ => {
                let mut target_offsets = Vec::with_capacity(anim_target_count as usize);
                for _ in 0..anim_target_count {
                    target_offsets.push(cursor.read_u32());
                }

                let mut targets = Vec::with_capacity(anim_target_count as usize);
                for offset in target_offsets {
                    targets.push(AnimTarget::parse(
                        cursor,
                        base_offset + offset as usize,
                        magic_val,
                    ));
                }

                Self::Standard { magic, targets }
            }
        }
    }

    pub fn serialize(&self, writer: &mut Writer, base_offset: usize) {
        writer.mark("AnimInfo");

        match self {
            AnimInfo::Standard { magic, targets } => {
                let magic_val = unsafe { std::mem::transmute_copy::<AnimInfoType, u32>(magic) };
                writer.write_u32(magic_val);

                writer.write_u8(targets.len() as u8);
                writer.write_u8(0);
                writer.write_u16(0);

                let mut target_offset_placeholders = Vec::new();
                for _ in targets {
                    target_offset_placeholders.push(writer.write_placeholder_u32());
                }

                for (i, target) in targets.iter().enumerate() {
                    let target_base = writer.pos();

                    let relative_offset = target_base - base_offset;
                    writer.patch_u32(target_offset_placeholders[i], relative_offset as u32);

                    target.serialize(writer, target_base);
                }
            }
            AnimInfo::ExtendedUserData { magic, data } => {
                let magic_val = unsafe { std::mem::transmute_copy::<AnimInfoType, u32>(magic) };
                writer.write_u32(magic_val);

                writer.write_u8(data.len() as u8);
                writer.write_u8(0);
                writer.write_u16(0);

                for data in data.iter() {
                    data.serialize(writer);
                }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtendedUserDataAnim {
    pub block_size: u32,

    pub unk_1: u16,
    pub unk_2: u16,

    pub block_size_2: u32,

    pub unk_3: u16,
    pub unk_4: u16,
    pub unk_5: u16,
    pub frame_count: u16,

    pub values: Vec<Vec<f32>>,

    pub key: String,
}

impl ExtendedUserDataAnim {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let block_size = cursor.read_u32();

        let unk_1 = cursor.read_u16();
        let entry_count = cursor.read_u16();
        let entries_inside_entry = cursor.read_u16();
        let unk_2 = cursor.read_u16();

        let block_size_2 = cursor.read_u32();

        let unk_3 = cursor.read_u16();
        let unk_4 = cursor.read_u16();
        let unk_5 = cursor.read_u16();
        let frame_count = cursor.read_u16();

        let mut values = Vec::new();
        for _ in 0..entry_count {
            let mut inner_values = Vec::new();

            for _ in 0..entries_inside_entry {
                inner_values.push(cursor.read_f32())
            }

            values.push(inner_values);
        }

        let restore = cursor.pos;
        let string_start = cursor.pos + cursor.read_u32() as usize;

        cursor.seek(string_start);

        let key = cursor.read_null_terminated_string();
        cursor.seek(restore);

        Self {
            block_size,
            unk_1,
            unk_2,
            block_size_2,
            unk_3,
            unk_4,
            unk_5,
            frame_count,
            values,

            key,
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("ExtendedUserDataAnim");

        writer.write_u32(self.block_size);
        writer.write_u16(self.unk_1);
        writer.write_u16(self.values.len() as u16);
        writer.write_u16(self.values[0].len() as u16);
        writer.write_u16(self.unk_2);
        writer.write_u32(self.block_size_2);
        writer.write_u16(self.unk_3);
        writer.write_u16(self.unk_4);
        writer.write_u16(self.unk_5);
        writer.write_u16(self.frame_count);

        for vec in &self.values {
            for val in vec {
                writer.write_f32(*val);
            }
        }

        let offset_pos = writer.write_placeholder_u32();

        let string_start = writer.pos();
        writer.write_null_terminated_string(&self.key);

        let relative_offset = (string_start - offset_pos) as u32;
        writer.patch_u32(offset_pos, relative_offset);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaneAnimInfo {
    pub frame_count: u16,
    pub is_looping: bool,
    pub textures: Vec<String>,
    pub contents: Vec<AnimContent>,
}

impl PaneAnimInfo {
    pub fn parse(cursor: &mut Cursor, section_start: usize) -> Self {
        let frame_count = cursor.read_u16();
        let is_looping = cursor.read_u8() != 0;
        let _reserve0 = cursor.read_u8();
        let texture_count = cursor.read_u16();
        let anim_content_count = cursor.read_u16();
        let anim_content_offset_array_offset = cursor.read_u32();

        let texture_offsets_start = cursor.pos;
        let mut texture_offsets = Vec::with_capacity(texture_count as usize);
        for _ in 0..texture_count {
            texture_offsets.push(cursor.read_u32());
        }

        let mut textures = Vec::with_capacity(texture_count as usize);
        for offset in texture_offsets {
            cursor.seek(texture_offsets_start + offset as usize);
            textures.push(cursor.read_null_terminated_string());
        }

        cursor.seek(section_start + anim_content_offset_array_offset as usize);
        let mut content_offsets = Vec::with_capacity(anim_content_count as usize);
        for _ in 0..anim_content_count {
            content_offsets.push(cursor.read_u32());
        }

        let mut contents = Vec::with_capacity(anim_content_count as usize);
        for offset in content_offsets {
            contents.push(AnimContent::parse(cursor, section_start + offset as usize));
        }

        Self {
            frame_count,
            is_looping,
            textures,
            contents,
        }
    }

    pub fn serialize(&self, writer: &mut Writer, section_start: usize) {
        writer.mark("PaneAnimInfo");
        writer.write_u16(self.frame_count);
        writer.write_u8(if self.is_looping { 1 } else { 0 });
        writer.write_u8(0);
        writer.write_u16(self.textures.len() as u16);
        writer.write_u16(self.contents.len() as u16);

        let anim_content_offset_array_offset_pos = writer.write_placeholder_u32();

        let texture_offsets_start = writer.pos();
        let mut texture_offset_placeholders = Vec::new();

        for _ in &self.textures {
            texture_offset_placeholders.push(writer.write_placeholder_u32());
        }

        for (i, texture_name) in self.textures.iter().enumerate() {
            let current_offset = writer.pos() - texture_offsets_start;
            writer.patch_u32(texture_offset_placeholders[i], current_offset as u32);
            writer.write_null_terminated_string(texture_name);
        }

        writer.align(4);

        let content_array_offset = writer.pos() - section_start;
        writer.patch_u32(
            anim_content_offset_array_offset_pos,
            content_array_offset as u32,
        );

        let mut content_offset_placeholders = Vec::new();

        for _ in &self.contents {
            content_offset_placeholders.push(writer.write_placeholder_u32());
        }

        for (i, content) in self.contents.iter().enumerate() {
            let content_base = writer.pos();

            let relative_offset = content_base - section_start;
            writer.patch_u32(content_offset_placeholders[i], relative_offset as u32);

            content.serialize(writer, content_base);
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnimContent {
    pub name: String,
    pub anim_type: AnimType,
    pub infos: Vec<AnimInfo>,
}

impl AnimContent {
    pub fn parse(cursor: &mut Cursor, base_offset: usize) -> Self {
        cursor.seek(base_offset);

        let name = cursor.read_fixed_string(0x1C);
        let anim_info_count = cursor.read_u8();
        let anim_type: AnimType = cursor.read_u8().into();
        let _reserve0 = cursor.read_u16();

        // Workaround for MiniGame_PictQuiz_00_MosaicNormal.bflan
        if matches!(anim_type, AnimType::User) {
            let info_array_offset = cursor.read_u32();

            cursor.seek(base_offset + info_array_offset as usize);
        }

        let mut info_offsets = Vec::with_capacity(anim_info_count as usize);
        for _ in 0..anim_info_count {
            info_offsets.push(cursor.read_u32());
        }

        let mut infos = Vec::with_capacity(anim_info_count as usize);
        for offset in info_offsets {
            infos.push(AnimInfo::parse(cursor, base_offset + offset as usize));
        }

        Self {
            name,
            anim_type,
            infos,
        }
    }

    pub fn serialize(&self, writer: &mut Writer, base_offset: usize) {
        writer.mark("AnimContent");
        writer.write_fixed_string(&self.name, 0x1C);
        writer.write_u8(self.infos.len() as u8);

        let type_val = match self.anim_type {
            AnimType::Pane => 0,
            AnimType::Material => 1,
            AnimType::User => 2,
            AnimType::PaneExt => 3,
            AnimType::StateMachine => 4,
        };

        writer.write_u8(type_val);
        writer.write_u16(0);

        let mut user_str_offset_pos = 0;

        // Workaround for MiniGame_PictQuiz_00_MosaicNormal.bflan
        if matches!(self.anim_type, AnimType::User) {
            let info_array_offset_pos = writer.write_placeholder_u32();
            user_str_offset_pos = writer.write_placeholder_u32();

            writer.patch_u32(info_array_offset_pos, (writer.pos() - base_offset) as u32);
        }

        let mut info_offset_placeholders = Vec::new();
        for _ in &self.infos {
            info_offset_placeholders.push(writer.write_placeholder_u32());
        }

        for (i, info) in self.infos.iter().enumerate() {
            let info_base = writer.pos();

            writer.patch_u32(
                info_offset_placeholders[i],
                (info_base - base_offset) as u32,
            );

            info.serialize(writer, info_base);
        }

        if matches!(self.anim_type, AnimType::User) {
            // man... dont question me.
            writer.patch_u32(user_str_offset_pos, 0x5C);
        }
    }
}

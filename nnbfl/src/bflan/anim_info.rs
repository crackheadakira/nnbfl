use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};

use crate::{
    bflan::targets::AnimTarget,
    core::{Cursor, FormatError, ReadWriteable, Writer, tchar_code32},
};

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, IntoPrimitive, FromPrimitive,
)]
#[repr(u32)]
pub enum AnimInfoType {
    #[default]
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

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, IntoPrimitive, FromPrimitive, Default,
)]
#[repr(u8)]
pub enum AnimType {
    #[default]
    Pane,
    Material,
    User,
    PaneExt,
    StateMachine,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnimInfo {
    Standard {
        anim_type: AnimInfoType,
        targets: Vec<AnimTarget>,
    },
}

impl Default for AnimInfo {
    fn default() -> Self {
        Self::Standard {
            anim_type: AnimInfoType::default(),
            targets: Vec::new(),
        }
    }
}

impl ReadWriteable for AnimInfo {
    fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let base_offset = cursor.ctx_section_start::<Self>()?;

        cursor.seek(base_offset)?;

        let magic_val = cursor.read_u32()?;
        let magic: AnimInfoType = magic_val.into();

        let anim_target_count = cursor.read_u8()?;
        let _reserve0 = cursor.read_u8()?;
        let _reserve1 = cursor.read_u16()?;

        let mut target_offsets = Vec::with_capacity(anim_target_count as usize);
        for _ in 0..anim_target_count {
            target_offsets.push(cursor.read_u32()?);
        }

        let mut targets = Vec::with_capacity(anim_target_count as usize);
        for offset in target_offsets {
            cursor.section_start = Some(base_offset + offset as usize);
            targets.push(AnimTarget::parse(cursor, &magic)?);
            cursor.section_start = None;
        }

        Ok(Self::Standard {
            anim_type: magic,
            targets,
        })
    }

    fn write(&self, writer: &mut Writer) {
        let base_offset = writer.ctx_section_start::<Self>();

        writer.mark("AnimInfo");

        match self {
            Self::Standard { anim_type, targets } => {
                writer.write_u32((*anim_type).into());

                writer.write_u8(targets.len() as u8);
                writer.write_u8(0);
                writer.write_u16(0);

                let mut target_offset_placeholders = Vec::with_capacity(targets.len());
                for _ in targets {
                    target_offset_placeholders.push(writer.write_placeholder_u32());
                }

                for (i, target) in targets.iter().enumerate() {
                    let target_base = writer.pos();

                    let relative_offset = target_base - base_offset;
                    writer.patch_u32(target_offset_placeholders[i], relative_offset as u32);

                    writer.section_start = Some(target_base);
                    target.write(writer);
                    writer.section_start = None;
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PaneAnimInfo {
    pub frame_count: u16,
    pub is_looping: bool,
    pub textures: Vec<String>,
    pub contents: Vec<AnimContent>,
}

impl ReadWriteable for PaneAnimInfo {
    fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let section_start = cursor.ctx_section_start::<Self>()?;

        let frame_count = cursor.read_u16()?;
        let is_looping = cursor.read_u8()? != 0;
        let _reserve0 = cursor.read_u8()?;
        let texture_count = cursor.read_u16()?;
        let anim_content_count = cursor.read_u16()?;
        let anim_content_offset_array_offset = cursor.read_u32()?;

        let texture_offsets_start = cursor.pos;
        let mut texture_offsets = Vec::with_capacity(texture_count as usize);
        for _ in 0..texture_count {
            texture_offsets.push(cursor.read_u32()?);
        }

        let mut textures = Vec::with_capacity(texture_count as usize);
        for offset in texture_offsets {
            cursor.seek(texture_offsets_start + offset as usize)?;
            textures.push(cursor.read_null_terminated_string()?);
        }

        cursor.seek(section_start + anim_content_offset_array_offset as usize)?;
        let mut content_offsets = Vec::with_capacity(anim_content_count as usize);
        for _ in 0..anim_content_count {
            content_offsets.push(cursor.read_u32()?);
        }

        let mut contents = Vec::with_capacity(anim_content_count as usize);
        for offset in content_offsets {
            cursor.section_start = Some(section_start + offset as usize);
            contents.push(AnimContent::parse(cursor)?);
            cursor.section_start = None;
        }

        Ok(Self {
            frame_count,
            is_looping,
            textures,
            contents,
        })
    }

    fn write(&self, writer: &mut Writer) {
        let section_start = writer.ctx_section_start::<Self>();

        writer.mark("PaneAnimInfo");
        writer.write_u16(self.frame_count);
        writer.write_u8(self.is_looping as u8);
        writer.write_u8(0);
        writer.write_u16(self.textures.len() as u16);
        writer.write_u16(self.contents.len() as u16);

        let anim_content_offset_array_offset_pos = writer.write_placeholder_u32();

        let texture_offsets_start = writer.pos();
        let mut texture_offset_placeholders = Vec::with_capacity(self.textures.len());

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

        let mut content_offset_placeholders = Vec::with_capacity(self.contents.len());

        for _ in &self.contents {
            content_offset_placeholders.push(writer.write_placeholder_u32());
        }

        for (i, content) in self.contents.iter().enumerate() {
            let content_base = writer.pos();

            let relative_offset = content_base - section_start;
            writer.patch_u32(content_offset_placeholders[i], relative_offset as u32);

            writer.section_start = Some(content_base);
            content.write(writer);
            writer.section_start = None;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AnimContent {
    pub name: String,
    pub anim_type: AnimType,
    pub infos: Vec<AnimInfo>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
}

impl AnimType {
    fn has_target_name(&self) -> bool {
        matches!(self, Self::User | Self::StateMachine)
    }
}

impl ReadWriteable for AnimContent {
    fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let base_offset = cursor.ctx_section_start::<Self>()?;
        cursor.seek(base_offset)?;

        let name = cursor.read_fixed_string(0x1C)?;
        let anim_info_count = cursor.read_u8()?;
        let anim_type: AnimType = cursor.read_u8()?.into();
        let _reserve0 = cursor.read_u16()?;

        let mut target_name = None;

        if anim_type.has_target_name() {
            let info_table_offset = cursor.read_u32()?;
            let name_table_offset = base_offset + cursor.read_u32()? as usize;

            cursor.seek(name_table_offset)?;
            let name_offset = cursor.read_u32()?;
            cursor.seek(name_table_offset + name_offset as usize)?;
            target_name = Some(cursor.read_null_terminated_string()?);

            cursor.seek(base_offset + info_table_offset as usize)?;
        }

        let mut info_offsets = Vec::with_capacity(anim_info_count as usize);
        for _ in 0..anim_info_count {
            info_offsets.push(cursor.read_u32()?);
        }

        let mut infos = Vec::with_capacity(anim_info_count as usize);
        for offset in info_offsets {
            cursor.section_start = Some(base_offset + offset as usize);
            infos.push(AnimInfo::parse(cursor)?);
            cursor.section_start = None;
        }

        Ok(Self {
            name,
            anim_type,
            infos,
            target_name,
        })
    }

    fn write(&self, writer: &mut Writer) {
        let base_offset = writer.ctx_section_start::<Self>();
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

        let name_table_pos = if self.anim_type.has_target_name() {
            let info_table_pos = writer.write_placeholder_u32();
            let name_table_pos = writer.write_placeholder_u32();

            writer.patch_u32(info_table_pos, (writer.pos() - base_offset) as u32);
            Some(name_table_pos)
        } else {
            None
        };

        let mut info_offset_placeholders = Vec::with_capacity(self.infos.len());
        for _ in &self.infos {
            info_offset_placeholders.push(writer.write_placeholder_u32());
        }

        for (i, info) in self.infos.iter().enumerate() {
            let info_base = writer.pos();

            writer.patch_u32(
                info_offset_placeholders[i],
                (info_base - base_offset) as u32,
            );

            writer.section_start = Some(info_base);
            info.write(writer);
            writer.section_start = None;
        }

        if let Some(name_table_pos) = name_table_pos {
            let name_table = writer.pos();
            writer.patch_u32(name_table_pos, (name_table - base_offset) as u32);

            let name_offset = writer.write_placeholder_u32();
            writer.patch_u32(name_offset, (writer.pos() - name_table) as u32);
            writer.write_null_terminated_string(self.target_name.as_deref().unwrap_or(""));
        }

        writer.align(4);
    }
}

use serde::{Deserialize, Serialize};

use crate::{
    bflyt::constants::MAGIC_USERDATA,
    core::{Cursor, Writer, tchar_code32},
    ui2d::userdata::ResUi2dUserDataSection,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ResBflanPaneAnimTag {
    pub tag_order: u16,
    pub group_count: u16,
    pub name_offset: u32,
    pub group_array_offset: u32,
    pub user_data_section_offset: u32,
    pub start_frame: u16,
    pub end_frame: u16,
    pub is_descending_bind: u8,
    pub reserve0: u8,
    pub reserve1: u16,

    pub o_name: String,
    pub groups: Vec<ResBflanGroup>,

    pub user_data: Option<ResUi2dUserDataSection>,
}

impl ResBflanPaneAnimTag {
    pub fn parse(cursor: &mut Cursor, section_start: usize) -> Self {
        let mut tag = Self {
            tag_order: cursor.read_u16(),
            group_count: cursor.read_u16(),
            name_offset: cursor.read_u32(),
            group_array_offset: cursor.read_u32(),
            user_data_section_offset: cursor.read_u32(),
            start_frame: cursor.read_u16(),
            end_frame: cursor.read_u16(),
            is_descending_bind: cursor.read_u8(),
            reserve0: cursor.read_u8(),
            reserve1: cursor.read_u16(),
            o_name: String::new(),
            groups: Vec::new(),
            user_data: None,
        };

        if tag.name_offset > 0 {
            cursor.seek(section_start + tag.name_offset as usize);
            tag.o_name = cursor.read_null_terminated_string();
        }

        if tag.group_count > 0 && tag.group_array_offset > 0 {
            cursor.seek(section_start + tag.group_array_offset as usize);
            for _ in 0..tag.group_count {
                tag.groups.push(ResBflanGroup {
                    group_name: cursor.read_fixed_string(0x21),
                    flag: cursor.read_u8(),
                    reserve0: cursor.read_u16(),
                });
            }
        }

        if tag.user_data_section_offset > 0 {
            cursor.seek(section_start + tag.user_data_section_offset as usize);

            let embed_magic = cursor.read_u32();
            let _embed_size = cursor.read_u32();

            if embed_magic == tchar_code32(b"usd1") {
                tag.user_data = Some(ResUi2dUserDataSection::parse(cursor));
            }
        }

        tag
    }

    pub fn serialize(&self, writer: &mut Writer, section_start: usize) {
        writer.mark("PaneAnimTag");
        writer.write_u16(self.tag_order);
        writer.write_u16(self.groups.len() as u16);

        let name_offset_pos = writer.write_placeholder_u32();
        let group_offset_pos = writer.write_placeholder_u32();
        let user_data_offset_pos = writer.write_placeholder_u32();

        writer.write_u16(self.start_frame);
        writer.write_u16(self.end_frame);
        writer.write_u8(self.is_descending_bind);
        writer.write_u8(self.reserve0);
        writer.write_u16(self.reserve1);

        writer.patch_u32(name_offset_pos, (writer.pos() - section_start) as u32);
        writer.write_null_terminated_string(&self.o_name);
        writer.align(4);

        writer.patch_u32(group_offset_pos, (writer.pos() - section_start) as u32);
        for group in &self.groups {
            writer.write_fixed_string(&group.group_name, 0x21);
            writer.write_u8(group.flag);
            writer.write_u16(group.reserve0);
        }

        if let Some(user_data) = &self.user_data {
            writer.align(4);
            writer.patch_u32(user_data_offset_pos, (writer.pos() - section_start) as u32);

            let embed_start = writer.pos();
            writer.write_u32(MAGIC_USERDATA);
            let embed_size_pos = writer.write_placeholder_u32();

            user_data.serialize(writer);

            let embed_size = (writer.pos() - embed_start) as u32;
            writer.patch_u32(embed_size_pos, embed_size);
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResBflanGroup {
    pub group_name: String,
    pub flag: u8,
    pub reserve0: u16,
}

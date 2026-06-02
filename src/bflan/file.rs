use serde::{Deserialize, Serialize};

use crate::{
    bflan::{anim_info::PaneAnimInfo, anim_tag::ResBflanPaneAnimTag, constants::*},
    bflyt::constants::*,
    core::{Cursor, SectionHeader, Writer, tchar_code32},
    ui2d::userdata::ResUi2dUserDataSection,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Bflan {
    pub magic: u32,
    pub endianness: u16,
    pub header_size: u16,
    pub micro_version: u16,
    pub minor_version: u8,
    pub major_version: u8,
    pub file_size: u32,
    pub section_count: u32,

    pub sections: Vec<BflanSections>,
}

impl Bflan {
    pub fn parse(file: &[u8]) -> Result<Self, String> {
        let mut cursor = Cursor { data: file, pos: 0 };

        let magic = cursor.read_u32();

        if magic != tchar_code32(b"FLAN") {
            return Err("bad magic".into());
        }

        let endianness = cursor.read_u16();
        let header_size = cursor.read_u16();
        let micro_version = cursor.read_u16();
        let minor_version = cursor.read_u8();
        let major_version = cursor.read_u8();
        let file_size = cursor.read_u32();
        let section_count = cursor.read_u32();

        let sections = BflanSections::parse(&mut cursor, section_count);

        Ok(Self {
            magic,
            endianness,
            header_size,
            micro_version,
            minor_version,
            major_version,
            file_size,
            section_count,
            sections,
        })
    }

    pub fn serialize(&self) -> Writer {
        let mut writer = Writer::new();

        writer.mark("File header");
        writer.write_u32(self.magic);
        writer.write_u16(self.endianness);
        writer.write_u16(self.header_size);
        writer.write_u16(self.micro_version);
        writer.write_u8(self.minor_version);
        writer.write_u8(self.major_version);

        let file_size_pos = writer.write_placeholder_u32();
        writer.write_u32(self.sections.len() as u32);

        for section in &self.sections {
            section.serialize(&mut writer);
        }

        let total_size = writer.pos() as u32;
        writer.patch_u32(file_size_pos, total_size);

        writer
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BflanSections {
    UserData(ResUi2dUserDataSection),
    PaneAnimTag(ResBflanPaneAnimTag),
    PaneAnimInfo(PaneAnimInfo),
    Unknown(SectionHeader, Vec<u8>),
}

impl BflanSections {
    pub fn parse(cursor: &mut Cursor, count: u32) -> Vec<Self> {
        let mut sections = Vec::new();

        for _ in 0..count {
            let section_start = cursor.pos;

            let header = SectionHeader {
                magic: cursor.read_u32(),
                size: cursor.read_u32(),
            };

            match header.magic {
                MAGIC_USERDATA => {
                    sections.push(Self::UserData(ResUi2dUserDataSection::parse(cursor)));
                }
                MAGIC_ANIMTAG => {
                    sections.push(Self::PaneAnimTag(ResBflanPaneAnimTag::parse(
                        cursor,
                        section_start,
                    )));
                }
                MAGIC_ANIMINFO => {
                    sections.push(Self::PaneAnimInfo(PaneAnimInfo::parse(
                        cursor,
                        section_start,
                    )));
                }
                _ => {
                    let data = cursor.read_bytes((header.size - 8) as usize).to_vec();

                    sections.push(Self::Unknown(header, data));
                }
            }

            cursor.seek(section_start + header.size as usize);
        }

        sections
    }

    pub fn serialize(&self, writer: &mut Writer) {
        let section_start = writer.pos();

        writer.mark("Section (header)");
        match self {
            Self::UserData(_) => writer.write_u32(MAGIC_USERDATA),
            Self::PaneAnimTag(_) => writer.write_u32(MAGIC_ANIMTAG),
            Self::PaneAnimInfo(_) => writer.write_u32(MAGIC_ANIMINFO),
            Self::Unknown(header, _) => writer.write_u32(header.magic),
        }

        let size_pos = writer.write_placeholder_u32();

        writer.mark("Section (data)");
        match self {
            Self::UserData(data) => data.serialize(writer),
            Self::PaneAnimTag(tag) => tag.serialize(writer, section_start),
            Self::PaneAnimInfo(info) => info.serialize(writer, section_start),
            Self::Unknown(_, raw_data) => {
                writer.write_bytes(raw_data);
            }
        }

        writer.align(4);

        let size = (writer.pos() - section_start) as u32;
        writer.patch_u32(size_pos, size);
    }
}

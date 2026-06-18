use serde::{Deserialize, Serialize};

use crate::{
    bflan::{anim_info::PaneAnimInfo, anim_tag::ResBflanPaneAnimTag, constants::*},
    bflyt::constants::*,
    core::{Cursor, FormatError, ReadWriteable, SectionHeader, Writer, tchar_code32},
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

    pub sections: Vec<BflanSections>,
}

impl ReadWriteable for Bflan {
    const EXTENSION: &'static str = "bflan";

    fn parse(file: &[u8]) -> Result<Self, FormatError> {
        let mut cursor = Cursor { data: file, pos: 0 };

        let magic = cursor.read_u32()?;
        if magic != tchar_code32(b"FLAN") {
            return Err(FormatError::InvalidMagic {
                expected: "FLAN",
                found: magic,
                offset: 0,
            });
        }

        let endianness = cursor.read_u16()?;
        let header_size = cursor.read_u16()?;
        let micro_version = cursor.read_u16()?;
        let minor_version = cursor.read_u8()?;
        let major_version = cursor.read_u8()?;
        let _file_size = cursor.read_u32()?;
        let section_count = cursor.read_u32()?;

        if (header_size as usize) > file.len() {
            return Err(FormatError::InvalidHeaderSize {
                specified_size: header_size as usize,
                actual_size: file.len(),
            });
        }

        let sections = BflanSections::parse(&mut cursor, section_count)?;

        Ok(Self {
            magic,
            endianness,
            header_size,
            micro_version,
            minor_version,
            major_version,
            sections,
        })
    }

    fn write(&self) -> Writer {
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
    pub fn parse(cursor: &mut Cursor, count: u32) -> Result<Vec<Self>, FormatError> {
        let mut sections = Vec::new();

        for i in 0..count {
            let section_start = cursor.pos;

            let header =
                SectionHeader::parse(cursor).map_err(|e| FormatError::SectionCountMismatch {
                    expected: count,
                    actual: i,
                    source: Box::new(e),
                })?;

            if header.size < 8 {
                return Err(FormatError::SectionCountMismatch {
                    expected: count,
                    actual: i,
                    source: Box::new(FormatError::MalformedSection {
                        section_type: format!("Header(0x{:08X})", header.magic),
                        offset: section_start,
                        reason: format!(
                            "Declared section size ({}) is smaller than minimum header size of 8 bytes",
                            header.size
                        ),
                    }),
                });
            }

            let parse_body = |cursor: &mut Cursor| -> Result<Self, FormatError> {
                let section_variant = match header.magic {
                    MAGIC_USERDATA => Self::UserData(ResUi2dUserDataSection::parse(cursor, false)?),
                    MAGIC_ANIMTAG => {
                        Self::PaneAnimTag(ResBflanPaneAnimTag::parse(cursor, section_start)?)
                    }
                    MAGIC_ANIMINFO => {
                        Self::PaneAnimInfo(PaneAnimInfo::parse(cursor, section_start)?)
                    }
                    _ => {
                        let remaining_payload = (header.size - 8) as usize;
                        let data = cursor.read_bytes(remaining_payload)?.to_vec();
                        Self::Unknown(header, data)
                    }
                };
                Ok(section_variant)
            };

            let section = parse_body(cursor).map_err(|e| FormatError::SectionCountMismatch {
                expected: count,
                actual: i,
                source: Box::new(e),
            })?;

            sections.push(section);

            cursor
                .seek(section_start + header.size as usize)
                .map_err(|e| FormatError::SectionCountMismatch {
                    expected: count,
                    actual: i,
                    source: Box::new(e),
                })?;
        }

        Ok(sections)
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

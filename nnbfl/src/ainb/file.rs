use crate::{
    ainb::blackboard::Blackboard,
    core::{Cursor, FileReadWriteable, FormatError, ReadWriteable, Writer, tchar_code32},
};

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct Ainb {
    pub version: u32,
    pub name: String,

    pub command_count: u32,
    pub element_count: u32,
    pub query_count: u32,
    pub attachment_count: u32,
    pub element_output_count: u32,

    pub blackboard: Blackboard,
}

impl ReadWriteable for Ainb {
    fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let magic = cursor.read_u32()?;

        if magic != tchar_code32(b"AIB ") {
            return Err(FormatError::InvalidMagic {
                expected: "AIB ",
                found: magic,
                offset: 0,
            });
        };

        let version = cursor.read_u32()?;
        cursor.version = version;

        let name_offset = cursor.read_u32()?;
        let command_count = cursor.read_u32()?;
        let element_count = cursor.read_u32()?;
        let query_count = cursor.read_u32()?;
        let attachment_count = cursor.read_u32()?;
        let element_output_count = cursor.read_u32()?;

        let blackboard_offset = cursor.read_u32()?;
        let string_pool_start = cursor.read_u32()?;

        cursor.string_pool_start = Some(string_pool_start as usize);

        let name = cursor.read_string_from_pool_by_offset(name_offset)?;
        let blackboard = cursor.at(blackboard_offset as usize, |c| Blackboard::parse(c))?;

        Ok(Self {
            version,
            name,
            command_count,
            element_count,
            query_count,
            attachment_count,
            element_output_count,
            blackboard,
        })
    }

    fn write(&self, writer: &mut Writer) {}
}

impl FileReadWriteable for Ainb {
    const INPUT_EXTENSION: &'static str = "ainb";
}

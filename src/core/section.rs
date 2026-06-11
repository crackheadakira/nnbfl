use serde::{Deserialize, Serialize};

use crate::core::{Cursor, FormatError};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct SectionHeader {
    pub magic: u32,
    pub size: u32,
}

impl SectionHeader {
    pub fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            magic: cursor.read_u32()?,
            size: cursor.read_u32()?,
        })
    }
}

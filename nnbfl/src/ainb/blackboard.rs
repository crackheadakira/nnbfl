use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};

use crate::core::BitPackable;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Blackboard {
    pub entry_string: BlackboardTypeEntry,
    pub entry_int: BlackboardTypeEntry,
    pub entry_u32: Option<BlackboardTypeEntry>,
    pub entry_float: BlackboardTypeEntry,
    pub entry_bool: BlackboardTypeEntry,
    pub entry_vector3f: BlackboardTypeEntry,
    pub entry_void: BlackboardTypeEntry,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BlackboardTypeEntry {
    pub count: u16,
    pub base_index: u16,
    pub base_offset: u16,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BlackboardEntry {
    pub flags: BlackboardFlags,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlackboardFlags {
    pub name_offset: u32,
    pub bgyml_id: u8,
    pub is_bgyml_id: bool,
}

impl BlackboardFlags {
    const NAME_OFFSET_MASK: u32 = 0x3F_FFFF;
    const BGYML_ID_MASK: u32 = 0x7F;
    const BGYML_ID_SHIFT: u32 = 24;
    const IS_BGYML_ID_BIT: u32 = 1 << 31;
}

impl BitPackable<u32> for BlackboardFlags {
    fn decode(raw: u32) -> Self {
        Self {
            name_offset: raw & Self::NAME_OFFSET_MASK,
            bgyml_id: ((raw >> Self::BGYML_ID_SHIFT) & Self::BGYML_ID_MASK) as u8,
            is_bgyml_id: raw & Self::IS_BGYML_ID_BIT != 0,
        }
    }

    fn encode(&self) -> u32 {
        (self.name_offset & Self::NAME_OFFSET_MASK)
            | (u32::from(self.bgyml_id) & Self::BGYML_ID_MASK) << Self::BGYML_ID_SHIFT
            | u32::from(self.is_bgyml_id) << 31
    }
}

#[derive(Debug, Serialize, Deserialize, IntoPrimitive, FromPrimitive)]
#[repr(u32)]
pub enum BlackboardType {
    #[default]
    String,
    S32,
    U32,
    Float,
    Bool,
    Vector3f,
    Pointer,
}

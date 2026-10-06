use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};

use crate::core::{BitPackable, Cursor, FormatError, ReadWriteable, Writer};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Blackboard {
    pub entry_string: BlackboardTypeEntry,
    pub entry_int: BlackboardTypeEntry,

    // Only on version >= 0x408
    pub entry_u32: Option<BlackboardTypeEntry>,

    pub entry_float: BlackboardTypeEntry,
    pub entry_bool: BlackboardTypeEntry,
    pub entry_vector3f: BlackboardTypeEntry,
    pub entry_void: BlackboardTypeEntry,

    pub string_entries: Vec<BlackboardEntry>,
    pub int_entries: Vec<BlackboardEntry>,
}

impl ReadWriteable for Blackboard {
    fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let entry_string = BlackboardTypeEntry::parse(cursor)?;
        let entry_int = BlackboardTypeEntry::parse(cursor)?;

        let entry_u32 = if cursor.version >= 0x408 {
            Some(BlackboardTypeEntry::parse(cursor)?)
        } else {
            None
        };

        let entry_float = BlackboardTypeEntry::parse(cursor)?;
        let entry_bool = BlackboardTypeEntry::parse(cursor)?;
        let entry_vector3f = BlackboardTypeEntry::parse(cursor)?;
        let entry_void = BlackboardTypeEntry::parse(cursor)?;

        let offset = cursor.pos;

        let total_entries = usize::from(entry_void.base_index) + usize::from(entry_void.count);
        let file_ref_offset = offset
            + total_entries * BlackboardEntry::STRUCT_SIZE
            + usize::from(entry_vector3f.base_offset)
            + usize::from(entry_vector3f.count) * 0xC;

        cursor.section_start = Some(file_ref_offset);

        let string_entries = Vec::new();

        let mut int_entries = Vec::new();

        for idx in 0..entry_int.count {
            cursor.seek(
                offset
                    + (entry_int.base_index as usize + idx as usize) * BlackboardEntry::STRUCT_SIZE,
            )?;
            let entry = BlackboardEntry::parse(cursor)?;

            int_entries.push(entry);
        }

        cursor.section_start = None;

        Ok(Self {
            entry_string,
            entry_int,
            entry_u32,
            entry_float,
            entry_bool,
            entry_vector3f,
            entry_void,
            string_entries,
            int_entries,
        })
    }

    fn write(&self, writer: &mut Writer) {
        // reminder, `base_index` & `base_offset` are sequentially increased depending on prior entries.
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BlackboardTypeEntry {
    pub count: u16,
    pub base_index: u16,
    pub base_offset: u16,
}

impl ReadWriteable for BlackboardTypeEntry {
    fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let s = Self {
            count: cursor.read_u16()?,
            base_index: cursor.read_u16()?,
            base_offset: cursor.read_u16()?,
        };

        let _ = cursor.read_u16()?;

        Ok(s)
    }

    fn write(&self, writer: &mut Writer) {
        writer.write_u16(self.count);
        writer.write_u16(self.base_index);
        writer.write_u16(self.base_offset);
        writer.write_u16(0);
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct FileReferenceEntry {
    pub file_path: String,
    pub file_path_hash: u32,
    pub file_name_hash: u32,
    pub file_ext_hash: u32,
}

impl ReadWriteable for FileReferenceEntry {
    fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            file_path: cursor.read_string_from_pool()?,
            file_path_hash: cursor.read_u32()?,
            file_name_hash: cursor.read_u32()?,
            file_ext_hash: cursor.read_u32()?,
        })
    }

    fn write(&self, writer: &mut Writer) {
        writer.write_string_offset(&self.file_path);
        writer.write_u32(self.file_path_hash);
        writer.write_u32(self.file_name_hash);
        writer.write_u32(self.file_ext_hash);
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BlackboardEntry {
    pub flags: BlackboardFlags,
    pub unk_string: String,
    pub name: String,

    pub file_reference: Option<FileReferenceEntry>,
}

impl BlackboardEntry {
    pub const STRUCT_SIZE: usize = 0x8;
}

impl ReadWriteable for BlackboardEntry {
    fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let flags = BlackboardFlags::decode(cursor.read_u32()?);
        let unk_string = cursor.read_string_from_pool()?;
        let name = cursor.read_string_from_pool_by_offset(flags.name_offset)?;

        let file_reference = if flags.has_file_ref {
            let file_ref_pool = cursor.ctx_section_start::<Self>()?;

            Some(cursor.at(
                file_ref_pool + (flags.file_ref_idx as u32 * 0x10) as usize,
                |c| FileReferenceEntry::parse(c),
            )?)
        } else {
            None
        };

        Ok(Self {
            flags,
            unk_string,
            name,
            file_reference,
        })
    }

    fn write(&self, writer: &mut Writer) {
        let name_offset = writer.intern_string(&self.name);

        let mut flags = self.flags.clone();
        flags.name_offset = name_offset;

        writer.write_u32(flags.encode());
        writer.write_string_offset(&self.unk_string);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlackboardFlags {
    pub name_offset: u32,
    pub inherit_mode: InheritMode,
    pub file_ref_idx: u8,
    pub has_file_ref: bool,
}

impl BlackboardFlags {
    const NAME_OFFSET_MASK: u32 = 0x3F_FFFF;

    const INHERIT_MODE_MASK: u32 = 0x3;
    const INHERIT_MODE_SHIFT: u32 = 22;

    const FILE_REF_IDX_MASK: u32 = 0x7F;
    const FILE_REF_IDX_SHIFT: u32 = 24;

    const HAS_FILE_REF_BIT: u32 = 1 << 31;
}

impl BitPackable<u32> for BlackboardFlags {
    fn decode(raw: u32) -> Self {
        Self {
            name_offset: raw & Self::NAME_OFFSET_MASK,
            inherit_mode: (((raw >> Self::INHERIT_MODE_SHIFT) & Self::INHERIT_MODE_MASK) as u8)
                .into(),
            file_ref_idx: ((raw >> Self::FILE_REF_IDX_SHIFT) & Self::FILE_REF_IDX_MASK) as u8,
            has_file_ref: raw & Self::HAS_FILE_REF_BIT != 0,
        }
    }

    fn encode(&self) -> u32 {
        (self.name_offset & Self::NAME_OFFSET_MASK)
            | ((self.inherit_mode as u32 & Self::INHERIT_MODE_MASK) << Self::INHERIT_MODE_SHIFT)
            | ((u32::from(self.file_ref_idx) & Self::FILE_REF_IDX_MASK) << Self::FILE_REF_IDX_SHIFT)
            | (u32::from(self.has_file_ref) << 31)
    }
}

#[derive(
    Default, Debug, Serialize, Deserialize, IntoPrimitive, FromPrimitive, Clone, Copy, PartialEq, Eq,
)]
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

#[derive(
    Default, Debug, Serialize, Deserialize, IntoPrimitive, FromPrimitive, Clone, Copy, PartialEq, Eq,
)]
#[repr(u8)]
pub enum InheritMode {
    #[default]
    InheritFromRoot,
    InheritFromParent,
    DontInherit,
}

pub trait BlackboardValue: Sized {
    const SERIALIZED_SIZE: usize = 0x4;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError>;
    fn write_value(&self, writer: &mut Writer);
}

impl BlackboardValue for String {
    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(cursor.read_string_from_pool()?)
    }

    fn write_value(&self, writer: &mut Writer) {
        writer.write_string_offset(self);
    }
}

impl BlackboardValue for i32 {
    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(cursor.read_i32()?)
    }

    fn write_value(&self, writer: &mut Writer) {
        writer.write_i32(*self);
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BlackboardParam<T: BlackboardValue> {
    pub name: String,
    pub notes: String,
    pub inherit_mode: InheritMode,
    pub file_reference: Option<FileReferenceEntry>,
    pub value: T,
}

use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};

use crate::{
    core::{BitPackable, Cursor, FormatError, ReadWriteable, Writer},
    ui2d::types::Vector3f,
};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Blackboard {
    pub string_entries: Vec<BlackboardParam<String>>,
    pub int_entries: Vec<BlackboardParam<i32>>,

    // Only on version >= 0x408
    pub uint_entries: Vec<BlackboardParam<u32>>,

    pub float_entries: Vec<BlackboardParam<f32>>,
    pub bool_entries: Vec<BlackboardParam<bool>>,
    pub vector3f_entries: Vec<BlackboardParam<Vector3f>>,
    pub void_entries: Vec<BlackboardParam<()>>,
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

        let entries_start = cursor.pos;

        let total_entries = usize::from(entry_void.base_index) + usize::from(entry_void.count);
        let values_start = entries_start + total_entries * BlackboardEntry::STRUCT_SIZE;

        let file_ref_offset = entries_start
            + total_entries * BlackboardEntry::STRUCT_SIZE
            + usize::from(entry_vector3f.base_offset)
            + usize::from(entry_vector3f.count) * 0xC;

        cursor.section_start = Some(file_ref_offset);

        let string_entries =
            entry_string.read_params::<String>(cursor, entries_start, values_start)?;
        let int_entries = entry_int.read_params::<i32>(cursor, entries_start, values_start)?;

        let uint_entries = if let Some(entry_u32) = entry_u32 {
            entry_u32.read_params::<u32>(cursor, entries_start, values_start)?
        } else {
            Vec::new()
        };

        let float_entries = entry_float.read_params::<f32>(cursor, entries_start, values_start)?;
        let bool_entries = entry_bool.read_params::<bool>(cursor, entries_start, values_start)?;
        let vector3f_entries =
            entry_vector3f.read_params::<Vector3f>(cursor, entries_start, values_start)?;
        let void_entries = entry_void.read_params::<()>(cursor, entries_start, values_start)?;

        cursor.section_start = None;

        Ok(Self {
            string_entries,
            int_entries,
            uint_entries,
            float_entries,
            bool_entries,
            vector3f_entries,
            void_entries,
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

impl BlackboardTypeEntry {
    pub fn read_params<T: BlackboardValue>(
        &self,
        cursor: &mut Cursor,
        entries_start: usize,
        values_start: usize,
    ) -> Result<Vec<BlackboardParam<T>>, FormatError> {
        let mut params = Vec::with_capacity(self.count as usize);

        for idx in 0..(self.count as usize) {
            let entry_address =
                entries_start + (self.base_index as usize + idx) * BlackboardEntry::STRUCT_SIZE;

            let value_address = values_start + self.base_offset as usize + idx * T::SERIALIZED_SIZE;

            let entry = cursor.at(entry_address, |c| BlackboardEntry::parse(c))?;
            let value = if T::SERIALIZED_SIZE == 0 {
                T::read_value(cursor)?
            } else {
                cursor.at(value_address, |c| T::read_value(c))?
            };

            params.push(BlackboardParam {
                name: entry.name,
                note: entry.note,
                inherit_mode: entry.flags.inherit_mode,
                file_reference: entry.file_reference,
                value,
            });
        }

        Ok(params)
    }
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
    pub note: String,
    pub name: String,

    pub file_reference: Option<FileReferenceEntry>,
}

impl BlackboardEntry {
    pub const STRUCT_SIZE: usize = 0x8;
}

impl ReadWriteable for BlackboardEntry {
    fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let flags = BlackboardFlags::decode(cursor.read_u32()?);
        let note = cursor.read_string_from_pool()?;
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
            note,
            name,
            file_reference,
        })
    }

    fn write(&self, writer: &mut Writer) {
        let name_offset = writer.intern_string(&self.name);

        let mut flags = self.flags.clone();
        flags.name_offset = name_offset;

        writer.write_u32(flags.encode());
        writer.write_string_offset(&self.note);
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
    const SERIALIZED_SIZE: usize;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError>;
    fn write_value(&self, writer: &mut Writer);
}

impl BlackboardValue for String {
    const SERIALIZED_SIZE: usize = 4;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(cursor.read_string_from_pool()?)
    }

    fn write_value(&self, writer: &mut Writer) {
        writer.write_string_offset(self);
    }
}

impl BlackboardValue for i32 {
    const SERIALIZED_SIZE: usize = 4;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(cursor.read_i32()?)
    }

    fn write_value(&self, writer: &mut Writer) {
        writer.write_i32(*self);
    }
}

impl BlackboardValue for u32 {
    const SERIALIZED_SIZE: usize = 4;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(cursor.read_u32()?)
    }

    fn write_value(&self, writer: &mut Writer) {
        writer.write_u32(*self);
    }
}

impl BlackboardValue for f32 {
    const SERIALIZED_SIZE: usize = 4;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(cursor.read_f32()?)
    }

    fn write_value(&self, writer: &mut Writer) {
        writer.write_f32(*self);
    }
}

impl BlackboardValue for bool {
    const SERIALIZED_SIZE: usize = 4;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(cursor.read_u32()? != 0)
    }

    fn write_value(&self, writer: &mut Writer) {
        writer.write_u32(*self as u32);
    }
}

impl BlackboardValue for Vector3f {
    const SERIALIZED_SIZE: usize = 12;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self::parse(cursor)?)
    }

    fn write_value(&self, writer: &mut Writer) {
        self.write(writer);
    }
}

impl BlackboardValue for () {
    const SERIALIZED_SIZE: usize = 0;

    fn read_value(_cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(())
    }

    fn write_value(&self, _writer: &mut Writer) {}
}

fn skip_blackboard_value<T: BlackboardValue>(_: &T) -> bool {
    T::SERIALIZED_SIZE == 0
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BlackboardParam<T: BlackboardValue> {
    pub name: String,
    pub note: String,
    pub inherit_mode: InheritMode,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub file_reference: Option<FileReferenceEntry>,

    #[serde(skip_serializing_if = "skip_blackboard_value", default)]
    pub value: T,
}

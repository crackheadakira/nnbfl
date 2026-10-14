use serde::{Deserialize, Serialize};

use crate::{
    ainb::blackboard::Blackboard,
    core::{Cursor, FileReadWriteable, FormatError, ReadWriteable, Writer, tchar_code32},
    ui2d::types::Vector3f,
};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Ainb {
    pub version: u32,
    pub name: String,

    pub command_count: u32,
    pub element_count: u32,
    pub query_count: u32,
    pub attachment_count: u32,
    pub element_output_count: u32,

    pub blackboard: Blackboard,
    pub enum_relocation_array: Vec<EnumRelocation>,
    pub property_table: Vec<u32>,
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

        let enum_relocation_array_offset = cursor.read_u32()?;
        let enum_relocation_array = cursor.at(enum_relocation_array_offset as usize, |c| {
            let array_count = c.read_u32()?;
            let mut array = Vec::new();

            for _ in 0..array_count {
                array.push(EnumRelocation::parse(c)?);
            }

            Ok(array)
        })?;

        let property_table_offset = cursor.read_u32()?;
        let property_table = cursor.at(property_table_offset as usize, |c| {
            let mut v = Vec::with_capacity(6);

            for _ in 0..6 {
                v.push(c.read_u32()?);
            }

            Ok(v)
        })?;

        Ok(Self {
            version,
            name,
            command_count,
            element_count,
            query_count,
            attachment_count,
            element_output_count,
            blackboard,
            enum_relocation_array,
            property_table,
        })
    }

    fn write(&self, writer: &mut Writer) {}
}

impl FileReadWriteable for Ainb {
    const INPUT_EXTENSION: &'static str = "ainb";
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct EnumRelocation {
    pub value_to_replace: i32,
    pub enum_class: String,
    pub enum_value: String,
}

impl ReadWriteable for EnumRelocation {
    fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let value_offset = cursor.read_u32()?;

        Ok(Self {
            value_to_replace: cursor.at(value_offset as usize, |c| c.read_i32())?,
            enum_class: cursor.read_string_from_pool()?,
            enum_value: cursor.read_string_from_pool()?,
        })
    }

    fn write(&self, writer: &mut Writer) {}
}

pub trait SerializableValue: Sized {
    const SERIALIZED_SIZE: usize;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError>;
    fn write_value(&self, writer: &mut Writer);
}

impl SerializableValue for String {
    const SERIALIZED_SIZE: usize = 4;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(cursor.read_string_from_pool()?)
    }

    fn write_value(&self, writer: &mut Writer) {
        writer.write_string_offset(self);
    }
}

impl SerializableValue for i32 {
    const SERIALIZED_SIZE: usize = 4;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(cursor.read_i32()?)
    }

    fn write_value(&self, writer: &mut Writer) {
        writer.write_i32(*self);
    }
}

impl SerializableValue for u32 {
    const SERIALIZED_SIZE: usize = 4;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(cursor.read_u32()?)
    }

    fn write_value(&self, writer: &mut Writer) {
        writer.write_u32(*self);
    }
}

impl SerializableValue for f32 {
    const SERIALIZED_SIZE: usize = 4;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(cursor.read_f32()?)
    }

    fn write_value(&self, writer: &mut Writer) {
        writer.write_f32(*self);
    }
}

impl SerializableValue for bool {
    const SERIALIZED_SIZE: usize = 4;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(cursor.read_u32()? != 0)
    }

    fn write_value(&self, writer: &mut Writer) {
        writer.write_u32(*self as u32);
    }
}

impl SerializableValue for Vector3f {
    const SERIALIZED_SIZE: usize = 12;

    fn read_value(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self::parse(cursor)?)
    }

    fn write_value(&self, writer: &mut Writer) {
        self.write(writer);
    }
}

impl SerializableValue for () {
    const SERIALIZED_SIZE: usize = 0;

    fn read_value(_cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(())
    }

    fn write_value(&self, _writer: &mut Writer) {}
}

pub(crate) fn skip_serializable_value<T: SerializableValue>(_: &T) -> bool {
    T::SERIALIZED_SIZE == 0
}

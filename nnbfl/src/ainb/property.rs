use serde::{Deserialize, Serialize};

use crate::{
    ainb::file::{SerializableValue, skip_serializable_value},
    core::{BitPackable, Cursor, FormatError, ReadWriteable, Writer},
};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
pub struct PropertyFlags {
    pub index: u32,
    pub is_use_index: bool,
    pub is_not_local_blackboard_index: bool,
    pub is_exb_index: bool,
    pub pulse_thread_local_storage: bool,
    pub set_pointer_flag_bit_zero: bool,
}

impl PropertyFlags {
    const INDEX_MASK: u32 = 0xFFFF;

    const HAS_USE_INDEX_BIT: u32 = 1 << 16;
    const HAS_LOCAL_BLACKBOARD_INDEX_BIT: u32 = 1 << 17;
    const HAS_EXB_BIT: u32 = 1 << 22;
    const HAS_PL_BIT: u32 = 1 << 23;
    const HAS_POINTER_BIT: u32 = 1 << 24;
}

impl BitPackable<u32> for PropertyFlags {
    fn decode(raw: u32) -> Self {
        Self {
            index: raw & Self::INDEX_MASK,
            is_use_index: raw & Self::HAS_USE_INDEX_BIT != 0,
            is_not_local_blackboard_index: raw & Self::HAS_LOCAL_BLACKBOARD_INDEX_BIT != 0,
            is_exb_index: raw & Self::HAS_EXB_BIT != 0,
            pulse_thread_local_storage: raw & Self::HAS_PL_BIT != 0,
            set_pointer_flag_bit_zero: raw & Self::HAS_POINTER_BIT != 0,
        }
    }

    fn encode(&self) -> u32 {
        (self.index & Self::INDEX_MASK)
            | (u32::from(self.is_use_index) << 16)
            | (u32::from(self.is_not_local_blackboard_index) << 17)
            | (u32::from(self.is_exb_index) << 22)
            | (u32::from(self.pulse_thread_local_storage) << 23)
            | (u32::from(self.set_pointer_flag_bit_zero) << 24)
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Property<T: SerializableValue> {
    pub name: String,
    pub property_flags: PropertyFlags,

    #[serde(skip_serializing_if = "skip_serializable_value", default)]
    pub value: T,
}

impl<T: SerializableValue> ReadWriteable for Property<T> {
    fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            name: cursor.read_string_from_pool()?,
            property_flags: PropertyFlags::decode(cursor.read_u32()?),
            value: T::read_value(cursor)?,
        })
    }

    fn write(&self, writer: &mut Writer) {
        writer.write_string_offset(&self.name);
        writer.write_u32(self.property_flags.encode());
        self.value.write_value(writer);
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PropertyPointer {
    pub name: String,
    pub class_name: String,
    pub property_flags: PropertyFlags,
}

impl ReadWriteable for PropertyPointer {
    fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            name: cursor.read_string_from_pool()?,
            class_name: cursor.read_string_from_pool()?,
            property_flags: PropertyFlags::decode(cursor.read_u32()?),
        })
    }

    fn write(&self, writer: &mut Writer) {
        writer.write_string_offset(&self.name);
        writer.write_string_offset(&self.class_name);
        writer.write_u32(self.property_flags.encode());
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PropertyTabe {}

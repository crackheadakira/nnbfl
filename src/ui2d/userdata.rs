use serde::{Deserialize, Serialize};

use crate::{
    core::{Cursor, Writer},
    ui2d::{systemdata::ResUi2dSystemDataArray, types::Ui2dUserDataType},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ResUi2dUserDataSection {
    pub user_data_count: u16,
    pub reserve0: u16,
    pub user_data: Vec<ResUi2dUserData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResUi2dUserData {
    pub name_offset: u32,
    pub data_array_offset: u32,
    pub data_count: u16,
    pub data_type: Ui2dUserDataType,
    pub reserve0: u8,
    pub data_array: Vec<ResUi2dUserDataInner>,
    pub o_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResUi2dUserDataInner {
    Float(f32),
    S32(i32),
    String(String),
    SystemData(ResUi2dSystemDataArray),
}

impl ResUi2dUserDataSection {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let user_data_count = cursor.read_u16();
        let reserve0 = cursor.read_u16();
        let mut user_data = Vec::new();

        for _ in 0..user_data_count {
            user_data.push(ResUi2dUserData::parse(cursor))
        }

        Self {
            user_data_count,
            reserve0,
            user_data,
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("UserData (section)");
        writer.write_u16(self.user_data.len() as u16);
        writer.write_u16(self.reserve0);

        for data in &self.user_data {
            data.serialize(writer);
        }
    }
}

impl ResUi2dUserData {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let base_offset = cursor.pos;

        let mut data = Self {
            name_offset: cursor.read_u32(),
            data_array_offset: cursor.read_u32(),
            data_count: cursor.read_u16(),
            data_type: cursor.read_u8().into(),
            reserve0: cursor.read_u8(),
            data_array: Vec::new(),
            o_name: String::new(),
        };

        let restore_point = cursor.pos;

        if data.data_array_offset > 0 {
            cursor.seek(base_offset + data.data_array_offset as usize);

            match data.data_type {
                Ui2dUserDataType::Float => {
                    for _ in 0..data.data_count {
                        data.data_array
                            .push(ResUi2dUserDataInner::Float(cursor.read_f32()));
                    }
                }
                Ui2dUserDataType::S32 => {
                    for _ in 0..data.data_count {
                        data.data_array
                            .push(ResUi2dUserDataInner::S32(cursor.read_i32()));
                    }
                }
                Ui2dUserDataType::String => {
                    let str_data = cursor.read_string(data.data_count as usize);
                    data.data_array.push(ResUi2dUserDataInner::String(str_data));
                }
                Ui2dUserDataType::SystemData => {
                    for _ in 0..data.data_count {
                        /*if let Some(sys_data) = ResUi2dSystemDataArray::parse(cursor) {
                            data.data_array
                                .push(ResUi2dUserDataInner::SystemData(sys_data));
                        }*/
                    }
                }
                _ => {}
            }
        }

        cursor.seek(base_offset + data.name_offset as usize);
        data.o_name = cursor.read_null_terminated_string();

        cursor.seek(restore_point);

        data
    }

    pub fn serialize(&self, writer: &mut Writer) {
        let base_offset = writer.pos();

        writer.mark("UserData (data)");
        let name_offset_pos = writer.write_placeholder_u32();
        let data_array_offset_pos = writer.write_placeholder_u32();
        writer.write_u16(self.data_count);

        let data_type_val = match self.data_type {
            Ui2dUserDataType::String => 0,
            Ui2dUserDataType::S32 => 1,
            Ui2dUserDataType::Float => 2,
            Ui2dUserDataType::SystemData => 3,
            Ui2dUserDataType::Invalid => 4,
        };
        writer.write_u8(data_type_val);
        writer.write_u8(self.reserve0);

        if !self.data_array.is_empty() {
            writer.patch_u32(data_array_offset_pos, (writer.pos() - base_offset) as u32);

            for item in &self.data_array {
                match item {
                    ResUi2dUserDataInner::Float(f) => writer.write_f32(*f),
                    ResUi2dUserDataInner::S32(s) => writer.write_i32(*s),
                    ResUi2dUserDataInner::String(s) => {
                        writer.write_fixed_string(s, self.data_count as usize);
                        writer.write_u8(0);
                    }
                    ResUi2dUserDataInner::SystemData(_) => {}
                }
            }
        }

        writer.patch_u32(name_offset_pos, (writer.pos() - base_offset) as u32);
        writer.write_null_terminated_string(&self.o_name);

        writer.align(4);
    }
}

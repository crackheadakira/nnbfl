use serde::{Deserialize, Serialize};

use crate::core::{Cursor, Writer};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Color4f {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color4f {
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            r: cursor.read_f32(),
            g: cursor.read_f32(),
            b: cursor.read_f32(),
            a: cursor.read_f32(),
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("Color4f");

        writer.write_f32(self.r);
        writer.write_f32(self.g);
        writer.write_f32(self.b);
        writer.write_f32(self.a);
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum Ui2dUserDataType {
    String = 0,
    S32 = 1,
    Float = 2,
    SystemData = 3,
    Invalid = 4,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct VertexPos {
    pub size_scale_width: f32,
    pub size_scale_height: f32,
    pub position_x_scale: f32,
    pub position_y_scale: f32,
}

impl VertexPos {
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            size_scale_width: cursor.read_f32(),
            size_scale_height: cursor.read_f32(),
            position_x_scale: cursor.read_f32(),
            position_y_scale: cursor.read_f32(),
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("VertexPos");
        writer.write_f32(self.size_scale_width);
        writer.write_f32(self.size_scale_height);
        writer.write_f32(self.position_x_scale);
        writer.write_f32(self.position_y_scale);
    }
}

impl From<u8> for Ui2dUserDataType {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::String,
            1 => Self::S32,
            2 => Self::Float,
            3 => Self::SystemData,
            _ => Self::Invalid,
        }
    }
}

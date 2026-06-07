use num_enum::{FromPrimitive, IntoPrimitive};
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

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Vector2f {
    pub x: f32,
    pub y: f32,
}

impl Vector2f {
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            x: cursor.read_f32(),
            y: cursor.read_f32(),
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("Vector2f");

        writer.write_f32(self.x);
        writer.write_f32(self.y);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Vector3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3f {
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            x: cursor.read_f32(),
            y: cursor.read_f32(),
            z: cursor.read_f32(),
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("Vector3f");

        writer.write_f32(self.x);
        writer.write_f32(self.y);
        writer.write_f32(self.z);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, IntoPrimitive, FromPrimitive)]
#[repr(u8)]
pub enum Ui2dUserDataType {
    String = 0,
    S32 = 1,
    Float = 2,
    SystemData = 3,
    #[num_enum(default)]
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

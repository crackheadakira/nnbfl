use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Color4f {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct VertexPos {
    pub size_scale_width: f32,
    pub size_scale_height: f32,
    pub position_x_scale: f32,
    pub position_y_scale: f32,
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

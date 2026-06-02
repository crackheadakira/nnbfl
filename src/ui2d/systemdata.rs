use serde::{Deserialize, Serialize};

use crate::ui2d::types::{Color4f, VertexPos};

#[derive(Debug, Serialize, Deserialize)]
pub struct ResUi2dSystemDataArray {
    pub reserve0: u16,
    pub count: u16,
    pub offset: u32,
    pub data_array: Vec<ResUi2dSystemDataInner>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResUi2dSystemDataInner {
    Layout(),
    Pane(ResUi2dPaneData),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResUi2dPaneData {
    VertexPos(VertexPos),
    ProceduralShape(ResUi2dSystemDataProceduralShape),
    Alignment(ResUi2dSystemDataAlignment),
    DropShadow(ResUi2dSystemDataDropShadow),
    MaskTexture(ResUi2dSystemDataMaskTexture),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResUi2dSystemDataAlignment {
    pub options: u32,
    pub margin: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResUi2dSystemDataDropShadow {
    pub texture_id: u16,
    pub u_options: u8,
    pub v_options: u8,
    pub flags: u8,
    pub reserve0: [u8; 3],
    pub reserve1: u8,
    pub reserve2: u8,
    pub reserve3: u8,
    pub reserve4: u8,
    pub reserve5: [u32; 5],
    pub reserve6: [f32; 2],
    pub reserve7: [f32; 2],
    pub reserve8: [f32; 2],
    pub reserve9: [f32; 2],
    pub reserve10: [f32; 2],
    pub reserve11: [f32; 2],
    pub reserve12: [f32; 2],
    pub reserve13: [f32; 2],
    pub reserve14: [f32; 2],
    pub reserve15: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResUi2dSystemDataMaskTexture {
    pub flags: u8,
    pub reserve0: [u8; 3],
    pub texture_id: u16,
    pub u_options: u8,
    pub v_options: u8,
    pub tex_ext_flags: u32,
    pub capture_texture_id: u16,
    pub capture_u_options: u8,
    pub capture_v_options: u8,
    pub is_use_capture_mask: u8,
    pub reserve1: [u8; 3],
    pub translation: [f32; 2],
    pub rotation: f32,
    pub scale: [f32; 2],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResUi2dSystemDataProceduralShape {
    pub options: u8,
    pub color0_options: u8,
    pub inner_shadow_options: u8,
    pub inner_shadow_base_comp: u8,
    pub color_overlay_options: u8,
    pub gradation_overlay_options: u8,
    pub drop_shadow_blend_mode: u8,
    pub drop_shadow_base_comp: u8,
    pub reserve0: [u8; 4],
    pub rounded_corner0: [f32; 4],
    pub rounded_corner1: [f32; 4],
    pub reserve1: f32,
    pub color0: Color4f,
    pub inner_shadow_color: Color4f,
    pub inner_shadow_transform: [f32; 3],
    pub color_overlay: Color4f,
    pub gradation_weights: [f32; 4],
    pub gradation_color_array: [Color4f; 4],
    pub gradation_rotation: f32,
    pub drop_shadow_color: Color4f,
    pub drop_shadow_transform: [f32; 3],
    pub reserve2: [u32; 4],
}

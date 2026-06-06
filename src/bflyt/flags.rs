use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum BflytOrigin {
    Center,
    LeftTop,
    RightBottom,
}

impl BflytOrigin {
    fn from_u8(value: u8) -> Self {
        match value & 0x03 {
            0 => BflytOrigin::Center,
            1 => BflytOrigin::LeftTop,
            2 => BflytOrigin::RightBottom,
            _ => BflytOrigin::Center,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum BflytParentOrigin {
    None,
    LeftTop,
    RightBottom,
}

impl BflytParentOrigin {
    fn from_u8(value: u8) -> Self {
        match value & 0x03 {
            0 => BflytParentOrigin::None,
            1 => BflytParentOrigin::LeftTop,
            2 => BflytParentOrigin::RightBottom,
            _ => BflytParentOrigin::None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct BflytOrigins {
    pub origin_x: BflytOrigin,
    pub origin_y: BflytOrigin,
    pub parent_origin_x: BflytParentOrigin,
    pub parent_origin_y: BflytParentOrigin,
}

impl BflytOrigins {
    pub fn decode(raw: u8) -> Self {
        Self {
            origin_x: BflytOrigin::from_u8(raw & 0x03),
            origin_y: BflytOrigin::from_u8((raw >> 2) & 0x03),
            parent_origin_x: BflytParentOrigin::from_u8((raw >> 4) & 0x03),
            parent_origin_y: BflytParentOrigin::from_u8(raw >> 6),
        }
    }

    pub fn encode(&self) -> u8 {
        let mut raw = 0u8;

        raw |= (self.origin_x as u8) & 0x03;
        raw |= ((self.origin_y as u8) & 0x03) << 2;
        raw |= ((self.parent_origin_x as u8) & 0x03) << 4;
        raw |= ((self.parent_origin_y as u8) & 0x03) << 6;

        raw
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct PaneFlags {
    pub is_visible: bool,
    pub is_scale_child_alpha: bool,
    pub reserve0: u8,
}

impl PaneFlags {
    pub fn decode(raw: u8) -> Self {
        Self {
            is_visible: (raw & 0x01) != 0,
            is_scale_child_alpha: ((raw >> 1) & 0x01) != 0,
            reserve0: raw >> 2,
        }
    }

    pub fn encode(&self) -> u8 {
        let mut raw = 0u8;

        if self.is_visible {
            raw |= 0x01;
        }

        if self.is_scale_child_alpha {
            raw |= 0x02;
        }

        raw |= (self.reserve0 << 2) & 0xFC;
        raw
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct PaneFlagsEx {
    pub is_no_scale_by_parts: bool,
    pub is_scale_size_by_parts_root: bool,
    pub is_ext_user_data: bool,
    pub reserve0: u8,
}

impl PaneFlagsEx {
    pub fn decode(raw: u8) -> Self {
        Self {
            is_no_scale_by_parts: (raw & 0x01) != 0,
            is_scale_size_by_parts_root: ((raw >> 1) & 0x01) != 0,
            is_ext_user_data: ((raw >> 2) & 0x01) != 0,
            reserve0: raw >> 3,
        }
    }

    pub fn encode(&self) -> u8 {
        let mut raw = 0u8;

        if self.is_no_scale_by_parts {
            raw |= 0x01;
        }

        if self.is_scale_size_by_parts_root {
            raw |= 0x02;
        }

        if self.is_ext_user_data {
            raw |= 0x04;
        }

        raw |= (self.reserve0 << 3) & 0xF8;
        raw
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct TextPaneFlags {
    pub is_enable_shadow: bool,
    pub is_limit_glyph_count_to_length: bool,
    pub is_invisible_border: bool,
    pub is_double_border: bool,
    pub is_per_character_transform: bool,
    pub is_enable_center_ceiling: bool,
    pub is_line_transform: bool,
    pub is_default_tag_processor: bool,
    pub is_per_character_transform_split_by_char_width: bool,
    pub is_mix_shadow_alpha: bool,
    pub is_reverse: bool,
    pub is_per_character_transform_origin_to_center: bool,
    pub reserve0: bool,
    pub reserve1: bool,
    pub reserve2: u16,
}

impl TextPaneFlags {
    pub fn decode(raw: u16) -> Self {
        Self {
            is_enable_shadow: (raw & 0x01) != 0,
            is_limit_glyph_count_to_length: ((raw >> 1) & 0x01) != 0,
            is_invisible_border: ((raw >> 2) & 0x01) != 0,
            is_double_border: ((raw >> 3) & 0x01) != 0,
            is_per_character_transform: ((raw >> 4) & 0x01) != 0,
            is_enable_center_ceiling: ((raw >> 5) & 0x01) != 0,
            is_line_transform: ((raw >> 6) & 0x01) != 0,
            is_default_tag_processor: ((raw >> 7) & 0x01) != 0,
            is_per_character_transform_split_by_char_width: ((raw >> 8) & 0x01) != 0,
            is_mix_shadow_alpha: ((raw >> 9) & 0x01) != 0,
            is_reverse: ((raw >> 10) & 0x01) != 0,
            is_per_character_transform_origin_to_center: ((raw >> 11) & 0x01) != 0,
            reserve0: ((raw >> 12) & 0x01) != 0,
            reserve1: ((raw >> 13) & 0x01) != 0,
            reserve2: (raw >> 14),
        }
    }

    pub fn encode(&self) -> u16 {
        let mut raw = 0u16;

        if self.is_enable_shadow {
            raw |= 1 << 0;
        }

        if self.is_limit_glyph_count_to_length {
            raw |= 1 << 1;
        }

        if self.is_invisible_border {
            raw |= 1 << 2;
        }

        if self.is_double_border {
            raw |= 1 << 3;
        }

        if self.is_per_character_transform {
            raw |= 1 << 4;
        }

        if self.is_enable_center_ceiling {
            raw |= 1 << 5;
        }

        if self.is_line_transform {
            raw |= 1 << 6;
        }

        if self.is_default_tag_processor {
            raw |= 1 << 7;
        }

        if self.is_per_character_transform_split_by_char_width {
            raw |= 1 << 8;
        }

        if self.is_mix_shadow_alpha {
            raw |= 1 << 9;
        }

        if self.is_reverse {
            raw |= 1 << 10;
        }

        if self.is_per_character_transform_origin_to_center {
            raw |= 1 << 11;
        }

        if self.reserve0 {
            raw |= 1 << 12;
        }

        if self.reserve1 {
            raw |= 1 << 13;
        }

        raw |= ((self.reserve2) & 0x03) << 14;

        raw
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WindowKind {
    Around,
    Horizontal,
    HorizontalNoContent,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct WindowFlags {
    pub use_layout_material: bool,
    pub use_vertex_color_for_all_window: bool,
    pub window_kind: WindowKind,
    pub not_draw_content: bool,
}

impl WindowFlags {
    pub fn decode(raw: u8) -> Self {
        let kind_bits = (raw >> 2) & 0x03;
        Self {
            use_layout_material: (raw & 0x01) != 0,
            use_vertex_color_for_all_window: ((raw >> 1) & 0x01) != 0,
            window_kind: match kind_bits {
                0 => WindowKind::Around,
                1 => WindowKind::Horizontal,
                2 => WindowKind::HorizontalNoContent,
                _ => WindowKind::Around,
            },
            not_draw_content: ((raw >> 4) & 0x01) != 0,
        }
    }

    pub fn encode(&self) -> u8 {
        let mut raw = 0u8;
        if self.use_layout_material {
            raw |= 1 << 0;
        }
        if self.use_vertex_color_for_all_window {
            raw |= 1 << 1;
        }
        raw |= ((self.window_kind as u8) & 0x03) << 2;
        if self.not_draw_content {
            raw |= 1 << 4;
        }
        raw
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TexWrapMode {
    Clamp,
    Repeat,
    Mirror,
}

impl From<u8> for TexWrapMode {
    fn from(val: u8) -> Self {
        match val & 0x03 {
            0 => TexWrapMode::Clamp,
            1 => TexWrapMode::Repeat,
            2 => TexWrapMode::Mirror,
            _ => TexWrapMode::Clamp,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TexFilter {
    Near,
    Linear,
}

impl From<u8> for TexFilter {
    fn from(val: u8) -> Self {
        match val & 0x3F {
            0 => TexFilter::Near,
            1 => TexFilter::Linear,
            _ => TexFilter::Near,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub struct TexOptions {
    pub wrap_mode: TexWrapMode,
    pub filter_mode: TexFilter,
}

impl TexOptions {
    pub fn decode(raw: u8) -> Self {
        Self {
            wrap_mode: (raw & 0x03).into(),
            filter_mode: (raw >> 2).into(),
        }
    }

    pub fn encode(&self) -> u8 {
        (self.wrap_mode as u8 & 0x03) | ((self.filter_mode as u8 & 0x3F) << 2)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub struct DropShadowFlags {
    pub is_stroke_enabled: bool,
    pub is_outer_glow_enabled: bool,
    pub is_drop_shadow_enabled: bool,
    pub is_knockout: bool,
    pub is_only_effect: bool,
    pub is_static_rendering: bool,
    pub is_degamma_enabled: bool,
}

impl DropShadowFlags {
    pub fn decode(raw: u8) -> Self {
        Self {
            is_stroke_enabled: (raw & 0x01) != 0,
            is_outer_glow_enabled: ((raw >> 1) & 0x01) != 0,
            is_drop_shadow_enabled: ((raw >> 2) & 0x01) != 0,
            is_knockout: ((raw >> 3) & 0x01) != 0,
            is_only_effect: ((raw >> 4) & 0x01) != 0,
            is_static_rendering: ((raw >> 5) & 0x01) != 0,
            is_degamma_enabled: ((raw >> 6) & 0x01) != 0,
        }
    }

    pub fn encode(&self) -> u8 {
        let mut raw = 0u8;
        if self.is_stroke_enabled {
            raw |= 1 << 0;
        }
        if self.is_outer_glow_enabled {
            raw |= 1 << 1;
        }
        if self.is_drop_shadow_enabled {
            raw |= 1 << 2;
        }
        if self.is_knockout {
            raw |= 1 << 3;
        }
        if self.is_only_effect {
            raw |= 1 << 4;
        }
        if self.is_static_rendering {
            raw |= 1 << 5;
        }
        if self.is_degamma_enabled {
            raw |= 1 << 6;
        }
        raw
    }
}

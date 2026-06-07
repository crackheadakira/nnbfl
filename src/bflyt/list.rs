use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};

use crate::{
    bflyt::flags::{TexFilter, TexWrapMode},
    core::{Cursor, Writer},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BflytLayout {
    pub is_centered: bool,
    pub reserve0: u8,
    pub reserve1: u16,
    pub width: f32,
    pub height: f32,
    pub parts_width: f32,
    pub parts_height: f32,
    pub name: String,
}

impl BflytLayout {
    pub fn parse(cursor: &mut Cursor) -> Self {
        Self {
            is_centered: cursor.read_u8() != 0,
            reserve0: cursor.read_u8(),
            reserve1: cursor.read_u16(),
            width: cursor.read_f32(),
            height: cursor.read_f32(),
            parts_width: cursor.read_f32(),
            parts_height: cursor.read_f32(),
            name: cursor.read_null_terminated_string(),
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_u8(self.is_centered.into());
        writer.write_u8(self.reserve0);
        writer.write_u16(self.reserve1);
        writer.write_f32(self.width);
        writer.write_f32(self.height);
        writer.write_f32(self.parts_width);
        writer.write_f32(self.parts_height);
        writer.write_null_terminated_string(&self.name);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BflytTextureList {
    pub textures: Vec<String>,
}

impl BflytTextureList {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let texture_count = cursor.read_u16();
        let _reserve0 = cursor.read_u16();

        let offsets_start = cursor.pos;
        let mut offsets = Vec::new();
        for _ in 0..texture_count {
            offsets.push(cursor.read_u32());
        }

        let mut textures = Vec::new();
        for offset in offsets {
            cursor.seek(offsets_start + offset as usize);
            textures.push(cursor.read_null_terminated_string());
        }

        Self { textures }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_u16(self.textures.len() as u16);
        writer.write_u16(0);

        let offsets_start = writer.pos();
        let mut offset_placeholders = Vec::new();
        for _ in &self.textures {
            offset_placeholders.push(writer.write_placeholder_u32());
        }

        for (i, name) in self.textures.iter().enumerate() {
            let offset = writer.pos() - offsets_start;
            writer.patch_u32(offset_placeholders[i], offset as u32);
            writer.write_null_terminated_string(name);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BflytFontList {
    pub fonts: Vec<String>,
}

impl BflytFontList {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let font_count = cursor.read_u16();
        let _reserve0 = cursor.read_u16();

        let offsets_start = cursor.pos;
        let mut offsets = Vec::new();
        for _ in 0..font_count {
            offsets.push(cursor.read_u32());
        }

        let mut fonts = Vec::new();
        for offset in offsets {
            cursor.seek(offsets_start + offset as usize);
            fonts.push(cursor.read_null_terminated_string());
        }

        Self { fonts }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_u16(self.fonts.len() as u16);
        writer.write_u16(0);

        let offsets_start = writer.pos();
        let mut offset_placeholders = Vec::new();
        for _ in &self.fonts {
            offset_placeholders.push(writer.write_placeholder_u32());
        }

        for (i, name) in self.fonts.iter().enumerate() {
            let offset = writer.pos() - offsets_start;
            writer.patch_u32(offset_placeholders[i], offset as u32);
            writer.write_null_terminated_string(name);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Color4u8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}
impl Color4u8 {
    fn parse(c: &mut Cursor) -> Self {
        Self {
            r: c.read_u8(),
            g: c.read_u8(),
            b: c.read_u8(),
            a: c.read_u8(),
        }
    }
    fn serialize(&self, w: &mut Writer) {
        w.write_u8(self.r);
        w.write_u8(self.g);
        w.write_u8(self.b);
        w.write_u8(self.a);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Color4f {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
impl Color4f {
    fn parse(c: &mut Cursor) -> Self {
        Self {
            r: c.read_f32(),
            g: c.read_f32(),
            b: c.read_f32(),
            a: c.read_f32(),
        }
    }
    fn serialize(&self, w: &mut Writer) {
        w.write_f32(self.r);
        w.write_f32(self.g);
        w.write_f32(self.b);
        w.write_f32(self.a);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialTextureOptions {
    pub wrap_mode: TexWrapMode,
    pub filter: TexFilter,
    pub reserve0: u8,
}

impl MaterialTextureOptions {
    pub fn decode(raw: u8) -> Self {
        Self {
            wrap_mode: (raw & 0x3).into(),
            filter: ((raw >> 2) & 0x3).into(),
            reserve0: ((raw >> 4) & 0xF) as u8,
        }
    }

    pub fn encode(&self) -> u8 {
        (self.wrap_mode as u8 & 0x3)
            | ((self.filter as u8 & 0x3) << 2)
            | ((self.reserve0 & 0xF) << 4)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialTextureExtension {
    pub is_capture_texture: bool,
    pub is_vecture_texture: bool,
    pub reserve0: u32,
}

impl MaterialTextureExtension {
    pub fn decode(raw: u32) -> Self {
        Self {
            is_capture_texture: (raw & 0x1) != 0,
            is_vecture_texture: ((raw >> 1) & 0x1) != 0,
            reserve0: ((raw >> 2) & 0x3FFFFFFF),
        }
    }

    pub fn encode(&self) -> u32 {
        (self.is_capture_texture as u32 & 0x1)
            | ((self.is_vecture_texture as u32 & 0x1) << 1)
            | ((self.reserve0 & 0x3FFFFFFF) << 2)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialTextureMap {
    #[serde(skip)]
    pub texture_index: u16,
    pub texture_name: String,
    pub u_options: MaterialTextureOptions,
    pub v_options: MaterialTextureOptions,
}

impl MaterialTextureMap {
    fn parse(c: &mut Cursor) -> Self {
        Self {
            texture_index: c.read_u16(),
            texture_name: String::new(),
            u_options: MaterialTextureOptions::decode(c.read_u8()),
            v_options: MaterialTextureOptions::decode(c.read_u8()),
        }
    }

    fn serialize(&self, w: &mut Writer) {
        w.write_u16(self.texture_index);
        w.write_u8(self.u_options.encode());
        w.write_u8(self.v_options.encode());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialTextureSrt {
    pub translation_x: f32,
    pub translation_y: f32,
    pub rotation: f32,
    pub scale_x: f32,
    pub scale_z: f32,
}

impl MaterialTextureSrt {
    fn parse(c: &mut Cursor) -> Self {
        Self {
            translation_x: c.read_f32(),
            translation_y: c.read_f32(),
            rotation: c.read_f32(),
            scale_x: c.read_f32(),
            scale_z: c.read_f32(),
        }
    }
    fn serialize(&self, w: &mut Writer) {
        w.write_f32(self.translation_x);
        w.write_f32(self.translation_y);
        w.write_f32(self.rotation);
        w.write_f32(self.scale_x);
        w.write_f32(self.scale_z);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialTexCoordGen {
    pub reserve0: u8,
    pub tex_gen_type: u8,
    pub reserve1: u16,
    pub reserve2: u32,
    pub reserve3: u64,
}

impl MaterialTexCoordGen {
    fn parse(c: &mut Cursor) -> Self {
        Self {
            reserve0: c.read_u8(),
            tex_gen_type: c.read_u8(),
            reserve1: c.read_u16(),
            reserve2: c.read_u32(),
            reserve3: {
                let lo = c.read_u32() as u64;
                let hi = c.read_u32() as u64;
                lo | (hi << 32)
            },
        }
    }
    fn serialize(&self, w: &mut Writer) {
        w.write_u8(self.reserve0);
        w.write_u8(self.tex_gen_type);
        w.write_u16(self.reserve1);
        w.write_u32(self.reserve2);
        w.write_u32((self.reserve3 & 0xFFFFFFFF) as u32);
        w.write_u32((self.reserve3 >> 32) as u32);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialTevCombiner {
    pub rgb_mode: CombinerTevMode,
    pub alpha_mode: CombinerTevMode,
    pub reserve1: u8,
    pub reserve2: u8,
}

impl MaterialTevCombiner {
    fn parse(c: &mut Cursor) -> Self {
        Self {
            rgb_mode: c.read_u8().into(),
            alpha_mode: c.read_u8().into(),
            reserve1: c.read_u8(),
            reserve2: c.read_u8(),
        }
    }
    fn serialize(&self, w: &mut Writer) {
        w.write_u8(self.rgb_mode.into());
        w.write_u8(self.alpha_mode.into());
        w.write_u8(self.reserve1);
        w.write_u8(self.reserve2);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialAlphaCompare {
    pub alpha_test_function: u8,
    pub reserve0: u8,
    pub reserve1: u16,
    pub alpha_compare_ref_value: f32,
}
impl MaterialAlphaCompare {
    fn parse(c: &mut Cursor) -> Self {
        Self {
            alpha_test_function: c.read_u8(),
            reserve0: c.read_u8(),
            reserve1: c.read_u16(),
            alpha_compare_ref_value: c.read_f32(),
        }
    }
    fn serialize(&self, w: &mut Writer) {
        w.write_u8(self.alpha_test_function);
        w.write_u8(self.reserve0);
        w.write_u16(self.reserve1);
        w.write_f32(self.alpha_compare_ref_value);
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum BlendMode {
    #[num_enum(default)]
    None,
    Blend,
    Logic,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum BlendFactor {
    #[num_enum(default)]
    V0,
    V1_0,
    DstColor,
    InvDstColor,
    SrcAlpha,
    InvSrcAlpha,
    DstAlpha,
    InvDstAlpha,
    SrcColor,
    InvSrcColor,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum LogicOp {
    #[num_enum(default)]
    Invalid,

    NoOp,
    Clear,
    Set,
    Copy,
    InvCopy,
    Inv,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Equiv,
    RevAnd,
    InvAnd,
    RevOr,
    InvOr,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum BlendOp {
    #[num_enum(default)]
    Invalid,

    Add,
    Subtract,
    ReverseSubtract,
    SelectMin,
    SelectMax,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialBlendMode {
    pub blend_op: BlendOp,
    pub function_source: BlendFactor,
    pub function_destination: BlendFactor,
    pub logic_op: LogicOp,
}

impl MaterialBlendMode {
    fn parse(c: &mut Cursor) -> Self {
        Self {
            blend_op: c.read_u8().into(),
            function_source: c.read_u8().into(),
            function_destination: c.read_u8().into(),
            logic_op: c.read_u8().into(),
        }
    }

    fn serialize(&self, w: &mut Writer) {
        w.write_u8(self.blend_op.into());
        w.write_u8(self.function_source.into());
        w.write_u8(self.function_destination.into());
        w.write_u8(self.logic_op.into());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialIndirectMatrix {
    pub translation: [f32; 2],
    pub rotation: f32,
}
impl MaterialIndirectMatrix {
    fn parse(c: &mut Cursor) -> Self {
        Self {
            translation: [c.read_f32(), c.read_f32()],
            rotation: c.read_f32(),
        }
    }
    fn serialize(&self, w: &mut Writer) {
        w.write_f32(self.translation[0]);
        w.write_f32(self.translation[1]);
        w.write_f32(self.rotation);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialProjectionTexGen {
    pub translation: [f32; 2],
    pub scale: [f32; 2],
    pub rotation: f32,
}

impl MaterialProjectionTexGen {
    fn parse(c: &mut Cursor) -> Self {
        Self {
            translation: [c.read_f32(), c.read_f32()],
            scale: [c.read_f32(), c.read_f32()],
            rotation: c.read_f32(),
        }
    }
    fn serialize(&self, w: &mut Writer) {
        for v in &self.translation {
            w.write_f32(*v);
        }
        for v in &self.scale {
            w.write_f32(*v);
        }
        w.write_f32(self.rotation);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialFontShadowColor {
    pub color0: Color4u8,
    pub color1: Color4u8,
}
impl MaterialFontShadowColor {
    fn parse(c: &mut Cursor) -> Self {
        Self {
            color0: Color4u8::parse(c),
            color1: Color4u8::parse(c),
        }
    }
    fn serialize(&self, w: &mut Writer) {
        self.color0.serialize(w);
        self.color1.serialize(w);
    }
}

// MaterialDetailedCombiner & these enums are from KillzXGaming's LayoutLibrary repository!
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, FromPrimitive, IntoPrimitive,
)]
#[repr(u8)]
pub enum TevSource {
    Primary = 0,
    Unknown2 = 2,
    Texture0 = 3,
    Texture1 = 4,
    Texture2 = 5,
    Texture3 = 6,
    Register = 13,
    #[num_enum(default)]
    Constant = 14,
    Previous = 15,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, FromPrimitive, IntoPrimitive,
)]
#[repr(u8)]
pub enum TevScale {
    #[num_enum(default)]
    V1,
    V2,
    V4,
    Unknown,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, FromPrimitive, IntoPrimitive,
)]
#[repr(u8)]
pub enum DetailedCombinerTevMode {
    #[num_enum(default)]
    Replace,
    Modulate,
    Add,
    AddSigned,
    Interpolate,
    Subtract,
    AddMultiplicate = 8,
    MultiplcateAdd,
    Overlay,
    Lighten,
    Darken,
    Indirect,
    BlendIndirect,
    EachIndirect,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, FromPrimitive, IntoPrimitive,
)]
#[repr(u8)]
pub enum CombinerTevMode {
    #[num_enum(default)]
    Replace,
    Modulate,
    Add,
    AddSigned,
    Interpolate,
    Subtract,
    AddMultiplicate,
    MultiplcateAdd,
    Overlay,
    Lighten,
    Darken,
    Indirect,
    BlendIndirect,
    EachIndirect,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, IntoPrimitive, FromPrimitive,
)]
#[repr(u8)]
pub enum TevColorOp {
    #[num_enum(default)]
    RGB,
    InvRGB,
    Alpha,
    InvAlpha,
    RRR,
    InvRRR,
    GGG,
    InvGGG,
    BBB,
    InvBBB,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, IntoPrimitive, FromPrimitive,
)]
#[repr(u8)]
pub enum TevAlphaOp {
    #[num_enum(default)]
    Alpha,
    InvAlpha,
    R,
    InvR,
    G,
    InvG,
    B,
    InvB,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedCombinerColorFlags {
    pub color_sources: [TevSource; 3],
    pub color_ops: [TevColorOp; 3],
    pub color_mode: DetailedCombinerTevMode,
    pub color_scale: TevScale,
}

impl DetailedCombinerColorFlags {
    pub fn from_u32(flags: u32) -> Self {
        let get_bits = |start: u8, len: u8| -> u8 { ((flags >> start) & ((1 << len) - 1)) as u8 };

        Self {
            color_sources: [
                get_bits(0, 4).into(),
                get_bits(4, 4).into(),
                get_bits(8, 4).into(),
            ],
            color_ops: [
                get_bits(12, 4).into(),
                get_bits(16, 4).into(),
                get_bits(20, 4).into(),
            ],
            color_mode: get_bits(24, 4).into(),
            color_scale: get_bits(28, 3).into(),
        }
    }

    pub fn to_u32(&self) -> u32 {
        let mut flags = 0u32;
        flags |= self.color_sources[0] as u32 & 0xF;
        flags |= (self.color_sources[1] as u32 & 0xF) << 4;
        flags |= (self.color_sources[2] as u32 & 0xF) << 8;
        flags |= (self.color_ops[0] as u32 & 0xF) << 12;
        flags |= (self.color_ops[1] as u32 & 0xF) << 16;
        flags |= (self.color_ops[2] as u32 & 0xF) << 20;
        flags |= (self.color_mode as u32 & 0xF) << 24;
        flags |= (self.color_scale as u32 & 0x7) << 28;
        flags
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedCombinerAlphaFlags {
    pub alpha_sources: [TevSource; 3],
    pub alpha_ops: [TevAlphaOp; 3],
    pub alpha_mode: DetailedCombinerTevMode,
    pub alpha_scale: TevScale,
}

impl DetailedCombinerAlphaFlags {
    pub fn from_i32(flags: i32) -> Self {
        let u_flags = flags as u32;
        let get_bits = |start: u8, len: u8| -> u8 { ((u_flags >> start) & ((1 << len) - 1)) as u8 };

        Self {
            alpha_sources: [
                get_bits(0, 4).into(),
                get_bits(4, 4).into(),
                get_bits(8, 4).into(),
            ],
            alpha_ops: [
                get_bits(12, 4).into(),
                get_bits(16, 4).into(),
                get_bits(20, 4).into(),
            ],
            alpha_mode: get_bits(24, 4).into(),
            alpha_scale: get_bits(28, 3).into(),
        }
    }

    pub fn to_i32(&self) -> i32 {
        let mut flags = 0u32;
        flags |= self.alpha_sources[0] as u32 & 0xF;
        flags |= (self.alpha_sources[1] as u32 & 0xF) << 4;
        flags |= (self.alpha_sources[2] as u32 & 0xF) << 8;
        flags |= (self.alpha_ops[0] as u32 & 0xF) << 12;
        flags |= (self.alpha_ops[1] as u32 & 0xF) << 16;
        flags |= (self.alpha_ops[2] as u32 & 0xF) << 20;
        flags |= (self.alpha_mode as u32 & 0xF) << 24;
        flags |= (self.alpha_scale as u32 & 0x3) << 28;
        flags as i32
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialDetailedCombiner {
    pub value: i32,

    pub color1: Color4u8,
    pub color2: Color4u8,
    pub color3: Color4u8,
    pub color4: Color4u8,
    pub color5: Color4u8,
    pub color6: Color4u8,

    pub entries: Vec<MaterialDetailedCombinerEntry>,
}

impl MaterialDetailedCombiner {
    pub fn parse(c: &mut Cursor, count: u8) -> Self {
        let mut combiner = Self {
            value: c.read_i32(),
            color1: Color4u8::parse(c),
            color2: Color4u8::parse(c),
            color3: Color4u8::parse(c),
            color4: Color4u8::parse(c),
            color5: Color4u8::parse(c),
            color6: Color4u8::parse(c),
            entries: Vec::new(),
        };

        for _ in 0..count {
            let entry = MaterialDetailedCombinerEntry::parse(c);
            combiner.entries.push(entry);
        }

        combiner
    }

    pub fn serialize(&self, w: &mut Writer) {
        w.write_i32(self.value);
        self.color1.serialize(w);
        self.color2.serialize(w);
        self.color3.serialize(w);
        self.color4.serialize(w);
        self.color5.serialize(w);
        self.color6.serialize(w);

        for entry in &self.entries {
            entry.serialize(w);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialDetailedCombinerEntry {
    pub color_flags: DetailedCombinerColorFlags,
    pub alpha_flags: DetailedCombinerAlphaFlags,

    pub unknown_1: u32,
    pub unknown_2: u32,
}

impl MaterialDetailedCombinerEntry {
    pub fn parse(c: &mut Cursor) -> Self {
        Self {
            color_flags: DetailedCombinerColorFlags::from_u32(c.read_u32()),
            alpha_flags: DetailedCombinerAlphaFlags::from_i32(c.read_i32()),

            unknown_1: c.read_u32(),
            unknown_2: c.read_u32(),
        }
    }

    pub fn serialize(&self, w: &mut Writer) {
        w.write_u32(self.color_flags.to_u32());
        w.write_i32(self.alpha_flags.to_i32());

        w.write_u32(self.unknown_1);
        w.write_u32(self.unknown_2);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialUserCombiner {
    pub name: String,
    pub reserve: [u32; 5],
}

impl MaterialUserCombiner {
    fn parse(c: &mut Cursor) -> Self {
        let name = c.read_fixed_string(0x60);
        let mut r = [0u32; 5];
        for v in &mut r {
            *v = c.read_u32();
        }
        Self { name, reserve: r }
    }
    fn serialize(&self, w: &mut Writer) {
        w.write_fixed_string(&self.name, 0x60);
        for v in &self.reserve {
            w.write_u32(*v);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialVectorTextureInfo {
    pub time: f32,
    pub color: Color4u8,
    pub reserve0: u64,
}
impl MaterialVectorTextureInfo {
    fn parse(c: &mut Cursor) -> Self {
        let time = c.read_f32();
        let color = Color4u8::parse(c);
        let lo = c.read_u32() as u64;
        let hi = c.read_u32() as u64;
        Self {
            time,
            color,
            reserve0: lo | (hi << 32),
        }
    }
    fn serialize(&self, w: &mut Writer) {
        w.write_f32(self.time);
        self.color.serialize(w);
        w.write_u32((self.reserve0 & 0xFFFFFFFF) as u32);
        w.write_u32((self.reserve0 >> 32) as u32);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialBrickRepeatShaderInfo {
    pub data: Vec<u8>,
}
impl MaterialBrickRepeatShaderInfo {
    fn parse(c: &mut Cursor) -> Self {
        let data = c.read_bytes(0x58).to_vec();
        Self { data }
    }
    fn serialize(&self, w: &mut Writer) {
        w.write_bytes(&self.data);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct MaterialInfo {
    pub tex_map_count: u8,
    pub tex_srt_count: u8,
    pub tex_coord_gen_count: u8,
    pub tev_combiner_count: u8,
    pub alpha_compare_count: u8,
    pub has_color_blend_mode: bool,
    pub reserve0: bool,
    pub has_alpha_blend_mode: bool,
    pub reserve1: u8,
    pub indirect_matrix_count: u8,
    pub projection_tex_gen_count: u8,
    pub font_shadow_color: u8,
    pub reserve2: bool,
    pub use_detailed_combiner: u8,
    pub user_combiner_count: u8,
    pub has_texture_extensions: u8,
    pub vector_texture_info_count: u8,
    pub brick_repeat_shader_info_count: u8,
    pub reserve3: u8,
}

impl MaterialInfo {
    pub fn decode(raw: u32) -> Self {
        Self {
            tex_map_count: (raw & 0x3) as u8,
            tex_srt_count: ((raw >> 2) & 0x3) as u8,
            tex_coord_gen_count: ((raw >> 4) & 0x3) as u8,
            tev_combiner_count: ((raw >> 6) & 0x7) as u8,
            alpha_compare_count: ((raw >> 9) & 0x1) as u8,
            has_color_blend_mode: ((raw >> 10) & 0x1) as u8 != 0,
            reserve0: ((raw >> 11) & 0x1) as u8 != 0,
            has_alpha_blend_mode: ((raw >> 12) & 0x1) as u8 != 0,
            reserve1: ((raw >> 13) & 0x1) as u8,
            indirect_matrix_count: ((raw >> 14) & 0x1) as u8,
            projection_tex_gen_count: ((raw >> 15) & 0x3) as u8,
            font_shadow_color: ((raw >> 17) & 0x1) as u8,
            reserve2: ((raw >> 18) & 0x1) as u8 != 0,
            use_detailed_combiner: ((raw >> 19) & 0x1) as u8,
            user_combiner_count: ((raw >> 20) & 0x1) as u8,
            has_texture_extensions: ((raw >> 21) & 0x1) as u8,
            vector_texture_info_count: ((raw >> 22) & 0x3) as u8,
            brick_repeat_shader_info_count: ((raw >> 24) & 0x3) as u8,
            reserve3: ((raw >> 26) & 0x3F) as u8,
        }
    }

    pub fn encode(&self) -> u32 {
        ((self.tex_map_count & 0x3) as u32)
            | (((self.tex_srt_count & 0x3) as u32) << 2)
            | (((self.tex_coord_gen_count & 0x3) as u32) << 4)
            | (((self.tev_combiner_count & 0x7) as u32) << 6)
            | (((self.alpha_compare_count & 0x1) as u32) << 9)
            | (((self.has_color_blend_mode as u8 & 0x1) as u32) << 10)
            | (((self.reserve0 as u8 & 0x1) as u32) << 11)
            | (((self.has_alpha_blend_mode as u8 & 0x1) as u32) << 12)
            | (((self.reserve1 & 0x1) as u32) << 13)
            | (((self.indirect_matrix_count & 0x1) as u32) << 14)
            | (((self.projection_tex_gen_count & 0x3) as u32) << 15)
            | (((self.font_shadow_color & 0x1) as u32) << 17)
            | (((self.reserve2 as u8 & 0x1) as u32) << 18)
            | (((self.use_detailed_combiner & 0x1) as u32) << 19)
            | (((self.user_combiner_count & 0x1) as u32) << 20)
            | (((self.has_texture_extensions & 0x1) as u32) << 21)
            | (((self.vector_texture_info_count & 0x3) as u32) << 22)
            | (((self.brick_repeat_shader_info_count & 0x3) as u32) << 24)
            | (((self.reserve3 & 0x3F) as u32) << 26)
    }
}

pub const MATERIAL_NAME_LEN: usize = 0x1c;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialColorEntry {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub color_u8: Option<Color4u8>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub color_f32: Option<Color4f>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BflytMaterial {
    pub material_name: String,

    pub reserve0: bool,
    pub reserve2: bool,

    pub color_types_byte: u8,
    pub colors: Vec<MaterialColorEntry>,

    pub tex_maps: Vec<MaterialTextureMap>,
    pub tex_extensions: Vec<MaterialTextureExtension>,
    pub tex_srts: Vec<MaterialTextureSrt>,
    pub tex_coord_gens: Vec<MaterialTexCoordGen>,
    pub tev_combiners: Vec<MaterialTevCombiner>,
    pub alpha_compares: Vec<MaterialAlphaCompare>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub blend_mode: Option<MaterialBlendMode>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub blend_mode_alpha: Option<MaterialBlendMode>,

    pub indirect_matrices: Vec<MaterialIndirectMatrix>,
    pub projection_tex_gens: Vec<MaterialProjectionTexGen>,
    pub font_shadow_colors: Vec<MaterialFontShadowColor>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub detailed_combiner: Option<MaterialDetailedCombiner>,

    pub user_combiners: Vec<MaterialUserCombiner>,
    pub vector_texture_infos: Vec<MaterialVectorTextureInfo>,
    pub brick_repeat_shader_infos: Vec<MaterialBrickRepeatShaderInfo>,
}

impl BflytMaterial {
    pub fn parse(cursor: &mut Cursor, mat_base: usize) -> Self {
        cursor.seek(mat_base);
        let material_name = cursor.read_fixed_string(MATERIAL_NAME_LEN);
        let material_info = MaterialInfo::decode(cursor.read_u32());

        let color_types_byte = cursor.read_u8();
        let color_count = cursor.read_u8();

        let color_data_base = mat_base + 0x20;
        let mut color_offset_bytes = Vec::new();
        for _ in 0..color_count {
            color_offset_bytes.push(cursor.read_u8());
        }

        let mut colors = Vec::new();
        for (i, &offset) in color_offset_bytes.iter().enumerate() {
            let is_float = ((color_types_byte >> i) & 1) != 0;
            let saved = cursor.pos;
            cursor.seek(color_data_base + offset as usize);
            let entry = if is_float {
                MaterialColorEntry {
                    color_u8: None,
                    color_f32: Some(Color4f::parse(cursor)),
                }
            } else {
                MaterialColorEntry {
                    color_u8: Some(Color4u8::parse(cursor)),
                    color_f32: None,
                }
            };
            colors.push(entry);
            cursor.seek(saved);
        }

        let color_section_size = {
            let mut max_end = 2 + color_count as usize;
            for (i, &offset) in color_offset_bytes.iter().enumerate() {
                let is_float = ((color_types_byte >> i) & 1) != 0;
                let end = offset as usize + if is_float { 16 } else { 4 };
                if end > max_end {
                    max_end = end;
                }
            }
            max_end
        };

        let after_color = mat_base + 0x20 + color_section_size;

        let tex_maps_base = after_color;
        cursor.seek(tex_maps_base);
        let mut tex_maps = Vec::new();
        for _ in 0..material_info.tex_map_count {
            tex_maps.push(MaterialTextureMap::parse(cursor));
        }

        let mut tex_extensions = Vec::new();
        if material_info.has_texture_extensions != 0 {
            for _ in 0..material_info.tex_map_count {
                tex_extensions.push(MaterialTextureExtension::decode(cursor.read_u32()));
            }
        }

        let mut tex_srts = Vec::new();
        for _ in 0..material_info.tex_srt_count {
            tex_srts.push(MaterialTextureSrt::parse(cursor));
        }

        let mut tex_coord_gens = Vec::new();
        for _ in 0..material_info.tex_coord_gen_count {
            tex_coord_gens.push(MaterialTexCoordGen::parse(cursor));
        }

        let mut tev_combiners = Vec::new();
        for _ in 0..material_info.tev_combiner_count {
            tev_combiners.push(MaterialTevCombiner::parse(cursor));
        }

        let mut alpha_compares = Vec::new();
        for _ in 0..material_info.alpha_compare_count {
            alpha_compares.push(MaterialAlphaCompare::parse(cursor));
        }

        let blend_mode = if material_info.has_color_blend_mode {
            Some(MaterialBlendMode::parse(cursor))
        } else {
            None
        };

        let blend_mode_alpha = if material_info.has_alpha_blend_mode {
            Some(MaterialBlendMode::parse(cursor))
        } else {
            None
        };

        let mut indirect_matrices = Vec::new();
        for _ in 0..material_info.indirect_matrix_count {
            indirect_matrices.push(MaterialIndirectMatrix::parse(cursor));
        }

        let mut projection_tex_gens = Vec::new();
        for _ in 0..material_info.projection_tex_gen_count {
            projection_tex_gens.push(MaterialProjectionTexGen::parse(cursor));
        }

        let mut font_shadow_colors = Vec::new();
        for _ in 0..material_info.font_shadow_color {
            font_shadow_colors.push(MaterialFontShadowColor::parse(cursor));
        }

        let detailed_combiner = if material_info.use_detailed_combiner != 0 {
            Some(MaterialDetailedCombiner::parse(
                cursor,
                material_info.tev_combiner_count,
            ))
        } else {
            None
        };

        let mut user_combiners = Vec::new();
        for _ in 0..material_info.user_combiner_count {
            user_combiners.push(MaterialUserCombiner::parse(cursor));
        }

        let mut vector_texture_infos = Vec::new();
        for _ in 0..material_info.vector_texture_info_count {
            vector_texture_infos.push(MaterialVectorTextureInfo::parse(cursor));
        }

        let mut brick_repeat_shader_infos = Vec::new();
        for _ in 0..material_info.brick_repeat_shader_info_count {
            brick_repeat_shader_infos.push(MaterialBrickRepeatShaderInfo::parse(cursor));
        }

        Self {
            material_name,
            color_types_byte,
            reserve0: material_info.reserve0,
            reserve2: material_info.reserve2,
            colors,
            tex_maps,
            tex_extensions,
            tex_srts,
            tex_coord_gens,
            tev_combiners,
            alpha_compares,
            blend_mode,
            blend_mode_alpha,
            indirect_matrices,
            projection_tex_gens,
            font_shadow_colors,
            detailed_combiner,
            user_combiners,
            vector_texture_infos,
            brick_repeat_shader_infos,
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_fixed_string(&self.material_name, MATERIAL_NAME_LEN);

        let material_info = MaterialInfo {
            tex_map_count: self.tex_maps.len() as u8,
            tex_srt_count: self.tex_srts.len() as u8,
            tex_coord_gen_count: self.tex_coord_gens.len() as u8,
            tev_combiner_count: self.tev_combiners.len() as u8,
            alpha_compare_count: self.alpha_compares.len() as u8,
            has_color_blend_mode: self.blend_mode.is_some(),
            reserve0: self.reserve0,
            has_alpha_blend_mode: self.blend_mode_alpha.is_some(),
            reserve1: 0,
            indirect_matrix_count: self.indirect_matrices.len() as u8,
            projection_tex_gen_count: self.projection_tex_gens.len() as u8,
            font_shadow_color: self.font_shadow_colors.len() as u8,
            reserve2: self.reserve2,
            use_detailed_combiner: self.detailed_combiner.is_some() as u8,
            user_combiner_count: self.user_combiners.len() as u8,
            has_texture_extensions: !self.tex_extensions.is_empty() as u8,
            vector_texture_info_count: self.vector_texture_infos.len() as u8,
            brick_repeat_shader_info_count: self.brick_repeat_shader_infos.len() as u8,
            reserve3: 0,
        };

        writer.write_u32(material_info.encode());

        writer.write_u8(self.color_types_byte);
        writer.write_u8(self.colors.len() as u8);

        let n = self.colors.len();
        let mut cumulative_offset = (2 + n) as u8;
        for entry in self.colors.iter() {
            writer.write_u8(cumulative_offset);
            cumulative_offset += if entry.color_u8.is_some() { 4 } else { 16 };
        }

        for entry in &self.colors {
            if let Some(c) = &entry.color_u8 {
                c.serialize(writer);
            } else if let Some(c) = &entry.color_f32 {
                c.serialize(writer);
            }
        }

        for tm in &self.tex_maps {
            tm.serialize(writer);
        }

        for ext in &self.tex_extensions {
            writer.write_u32(ext.encode());
        }

        for ts in &self.tex_srts {
            ts.serialize(writer);
        }

        for tg in &self.tex_coord_gens {
            tg.serialize(writer);
        }

        for tc in &self.tev_combiners {
            tc.serialize(writer);
        }

        for ac in &self.alpha_compares {
            ac.serialize(writer);
        }

        if let Some(blend_mode) = &self.blend_mode {
            blend_mode.serialize(writer);
        }

        if let Some(blend_mode) = &self.blend_mode_alpha {
            blend_mode.serialize(writer);
        }

        for im in &self.indirect_matrices {
            im.serialize(writer);
        }

        for pg in &self.projection_tex_gens {
            pg.serialize(writer);
        }

        for fs in &self.font_shadow_colors {
            fs.serialize(writer);
        }

        if let Some(detailed_combiner) = &self.detailed_combiner {
            detailed_combiner.serialize(writer);
        }

        for uc in &self.user_combiners {
            uc.serialize(writer);
        }

        for vi in &self.vector_texture_infos {
            vi.serialize(writer);
        }

        for br in &self.brick_repeat_shader_infos {
            br.serialize(writer);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BflytMaterialList {
    pub materials: Vec<BflytMaterial>,
}

impl BflytMaterialList {
    pub fn parse(cursor: &mut Cursor, section_start: usize) -> Self {
        let mat_list_base = section_start;

        let material_count = cursor.read_u16();
        let _reserve0 = cursor.read_u16();

        let mut offsets = Vec::new();
        for _ in 0..material_count {
            offsets.push(cursor.read_u32());
        }

        let saved = cursor.pos;
        let mut materials = Vec::new();
        for offset in offsets {
            let mat_base = mat_list_base + offset as usize;
            materials.push(BflytMaterial::parse(cursor, mat_base));
        }
        cursor.seek(saved);

        Self { materials }
    }

    pub fn serialize(&self, writer: &mut Writer, section_start: usize) {
        let mat_list_base = section_start;
        writer.write_u16(self.materials.len() as u16);
        writer.write_u16(0);

        let mut offset_placeholders = Vec::new();
        for _ in &self.materials {
            offset_placeholders.push(writer.write_placeholder_u32());
        }

        for (i, material) in self.materials.iter().enumerate() {
            writer.align(4);
            let offset = writer.pos() - mat_list_base;
            writer.patch_u32(offset_placeholders[i], offset as u32);
            material.serialize(writer);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturePaneInfo {
    pub pane_name0: String,
    pub pane_name1: String,
    pub reserve0: [u32; 6],
    pub values: [u8; 8],
    pub reserve1: f32,
    pub reserve2: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BflytCaptureTextureList {
    pub infos: Vec<CapturePaneInfo>,
}

impl BflytCaptureTextureList {
    pub fn parse(cursor: &mut Cursor, section_start: usize) -> Self {
        let ctl_base = section_start;
        let count = cursor.read_u32();

        let mut infos = Vec::new();
        for _ in 0..count {
            let pane_name_offset0 = cursor.read_u32();
            let pane_name_offset1 = cursor.read_u32();
            let mut reserve0 = [0u32; 6];
            for v in &mut reserve0 {
                *v = cursor.read_u32();
            }
            let mut values = [0u8; 8];
            for v in &mut values {
                *v = cursor.read_u8();
            }
            let reserve1 = cursor.read_f32();
            let reserve2 = cursor.read_f32();

            let saved = cursor.pos;
            cursor.seek(ctl_base + pane_name_offset0 as usize);
            let pane_name0 = cursor.read_null_terminated_string();
            cursor.seek(ctl_base + pane_name_offset1 as usize);
            let pane_name1 = cursor.read_null_terminated_string();
            cursor.seek(saved);

            infos.push(CapturePaneInfo {
                pane_name0,
                pane_name1,
                reserve0,
                values,
                reserve1,
                reserve2,
            });
        }

        Self { infos }
    }

    pub fn serialize(&self, writer: &mut Writer, section_start: usize) {
        let ctl_base = section_start;
        writer.write_u32(self.infos.len() as u32);

        let mut name0_placeholders = Vec::new();
        let mut name1_placeholders = Vec::new();

        for info in &self.infos {
            name0_placeholders.push(writer.write_placeholder_u32());
            name1_placeholders.push(writer.write_placeholder_u32());
            for v in &info.reserve0 {
                writer.write_u32(*v);
            }
            for v in &info.values {
                writer.write_u8(*v);
            }
            writer.write_f32(info.reserve1);
            writer.write_f32(info.reserve2);
        }

        for (i, info) in self.infos.iter().enumerate() {
            let off0 = writer.pos() - ctl_base;
            writer.patch_u32(name0_placeholders[i], off0 as u32);
            writer.write_null_terminated_string(&info.pane_name0);

            let off1 = writer.pos() - ctl_base;
            writer.patch_u32(name1_placeholders[i], off1 as u32);
            writer.write_null_terminated_string(&info.pane_name1);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorGraphicsInfo {
    pub reserve1: u32,
    pub reserve2: u32,
    pub reserve3: u32,
    pub bnvg_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BflytVectorGraphicsList {
    pub infos: Vec<VectorGraphicsInfo>,
}

impl BflytVectorGraphicsList {
    pub fn parse(cursor: &mut Cursor, section_start: usize) -> Self {
        let vgl_base = section_start;
        let count = cursor.read_u32();

        let mut offsets = Vec::new();
        for _ in 0..count {
            offsets.push(cursor.read_u32());
        }

        let saved = cursor.pos;
        let mut infos = Vec::new();
        for offset in offsets {
            cursor.seek(vgl_base + offset as usize);
            let reserve1 = cursor.read_u32();
            let reserve2 = cursor.read_u32();
            let reserve3 = cursor.read_u32();
            let bnvg_name = cursor.read_null_terminated_string();
            infos.push(VectorGraphicsInfo {
                reserve1,
                reserve2,
                reserve3,
                bnvg_name,
            });
        }
        cursor.seek(saved);

        Self { infos }
    }

    pub fn serialize(&self, writer: &mut Writer, section_start: usize) {
        let vgl_base = section_start;
        writer.write_u32(self.infos.len() as u32);

        let mut offset_placeholders = Vec::new();
        for _ in &self.infos {
            offset_placeholders.push(writer.write_placeholder_u32());
        }

        for (i, info) in self.infos.iter().enumerate() {
            let offset = writer.pos() - vgl_base;
            writer.patch_u32(offset_placeholders[i], offset as u32);
            writer.write_u32(info.reserve1);
            writer.write_u32(info.reserve2);
            writer.write_u32(info.reserve3);
            writer.write_null_terminated_string(&info.bnvg_name);
        }
    }
}

pub const GROUP_NAME_LEN: usize = 0x21;
pub const GROUP_PANE_NAME_LEN: usize = 0x18;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BflytGroup {
    pub group_name: String,
    pub reserve0: u8,
    pub child_names: Vec<String>,
}

impl BflytGroup {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let group_name = cursor.read_fixed_string(GROUP_NAME_LEN);
        let reserve0 = cursor.read_u8();
        let child_count = cursor.read_u16();
        let mut child_names = Vec::new();
        for _ in 0..child_count {
            child_names.push(cursor.read_fixed_string(GROUP_PANE_NAME_LEN));
        }
        Self {
            group_name,
            reserve0,
            child_names,
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_fixed_string(&self.group_name, GROUP_NAME_LEN);
        writer.write_u8(self.reserve0);
        writer.write_u16(self.child_names.len() as u16);
        for name in &self.child_names {
            writer.write_fixed_string(name, GROUP_PANE_NAME_LEN);
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BflytControlSource {
    pub control_name: String,
    pub reserve0_name: String,

    pub pane_bindings: Vec<String>,
    pub core_anims: Vec<String>,

    pub pane_names: Vec<String>,
    pub anim_names: Vec<String>,
}

impl BflytControlSource {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let section_start = cursor.pos - 8;

        let reserve0_offset = cursor.read_u32() as usize;
        let name_array_offset = cursor.read_u32() as usize;
        let pane_count = cursor.read_u16() as usize;
        let anim_count = cursor.read_u16() as usize;
        let pane_name_offset_arr = cursor.read_u32() as usize;
        let anim_name_offset_arr = cursor.read_u32() as usize;

        let control_name = cursor.read_null_terminated_string();

        cursor.seek(section_start + reserve0_offset);
        let reserve0_name = cursor.read_null_terminated_string();

        let na_base = section_start + name_array_offset;
        cursor.seek(na_base);

        let mut pane_bindings = Vec::with_capacity(pane_count);
        for _ in 0..pane_count {
            pane_bindings.push(cursor.read_fixed_string(GROUP_PANE_NAME_LEN));
        }

        let core_table_base = na_base + (pane_count * GROUP_PANE_NAME_LEN);
        cursor.seek(core_table_base);

        let mut core_offsets = Vec::with_capacity(anim_count);
        for _ in 0..anim_count {
            core_offsets.push(cursor.read_u32() as usize);
        }

        let mut core_anims = Vec::with_capacity(anim_count);
        for offset in core_offsets {
            cursor.seek(core_table_base + offset);
            core_anims.push(cursor.read_null_terminated_string());
        }

        let pane_table_base = section_start + pane_name_offset_arr;
        cursor.seek(pane_table_base);

        let mut pane_offsets = Vec::with_capacity(pane_count);
        for _ in 0..pane_count {
            pane_offsets.push(cursor.read_u32() as usize);
        }

        let mut pane_names = Vec::with_capacity(pane_count);
        for offset in pane_offsets {
            cursor.seek(pane_table_base + offset);
            pane_names.push(cursor.read_null_terminated_string());
        }

        let anim_table_base = section_start + anim_name_offset_arr;
        cursor.seek(anim_table_base);

        let mut anim_offsets = Vec::with_capacity(anim_count);
        for _ in 0..anim_count {
            anim_offsets.push(cursor.read_u32() as usize);
        }

        let mut anim_names = Vec::with_capacity(anim_count);
        for offset in anim_offsets {
            cursor.seek(anim_table_base + offset);
            anim_names.push(cursor.read_null_terminated_string());
        }

        Self {
            control_name,
            reserve0_name,
            pane_bindings,
            core_anims,
            pane_names,
            anim_names,
        }
    }

    pub fn serialize(&self, writer: &mut Writer, section_start: usize) {
        let pane_count = self.pane_names.len();
        let anim_count = self.anim_names.len();

        let reserve0_offset_pos = writer.write_placeholder_u32();
        let name_array_offset_pos = writer.write_placeholder_u32();
        writer.write_u16(pane_count as u16);
        writer.write_u16(anim_count as u16);
        let pane_name_offset_arr_pos = writer.write_placeholder_u32();
        let anim_name_offset_arr_pos = writer.write_placeholder_u32();

        writer.write_null_terminated_string(&self.control_name);
        writer.align(4);

        let reserve0_off = writer.pos() - section_start;
        writer.patch_u32(reserve0_offset_pos, reserve0_off as u32);
        writer.write_null_terminated_string(&self.reserve0_name);
        writer.align(4);

        let name_array_off = writer.pos() - section_start;
        writer.patch_u32(name_array_offset_pos, name_array_off as u32);

        for binding in &self.pane_bindings {
            writer.write_fixed_string(binding, GROUP_PANE_NAME_LEN);
        }

        let core_table_base = writer.pos();
        let mut core_phs = Vec::with_capacity(anim_count);
        for _ in 0..anim_count {
            core_phs.push(writer.write_placeholder_u32());
        }

        for (i, name) in self.core_anims.iter().enumerate() {
            let off = writer.pos() - core_table_base;
            writer.patch_u32(core_phs[i], off as u32);
            writer.write_null_terminated_string(name);
        }
        writer.align(4);

        let pane_table_base = writer.pos();
        let pane_name_off = pane_table_base - section_start;
        writer.patch_u32(pane_name_offset_arr_pos, pane_name_off as u32);

        let mut pna_phs = Vec::with_capacity(pane_count);
        for _ in 0..pane_count {
            pna_phs.push(writer.write_placeholder_u32());
        }

        for (i, name) in self.pane_names.iter().enumerate() {
            let off = writer.pos() - pane_table_base;
            writer.patch_u32(pna_phs[i], off as u32);
            writer.write_null_terminated_string(name);
        }

        writer.align(4);

        let anim_table_base = writer.pos();
        let anim_name_off = anim_table_base - section_start;
        writer.patch_u32(anim_name_offset_arr_pos, anim_name_off as u32);

        let mut ana_phs = Vec::with_capacity(anim_count);
        for _ in 0..anim_count {
            ana_phs.push(writer.write_placeholder_u32());
        }

        for (i, name) in self.anim_names.iter().enumerate() {
            let off = writer.pos() - anim_table_base;
            writer.patch_u32(ana_phs[i], off as u32);
            writer.write_null_terminated_string(name);
        }

        writer.align(4);
    }
}

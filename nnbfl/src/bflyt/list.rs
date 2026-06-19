use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};

use crate::{
    bflyt::{
        flags::{TexFilter, TexWrapMode},
        pane::Color4u8,
    },
    core::{Cursor, FormatError, Writer},
    ui2d::types::{Color4f, Vector2f},
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
    pub fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            is_centered: cursor.read_u8()? != 0,
            reserve0: cursor.read_u8()?,
            reserve1: cursor.read_u16()?,
            width: cursor.read_f32()?,
            height: cursor.read_f32()?,
            parts_width: cursor.read_f32()?,
            parts_height: cursor.read_f32()?,
            name: cursor.read_null_terminated_string()?,
        })
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
    pub fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let texture_count = cursor.read_u16()?;
        let _reserve0 = cursor.read_u16()?;

        let offsets_start = cursor.pos;
        let mut offsets = Vec::new();
        for _ in 0..texture_count {
            offsets.push(cursor.read_u32()?);
        }

        let mut textures = Vec::new();
        for offset in offsets {
            cursor.seek(offsets_start + offset as usize)?;
            textures.push(cursor.read_null_terminated_string()?);
        }

        Ok(Self { textures })
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
    pub fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let font_count = cursor.read_u16()?;
        let _reserve0 = cursor.read_u16()?;

        let offsets_start = cursor.pos;
        let mut offsets = Vec::new();
        for _ in 0..font_count {
            offsets.push(cursor.read_u32()?);
        }

        let mut fonts = Vec::new();
        for offset in offsets {
            cursor.seek(offsets_start + offset as usize)?;
            fonts.push(cursor.read_null_terminated_string()?);
        }

        Ok(Self { fonts })
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
            reserve0: (raw >> 4) & 0xF,
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
    fn parse(c: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            texture_index: c.read_u16()?,
            texture_name: String::new(),
            u_options: MaterialTextureOptions::decode(c.read_u8()?),
            v_options: MaterialTextureOptions::decode(c.read_u8()?),
        })
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
    fn parse(c: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            translation_x: c.read_f32()?,
            translation_y: c.read_f32()?,
            rotation: c.read_f32()?,
            scale_x: c.read_f32()?,
            scale_z: c.read_f32()?,
        })
    }
    fn serialize(&self, w: &mut Writer) {
        w.write_f32(self.translation_x);
        w.write_f32(self.translation_y);
        w.write_f32(self.rotation);
        w.write_f32(self.scale_x);
        w.write_f32(self.scale_z);
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum TexGenSrc {
    #[num_enum(default)]
    Tex0,
    Tex1,
    Tex2,
    OrthogonalProjection,
    PaneBasedProjection,
    PerspectiveProjection,
    PaneBasedPerspectiveProjection,
    BrickRepeat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialTexCoordGen {
    pub reserve0: u8,
    pub tex_gen_source: TexGenSrc,
    pub reserve1: u16,
    pub reserve2: u32,
    pub reserve3: u64,
}

impl MaterialTexCoordGen {
    fn parse(c: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            reserve0: c.read_u8()?,
            tex_gen_source: c.read_u8()?.into(),
            reserve1: c.read_u16()?,
            reserve2: c.read_u32()?,
            reserve3: {
                let lo = c.read_u32()? as u64;
                let hi = c.read_u32()? as u64;
                lo | (hi << 32)
            },
        })
    }
    fn serialize(&self, w: &mut Writer) {
        w.write_u8(self.reserve0);
        w.write_u8(self.tex_gen_source.into());
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
    fn parse(c: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            rgb_mode: c.read_u8()?.into(),
            alpha_mode: c.read_u8()?.into(),
            reserve1: c.read_u8()?,
            reserve2: c.read_u8()?,
        })
    }
    fn serialize(&self, w: &mut Writer) {
        w.write_u8(self.rgb_mode.into());
        w.write_u8(self.alpha_mode.into());
        w.write_u8(self.reserve1);
        w.write_u8(self.reserve2);
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum AlphaCompare {
    Never,
    Less,
    LEqual,
    Equal,
    NEqual,
    GEqual,
    Greater,
    #[num_enum(default)]
    Always,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialAlphaCompare {
    pub compare: AlphaCompare,
    pub reserve0: u8,
    pub reserve1: u16,
    pub alpha_compare_ref_value: f32,
}

impl MaterialAlphaCompare {
    fn parse(c: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            compare: c.read_u8()?.into(),
            reserve0: c.read_u8()?,
            reserve1: c.read_u16()?,
            alpha_compare_ref_value: c.read_f32()?,
        })
    }

    fn serialize(&self, w: &mut Writer) {
        w.write_u8(self.compare.into());
        w.write_u8(self.reserve0);
        w.write_u16(self.reserve1);
        w.write_f32(self.alpha_compare_ref_value);
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, FromPrimitive, IntoPrimitive)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, FromPrimitive, IntoPrimitive)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, FromPrimitive, IntoPrimitive)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MaterialBlendMode {
    None,
    Blend {
        blend_op: BlendOp,
        function_source: BlendFactor,
        function_destination: BlendFactor,
    },
    Logic {
        logic_op: LogicOp,
    },
}

impl MaterialBlendMode {
    pub fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let blend_op_raw = cursor.read_u8()?;
        let src_factor_raw = cursor.read_u8()?;
        let dst_factor_raw = cursor.read_u8()?;
        let logic_op_raw = cursor.read_u8()?;

        let out = if logic_op_raw != 0 && blend_op_raw == 0 {
            Self::Logic {
                logic_op: logic_op_raw.into(),
            }
        } else if blend_op_raw != 0 {
            Self::Blend {
                blend_op: blend_op_raw.into(),
                function_source: src_factor_raw.into(),
                function_destination: dst_factor_raw.into(),
            }
        } else {
            Self::None
        };

        Ok(out)
    }

    pub fn serialize(&self, w: &mut Writer) {
        match self {
            Self::None => {
                w.write_bytes(&[0, 0, 0, 0]);
            }
            Self::Blend {
                blend_op,
                function_source,
                function_destination,
            } => {
                w.write_u8((*blend_op).into());
                w.write_u8((*function_source).into());
                w.write_u8((*function_destination).into());
                w.write_u8(0);
            }
            Self::Logic { logic_op } => {
                w.write_bytes(&[0, 0, 0]);
                w.write_u8((*logic_op).into());
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialIndirectMatrix {
    pub rotation: f32,
    pub scale: Vector2f,
}

impl MaterialIndirectMatrix {
    fn parse(c: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            rotation: c.read_f32()?,
            scale: Vector2f::parse(c)?,
        })
    }

    fn serialize(&self, w: &mut Writer) {
        w.write_f32(self.rotation);
        self.scale.serialize(w);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialProjectionTexGen {
    pub translation: [f32; 2],
    pub scale: [f32; 2],
    pub rotation: f32,
}

impl MaterialProjectionTexGen {
    fn parse(c: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            translation: [c.read_f32()?, c.read_f32()?],
            scale: [c.read_f32()?, c.read_f32()?],
            rotation: c.read_f32()?,
        })
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
    fn parse(c: &mut Cursor) -> Result<Self, FormatError> {
        Ok(Self {
            color0: Color4u8::parse(c)?,
            color1: Color4u8::parse(c)?,
        })
    }

    fn serialize(&self, w: &mut Writer) {
        self.color0.serialize(w);
        self.color1.serialize(w);
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, FromPrimitive, IntoPrimitive, Hash,
)]
#[repr(u8)]
pub enum TevSource {
    Primary = 0,
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
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, FromPrimitive, IntoPrimitive, Hash,
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
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, IntoPrimitive, FromPrimitive,
)]
#[repr(u8)]
pub enum TevKonstSel {
    BlackColor,
    #[num_enum(default)]
    WhiteColor,
    K0,
    K1,
    K2,
    K3,
    K4,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, IntoPrimitive, FromPrimitive,
)]
#[repr(u8)]
pub enum DetailedCombinerStageMode {
    #[num_enum(default)]
    Replace,
    Modulate,
    Add,
    AddSigned,
    Interpolate,
    Subtract,
    AddMult = 8,
    MultiplicateAdd,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DetailedCombinerColorStageConfig {
    pub sources: [TevSource; 3],
    pub operands: [TevColorOp; 3],
    pub mode: DetailedCombinerStageMode,
    pub scale: TevScale,
    pub copy_reg: bool,
    pub konst_sel: TevKonstSel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DetailedCombinerAlphaStageConfig {
    pub sources: [TevSource; 3],
    pub operands: [TevAlphaOp; 3],
    pub mode: DetailedCombinerStageMode,
    pub scale: TevScale,
    pub copy_reg: bool,
    pub konst_sel: TevKonstSel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialDetailedCombinerEntry {
    pub color_config: DetailedCombinerColorStageConfig,
    pub alpha_config: DetailedCombinerAlphaStageConfig,
    pub reserve0: u32,
}

impl MaterialDetailedCombinerEntry {
    pub fn parse(c: &mut Cursor) -> Result<Self, FormatError> {
        let color_flags = c.read_u32()?;
        let alpha_flags = c.read_u32()?;
        let constant_selectors = c.read_u32()?;
        let reserve0 = c.read_u32()?;

        let color_config = DetailedCombinerColorStageConfig {
            sources: [
                ((color_flags & 0xF) as u8).into(),
                (((color_flags >> 4) & 0xF) as u8).into(),
                (((color_flags >> 8) & 0xF) as u8).into(),
            ],
            operands: [
                (((color_flags >> 12) & 0xF) as u8).into(),
                (((color_flags >> 16) & 0xF) as u8).into(),
                (((color_flags >> 20) & 0xF) as u8).into(),
            ],
            mode: (((color_flags >> 24) & 0xF) as u8).into(),
            scale: (((color_flags >> 28) & 0x3) as u8).into(),
            copy_reg: ((color_flags >> 30) & 0x1) as u8 != 0,
            konst_sel: ((constant_selectors & 0xF) as u8).into(),
        };

        let alpha_config = DetailedCombinerAlphaStageConfig {
            sources: [
                ((alpha_flags & 0xF) as u8).into(),
                (((alpha_flags >> 4) & 0xF) as u8).into(),
                (((alpha_flags >> 8) & 0xF) as u8).into(),
            ],
            operands: [
                (((alpha_flags >> 12) & 0xF) as u8).into(),
                (((alpha_flags >> 16) & 0xF) as u8).into(),
                (((alpha_flags >> 20) & 0xF) as u8).into(),
            ],
            mode: (((alpha_flags >> 24) & 0xF) as u8).into(),
            scale: (((alpha_flags >> 28) & 0x3) as u8).into(),
            copy_reg: ((alpha_flags >> 30) & 0x1) as u8 != 0,
            konst_sel: ((constant_selectors & 0xF) as u8).into(),
        };

        Ok(Self {
            color_config,
            alpha_config,
            reserve0,
        })
    }

    pub fn serialize(&self, w: &mut Writer) {
        let mut color_flags = 0u32;
        color_flags |= self.color_config.sources[0] as u32 & 0xF;
        color_flags |= (self.color_config.sources[1] as u32 & 0xF) << 4;
        color_flags |= (self.color_config.sources[2] as u32 & 0xF) << 8;
        color_flags |= (self.color_config.operands[0] as u32 & 0xF) << 12;
        color_flags |= (self.color_config.operands[1] as u32 & 0xF) << 16;
        color_flags |= (self.color_config.operands[2] as u32 & 0xF) << 20;
        color_flags |= (self.color_config.mode as u32 & 0xF) << 24;
        color_flags |= (self.color_config.scale as u32 & 0x3) << 28;
        color_flags |= (self.color_config.copy_reg as u32 & 0x1) << 30;

        let mut alpha_flags = 0u32;
        alpha_flags |= self.alpha_config.sources[0] as u32 & 0xF;
        alpha_flags |= (self.alpha_config.sources[1] as u32 & 0xF) << 4;
        alpha_flags |= (self.alpha_config.sources[2] as u32 & 0xF) << 8;
        alpha_flags |= (self.alpha_config.operands[0] as u32 & 0xF) << 12;
        alpha_flags |= (self.alpha_config.operands[1] as u32 & 0xF) << 16;
        alpha_flags |= (self.alpha_config.operands[2] as u32 & 0xF) << 20;
        alpha_flags |= (self.alpha_config.mode as u32 & 0xF) << 24;
        alpha_flags |= (self.alpha_config.scale as u32 & 0x3) << 28;
        alpha_flags |= (self.alpha_config.copy_reg as u32 & 0x1) << 30;

        let mut constant_selectors = 0u32;
        constant_selectors |= self.color_config.konst_sel as u32 & 0xF;
        constant_selectors |= (self.alpha_config.konst_sel as u32 & 0xF) << 4;

        w.write_u32(color_flags);
        w.write_u32(alpha_flags);
        w.write_u32(constant_selectors);
        w.write_u32(self.reserve0);
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
    pub stage_flags: u32,

    pub entries: Vec<MaterialDetailedCombinerEntry>,
}

impl MaterialDetailedCombiner {
    pub fn parse(c: &mut Cursor, count: u8) -> Result<Self, FormatError> {
        let mut combiner = Self {
            value: c.read_i32()?,
            color1: Color4u8::parse(c)?,
            color2: Color4u8::parse(c)?,
            color3: Color4u8::parse(c)?,
            color4: Color4u8::parse(c)?,
            color5: Color4u8::parse(c)?,
            stage_flags: c.read_u32()?,
            entries: Vec::new(),
        };

        for _ in 0..count {
            let entry = MaterialDetailedCombinerEntry::parse(c)?;
            combiner.entries.push(entry);
        }

        Ok(combiner)
    }

    pub fn serialize(&self, w: &mut Writer) {
        w.write_i32(self.value);
        self.color1.serialize(w);
        self.color2.serialize(w);
        self.color3.serialize(w);
        self.color4.serialize(w);
        self.color5.serialize(w);
        w.write_u32(self.stage_flags);

        for entry in &self.entries {
            entry.serialize(w);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialUserCombiner {
    pub name: String,
    pub reserve: [u32; 5],
}

impl MaterialUserCombiner {
    fn parse(c: &mut Cursor) -> Result<Self, FormatError> {
        let name = c.read_fixed_string(0x60)?;
        let mut reserve = [0u32; 5];

        for val in &mut reserve {
            *val = c.read_u32()?;
        }

        Ok(Self { name, reserve })
    }

    fn serialize(&self, w: &mut Writer) {
        w.write_fixed_string(&self.name, 0x60);

        for val in &self.reserve {
            w.write_u32(*val);
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
    fn parse(c: &mut Cursor) -> Result<Self, FormatError> {
        let time = c.read_f32()?;
        let color = Color4u8::parse(c)?;
        let lo = c.read_u32()? as u64;
        let hi = c.read_u32()? as u64;

        Ok(Self {
            time,
            color,
            reserve0: lo | (hi << 32),
        })
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
    fn parse(c: &mut Cursor) -> Result<Self, FormatError> {
        let data = c.read_bytes(0x58)?.to_vec();

        Ok(Self { data })
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
    pub has_alpha_compare: bool,
    pub has_color_blend_mode: bool,
    pub reserve0: bool,
    pub has_alpha_blend_mode: bool,
    pub reserve1: u8,
    pub has_indirect_matrix: bool,
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
            has_alpha_compare: ((raw >> 9) & 0x1) as u8 != 0,
            has_color_blend_mode: ((raw >> 10) & 0x1) as u8 != 0,
            reserve0: ((raw >> 11) & 0x1) as u8 != 0,
            has_alpha_blend_mode: ((raw >> 12) & 0x1) as u8 != 0,
            reserve1: ((raw >> 13) & 0x1) as u8,
            has_indirect_matrix: ((raw >> 14) & 0x1) as u8 != 0,
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
            | (((self.has_alpha_compare as u8 & 0x1) as u32) << 9)
            | (((self.has_color_blend_mode as u8 & 0x1) as u32) << 10)
            | (((self.reserve0 as u8 & 0x1) as u32) << 11)
            | (((self.has_alpha_blend_mode as u8 & 0x1) as u32) << 12)
            | (((self.reserve1 & 0x1) as u32) << 13)
            | (((self.has_indirect_matrix as u8 & 0x1) as u32) << 14)
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
    pub alpha_compare: Option<MaterialAlphaCompare>,

    pub blend_mode: Option<MaterialBlendMode>,

    pub blend_mode_alpha: Option<MaterialBlendMode>,

    pub indirect_matrix: Option<MaterialIndirectMatrix>,
    pub projection_tex_gens: Vec<MaterialProjectionTexGen>,
    pub font_shadow_colors: Vec<MaterialFontShadowColor>,

    pub detailed_combiner: Option<MaterialDetailedCombiner>,

    pub user_combiners: Vec<MaterialUserCombiner>,
    pub vector_texture_infos: Vec<MaterialVectorTextureInfo>,
    pub brick_repeat_shader_infos: Vec<MaterialBrickRepeatShaderInfo>,
}

impl BflytMaterial {
    pub fn parse(cursor: &mut Cursor, mat_base: usize) -> Result<Self, FormatError> {
        cursor.seek(mat_base)?;
        let material_name = cursor.read_fixed_string(MATERIAL_NAME_LEN)?;
        let material_info = MaterialInfo::decode(cursor.read_u32()?);

        let color_types_byte = cursor.read_u8()?;
        let color_count = cursor.read_u8()?;

        let color_data_base = mat_base + 0x20;
        let mut color_offset_bytes = Vec::new();
        for _ in 0..color_count {
            color_offset_bytes.push(cursor.read_u8()?);
        }

        let mut colors = Vec::new();
        for (i, &offset) in color_offset_bytes.iter().enumerate() {
            let is_float = ((color_types_byte >> i) & 1) != 0;
            let saved = cursor.pos;
            cursor.seek(color_data_base + offset as usize)?;

            let entry = if is_float {
                MaterialColorEntry {
                    color_u8: None,
                    color_f32: Some(Color4f::parse(cursor)?),
                }
            } else {
                MaterialColorEntry {
                    color_u8: Some(Color4u8::parse(cursor)?),
                    color_f32: None,
                }
            };

            colors.push(entry);
            cursor.seek(saved)?;
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
        cursor.seek(tex_maps_base)?;

        let mut tex_maps = Vec::new();
        for _ in 0..material_info.tex_map_count {
            tex_maps.push(MaterialTextureMap::parse(cursor)?);
        }

        let mut tex_extensions = Vec::new();
        if material_info.has_texture_extensions != 0 {
            for _ in 0..material_info.tex_map_count {
                tex_extensions.push(MaterialTextureExtension::decode(cursor.read_u32()?));
            }
        }

        let mut tex_srts = Vec::new();
        for _ in 0..material_info.tex_srt_count {
            tex_srts.push(MaterialTextureSrt::parse(cursor)?);
        }

        let mut tex_coord_gens = Vec::new();
        for _ in 0..material_info.tex_coord_gen_count {
            tex_coord_gens.push(MaterialTexCoordGen::parse(cursor)?);
        }

        let mut tev_combiners = Vec::new();
        for _ in 0..material_info.tev_combiner_count {
            tev_combiners.push(MaterialTevCombiner::parse(cursor)?);
        }

        let alpha_compare = if material_info.has_alpha_compare {
            Some(MaterialAlphaCompare::parse(cursor)?)
        } else {
            None
        };

        let blend_mode = if material_info.has_color_blend_mode {
            Some(MaterialBlendMode::parse(cursor)?)
        } else {
            None
        };

        let blend_mode_alpha = if material_info.has_alpha_blend_mode {
            Some(MaterialBlendMode::parse(cursor)?)
        } else {
            None
        };

        let indirect_matrix = if material_info.has_indirect_matrix {
            Some(MaterialIndirectMatrix::parse(cursor)?)
        } else {
            None
        };

        let detailed_combiner = if material_info.use_detailed_combiner != 0 {
            Some(MaterialDetailedCombiner::parse(
                cursor,
                material_info.tev_combiner_count,
            )?)
        } else {
            None
        };

        let mut projection_tex_gens = Vec::new();
        for _ in 0..material_info.projection_tex_gen_count {
            projection_tex_gens.push(MaterialProjectionTexGen::parse(cursor)?);
        }

        let mut font_shadow_colors = Vec::new();
        for _ in 0..material_info.font_shadow_color {
            font_shadow_colors.push(MaterialFontShadowColor::parse(cursor)?);
        }

        let mut user_combiners = Vec::new();
        for _ in 0..material_info.user_combiner_count {
            user_combiners.push(MaterialUserCombiner::parse(cursor)?);
        }

        let mut vector_texture_infos = Vec::new();
        for _ in 0..material_info.vector_texture_info_count {
            vector_texture_infos.push(MaterialVectorTextureInfo::parse(cursor)?);
        }

        let mut brick_repeat_shader_infos = Vec::new();
        for _ in 0..material_info.brick_repeat_shader_info_count {
            brick_repeat_shader_infos.push(MaterialBrickRepeatShaderInfo::parse(cursor)?);
        }

        Ok(Self {
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
            alpha_compare,
            blend_mode,
            blend_mode_alpha,
            indirect_matrix,
            projection_tex_gens,
            font_shadow_colors,
            detailed_combiner,
            user_combiners,
            vector_texture_infos,
            brick_repeat_shader_infos,
        })
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.write_fixed_string(&self.material_name, MATERIAL_NAME_LEN);

        let material_info = MaterialInfo {
            tex_map_count: self.tex_maps.len() as u8,
            tex_srt_count: self.tex_srts.len() as u8,
            tex_coord_gen_count: self.tex_coord_gens.len() as u8,
            tev_combiner_count: self.tev_combiners.len() as u8,
            has_alpha_compare: self.alpha_compare.is_some(),
            has_color_blend_mode: self.blend_mode.is_some(),
            reserve0: self.reserve0,
            has_alpha_blend_mode: self.blend_mode_alpha.is_some(),
            reserve1: 0,
            has_indirect_matrix: self.indirect_matrix.is_some(),
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

        if let Some(alpha_compare) = &self.alpha_compare {
            alpha_compare.serialize(writer);
        }

        if let Some(blend_mode) = &self.blend_mode {
            blend_mode.serialize(writer);
        }

        if let Some(blend_mode) = &self.blend_mode_alpha {
            blend_mode.serialize(writer);
        }

        if let Some(indirect_matrix) = &self.indirect_matrix {
            indirect_matrix.serialize(writer);
        }

        if let Some(detailed_combiner) = &self.detailed_combiner {
            detailed_combiner.serialize(writer);
        }

        for pg in &self.projection_tex_gens {
            pg.serialize(writer);
        }

        for fs in &self.font_shadow_colors {
            fs.serialize(writer);
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
    pub fn parse(cursor: &mut Cursor, section_start: usize) -> Result<Self, FormatError> {
        let mat_list_base = section_start;

        let material_count = cursor.read_u16()?;
        let _reserve0 = cursor.read_u16()?;

        let mut offsets = Vec::new();
        for _ in 0..material_count {
            offsets.push(cursor.read_u32()?);
        }

        let saved = cursor.pos;
        let mut materials = Vec::new();

        for offset in offsets {
            let mat_base = mat_list_base + offset as usize;
            materials.push(BflytMaterial::parse(cursor, mat_base)?);
        }

        cursor.seek(saved)?;

        Ok(Self { materials })
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
    pub fn parse(cursor: &mut Cursor, section_start: usize) -> Result<Self, FormatError> {
        let ctl_base = section_start;
        let count = cursor.read_u32()?;

        let mut infos = Vec::new();
        for _ in 0..count {
            let pane_name_offset0 = cursor.read_u32()?;
            let pane_name_offset1 = cursor.read_u32()?;
            let mut reserve0 = [0u32; 6];

            for val in &mut reserve0 {
                *val = cursor.read_u32()?;
            }

            let mut values = [0u8; 8];
            for val in &mut values {
                *val = cursor.read_u8()?;
            }

            let reserve1 = cursor.read_f32()?;
            let reserve2 = cursor.read_f32()?;

            let saved = cursor.pos;
            cursor.seek(ctl_base + pane_name_offset0 as usize)?;
            let pane_name0 = cursor.read_null_terminated_string()?;
            cursor.seek(ctl_base + pane_name_offset1 as usize)?;
            let pane_name1 = cursor.read_null_terminated_string()?;
            cursor.seek(saved)?;

            infos.push(CapturePaneInfo {
                pane_name0,
                pane_name1,
                reserve0,
                values,
                reserve1,
                reserve2,
            });
        }

        Ok(Self { infos })
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
    pub fn parse(cursor: &mut Cursor, section_start: usize) -> Result<Self, FormatError> {
        let vgl_base = section_start;
        let count = cursor.read_u32()?;

        let mut offsets = Vec::new();
        for _ in 0..count {
            offsets.push(cursor.read_u32()?);
        }

        let saved = cursor.pos;
        let mut infos = Vec::new();
        for offset in offsets {
            cursor.seek(vgl_base + offset as usize)?;
            let reserve1 = cursor.read_u32()?;
            let reserve2 = cursor.read_u32()?;
            let reserve3 = cursor.read_u32()?;
            let bnvg_name = cursor.read_null_terminated_string()?;
            infos.push(VectorGraphicsInfo {
                reserve1,
                reserve2,
                reserve3,
                bnvg_name,
            });
        }

        cursor.seek(saved)?;

        Ok(Self { infos })
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
    pub fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let group_name = cursor.read_fixed_string(GROUP_NAME_LEN)?;
        let reserve0 = cursor.read_u8()?;
        let child_count = cursor.read_u16()?;
        let mut child_names = Vec::new();

        for _ in 0..child_count {
            child_names.push(cursor.read_fixed_string(GROUP_PANE_NAME_LEN)?);
        }

        Ok(Self {
            group_name,
            reserve0,
            child_names,
        })
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
    pub fn parse(cursor: &mut Cursor) -> Result<Self, FormatError> {
        let section_start = cursor.pos - 8;

        let reserve0_offset = cursor.read_u32()? as usize;
        let name_array_offset = cursor.read_u32()? as usize;
        let pane_count = cursor.read_u16()? as usize;
        let anim_count = cursor.read_u16()? as usize;
        let pane_name_offset_arr = cursor.read_u32()? as usize;
        let anim_name_offset_arr = cursor.read_u32()? as usize;

        let control_name = cursor.read_null_terminated_string()?;

        cursor.seek(section_start + reserve0_offset)?;
        let reserve0_name = cursor.read_null_terminated_string()?;

        let na_base = section_start + name_array_offset;
        cursor.seek(na_base)?;

        let mut pane_bindings = Vec::with_capacity(pane_count);
        for _ in 0..pane_count {
            pane_bindings.push(cursor.read_fixed_string(GROUP_PANE_NAME_LEN)?);
        }

        let core_table_base = na_base + (pane_count * GROUP_PANE_NAME_LEN);
        cursor.seek(core_table_base)?;

        let mut core_offsets = Vec::with_capacity(anim_count);
        for _ in 0..anim_count {
            core_offsets.push(cursor.read_u32()? as usize);
        }

        let mut core_anims = Vec::with_capacity(anim_count);
        for offset in core_offsets {
            cursor.seek(core_table_base + offset)?;
            core_anims.push(cursor.read_null_terminated_string()?);
        }

        let pane_table_base = section_start + pane_name_offset_arr;
        cursor.seek(pane_table_base)?;

        let mut pane_offsets = Vec::with_capacity(pane_count);
        for _ in 0..pane_count {
            pane_offsets.push(cursor.read_u32()? as usize);
        }

        let mut pane_names = Vec::with_capacity(pane_count);
        for offset in pane_offsets {
            cursor.seek(pane_table_base + offset)?;
            pane_names.push(cursor.read_null_terminated_string()?);
        }

        let anim_table_base = section_start + anim_name_offset_arr;
        cursor.seek(anim_table_base)?;

        let mut anim_offsets = Vec::with_capacity(anim_count);
        for _ in 0..anim_count {
            anim_offsets.push(cursor.read_u32()? as usize);
        }

        let mut anim_names = Vec::with_capacity(anim_count);
        for offset in anim_offsets {
            cursor.seek(anim_table_base + offset)?;
            anim_names.push(cursor.read_null_terminated_string()?);
        }

        Ok(Self {
            control_name,
            reserve0_name,
            pane_bindings,
            core_anims,
            pane_names,
            anim_names,
        })
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

use serde::{Deserialize, Serialize};

use crate::{bflan_writer::Writer, tchar_code32};

#[derive(Debug, Serialize, Deserialize)]
#[repr(u32)]
pub enum AnimInfoType {
    Invalid = 0,
    PerCharacterTransformCurveAnim = tchar_code32(b"FLCC"),
    ExtendedUserDataAnim = tchar_code32(b"FLEU"),

    PerCharacterTransformAnim = tchar_code32(b"FLCT"),
    PaneSrtAnim = tchar_code32(b"FLPA"),
    VertexColorAnim = tchar_code32(b"FLVC"),
    VisibilityAnim = tchar_code32(b"FLVI"),
    DropShadowAnim = tchar_code32(b"FLDS"),
    MaskTextureAnim = tchar_code32(b"FLMT"),
    ProceduralShapeAnim = tchar_code32(b"FLPS"),
    WindowAnim = tchar_code32(b"FLWN"),
    StateMachineAnim = tchar_code32(b"FSMA"),

    AlphaCompareAnim = tchar_code32(b"FLAC"),
    FontShadowAnim = tchar_code32(b"FLFS"),
    IndirectSrtAnim = tchar_code32(b"FLIM"),
    MaterialColorAnim = tchar_code32(b"FLMC"),
    TextureSrtAnim = tchar_code32(b"FLTS"),
    TexturePatternAnim = tchar_code32(b"FLTP"),
    BrickRepeatAnim = tchar_code32(b"FTBR"),
    VectorGraphicsAnim = tchar_code32(b"FVGA"),
}

impl From<u32> for AnimInfoType {
    fn from(v: u32) -> Self {
        match v {
            x if x == tchar_code32(b"FLCC") => Self::PerCharacterTransformCurveAnim,
            x if x == tchar_code32(b"FLEU") => Self::ExtendedUserDataAnim,
            x if x == tchar_code32(b"FLCT") => Self::PerCharacterTransformAnim,
            x if x == tchar_code32(b"FLPA") => Self::PaneSrtAnim,
            x if x == tchar_code32(b"FLVC") => Self::VertexColorAnim,
            x if x == tchar_code32(b"FLVI") => Self::VisibilityAnim,
            x if x == tchar_code32(b"FLDS") => Self::DropShadowAnim,
            x if x == tchar_code32(b"FLMT") => Self::MaskTextureAnim,
            x if x == tchar_code32(b"FLPS") => Self::ProceduralShapeAnim,
            x if x == tchar_code32(b"FLWN") => Self::WindowAnim,
            x if x == tchar_code32(b"FSMA") => Self::StateMachineAnim,
            x if x == tchar_code32(b"FLAC") => Self::AlphaCompareAnim,
            x if x == tchar_code32(b"FLFS") => Self::FontShadowAnim,
            x if x == tchar_code32(b"FLIM") => Self::IndirectSrtAnim,
            x if x == tchar_code32(b"FLMC") => Self::MaterialColorAnim,
            x if x == tchar_code32(b"FLTS") => Self::TextureSrtAnim,
            x if x == tchar_code32(b"FLTP") => Self::TexturePatternAnim,
            x if x == tchar_code32(b"FTBR") => Self::BrickRepeatAnim,
            x if x == tchar_code32(b"FVGA") => Self::VectorGraphicsAnim,
            _ => Self::Invalid,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[repr(u32)]
pub enum SectionType {
    Other = 0,
    UserData = tchar_code32(b"usd1"),
    PaneAnimInfo = tchar_code32(b"pai1"),
    PaneAnimShare = tchar_code32(b"pah1"),
    PaneAnimTag = tchar_code32(b"pat1"),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Color4f {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl From<u32> for SectionType {
    fn from(v: u32) -> Self {
        match v {
            x if x == tchar_code32(b"pai1") => SectionType::PaneAnimInfo,
            x if x == tchar_code32(b"pah1") => SectionType::PaneAnimShare,
            x if x == tchar_code32(b"pat1") => SectionType::PaneAnimTag,
            _ => SectionType::Other,
        }
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

pub struct Cursor<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Cursor<'a> {
    fn read<T: Copy>(&mut self) -> T {
        let size = std::mem::size_of::<T>();
        let end = self.pos + size;
        let bytes = &self.data[self.pos..end];
        self.pos = end;

        unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const T) }
    }

    fn read_u32(&mut self) -> u32 {
        u32::from_le(self.read::<u32>())
    }

    fn read_i32(&mut self) -> i32 {
        i32::from_le(self.read::<i32>())
    }

    fn read_u16(&mut self) -> u16 {
        u16::from_le(self.read::<u16>())
    }

    fn read_u8(&mut self) -> u8 {
        u8::from_le(self.read::<u8>())
    }

    fn read_f32(&mut self) -> f32 {
        let bytes = self.read_bytes(4);
        let arr: [u8; 4] = bytes.try_into().unwrap();
        f32::from_le_bytes(arr)
    }

    fn read_string(&mut self, len: usize) -> String {
        let bytes = self.read_bytes(len);
        String::from_utf8_lossy(bytes).into_owned()
    }

    fn read_fixed_string(&mut self, len: usize) -> String {
        let bytes = self.read_bytes(len);
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(len);
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }

    fn read_null_terminated_string(&mut self) -> String {
        let start = self.pos;
        let mut end = start;

        while end < self.data.len() && self.data[end] != 0 {
            end += 1;
        }

        let bytes = &self.data[start..end];

        self.pos = if end < self.data.len() { end + 1 } else { end };

        String::from_utf8_lossy(bytes).into_owned()
    }

    fn read_bytes(&mut self, len: usize) -> &[u8] {
        let start = self.pos;
        self.pos += len;
        &self.data[start..start + len]
    }

    fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }

    fn seek_relative(&mut self, pos: usize) {
        self.pos += pos;
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BflanFile {
    pub header: BflanHeader,
    pub sections: Vec<Sections>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StepKey {
    pub frame: f32,
    pub value: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HermiteKey {
    pub frame: f32,
    pub value: f32,
    pub slope: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Curve {
    Constant(Vec<f32>),
    Step(Vec<StepKey>),
    Hermite(Vec<HermiteKey>),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TargetIndex {
    PerCharacterTransformCurve(PerCharacterTransformCurveTarget),
    PerCharacterTransform(PerCharacterTransformTarget),
    PaneSrt(PaneSrtTarget),
    VertexColor(VertexColorTarget),
    Visibility,
    MaterialColor(MaterialColorTarget),
    TextureSrt(TextureSrtTarget),
    TexturePattern(TexturePatternTarget),
    AlphaCompare,
    FontShadow,
    Raw(u8),
}

impl TargetIndex {
    pub fn to_raw(&self) -> u8 {
        match self {
            TargetIndex::PaneSrt(PaneSrtTarget::TranslateX) => 0,
            TargetIndex::PaneSrt(PaneSrtTarget::TranslateY) => 1,
            TargetIndex::PaneSrt(PaneSrtTarget::TranslateZ) => 2,
            TargetIndex::PaneSrt(PaneSrtTarget::RotateX) => 3,
            TargetIndex::PaneSrt(PaneSrtTarget::RotateY) => 4,
            TargetIndex::PaneSrt(PaneSrtTarget::RotateZ) => 5,
            TargetIndex::PaneSrt(PaneSrtTarget::ScaleX) => 6,
            TargetIndex::PaneSrt(PaneSrtTarget::ScaleY) => 7,
            TargetIndex::PaneSrt(PaneSrtTarget::SizeX) => 8,
            TargetIndex::PaneSrt(PaneSrtTarget::SizeY) => 9,
            TargetIndex::Raw(r) => *r,
            _ => 0,
        }
    }

    pub fn resolve(magic: u32, raw: u8) -> Self {
        match magic {
            m if m == tchar_code32(b"FLPA") => match raw {
                0 => Self::PaneSrt(PaneSrtTarget::TranslateX),
                1 => Self::PaneSrt(PaneSrtTarget::TranslateY),
                2 => Self::PaneSrt(PaneSrtTarget::TranslateZ),
                3 => Self::PaneSrt(PaneSrtTarget::RotateX),
                4 => Self::PaneSrt(PaneSrtTarget::RotateY),
                5 => Self::PaneSrt(PaneSrtTarget::RotateZ),
                6 => Self::PaneSrt(PaneSrtTarget::ScaleX),
                7 => Self::PaneSrt(PaneSrtTarget::ScaleY),
                8 => Self::PaneSrt(PaneSrtTarget::SizeX),
                9 => Self::PaneSrt(PaneSrtTarget::SizeY),
                _ => Self::Raw(raw),
            },

            _ => Self::Raw(raw),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PerCharacterTransformTarget {
    EvalTypeOffset,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TexturePatternTarget {
    Image = 0,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PerCharacterTransformCurveTarget {
    TranslateX = 0,
    TranslateY = 1,
    TranslateZ = 2,
    RotateX = 3,
    RotateY = 4,
    RotateZ = 5,
    LeftTopRed = 6,
    LeftTopGreen = 7,
    LeftTopBlue = 8,
    LeftTopAlpha = 9,
    LeftBottomRed = 10,
    LeftBottomGreen = 11,
    LeftBottomBlue = 12,
    LeftBottomAlpha = 13,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum VertexColorTarget {
    LeftTopRed = 0,
    LeftTopGreen = 1,
    LeftTopBlue = 2,
    LeftTopAlpha = 3,
    RightTopRed = 4,
    RightTopGreen = 5,
    RightTopBlue = 6,
    RightTopAlpha = 7,
    LeftBottomRed = 8,
    LeftBottomGreen = 9,
    LeftBottomBlue = 10,
    LeftBottomAlpha = 11,
    RightBottomRed = 12,
    RightBottomGreen = 13,
    RightBottomBlue = 14,
    RightBottomAlpha = 15,
    PaneAlpha = 16,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PaneSrtTarget {
    TranslateX = 0,
    TranslateY = 1,
    TranslateZ = 2,
    RotateX = 3,
    RotateY = 4,
    RotateZ = 5,
    ScaleX = 6,
    ScaleY = 7,
    SizeX = 8,
    SizeY = 9,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MaterialColorTarget {
    BufferRed = 0,
    BufferGreen = 1,
    BufferBlue = 2,
    BufferAlpha = 3,
    Constant0Red = 4,
    Constant0Green = 5,
    Constant0Blue = 6,
    Constant0Alpha = 7,
    Color0Red = 8,
    Color0Green = 9,
    Color0Blue = 10,
    Color0Alpha = 11,
    Color1Red = 12,
    Color1Green = 13,
    Color1Blue = 14,
    Color1Alpha = 15,
    Color2Red = 16,
    Color2Green = 17,
    Color2Blue = 18,
    Color2Alpha = 19,
    Color3Red = 20,
    Color3Green = 21,
    Color3Blue = 22,
    Color3Alpha = 23,
    Color4Red = 24,
    Color4Green = 25,
    Color4Blue = 26,
    Color4Alpha = 27,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TextureSrtTarget {
    TranslateU = 0,
    TranslateV = 1,
    Rotate = 2,
    ScaleU = 3,
    ScaleV = 4,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AnimType {
    Pane = 0,
    Material = 1,
    User = 2,
    PaneExt = 3,
    StateMachine = 4,
}

impl From<u8> for AnimType {
    fn from(value: u8) -> Self {
        match value {
            0 => AnimType::Pane,
            1 => AnimType::Material,
            2 => AnimType::User,
            3 => AnimType::PaneExt,
            4 => AnimType::StateMachine,
            // fallback
            _ => AnimType::Pane,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnimTarget {
    pub reserve0: u8,
    pub target: TargetIndex,
    pub curve: Curve,
}

/*
#[derive(Debug, Serialize, Deserialize)]
pub struct AnimInfo {
    pub magic: AnimInfoType,
    pub targets: Vec<AnimTarget>,
}*/

#[derive(Debug, Serialize, Deserialize)]
pub enum AnimInfo {
    Standard {
        magic: AnimInfoType,
        targets: Vec<AnimTarget>,
    },
    ExtendedUserData {
        magic: AnimInfoType,
        data: Vec<ExtendedUserDataAnim>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtendedUserDataAnim {
    pub block_size: u32,

    pub unk_1: u16,
    pub entry_count: u16,
    pub entries_inside_entry: u16,
    pub unk_2: u16,

    pub block_size_2: u32,

    pub unk_3: u16,
    pub unk_4: u16,
    pub unk_5: u16,
    pub frame_count: u16,

    pub values: Vec<f32>,

    pub key: String,
}

impl ExtendedUserDataAnim {
    pub fn parse(cursor: &mut Cursor) -> Self {
        let block_size = cursor.read_u32();

        let unk_1 = cursor.read_u16();
        let entry_count = cursor.read_u16();
        let entries_inside_entry = cursor.read_u16();
        let unk_2 = cursor.read_u16();

        let block_size_2 = cursor.read_u32();

        let unk_3 = cursor.read_u16();
        let unk_4 = cursor.read_u16();
        let unk_5 = cursor.read_u16();
        let frame_count = cursor.read_u16();

        let count = entry_count * entries_inside_entry;

        let mut values = Vec::new();
        for _ in 0..count {
            values.push(cursor.read_f32());
        }

        let restore = cursor.pos;
        let string_start = cursor.pos + cursor.read_u32() as usize;

        cursor.seek(string_start);

        let key = cursor.read_null_terminated_string();
        cursor.seek(restore);

        Self {
            block_size,
            unk_1,
            entry_count,
            entries_inside_entry,
            unk_2,
            block_size_2,
            unk_3,
            unk_4,
            unk_5,
            frame_count,
            values,

            key,
        }
    }

    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("ExtendedUserDataAnim");

        writer.write_u32(self.block_size);
        writer.write_u16(self.unk_1);
        writer.write_u16(self.entry_count);
        writer.write_u16(self.entries_inside_entry);
        writer.write_u16(self.unk_2);
        writer.write_u32(self.block_size_2);
        writer.write_u16(self.unk_3);
        writer.write_u16(self.unk_4);
        writer.write_u16(self.unk_5);
        writer.write_u16(self.frame_count);

        for val in &self.values {
            writer.write_f32(*val);
        }

        let offset_pos = writer.write_placeholder_u32();

        let string_start = writer.pos();
        writer.write_null_terminated_string(&self.key);

        let relative_offset = (string_start - offset_pos) as u32;
        writer.patch_u32(offset_pos, relative_offset);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnimContent {
    pub name: String,
    pub anim_type: AnimType,
    pub infos: Vec<AnimInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaneAnimInfo {
    pub frame_count: u16,
    pub is_looping: bool,
    pub textures: Vec<String>,
    pub contents: Vec<AnimContent>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BflanHeader {
    pub magic: [u8; 4],
    pub endianness: u16,
    pub header_size: u16,
    pub micro_version: u16,
    pub minor_version: u8,
    pub major_version: u8,
    pub file_size: u32,
    pub section_count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct SectionHeader {
    pub magic: SectionType,
    pub size: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Sections {
    UserData(ResUi2dUserDataSection),
    PaneAnimTag(ResBflanPaneAnimTag),
    PaneAnimInfo(PaneAnimInfo),
    Unknown(SectionHeader),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResUi2dUserDataSection {
    pub user_data_count: u16,
    pub reserve0: u16,
    pub user_data: Vec<ResUi2dUserData>,
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
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResUi2dUserDataInner {
    Float(f32),
    S32(i32),
    String(String),
    SystemData(ResUi2dSystemDataArray),
}

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

#[derive(Debug, Serialize, Deserialize)]
pub struct VertexPos {
    pub size_scale_width: f32,
    pub size_scale_height: f32,
    pub position_x_scale: f32,
    pub position_y_scale: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResBflanGroup {
    pub group_name: String,
    pub flag: u8,
    pub reserve0: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResBflanPaneAnimTag {
    pub tag_order: u16,
    pub group_count: u16,
    pub name_offset: u32,
    pub group_array_offset: u32,
    pub user_data_section_offset: u32,
    pub start_frame: u16,
    pub end_frame: u16,
    pub is_descending_bind: u8,
    pub reserve0: u8,
    pub reserve1: u16,

    pub o_name: String,
    pub groups: Vec<ResBflanGroup>,

    pub user_data: Option<ResUi2dUserDataSection>,
}

impl PaneAnimInfo {
    pub fn parse(cursor: &mut Cursor, section_start: usize) -> Self {
        let frame_count = cursor.read_u16();
        let is_looping = cursor.read_u8() != 0;
        let _reserve0 = cursor.read_u8();
        let texture_count = cursor.read_u16();
        let anim_content_count = cursor.read_u16();
        let anim_content_offset_array_offset = cursor.read_u32();

        let texture_offsets_start = cursor.pos;
        let mut texture_offsets = Vec::with_capacity(texture_count as usize);
        for _ in 0..texture_count {
            texture_offsets.push(cursor.read_u32());
        }

        let mut textures = Vec::with_capacity(texture_count as usize);
        for offset in texture_offsets {
            cursor.seek(texture_offsets_start + offset as usize);
            textures.push(cursor.read_null_terminated_string());
        }

        cursor.seek(section_start + anim_content_offset_array_offset as usize);
        let mut content_offsets = Vec::with_capacity(anim_content_count as usize);
        for _ in 0..anim_content_count {
            content_offsets.push(cursor.read_u32());
        }

        let mut contents = Vec::with_capacity(anim_content_count as usize);
        for offset in content_offsets {
            contents.push(AnimContent::parse(cursor, section_start + offset as usize));
        }

        Self {
            frame_count,
            is_looping,
            textures,
            contents,
        }
    }

    pub fn serialize(&self, writer: &mut Writer, section_start: usize) {
        writer.mark("PaneAnimInfo");
        writer.write_u16(self.frame_count);
        writer.write_u8(if self.is_looping { 1 } else { 0 });
        writer.write_u8(0);
        writer.write_u16(self.textures.len() as u16);
        writer.write_u16(self.contents.len() as u16);

        let anim_content_offset_array_offset_pos = writer.write_placeholder_u32();

        let texture_offsets_start = writer.pos();
        let mut texture_offset_placeholders = Vec::new();

        for _ in &self.textures {
            texture_offset_placeholders.push(writer.write_placeholder_u32());
        }

        for (i, texture_name) in self.textures.iter().enumerate() {
            let current_offset = writer.pos() - texture_offsets_start;
            writer.patch_u32(texture_offset_placeholders[i], current_offset as u32);
            writer.write_null_terminated_string(texture_name);
        }

        writer.align(4);

        let content_array_offset = writer.pos() - section_start;
        writer.patch_u32(
            anim_content_offset_array_offset_pos,
            content_array_offset as u32,
        );

        let mut content_offset_placeholders = Vec::new();

        for _ in &self.contents {
            content_offset_placeholders.push(writer.write_placeholder_u32());
        }

        for (i, content) in self.contents.iter().enumerate() {
            let content_base = writer.pos();

            let relative_offset = content_base - section_start;
            writer.patch_u32(content_offset_placeholders[i], relative_offset as u32);

            content.serialize(writer, content_base);
        }
    }
}

impl AnimContent {
    pub fn parse(cursor: &mut Cursor, base_offset: usize) -> Self {
        cursor.seek(base_offset);

        let name = cursor.read_fixed_string(0x1C);
        let anim_info_count = cursor.read_u8();
        let anim_type: AnimType = cursor.read_u8().into();
        let _reserve0 = cursor.read_u16();

        // Workaround for MiniGame_PictQuiz_00_MosaicNormal.bflan
        if matches!(anim_type, AnimType::User) {
            let info_array_offset = cursor.read_u32();

            cursor.seek(base_offset + info_array_offset as usize);
        }

        let mut info_offsets = Vec::with_capacity(anim_info_count as usize);
        for _ in 0..anim_info_count {
            info_offsets.push(cursor.read_u32());
        }

        let mut infos = Vec::with_capacity(anim_info_count as usize);
        for offset in info_offsets {
            infos.push(AnimInfo::parse(cursor, base_offset + offset as usize));
        }

        Self {
            name,
            anim_type,
            infos,
        }
    }

    pub fn serialize(&self, writer: &mut Writer, base_offset: usize) {
        writer.mark("AnimContent");
        writer.write_fixed_string(&self.name, 0x1C);
        writer.write_u8(self.infos.len() as u8);

        let type_val = match self.anim_type {
            AnimType::Pane => 0,
            AnimType::Material => 1,
            AnimType::User => 2,
            AnimType::PaneExt => 3,
            AnimType::StateMachine => 4,
        };

        writer.write_u8(type_val);
        writer.write_u16(0);

        let mut user_str_offset_pos = 0;

        // Workaround for MiniGame_PictQuiz_00_MosaicNormal.bflan
        if matches!(self.anim_type, AnimType::User) {
            let info_array_offset_pos = writer.write_placeholder_u32();
            user_str_offset_pos = writer.write_placeholder_u32();

            writer.patch_u32(info_array_offset_pos, (writer.pos() - base_offset) as u32);
        }

        let mut info_offset_placeholders = Vec::new();
        for _ in &self.infos {
            info_offset_placeholders.push(writer.write_placeholder_u32());
        }

        for (i, info) in self.infos.iter().enumerate() {
            let info_base = writer.pos();

            writer.patch_u32(
                info_offset_placeholders[i],
                (info_base - base_offset) as u32,
            );

            info.serialize(writer, info_base);
        }

        if matches!(self.anim_type, AnimType::User) {
            // man... dont question me.
            writer.patch_u32(user_str_offset_pos, 0x5C);
        }
    }
}

impl AnimInfo {
    pub fn parse(cursor: &mut Cursor, base_offset: usize) -> Self {
        cursor.seek(base_offset);

        let magic_val = cursor.read_u32();
        let magic: AnimInfoType = magic_val.into();

        let anim_target_count = cursor.read_u8();
        let _reserve0 = cursor.read_u8();
        let _reserve1 = cursor.read_u16();

        match magic {
            AnimInfoType::ExtendedUserDataAnim => {
                let mut data_array = Vec::new();
                for _ in 0..anim_target_count {
                    let data = ExtendedUserDataAnim::parse(cursor);
                    data_array.push(data);
                }

                Self::ExtendedUserData {
                    magic,
                    data: data_array,
                }
            }
            _ => {
                let mut target_offsets = Vec::with_capacity(anim_target_count as usize);
                for _ in 0..anim_target_count {
                    target_offsets.push(cursor.read_u32());
                }

                let mut targets = Vec::with_capacity(anim_target_count as usize);
                for offset in target_offsets {
                    targets.push(AnimTarget::parse(
                        cursor,
                        base_offset + offset as usize,
                        magic_val,
                    ));
                }

                Self::Standard { magic, targets }
            }
        }
    }

    pub fn serialize(&self, writer: &mut Writer, base_offset: usize) {
        writer.mark("AnimInfo");

        match self {
            AnimInfo::Standard { magic, targets } => {
                let magic_val = unsafe { std::mem::transmute_copy::<AnimInfoType, u32>(magic) };
                writer.write_u32(magic_val);

                writer.write_u8(targets.len() as u8);
                writer.write_u8(0);
                writer.write_u16(0);

                let mut target_offset_placeholders = Vec::new();
                for _ in targets {
                    target_offset_placeholders.push(writer.write_placeholder_u32());
                }

                for (i, target) in targets.iter().enumerate() {
                    let target_base = writer.pos();

                    let relative_offset = target_base - base_offset;
                    writer.patch_u32(target_offset_placeholders[i], relative_offset as u32);

                    target.serialize_bflan(writer, target_base);
                }
            }
            AnimInfo::ExtendedUserData { magic, data } => {
                let magic_val = unsafe { std::mem::transmute_copy::<AnimInfoType, u32>(magic) };
                writer.write_u32(magic_val);

                writer.write_u8(data.len() as u8);
                writer.write_u8(0);
                writer.write_u16(0);

                for data in data.iter() {
                    data.serialize(writer);
                }
            }
        }
    }
}

impl AnimTarget {
    pub fn parse(cursor: &mut Cursor, base_offset: usize, parent_magic: u32) -> Self {
        cursor.seek(base_offset);

        let reserve0 = cursor.read_u8();
        let target_raw = cursor.read_u8();
        let curve_type = cursor.read_u8();
        let _reserve1 = cursor.read_u8();
        let frame_count = cursor.read_u16();
        let _reserve2 = cursor.read_u16();
        let key_array_offset = cursor.read_u32();

        let target = TargetIndex::resolve(parent_magic, target_raw);

        cursor.seek(base_offset + key_array_offset as usize);

        let curve = match curve_type {
            0 => {
                let mut keys = Vec::with_capacity(frame_count as usize);
                for _ in 0..frame_count {
                    keys.push(cursor.read_f32());
                }
                Curve::Constant(keys)
            }
            1 => {
                let mut keys = Vec::with_capacity(frame_count as usize);
                for _ in 0..frame_count {
                    keys.push(StepKey {
                        frame: cursor.read_f32(),
                        value: cursor.read_u16(),
                    });
                    cursor.seek_relative(2);
                }
                Curve::Step(keys)
            }
            2 => {
                let mut keys = Vec::with_capacity(frame_count as usize);
                for _ in 0..frame_count {
                    keys.push(HermiteKey {
                        frame: cursor.read_f32(),
                        value: cursor.read_f32(),
                        slope: cursor.read_f32(),
                    });
                }
                Curve::Hermite(keys)
            }
            _ => Curve::Constant(Vec::new()),
        };

        Self {
            reserve0,
            target,
            curve,
        }
    }

    pub fn serialize_bflan(&self, writer: &mut Writer, base_offset: usize) {
        writer.mark("AnimTarget");
        writer.write_u8(self.reserve0);
        writer.write_u8(self.target.to_raw());

        let (curve_type, frame_count) = match &self.curve {
            Curve::Constant(keys) => (0, keys.len()),
            Curve::Step(keys) => (1, keys.len()),
            Curve::Hermite(keys) => (2, keys.len()),
        };

        writer.write_u8(curve_type);
        writer.write_u8(0);
        writer.write_u16(frame_count as u16);
        writer.write_u16(0);

        let key_array_offset_pos = writer.write_placeholder_u32();

        let keys_base = writer.pos();
        writer.patch_u32(key_array_offset_pos, (keys_base - base_offset) as u32);

        match &self.curve {
            Curve::Constant(keys) => {
                for key in keys {
                    writer.write_f32(*key);
                }
            }
            Curve::Step(keys) => {
                for key in keys {
                    writer.write_f32(key.frame);
                    writer.write_u16(key.value);
                    writer.write_u16(0);
                }
            }
            Curve::Hermite(keys) => {
                for key in keys {
                    writer.write_f32(key.frame);
                    writer.write_f32(key.value);
                    writer.write_f32(key.slope);
                }
            }
        }
    }
}

impl ResBflanPaneAnimTag {
    pub fn parse(cursor: &mut Cursor, section_start: usize) -> Self {
        let mut tag = Self {
            tag_order: cursor.read_u16(),
            group_count: cursor.read_u16(),
            name_offset: cursor.read_u32(),
            group_array_offset: cursor.read_u32(),
            user_data_section_offset: cursor.read_u32(),
            start_frame: cursor.read_u16(),
            end_frame: cursor.read_u16(),
            is_descending_bind: cursor.read_u8(),
            reserve0: cursor.read_u8(),
            reserve1: cursor.read_u16(),
            o_name: String::new(),
            groups: Vec::new(),
            user_data: None,
        };

        if tag.name_offset > 0 {
            cursor.seek(section_start + tag.name_offset as usize);
            tag.o_name = cursor.read_null_terminated_string();
        }

        if tag.group_count > 0 && tag.group_array_offset > 0 {
            cursor.seek(section_start + tag.group_array_offset as usize);
            for _ in 0..tag.group_count {
                tag.groups.push(ResBflanGroup {
                    group_name: cursor.read_fixed_string(0x21),
                    flag: cursor.read_u8(),
                    reserve0: cursor.read_u16(),
                });
            }
        }

        if tag.user_data_section_offset > 0 {
            cursor.seek(section_start + tag.user_data_section_offset as usize);

            let embed_magic = cursor.read_u32();
            let _embed_size = cursor.read_u32();

            if embed_magic == tchar_code32(b"usd1") {
                tag.user_data = Some(ResUi2dUserDataSection::parse(cursor));
            }
        }

        tag
    }
}

impl BflanFile {
    pub fn parse_file(file: &[u8]) -> Result<Self, String> {
        let mut cursor = Cursor { data: file, pos: 0 };

        let header = Self::parse_header(&mut cursor)?;
        let sections = Self::parse_sections(&mut cursor, header.section_count);

        Ok(Self { header, sections })
    }

    fn parse_header(cur: &mut Cursor) -> Result<BflanHeader, String> {
        let magic = cur.read_bytes(4).try_into().unwrap();
        if &magic != b"FLAN" {
            return Err("bad magic".into());
        }

        Ok(BflanHeader {
            magic,
            endianness: cur.read_u16(),
            header_size: cur.read_u16(),
            micro_version: cur.read_u16(),
            minor_version: cur.read_u8(),
            major_version: cur.read_u8(),
            file_size: cur.read_u32(),
            section_count: cur.read_u32(),
        })
    }

    fn parse_sections(cur: &mut Cursor, count: u32) -> Vec<Sections> {
        let mut sections = Vec::new();

        for _ in 0..count {
            let section_start = cur.pos;

            let header = SectionHeader {
                magic: cur.read_u32().into(),
                size: cur.read_u32(),
            };

            match header.magic {
                SectionType::UserData => {
                    sections.push(Sections::UserData(ResUi2dUserDataSection::parse(cur)));
                }
                SectionType::PaneAnimTag => {
                    sections.push(Sections::PaneAnimTag(ResBflanPaneAnimTag::parse(
                        cur,
                        section_start,
                    )));
                }
                SectionType::PaneAnimInfo => {
                    sections.push(Sections::PaneAnimInfo(PaneAnimInfo::parse(
                        cur,
                        section_start,
                    )));
                }
                _ => {
                    sections.push(Sections::Unknown(header));
                }
            }

            cur.seek(section_start + header.size as usize);
        }

        sections
    }
}

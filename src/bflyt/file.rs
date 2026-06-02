use serde::{Deserialize, Serialize};

use crate::{
    core::{Cursor, Writer, tchar_code32},
    ui2d::userdata::ResUi2dUserDataSection,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Bflyt {
    pub magic: u32,
    pub endianness: u16,
    pub header_size: u16,
    pub micro_version: u16,
    pub minor_version: u8,
    pub major_version: u8,
    pub file_size: u32,
    pub section_count: u32,
}

impl Bflyt {
    pub fn parse(file: &[u8]) -> Result<Self, String> {
        let mut cursor = Cursor { data: file, pos: 0 };

        let magic = cursor.read_u32();

        if magic != tchar_code32(b"FLYT") {
            return Err("bad magic".into());
        }

        let endianness = cursor.read_u16();
        let header_size = cursor.read_u16();
        let micro_version = cursor.read_u16();
        let minor_version = cursor.read_u8();
        let major_version = cursor.read_u8();
        let file_size = cursor.read_u32();
        let section_count = cursor.read_u32();

        Ok(Self {
            magic,
            endianness,
            header_size,
            micro_version,
            minor_version,
            major_version,
            file_size,
            section_count,
        })
    }

    pub fn serialize(&self) -> Writer {
        let mut writer = Writer::new();

        writer.mark("File header");

        writer
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BflytSections {
    UserData(ResUi2dUserDataSection),
    Layout(BflytLayout),
    TextureList(BflytTextureList),
    FontList(BflytFontList),
    MaterialList(BflytMaterialList),
    CaptureTextureList(BflytCaptureTextureList),
    VectorGraphicsList(VectorGraphicsList),
    Pane(BflytPane),
    PicturePane(BflytPicturePane),
    TextBoxPane(BflytTextBoxPane),
    WindowPane(BflytWindowPane),
    PartsPane(BflytPartsPane),
    AlignmentPane(BflytAlignmentPane),
    Group(BflytGroup),
    ControlSource(BflytControlSource),
}

use serde::{Deserialize, Serialize};

use crate::{
    bflyt::{
        constants::*,
        list::{
            BflytCaptureTextureList, BflytControlSource, BflytFontList, BflytGroup, BflytLayout,
            BflytMaterialList, BflytTextureList, BflytVectorGraphicsList,
        },
        pane::{
            BflytAlignmentPane, BflytCapturePane, BflytPane, BflytPartsPane, BflytPicturePane,
            BflytTextBoxPane, BflytWindowPane,
        },
    },
    core::{Cursor, SectionHeader, Writer, tchar_code32},
    ui2d::userdata::ResUi2dUserDataSection,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BflytSection {
    UserData(ResUi2dUserDataSection),
    Layout(BflytLayout),
    TextureList(BflytTextureList),
    FontList(BflytFontList),
    MaterialList(BflytMaterialList),
    CaptureTextureList(BflytCaptureTextureList),
    VectorGraphicsList(BflytVectorGraphicsList),
    Pane(BflytPane),
    PicturePane(BflytPicturePane),
    TextBoxPane(BflytTextBoxPane),
    WindowPane(BflytWindowPane),
    PartsPane(BflytPartsPane),
    AlignmentPane(BflytAlignmentPane),
    CapturePane(BflytCapturePane),
    BoundingPane(BflytPane),
    ScissorPane(BflytPane),
    Group(BflytGroup),
    ControlSource(BflytControlSource),
    PaneStart,
    PaneEnd,
    GroupStart,
    GroupEnd,
    Unknown(SectionHeader, Vec<u8>),
}

impl BflytSection {
    pub fn parse(cursor: &mut Cursor, last_was_pane: &mut bool, is_embed: bool) -> Self {
        let section_start = cursor.pos;
        let magic = cursor.read_u32();
        let section_size = cursor.read_u32();
        let end = section_start + section_size as usize;

        let section = match magic {
            MAGIC_USERDATA => {
                let s = ResUi2dUserDataSection::parse(cursor, *last_was_pane);
                BflytSection::UserData(s)
            }
            MAGIC_LAYOUT => {
                let s = BflytLayout::parse(cursor);
                if !is_embed {
                    *last_was_pane = false;
                }
                BflytSection::Layout(s)
            }
            MAGIC_TEXTURELIST => {
                let s = BflytTextureList::parse(cursor);
                BflytSection::TextureList(s)
            }
            MAGIC_FONTLIST => {
                let s = BflytFontList::parse(cursor);
                BflytSection::FontList(s)
            }
            MAGIC_MATERIALLIST => {
                let s = BflytMaterialList::parse(cursor, section_start);
                BflytSection::MaterialList(s)
            }
            MAGIC_CAPTURETEXTURELIST => {
                let s = BflytCaptureTextureList::parse(cursor, section_start);
                BflytSection::CaptureTextureList(s)
            }
            MAGIC_VECTORGRAPHICSLIST => {
                let s = BflytVectorGraphicsList::parse(cursor, section_start);
                BflytSection::VectorGraphicsList(s)
            }
            MAGIC_PANESTART => BflytSection::PaneStart,
            MAGIC_PANEEND => BflytSection::PaneEnd,
            MAGIC_GROUPSTART => BflytSection::GroupStart,
            MAGIC_GROUPEND => BflytSection::GroupEnd,
            MAGIC_PANE => {
                let s = BflytPane::parse(cursor);
                if !is_embed {
                    *last_was_pane = true;
                }
                BflytSection::Pane(s)
            }
            MAGIC_PICTUREPANE => {
                let s = BflytPicturePane::parse(cursor);
                BflytSection::PicturePane(s)
            }
            MAGIC_TEXTBOXPANE => {
                let s = BflytTextBoxPane::parse(cursor, section_start);
                BflytSection::TextBoxPane(s)
            }
            MAGIC_WINDOWPANE => {
                let s = BflytWindowPane::parse(cursor);
                BflytSection::WindowPane(s)
            }
            MAGIC_PARTSPANE => {
                let s = BflytPartsPane::parse(cursor, last_was_pane);
                BflytSection::PartsPane(s)
            }
            MAGIC_ALIGNMENTPANE => {
                let s = BflytAlignmentPane::parse(cursor);
                BflytSection::AlignmentPane(s)
            }
            MAGIC_CAPTUREPANE => {
                let s = BflytCapturePane::parse(cursor);
                BflytSection::CapturePane(s)
            }
            MAGIC_BOUNDINGPANE => {
                let s = BflytPane::parse(cursor);
                BflytSection::BoundingPane(s)
            }
            MAGIC_SCISSORPANE => {
                let s = BflytPane::parse(cursor);
                BflytSection::ScissorPane(s)
            }
            MAGIC_GROUP => {
                let s = BflytGroup::parse(cursor);
                BflytSection::Group(s)
            }
            MAGIC_CONTROLSOURCE => {
                let s = BflytControlSource::parse(cursor);
                BflytSection::ControlSource(s)
            }
            _ => {
                println!("Got unknown pane w/ magic: {magic}");

                let data_size = (section_size as usize).saturating_sub(8);
                let data = cursor
                    .read_bytes(data_size.min(end.saturating_sub(cursor.pos)))
                    .to_vec();
                BflytSection::Unknown(
                    SectionHeader {
                        magic,
                        size: section_size,
                    },
                    data,
                )
            }
        };

        cursor.seek(end);

        section
    }

    pub fn serialize(&self, writer: &mut Writer) {
        let section_start = writer.pos();
        let magic = section_magic(self);

        writer.write_u32(magic);
        let size_pos = writer.write_placeholder_u32();

        writer.mark(&format!("BflytSection {}", section_name(self)));

        match self {
            Self::UserData(s) => s.serialize(writer),
            Self::Layout(s) => s.serialize(writer),
            Self::TextureList(s) => s.serialize(writer),
            Self::FontList(s) => s.serialize(writer),
            Self::MaterialList(s) => s.serialize(writer, section_start),
            Self::CaptureTextureList(s) => s.serialize(writer, section_start),
            Self::VectorGraphicsList(s) => s.serialize(writer, section_start),
            Self::Pane(s) | Self::BoundingPane(s) | Self::ScissorPane(s) => s.serialize(writer),
            Self::PicturePane(s) => s.serialize(writer),
            Self::TextBoxPane(s) => s.serialize(writer, section_start),
            Self::WindowPane(s) => s.serialize(writer),
            Self::PartsPane(s) => s.serialize(writer, section_start),
            Self::AlignmentPane(s) => s.serialize(writer),
            Self::CapturePane(s) => s.serialize(writer),
            Self::Group(s) => s.serialize(writer),
            Self::ControlSource(s) => s.serialize(writer, section_start),
            Self::Unknown(_, data) => writer.write_bytes(data),
            Self::PaneStart | Self::PaneEnd | Self::GroupStart | Self::GroupEnd => {}
        }

        writer.align(4);

        let size = (writer.pos() - section_start) as u32;
        writer.patch_u32(size_pos, size);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bflyt {
    pub magic: u32,
    pub endianness: u16,
    pub header_size: u16,
    pub micro_version: u16,
    pub minor_version: u8,
    pub major_version: u8,
    pub sections: Vec<BflytSection>,
}

impl Bflyt {
    pub fn parse(file: &[u8]) -> Result<Self, String> {
        let mut cursor = Cursor { data: file, pos: 0 };

        let magic = cursor.read_u32();
        if magic != tchar_code32(b"FLYT") {
            return Err("bad magic: expected FLYT".into());
        }

        let endianness = cursor.read_u16();
        let header_size = cursor.read_u16();
        let micro_version = cursor.read_u16();
        let minor_version = cursor.read_u8();
        let major_version = cursor.read_u8();
        let _file_size = cursor.read_u32();
        let section_count = cursor.read_u32();

        cursor.seek(header_size as usize);

        let mut sections = Vec::new();

        let mut last_was_pane = false;
        for _ in 0..section_count {
            let section = BflytSection::parse(&mut cursor, &mut last_was_pane, false);

            sections.push(section);
        }

        let mut bflyt = Self {
            magic,
            endianness,
            header_size,
            micro_version,
            minor_version,
            major_version,
            sections,
        };

        bflyt.resolve_names();
        Ok(bflyt)
    }

    pub fn serialize(&self) -> Writer {
        let mut this = Self {
            magic: self.magic,
            endianness: self.endianness,
            header_size: self.header_size,
            micro_version: self.micro_version,
            minor_version: self.minor_version,
            major_version: self.major_version,
            sections: self.sections.clone(),
        };

        this.rebuild_indices();
        let mut writer = Writer::new();

        writer.mark("File header");
        writer.write_u32(self.magic);
        writer.write_u16(self.endianness);
        writer.write_u16(self.header_size);
        writer.write_u16(self.micro_version);
        writer.write_u8(self.minor_version);
        writer.write_u8(self.major_version);

        let file_size_pos = writer.write_placeholder_u32();
        writer.write_u32(self.sections.len() as u32);

        while writer.pos() < self.header_size as usize {
            writer.write_u8(0);
        }

        for section in &this.sections {
            section.serialize(&mut writer);
        }

        let total = writer.pos() as u32;
        writer.patch_u32(file_size_pos, total);

        writer
    }

    fn get_texture_names(&self) -> Vec<String> {
        self.sections
            .iter()
            .find_map(|s| {
                if let BflytSection::TextureList(t) = s {
                    Some(t.textures.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    fn resolve_names(&mut self) {
        let textures = self.get_texture_names();

        for section in &mut self.sections {
            if let BflytSection::MaterialList(ml) = section {
                for mat in &mut ml.materials {
                    for tm in &mut mat.tex_maps {
                        tm.texture_name = textures
                            .get(tm.texture_index as usize)
                            .cloned()
                            .unwrap_or_default();
                    }
                }
            }
        }
    }

    fn rebuild_indices(&mut self) {
        let textures = self.get_texture_names();

        for section in &mut self.sections {
            if let BflytSection::MaterialList(ml) = section {
                for mat in &mut ml.materials {
                    for tm in &mut mat.tex_maps {
                        tm.texture_index = textures
                            .iter()
                            .position(|t| t == &tm.texture_name)
                            .unwrap_or(0) as u16;
                    }
                }
            }
        }
    }
}

fn section_magic(section: &BflytSection) -> u32 {
    match section {
        BflytSection::UserData(_) => MAGIC_USERDATA,
        BflytSection::Layout(_) => MAGIC_LAYOUT,
        BflytSection::TextureList(_) => MAGIC_TEXTURELIST,
        BflytSection::FontList(_) => MAGIC_FONTLIST,
        BflytSection::MaterialList(_) => MAGIC_MATERIALLIST,
        BflytSection::CaptureTextureList(_) => MAGIC_CAPTURETEXTURELIST,
        BflytSection::VectorGraphicsList(_) => MAGIC_VECTORGRAPHICSLIST,
        BflytSection::Pane(_) => MAGIC_PANE,
        BflytSection::PicturePane(_) => MAGIC_PICTUREPANE,
        BflytSection::TextBoxPane(_) => MAGIC_TEXTBOXPANE,
        BflytSection::WindowPane(_) => MAGIC_WINDOWPANE,
        BflytSection::PartsPane(_) => MAGIC_PARTSPANE,
        BflytSection::AlignmentPane(_) => MAGIC_ALIGNMENTPANE,
        BflytSection::CapturePane(_) => MAGIC_CAPTUREPANE,
        BflytSection::BoundingPane(_) => MAGIC_BOUNDINGPANE,
        BflytSection::ScissorPane(_) => MAGIC_SCISSORPANE,
        BflytSection::Group(_) => MAGIC_GROUP,
        BflytSection::ControlSource(_) => MAGIC_CONTROLSOURCE,
        BflytSection::PaneStart => MAGIC_PANESTART,
        BflytSection::PaneEnd => MAGIC_PANEEND,
        BflytSection::GroupStart => MAGIC_GROUPSTART,
        BflytSection::GroupEnd => MAGIC_GROUPEND,
        BflytSection::Unknown(h, _) => h.magic,
    }
}

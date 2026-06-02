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

#[derive(Debug, Serialize, Deserialize)]
pub struct PaneNode {
    pub pane: BflytSection,
    pub children: Vec<PaneNode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroupNode {
    pub group: BflytGroup,
    pub children: Vec<GroupNode>,
}

#[derive(Debug, Serialize, Deserialize)]
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
    Unknown(SectionHeader, Vec<u8>),
}

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
    pub sections: Vec<BflytSection>,
    pub pane_tree: Vec<PaneNode>,
    pub group_tree: Vec<GroupNode>,
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
        let file_size = cursor.read_u32();
        let section_count = cursor.read_u32();

        cursor.seek(header_size as usize);

        let sections = parse_flat_sections(&mut cursor, section_count, file.len());

        let mut idx = 0;
        let pane_tree = build_pane_tree(&sections, &mut idx);
        idx = 0;
        let group_tree = build_group_tree(&sections, &mut idx);

        Ok(Self {
            magic,
            endianness,
            header_size,
            micro_version,
            minor_version,
            major_version,
            file_size,
            section_count,
            sections,
            pane_tree,
            group_tree,
        })
    }

    pub fn serialize(&self) -> Writer {
        let mut writer = Writer::new();

        writer.mark("File header");
        writer.write_u32(self.magic);
        writer.write_u16(self.endianness);
        writer.write_u16(self.header_size);
        writer.write_u16(self.micro_version);
        writer.write_u8(self.minor_version);
        writer.write_u8(self.major_version);

        let file_size_pos = writer.write_placeholder_u32();
        let section_count_pos = writer.write_placeholder_u32();

        while writer.pos() < self.header_size as usize {
            writer.write_u8(0);
        }

        let mut section_count = 0u32;
        for section in &self.sections {
            serialize_section(section, &mut writer);
            section_count += 1;
        }

        let total = writer.pos() as u32;
        writer.patch_u32(file_size_pos, total);
        writer.patch_u32(section_count_pos, section_count);

        writer
    }
}

fn parse_flat_sections(cursor: &mut Cursor, count: u32, file_len: usize) -> Vec<BflytSection> {
    let mut sections = Vec::new();
    let mut last_was_pane = false;

    for _ in 0..count {
        if cursor.pos + 8 > file_len {
            break;
        }

        let section_start = cursor.pos;
        let magic = cursor.read_u32();
        let section_size = cursor.read_u32();
        let end = section_start + section_size as usize;

        let section = match magic {
            MAGIC_USERDATA => {
                let s = ResUi2dUserDataSection::parse(cursor, last_was_pane);
                BflytSection::UserData(s)
            }
            MAGIC_LAYOUT => {
                let s = BflytLayout::parse(cursor);
                last_was_pane = false;
                BflytSection::Layout(s)
            }
            MAGIC_TEXTURELIST => {
                let s = BflytTextureList::parse(cursor, section_start);
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
            MAGIC_PANESTART => BflytSection::Unknown(
                SectionHeader {
                    magic,
                    size: section_size,
                },
                Vec::new(),
            ),
            MAGIC_PANEEND => BflytSection::Unknown(
                SectionHeader {
                    magic,
                    size: section_size,
                },
                Vec::new(),
            ),
            MAGIC_GROUPSTART => BflytSection::Unknown(
                SectionHeader {
                    magic,
                    size: section_size,
                },
                Vec::new(),
            ),
            MAGIC_GROUPEND => BflytSection::Unknown(
                SectionHeader {
                    magic,
                    size: section_size,
                },
                Vec::new(),
            ),
            MAGIC_PANE => {
                let s = BflytPane::parse(cursor);
                last_was_pane = true;
                BflytSection::Pane(s)
            }
            MAGIC_PICTUREPANE => {
                let s = BflytPicturePane::parse(cursor);
                BflytSection::PicturePane(s)
            }
            MAGIC_TEXTBOXPANE => {
                let s = BflytTextBoxPane::parse(cursor, section_start, end);
                BflytSection::TextBoxPane(s)
            }
            MAGIC_WINDOWPANE => {
                let s = BflytWindowPane::parse(cursor, section_start);
                BflytSection::WindowPane(s)
            }
            MAGIC_PARTSPANE => {
                let s = BflytPartsPane::parse(cursor, section_start, section_size);
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
                let s = BflytControlSource::parse(cursor, section_start);
                BflytSection::ControlSource(s)
            }
            _ => {
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

        sections.push(section);
        cursor.seek(end.min(file_len));
    }

    sections
}

fn build_pane_tree(sections: &[BflytSection], idx: &mut usize) -> Vec<PaneNode> {
    let mut nodes = Vec::new();
    while *idx < sections.len() {
        match &sections[*idx] {
            BflytSection::Unknown(h, _) if h.magic == MAGIC_PANESTART => {
                *idx += 1;

                if *idx < sections.len() {
                    let pane_section = sections[*idx].clone_pane_section();
                    *idx += 1;
                    let children = build_pane_tree(sections, idx);

                    if *idx < sections.len() {
                        if let BflytSection::Unknown(h, _) = &sections[*idx] {
                            if h.magic == MAGIC_PANEEND {
                                *idx += 1;
                            }
                        }
                    }
                    if let Some(pane) = pane_section {
                        nodes.push(PaneNode { pane, children });
                    }
                }
            }
            BflytSection::Unknown(h, _) if h.magic == MAGIC_PANEEND => {
                break;
            }
            _ => {
                *idx += 1;
            }
        }
    }
    nodes
}

fn build_group_tree(sections: &[BflytSection], idx: &mut usize) -> Vec<GroupNode> {
    let mut nodes = Vec::new();
    while *idx < sections.len() {
        match &sections[*idx] {
            BflytSection::Unknown(h, _) if h.magic == MAGIC_GROUPSTART => {
                *idx += 1;
                if *idx < sections.len() {
                    if let BflytSection::Group(g) = &sections[*idx] {
                        let group = g.clone_group();
                        *idx += 1;
                        let children = build_group_tree(sections, idx);
                        if *idx < sections.len() {
                            if let BflytSection::Unknown(h, _) = &sections[*idx] {
                                if h.magic == MAGIC_GROUPEND {
                                    *idx += 1;
                                }
                            }
                        }
                        nodes.push(GroupNode { group, children });
                    }
                }
            }
            BflytSection::Unknown(h, _) if h.magic == MAGIC_GROUPEND => break,
            _ => {
                *idx += 1;
            }
        }
    }
    nodes
}

fn serialize_section(section: &BflytSection, writer: &mut Writer) {
    let section_start = writer.pos();
    let magic = section_magic(section);
    writer.write_u32(magic);
    let size_pos = writer.write_placeholder_u32();

    writer.mark(&format!("Section {}", section_name(section)));

    match section {
        BflytSection::UserData(s) => s.serialize(writer),
        BflytSection::Layout(s) => s.serialize(writer),
        BflytSection::TextureList(s) => s.serialize(writer),
        BflytSection::FontList(s) => s.serialize(writer),
        BflytSection::MaterialList(s) => s.serialize(writer, section_start),
        BflytSection::CaptureTextureList(s) => s.serialize(writer, section_start),
        BflytSection::VectorGraphicsList(s) => s.serialize(writer, section_start),
        BflytSection::Pane(s) | BflytSection::BoundingPane(s) | BflytSection::ScissorPane(s) => {
            s.serialize(writer)
        }
        BflytSection::PicturePane(s) => s.serialize(writer),
        BflytSection::TextBoxPane(s) => s.serialize(writer, section_start),
        BflytSection::WindowPane(s) => s.serialize(writer, section_start),
        BflytSection::PartsPane(s) => s.serialize(writer, section_start),
        BflytSection::AlignmentPane(s) => s.serialize(writer),
        BflytSection::CapturePane(s) => s.serialize(writer),
        BflytSection::Group(s) => s.serialize(writer),
        BflytSection::ControlSource(s) => s.serialize(writer, section_start),
        BflytSection::Unknown(_, data) => writer.write_bytes(data),
    }

    writer.align(4);
    let size = (writer.pos() - section_start) as u32;
    writer.patch_u32(size_pos, size);
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
        BflytSection::Unknown(h, _) => h.magic,
    }
}

trait ClonePaneSection {
    fn clone_pane_section(&self) -> Option<BflytSection>;
}

impl ClonePaneSection for BflytSection {
    fn clone_pane_section(&self) -> Option<BflytSection> {
        None
    }
}

trait CloneGroup {
    fn clone_group(&self) -> BflytGroup;
}

impl CloneGroup for crate::bflyt::list::BflytGroup {
    fn clone_group(&self) -> BflytGroup {
        BflytGroup {
            group_name: self.group_name.clone(),
            reserve0: self.reserve0,
            child_names: self.child_names.clone(),
        }
    }
}

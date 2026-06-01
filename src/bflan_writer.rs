use crate::bflan::{
    BflanFile, ResBflanPaneAnimTag, ResUi2dUserData, ResUi2dUserDataInner, ResUi2dUserDataSection,
    SectionType, Sections, Ui2dUserDataType,
};

pub struct Writer {
    pub buffer: Vec<u8>,
    pub breadcrumbs: Vec<(usize, String)>,
}

impl Writer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            breadcrumbs: Vec::new(),
        }
    }

    pub fn mark(&mut self, name: &str) {
        self.breadcrumbs.push((self.pos(), name.to_string()));
    }

    pub fn pos(&self) -> usize {
        self.buffer.len()
    }

    pub fn write_u8(&mut self, val: u8) {
        self.buffer.push(val);
    }

    pub fn write_u16(&mut self, val: u16) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_u32(&mut self, val: u32) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_i32(&mut self, val: i32) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_f32(&mut self, val: f32) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    pub fn write_fixed_string(&mut self, s: &str, len: usize) {
        let bytes = s.as_bytes();
        let write_len = bytes.len().min(len);
        self.buffer.extend_from_slice(&bytes[..write_len]);

        for _ in write_len..len {
            self.buffer.push(0);
        }
    }

    pub fn write_null_terminated_string(&mut self, s: &str) {
        self.buffer.extend_from_slice(s.as_bytes());
        self.buffer.push(0);
    }

    pub fn write_placeholder_u32(&mut self) -> usize {
        let pos = self.pos();
        self.write_u32(0);
        pos
    }

    pub fn patch_u32(&mut self, pos: usize, val: u32) {
        let bytes = val.to_le_bytes();
        self.buffer[pos..pos + 4].copy_from_slice(&bytes);
    }

    pub fn align(&mut self, alignment: usize) {
        let remainder = self.pos() % alignment;
        if remainder != 0 {
            let padding = alignment - remainder;
            for _ in 0..padding {
                self.write_u8(0);
            }
        }
    }
}

pub fn serialize_bflan(file: BflanFile) -> Writer {
    let mut writer = Writer::new();

    writer.mark("File header");
    writer.write_bytes(&file.header.magic);
    writer.write_u16(file.header.endianness);
    writer.write_u16(file.header.header_size);
    writer.write_u16(file.header.micro_version);
    writer.write_u8(file.header.minor_version);
    writer.write_u8(file.header.major_version);

    let file_size_pos = writer.write_placeholder_u32();
    writer.write_u32(file.sections.len() as u32);

    for section in file.sections {
        section.serialize(&mut writer);
    }

    let total_size = writer.pos() as u32;
    writer.patch_u32(file_size_pos, total_size);

    writer
}

impl Sections {
    pub fn serialize(&self, writer: &mut Writer) {
        let section_start = writer.pos();

        writer.mark("Section (header)");
        match self {
            Sections::UserData(_) => writer.write_u32(SectionType::UserData as u32),
            Sections::PaneAnimTag(_) => writer.write_u32(SectionType::PaneAnimTag as u32),
            Sections::PaneAnimInfo(_) => writer.write_u32(SectionType::PaneAnimInfo as u32),
            Sections::Unknown(header) => writer.write_u32(header.magic as u32),
        }

        let size_pos = writer.write_placeholder_u32();

        writer.mark("Section (data)");
        match self {
            Sections::UserData(data) => data.serialize(writer),
            Sections::PaneAnimTag(tag) => tag.serialize(writer, section_start),
            Sections::PaneAnimInfo(info) => info.serialize(writer, section_start),
            Sections::Unknown(_) => {}
        }

        writer.align(4);

        let size = (writer.pos() - section_start) as u32;
        writer.patch_u32(size_pos, size);
    }
}

impl ResBflanPaneAnimTag {
    pub fn serialize(&self, writer: &mut Writer, section_start: usize) {
        writer.mark("PaneAnimTag");
        writer.write_u16(self.tag_order);
        writer.write_u16(self.groups.len() as u16);

        let name_offset_pos = writer.write_placeholder_u32();
        let group_offset_pos = writer.write_placeholder_u32();
        let user_data_offset_pos = writer.write_placeholder_u32();

        writer.write_u16(self.start_frame);
        writer.write_u16(self.end_frame);
        writer.write_u8(self.is_descending_bind);
        writer.write_u8(self.reserve0);
        writer.write_u16(self.reserve1);

        writer.patch_u32(name_offset_pos, (writer.pos() - section_start) as u32);
        writer.write_null_terminated_string(&self.o_name);
        writer.align(4);

        writer.patch_u32(group_offset_pos, (writer.pos() - section_start) as u32);
        for group in &self.groups {
            writer.write_fixed_string(&group.group_name, 0x21);
            writer.write_u8(group.flag);
            writer.write_u16(group.reserve0);
        }

        if let Some(user_data) = &self.user_data {
            writer.align(4);
            writer.patch_u32(user_data_offset_pos, (writer.pos() - section_start) as u32);

            let embed_start = writer.pos();
            writer.write_u32(SectionType::UserData as u32);
            let embed_size_pos = writer.write_placeholder_u32();

            user_data.serialize(writer);

            let embed_size = (writer.pos() - embed_start) as u32;
            writer.patch_u32(embed_size_pos, embed_size);
        }
    }
}

impl ResUi2dUserDataSection {
    pub fn serialize(&self, writer: &mut Writer) {
        writer.mark("UserData (section)");
        writer.write_u16(self.user_data.len() as u16);
        writer.write_u16(self.reserve0);

        for data in &self.user_data {
            data.serialize(writer);
        }
    }
}

impl ResUi2dUserData {
    pub fn serialize(&self, writer: &mut Writer) {
        let base_offset = writer.pos();

        writer.mark("UserData (data)");
        let name_offset_pos = writer.write_placeholder_u32();
        let data_array_offset_pos = writer.write_placeholder_u32();
        writer.write_u16(self.data_count);

        let data_type_val = match self.data_type {
            Ui2dUserDataType::String => 0,
            Ui2dUserDataType::S32 => 1,
            Ui2dUserDataType::Float => 2,
            Ui2dUserDataType::SystemData => 3,
            Ui2dUserDataType::Invalid => 4,
        };
        writer.write_u8(data_type_val);
        writer.write_u8(self.reserve0);

        if !self.data_array.is_empty() {
            writer.patch_u32(data_array_offset_pos, (writer.pos() - base_offset) as u32);

            for item in &self.data_array {
                match item {
                    ResUi2dUserDataInner::Float(f) => writer.write_f32(*f),
                    ResUi2dUserDataInner::S32(s) => writer.write_i32(*s),
                    ResUi2dUserDataInner::String(s) => {
                        writer.write_fixed_string(s, self.data_count as usize);
                        writer.write_u8(0);
                    }
                    ResUi2dUserDataInner::SystemData(_) => {}
                }
            }
        }

        writer.patch_u32(name_offset_pos, (writer.pos() - base_offset) as u32);
        writer.write_null_terminated_string(&self.o_name);

        writer.align(4);
    }
}

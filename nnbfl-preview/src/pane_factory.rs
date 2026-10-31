use nnbfl::{
    bflyt::{
        file::BflytSection,
        flags::{PaneFlags, WindowFlags},
        list::{
            MATERIAL_NAME_LEN, Material, MaterialColorEntry, MaterialColorInterpolation,
            MaterialList,
        },
        pane::{
            AlignmentPane, Pane, PicturePane, TextAlignment, TextBoxPane, WindowContent,
            WindowFrame, WindowPane,
        },
    },
    ui2d::types::{Color4u8, Vector2f},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewPaneKind {
    Null,
    Picture,
    TextBox,
    Window,
    Bounding,
    Scissor,
    Alignment,
}

const WHITE: Color4u8 = Color4u8 {
    r: 255,
    g: 255,
    b: 255,
    a: 255,
};

const BLACK: Color4u8 = Color4u8 {
    r: 0,
    g: 0,
    b: 0,
    a: 255,
};

const TEXT: &str = "Text\0";

impl NewPaneKind {
    pub const ALL: [Self; 7] = [
        Self::Null,
        Self::Picture,
        Self::TextBox,
        Self::Window,
        Self::Bounding,
        Self::Scissor,
        Self::Alignment,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Picture => "Picture",
            Self::TextBox => "Text box",
            Self::Window => "Window",
            Self::Bounding => "Bounding",
            Self::Scissor => "Scissor",
            Self::Alignment => "Alignment",
        }
    }

    pub fn name_prefix(self) -> &'static str {
        match self {
            Self::Null => "N",
            Self::Picture => "P",
            Self::TextBox => "T",
            Self::Window => "W",
            Self::Bounding => "B",
            Self::Scissor => "S",
            Self::Alignment => "A",
        }
    }

    pub fn needs_font(self) -> bool {
        self == Self::TextBox
    }
}

pub fn new_pane_section(
    kind: NewPaneKind,
    name: &str,
    materials: &mut MaterialList,
) -> BflytSection {
    let base = Pane {
        pane_flags: PaneFlags {
            is_visible: true,
            ..Default::default()
        },
        alpha: 255,
        pane_name: name.to_string(),
        scale: Vector2f { x: 1.0, y: 1.0 },
        size: Vector2f { x: 100.0, y: 100.0 },
        ..Default::default()
    };

    match kind {
        NewPaneKind::Null => BflytSection::Pane(base),
        NewPaneKind::Bounding => BflytSection::BoundingPane(base),
        NewPaneKind::Scissor => BflytSection::ScissorPane(base),
        NewPaneKind::Alignment => BflytSection::AlignmentPane(AlignmentPane {
            base,
            ..Default::default()
        }),

        NewPaneKind::Picture => BflytSection::PicturePane(PicturePane {
            base,
            top_left_vertex_color: WHITE,
            top_right_vertex_color: WHITE,
            bottom_left_vertex_color: WHITE,
            bottom_right_vertex_color: WHITE,
            material_index: add_material(materials, name),
            ..Default::default()
        }),

        NewPaneKind::TextBox => {
            let size = (TEXT.encode_utf16().count() * 2) as u16;

            BflytSection::TextBoxPane(TextBoxPane {
                base,
                text_buffer_size: size,
                text_length: size,
                material_index: add_material(materials, name),
                font_index: 0,
                text_alignment: TextAlignment::Synchronous,
                font_top_color: WHITE,
                font_bottom_color: WHITE,
                font_size: Vector2f { x: 32.0, y: 32.0 },
                shadow_translation: Vector2f { x: 1.0, y: 1.0 },
                shadow_size: Vector2f { x: 1.0, y: 1.0 },
                shadow_top_color: BLACK,
                shadow_bottom_color: BLACK,
                text: Some(TEXT.to_string()),
                ..Default::default()
            })
        }

        NewPaneKind::Window => {
            let content = add_material(materials, name);
            let frame = add_material(materials, &format!("{name}_F"));

            BflytSection::WindowPane(WindowPane {
                base,
                frame_size_left: 16,
                frame_size_right: 16,
                frame_size_top: 16,
                frame_size_bottom: 16,
                flag: WindowFlags::default(),
                content: WindowContent {
                    top_left_vertex_color: WHITE,
                    top_right_vertex_color: WHITE,
                    bottom_left_vertex_color: WHITE,
                    bottom_right_vertex_color: WHITE,
                    material_index: content,
                    picture_uvs: Vec::new(),
                },
                frames: vec![WindowFrame {
                    material_index: frame,
                    ..Default::default()
                }],
                ..Default::default()
            })
        }
    }
}

pub(crate) fn add_material(materials: &mut MaterialList, base: &str) -> u16 {
    let color = |c: Color4u8| MaterialColorEntry {
        color_u8: Some(c),
        color_f32: None,
    };

    materials.materials.push(Material {
        material_name: unique_material_name(materials, base),
        interpolation_colors: MaterialColorInterpolation {
            black_color: color(Color4u8::default()),
            white_color: color(WHITE),
        },
        ..Default::default()
    });

    (materials.materials.len() - 1) as u16
}

pub(crate) fn unique_material_name(materials: &MaterialList, base: &str) -> String {
    let taken = |name: &str| {
        materials
            .materials
            .iter()
            .any(|m| m.material_name.trim_end_matches('\0') == name)
    };

    if !taken(base) {
        return base.to_string();
    }

    (0..)
        .map(|n| {
            let suffix = format!("_{n:02}");
            let end = base.len().min(MATERIAL_NAME_LEN - suffix.len());
            format!("{}{suffix}", &base[..end])
        })
        .find(|name| !taken(name))
        .unwrap()
}

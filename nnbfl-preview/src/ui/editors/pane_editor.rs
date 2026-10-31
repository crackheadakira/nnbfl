use nnbfl::{
    bflyt::{
        file::BflytSection,
        flags::{TexFilter, TexWrapMode},
        pane::{
            AlignmentPane, LineTransform, LineTransformTable, PANE_NAME_LEN, Pane, PartsPane,
            PerCharacterTransform, PicturePane, TextBoxPane, TextureUv, USER_NAME_LEN, WindowFrame,
            WindowPane,
        },
    },
    ui2d::{
        systemdata::{
            Alignment, DropShadow, LayoutData, MaskTexture, PaneData, ProceduralShape, SystemData,
        },
        types::VertexPos,
        userdata::{UserData, UserDataContent},
    },
};

use super::fields::{
    HORIZONTAL, VERTICAL, check, color, color_f, combo, drag, drag_in, floats, grid, index_combo,
    row, section, text, vec2, vec3,
};
use crate::{
    pane_tree::{PaneId, PaneNode},
    traits::Displaying,
    ui::{
        DrawUi, DrawUiWith,
        array_editor::{ArrayEditor, ArrayLayout, ArrayOp, optional},
    },
};

const TEXTURE_FLIPS: &[&str] = &[
    "None",
    "Flip U",
    "Flip V",
    "Rotate 90",
    "Rotate 180",
    "Rotate 270",
];

const BLEND_MODES: &[&str] = &[
    "Normal",
    "Multiply",
    "Addition",
    "Subtraction",
    "Normal (max alpha)",
];

#[derive(Debug, Clone, Default)]
pub struct PaneEditor {
    pub is_editor_visible: bool,
    name: Option<(PaneId, String, String)>,
    rename: Option<String>,
}

pub struct PaneEditorContext<'a> {
    pub name_error: Option<&'a str>,
    pub materials: &'a [String],
    pub fonts: &'a [String],
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PaneEditorOutput {
    pub changed: bool,
    pub reexpand: bool,
}

impl PaneEditor {
    pub fn pending_name(&self, pane: PaneId) -> Option<&str> {
        self.name
            .as_ref()
            .filter(|(id, _, _)| *id == pane)
            .map(|(_, _, typed)| typed.as_str())
    }

    pub fn take_rename(&mut self) -> Option<String> {
        self.rename.take()
    }

    fn name_row(&mut self, ui: &mut egui::Ui, pane: &PaneNode, name_error: Option<&str>) {
        let label = pane.label.trim_end_matches('\0');

        if self
            .name
            .as_ref()
            .is_none_or(|(id, was, _)| *id != pane.pane_idx || was != label)
        {
            self.name = Some((pane.pane_idx, label.to_string(), label.to_string()));
        }

        if pane.parts_source.is_some() {
            ui.label(format!("Name: {label} (from the parts layout)"));
        } else if let Some((_, _, typed)) = &mut self.name {
            ui.horizontal(|ui| {
                ui.label("Name:");
                let response = ui.add(egui::TextEdit::singleline(typed).char_limit(PANE_NAME_LEN));

                let entered =
                    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                let can_rename = typed != label && name_error.is_none();

                if ui
                    .add_enabled(can_rename, egui::Button::new("Rename"))
                    .clicked()
                    || (entered && can_rename)
                {
                    self.rename = Some(typed.clone());
                }
            });

            if typed != label
                && let Some(error) = name_error
            {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        }
    }
}

impl DrawUiWith<(&mut PaneNode, PaneEditorContext<'_>), PaneEditorOutput> for PaneEditor {
    fn draw_with(
        &mut self,
        ui: &mut egui::Ui,
        (pane, ctx): (&mut PaneNode, PaneEditorContext<'_>),
    ) -> PaneEditorOutput {
        let mut out = PaneEditorOutput::default();
        let mut is_open = self.is_editor_visible;

        egui::Window::new("Pane Editor")
            .collapsible(false)
            .resizable(true)
            .default_width(380.0)
            .open(&mut is_open)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        self.name_row(ui, pane, ctx.name_error);
                        ui.weak(format!("{} pane", pane.section.kind_name()));

                        if let Some(source) = &pane.parts_source {
                            ui.weak(format!(
                                "From parts layout {source}; override editing comes later"
                            ));
                        }

                        ui.separator();

                        ui.add_enabled_ui(pane.parts_source.is_none(), |ui| {
                            out.changed |=
                                section_fields(ui, &mut pane.section, &ctx, &mut out.reexpand);

                            out.changed |= section(ui, "User Data", true, |ui| {
                                optional(ui, "User Data", &mut pane.user_data, |ui, array| {
                                    ArrayEditor::new("user_data", &mut array.user_data)
                                        .item_label(|_, entry| entry.o_name.clone())
                                        .show(ui, |ui, _, entry| entry.draw(ui))
                                        .changed
                                })
                            });
                        });
                    });
            });

        self.is_editor_visible = is_open;
        out
    }
}

fn section_fields(
    ui: &mut egui::Ui,
    section: &mut BflytSection,
    ctx: &PaneEditorContext<'_>,
    reexpand: &mut bool,
) -> bool {
    let mut changed = false;

    if let Some(base) = section.get_base_pane_mut() {
        changed |= base_fields(ui, base);
    }

    changed |= match section {
        BflytSection::PicturePane(pic) => picture_fields(ui, pic, ctx.materials),
        BflytSection::TextBoxPane(txt) => text_box_fields(ui, txt, ctx),
        BflytSection::WindowPane(win) => window_fields(ui, win, ctx.materials),
        BflytSection::PartsPane(parts) => {
            let magnified = parts_fields(ui, parts);
            *reexpand |= magnified;
            magnified
        }
        BflytSection::AlignmentPane(align) => alignment_fields(ui, align),
        _ => false,
    };

    changed
}

fn base_fields(ui: &mut egui::Ui, base: &mut Pane) -> bool {
    let mut changed = section(ui, "Pane", true, |ui| {
        grid(ui, "pane_base", |ui| {
            let mut changed = false;

            changed |= text(ui, "User Name", &mut base.user_name, USER_NAME_LEN);
            changed |= check(ui, "Visible", &mut base.pane_flags.is_visible);
            changed |= drag_in(ui, "Alpha", &mut base.alpha, 0..=255);
            changed |= check(
                ui,
                "Influenced Alpha",
                &mut base.pane_flags.influenced_alpha,
            );
            changed |= check(ui, "Location Adjust", &mut base.pane_flags.location_adjust);

            changed |= vec3(ui, "Translate", &mut base.translation, 0.5);
            changed |= vec3(ui, "Rotate", &mut base.rotation, 0.1);
            changed |= vec2(ui, "Scale", &mut base.scale, 0.01);
            changed |= vec2(ui, "Size", &mut base.size, 0.5);

            changed |= combo(ui, "Origin X", &mut base.position.position_x, HORIZONTAL);
            changed |= combo(ui, "Origin Y", &mut base.position.position_y, VERTICAL);
            changed |= combo(
                ui,
                "Parent Origin X",
                &mut base.position.parent_relative_position_x,
                HORIZONTAL,
            );
            changed |= combo(
                ui,
                "Parent Origin Y",
                &mut base.position.parent_relative_position_y,
                VERTICAL,
            );

            changed |= check(
                ui,
                "Ignore Parts Magnify",
                &mut base.flag_ex.is_no_scale_by_parts,
            );
            changed |= check(
                ui,
                "Magnify To Parts Bounds",
                &mut base.flag_ex.is_scale_size_by_parts_root,
            );
            changed |= check(
                ui,
                "User Data Animation",
                &mut base.flag_ex.is_ext_user_data,
            );

            changed
        })
    });

    changed |= section(ui, "Runtime Flags", false, |ui| {
        ui.weak("Set by the runtime; normally left off");

        grid(ui, "pane_runtime", |ui| {
            let flags = &mut base.pane_flags;
            let mut changed = false;

            changed |= check(ui, "User Allocated", &mut flags.user_allocated);
            changed |= check(ui, "Global Matrix Dirty", &mut flags.is_global_matrix_dirty);
            changed |= check(ui, "SRT Matrix User", &mut flags.is_srt_matrix_user);
            changed |= check(ui, "Global Matrix User", &mut flags.is_global_matrix_user);
            changed |= check(
                ui,
                "Constant Buffer Ready",
                &mut flags.is_constant_buffer_ready,
            );
            changed |= drag(ui, "Extended Reserve", &mut base.flag_ex.reserve0, 1.0);

            changed
        })
    });

    changed
}

fn uv_editor(ui: &mut egui::Ui, id: &str, uvs: &mut Vec<TextureUv>) -> bool {
    ArrayEditor::new(id, uvs)
        .title("Texture Coordinates")
        .max(3)
        .item_label(|i, _| format!("Coordinates {}", i + 1))
        .show(ui, |ui, i, uv| {
            grid(ui, (id, i), |ui| {
                let mut changed = false;
                changed |= vec2(ui, "Top Left", &mut uv.top_left, 0.01);
                changed |= vec2(ui, "Top Right", &mut uv.top_right, 0.01);
                changed |= vec2(ui, "Bottom Left", &mut uv.bottom_left, 0.01);
                changed |= vec2(ui, "Bottom Right", &mut uv.bottom_right, 0.01);
                changed
            })
        })
        .changed
}

fn picture_fields(ui: &mut egui::Ui, pic: &mut PicturePane, materials: &[String]) -> bool {
    section(ui, "Picture", true, |ui| {
        let mut changed = grid(ui, "picture", |ui| {
            let mut changed = false;

            changed |= index_combo(ui, "Material", &mut pic.material_index, materials);
            changed |= color(ui, "Top Left Color", &mut pic.top_left_vertex_color);
            changed |= color(ui, "Top Right Color", &mut pic.top_right_vertex_color);
            changed |= color(ui, "Bottom Left Color", &mut pic.bottom_left_vertex_color);
            changed |= color(ui, "Bottom Right Color", &mut pic.bottom_right_vertex_color);

            let mut is_shape = pic.shape_info_array_index.is_some();

            if check(ui, "Shape", &mut is_shape) {
                pic.shape_info_array_index = is_shape.then_some(0);
                changed = true;
            }

            if let Some(index) = &mut pic.shape_info_array_index {
                changed |= drag(ui, "Shape Info Index", index, 1.0);
            }

            pic.is_shape = pic.shape_info_array_index.is_some();
            changed
        });

        changed |= uv_editor(ui, "picture_uvs", &mut pic.texture_uvs);
        changed
    })
}

fn text_box_fields(ui: &mut egui::Ui, txt: &mut TextBoxPane, ctx: &PaneEditorContext<'_>) -> bool {
    let mut changed = section(ui, "Text Box", true, |ui| {
        let mut shown = txt
            .text
            .as_deref()
            .unwrap_or_default()
            .trim_end_matches('\0')
            .to_string();

        let mut changed = false;

        ui.label("Text");

        if ui
            .add(
                egui::TextEdit::multiline(&mut shown)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            )
            .changed()
        {
            set_text(txt, &shown);
            changed = true;
        }

        changed |= grid(ui, "text_box", |ui| {
            let mut changed = false;
            let length = txt.text_length;

            changed |= row(ui, "Buffer Size", |ui| {
                ui.add(egui::DragValue::new(&mut txt.text_buffer_size).range(length..=u16::MAX))
                    .on_hover_text("Bytes reserved for text the game sets at runtime")
                    .changed()
            });

            let mut label = txt.label.clone().unwrap_or_default();

            if text(ui, "Text Label", &mut label, usize::MAX) {
                txt.label = (!label.is_empty()).then_some(label);
                changed = true;
            }

            changed |= index_combo(ui, "Font", &mut txt.font_index, ctx.fonts);
            changed |= index_combo(ui, "Material", &mut txt.material_index, ctx.materials);
            changed |= vec2(ui, "Font Size", &mut txt.font_size, 0.5);
            changed |= drag(ui, "Character Space", &mut txt.character_space, 0.1);
            changed |= drag(ui, "Line Space", &mut txt.line_space, 0.1);
            changed |= drag(ui, "Italic Tilt", &mut txt.italic_tilt, 0.01);
            changed |= color(ui, "Top Color", &mut txt.font_top_color);
            changed |= color(ui, "Bottom Color", &mut txt.font_bottom_color);
            changed |= combo(ui, "Origin X", &mut txt.text_origin.horizontal, HORIZONTAL);
            changed |= combo(ui, "Origin Y", &mut txt.text_origin.vertical, VERTICAL);
            changed |= combo(
                ui,
                "Alignment",
                &mut txt.text_alignment,
                &["Synchronous", "Left", "Center", "Right"],
            );

            changed
        });

        changed
    });

    changed |= section(ui, "Text Flags", false, |ui| {
        grid(ui, "text_flags", |ui| {
            let flags = &mut txt.text_flags;
            let mut changed = false;

            changed |= check(
                ui,
                "Limit Glyphs To Length",
                &mut flags.is_limit_glyph_count_to_length,
            );
            changed |= check(ui, "Invisible Border", &mut flags.is_invisible_border);
            changed |= check(ui, "Double Border", &mut flags.is_double_border);
            changed |= check(ui, "Center Ceiling", &mut flags.is_enable_center_ceiling);
            changed |= check(
                ui,
                "Default Tag Processor",
                &mut flags.is_default_tag_processor,
            );
            changed |= check(ui, "Reverse", &mut flags.is_reverse);
            changed |= check(ui, "Keep Font Scale", &mut flags.is_keeping_font_scale);

            changed
        })
    });

    changed |= section(ui, "Shadow", false, |ui| {
        grid(ui, "text_shadow", |ui| {
            let mut changed = false;

            changed |= check(ui, "Enabled", &mut txt.text_flags.is_enable_shadow);
            changed |= check(ui, "Mix Alpha", &mut txt.text_flags.is_mix_shadow_alpha);
            changed |= vec2(ui, "Offset", &mut txt.shadow_translation, 0.1);
            changed |= vec2(ui, "Scale", &mut txt.shadow_size, 0.01);
            changed |= drag(ui, "Italic Tilt", &mut txt.shadow_italic_tilt, 0.01);
            changed |= color(ui, "Top Color", &mut txt.shadow_top_color);
            changed |= color(ui, "Bottom Color", &mut txt.shadow_bottom_color);

            changed
        })
    });

    changed |= section(ui, "Line Transforms", false, |ui| {
        let changed = optional(
            ui,
            "Line Transforms",
            &mut txt.line_transforms,
            |ui, table: &mut LineTransformTable| {
                ArrayEditor::new("line_transforms", &mut table.entries)
                    .item_label(|i, _| format!("Line {}", i + 1))
                    .show(ui, |ui, i, line: &mut LineTransform| {
                        grid(ui, ("line_transform", i), |ui| {
                            drag(ui, "Offset", &mut line.line_offset, 0.1)
                                | drag(ui, "Width", &mut line.line_width, 0.1)
                        })
                    })
                    .changed
            },
        );

        txt.text_flags.is_line_transform = txt.line_transforms.is_some();
        changed
    });

    changed |= section(ui, "Per-Character Transform", false, |ui| {
        let changed = optional(
            ui,
            "Per-Character Transform",
            &mut txt.per_character_transform,
            |ui, pct: &mut PerCharacterTransform| {
                grid(ui, "per_char", |ui| {
                    let flags = &mut txt.text_flags;
                    let mut changed = false;

                    changed |= drag(ui, "Time Offset", &mut pct.eval_time_offset, 0.1);
                    changed |= drag(ui, "Time Width", &mut pct.eval_time_width, 0.1);
                    changed |= combo(ui, "Loop", &mut pct.loop_type, &["One Time", "Loop"]);
                    changed |= combo(ui, "Origin Y", &mut pct.origin_v, VERTICAL);
                    changed |= drag(ui, "Origin Y Offset", &mut pct.origin_v_offset, 0.1);
                    changed |= drag(ui, "Fixed Space Width", &mut pct.fix_space_width, 0.1);
                    changed |= combo(
                        ui,
                        "Space Origin",
                        &mut pct.fix_space_or_insert_space_origin,
                        HORIZONTAL,
                    );
                    changed |= check(
                        ui,
                        "Split By Char Width",
                        &mut flags.is_per_character_transform_split_by_char_width,
                    );
                    changed |= check(
                        ui,
                        "Origin To Center",
                        &mut flags.is_per_character_transform_origin_to_center,
                    );
                    changed |= check(
                        ui,
                        "Fixed Space",
                        &mut flags.is_per_character_transform_fix_space,
                    );
                    changed |= check(
                        ui,
                        "Split Insert Space",
                        &mut flags.is_per_character_transform_split_insert_space,
                    );

                    row(ui, "Curves", |ui| {
                        ui.weak(if pct.anim_info.is_some() {
                            "Present (not editable here)"
                        } else {
                            "None"
                        });
                        false
                    });

                    changed
                })
            },
        );

        txt.text_flags.is_per_character_transform = txt.per_character_transform.is_some();
        changed
    });

    changed
}

pub fn set_text(txt: &mut TextBoxPane, shown: &str) {
    let units = shown.encode_utf16().take(u16::MAX as usize / 2 - 1).count();
    let shown: String =
        String::from_utf16_lossy(&shown.encode_utf16().take(units).collect::<Vec<_>>());
    let length = ((units + 1) * 2) as u16;

    txt.text = Some(format!("{shown}\0"));
    txt.text_length = length;
    txt.text_buffer_size = txt.text_buffer_size.max(length);
}

fn window_fields(ui: &mut egui::Ui, win: &mut WindowPane, materials: &[String]) -> bool {
    let mut changed = section(ui, "Window", true, |ui| {
        grid(ui, "window", |ui| {
            let mut changed = false;

            changed |= combo(
                ui,
                "Kind",
                &mut win.flag.window_kind,
                &["Around", "Horizontal", "Horizontal (no content)"],
            );
            changed |= check(ui, "Hide Content", &mut win.flag.not_draw_content);
            changed |= check(
                ui,
                "Left Corner Material",
                &mut win.flag.use_left_corner_material,
            );
            changed |= check(
                ui,
                "Vertex Color For All",
                &mut win.flag.use_vertex_color_for_all_window,
            );

            changed |= row(ui, "Inflation L R T B", |ui| {
                inset(
                    ui,
                    [
                        &mut win.inflation_left,
                        &mut win.inflation_right,
                        &mut win.inflation_top,
                        &mut win.inflation_bottom,
                    ],
                )
            });

            changed |= row(ui, "Frame Size L R T B", |ui| {
                inset(
                    ui,
                    [
                        &mut win.frame_size_left,
                        &mut win.frame_size_right,
                        &mut win.frame_size_top,
                        &mut win.frame_size_bottom,
                    ],
                )
            });

            changed
        })
    });

    changed |= section(ui, "Window Content", false, |ui| {
        let content = &mut win.content;

        let mut changed = grid(ui, "window_content", |ui| {
            let mut changed = false;

            changed |= index_combo(ui, "Material", &mut content.material_index, materials);
            changed |= color(ui, "Top Left Color", &mut content.top_left_vertex_color);
            changed |= color(ui, "Top Right Color", &mut content.top_right_vertex_color);
            changed |= color(
                ui,
                "Bottom Left Color",
                &mut content.bottom_left_vertex_color,
            );
            changed |= color(
                ui,
                "Bottom Right Color",
                &mut content.bottom_right_vertex_color,
            );

            changed
        });

        changed |= uv_editor(ui, "window_uvs", &mut content.picture_uvs);
        changed
    });

    changed |= section(ui, "Window Frames", false, |ui| {
        ArrayEditor::new("window_frames", &mut win.frames)
            .max(8)
            .item_label(|i, _| format!("Frame {}", i + 1))
            .show(ui, |ui, i, frame: &mut WindowFrame| {
                grid(ui, ("window_frame", i), |ui| {
                    index_combo(ui, "Material", &mut frame.material_index, materials)
                        | combo(ui, "Flip", &mut frame.texture_flip_mode, TEXTURE_FLIPS)
                })
            })
            .changed
    });

    changed
}

fn inset(ui: &mut egui::Ui, values: [&mut i16; 4]) -> bool {
    ui.horizontal(|ui| {
        let mut changed = false;

        for value in values {
            changed |= ui.add(egui::DragValue::new(value)).changed();
        }

        changed
    })
    .inner
}

fn parts_fields(ui: &mut egui::Ui, parts: &mut PartsPane) -> bool {
    section(ui, "Parts", true, |ui| {
        grid(ui, "parts", |ui| {
            let mut changed = false;

            row(ui, "Layout", |ui| {
                ui.label(&parts.o_layout_name);
                false
            });

            changed |= drag(ui, "Magnify X", &mut parts.magnify_x, 0.01);
            changed |= drag(ui, "Magnify Y", &mut parts.magnify_y, 0.01);

            row(ui, "Overrides", |ui| {
                ui.label(parts.properties.len().to_string());
                false
            });

            changed
        })
    })
}

fn alignment_fields(ui: &mut egui::Ui, align: &mut AlignmentPane) -> bool {
    section(ui, "Alignment", true, |ui| {
        ui.weak("The preview doesn't apply alignment yet");

        grid(ui, "alignment", |ui| {
            let mut changed = false;

            changed |= drag(ui, "Direction", &mut align.direction, 1.0);
            changed |= drag(ui, "Default Margin", &mut align.default_margin, 0.1);
            changed |= check(ui, "Align Last Pane", &mut align.is_align_last_pane);
            changed |= check(ui, "Vertical", &mut align.is_vertical_alignment);

            changed
        })
    })
}

impl DrawUi for UserData {
    fn draw(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.horizontal(|ui| {
            changed |= ui
                .add(egui::TextEdit::singleline(&mut self.o_name))
                .changed();

            let current = match &self.content {
                UserDataContent::String(_) => 0,
                UserDataContent::S32(_) => 1,
                UserDataContent::Float(_) => 2,
                UserDataContent::SystemData(_) => 3,
            };

            let names = ["String", "Int Array", "Float Array", "System Data Array"];

            egui::ComboBox::from_id_salt(ui.next_auto_id())
                .selected_text(names[current])
                .show_ui(ui, |ui| {
                    for (i, name) in names.iter().enumerate() {
                        if ui.selectable_label(i == current, *name).clicked() && i != current {
                            self.content = match i {
                                0 => UserDataContent::String(Default::default()),
                                1 => UserDataContent::S32(Default::default()),
                                2 => UserDataContent::Float(Default::default()),
                                _ => UserDataContent::SystemData(Default::default()),
                            };
                            changed = true;
                        }
                    }
                });
        });

        ui.add_space(4.0);

        changed |= match &mut self.content {
            UserDataContent::String(string) => ui.add(egui::TextEdit::singleline(string)).changed(),
            UserDataContent::S32(items) => {
                ArrayEditor::new("s32", items)
                    .layout(ArrayLayout::Inline)
                    .show(ui, |ui, _, item| {
                        ui.add(egui::DragValue::new(item)).changed()
                    })
                    .changed
            }

            UserDataContent::Float(items) => {
                ArrayEditor::new("f32", items)
                    .layout(ArrayLayout::Inline)
                    .show(ui, |ui, _, item| {
                        ui.add(egui::DragValue::new(item).speed(0.01)).changed()
                    })
                    .changed
            }

            UserDataContent::SystemData(groups) => {
                ArrayEditor::new("system_data_groups", groups)
                    .item_label(|i, _| format!("Group {}", i + 1))
                    .show(ui, |ui, i, group| {
                        let response = ArrayEditor::new(("system_data", i), group)
                            .item_label(|_, data| system_data_name(data).to_string())
                            .show(ui, |ui, _, data| data.draw(ui));

                        if let Some(ArrayOp::Added(at)) = response.op {
                            group[at] = new_system_data(SYSTEM_DATA_KINDS[0]);
                        }

                        response.changed
                    })
                    .changed
            }
        };

        changed
    }
}

const SYSTEM_DATA_KINDS: [&str; 6] = [
    "Vertex Position 0",
    "Vertex Position 1",
    "Procedural Shape",
    "Alignment",
    "Drop Shadow",
    "Mask Texture",
];

fn new_system_data(kind: &str) -> SystemData {
    SystemData::Pane(match kind {
        "Vertex Position 1" => PaneData::VertexPos1(Default::default()),
        "Procedural Shape" => PaneData::ProceduralShape(Default::default()),
        "Alignment" => PaneData::Alignment(Default::default()),
        "Drop Shadow" => PaneData::DropShadow(Default::default()),
        "Mask Texture" => PaneData::MaskTexture(Default::default()),
        _ => PaneData::VertexPos0(Default::default()),
    })
}

fn system_data_name(data: &SystemData) -> &'static str {
    match data {
        SystemData::Layout(LayoutData::AnimTagName(_)) => "Animation Tag Names",
        SystemData::Layout(LayoutData::Invalid) => "Invalid Layout Data",
        SystemData::Pane(pane) => pane_data_name(pane),
    }
}

fn pane_data_name(data: &PaneData) -> &'static str {
    match data {
        PaneData::VertexPos0(_) => "Vertex Position 0",
        PaneData::VertexPos1(_) => "Vertex Position 1",
        PaneData::ProceduralShape(_) => "Procedural Shape",
        PaneData::Alignment(_) => "Alignment",
        PaneData::DropShadow(_) => "Drop Shadow",
        PaneData::MaskTexture(_) => "Mask Texture",
    }
}

impl DrawUi for SystemData {
    fn draw(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        let current = system_data_name(self);

        ui.horizontal(|ui| {
            ui.label("Type:");

            egui::ComboBox::from_id_salt(ui.next_auto_id())
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for name in SYSTEM_DATA_KINDS {
                        if ui.selectable_label(current == name, name).clicked() && current != name {
                            *self = new_system_data(name);
                            changed = true;
                        }
                    }
                });
        });

        let id = ui.next_auto_id();

        changed |= match self {
            SystemData::Layout(LayoutData::AnimTagName(names)) => {
                ArrayEditor::new(("anim_tag_names", id), names)
                    .layout(ArrayLayout::Inline)
                    .show(ui, |ui, _, name| {
                        ui.add(egui::TextEdit::singleline(name).desired_width(120.0))
                            .changed()
                    })
                    .changed
            }
            SystemData::Layout(LayoutData::Invalid) => false,
            SystemData::Pane(PaneData::VertexPos0(pos) | PaneData::VertexPos1(pos)) => {
                grid(ui, id, |ui| vertex_pos_fields(ui, pos))
            }
            SystemData::Pane(PaneData::ProceduralShape(shape)) => {
                grid(ui, id, |ui| procedural_shape_fields(ui, shape))
            }
            SystemData::Pane(PaneData::Alignment(align)) => {
                grid(ui, id, |ui| alignment_data_fields(ui, align))
            }
            SystemData::Pane(PaneData::DropShadow(shadow)) => {
                grid(ui, id, |ui| drop_shadow_fields(ui, shadow))
            }
            SystemData::Pane(PaneData::MaskTexture(mask)) => {
                grid(ui, id, |ui| mask_texture_fields(ui, mask))
            }
        };

        changed
    }
}

fn vertex_pos_fields(ui: &mut egui::Ui, pos: &mut VertexPos) -> bool {
    drag(ui, "Size Scale Width", &mut pos.size_scale_width, 0.01)
        | drag(ui, "Size Scale Height", &mut pos.size_scale_height, 0.01)
        | drag(ui, "Position X Scale", &mut pos.position_x_scale, 0.01)
        | drag(ui, "Position Y Scale", &mut pos.position_y_scale, 0.01)
}

fn alignment_data_fields(ui: &mut egui::Ui, align: &mut Alignment) -> bool {
    drag(ui, "Options", &mut align.options, 1.0) | drag(ui, "Margin", &mut align.margin, 0.1)
}

fn procedural_shape_fields(ui: &mut egui::Ui, s: &mut ProceduralShape) -> bool {
    let mut changed = false;

    changed |= drag(ui, "Options", &mut s.options, 1.0);
    changed |= drag(ui, "Color Options", &mut s.color0_options, 1.0);
    changed |= drag(ui, "Inner Shadow Options", &mut s.inner_shadow_options, 1.0);
    changed |= drag(ui, "Inner Shadow Base", &mut s.inner_shadow_base_comp, 1.0);

    changed |= drag(
        ui,
        "Color Overlay Options",
        &mut s.color_overlay_options,
        1.0,
    );

    changed |= drag(
        ui,
        "Gradation Options",
        &mut s.gradation_overlay_options,
        1.0,
    );

    changed |= drag(ui, "Drop Shadow Blend", &mut s.drop_shadow_blend_mode, 1.0);
    changed |= drag(ui, "Drop Shadow Base", &mut s.drop_shadow_base_comp, 1.0);
    changed |= floats(ui, "Rounded Corners 0", &mut s.rounded_corner0, 0.1);
    changed |= floats(ui, "Rounded Corners 1", &mut s.rounded_corner1, 0.1);
    changed |= drag(ui, "Inner Stroke Size", &mut s.inner_stroke_size, 0.1);
    changed |= color_f(ui, "Color", &mut s.color0);
    changed |= color_f(ui, "Inner Shadow Color", &mut s.inner_shadow_color);

    changed |= floats(
        ui,
        "Inner Shadow Transform",
        &mut s.inner_shadow_transform,
        0.1,
    );

    changed |= color_f(ui, "Color Overlay", &mut s.color_overlay);
    changed |= floats(ui, "Gradation Weights", &mut s.gradation_weights, 0.01);

    for (i, c) in s.gradation_color_array.iter_mut().enumerate() {
        changed |= color_f(ui, &format!("Gradation Color {}", i + 1), c);
    }

    changed |= drag(ui, "Gradation Rotation", &mut s.gradation_rotation, 0.1);
    changed |= color_f(ui, "Drop Shadow Color", &mut s.drop_shadow_color);
    changed |= floats(
        ui,
        "Drop Shadow Transform",
        &mut s.drop_shadow_transform,
        0.1,
    );

    changed
}

fn wrap_filter(
    ui: &mut egui::Ui,
    label: &str,
    wrap: &mut TexWrapMode,
    filter: &mut TexFilter,
) -> bool {
    combo(
        ui,
        &format!("{label} Wrap"),
        wrap,
        &["Clamp", "Repeat", "Mirror"],
    ) | combo(ui, &format!("{label} Filter"), filter, &["Near", "Linear"])
}

fn drop_shadow_fields(ui: &mut egui::Ui, d: &mut DropShadow) -> bool {
    let mut changed = false;

    changed |= drag(ui, "Texture Index", &mut d.texture_id, 1.0);

    changed |= wrap_filter(
        ui,
        "U",
        &mut d.u_options.wrap_mode,
        &mut d.u_options.filter_mode,
    );

    changed |= wrap_filter(
        ui,
        "V",
        &mut d.v_options.wrap_mode,
        &mut d.v_options.filter_mode,
    );

    changed |= drag(ui, "Max Size", &mut d.max_size, 1.0);

    changed |= check(ui, "Stroke", &mut d.flags.is_stroke_enabled);
    changed |= combo(ui, "Stroke Blend", &mut d.stroke_blend_mode, BLEND_MODES);
    changed |= drag(ui, "Stroke Size", &mut d.stroke_size, 0.1);
    changed |= color_f(ui, "Stroke Color", &mut d.stroke_color);

    changed |= check(ui, "Outer Glow", &mut d.flags.is_outer_glow_enabled);

    changed |= combo(
        ui,
        "Outer Glow Blend",
        &mut d.outer_glow_blend_mode,
        BLEND_MODES,
    );

    changed |= color_f(ui, "Outer Glow Color", &mut d.outer_glow_color);
    changed |= drag(ui, "Outer Glow Spread", &mut d.outer_glow_spread, 0.1);
    changed |= drag(ui, "Outer Glow Size", &mut d.outer_glow_size, 0.1);

    changed |= check(ui, "Drop Shadow", &mut d.flags.is_drop_shadow_enabled);

    changed |= combo(
        ui,
        "Drop Shadow Blend",
        &mut d.drop_shadow_blend_mode,
        BLEND_MODES,
    );

    changed |= color_f(ui, "Drop Shadow Color", &mut d.drop_shadow_color);
    changed |= drag(ui, "Drop Shadow Angle", &mut d.drop_shadow_angle, 0.5);
    changed |= drag(ui, "Drop Shadow Distance", &mut d.drop_shadow_distance, 0.1);
    changed |= drag(ui, "Drop Shadow Spread", &mut d.drop_shadow_spread, 0.1);
    changed |= drag(ui, "Drop Shadow Size", &mut d.drop_shadow_size, 0.1);

    changed |= check(ui, "Knockout", &mut d.flags.is_knockout);
    changed |= check(ui, "Effect Only", &mut d.flags.is_only_effect);
    changed |= check(ui, "Static Rendering", &mut d.flags.is_static_rendering);
    changed |= check(ui, "Degamma", &mut d.flags.is_degamma_enabled);

    changed
}

fn mask_texture_fields(ui: &mut egui::Ui, m: &mut MaskTexture) -> bool {
    let mut changed = false;

    changed |= check(ui, "Mask With Pane", &mut m.flags.mask_with_pane);
    changed |= check(ui, "Static Rendering", &mut m.flags.mask_static_rendering);
    changed |= drag(ui, "Texture Index", &mut m.texture_id, 1.0);
    changed |= wrap_filter(ui, "U", &mut m.u_options.wrap_mode, &mut m.u_options.filter);
    changed |= wrap_filter(ui, "V", &mut m.v_options.wrap_mode, &mut m.v_options.filter);
    changed |= drag(ui, "Texture Ext Flags", &mut m.tex_ext_flags, 1.0);
    changed |= check(ui, "Use Capture Mask", &mut m.is_use_capture_mask);
    changed |= drag(ui, "Capture Texture Index", &mut m.capture_texture_id, 1.0);

    changed |= wrap_filter(
        ui,
        "Capture U",
        &mut m.capture_u_options.wrap_mode,
        &mut m.capture_u_options.filter,
    );

    changed |= wrap_filter(
        ui,
        "Capture V",
        &mut m.capture_v_options.wrap_mode,
        &mut m.capture_v_options.filter,
    );

    changed |= floats(ui, "Translate", &mut m.translation, 0.01);
    changed |= drag(ui, "Rotate", &mut m.rotation, 0.1);
    changed |= floats(ui, "Scale", &mut m.scale, 0.01);

    changed
}

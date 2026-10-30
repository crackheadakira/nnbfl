use nnbfl::{
    bflyt::list::{
        BlendFactor, BlendOp, CombinerTevMode, DetailedCombinerAlphaStageConfig,
        DetailedCombinerColorStageConfig, DetailedCombinerStageMode, LogicOp, MATERIAL_NAME_LEN,
        Material, MaterialBlendMode, MaterialList, MaterialTextureOptions, TevAlphaOp, TevColorOp,
        TevKonstSel, TevScale, TevSource, TextureList,
    },
    ui2d::types::{Color4f, Color4u8, Vector2f},
};

use std::collections::HashMap;

use crate::{
    pane_tree::{PaneId, material_name_error},
    ui::{
        DrawUi, DrawUiWith,
        array_editor::{ArrayEditor, ArrayLayout, optional},
        general::UiAction,
    },
};

#[derive(Default)]
pub struct MaterialEditor {
    pub selected_material: usize,
    pub is_editor_visible: bool,
    name: Option<(usize, String, String)>,
}

pub struct MaterialEditorContext<'a> {
    pub list: &'a mut MaterialList,
    pub textures: &'a mut Option<TextureList>,
    pub bntx_textures: &'a [String],
    pub users: &'a HashMap<u16, Vec<PaneId>>,
}

#[derive(Default)]
pub struct MaterialEditorOutput {
    pub changed: bool,
    pub action: Option<UiAction>,
}

pub struct TexturePicker<'a> {
    pub textures: &'a mut Option<TextureList>,
    pub bntx_textures: &'a [String],
}

impl MaterialEditor {
    pub fn draw(&mut self, ui: &mut egui::Ui, ctx: MaterialEditorContext) -> MaterialEditorOutput {
        let MaterialEditorContext {
            list,
            textures,
            bntx_textures,
            users,
        } = ctx;

        let mut output = MaterialEditorOutput::default();
        let mut open = self.is_editor_visible;

        egui::Window::new("Material Editor")
            .collapsible(false)
            .resizable(true)
            .open(&mut open)
            .show(ui, |ui| {
                egui::Panel::left("mat_editor_list")
                    .resizable(true)
                    .min_size(144.0)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .auto_shrink(false)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.weak(format!("{} Total Materials", list.materials.len()));

                                    if ui.button("➕ Add").clicked() {
                                        output.action = Some(UiAction::AddMaterial);
                                    }
                                });

                                ui.separator();

                                for (idx, material) in list.materials.iter().enumerate() {
                                    let name = material.material_name.trim_end_matches('\0');
                                    let unused = users.get(&(idx as u16)).is_none_or(Vec::is_empty);

                                    let text = if unused {
                                        egui::RichText::new(format!("[{}] {name}", idx + 1)).weak()
                                    } else {
                                        egui::RichText::new(format!("[{}] {name}", idx + 1))
                                    };

                                    if ui
                                        .selectable_label(self.selected_material == idx, text)
                                        .clicked()
                                    {
                                        self.selected_material = idx;
                                    }
                                }
                            })
                    });

                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        egui::Frame::new()
                            .inner_margin(egui::Margin {
                                left: 12,
                                right: 4,
                                top: 4,
                                bottom: 4,
                            })
                            .show(ui, |ui| {
                                let index = self.selected_material;

                                let Some(material) = list.materials.get(index) else {
                                    return;
                                };

                                let current = material.material_name.trim_end_matches('\0');

                                if self
                                    .name
                                    .as_ref()
                                    .is_none_or(|(i, was, _)| *i != index || was != current)
                                {
                                    self.name = Some((index, current.into(), current.into()));
                                }

                                if let Some(action) = self.header(ui, list, users, index) {
                                    output.action = Some(action);
                                }

                                ui.separator();

                                if let Some(material) = list.materials.get_mut(index) {
                                    output.changed |= material.draw_with(
                                        ui,
                                        TexturePicker {
                                            textures,
                                            bntx_textures,
                                        },
                                    );
                                }
                            });
                    })
            });

        self.is_editor_visible = open;
        output
    }

    fn header(
        &mut self,
        ui: &mut egui::Ui,
        list: &MaterialList,
        users: &HashMap<u16, Vec<PaneId>>,
        index: usize,
    ) -> Option<UiAction> {
        let mut action = None;
        let (_, current, typed) = self.name.as_mut()?;
        let error = material_name_error(Some(list), typed, Some(index as u16));

        ui.horizontal(|ui| {
            ui.label("Name:");
            let response = ui.add(egui::TextEdit::singleline(typed).char_limit(MATERIAL_NAME_LEN));
            let entered = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let can_rename = typed != current && error.is_none();

            if ui
                .add_enabled(can_rename, egui::Button::new("Rename"))
                .clicked()
                || (entered && can_rename)
            {
                action = Some(UiAction::RenameMaterial {
                    index: index as u16,
                    name: typed.clone(),
                });
            }
        });

        if typed != current
            && let Some(error) = &error
        {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }

        let used_by = users.get(&(index as u16)).map_or(0, Vec::len);

        ui.horizontal(|ui| {
            ui.weak(match used_by {
                0 => "Not used by any pane".to_string(),
                1 => "Used by 1 pane".to_string(),
                n => format!("Used by {n} panes"),
            });

            if ui.button("📋 Duplicate").clicked() {
                action = Some(UiAction::DuplicateMaterial(index as u16));
            }

            if ui
                .add_enabled(used_by == 0, egui::Button::new("🗑 Delete"))
                .on_disabled_hover_text("Assign other materials to its panes first")
                .clicked()
            {
                action = Some(UiAction::DeleteMaterial(index as u16));
            }
        });

        action
    }
}

impl DrawUiWith<TexturePicker<'_>> for Material {
    fn draw_with(&mut self, ui: &mut egui::Ui, mut picker: TexturePicker) -> bool {
        let mut changed = false;

        changed |= ui
            .checkbox(&mut self.use_texture_only, "Use Texture Only")
            .changed();

        changed |= ui
            .checkbox(
                &mut self.use_thresholding_alpha_interpolation,
                "Use Thresholding Alpha Interpolation",
            )
            .changed();

        ui.horizontal(|ui| {
            ui.heading("Interpolation Colors");
        });

        ui.horizontal(|ui| {
            if let Some(ref mut color) = self.interpolation_colors.black_color.color_f32 {
                changed |= color.draw_with(ui, "Black Color")
            } else if let Some(ref mut color) = self.interpolation_colors.black_color.color_u8 {
                changed |= color.draw_with(ui, "Black Color")
            }
        });

        ui.horizontal(|ui| {
            if let Some(ref mut color) = self.interpolation_colors.white_color.color_f32 {
                changed |= color.draw_with(ui, "White Color")
            } else if let Some(ref mut color) = self.interpolation_colors.white_color.color_u8 {
                changed |= color.draw_with(ui, "White Color")
            }
        });

        ui.separator();
        let tex_count = self.tex_maps.len();
        let maps = ArrayEditor::new("tex_maps", &mut self.tex_maps)
            .title("Texture Maps")
            .item_label(|i, _| format!("Texture {}", i + 1))
            .show(ui, |ui, idx, tex_map| {
                let mut changed = texture_combo(ui, idx, &mut tex_map.texture_name, &mut picker);
                ui.horizontal(|ui| {
                    changed |= tex_map.u_options.draw_with(ui, ("U", idx));
                    changed |= tex_map.v_options.draw_with(ui, ("V", idx));
                });

                if let Some(tex_ext) = self.tex_extensions.get_mut(idx) {
                    ui.horizontal(|ui| {
                        changed |= ui
                            .checkbox(&mut tex_ext.is_capture_texture, "Capture Texture")
                            .changed();
                        changed |= ui
                            .checkbox(&mut tex_ext.is_vector_texture, "Vector Texture")
                            .changed();
                    });
                }

                changed
            });

        changed |= maps.changed;

        if let Some(op) = maps.op {
            let grow = !self.tex_extensions.is_empty() && self.tex_extensions.len() == tex_count;

            op.mirror(&mut self.tex_extensions, grow);
            let srts_parallel = self.tex_srts.len() == tex_count;
            op.mirror(&mut self.tex_srts, srts_parallel);
            let gens_parallel = self.tex_coord_gens.len() == tex_count;
            op.mirror(&mut self.tex_coord_gens, gens_parallel);
            op.mirror(&mut self.projection_tex_gens, false);
        }

        let tex_count = self.tex_maps.len();

        ui.separator();
        changed |= ArrayEditor::new("tex_srts", &mut self.tex_srts)
            .title("Texture SRTs")
            .max(tex_count)
            .layout(ArrayLayout::Cards)
            .item_label(texture_label)
            .show(ui, |ui, idx, tex_srt| {
                let mut changed = false;
                egui::Grid::new(format!("tex_srt_grid_{idx}"))
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        for (label, value, speed) in [
                            ("Rotate", &mut tex_srt.rotate, 0.1),
                            ("Translate U", &mut tex_srt.translate_u, 0.5),
                            ("Translate V", &mut tex_srt.translate_v, 0.5),
                            ("Scale U", &mut tex_srt.scale_u, 0.01),
                            ("Scale V", &mut tex_srt.scale_v, 0.01),
                        ] {
                            ui.label(label);
                            changed |= ui.add(egui::DragValue::new(value).speed(speed)).changed();
                            ui.end_row();
                        }
                    });

                changed
            })
            .changed;

        ui.separator();
        changed |= ArrayEditor::new("tex_coord_gens", &mut self.tex_coord_gens)
            .title("Texture Coordinate Generators")
            .max(tex_count)
            .item_label(texture_label)
            .show(ui, |ui, idx, tex_coord_gen| {
                let mut changed = false;
                egui::ComboBox::new(
                    format!("tex_coord_gen_{idx}"),
                    format!("Texture {} Source", idx + 1),
                )
                .selected_text(format!("{:?}", tex_coord_gen.tex_gen_source))
                .show_ui(ui, |ui| {
                    use nnbfl::bflyt::list::TexGenSrc;

                    changed |= ui
                        .selectable_value(
                            &mut tex_coord_gen.tex_gen_source,
                            TexGenSrc::Tex0,
                            "Texture 0",
                        )
                        .clicked();

                    changed |= ui
                        .selectable_value(
                            &mut tex_coord_gen.tex_gen_source,
                            TexGenSrc::Tex1,
                            "Texture 1",
                        )
                        .clicked();

                    changed |= ui
                        .selectable_value(
                            &mut tex_coord_gen.tex_gen_source,
                            TexGenSrc::Tex2,
                            "Texture 2",
                        )
                        .clicked();

                    changed |= ui
                        .selectable_value(
                            &mut tex_coord_gen.tex_gen_source,
                            TexGenSrc::OrthogonalProjection,
                            "Orthogonal Projection",
                        )
                        .clicked();

                    changed |= ui
                        .selectable_value(
                            &mut tex_coord_gen.tex_gen_source,
                            TexGenSrc::PaneBasedOrthogonalProjection,
                            "Pane Based Orthogonal Projection",
                        )
                        .clicked();

                    changed |= ui
                        .selectable_value(
                            &mut tex_coord_gen.tex_gen_source,
                            TexGenSrc::PaneBasedPerspectiveProjection,
                            "Pane Based Perspective Projection",
                        )
                        .clicked();

                    changed |= ui
                        .selectable_value(
                            &mut tex_coord_gen.tex_gen_source,
                            TexGenSrc::PerspectiveProjection,
                            "Perspective Projection",
                        )
                        .clicked();

                    changed |= ui
                        .selectable_value(
                            &mut tex_coord_gen.tex_gen_source,
                            TexGenSrc::BrickRepeat,
                            "Brick Repeat",
                        )
                        .clicked();
                });

                changed
            })
            .changed;

        ui.separator();
        changed |= ArrayEditor::new("projection_tex_gens", &mut self.projection_tex_gens)
            .title("Projection Texture Generators")
            .max(tex_count)
            .item_label(texture_label)
            .show(ui, |ui, _, proj_tex_gen| {
                let mut changed = false;
                changed |= ui
                    .checkbox(
                        &mut proj_tex_gen.flags.fitting_layout_size,
                        "Fitting Layout Size",
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut proj_tex_gen.flags.fitting_pane_size,
                        "Fitting Pane Size",
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut proj_tex_gen.flags.adjust_projection_scale_rotate,
                        "Adjust Projection, Scale & Rotate",
                    )
                    .changed();

                changed |= proj_tex_gen.translation.draw_with(ui, "Translation");
                ui.add_space(4.0);
                changed |= proj_tex_gen.scale.draw_with(ui, "Scale");
                changed
            })
            .changed;

        ui.separator();
        changed |= ArrayEditor::new("tev_combiners", &mut self.tev_combiners)
            .title("Texture Environment Unit Combiners")
            .max(tex_count / 2)
            .item_label(|i, _| format!("Combiner {}", i + 1))
            .show(ui, |ui, idx, tev_combiner| {
                tev_combiner.alpha_mode.draw_with(ui, ("Alpha Mode", idx))
                    | tev_combiner.rgb_mode.draw_with(ui, ("RGB Mode", idx))
            })
            .changed;

        ui.separator();
        changed |= optional(
            ui,
            "Alpha Compare",
            &mut self.alpha_compare,
            |ui, alpha_compare| {
                let mut changed = false;
                ui.label("Reference Value");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut alpha_compare.alpha_compare_ref_value)
                            .speed(0.5)
                            .range(0.0..=1.0),
                    )
                    .changed();

                egui::ComboBox::new("mat_editor_alpha_compare_op", "Compare Operand")
                    .selected_text(format!("{:?}", alpha_compare.compare))
                    .show_ui(ui, |ui| {
                        use nnbfl::bflyt::list::AlphaCompare;

                        changed |= ui
                            .selectable_value(
                                &mut alpha_compare.compare,
                                AlphaCompare::Never,
                                "Never",
                            )
                            .clicked();

                        changed |= ui
                            .selectable_value(
                                &mut alpha_compare.compare,
                                AlphaCompare::Less,
                                "Less",
                            )
                            .clicked();

                        changed |= ui
                            .selectable_value(
                                &mut alpha_compare.compare,
                                AlphaCompare::LessThanEqual,
                                "Less Than Equal",
                            )
                            .clicked();

                        changed |= ui
                            .selectable_value(
                                &mut alpha_compare.compare,
                                AlphaCompare::Equal,
                                "Equal",
                            )
                            .clicked();

                        changed |= ui
                            .selectable_value(
                                &mut alpha_compare.compare,
                                AlphaCompare::NotEqual,
                                "Not Equal",
                            )
                            .clicked();

                        changed |= ui
                            .selectable_value(
                                &mut alpha_compare.compare,
                                AlphaCompare::GreaterThanEqual,
                                "Greater Than Equal",
                            )
                            .clicked();

                        changed |= ui
                            .selectable_value(
                                &mut alpha_compare.compare,
                                AlphaCompare::Greater,
                                "Greater",
                            )
                            .clicked();

                        changed |= ui
                            .selectable_value(
                                &mut alpha_compare.compare,
                                AlphaCompare::Always,
                                "Always",
                            )
                            .clicked();
                    });
                changed
            },
        );

        ui.separator();
        changed |= optional(ui, "Blend Mode", &mut self.blend_mode, |ui, blend_mode| {
            blend_mode.draw_with(ui, "blend_mode")
        });

        ui.separator();
        changed |= optional(
            ui,
            "Alpha Blend Mode",
            &mut self.blend_mode_alpha,
            |ui, blend_mode| blend_mode.draw_with(ui, "blend_mode_alpha"),
        );

        ui.separator();
        changed |= optional(
            ui,
            "Indirect Matrix",
            &mut self.indirect_matrix,
            |ui, indirect_matrix| {
                ui.label("Rotation");
                ui.add(egui::DragValue::new(&mut indirect_matrix.rotation).speed(0.5))
                    .changed()
                    | indirect_matrix.scale.draw_with(ui, "Scale")
            },
        );

        ui.separator();
        changed |= optional(
            ui,
            "Font Shadow Color",
            &mut self.font_shadow_color,
            |ui, shadow| {
                shadow.black_color.draw_with(ui, "Black Color")
                    | shadow.white_color.draw_with(ui, "White Color")
            },
        );

        ui.separator();
        changed |= optional(
            ui,
            "Detailed Combiner",
            &mut self.detailed_combiner,
            |ui, detailed| {
                let mut changed = false;
                changed |= detailed.color1.draw_with(ui, "Color 1");
                changed |= detailed.color2.draw_with(ui, "Color 2");
                changed |= detailed.color3.draw_with(ui, "Color 3");
                changed |= detailed.color4.draw_with(ui, "Color 4");
                changed |= detailed.color5.draw_with(ui, "Color 5");

                changed |= ArrayEditor::new("detailed_entries", &mut detailed.entries)
                    .title("Combiners")
                    .layout(ArrayLayout::Cards)
                    .item_label(|i, _| format!("Combiner {}", i + 1))
                    .show(ui, |ui, _, entry| {
                        ui.set_min_width(150.0);
                        ui.label("Alpha Config");
                        let alpha = entry.alpha_config.draw(ui);
                        ui.add_space(12.0);
                        ui.label("Color Config");
                        alpha | entry.color_config.draw(ui)
                    })
                    .changed;

                changed
            },
        );

        changed
    }
}

impl DrawUiWith<&str> for Color4f {
    fn draw_with(&mut self, ui: &mut egui::Ui, label: &str) -> bool {
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.label(label);

            let mut color_array = (*self).into();

            if ui
                .color_edit_button_rgba_unmultiplied(&mut color_array)
                .changed()
            {
                *self = color_array.into();
                changed = true;
            }
        });

        changed
    }
}

impl DrawUiWith<&str> for Color4u8 {
    fn draw_with(&mut self, ui: &mut egui::Ui, label: &str) -> bool {
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.label(label);

            let mut color_array = (*self).into();

            if ui
                .color_edit_button_rgba_unmultiplied(&mut color_array)
                .changed()
            {
                *self = color_array.into();
                changed = true;
            }
        });

        changed
    }
}

impl DrawUiWith<&str> for Vector2f {
    fn draw_with(&mut self, ui: &mut egui::Ui, label: &str) -> bool {
        let mut changed = false;

        ui.label(label);
        ui.horizontal(|ui| {
            ui.weak("X");
            changed |= ui
                .add(egui::DragValue::new(&mut self.x).speed(0.1))
                .changed();

            ui.add_space(4.0);

            ui.weak("Y");
            changed |= ui
                .add(egui::DragValue::new(&mut self.y).speed(0.1))
                .changed();
        });

        changed
    }
}

impl DrawUiWith<(&str, usize)> for MaterialTextureOptions {
    fn draw_with(&mut self, ui: &mut egui::Ui, state: (&str, usize)) -> bool {
        let (prefix, idx) = state;
        let mut changed = false;

        ui.vertical(|ui| {
            {
                let combo_id = format!("tex_map_{}_filter_{idx}", prefix.to_lowercase());
                let combo_label = format!("{prefix} Filter");

                egui::ComboBox::new(combo_id, combo_label)
                    .selected_text(format!("{:?}", self.filter))
                    .show_ui(ui, |ui| {
                        use nnbfl::bflyt::flags::TexFilter;

                        changed |= ui
                            .selectable_value(&mut self.filter, TexFilter::Linear, "Linear")
                            .clicked();

                        changed |= ui
                            .selectable_value(&mut self.filter, TexFilter::Near, "Near")
                            .clicked();
                    });
            }

            {
                let combo_id = format!("tex_map_{}_wrap_mode_{idx}", prefix.to_lowercase());
                let combo_label = format!("{prefix} Wrap Mode");

                egui::ComboBox::new(combo_id, combo_label)
                    .selected_text(format!("{:?}", self.wrap_mode))
                    .show_ui(ui, |ui| {
                        use nnbfl::bflyt::flags::TexWrapMode;

                        changed |= ui
                            .selectable_value(&mut self.wrap_mode, TexWrapMode::Clamp, "Clamp")
                            .clicked();

                        changed |= ui
                            .selectable_value(&mut self.wrap_mode, TexWrapMode::Repeat, "Repeat")
                            .clicked();

                        changed |= ui
                            .selectable_value(&mut self.wrap_mode, TexWrapMode::Mirror, "Mirror")
                            .clicked();
                    });
            }
        });

        changed
    }
}

impl DrawUiWith<(&str, usize)> for CombinerTevMode {
    fn draw_with(&mut self, ui: &mut egui::Ui, state: (&str, usize)) -> bool {
        let (prefix, idx) = state;

        let mut changed = false;

        let combo_id = format!("{}_combiner_tev_mode_{idx}", prefix.to_lowercase());

        egui::ComboBox::new(combo_id, prefix)
            .selected_text(format!("{self:?}"))
            .show_ui(ui, |ui| {
                changed |= ui
                    .selectable_value(self, Self::Replace, "Replace")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::Modulate, "Modulate")
                    .clicked();

                changed |= ui.selectable_value(self, Self::Add, "Add").clicked();
                changed |= ui
                    .selectable_value(self, Self::AddSigned, "Add Signed")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::Interpolate, "Interpolate")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::Subtract, "Subtract")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::AddMultiply, "Add Multiply")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::MultiplyAdd, "Multiply Add")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::Overlay, "Overlay")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::Lighten, "Lighten")
                    .clicked();

                changed |= ui.selectable_value(self, Self::Darken, "Darken").clicked();

                changed |= ui
                    .selectable_value(self, Self::Indirect, "Indirect")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::BlendIndirect, "Blend Indirect")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::EachIndirect, "Each Indirect")
                    .clicked();
            });

        changed
    }
}

impl DrawUiWith<&str> for MaterialBlendMode {
    fn draw_with(&mut self, ui: &mut egui::Ui, label: &str) -> bool {
        let mut changed = false;

        let current_variant_name = match self {
            MaterialBlendMode::None => "None",
            MaterialBlendMode::Blend { .. } => "Blend",
            MaterialBlendMode::Logic { .. } => "Logic",
        };

        ui.horizontal(|ui| {
            ui.label("Blend Mode:");

            egui::ComboBox::from_id_salt(format!("material_blend_mode_select_{label}"))
                .selected_text(current_variant_name)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(current_variant_name == "None", "None")
                        .clicked()
                    {
                        *self = MaterialBlendMode::None;

                        changed = true;
                    }

                    if ui
                        .selectable_label(current_variant_name == "Blend", "Blend")
                        .clicked()
                    {
                        *self = MaterialBlendMode::Blend {
                            blend_op: BlendOp::Add,
                            function_source: BlendFactor::V0,
                            function_destination: BlendFactor::V0,
                        };

                        changed = true;
                    }

                    if ui
                        .selectable_label(current_variant_name == "Logic", "Logic")
                        .clicked()
                    {
                        *self = MaterialBlendMode::Logic {
                            logic_op: LogicOp::NoOp,
                        };

                        changed = true;
                    }
                });
        });

        match self {
            MaterialBlendMode::None => {}
            MaterialBlendMode::Logic { logic_op } => {
                ui.indent(format!("logic_properties_{label}"), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Logic OP:");

                        egui::ComboBox::from_id_salt(format!("logic_op_select_{label}"))
                            .selected_text(format!("{logic_op:?}"))
                            .show_ui(ui, |ui| {
                                let ops = [
                                    LogicOp::NoOp,
                                    LogicOp::Clear,
                                    LogicOp::Set,
                                    LogicOp::Copy,
                                    LogicOp::InvCopy,
                                    LogicOp::Inv,
                                    LogicOp::And,
                                    LogicOp::Nand,
                                    LogicOp::Or,
                                    LogicOp::Nor,
                                    LogicOp::Xor,
                                    LogicOp::Equiv,
                                    LogicOp::RevAnd,
                                    LogicOp::InvAnd,
                                    LogicOp::RevOr,
                                    LogicOp::InvOr,
                                ];

                                for op in ops {
                                    changed |= ui
                                        .selectable_value(logic_op, op, format!("{op:?}"))
                                        .clicked();
                                }
                            });
                    });
                });
            }
            MaterialBlendMode::Blend {
                blend_op,
                function_source,
                function_destination,
            } => {
                ui.indent(format!("blend_properties_{label}"), |ui| {
                    egui::Grid::new(format!("blend_mode_grid_{label}"))
                        .num_columns(2)
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Blend Operand:");
                            egui::ComboBox::from_id_salt(format!("blend_op_select_{label}"))
                                .selected_text(format!("{blend_op:?}"))
                                .show_ui(ui, |ui| {
                                    let ops = [
                                        BlendOp::Add,
                                        BlendOp::Subtract,
                                        BlendOp::ReverseSubtract,
                                        BlendOp::SelectMin,
                                        BlendOp::SelectMax,
                                    ];

                                    for op in ops {
                                        changed |= ui
                                            .selectable_value(blend_op, op, format!("{op:?}"))
                                            .clicked();
                                    }
                                });
                            ui.end_row();

                            ui.label("Source Factor:");
                            changed |= function_source.draw(ui);
                            ui.end_row();

                            ui.label("Destination Factor:");
                            changed |= function_destination.draw(ui);
                            ui.end_row();
                        });
                });
            }
        }

        changed
    }
}

impl DrawUi for BlendFactor {
    fn draw(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        egui::ComboBox::from_id_salt(ui.next_auto_id())
            .selected_text(format!("{self:?}"))
            .show_ui(ui, |ui| {
                changed |= ui.selectable_value(self, Self::V0, "V0").clicked();
                changed |= ui.selectable_value(self, Self::V1_0, "V1_0").clicked();

                changed |= ui
                    .selectable_value(self, Self::SrcColor, "Source Color")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::DstColor, "DST Color")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::SrcAlpha, "Source Alpha")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::DstAlpha, "DST Alpha")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::InvSrcColor, "Inverse Source Color")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::InvDstColor, "Inverse DST Color")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::InvSrcAlpha, "Inverse Source Alpha")
                    .clicked();

                changed |= ui
                    .selectable_value(self, Self::InvDstAlpha, "Inverse DST Alpha")
                    .clicked();
            });

        changed
    }
}

impl DrawUi for TevSource {
    fn draw(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        egui::ComboBox::from_id_salt(ui.next_auto_id())
            .selected_text(format!("{self:?}"))
            .show_ui(ui, |ui| {
                let sources = [
                    Self::Primary,
                    Self::Texture0,
                    Self::Texture1,
                    Self::Texture2,
                    Self::Texture3,
                    Self::Register,
                    Self::Constant,
                    Self::Previous,
                ];

                for source in sources {
                    changed |= ui
                        .selectable_value(self, source, format!("{source:?}"))
                        .clicked();
                }
            });

        changed
    }
}

impl DrawUi for DetailedCombinerStageMode {
    fn draw(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        egui::ComboBox::from_id_salt(ui.next_auto_id())
            .selected_text(format!("{self:?}"))
            .show_ui(ui, |ui| {
                let stages = [
                    Self::Replace,
                    Self::Modulate,
                    Self::Add,
                    Self::AddSigned,
                    Self::Interpolate,
                    Self::Subtract,
                    Self::AddMult,
                    Self::MultiplicateAdd,
                ];

                for stage in stages {
                    changed |= ui
                        .selectable_value(self, stage, format!("{stage:?}"))
                        .clicked();
                }
            });

        changed
    }
}

impl DrawUi for TevScale {
    fn draw(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        egui::ComboBox::from_id_salt(ui.next_auto_id())
            .selected_text(format!("{self:?}"))
            .show_ui(ui, |ui| {
                let scales = [Self::V1, Self::V2, Self::V4];

                for scale in scales {
                    changed |= ui
                        .selectable_value(self, scale, format!("{scale:?}"))
                        .clicked();
                }
            });

        changed
    }
}

impl DrawUi for TevKonstSel {
    fn draw(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        egui::ComboBox::from_id_salt(ui.next_auto_id())
            .selected_text(format!("{self:?}"))
            .show_ui(ui, |ui| {
                let konsts = [
                    Self::BlackColor,
                    Self::WhiteColor,
                    Self::K0,
                    Self::K1,
                    Self::K2,
                    Self::K3,
                    Self::K4,
                ];

                for konst in konsts {
                    changed |= ui
                        .selectable_value(self, konst, format!("{konst:?}"))
                        .clicked();
                }
            });

        changed
    }
}

impl DrawUi for TevColorOp {
    fn draw(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        egui::ComboBox::from_id_salt(ui.next_auto_id())
            .selected_text(format!("{self:?}"))
            .show_ui(ui, |ui| {
                let ops = [
                    Self::Rgb,
                    Self::InvRgb,
                    Self::Alpha,
                    Self::InvAlpha,
                    Self::Rrr,
                    Self::InvRrr,
                    Self::Ggg,
                    Self::InvGgg,
                    Self::Bbb,
                    Self::InvBbb,
                ];

                for op in ops {
                    changed |= ui.selectable_value(self, op, format!("{op:?}")).clicked();
                }
            });

        changed
    }
}

impl DrawUi for TevAlphaOp {
    fn draw(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        egui::ComboBox::from_id_salt(ui.next_auto_id())
            .selected_text(format!("{self:?}"))
            .show_ui(ui, |ui| {
                let ops = [
                    Self::Alpha,
                    Self::InvAlpha,
                    Self::R,
                    Self::InvR,
                    Self::G,
                    Self::InvG,
                    Self::B,
                    Self::InvB,
                ];

                for op in ops {
                    changed |= ui.selectable_value(self, op, format!("{op:?}")).clicked();
                }
            });

        changed
    }
}

impl DrawUi for DetailedCombinerColorStageConfig {
    fn draw(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        egui::Grid::new(ui.next_auto_id())
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Stage Mode:");
                changed |= self.mode.draw(ui);
                ui.end_row();

                ui.label("Scale:");
                changed |= self.scale.draw(ui);
                ui.end_row();

                ui.label("Konst Sel:");
                changed |= self.konst_sel.draw(ui);
                ui.end_row();

                for (i, source) in self.sources.iter_mut().enumerate() {
                    ui.label(format!("Source {}:", i + 1));
                    changed |= source.draw(ui);
                    ui.end_row();
                }

                for (i, operand) in self.operands.iter_mut().enumerate() {
                    ui.label(format!("Color Operand {}:", i + 1));
                    changed |= operand.draw(ui);
                    ui.end_row();
                }
            });

        changed |= ui.checkbox(&mut self.copy_reg, "Copy Reg").changed();

        changed
    }
}

impl DrawUi for DetailedCombinerAlphaStageConfig {
    fn draw(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        egui::Grid::new(ui.next_auto_id())
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Stage Mode:");
                changed |= self.mode.draw(ui);
                ui.end_row();

                ui.label("Scale:");
                changed |= self.scale.draw(ui);
                ui.end_row();

                ui.label("Konst Sel:");
                changed |= self.konst_sel.draw(ui);
                ui.end_row();

                for (i, source) in self.sources.iter_mut().enumerate() {
                    ui.label(format!("Source {}:", i + 1));
                    changed |= source.draw(ui);
                    ui.end_row();
                }

                for (i, operand) in self.operands.iter_mut().enumerate() {
                    ui.label(format!("Color Operand {}:", i + 1));
                    changed |= operand.draw(ui);
                    ui.end_row();
                }
            });

        changed |= ui.checkbox(&mut self.copy_reg, "Copy Reg").changed();

        changed
    }
}

fn texture_label<T>(i: usize, _: &T) -> String {
    format!("Texture {}", i + 1)
}

fn texture_combo(
    ui: &mut egui::Ui,
    idx: usize,
    name: &mut String,
    picker: &mut TexturePicker,
) -> bool {
    let mut changed = false;
    let listed: Vec<String> = picker
        .textures
        .as_ref()
        .map(|list| {
            list.textures
                .iter()
                .map(|t| t.trim_end_matches('\0').to_string())
                .collect()
        })
        .unwrap_or_default();

    let current = name.trim_end_matches('\0').to_string();
    let missing = !current.is_empty() && !listed.contains(&current);

    ui.horizontal(|ui| {
        ui.label("Texture");
        egui::ComboBox::from_id_salt(("tex_map_texture", idx))
            .selected_text(if current.is_empty() {
                "(none)"
            } else {
                &current
            })
            .show_ui(ui, |ui| {
                for texture in &listed {
                    if ui.selectable_label(*texture == current, texture).clicked() {
                        *name = texture.clone();
                        changed = true;
                    }
                }

                let extra: Vec<&String> = picker
                    .bntx_textures
                    .iter()
                    .filter(|t| !listed.contains(t))
                    .collect();

                if !extra.is_empty() {
                    ui.separator();
                    ui.weak("Add from BNTX");

                    for texture in extra {
                        if ui
                            .selectable_label(false, format!("➕ {texture}"))
                            .clicked()
                        {
                            picker
                                .textures
                                .get_or_insert_default()
                                .textures
                                .push(texture.clone());
                            *name = texture.clone();
                            changed = true;
                        }
                    }
                }
            });

        if missing {
            ui.colored_label(ui.visuals().warn_fg_color, "not in the texture list");
        }
    });

    changed
}

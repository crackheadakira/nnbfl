use nnbfl::{
    bflyt::{file::BflytSection, pane::PANE_NAME_LEN},
    ui2d::{
        systemdata::{LayoutData, PaneData, SystemData},
        userdata::{UserData, UserDataContent},
    },
};

use crate::{
    pane_tree::{PaneId, PaneNode},
    ui::{
        DrawUi, DrawUiWith,
        array_editor::{ArrayEditor, ArrayLayout, optional},
    },
};

#[derive(Debug, Clone, Default)]
pub struct PaneEditor {
    pub is_editor_visible: bool,
    name: Option<(PaneId, String, String)>,
    rename: Option<String>,
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
}

impl DrawUiWith<(&mut PaneNode, Option<&str>, &[String])> for PaneEditor {
    fn draw_with(
        &mut self,
        ui: &mut egui::Ui,
        (pane, name_error, materials): (&mut PaneNode, Option<&str>, &[String]),
    ) -> bool {
        let mut changed = false;
        let mut is_open = self.is_editor_visible;

        egui::Window::new("Detailed Pane Editor")
            .collapsible(false)
            .resizable(true)
            .open(&mut is_open)
            .show(ui, |ui| {
                ui.heading("Pane Name");

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
                        let response =
                            ui.add(egui::TextEdit::singleline(typed).char_limit(PANE_NAME_LEN));

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

                ui.separator();

                if let BflytSection::PartsPane(pp) = &mut pane.section {
                    ui.label(format!("Layout File Name: {}", pp.o_layout_name));
                }

                if let Some(source) = &pane.parts_source {
                    ui.weak(format!(
                        "From parts layout {source}; override editing comes later"
                    ));
                }

                let editable = pane.parts_source.is_none();

                ui.add_enabled_ui(editable, |ui| {
                    changed |= optional(ui, "User Data", &mut pane.user_data, |ui, array| {
                        ArrayEditor::new("user_data", &mut array.user_data)
                            .item_label(|_, entry| entry.o_name.clone())
                            .show(ui, |ui, _, entry| entry.draw(ui))
                            .changed
                    });

                    if let BflytSection::PicturePane(pic) = &mut pane.section {
                        changed |= pic
                            .top_left_vertex_color
                            .draw_with(ui, "Top Left Vertex Color");

                        changed |= pic
                            .top_right_vertex_color
                            .draw_with(ui, "Top Right Vertex Color");

                        changed |= pic
                            .bottom_left_vertex_color
                            .draw_with(ui, "Bottom Left Vertex Color");

                        changed |= pic
                            .bottom_right_vertex_color
                            .draw_with(ui, "Bottom Right Vertex Color");
                    }

                    match &mut pane.section {
                        BflytSection::PicturePane(pic) => {
                            changed |=
                                material_combo(ui, "Material", &mut pic.material_index, materials);
                        }
                        BflytSection::TextBoxPane(txt) => {
                            changed |=
                                material_combo(ui, "Material", &mut txt.material_index, materials);
                        }
                        BflytSection::WindowPane(win) => {
                            changed |= material_combo(
                                ui,
                                "Content Material",
                                &mut win.content.material_index,
                                materials,
                            );

                            for (i, frame) in win.frames.iter_mut().enumerate() {
                                changed |= material_combo(
                                    ui,
                                    &format!("Frame {} Material", i + 1),
                                    &mut frame.material_index,
                                    materials,
                                );
                            }
                        }
                        _ => {}
                    }
                });
            });

        self.is_editor_visible = is_open;
        changed
    }
}

impl DrawUi for UserData {
    fn draw(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.horizontal(|ui| {
            changed |= ui
                .add(egui::TextEdit::singleline(&mut self.o_name))
                .changed();

            let current_type = match &self.content {
                UserDataContent::String(_) => "String",
                UserDataContent::S32(_) => "Int Array",
                UserDataContent::Float(_) => "Float Array",
                UserDataContent::SystemData(_) => "System Data Array",
            };

            egui::ComboBox::from_id_salt(format!("type_select_{}", self.o_name))
                .selected_text(current_type)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(current_type == "String", "String")
                        .clicked()
                    {
                        self.content = UserDataContent::String(Default::default());
                        changed |= true;
                    }

                    if ui
                        .selectable_label(current_type == "Int Array", "Int Array")
                        .clicked()
                    {
                        self.content = UserDataContent::S32(Default::default());
                        changed |= true;
                    }

                    if ui
                        .selectable_label(current_type == "Float Array", "Float Array")
                        .clicked()
                    {
                        self.content = UserDataContent::Float(Default::default());
                        changed |= true;
                    }

                    if ui
                        .selectable_label(current_type == "System Data Array", "System Data Array")
                        .clicked()
                    {
                        self.content = UserDataContent::SystemData(Default::default());
                        changed |= true;
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

            UserDataContent::SystemData(items) => {
                let mut vec_changed = false;

                for system_data_container in items {
                    for system_data in system_data_container {
                        vec_changed |= system_data.draw(ui);
                    }
                }

                vec_changed
            }
        };

        changed
    }
}

impl DrawUi for SystemData {
    fn draw(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        let current_type = match &self {
            Self::Layout(_) => "Layout",
            Self::Pane(_) => "Pane",
        };

        ui.horizontal(|ui| {
            ui.label("System Data Type:");

            egui::ComboBox::from_id_salt(format!("type_select_system_data_{current_type}"))
                .selected_text(current_type)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(current_type == "Layout", "Layout")
                        .clicked()
                    {
                        *self = Self::Layout(Default::default());
                        changed |= true;
                    }

                    if ui
                        .selectable_label(current_type == "Pane", "Pane")
                        .clicked()
                    {
                        *self = Self::Pane(Default::default());
                        changed |= true;
                    }
                });
        });

        match self {
            Self::Layout(layout) => {
                if let LayoutData::AnimTagName(names) = layout {
                    changed |= ArrayEditor::new("anim_tag_names", names)
                        .title("Animation Tag Names")
                        .layout(ArrayLayout::Inline)
                        .show(ui, |ui, _, name| {
                            ui.add(egui::TextEdit::singleline(name).desired_width(120.0))
                                .changed()
                        })
                        .changed;
                }
            }

            Self::Pane(pane) => {
                let current_pane_type = match pane {
                    PaneData::Alignment(_) => "Alignment",
                    PaneData::VertexPos0(_) => "Vertex Position 0",
                    PaneData::VertexPos1(_) => "Vertex Position 1",
                    PaneData::ProceduralShape(_) => "Procedural Shape",
                    PaneData::DropShadow(_) => "Drop Shadow",
                    PaneData::MaskTexture(_) => "Mask Texture",
                };

                ui.horizontal(|ui| {
                    ui.label("Pane Type:");

                    egui::ComboBox::from_id_salt(format!(
                        "type_select_system_data_pane_{current_pane_type}"
                    ))
                    .selected_text(current_pane_type)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(current_pane_type == "Alignment", "Alignment")
                            .clicked()
                        {
                            *pane = PaneData::Alignment(Default::default());
                            changed |= true;
                        }

                        if ui
                            .selectable_label(
                                current_pane_type == "Vertex Position 0",
                                "Vertex Position 0",
                            )
                            .clicked()
                        {
                            *pane = PaneData::VertexPos0(Default::default());
                            changed |= true;
                        }

                        if ui
                            .selectable_label(
                                current_pane_type == "Vertex Position 1",
                                "Vertex Position 1",
                            )
                            .clicked()
                        {
                            *pane = PaneData::VertexPos1(Default::default());
                            changed |= true;
                        }

                        if ui
                            .selectable_label(
                                current_pane_type == "Procedural Shape",
                                "Procedural Shape",
                            )
                            .clicked()
                        {
                            *pane = PaneData::ProceduralShape(Default::default());
                            changed |= true;
                        }

                        if ui
                            .selectable_label(current_pane_type == "Drop Shadow", "Drop Shadow")
                            .clicked()
                        {
                            *pane = PaneData::DropShadow(Default::default());
                            changed |= true;
                        }

                        if ui
                            .selectable_label(current_pane_type == "Mask Texture", "Mask Texture")
                            .clicked()
                        {
                            *pane = PaneData::MaskTexture(Default::default());
                            changed |= true;
                        }
                    });
                });
            }
        }

        changed
    }
}

fn material_combo(ui: &mut egui::Ui, label: &str, index: &mut u16, materials: &[String]) -> bool {
    let mut changed = false;
    let current = materials
        .get(*index as usize)
        .map_or_else(|| format!("#{index} (missing)"), |name| name.clone());

    ui.horizontal(|ui| {
        ui.label(label);

        egui::ComboBox::from_id_salt(("pane_material", label))
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (i, name) in materials.iter().enumerate() {
                    if ui
                        .selectable_label(*index as usize == i, format!("[{}] {name}", i + 1))
                        .clicked()
                        && *index as usize != i
                    {
                        *index = i as u16;
                        changed = true;
                    }
                }
            });
    });

    changed
}

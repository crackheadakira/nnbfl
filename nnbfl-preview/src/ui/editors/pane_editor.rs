use nnbfl::{
    bflyt::{file::BflytSection, pane::PANE_NAME_LEN},
    ui2d::{
        systemdata::{LayoutData, PaneData, SystemData},
        userdata::{UserData, UserDataContent},
    },
};

use crate::{
    pane_tree::{PaneId, PaneNode},
    ui::{DrawUi, DrawUiWith},
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

impl DrawUiWith<(&mut PaneNode, Option<&str>)> for PaneEditor {
    fn draw_with(
        &mut self,
        ui: &mut egui::Ui,
        (pane, name_error): (&mut PaneNode, Option<&str>),
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

                ui.horizontal(|ui| {
                    ui.heading("User Data");

                    if pane.user_data.is_none() && ui.button("➕ Add").clicked() {
                        pane.user_data = Some(Default::default());
                        changed |= true;
                    }
                });

                if let Some(user_data_array) = &mut pane.user_data {
                    for user_data in user_data_array.user_data.iter_mut() {
                        changed |= user_data.draw(ui);

                        ui.add_space(8.0);
                    }
                }

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

                    ui.label(format!("Picture Material Index {}", pic.material_index));

                    if let Some(tq) = &mut pane.textured_quad {
                        ui.label(format!("Quad Material Index {}", tq.material_idx));
                    }
                }
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
                ui.horizontal(|ui| {
                    let mut vec_changed = false;

                    for item in items.iter_mut() {
                        let mut val_str = item.to_string();
                        let response =
                            ui.add(egui::TextEdit::singleline(&mut val_str).desired_width(40.0));

                        if response.changed()
                            && let Ok(parsed) = val_str.parse::<i32>()
                        {
                            *item = parsed;
                            vec_changed = true;
                        }
                    }

                    if ui.button("➕").clicked() {
                        items.push(0);
                        vec_changed = true;
                    }

                    if !items.is_empty() && ui.button("🗑").clicked() {
                        items.pop();
                        vec_changed = true;
                    }

                    vec_changed
                })
                .inner
            }

            UserDataContent::Float(items) => {
                ui.horizontal(|ui| {
                    let mut vec_changed = false;

                    for item in items.iter_mut() {
                        let mut val_str = item.to_string();
                        let response =
                            ui.add(egui::TextEdit::singleline(&mut val_str).desired_width(50.0));

                        if response.changed()
                            && let Ok(parsed) = val_str.parse::<f32>()
                        {
                            *item = parsed;
                            vec_changed = true;
                        }
                    }

                    if ui.button("➕").clicked() {
                        items.push(0.0);
                        vec_changed = true;
                    }
                    if !items.is_empty() && ui.button("🗑").clicked() {
                        items.pop();
                        vec_changed = true;
                    }

                    vec_changed
                })
                .inner
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
                    ui.horizontal(|ui| {
                        ui.weak("Animation Tag Names");

                        if ui.button("➕").clicked() {
                            names.push("".to_owned());
                            changed = true;
                        }

                        if !names.is_empty() && ui.button("🗑").clicked() {
                            names.pop();
                            changed = true;
                        }
                    });

                    for name in names.iter_mut() {
                        changed |= ui.add(egui::TextEdit::singleline(name)).changed();
                    }
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

use nnbfl::bflyt::list::Group;

use crate::{
    pane_tree::{PaneNode, PaneTree},
    ui::{DrawUiWith, array_editor::ArrayEditor},
};

#[derive(Debug, Clone, Default)]
pub struct GroupEditor {
    pub is_editor_visible: bool,
    pub new_group_name: String,
}

impl DrawUiWith<&mut PaneTree> for GroupEditor {
    fn draw_with(&mut self, ui: &mut egui::Ui, tree: &mut PaneTree) -> bool {
        let mut changed = false;

        if !self.is_editor_visible {
            return changed;
        }

        let mut available_panes = Vec::new();
        let mut pane_stack = tree.roots.iter().collect::<Vec<&PaneNode>>();
        while let Some(node) = pane_stack.pop() {
            if node.parts_source.is_none()
                && let Some((name, _)) = tree
                    .label_map
                    .iter()
                    .find(|&(_, &val)| val == node.pane_idx)
            {
                available_panes.push(name.clone());
            }

            for child in &node.children {
                pane_stack.push(child);
            }
        }

        egui::Window::new("Group Editor")
            .collapsible(false)
            .resizable(true)
            .open(&mut self.is_editor_visible)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("group_editor_scroll")
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        ui.heading("Root Group");
                        changed |= tree.group.data.draw_with(ui, &mut available_panes);

                        ui.separator();
                        changed |= ArrayEditor::new("subgroups", &mut tree.group.children)
                            .title("Subgroups")
                            .addable(false)
                            .duplicable(false)
                            .show(ui, |ui, _, group| group.draw_with(ui, &mut available_panes))
                            .changed;

                        ui.separator();

                        ui.horizontal(|ui| {
                            ui.label("New Subgroup Name:");
                            ui.text_edit_singleline(&mut self.new_group_name);

                            if ui.button("Create Group").clicked()
                                && !self.new_group_name.is_empty()
                            {
                                let formatted_name = self.new_group_name.trim().to_string();

                                if !tree
                                    .group
                                    .children
                                    .iter()
                                    .any(|g| g.group_name == formatted_name)
                                {
                                    tree.group.children.push(Group {
                                        group_name: formatted_name,
                                        child_names: Vec::new(),
                                    });
                                    self.new_group_name.clear();
                                    changed |= true;
                                }
                            }
                        });
                    });
            });

        changed
    }
}

impl DrawUiWith<&mut [String]> for Group {
    fn draw_with(&mut self, ui: &mut egui::Ui, available_panes: &mut [String]) -> bool {
        let mut changed = false;

        ui.collapsing(format!("Group: {}", self.group_name), |ui| {
            ui.weak(format!("Contains {} bound items", self.child_names.len()));

            changed |=
                ArrayEditor::new(("group_children", &self.group_name), &mut self.child_names)
                    .addable(false)
                    .duplicable(false)
                    .item_label(|_, name| name.clone())
                    .show(ui, |_, _, _| false)
                    .changed;

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                let dropdown_id = egui::Id::new("add_to_group").with(&self.group_name);

                egui::ComboBox::from_id_salt(dropdown_id)
                    .selected_text("➕ Add Pane...")
                    .show_ui(ui, |ui| {
                        for name in available_panes {
                            if self.child_names.contains(name) {
                                continue;
                            };

                            if ui.selectable_label(false, name.clone()).clicked() {
                                self.child_names.push(name.clone());
                                changed |= true;
                            }
                        }
                    });

                if ui.button("🗑").on_hover_text("Clear Group").clicked()
                    && !self.child_names.is_empty()
                {
                    self.child_names.clear();
                    changed |= true;
                }
            });
        });

        changed
    }
}

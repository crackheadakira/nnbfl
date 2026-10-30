use crate::{
    ops::duplicate::{self, DuplicateNames, DuplicateOptions, RenameRule},
    pane_tree::{PaneId, PaneTree},
    ui::general::UiAction,
};

const ERROR: egui::Color32 = egui::Color32::from_rgb(224, 96, 96);

#[derive(Default)]
pub struct DuplicateDialog {
    pub options: DuplicateOptions,
    open: Option<OpenDialog>,
}

struct OpenDialog {
    source: PaneId,
    label: String,
    rule: RenameRule,
    names: DuplicateNames,
}

impl DuplicateDialog {
    pub fn open(&mut self, tree: &PaneTree, source: PaneId) {
        let label = tree
            .find_by_idx(source)
            .map(|n| n.label.trim_end_matches('\0').to_string())
            .unwrap_or_default();

        self.open = duplicate::plan(tree, source, None).map(|names| OpenDialog {
            source,
            label,
            rule: RenameRule::default(),
            names,
        });
    }

    pub fn show(&mut self, ctx: &egui::Context, tree: &PaneTree) -> Option<UiAction> {
        let dialog = self.open.as_mut()?;
        let mut action = None;
        let mut keep_open = true;
        let mut close = false;

        egui::Window::new(format!("Duplicate {}", dialog.label))
            .collapsible(false)
            .resizable(true)
            .open(&mut keep_open)
            .show(ctx, |ui| {
                let mut replanned = false;
                egui::Grid::new("duplicate_rule")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Find");
                        replanned |= ui.text_edit_singleline(&mut dialog.rule.find).changed();
                        ui.end_row();

                        ui.label("Replace with");
                        replanned |= ui.text_edit_singleline(&mut dialog.rule.replace).changed();
                        ui.end_row();
                    });

                if replanned
                    && let Some(names) = duplicate::plan(tree, dialog.source, Some(&dialog.rule))
                {
                    dialog.names = names;
                }

                ui.checkbox(&mut self.options.anims, "Copy animation tracks");
                ui.checkbox(
                    &mut self.options.groups,
                    "Add the copies to the same groups",
                );

                ui.separator();

                let (pane_errors, material_errors) = duplicate::errors(tree, &dialog.names);

                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.strong("Panes");
                        rows(
                            ui,
                            "duplicate_panes",
                            dialog.names.panes.iter_mut().map(|r| (&r.old, &mut r.new)),
                            &pane_errors,
                        );

                        if !dialog.names.materials.is_empty() {
                            ui.add_space(6.0);
                            ui.strong("Materials");

                            rows(
                                ui,
                                "duplicate_materials",
                                dialog
                                    .names
                                    .materials
                                    .iter_mut()
                                    .map(|r| (&r.old, &mut r.new)),
                                &material_errors,
                            );
                        }
                    });

                if !dialog.names.shared.is_empty() {
                    ui.label(format!(
                        "Stays shared with panes outside the copy: {}",
                        dialog.names.shared.join(", ")
                    ));
                }

                let bound = duplicate::control_bindings(tree, &dialog.names);
                if !bound.is_empty() {
                    ui.colored_label(
                        egui::Color32::from_rgb(224, 192, 96),
                        format!(
                            "The control source binds {}, the copies won't be bound",
                            bound.join(", ")
                        ),
                    );
                }

                ui.separator();

                let valid = pane_errors
                    .iter()
                    .chain(&material_errors)
                    .all(Option::is_none);

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(valid, egui::Button::new("Duplicate"))
                        .clicked()
                    {
                        action = Some(UiAction::DuplicateWithNames {
                            source: dialog.source,
                            names: dialog.names.clone(),
                            options: self.options,
                        });

                        close = true;
                    }

                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });

        if close || !keep_open {
            self.open = None;
        }

        action
    }
}

fn rows<'a>(
    ui: &mut egui::Ui,
    id: &str,
    rows: impl Iterator<Item = (&'a String, &'a mut String)>,
    errors: &[Option<String>],
) {
    egui::Grid::new(id)
        .num_columns(3)
        .striped(true)
        .show(ui, |ui| {
            for ((old, new), error) in rows.zip(errors) {
                ui.label(old.as_str());
                ui.label("→");

                let edit = egui::TextEdit::singleline(new).desired_width(200.0);
                let edit = match error {
                    Some(_) => edit.text_color(ERROR),
                    None => edit,
                };

                let response = ui.add(edit);
                if let Some(error) = error {
                    response.on_hover_text(error);
                }

                ui.end_row();
            }
        });
}

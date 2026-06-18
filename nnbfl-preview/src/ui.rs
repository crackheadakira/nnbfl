use std::{collections::HashSet, path::PathBuf};

use egui::Context;

use crate::bflyt_view::{BflytView, PaneInfo};

pub struct UiState {
    pub selected_pane: Option<usize>,
    pub hidden_panes: HashSet<usize>,
    pub error_message: Option<String>,
    pub pending_action: Option<UiAction>,
}

pub enum UiAction {
    LoadFile(PathBuf),
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            selected_pane: None,
            hidden_panes: HashSet::new(),
            error_message: None,
            pending_action: None,
        }
    }
}

pub fn draw_ui(ctx: &Context, view: &Option<BflytView>, state: &mut UiState) {
    egui::SidePanel::left("pane_tree")
        .default_width(220.0)
        .show(ctx, |ui| {
            ui.heading("Pane Tree");
            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    if let Some(view) = view {
                        for (i, pane) in view.panes.iter().enumerate() {
                            let indent = pane.depth as f32 * 12.0;
                            ui.horizontal(|ui| {
                                ui.add_space(indent);

                                let selected = state.selected_pane == Some(i);
                                let label =
                                    egui::RichText::new(format!("[{}] {}", pane.kind, pane.label));
                                let is_hidden = state.hidden_panes.contains(&i);

                                if is_hidden {
                                    ui.label("Hidden");
                                }

                                let response = ui.selectable_label(selected, label);
                                response.context_menu(|ui| {
                                    if !is_hidden && ui.button("Hide").clicked() {
                                        state.hidden_panes.insert(i);
                                        ui.close_menu();
                                    }

                                    if !is_hidden && ui.button("Hide All").clicked() {
                                        hide_pane_recursive(i, view, &mut state.hidden_panes);
                                        ui.close_menu();
                                    }

                                    if is_hidden && ui.button("Show").clicked() {
                                        state.hidden_panes.remove(&i);
                                        ui.close_menu();
                                    }

                                    if is_hidden && ui.button("Show All").clicked() {
                                        show_pane_recursive(i, view, &mut state.hidden_panes);
                                        ui.close_menu();
                                    }
                                });

                                if response.clicked() {
                                    state.selected_pane = Some(i);
                                }
                            });
                        }
                    } else {
                        ui.label("No .bflyt file loaded");
                    }
                });
        });

    if let Some(view) = view {
        egui::TopBottomPanel::bottom("properties")
            .default_height(150.0)
            .show(ctx, |ui| {
                ui.heading("Properties");
                ui.separator();

                if let Some(idx) = state.selected_pane {
                    if let Some(pane) = view.panes.get(idx) {
                        draw_pane_properties(ui, pane);
                    }
                } else {
                    ui.label("Select a pane in the tree to inspect it.");
                }
            });
    }

    // maybe somehow can be done without a clone?
    if let Some(err) = state.error_message.clone() {
        egui::Window::new("Error")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(err);

                if ui.button("Close").clicked() {
                    state.error_message = None;
                }
            });
    };

    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Load File...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Bflyt", &["bflyt"])
                        .pick_file()
                    {
                        state.pending_action = Some(UiAction::LoadFile(path));
                        state.hidden_panes.clear();
                        state.selected_pane = None;
                    }

                    ui.close_menu();
                }
            })
        })
    });
}

fn hide_pane_recursive(idx: usize, view: &BflytView, hidden_set: &mut HashSet<usize>) {
    hidden_set.insert(idx);

    let base_depth = view.panes[idx].depth;
    for next_idx in (idx + 1)..view.panes.len() {
        if view.panes[next_idx].depth > base_depth {
            hidden_set.insert(next_idx);
        } else {
            break;
        }
    }
}

fn show_pane_recursive(idx: usize, view: &BflytView, hidden_set: &mut HashSet<usize>) {
    hidden_set.remove(&idx);

    let base_depth = view.panes[idx].depth;
    for next_idx in (idx + 1)..view.panes.len() {
        if view.panes[next_idx].depth > base_depth {
            hidden_set.remove(&next_idx);
        } else {
            break;
        }
    }
}

fn draw_pane_properties(ui: &mut egui::Ui, pane: &PaneInfo) {
    egui::Grid::new("pane_props")
        .num_columns(2)
        .striped(true)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label("Name");
            ui.label(&pane.label);
            ui.end_row();

            ui.label("Kind");
            ui.label(&pane.kind);
            ui.end_row();

            ui.label("X");
            ui.label(format!("{:.2}", pane.x));
            ui.end_row();

            ui.label("Y");
            ui.label(format!("{:.2}", pane.y));
            ui.end_row();

            ui.label("Width");
            ui.label(format!("{:.2}", pane.width));
            ui.end_row();

            ui.label("Height");
            ui.label(format!("{:.2}", pane.height));
            ui.end_row();

            ui.label("Depth");
            ui.label(format!("{}", pane.depth));
            ui.end_row();
        });
}

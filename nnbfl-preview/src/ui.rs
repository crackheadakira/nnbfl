use std::{collections::HashSet, path::PathBuf};

use egui::Ui;
use nnbfl::bflyt::file::BflytSection;

use crate::{
    anim_state::AnimPlayer,
    bflyt_view::{BflytView, PaneInfo},
    camera::Camera,
};

pub const SUPPORTED_SARC_EXTENSIONS: &[&str] = &[
    "blarc",
    "sarc",
    "Nin_NX_NVN",
    "blarc.zs",
    "sarc.zs",
    "Nin_NX_NVN.zs",
];

#[derive(Default)]
pub struct UiState {
    pub selected_pane: Option<usize>,
    pub hidden_panes: HashSet<usize>,
    pub error_message: Option<String>,
    pub pending_action: Option<UiAction>,
    pub clip_to_root: bool,
    pub only_textured: bool,
    pub anim_names: Vec<String>,
    pub pending_play_anim: Option<String>,
}

pub enum UiAction {
    LoadFile(PathBuf),
    SetBlarcDir(PathBuf),
}

pub fn draw_ui(
    ui: &mut Ui,
    view: &Option<BflytView>,
    state: &mut UiState,
    camera: &Camera,
    anim_player: &mut AnimPlayer,
    screen_w: f32,
    screen_h: f32,
) {
    if let Some(view) = view {
        let viewport_rect = ui.content_rect();
        let painter = ui.painter().with_clip_rect(viewport_rect);

        for (i, pane) in view.panes.iter().enumerate() {
            if let BflytSection::TextBoxPane(text_box) = &pane.section
                && let Some(text) = &text_box.text
                && let Some(quad) = view.quads.get(i)
                && !state.hidden_panes.contains(&i)
                && pane.visible
            {
                let center_x = quad.x + (quad.width * 0.5);
                let center_y = quad.y + (quad.height * 0.5);

                let screen_pos = camera.world_to_screen([center_x, center_y], screen_w, screen_h);
                let font_size = (32.0 * camera.zoom).clamp(8.0, 128.0);
                let font_id = egui::FontId::proportional(font_size);

                let shadow_offset = (font_size * 0.08).max(1.5);
                let shadow_pos =
                    egui::pos2(screen_pos.x + shadow_offset, screen_pos.y + shadow_offset);

                painter.text(
                    shadow_pos,
                    egui::Align2::CENTER_CENTER,
                    text,
                    font_id.clone(),
                    egui::Color32::from_black_alpha(220),
                );

                painter.text(
                    screen_pos,
                    egui::Align2::CENTER_CENTER,
                    text,
                    font_id,
                    egui::Color32::WHITE,
                );
            }
        }
    }

    egui::Panel::left("pane_tree")
        .default_size(220.0)
        .show_inside(ui, |ui| {
            ui.heading("Pane Tree");
            ui.checkbox(&mut state.clip_to_root, "Clip to root pane");
            ui.checkbox(&mut state.only_textured, "Draw only textured quads");
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

                                let response = ui.selectable_label(selected, label);
                                response.context_menu(|ui| {
                                    if !is_hidden && ui.button("Hide").clicked() {
                                        state.hidden_panes.insert(i);
                                        ui.close();
                                    }

                                    if !is_hidden && ui.button("Hide All").clicked() {
                                        hide_pane_recursive(i, view, &mut state.hidden_panes);
                                        ui.close();
                                    }

                                    if is_hidden && ui.button("Show").clicked() {
                                        state.hidden_panes.remove(&i);
                                        ui.close();
                                    }

                                    if is_hidden && ui.button("Show All").clicked() {
                                        show_pane_recursive(i, view, &mut state.hidden_panes);
                                        ui.close();
                                    }
                                });

                                if response.clicked() {
                                    state.selected_pane = Some(i);
                                }

                                if is_hidden {
                                    ui.label("Hidden");
                                }
                            });
                        }
                    } else {
                        ui.label("No .bflyt file loaded");
                    }
                });
        });
    if !state.anim_names.is_empty() || view.is_some() {
        egui::Panel::bottom("master_bottom_shelf")
            .default_size(150.0)
            .resizable(true)
            .show_inside(ui, |ui| {
                ui.columns(2, |layout_cols| {
                    let ui = &mut layout_cols[0];
                    if !state.anim_names.is_empty() {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.heading("Animation Sequences");
                                ui.add_space(8.0);

                                if anim_player.is_playing() {
                                    ui.label(
                                        egui::RichText::new("Playing")
                                            .color(egui::Color32::GREEN)
                                            .strong(),
                                    );
                                } else if anim_player.active.is_some() {
                                    ui.label(
                                        egui::RichText::new("Paused").color(egui::Color32::GOLD),
                                    );
                                } else {
                                    ui.label(
                                        egui::RichText::new("Idle").color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.separator();

                            ui.columns(2, |anim_sub_cols| {
                                anim_sub_cols[0].vertical(|ui| {
                                    egui::ScrollArea::vertical()
                                        .id_salt("anim_selection_grid")
                                        .max_height(90.0)
                                        .show(ui, |ui| {
                                            ui.horizontal_wrapped(|ui| {
                                                for (idx, name) in
                                                    state.anim_names.iter().enumerate()
                                                {
                                                    let is_active = anim_player.active == Some(idx);
                                                    let btn_text = if is_active
                                                        && anim_player.anims[idx].playing
                                                    {
                                                        name.clone()
                                                    } else {
                                                        name.clone()
                                                    };

                                                    if ui
                                                        .selectable_label(is_active, btn_text)
                                                        .clicked()
                                                    {
                                                        state.pending_play_anim =
                                                            Some(name.clone());
                                                    }
                                                }
                                            });
                                        });
                                });

                                anim_sub_cols[1].vertical(|ui| {
                                    if let Some(idx) = anim_player.active
                                        && let Some(anim) = anim_player.anims.get_mut(idx)
                                    {
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new(&anim.name).strong());
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.small(format!(
                                                        "F: {:.1} / {:.0}",
                                                        anim.frame,
                                                        anim.frame_count()
                                                    ));
                                                },
                                            );
                                        });

                                        ui.horizontal(|ui| {
                                            let play_toggle =
                                                if anim.playing { "Pause" } else { "Play" };
                                            if ui.button(play_toggle).clicked() {
                                                anim.playing = !anim.playing;
                                            }
                                            if ui.button("Stop").clicked() {
                                                anim.frame = 0.0;
                                                anim.playing = false;
                                            }
                                            ui.small(format!("Looping: {}", anim.is_looping()));
                                        });

                                        ui.add_space(4.0);

                                        let max_frame = anim.frame_count() - 1.0;
                                        let mut temporary_frame = anim.frame;

                                        let slider_res = ui.add(
                                            egui::Slider::new(
                                                &mut temporary_frame,
                                                0.0..=max_frame,
                                            )
                                            .show_value(false)
                                            .trailing_fill(true),
                                        );

                                        if slider_res.changed() {
                                            anim.frame = temporary_frame;
                                            if slider_res.dragged() {
                                                anim.playing = false;
                                            }
                                        }
                                        if slider_res.drag_stopped() {
                                            anim.playing = true;
                                        }
                                    }
                                });
                            });
                        });
                    }

                    let ui = &mut layout_cols[1];
                    if let Some(view) = view {
                        ui.vertical(|ui| {
                            ui.heading("Properties");
                            ui.separator();

                            if let Some(idx) = state.selected_pane {
                                if let Some(pane) = view.panes.get(idx) {
                                    draw_pane_properties(ui, pane);
                                }
                            } else {
                                ui.centered_and_justified(|ui| {
                                    ui.label("Select a pane in the tree to inspect it.");
                                });
                            }
                        });
                    }
                });
            });
    }

    // maybe somehow can be done without a clone?
    if let Some(err) = state.error_message.clone() {
        egui::Window::new("Error")
            .collapsible(false)
            .resizable(false)
            .show(ui, |ui| {
                ui.label(err);

                if ui.button("Close").clicked() {
                    state.error_message = None;
                }
            });
    };

    egui::Panel::top("menu_bar").show_inside(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Load File...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter(
                            "Supported files",
                            &[SUPPORTED_SARC_EXTENSIONS, &["bflyt"]].concat(),
                        )
                        .pick_file()
                    {
                        state.pending_action = Some(UiAction::LoadFile(path));
                        state.hidden_panes.clear();
                        state.selected_pane = None;
                    }

                    ui.close();
                }

                if ui.button("Set blarc folder...").clicked() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        state.pending_action = Some(UiAction::SetBlarcDir(dir));
                    }
                    ui.close();
                }
            });
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

            if let Some(source) = &pane.parts_source {
                ui.label("Parts Source");
                ui.label(source);
                ui.end_row();
            }

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

            ui.label("Visible");
            ui.label(format!("{}", pane.visible));
            ui.end_row();
        });
}

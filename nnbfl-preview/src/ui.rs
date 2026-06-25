use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use egui::Ui;
use nnbfl::{
    bflyt::{
        file::BflytSection,
        list::{BflytLayout, BflytMaterialList, MaterialBlendMode},
        pane::BflytPane,
    },
    ui2d::types::{Vector2f, Vector3f},
};

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
    pub no_textured: bool,
    pub quad_for_textured: bool,
    pub anim_names: Vec<String>,
    pub pending_play_anim: Option<String>,
    pub sidebar_tab: SidebarTab,
    pub active_debug_stage: u32,

    pub localized_strings: HashMap<String, String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SidebarTab {
    #[default]
    Panes,
    Materials,
    Properties,
}

pub enum UiAction {
    LoadFile(PathBuf),
    SetBlarcDir(PathBuf),
    LoadMal(PathBuf),
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
                && let Some(quad) = view.quads.get(i)
                && !state.hidden_panes.contains(&i)
                && pane.visible
            {
                let pane_label = pane.label.trim_end_matches('\0');
                let lookup_key = if let Some(source) = &pane.parts_source {
                    format!("{}:{source}-{pane_label}", view.file_name)
                } else {
                    format!("{}:{pane_label}", view.file_name)
                };

                // println!("{lookup_key}");

                let default_text = text_box.text.as_deref().unwrap_or("");
                let display_text = state
                    .localized_strings
                    .get(&lookup_key)
                    .map(|s| s.as_str())
                    .unwrap_or(default_text);

                if display_text.is_empty() {
                    continue;
                }

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
                    display_text,
                    font_id.clone(),
                    egui::Color32::from_black_alpha(220),
                );

                painter.text(
                    screen_pos,
                    egui::Align2::CENTER_CENTER,
                    display_text,
                    font_id,
                    egui::Color32::WHITE,
                );
            }
        }
    }

    egui::Panel::left("pane_tree")
        .default_size(220.0)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.sidebar_tab, SidebarTab::Panes, "Pane Tree");
                ui.selectable_value(&mut state.sidebar_tab, SidebarTab::Materials, "Materials");
                ui.selectable_value(&mut state.sidebar_tab, SidebarTab::Properties, "Properties");
            });
            ui.separator();

            match state.sidebar_tab {
                SidebarTab::Panes => {
                    ui.heading("Pane Tree");
                    ui.checkbox(&mut state.clip_to_root, "Clip to root pane");
                    ui.checkbox(&mut state.only_textured, "Draw only textures");
                    ui.checkbox(
                        &mut state.quad_for_textured,
                        "Draw pane outlines for textures",
                    );
                    ui.checkbox(&mut state.no_textured, "Draw only pane outlines");
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
                                        let label = egui::RichText::new(format!(
                                            "[{}] {}",
                                            pane.kind, pane.label
                                        ));

                                        let is_hidden = state.hidden_panes.contains(&i);

                                        let response = ui.selectable_label(selected, label);
                                        response.context_menu(|ui| {
                                            if !is_hidden && ui.button("Hide").clicked() {
                                                state.hidden_panes.insert(i);
                                                ui.close();
                                            }
                                            if !is_hidden && ui.button("Hide All").clicked() {
                                                hide_pane_recursive(
                                                    i,
                                                    view,
                                                    &mut state.hidden_panes,
                                                );
                                                ui.close();
                                            }
                                            if is_hidden && ui.button("Show").clicked() {
                                                state.hidden_panes.remove(&i);
                                                ui.close();
                                            }
                                            if is_hidden && ui.button("Show All").clicked() {
                                                show_pane_recursive(
                                                    i,
                                                    view,
                                                    &mut state.hidden_panes,
                                                );
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
                }
                SidebarTab::Materials => {
                    ui.heading("Material List");
                    ui.separator();

                    if let Some(view) = view {
                        if let Some(material_list) = &view.material_list {
                            draw_material_list(ui, material_list);
                        } else {
                            ui.label("Bflyt file has no material list");
                        }
                    } else {
                        ui.label("No .bflyt file loaded");
                    }
                }
                SidebarTab::Properties => {
                    if let Some(view) = view {
                        ui.vertical(|ui| {
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
                    } else {
                        ui.label("No .bflyt file loaded");
                    }
                }
            }
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

                                                    if ui
                                                        .selectable_label(is_active, name)
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

                                            if ui.button("Loop").clicked() {
                                                anim.set_looping();
                                                anim.frame = 0.0;
                                                anim.playing = true;
                                            }

                                            ui.small(format!("Looping: {}", anim.is_looping()));
                                        });

                                        ui.add_space(4.0);

                                        let max_frame = anim.frame_count();
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
                    if view.is_some() {
                        ui.vertical(|ui| {
                            ui.heading("Shader Diagnostics");
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label("Debug Layer:");

                                let current_label = match state.active_debug_stage {
                                    0 => "Disabled",
                                    1 => "1. Layer 0 Raw Texture",
                                    2 => "2. Layer 1 Raw Texture",
                                    3 => "3. Layer 2 Raw Texture",
                                    4 => "4. Post-Texture Combiner Blend",
                                    5 => "5. Indirect Raw Vector Offset",
                                    6 => "6. Indirect Displaced UV Coordinates",
                                    7 => "7. Indirect Isolated Sample Output",
                                    8 => "8. Composite Layer Alpha (Grayscale)",
                                    _ => "Unknown Stage",
                                };

                                egui::ComboBox::from_id_salt("shader_debug_combobox")
                                    .selected_text(current_label)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut state.active_debug_stage,
                                            0,
                                            "Disabled",
                                        );

                                        ui.selectable_value(
                                            &mut state.active_debug_stage,
                                            1,
                                            "1. Layer 0 Raw Texture",
                                        );

                                        ui.selectable_value(
                                            &mut state.active_debug_stage,
                                            2,
                                            "2. Layer 1 Raw Texture",
                                        );

                                        ui.selectable_value(
                                            &mut state.active_debug_stage,
                                            3,
                                            "3. Layer 2 Raw Texture",
                                        );

                                        ui.selectable_value(
                                            &mut state.active_debug_stage,
                                            4,
                                            "4. Post-Texture Combiner Blend",
                                        );

                                        ui.selectable_value(
                                            &mut state.active_debug_stage,
                                            5,
                                            "5. Indirect Raw Vector Offset",
                                        );

                                        ui.selectable_value(
                                            &mut state.active_debug_stage,
                                            6,
                                            "6. Indirect Displaced UV Coordinates",
                                        );

                                        ui.selectable_value(
                                            &mut state.active_debug_stage,
                                            7,
                                            "7. Indirect Isolated Sample Output",
                                        );

                                        ui.selectable_value(
                                            &mut state.active_debug_stage,
                                            8,
                                            "8. Composite Layer Alpha (Grayscale)",
                                        );
                                    });
                            });
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

                if ui.button("Load MALs...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Supported files", SUPPORTED_SARC_EXTENSIONS)
                        .pick_file()
                    {
                        state.pending_action = Some(UiAction::LoadMal(path));
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
    for child in view.descendants(idx) {
        hidden_set.insert(child);
    }
}

fn show_pane_recursive(idx: usize, view: &BflytView, hidden_set: &mut HashSet<usize>) {
    hidden_set.remove(&idx);
    for child in view.descendants(idx) {
        hidden_set.remove(&child);
    }
}

fn draw_pane_properties(ui: &mut Ui, pane: &PaneInfo) {
    egui::ScrollArea::vertical()
        .id_salt("pane_properties_scroll")
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.heading("Core Properties");
            ui.add_space(4.0);

            egui::Grid::new("pane_info_core")
                .num_columns(2)
                .striped(true)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    draw_string(ui, "Name", &pane.label);
                    draw_string(ui, "Kind", &pane.kind);

                    draw_prop_f32(ui, "X", pane.x);
                    draw_prop_f32(ui, "Y", pane.y);
                    draw_prop_f32(ui, "Width", pane.width);
                    draw_prop_f32(ui, "Height", pane.height);
                    draw_prop(ui, "Depth", pane.depth);
                    draw_prop(ui, "Visible", pane.visible);

                    if let Some(source) = &pane.parts_source {
                        draw_string(ui, "Parts Source", source);
                    }

                    if let Some(parent_idx) = &pane.parent_idx {
                        draw_prop(ui, "Parent Index", parent_idx);
                    }
                });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            ui.heading("Section Details");
            ui.add_space(4.0);

            match &pane.section {
                BflytSection::Layout(layout) => {
                    egui::Grid::new("bflyt_layout_grid")
                        .num_columns(2)
                        .striped(true)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            draw_layout_section(ui, layout);
                        });
                }
                BflytSection::Pane(pane_detail) => {
                    egui::Grid::new("bflyt_pane_grid")
                        .num_columns(2)
                        .striped(true)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            draw_pane_section(ui, pane_detail);
                        });
                }
                _ => {
                    ui.weak("Unimplemented section metadata type");
                }
            }
        });
}

fn draw_layout_section(ui: &mut Ui, layout: &BflytLayout) {
    draw_string(ui, "Name", &layout.name);
    draw_prop(ui, "Centered", layout.is_centered);
    draw_prop_f32(ui, "Width", layout.width);
    draw_prop_f32(ui, "Height", layout.height);
    draw_prop_f32(ui, "Parts Width", layout.parts_width);
    draw_prop_f32(ui, "Parts Height", layout.parts_height);
}

fn draw_pane_section(ui: &mut Ui, pane: &BflytPane) {
    draw_string(ui, "Name", &pane.pane_name);
    draw_prop_debug(ui, "Origin X", pane.origin.origin_x);
    draw_prop_debug(ui, "Origin Y", pane.origin.origin_y);
    draw_prop_debug(ui, "Parent Origin X", pane.origin.parent_origin_x);
    draw_prop_debug(ui, "Parent Origin Y", pane.origin.parent_origin_y);

    draw_vector_3f(ui, "Translation", pane.translation);
    draw_vector_3f(ui, "Rotation", pane.rotation);
    draw_vector_2f(ui, "Scale", pane.scale);
    draw_vector_2f(ui, "Size", pane.size);

    draw_prop(ui, "Alpha", pane.alpha);
    draw_prop(ui, "Influenced Alpha", pane.pane_flags.influenced_alpha);
    draw_prop(ui, "Visible", pane.pane_flags.is_visible);

    draw_prop(ui, "Extended User Data", pane.flag_ex.is_ext_user_data);
    draw_prop(ui, "No Scale By Parts", pane.flag_ex.is_no_scale_by_parts);
    draw_prop(
        ui,
        "Scale Size By Parts Root",
        pane.flag_ex.is_scale_size_by_parts_root,
    );
}

fn draw_material_list(ui: &mut Ui, list: &BflytMaterialList) {
    ui.label(format!("Total Materials: {}", list.materials.len()));
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .id_salt("material_sidebar_scroll")
        .show(ui, |ui| {
            for (idx, material) in list.materials.iter().enumerate() {
                let header_text = format!("[{idx}] {}", material.material_name);

                egui::CollapsingHeader::new(header_text)
                    .id_salt(ui.id().with(idx))
                    .show(ui, |ui| {
                        if !material.colors.is_empty() {
                            egui::CollapsingHeader::new(format!(
                                "Colors ({})",
                                material.colors.len()
                            ))
                            .id_salt("colors")
                            .show(ui, |ui| {
                                draw_vec_grid(
                                    ui,
                                    "colors_grid",
                                    &material.colors,
                                    |ui, i, color| {
                                        if let Some(color) = &color.color_f32 {
                                            draw_prop_f32(ui, &format!("[{i}] Red"), color.r);
                                            draw_prop_f32(ui, &format!("[{i}] Green"), color.g);
                                            draw_prop_f32(ui, &format!("[{i}] Blue"), color.b);
                                            draw_prop_f32(ui, &format!("[{i}] Alpha"), color.a);
                                        } else if let Some(color) = &color.color_u8 {
                                            draw_prop(ui, &format!("[{i}] Red"), color.r);
                                            draw_prop(ui, &format!("[{i}] Green"), color.g);
                                            draw_prop(ui, &format!("[{i}] Blue"), color.b);
                                            draw_prop(ui, &format!("[{i}] Alpha"), color.a);
                                        }
                                    },
                                );
                            });
                        }

                        if !material.tex_maps.is_empty() {
                            egui::CollapsingHeader::new(format!(
                                "Texture Maps ({})",
                                material.tex_maps.len()
                            ))
                            .id_salt("tex_sub")
                            .show(ui, |ui| {
                                draw_vec_grid(ui, "tex_grid", &material.tex_maps, |ui, i, tex| {
                                    draw_string(ui, &format!("[{i}] Name"), &tex.texture_name);
                                    draw_prop_debug(
                                        ui,
                                        &format!("[{i}] U Filter"),
                                        tex.u_options.filter,
                                    );
                                    draw_prop_debug(
                                        ui,
                                        &format!("[{i}] V Filter"),
                                        tex.v_options.filter,
                                    );
                                    draw_prop_debug(
                                        ui,
                                        &format!("[{i}] U Wrap"),
                                        tex.u_options.wrap_mode,
                                    );
                                    draw_prop_debug(
                                        ui,
                                        &format!("[{i}] V Wrap"),
                                        tex.v_options.wrap_mode,
                                    );
                                });
                            });
                        }

                        if !material.tex_extensions.is_empty() {
                            egui::CollapsingHeader::new(format!(
                                "Texture Extensions ({})",
                                material.tex_extensions.len()
                            ))
                            .id_salt("tex_ext")
                            .show(ui, |ui| {
                                draw_vec_grid(
                                    ui,
                                    "tex_ext_grid",
                                    &material.tex_extensions,
                                    |ui, i, ext| {
                                        draw_prop(
                                            ui,
                                            &format!("[{i}] Capture Tex"),
                                            ext.is_capture_texture,
                                        );
                                        draw_prop(
                                            ui,
                                            &format!("[{i}] Vector Tex"),
                                            ext.is_vecture_texture,
                                        );
                                    },
                                );
                            });
                        }

                        if !material.tex_srts.is_empty() {
                            egui::CollapsingHeader::new(format!(
                                "Texture SRTs ({})",
                                material.tex_srts.len()
                            ))
                            .id_salt("tex_srt")
                            .show(ui, |ui| {
                                draw_vec_grid(ui, "srt_grid", &material.tex_srts, |ui, i, srt| {
                                    draw_prop_f32(ui, &format!("[{i}] Rotate"), srt.rotate);
                                    draw_prop_f32(ui, &format!("[{i}] Scale U"), srt.scale_u);
                                    draw_prop_f32(ui, &format!("[{i}] Scale V"), srt.scale_v);
                                    draw_prop_f32(
                                        ui,
                                        &format!("[{i}] Translate U"),
                                        srt.translate_u,
                                    );
                                    draw_prop_f32(
                                        ui,
                                        &format!("[{i}] Translate V"),
                                        srt.translate_v,
                                    );
                                });
                            });
                        }

                        if !material.tex_coord_gens.is_empty() {
                            egui::CollapsingHeader::new(format!(
                                "Texture Coord Gens ({})",
                                material.tex_coord_gens.len()
                            ))
                            .id_salt("tex_gen")
                            .show(ui, |ui| {
                                draw_vec_grid(
                                    ui,
                                    "coord_gen_grid",
                                    &material.tex_coord_gens,
                                    |ui, i, coord_gen| {
                                        draw_prop_debug(
                                            ui,
                                            &format!("[{i}] Source"),
                                            coord_gen.tex_gen_source,
                                        );
                                    },
                                );
                            });
                        }

                        if !material.projection_tex_gens.is_empty() {
                            egui::CollapsingHeader::new(format!(
                                "Projection Tex Gens ({})",
                                material.projection_tex_gens.len()
                            ))
                            .id_salt("proj_gen")
                            .show(ui, |ui| {
                                draw_vec_grid(
                                    ui,
                                    "proj_gen_grid",
                                    &material.projection_tex_gens,
                                    |ui, i, proj_gen| {
                                        draw_prop(
                                            ui,
                                            &format!("[{i}] Adjust Projection Scale Rotate"),
                                            proj_gen.flags.adjust_projection_scale_rotate,
                                        );

                                        draw_prop(
                                            ui,
                                            &format!("[{i}] Fitting Layout Size"),
                                            proj_gen.flags.fitting_layout_size,
                                        );

                                        draw_prop(
                                            ui,
                                            &format!("[{i}] Fitting Pane Size"),
                                            proj_gen.flags.fitting_pane_size,
                                        );

                                        draw_vector_2f(
                                            ui,
                                            &format!("[{i}] Translation"),
                                            proj_gen.scale,
                                        );
                                        draw_vector_2f(
                                            ui,
                                            &format!("[{i}] Scale"),
                                            proj_gen.translation,
                                        );
                                    },
                                );
                            });
                        }

                        if !material.tev_combiners.is_empty() {
                            egui::CollapsingHeader::new(format!(
                                "Texture Environment Combiners ({})",
                                material.tev_combiners.len()
                            ))
                            .id_salt("tev_comb")
                            .show(ui, |ui| {
                                draw_vec_grid(
                                    ui,
                                    "tev_grid",
                                    &material.tev_combiners,
                                    |ui, i, combiner| {
                                        draw_prop_debug(
                                            ui,
                                            &format!("[{i}] RGB Mode"),
                                            combiner.rgb_mode,
                                        );
                                        draw_prop_debug(
                                            ui,
                                            &format!("[{i}] Alpha Mode"),
                                            combiner.alpha_mode,
                                        );
                                    },
                                );
                            });
                        }

                        if material.alpha_compare.is_some() {
                            egui::CollapsingHeader::new("Alpha Compare")
                                .id_salt("alp_comp")
                                .show(ui, |ui| {
                                    if let Some(compare) = &material.alpha_compare {
                                        egui::Grid::new(ui.id().with("alpha_comp_grid"))
                                            .striped(true)
                                            .show(ui, |ui| {
                                                draw_prop_debug(ui, "Compare OP", compare.compare);
                                                draw_prop_f32(
                                                    ui,
                                                    "Reference Value",
                                                    compare.alpha_compare_ref_value,
                                                );
                                            });
                                    } else {
                                        ui.weak("None");
                                    }
                                });
                        }

                        if material.blend_mode.is_some() {
                            egui::CollapsingHeader::new("Blend Mode")
                                .id_salt("blend_mode")
                                .show(ui, |ui| {
                                    egui::Grid::new(ui.id().with("blend_grid"))
                                        .striped(true)
                                        .show(ui, |ui| {
                                            draw_blend_mode(ui, &material.blend_mode);
                                        });
                                });
                        }

                        if material.blend_mode_alpha.is_some() {
                            egui::CollapsingHeader::new("Alpha Blend Mode")
                                .id_salt("alp_blend_mode")
                                .show(ui, |ui| {
                                    egui::Grid::new(ui.id().with("alpha_blend_grid"))
                                        .striped(true)
                                        .show(ui, |ui| {
                                            draw_blend_mode(ui, &material.blend_mode_alpha);
                                        });
                                });
                        }

                        if let Some(indirect_matrix) = &material.indirect_matrix {
                            egui::CollapsingHeader::new("Indirect Matrix")
                                .id_salt("ind_mtx")
                                .show(ui, |ui| {
                                    egui::Grid::new(ui.id().with("ind_mtx_grid"))
                                        .striped(true)
                                        .show(ui, |ui| {
                                            draw_prop(ui, "Rotation", indirect_matrix.rotation);
                                            draw_vector_2f(ui, "Scale", indirect_matrix.scale);
                                        });
                                });
                        }

                        if let Some(fcs) = &material.font_shadow_color {
                            egui::CollapsingHeader::new("Font Shadow Color")
                                .id_salt("f_sh_clr")
                                .show(ui, |ui| {
                                    egui::Grid::new(ui.id().with("f_sh_clr_grid"))
                                        .striped(true)
                                        .show(ui, |ui| {
                                            draw_prop(ui, &format!("Color 1, Red"), fcs.color0.r);
                                            draw_prop(ui, &format!("Color 1, Green"), fcs.color0.g);
                                            draw_prop(ui, &format!("Color 1, Blue"), fcs.color0.b);
                                            draw_prop(ui, &format!("Color 1, Alpha"), fcs.color0.a);
                                            draw_prop(ui, &format!("Color 2, Red"), fcs.color1.r);
                                            draw_prop(ui, &format!("Color 2, Green"), fcs.color1.g);
                                            draw_prop(ui, &format!("Color 2, Blue"), fcs.color1.b);
                                            draw_prop(ui, &format!("Color 2, Alpha"), fcs.color1.a);
                                        });
                                });
                        }

                        // TODO: add
                        /*if let Some(dc) = &material.detailed_combiner {
                            egui::CollapsingHeader::new("Detailed Combiner")
                                .id_salt("dt_comb")
                                .show(ui, |ui| {
                                    egui::Grid::new(ui.id().with("dt_comb_grid"))
                                        .striped(true)
                                        .show(ui, |ui| {
                                            draw_prop(ui, "Stage Flags", dc.stage_flags);
                                            draw_prop(ui, "Stage Flags", dc.);
                                        });
                                });
                        }*/
                    });
            }
        });
}

fn draw_vec_grid<T>(
    ui: &mut Ui,
    id_source: &str,
    items: &[T],
    mut draw_item: impl FnMut(&mut Ui, usize, &T),
) {
    if items.is_empty() {
        ui.weak("None");
        return;
    }

    let len = items.len();
    egui::Grid::new(ui.id().with(id_source))
        .striped(true)
        .show(ui, |ui| {
            for (i, item) in items.iter().enumerate() {
                draw_item(ui, i, item);

                if i < len - 1 {
                    ui.label("-");
                    ui.label("-");
                    ui.end_row();
                }
            }
        });
}

fn draw_blend_mode(ui: &mut Ui, blend_mode: &Option<MaterialBlendMode>) {
    if let Some(blend_mode) = blend_mode {
        match blend_mode {
            MaterialBlendMode::None => {
                ui.weak("None");
            }
            MaterialBlendMode::Logic { logic_op } => {
                draw_prop_debug(ui, "Logic OP", logic_op);
            }
            MaterialBlendMode::Blend {
                blend_op,
                function_source,
                function_destination,
            } => {
                draw_prop_debug(ui, "Blend OP", blend_op);
                draw_prop_debug(ui, "Function Source", function_source);
                draw_prop_debug(ui, "Function Destination", function_destination);
            }
        }
    } else {
        ui.weak("None");
    }
}

fn draw_vector_2f(ui: &mut egui::Ui, label: &str, vector: Vector2f) {
    ui.strong(label);
    ui.label(format!("({:.2}, {:.2})", vector.x, vector.y));
    ui.end_row();
}

fn draw_vector_3f(ui: &mut egui::Ui, label: &str, vector: Vector3f) {
    ui.strong(label);
    ui.label(format!(
        "({:.2}, {:.2}, {:.2})",
        vector.x, vector.y, vector.z
    ));
    ui.end_row();
}

fn draw_prop(ui: &mut egui::Ui, label: &str, value: impl std::fmt::Display) {
    ui.strong(label);
    ui.label(value.to_string());
    ui.end_row();
}

fn draw_prop_debug(ui: &mut egui::Ui, label: &str, value: impl std::fmt::Debug) {
    ui.strong(label);
    ui.label(format!("{:?}", value));
    ui.end_row();
}

fn draw_string(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.strong(label);
    ui.label(value);
    ui.end_row();
}

fn draw_prop_f32(ui: &mut egui::Ui, label: &str, value: f32) {
    ui.strong(label);
    ui.label(format!("{:.2}", value));
    ui.end_row();
}

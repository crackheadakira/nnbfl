use std::{collections::HashSet, path::PathBuf};

use tomolib::formats::bntx::{Bntx, Texture, image::can_encode};

use crate::{
    font::glyph_atlas::{GLYPH_ATLAS_TEXTURE_NAME, GlyphAtlas},
    renderer::texture::PreviewCallback,
    texture_import::{png_size, texture_name_error},
    ui::{DrawUiWith, general::UiAction},
};

#[derive(Default)]
pub struct TextureEditor {
    pub is_editor_visible: bool,
    pub selected_texture: Option<String>,
    import: Option<ImportForm>,
    replace: Option<PickedPng>,
}

struct PickedPng {
    texture: String,
    path: PathBuf,
    size: Result<(u32, u32), String>,
}

struct ImportForm {
    template: String,
    name: String,
    png: Option<PickedPng>,
    resize: bool,
}

pub struct TextureEditState<'a> {
    pub main_bntx: Option<&'a Bntx>,
    pub atlas: &'a GlyphAtlas,
    pub editable: bool,
    pub taken: &'a HashSet<String>,
}

fn pick_png(texture: &str) -> Option<PickedPng> {
    let path = rfd::FileDialog::new()
        .set_title("Choose a PNG")
        .add_filter("PNG image", &["png"])
        .pick_file()?;

    Some(PickedPng {
        texture: texture.to_string(),
        size: png_size(&path),
        path,
    })
}

impl TextureEditor {
    fn edit_controls(
        &mut self,
        ui: &mut egui::Ui,
        tex: &Texture,
        taken: &HashSet<String>,
    ) -> Option<UiAction> {
        let mut action = None;
        let target = (tex.info.width, tex.info.height);

        if !can_encode(&tex.info) {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!("{} textures can't be encoded", tex.info.format.name()),
            );
        }

        ui.horizontal(|ui| {
            if ui.button("Replace texture...").clicked()
                && let Some(picked) = pick_png(&tex.name)
            {
                if picked.size.as_ref().is_ok_and(|size| *size == target) {
                    action = Some(UiAction::ReplaceTexture {
                        name: tex.name.clone(),
                        png: picked.path,
                        resize: false,
                    });
                } else {
                    self.replace = Some(picked);
                }
            }

            if ui.button("Import as new texture…").clicked() {
                self.import = Some(ImportForm {
                    template: tex.name.clone(),
                    name: format!("{}_New", tex.name),
                    png: None,
                    resize: true,
                });
            }
        });

        if let Some(picked) = self.replace.take_if(|p| p.texture == tex.name) {
            let mut keep = true;

            match &picked.size {
                Ok((w, h)) => {
                    ui.label(format!(
                        "{} is {w}x{h}, the texture is {}x{}",
                        picked.path.display(),
                        target.0,
                        target.1
                    ));

                    ui.horizontal(|ui| {
                        if ui.button("Resize and replace").clicked() {
                            action = Some(UiAction::ReplaceTexture {
                                name: tex.name.clone(),
                                png: picked.path.clone(),
                                resize: true,
                            });

                            keep = false;
                        }

                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                }
                Err(err) => {
                    ui.colored_label(ui.visuals().error_fg_color, err);
                    keep = !ui.button("Dismiss").clicked();
                }
            }

            if keep {
                self.replace = Some(picked);
            }
        }

        let mut close = false;
        if let Some(form) = self.import.as_mut().filter(|f| f.template == tex.name) {
            ui.add_space(8.0);
            ui.strong(format!("New texture like {}", tex.name));

            let name_error = texture_name_error(&form.name, |name| taken.contains(name));
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut form.name);
            });

            if let Some(error) = &name_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }

            ui.horizontal(|ui| {
                if ui.button("Choose PNG…").clicked()
                    && let Some(picked) = pick_png(&tex.name)
                {
                    form.png = Some(picked);
                }

                match &form.png {
                    None => {
                        ui.weak("No PNG chosen");
                    }
                    Some(PickedPng {
                        path,
                        size: Ok((w, h)),
                        ..
                    }) => {
                        ui.label(format!("{} ({w}x{h})", path.display()));
                    }
                    Some(PickedPng { size: Err(err), .. }) => {
                        ui.colored_label(ui.visuals().error_fg_color, err);
                    }
                }
            });

            let mismatch = form
                .png
                .as_ref()
                .and_then(|p| p.size.as_ref().ok())
                .is_some_and(|size| *size != target);

            if mismatch {
                ui.checkbox(
                    &mut form.resize,
                    format!("Resize to {}x{}", target.0, target.1),
                );
            }

            let ready = name_error.is_none()
                && form.png.as_ref().is_some_and(|p| p.size.is_ok())
                && (!mismatch || form.resize);

            ui.horizontal(|ui| {
                if ui.add_enabled(ready, egui::Button::new("Import")).clicked()
                    && let Some(png) = &form.png
                {
                    action = Some(UiAction::ImportTexture {
                        template: tex.name.clone(),
                        name: form.name.clone(),
                        png: png.path.clone(),
                        resize: mismatch,
                    });
                    self.selected_texture = Some(form.name.clone());
                    close = true;
                }

                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        }

        if close {
            self.import = None;
        }

        action
    }
}

impl DrawUiWith<TextureEditState<'_>, Option<UiAction>> for TextureEditor {
    fn draw_with(&mut self, ui: &mut egui::Ui, state: TextureEditState) -> Option<UiAction> {
        let main_bntx = state.main_bntx;
        let atlas = state.atlas;
        let mut action = None;
        let mut open = self.is_editor_visible;

        egui::Window::new("Texture Editor")
            .collapsible(false)
            .resizable(true)
            .default_size([560.0, 400.0])
            .open(&mut open)
            .fade_out(false)
            .show(ui, |ui| {
                egui::Panel::left("texture_editor_list")
                    .resizable(true)
                    .min_size(180.0)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("texture_editor_list_scroll")
                            .auto_shrink(false)
                            .show(ui, |ui| {
                                let total_count = main_bntx.map_or(1, |b| b.textures.len() + 1);
                                ui.weak(format!("{} Total Textures", total_count));
                                ui.separator();

                                let is_atlas_selected = self.selected_texture.as_deref()
                                    == Some(GLYPH_ATLAS_TEXTURE_NAME);

                                if ui
                                    .selectable_label(is_atlas_selected, "Glyph Atlas")
                                    .clicked()
                                {
                                    self.selected_texture =
                                        Some(GLYPH_ATLAS_TEXTURE_NAME.to_string());
                                }

                                ui.separator();

                                if let Some(bntx) = main_bntx {
                                    for tex in &bntx.textures {
                                        let is_selected =
                                            self.selected_texture.as_deref() == Some(&tex.name);

                                        if ui.selectable_label(is_selected, &tex.name).clicked() {
                                            self.selected_texture = Some(tex.name.clone());
                                        }
                                    }
                                } else {
                                    ui.weak("No additional textures in file.");
                                }
                            });
                    });

                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: 0,
                        bottom: 0,
                    })
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("texture_editor_detail_scroll")
                            .auto_shrink(false)
                            .show(ui, |ui| {
                                let Some(selected_name) = self.selected_texture.clone() else {
                                    ui.label("Select a texture on the left.");
                                    return;
                                };

                                let is_atlas = selected_name == GLYPH_ATLAS_TEXTURE_NAME;

                                let (width, height, format_str, mips, arrays) = if is_atlas {
                                    let atlas_size = atlas.used_bounds();

                                    let active_w = atlas_size.0.max(64);
                                    let active_h = atlas_size.1.max(active_w);

                                    (active_w, active_h, "R8_UNORM / RGBA8".to_owned(), 1, 1)
                                } else if let Some(tex) = main_bntx
                                    .into_iter()
                                    .flat_map(|bntx| bntx.textures.iter())
                                    .find(|tex| tex.name == selected_name)
                                {
                                    (
                                        tex.info.width,
                                        tex.info.height,
                                        tex.info.format.name(),
                                        tex.info.mip_count,
                                        tex.info.array_count,
                                    )
                                } else {
                                    ui.label("This texture is no longer loaded.");
                                    return;
                                };

                                ui.heading(if is_atlas {
                                    "Glyph Atlas (System Font)"
                                } else {
                                    &selected_name
                                });

                                ui.add_space(4.0);

                                egui::Grid::new("texture_editor_info_grid")
                                    .num_columns(2)
                                    .spacing([16.0, 6.0])
                                    .show(ui, |ui| {
                                        ui.label("Dimensions:");
                                        ui.label(format!("{} x {}", width, height));
                                        ui.end_row();

                                        ui.label("Format:");
                                        ui.label(format_str);
                                        ui.end_row();

                                        ui.label("Mip Levels:");
                                        ui.label(mips.to_string());
                                        ui.end_row();

                                        ui.label("Array Count:");
                                        ui.label(arrays.to_string());
                                        ui.end_row();
                                    });

                                if !is_atlas
                                    && let Some(tex) = main_bntx
                                        .into_iter()
                                        .flat_map(|bntx| bntx.textures.iter())
                                        .find(|tex| tex.name == selected_name)
                                {
                                    ui.add_space(8.0);

                                    if state.editable {
                                        action = self.edit_controls(ui, tex, state.taken);
                                    } else {
                                        ui.weak(
                                            "Textures can only be edited in layouts opened from an archive",
                                        );
                                    }
                                }

                                ui.add_space(8.0);
                                ui.separator();
                                ui.add_space(4.0);

                                let aspect = width as f32 / (height.max(1) as f32);

                                let avail_width = ui.available_width().max(64.0);
                                let preview_height = avail_width / aspect;

                                let size = egui::vec2(avail_width, preview_height);

                                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());

                                ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                                    rect,
                                    PreviewCallback {
                                        texture_name: selected_name,
                                    },
                                ));
                            });
                    });
            });

        self.is_editor_visible = open;
        action
    }
}

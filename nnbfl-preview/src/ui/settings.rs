use std::path::PathBuf;

use crate::{
    config::{AppConfig, GameProfile, PRESETS},
    game_config::Language,
    ui::{
        DrawUiWith,
        array_editor::ArrayEditor,
        editors::fields::{grid, row, text},
    },
};

#[derive(Debug, Default)]
pub struct SettingsWindow {
    pub is_open: bool,
}

impl DrawUiWith<&mut AppConfig, bool> for SettingsWindow {
    fn draw_with(&mut self, ui: &mut egui::Ui, config: &mut AppConfig) -> bool {
        let mut changed = false;
        let mut is_open = self.is_open;

        egui::Window::new("Settings")
            .open(&mut is_open)
            .collapsible(false)
            .default_width(460.0)
            .show(ui.ctx(), |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Game Profile");
                    changed |= profile_combo(ui, "settings_active_profile", config);

                    ui.separator();
                    ui.heading("System Fonts");
                    ui.weak(
                        "The console's shared fonts (nintendo_udsg-r_std_003.bfttf and others) ship with no game. \
                         \nDump them from your own console, or copy them from you emulator setup, \
                         and choose that folder here.",
                    );

                    changed |= grid(ui, "settings_fonts", |ui| {
                        row(ui, "Folder", |ui| folder_picker(ui, &mut config.system_font_dir))
                    });

                    ui.separator();
                    ui.heading("Language");

                    changed |= grid(ui, "settings_language", |ui| {
                        row(ui, "Default", |ui| {
                            language_combo(ui, "settings_default_language", &mut config.language)
                        })
                    });

                    ui.separator();
                    ui.heading("Profiles");
                    ui.weak("A profile is a font folder (or font archive) and a layout folder");

                    ui.horizontal(|ui| {
                        ui.menu_button("New From Preset", |ui| {
                            for preset in PRESETS {
                                if ui.button(preset.name).on_hover_text(preset.hint).clicked() {
                                    config.profiles.push(GameProfile::from_preset(preset));
                                    changed = true;
                                    ui.close();
                                }
                            }
                        });

                        if ui.button("Import…").clicked()
                            && let Some(path) =
                                rfd::FileDialog::new().add_filter("Profile", &["toml"]).pick_file()
                        {
                            match GameProfile::load_from(&path) {
                                Ok(profile) => {
                                    config.profiles.push(profile);
                                    changed = true;
                                }
                                Err(e) => log::error!("Couldn't import {}: {e}", path.display()),
                            }
                        }
                    });

                    let active = config
                        .profiles
                        .iter()
                        .position(|p| Some(&p.name) == config.active_profile.as_ref());

                    let count = config.profiles.len();

                    let edited = ArrayEditor::new("settings_profiles", &mut config.profiles)
                        .item_label(|i, p: &GameProfile| {
                            if p.name.is_empty() {
                                format!("Profile {}", i + 1)
                            } else {
                                p.name.clone()
                            }
                        })
                        .show(ui, profile_fields)
                        .changed;

                    if edited {
                        config.active_profile = active
                            .filter(|_| config.profiles.len() == count)
                            .and_then(|i| config.profiles.get(i))
                            .map(|p| p.name.clone());
                        changed = true;
                    }
                });
            });

        self.is_open = is_open;
        changed
    }
}

pub fn profile_combo(ui: &mut egui::Ui, id: &str, config: &mut AppConfig) -> bool {
    let shown = config
        .active()
        .map_or("None", |p| p.name.as_str())
        .to_string();

    let mut changed = false;

    egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .show_ui(ui, |ui| {
            changed |= ui
                .selectable_value(&mut config.active_profile, None, "None")
                .changed();

            for profile in &config.profiles {
                let name = Some(profile.name.clone());
                changed |= ui
                    .selectable_value(&mut config.active_profile, name, &profile.name)
                    .changed();
            }
        });

    changed
}

fn profile_fields(ui: &mut egui::Ui, i: usize, profile: &mut GameProfile) -> bool {
    let changed = grid(ui, ("settings_profile", i), |ui| {
        let mut changed = text(ui, "Name", &mut profile.name, usize::MAX);

        changed |= row(ui, "Fonts", |ui| {
            let changed = path_picker(ui, &mut profile.fonts, true);

            if !profile.fonts.as_os_str().is_empty() && !profile.fonts.exists() {
                ui.colored_label(ui.visuals().error_fg_color, "Not found");
            }
            changed
        });

        changed |= row(ui, "Layouts", |ui| {
            path_picker(ui, &mut profile.layouts, false)
        });

        let mut patterns = profile.font_files.join(", ");

        if text(ui, "Font Files", &mut patterns, usize::MAX) {
            profile.font_files = patterns
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();

            changed |= true;
        }

        changed
    });

    ui.weak("Font files: comma-separated patterns such as *.sarc.zs; empty uses every file");

    if ui.button("Export…").clicked()
        && let Some(path) = rfd::FileDialog::new()
            .add_filter("Profile", &["toml"])
            .set_file_name(format!("{}.toml", profile.name))
            .save_file()
        && let Err(e) = profile.save_to(&path)
    {
        log::error!("Couldn't export to {}: {e}", path.display());
    }

    changed
}

fn path_picker(ui: &mut egui::Ui, path: &mut PathBuf, allow_file: bool) -> bool {
    let mut folder = Some(path.clone()).filter(|p| !p.as_os_str().is_empty());
    let mut changed = folder_picker(ui, &mut folder);

    if allow_file
        && ui.button("Archive…").clicked()
        && let Some(file) = rfd::FileDialog::new().pick_file()
    {
        folder = Some(file);
        changed = true;
    }

    *path = folder.unwrap_or_default();
    changed
}

fn language_combo(ui: &mut egui::Ui, id: &str, language: &mut Language) -> bool {
    let mut changed = false;

    egui::ComboBox::from_id_salt(id)
        .selected_text(language.label())
        .show_ui(ui, |ui| {
            for &option in Language::ALL {
                changed |= ui
                    .selectable_value(language, option, option.label())
                    .changed();
            }
        });

    changed
}

fn folder_picker(ui: &mut egui::Ui, folder: &mut Option<PathBuf>) -> bool {
    ui.horizontal(|ui| {
        let mut changed = false;

        match folder {
            Some(path) => ui.label(path.display().to_string()),
            None => ui.weak("Not set"),
        };

        if ui.button("Choose…").clicked()
            && let Some(dir) = rfd::FileDialog::new().pick_folder()
        {
            *folder = Some(dir);
            changed = true;
        }

        if folder.is_some() && ui.button("Clear").clicked() {
            *folder = None;
            changed = true;
        }

        changed
    })
    .inner
}

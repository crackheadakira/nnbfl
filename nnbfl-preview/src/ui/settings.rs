use std::path::PathBuf;

use crate::{
    config::{AppConfig, GameKind, GameProfile},
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
                    ui.weak("Fonts a layout's archive lacks are looked up in the active game's romfs");
                    changed |= profile_combo(ui, "settings_active_profile", config);

                    ui.separator();
                    ui.heading("System Fonts");
                    ui.weak(
                        "The console's shared fonts (nintendo_udsg-r_std_003.bfttf and others) ship with no game. \
                         Dump them from your own console, or copy them from you emulator setup, \
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
                        .show(ui, |ui, i, profile| profile_fields(ui, i, profile))
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
    grid(ui, ("settings_profile", i), |ui| {
        let mut changed = text(ui, "Name", &mut profile.name, usize::MAX);

        changed |= row(ui, "Game", |ui| {
            let mut changed = false;

            egui::ComboBox::from_id_salt(("settings_profile_kind", i))
                .selected_text(profile.kind.label())
                .show_ui(ui, |ui| {
                    for kind in GameKind::ALL {
                        changed |= ui
                            .selectable_value(&mut profile.kind, kind, kind.label())
                            .changed();
                    }
                });

            changed
        });

        changed |= row(ui, "romfs", |ui| {
            let mut romfs = Some(profile.romfs.clone()).filter(|p| !p.as_os_str().is_empty());
            let changed = folder_picker(ui, &mut romfs);
            profile.romfs = romfs.unwrap_or_default();
            changed
        });

        changed |= row(ui, "Language", |ui| {
            let mut changed = false;

            egui::ComboBox::from_id_salt(("settings_profile_language", i))
                .selected_text(profile.language.map_or("Default", |l| l.label()))
                .show_ui(ui, |ui| {
                    changed |= ui
                        .selectable_value(&mut profile.language, None, "Default")
                        .changed();

                    for &language in Language::ALL {
                        changed |= ui
                            .selectable_value(
                                &mut profile.language,
                                Some(language),
                                language.label(),
                            )
                            .changed();
                    }
                });

            changed
        });

        changed
    })
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

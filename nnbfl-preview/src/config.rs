use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::game_config::Language;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub active_profile: Option<String>,
    pub system_font_dir: Option<PathBuf>,
    pub layout_dir: Option<PathBuf>,
    pub language: Language,
    pub output_gamma: OutputGamma,
    pub profiles: Vec<GameProfile>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
pub enum OutputGamma {
    #[default]
    Unorm,
    Srgb,
}

impl OutputGamma {
    pub fn label(self) -> &'static str {
        match self {
            OutputGamma::Unorm => "UNORM",
            OutputGamma::Srgb => "sRGB",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameProfile {
    pub name: String,
    pub fonts: PathBuf,
    pub layouts: PathBuf,

    /// File name patterns (`*` wildcards) picking the font sources in `fonts`
    pub font_files: Vec<String>,
    pub output_gamma: OutputGamma,
}

pub struct Preset {
    pub name: &'static str,
    pub font_files: &'static [&'static str],
    pub output_gamma: OutputGamma,
    pub hint: &'static str,
}

pub const PRESETS: &[Preset] = &[
    Preset {
        name: "Generic",
        font_files: &[],
        output_gamma: OutputGamma::Unorm,
        hint: "Every file in the font folder",
    },
    Preset {
        name: "Animal Crossing: New Horizons",
        font_files: &["*.sarc.zs"],
        output_gamma: OutputGamma::Unorm,
        hint: "Fonts: romfs/Font, layouts: romfs/Layout",
    },
    Preset {
        name: "Tomodachi Life: Living the Dream",
        font_files: &["Font*.bfarc.zs"],
        output_gamma: OutputGamma::Srgb,
        hint: "Fonts: romfs/Font, layouts: romfs/Layout",
    },
    Preset {
        name: "Mario Kart 8 Deluxe",
        font_files: &["font.sarc"],
        output_gamma: OutputGamma::Unorm,
        hint: "Fonts: romfs/UI/<language> (bitmap fonts), layouts: romfs/UI; UNORM output is a guess",
    },
];

impl GameProfile {
    pub fn from_preset(preset: &Preset) -> Self {
        Self {
            name: preset.name.to_string(),
            font_files: preset.font_files.iter().map(|p| p.to_string()).collect(),
            output_gamma: preset.output_gamma,
            ..Default::default()
        }
    }

    pub fn load_from(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        toml::from_str(&text).map_err(|e| e.to_string())
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }
}

impl AppConfig {
    pub fn path() -> Option<PathBuf> {
        Some(config_dir()?.join("nnbfl-preview").join("config.toml"))
    }

    pub fn load() -> Self {
        Self::path()
            .map(|p| Self::load_from(&p))
            .unwrap_or_default()
    }

    pub fn load_from(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };

        toml::from_str(&text).unwrap_or_else(|e| {
            log::warn!(
                "Config {} is unreadable, using defaults: {e}",
                path.display()
            );

            Self::default()
        })
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else {
            log::warn!("No config directory, settings are not saved");
            return;
        };

        if let Err(e) = self.save_to(&path) {
            log::warn!("Couldn't save config to {}: {e}", path.display());
        }
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }

        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }

    pub fn active(&self) -> Option<&GameProfile> {
        let name = self.active_profile.as_deref()?;
        self.profiles.iter().find(|p| p.name == name)
    }

    pub fn profile(&self, name: &str) -> Option<&GameProfile> {
        self.profiles.iter().find(|p| p.name == name)
    }

    pub fn output_gamma(&self) -> OutputGamma {
        self.active().map_or(self.output_gamma, |p| p.output_gamma)
    }

    pub fn set_output_gamma(&mut self, gamma: OutputGamma) {
        let name = self.active_profile.clone();

        match name.and_then(|n| self.profiles.iter_mut().find(|p| p.name == n)) {
            Some(profile) => profile.output_gamma = gamma,
            None => self.output_gamma = gamma,
        }
    }

    pub fn layout_dir(&self) -> Option<PathBuf> {
        match self.active() {
            Some(profile) if !profile.layouts.as_os_str().is_empty() => {
                Some(profile.layouts.clone())
            }
            _ => self.layout_dir.clone(),
        }
    }

    pub fn set_layout_dir(&mut self, dir: PathBuf) {
        let name = self.active_profile.clone();

        match name.and_then(|n| self.profiles.iter_mut().find(|p| p.name == n)) {
            Some(profile) => profile.layouts = dir,
            None => self.layout_dir = Some(dir),
        }
    }
}

fn config_dir() -> Option<PathBuf> {
    let env = |key: &str| {
        std::env::var_os(key)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };

    if cfg!(target_os = "windows") {
        env("APPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|home| home.join("Library/Application Support"))
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|home| home.join(".config")))
    }
}

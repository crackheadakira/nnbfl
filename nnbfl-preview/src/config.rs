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
    pub profiles: Vec<GameProfile>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameProfile {
    pub name: String,
    pub kind: GameKind,
    pub romfs: PathBuf,
    pub language: Option<Language>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameKind {
    #[default]
    Generic,
    AnimalCrossing,
    TomodachiLife,
}

impl GameKind {
    pub const ALL: [GameKind; 3] = [
        GameKind::Generic,
        GameKind::AnimalCrossing,
        GameKind::TomodachiLife,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GameKind::Generic => "Generic",
            GameKind::AnimalCrossing => "Animal Crossing: New Horizons",
            GameKind::TomodachiLife => "Tomodachi Life: Living the Dream",
        }
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

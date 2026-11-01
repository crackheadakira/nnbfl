use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use nnbfl::{bfttf::file::BinaryFontPlatform, sarc::file::MagicFiles};

use crate::{archive::EntryPath, config::GameProfile, game_config::Language};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontKind {
    Complex,

    /// Encrypted `.bfttf` / `.bfotf`
    Scalable,

    /// Plain `.ttf` / `.otf` / `.ttc`
    Plain,

    Bitmap,
}

#[derive(Debug)]
pub struct FontFile {
    pub name: String,
    pub kind: FontKind,
    pub bytes: Arc<[u8]>,
    pub origin: String,
}

/// Emulator dumps of the console's shared fonts
const DUMP_NAMES: &[(&str, &str)] = &[
    ("fontstandard.ttf", "nintendo_udsg-r_std_003.bfttf"),
    (
        "fontchinesesimplified.ttf",
        "nintendo_udsg-r_org_zh-cn_003.bfttf",
    ),
    (
        "fontextendedchinesesimplified.ttf",
        "nintendo_udsg-r_ext_zh-cn_003.bfttf",
    ),
    (
        "fontchinesetraditional.ttf",
        "nintendo_udjxh-db_zh-tw_003.bfttf",
    ),
    ("fontkorean.ttf", "nintendo_udsg-r_ko_003.bfttf"),
    ("fontnintendoextended.ttf", "nintendo_ext_003.bfttf"),
];

static NEXT_INDEX_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct FontIndex {
    id: u64,
    files: HashMap<String, Arc<FontFile>>,
}

impl Default for FontIndex {
    fn default() -> Self {
        Self {
            id: NEXT_INDEX_ID.fetch_add(1, Ordering::Relaxed),
            files: HashMap::new(),
        }
    }
}

impl FontIndex {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn get(&self, name: &str) -> Option<&Arc<FontFile>> {
        self.files.get(&key(name))
    }

    pub fn from_archive_files(origin: &str, files: &[(EntryPath, MagicFiles)]) -> Self {
        let mut index = Self::default();

        for (_, file) in files {
            index.add_magic(origin, file);
        }

        index
    }

    pub fn from_paths(paths: &[PathBuf]) -> Self {
        let mut index = Self::default();

        for path in paths {
            let Ok(data) = std::fs::read(path) else {
                log::warn!("Couldn't read font source {}", path.display());
                continue;
            };

            let origin = path.display().to_string();
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            let mut files = Vec::new();

            crate::extract_all_files_recursive(data, &mut files, name);

            for file in &files {
                index.add_magic(&origin, file);
            }
        }

        index
    }

    fn add_magic(&mut self, origin: &str, file: &MagicFiles) {
        let (Some(name), bytes) = magic_parts(file) else {
            return;
        };

        if let Some(kind) = classify(bytes) {
            self.add(name, kind, bytes.into(), origin);
        }
    }

    fn add(&mut self, name: &str, kind: FontKind, bytes: Arc<[u8]>, origin: &str) {
        let name = key(name);

        let file = Arc::new(FontFile {
            name: name.clone(),
            kind,
            bytes,
            origin: origin.to_string(),
        });

        let mut keys = vec![name.clone()];

        if let Some((stem, ext)) = name.rsplit_once(".bf") {
            keys.push(format!("{stem}.f{ext}"));
        }

        if let Some((_, firmware)) = DUMP_NAMES.iter().find(|(dump, _)| *dump == name) {
            keys.push(firmware.to_string());
        }

        for key in keys {
            self.files.entry(key).or_insert_with(|| file.clone());
        }
    }
}

pub fn profile_sources(profile: &GameProfile, language: Language) -> Vec<PathBuf> {
    if profile.fonts.is_file() {
        return vec![profile.fonts.clone()];
    }

    let mut files: Vec<PathBuf> = list_files(&profile.fonts, true)
        .into_iter()
        .filter(|path| {
            let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase());
            let name = name.as_deref().unwrap_or_default();
            profile.font_files.is_empty()
                || profile
                    .font_files
                    .iter()
                    .any(|p| wildcard(&p.to_lowercase(), name))
        })
        .collect();

    let code = language.code_str().to_lowercase();
    let region = format!("_{}.", language.region_short().to_lowercase());

    files.sort_by_key(|path| {
        let name = path.to_string_lossy().to_lowercase();
        !(name.contains(&code) || name.contains(&region))
    });

    files
}

fn wildcard(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((head, rest)) => {
            name.starts_with(head)
                && (0..=name.len() - head.len()).any(|i| {
                    name.is_char_boundary(head.len() + i) && wildcard(rest, &name[head.len() + i..])
                })
        }
    }
}

pub fn list_files(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();

    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_file() {
            out.push(path);
        } else if recursive && path.is_dir() {
            out.extend(list_files(&path, true));
        }
    }

    out.sort();
    out
}

fn key(name: &str) -> String {
    name.rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .to_lowercase()
}

fn classify(bytes: &[u8]) -> Option<FontKind> {
    let magic: [u8; 4] = bytes.get(..4)?.try_into().ok()?;

    match &magic {
        b"FCPX" => Some(FontKind::Complex),
        b"FFNT" | b"CFNT" | b"RFNT" => Some(FontKind::Bitmap),
        [0, 1, 0, 0] | b"OTTO" | b"true" | b"ttcf" => Some(FontKind::Plain),
        _ if BinaryFontPlatform::from_magic(u32::from_be_bytes(magic)).is_ok() => {
            Some(FontKind::Scalable)
        }
        _ => None,
    }
}

fn magic_parts(file: &MagicFiles) -> (Option<&str>, &[u8]) {
    let (name, bytes) = match file {
        MagicFiles::Bflyt(name, bytes)
        | MagicFiles::Bflan(name, bytes)
        | MagicFiles::Bntx(name, bytes)
        | MagicFiles::Bfcpx(name, bytes)
        | MagicFiles::Msbp(name, bytes)
        | MagicFiles::Msbt(name, bytes)
        | MagicFiles::Sarc(name, bytes)
        | MagicFiles::Yaz0(name, bytes)
        | MagicFiles::Zstd(name, bytes)
        | MagicFiles::Unknown(name, bytes) => (name, bytes),
    };

    (name.as_deref(), bytes)
}

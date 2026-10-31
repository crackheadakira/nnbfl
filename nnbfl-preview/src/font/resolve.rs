use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use nnbfl::{
    bfcpx::file::{Bfcpx, BfcpxDescription, BfcpxFont},
    bfttf::file::Bfttf,
    core::FileReadWriteable,
};

use super::{
    glyph_atlas::FontFace,
    library::{FontFile, FontIndex, FontKind, list_files, profile_sources},
};
use crate::{
    config::{AppConfig, GameProfile},
    game_config::Language,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FontEnvironment {
    pub profile: Option<GameProfile>,
    pub system_font_dir: Option<PathBuf>,
    pub language: Language,
}

impl FontEnvironment {
    pub fn from_config(config: &AppConfig, profile: Option<&str>) -> Self {
        let profile = match profile {
            Some(name) => {
                let found = config.profile(name).cloned();

                if found.is_none() {
                    log::warn!("No profile named '{name}' in the config");
                }

                found
            }
            None => config.active().cloned(),
        };

        Self {
            language: config.language,
            profile,
            system_font_dir: config.system_font_dir.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MemberSource {
    Found {
        origin: String,
    },

    /// A bitmap font
    BitmapStandIn,

    /// A console font that wasn't supplied
    SystemStandIn,

    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemberMetrics {
    pub width_scale: f32,
    pub height_scale: f32,
    pub baseline_offset: i16,
    pub ascent: u16,
    pub descent: u16,
    pub bearing_x: i16,
    pub char_width: Option<u16>,
    pub exclude_from_height: bool,
}

impl Default for MemberMetrics {
    fn default() -> Self {
        Self {
            width_scale: 1.0,
            height_scale: 1.0,
            baseline_offset: 0,
            ascent: 0,
            descent: 0,
            bearing_x: 0,
            char_width: None,
            exclude_from_height: false,
        }
    }
}

impl From<&BfcpxFont> for MemberMetrics {
    fn from(font: &BfcpxFont) -> Self {
        Self {
            width_scale: font.width_scale,
            height_scale: font.height_scale,
            baseline_offset: font.baseline_offset as i16,
            ascent: font.ascent,
            descent: font.descent,
            bearing_x: font.bearing_x as i16,
            char_width: font
                .flags
                .is_override_char_width
                .then_some(font.override_char_width),
            exclude_from_height: font.flags.is_exclude_from_height_calculation,
        }
    }
}

#[derive(Debug, Clone)]
enum FaceData {
    File(Arc<FontFile>, u32),

    /// The installed fallback
    Placeholder(Option<u32>),

    None,
}

#[derive(Debug, Clone)]
pub struct FontMember {
    pub file: String,
    pub ranges: Vec<(u32, u32)>,
    pub metrics: MemberMetrics,
    pub source: MemberSource,
    data: FaceData,
    face: Option<Option<usize>>,
}

impl FontMember {
    fn covers(&self, code: u32) -> bool {
        self.ranges.is_empty()
            || self
                .ranges
                .iter()
                .any(|&(lo, hi)| (lo..=hi).contains(&code))
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedFont {
    pub name: String,
    pub members: Vec<FontMember>,
}

#[derive(Default)]
pub struct FontCache {
    faces: Vec<FontFace>,
    face_ids: HashMap<(String, u32), usize>,
    fonts: Vec<ResolvedFont>,
    font_ids: HashMap<(u64, String), usize>,
    env: FontEnvironment,
    profile: Vec<Arc<FontIndex>>,
    system: Option<Arc<FontIndex>>,
    indices: HashMap<PathBuf, Arc<FontIndex>>,
    warned: HashSet<String>,
}

const NOTO_CJK: &[&str] = &[
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
];

impl FontCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_environment(&mut self, env: FontEnvironment) -> bool {
        if env == self.env && (env.profile.is_none() || !self.profile.is_empty()) {
            return false;
        }

        self.profile = env
            .profile
            .as_ref()
            .map(|p| profile_sources(p, env.language))
            .unwrap_or_default()
            .into_iter()
            .map(|path| {
                self.indices
                    .entry(path.clone())
                    .or_insert_with(|| {
                        let index = FontIndex::from_paths(std::slice::from_ref(&path));
                        log::info!("Indexed {} fonts in {}", index.len(), path.display());
                        Arc::new(index)
                    })
                    .clone()
            })
            .collect();

        self.system = env
            .system_font_dir
            .as_ref()
            .map(|dir| Arc::new(FontIndex::from_paths(&list_files(dir, false))));

        self.env = env;
        self.fonts.clear();
        self.font_ids.clear();
        self.warned.clear();

        true
    }

    pub fn face(&self, id: usize) -> &FontFace {
        &self.faces[id]
    }

    pub fn font(&self, id: usize) -> &ResolvedFont {
        &self.fonts[id]
    }

    pub fn resolve(&mut self, archive: &FontIndex, name: &str) -> usize {
        let key = (archive.id(), name.to_lowercase());
        if let Some(&id) = self.font_ids.get(&key) {
            return id;
        }

        let mut members = Vec::new();

        match self.lookup(archive, name) {
            Some(file) if file.kind == FontKind::Complex => match Bfcpx::parse_file(&file.bytes) {
                Ok(bfcpx) => self.flatten(archive, &bfcpx.description, &mut members),
                Err(e) => log::warn!("Couldn't read complex font '{name}': {e}"),
            },
            Some(_) => members.push(self.member(archive, name, 0, Vec::new(), Default::default())),
            None if !name.is_empty() => {
                self.warn_once(
                    name,
                    format!("Font '{name}' wasn't found, using the placeholder"),
                );
            }
            None => {}
        }

        if members.is_empty() {
            members.push(FontMember {
                file: name.to_string(),
                ranges: Vec::new(),
                metrics: Default::default(),
                source: MemberSource::Missing,
                data: FaceData::Placeholder(None),
                face: None,
            });
        }

        let id = self.fonts.len();

        self.fonts.push(ResolvedFont {
            name: name.to_string(),
            members,
        });

        self.font_ids.insert(key, id);
        id
    }

    pub fn face_for(&mut self, font: usize, ch: char) -> Option<(usize, MemberMetrics)> {
        let code = ch as u32;
        let mut fallback = None;

        for i in 0..self.fonts[font].members.len() {
            if !self.fonts[font].members[i].covers(code) {
                continue;
            }

            let Some(face) = self.member_face(font, i) else {
                continue;
            };

            let metrics = self.fonts[font].members[i].metrics;

            if self.faces[face].glyph_id_for_char(ch).is_some() {
                return Some((face, metrics));
            }

            fallback.get_or_insert((face, metrics));
        }

        fallback
    }

    pub fn primary_face(&mut self, font: usize) -> Option<usize> {
        let members = &self.fonts[font].members;

        let mut order: Vec<usize> = (0..members.len())
            .filter(|&i| !members[i].metrics.exclude_from_height && members[i].ranges.is_empty())
            .collect();

        order.extend(0..members.len());

        order.into_iter().find_map(|i| self.member_face(font, i))
    }

    fn member_face(&mut self, font: usize, member: usize) -> Option<usize> {
        if let Some(face) = self.fonts[font].members[member].face {
            return face;
        }

        let face = match self.fonts[font].members[member].data.clone() {
            FaceData::File(file, ttc) => self.load_file_face(&file, ttc),
            FaceData::Placeholder(collection) => self.load_placeholder(collection),
            FaceData::None => None,
        };

        self.fonts[font].members[member].face = Some(face);
        face
    }

    fn load_file_face(&mut self, file: &FontFile, ttc: u32) -> Option<usize> {
        let key = (format!("{}:{}", file.origin, file.name), ttc);

        if let Some(&id) = self.face_ids.get(&key) {
            return Some(id);
        }

        let bytes = match file.kind {
            FontKind::Scalable => match Bfttf::parse_file(&file.bytes) {
                Ok(font) => font.font.data,
                Err(e) => {
                    log::warn!("Couldn't decrypt '{}': {e}", file.name);
                    return None;
                }
            },
            FontKind::Plain => file.bytes.to_vec(),
            FontKind::Complex | FontKind::Bitmap => return None,
        };

        let face = FontFace::load(bytes, ttc)?;
        Some(self.push_face(key, face))
    }

    fn load_placeholder(&mut self, collection: Option<u32>) -> Option<usize> {
        if let Some(index) = collection
            && let Some(path) = NOTO_CJK.iter().find(|p| std::path::Path::new(p).is_file())
        {
            let key = (path.to_string(), index);

            if let Some(&id) = self.face_ids.get(&key) {
                return Some(id);
            }

            if let Ok(bytes) = std::fs::read(path)
                && let Some(face) = FontFace::load(bytes, index)
            {
                return Some(self.push_face(key, face));
            }
        }

        let key = ("<placeholder>".to_string(), 0);

        if let Some(&id) = self.face_ids.get(&key) {
            return Some(id);
        }

        let bytes = crate::chinese_font::load_chinese_font()
            .map(|data| (*data.font).as_ref().to_vec())
            .map_err(|e| log::warn!("No placeholder font installed: {e}"))
            .ok()?;

        let face = FontFace::load(bytes, 0)?;
        Some(self.push_face(key, face))
    }

    fn push_face(&mut self, key: (String, u32), face: FontFace) -> usize {
        let id = self.faces.len();
        self.faces.push(face);
        self.face_ids.insert(key, id);

        id
    }

    fn flatten(&mut self, archive: &FontIndex, desc: &BfcpxDescription, out: &mut Vec<FontMember>) {
        match desc {
            BfcpxDescription::Multi(multi) => {
                self.flatten(archive, &multi.child, out);
                self.flatten(archive, &multi.next, out);
            }
            BfcpxDescription::Scalable(scalable) => {
                for font in &scalable.font_array {
                    let ranges = font.char_ranges.iter().map(|r| (r.min, r.max)).collect();

                    out.push(self.member(
                        archive,
                        &font.font_name,
                        font.ttc_font_idx,
                        ranges,
                        font.into(),
                    ));
                }
            }
            BfcpxDescription::Res(res) => {
                let ranges = res.char_ranges.iter().map(|r| (r.min, r.max)).collect();

                out.push(FontMember {
                    file: res.font_name.clone(),
                    ranges,
                    metrics: Default::default(),
                    source: MemberSource::BitmapStandIn,
                    data: FaceData::Placeholder(None),
                    face: None,
                });
            }
        }
    }

    fn member(
        &mut self,
        archive: &FontIndex,
        name: &str,
        ttc: u32,
        ranges: Vec<(u32, u32)>,
        metrics: MemberMetrics,
    ) -> FontMember {
        let (source, data) = match self.lookup(archive, name) {
            Some(file) if file.kind == FontKind::Bitmap => {
                (MemberSource::BitmapStandIn, FaceData::Placeholder(None))
            }
            Some(file) => (
                MemberSource::Found {
                    origin: file.origin.clone(),
                },
                FaceData::File(file, ttc),
            ),
            None => match console_script(name) {
                Some(collection) => {
                    self.warn_once(
                        name,
                        format!("Console font '{name}' isn't in the system font folder, using an installed font"),
                    );
                    (
                        MemberSource::SystemStandIn,
                        FaceData::Placeholder(collection),
                    )
                }
                None => {
                    self.warn_once(name, format!("Font '{name}' wasn't found"));
                    (MemberSource::Missing, FaceData::None)
                }
            },
        };

        FontMember {
            file: name.to_string(),
            ranges,
            metrics,
            source,
            data,
            face: None,
        }
    }

    fn lookup(&self, archive: &FontIndex, name: &str) -> Option<Arc<FontFile>> {
        let localized = name
            .rsplit_once('.')
            .map(|(stem, ext)| format!("{stem}_{}.{ext}", self.env.language.code_str()));

        let sources = std::iter::once(archive)
            .chain(self.profile.iter().map(Arc::as_ref))
            .chain(self.system.as_deref());

        let names: Vec<&str> = localized.as_deref().into_iter().chain([name]).collect();

        sources
            .flat_map(|index| names.iter().filter_map(move |n| index.get(n)))
            .next()
            .cloned()
    }

    fn warn_once(&mut self, name: &str, message: String) {
        if self.warned.insert(name.to_lowercase()) {
            log::warn!("{message}");
        }
    }
}

fn console_script(name: &str) -> Option<Option<u32>> {
    let name = name.to_lowercase();

    if !name.starts_with("nintendo_") {
        return None;
    }

    if name.contains("zh-cn") {
        Some(Some(2))
    } else if name.contains("zh-tw") {
        Some(Some(3))
    } else if name.contains("_ko") {
        Some(Some(1))
    } else if name.starts_with("nintendo_ext") {
        None
    } else {
        Some(Some(0))
    }
}

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
    pub override_bearing: bool,
    pub exclude_from_height: bool,
    pub base_size: u32,
    pub line_feed_delta: i32,

    /// Which scalable descriptor of the bfcpx this member belongs to
    pub group: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextFontMetrics {
    pub width: f32,
    pub height: f32,
    pub ascent: f32,
    pub baseline_pos: f32,
    pub line_feed: f32,
}

impl TextFontMetrics {
    pub fn from_bffnt(bytes: &[u8]) -> Option<Self> {
        let big_endian = bytes.get(4..6)? == [0xFE, 0xFF];
        let u16_at = |at: usize| {
            let raw = [*bytes.get(at)?, *bytes.get(at + 1)?];

            Some(if big_endian {
                u16::from_be_bytes(raw)
            } else {
                u16::from_le_bytes(raw)
            })
        };

        let u32_at = |at: usize| {
            let raw: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;

            Some(if big_endian {
                u32::from_be_bytes(raw)
            } else {
                u32::from_le_bytes(raw)
            })
        };

        let section = |magic: &[u8; 4]| {
            let mut at = usize::from(u16_at(6)?);

            while let Some(size) = u32_at(at + 4) {
                if bytes.get(at..at + 4)? == magic {
                    return Some(at);
                }

                at = at.checked_add(usize::try_from(size).ok().filter(|&s| s > 0)?)?;
            }

            None
        };

        let finf = section(b"FINF")?;
        let tglp = section(b"TGLP")?;

        Some(Self {
            height: f32::from(*bytes.get(finf + 9)?),
            width: f32::from(*bytes.get(finf + 10)?),
            ascent: f32::from(*bytes.get(finf + 11)?),
            line_feed: f32::from(u16_at(finf + 12)?),
            baseline_pos: f32::from(u16_at(tglp + 16)?),
        })
    }

    fn pair(self, other: Self) -> Self {
        let first = self.height >= other.height;
        let taller = if first { self } else { other };

        Self {
            width: self.width.max(other.width),
            height: self.height.max(other.height),
            ascent: taller.ascent,
            baseline_pos: taller.baseline_pos,
            line_feed: self.line_feed.max(other.line_feed),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct FaceRatios {
    ascent: f32,
    height: f32,
    bbox_ascent: f32,
    bbox_height: f32,
}

impl FaceRatios {
    fn of(face: &FontFace, metrics: &MemberMetrics) -> Self {
        let em = face.units_per_em.max(1.0);
        let (mut ascent, mut descent) = (face.win_ascent, face.win_descent);
        let (mut bbox_max, mut bbox_min) = (face.bbox_y_max, face.bbox_y_min);

        if metrics.ascent != 0 {
            ascent = f32::from(metrics.ascent);
            bbox_max = ascent;
        }

        if metrics.descent != 0 {
            descent = f32::from(metrics.descent);
            bbox_min = -descent;
        }

        Self {
            ascent: ascent / em,
            height: (ascent + descent) / em,
            bbox_ascent: bbox_max / em,
            bbox_height: (bbox_max - bbox_min) / em,
        }
    }

    fn at(self, size: f32, line_feed_delta: i32) -> TextFontMetrics {
        let round = |v: f32| (v + 0.5).floor();
        let height = round(self.height * size);

        TextFontMetrics {
            width: size,
            height,
            ascent: round(self.ascent * size),
            baseline_pos: round(self.bbox_ascent * size),
            line_feed: height + line_feed_delta as f32,
        }
    }
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
            override_bearing: false,
            exclude_from_height: false,
            base_size: 0,
            line_feed_delta: 0,
            group: 0,
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
            override_bearing: font.flags.is_override_bearing_x,
            exclude_from_height: font.flags.is_exclude_from_height_calculation,
            ..Default::default()
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
    pub bitmap: Option<TextFontMetrics>,
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
                Ok(bfcpx) => self.flatten(archive, &bfcpx.description, &mut members, &mut 0),
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
                bitmap: None,
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

    pub fn text_metrics(&mut self, font: usize, fallback_size: f32) -> Option<TextFontMetrics> {
        let groups: Vec<u16> = {
            let mut groups: Vec<u16> = self.fonts[font]
                .members
                .iter()
                .filter(|m| m.metrics.base_size > 0 || m.bitmap.is_some())
                .map(|m| m.metrics.group)
                .collect();

            groups.dedup();
            groups
        };

        if groups.is_empty() {
            let face = self.primary_face(font)?;
            let ratios = FaceRatios::of(&self.faces[face], &MemberMetrics::default());
            return Some(ratios.at((fallback_size + 0.5).floor().max(1.0), 0));
        }

        groups
            .into_iter()
            .filter_map(|group| self.group_metrics(font, group))
            .reduce(TextFontMetrics::pair)
    }

    fn group_metrics(&mut self, font: usize, group: u16) -> Option<TextFontMetrics> {
        if let Some(bitmap) = self.fonts[font]
            .members
            .iter()
            .find(|m| m.metrics.group == group && m.bitmap.is_some())
            .and_then(|m| m.bitmap)
        {
            return Some(bitmap);
        }

        let (ratios, metrics) = self.group_ratios(font, group)?;
        Some(ratios.at(metrics.base_size as f32, metrics.line_feed_delta))
    }

    pub fn cell_ascent(
        &mut self,
        font: usize,
        member: MemberMetrics,
        face: usize,
        size: f32,
    ) -> f32 {
        let ratio = self.fonts[font]
            .members
            .iter()
            .any(|m| {
                m.metrics.group == member.group && m.bitmap.is_none() && m.metrics.base_size > 0
            })
            .then(|| self.group_ratios(font, member.group))
            .flatten()
            .map_or_else(
                || FaceRatios::of(&self.faces[face], &member).bbox_ascent,
                |(ratios, _)| ratios.bbox_ascent,
            );

        (ratio * size + 0.5).floor()
    }

    fn group_ratios(&mut self, font: usize, group: u16) -> Option<(FaceRatios, MemberMetrics)> {
        let members: Vec<usize> = (0..self.fonts[font].members.len())
            .filter(|&i| self.fonts[font].members[i].metrics.group == group)
            .filter(|&i| self.fonts[font].members[i].metrics.base_size > 0)
            .filter(|&i| self.fonts[font].members[i].bitmap.is_none())
            .collect();

        let first = *members.first()?;
        let metrics = self.fonts[font].members[first].metrics;
        let mut tallest: Option<FaceRatios> = None;
        let mut any = None;

        for i in members {
            let Some(face) = self.member_face(font, i) else {
                continue;
            };

            let member = self.fonts[font].members[i].metrics;
            let ratios = FaceRatios::of(&self.faces[face], &member);
            any.get_or_insert(ratios);

            if !member.exclude_from_height
                && tallest.is_none_or(|t| ratios.bbox_height > t.bbox_height)
            {
                tallest = Some(ratios);
            }
        }

        Some((tallest.or(any)?, metrics))
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

    fn flatten(
        &mut self,
        archive: &FontIndex,
        desc: &BfcpxDescription,
        out: &mut Vec<FontMember>,
        group: &mut u16,
    ) {
        match desc {
            BfcpxDescription::Multi(multi) => {
                self.flatten(archive, &multi.child, out, group);
                self.flatten(archive, &multi.next, out, group);
            }
            BfcpxDescription::Scalable(scalable) => {
                for font in &scalable.font_array {
                    let ranges = font.char_ranges.iter().map(|r| (r.min, r.max)).collect();
                    let metrics = MemberMetrics {
                        base_size: (scalable.width + 0.5) as u32,
                        line_feed_delta: scalable.line_feed_delta as i32,
                        group: *group,
                        ..font.into()
                    };

                    out.push(self.member(
                        archive,
                        &font.font_name,
                        font.ttc_font_idx,
                        ranges,
                        metrics,
                    ));
                }

                *group += 1;
            }
            BfcpxDescription::Res(res) => {
                let ranges = res.char_ranges.iter().map(|r| (r.min, r.max)).collect();
                let bitmap = self
                    .lookup(archive, &res.font_name)
                    .and_then(|file| TextFontMetrics::from_bffnt(&file.bytes));

                out.push(FontMember {
                    file: res.font_name.clone(),
                    ranges,
                    metrics: MemberMetrics {
                        group: *group,
                        base_size: bitmap.map_or(0, |b| b.height as u32),
                        ..Default::default()
                    },
                    bitmap,
                    source: MemberSource::BitmapStandIn,
                    data: FaceData::Placeholder(None),
                    face: None,
                });

                *group += 1;
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
        let file = self.lookup(archive, name);
        let bitmap = file
            .as_ref()
            .filter(|file| file.kind == FontKind::Bitmap)
            .and_then(|file| TextFontMetrics::from_bffnt(&file.bytes));

        let metrics = MemberMetrics {
            base_size: bitmap.map_or(metrics.base_size, |b| b.height as u32),
            ..metrics
        };

        let (source, data) = match file {
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
            bitmap,
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

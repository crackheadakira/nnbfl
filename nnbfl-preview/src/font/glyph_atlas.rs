use std::collections::HashMap;

use ttf_parser::{Face, GlyphId};

use super::resolve::MemberMetrics;
use crate::renderer::texture::TextureCache;

pub const GLYPH_ATLAS_TEXTURE_NAME: &str = "__glyph_atlas";
const PADDING: u32 = 1;

#[derive(Debug, Clone, Copy, Default)]
pub struct AtlasGlyph {
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],

    /// Bitmap size in pixels, at the size it was rasterized.
    pub width: f32,
    pub height: f32,

    /// Offset from the pen position to the bitmap's left/bottom edge.
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub advance: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphCell {
    pub bitmap: AtlasGlyph,
    pub width: f32,
    pub height: f32,
    pub bitmap_row: f32,
    pub rows: f32,
    pub advance: f32,
    pub left: f32,

    /// How far the cell top is above the baseline
    pub top: f32,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
struct GlyphKey {
    font_id: usize,
    glyph_index: u16,
    size_bits: u32,
}

#[derive(Default)]
pub struct GlyphAtlas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    glyphs: HashMap<GlyphKey, AtlasGlyph>,
    cursor_x: u32,
    cursor_y: u32,
    shelf_height: u32,
    dirty: bool,

    max_used_x: u32,
    max_used_y: u32,
}

impl GlyphAtlas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height) as usize],
            glyphs: HashMap::new(),
            cursor_x: PADDING,
            cursor_y: PADDING,
            shelf_height: 0,
            dirty: true,
            max_used_x: 0,
            max_used_y: 0,
        }
    }

    pub fn used_bounds(&self) -> (u32, u32) {
        (self.max_used_x, self.max_used_y)
    }

    pub fn utilization_percentage(&self) -> f32 {
        let total_pixels = (self.width * self.height) as f32;
        let used_pixels = (self.max_used_x * self.max_used_y) as f32;

        if total_pixels == 0.0 {
            0.0
        } else {
            (used_pixels / total_pixels) * 100.0
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn glyph(
        &mut self,
        font: &FontFace,
        font_id: usize,
        glyph_index: GlyphId,
        size_px: f32,
    ) -> Option<AtlasGlyph> {
        puffin::profile_function!();
        let key = GlyphKey {
            font_id,
            glyph_index: glyph_index.0,
            size_bits: size_px.to_bits(),
        };

        if let Some(existing) = self.glyphs.get(&key) {
            return Some(*existing);
        }

        let (metrics, bitmap) = font.rasterize_glyph(glyph_index, size_px);

        let entry = if metrics.width == 0 || metrics.height == 0 {
            AtlasGlyph {
                bearing_x: metrics.xmin as f32,
                bearing_y: metrics.ymin as f32,
                advance: metrics.advance_width,
                ..Default::default()
            }
        } else {
            let (x, y) = self.allocate(metrics.width as u32, metrics.height as u32)?;
            self.blit(&bitmap, metrics.width as u32, metrics.height as u32, x, y);

            AtlasGlyph {
                uv_min: [x as f32 / self.width as f32, y as f32 / self.height as f32],
                uv_max: [
                    (x + metrics.width as u32) as f32 / self.width as f32,
                    (y + metrics.height as u32) as f32 / self.height as f32,
                ],
                width: metrics.width as f32,
                height: metrics.height as f32,
                bearing_x: metrics.xmin as f32,
                bearing_y: metrics.ymin as f32,
                advance: metrics.advance_width,
            }
        };

        self.glyphs.insert(key, entry);
        Some(entry)
    }

    pub fn cell(
        &mut self,
        font: &FontFace,
        font_id: usize,
        glyph_index: GlyphId,
        size: f32,
        ascent: f32,
        member: &MemberMetrics,
    ) -> Option<GlyphCell> {
        let bitmap = self.glyph(font, font_id, glyph_index, size)?;
        let (w, h) = (bitmap.width, bitmap.height);
        let (scale_w, scale_h) = (member.width_scale, member.height_scale);
        let bearing = f32::from(member.bearing_x);

        let offset_from_top = ascent - (bitmap.bearing_y + h);
        let cached_height = h + offset_from_top;

        let (advance, left) = if member.override_bearing && w != 0.0 {
            (
                ((w + bearing) * scale_w).trunc(),
                ((bearing / 2.0).trunc() * scale_w).trunc(),
            )
        } else if let Some(char_width) = member.char_width.map(f32::from) {
            (
                (char_width * scale_w).trunc(),
                (((char_width - w) / 2.0).trunc() * scale_w).trunc(),
            )
        } else {
            (
                ((bitmap.advance.round() + bearing) * scale_w).trunc(),
                ((bitmap.bearing_x + (bearing / 2.0).trunc()) * scale_w).trunc(),
            )
        };

        Some(GlyphCell {
            bitmap,
            width: (w * scale_w).trunc(),
            height: (cached_height * scale_h).trunc() + 1.0,
            bitmap_row: offset_from_top,
            rows: cached_height + 1.0,
            advance,
            left,
            top: (ascent * scale_h + f32::from(member.baseline_offset)).trunc(),
        })
    }

    fn allocate(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        puffin::profile_function!();
        if self.cursor_x + w + PADDING > self.width {
            self.cursor_x = PADDING;
            self.cursor_y += self.shelf_height + PADDING;
            self.shelf_height = 0;
        }

        if self.cursor_y + h + PADDING > self.height {
            log::warn!(
                "GlyphAtlas: out of space ({}x{}) placing a {w}x{h} glyph - dropping it",
                self.width,
                self.height
            );
            return None;
        }

        let pos = (self.cursor_x, self.cursor_y);
        self.cursor_x += w + PADDING;
        self.shelf_height = self.shelf_height.max(h);

        self.max_used_x = self.max_used_x.max(pos.0 + w);
        self.max_used_y = self.max_used_y.max(pos.1 + h);

        Some(pos)
    }

    fn blit(&mut self, bitmap: &[u8], w: u32, h: u32, x: u32, y: u32) {
        puffin::profile_function!();
        for row in 0..h {
            let src_start = (row * w) as usize;
            let dst_start = ((y + row) * self.width + x) as usize;

            self.pixels[dst_start..dst_start + w as usize]
                .copy_from_slice(&bitmap[src_start..src_start + w as usize]);
        }

        self.dirty = true;
    }

    /// Uploads again on the next [`Self::upload`], for a cache that lost the atlas
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, cache: &mut TextureCache) {
        puffin::profile_function!();

        if !self.dirty {
            return;
        }

        let rgba: Vec<u8> = self
            .pixels
            .iter()
            .flat_map(|&coverage| [255, 255, 255, coverage])
            .collect();

        cache.insert_rgba(
            device,
            queue,
            GLYPH_ATLAS_TEXTURE_NAME,
            &rgba,
            self.width,
            self.height,
        );

        self.dirty = false;
    }
}

pub struct FontFace {
    face_index: u32,
    fontdue_font: fontdue::Font,
    pub units_per_em: f32,
    pub ascender: f32,
    pub descender: f32,
    pub line_gap: f32,

    pub win_ascent: f32,
    pub win_descent: f32,
    pub bbox_y_max: f32,
    pub bbox_y_min: f32,
}

impl FontFace {
    pub fn load(data: Vec<u8>, face_index: u32) -> Option<Self> {
        puffin::profile_function!();

        let face = Face::parse(&data, face_index).ok()?;
        let units_per_em = face.units_per_em() as f32;
        let ascender = face.ascender() as f32;
        let descender = face.descender() as f32;
        let line_gap = face.line_gap() as f32;

        let (win_ascent, win_descent) = face.tables().os2.map_or((ascender, -descender), |os2| {
            (
                f32::from(os2.windows_ascender() as u16),
                (-i32::from(os2.windows_descender())) as u16 as f32,
            )
        });

        let bbox = face.global_bounding_box();

        let fontdue_font = {
            puffin::profile_scope!("fontdue::Font::from_bytes");
            fontdue::Font::from_bytes(
                data,
                fontdue::FontSettings {
                    collection_index: face_index,
                    load_substitutions: false,
                    ..Default::default()
                },
            )
            .map_err(|e| log::error!("fontdue failed to load font: {e}"))
            .ok()?
        };

        Some(Self {
            face_index,
            fontdue_font,
            units_per_em,
            ascender,
            descender,
            line_gap,
            win_ascent,
            win_descent,
            bbox_y_max: f32::from(bbox.y_max),
            bbox_y_min: f32::from(bbox.y_min),
        })
    }

    pub fn rasterize_glyph(&self, glyph_id: GlyphId, size_px: f32) -> (fontdue::Metrics, Vec<u8>) {
        puffin::profile_function!();
        self.fontdue_font.rasterize_indexed(glyph_id.0, size_px)
    }

    pub fn glyph_id_for_char(&self, ch: char) -> Option<GlyphId> {
        let idx = self.fontdue_font.lookup_glyph_index(ch);

        if idx == 0 { None } else { Some(GlyphId(idx)) }
    }

    pub fn scale_factor(&self, size_px: f32) -> f32 {
        size_px / self.units_per_em
    }

    pub fn kerning(&self, left: GlyphId, right: GlyphId, size_px: f32) -> f32 {
        puffin::profile_function!();

        self.fontdue_font
            .horizontal_kern_indexed(left.0, right.0, size_px)
            .unwrap_or(0.0)
    }
}

pub struct GlyphData {
    pub fonts: super::resolve::FontCache,
    pub atlas: GlyphAtlas,
}

impl Default for GlyphData {
    fn default() -> Self {
        Self {
            fonts: super::resolve::FontCache::new(),
            atlas: GlyphAtlas::new(1024, 1024),
        }
    }
}

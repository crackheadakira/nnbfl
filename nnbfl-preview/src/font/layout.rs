use nnbfl::bflyt::pane::{HorizontalPosition, TextAlignment, TextPosition, VerticalPosition};
use ttf_parser::GlyphId;

use super::{
    glyph_atlas::{GlyphCell, GlyphData},
    resolve::{FontCache, MemberMetrics},
};

pub struct PlacedGlyph {
    pub face: usize,
    pub glyph: GlyphId,
    pub member: MemberMetrics,
    pub size: f32,
    pub cell: GlyphCell,
}

pub fn place_glyph(
    glyphs: &mut GlyphData,
    font: usize,
    ch: char,
    fallback_size: f32,
) -> Option<PlacedGlyph> {
    let (face_id, member) = glyphs.fonts.face_for(font, ch)?;
    let glyph = glyphs.fonts.face(face_id).glyph_id_for_char(ch)?;

    let size = match member.base_size {
        0 => (fallback_size + 0.5).floor().max(1.0),
        size => size as f32,
    };

    let ascent = glyphs.fonts.cell_ascent(font, member, face_id, size);
    let face = glyphs.fonts.face(face_id);
    let cell = glyphs
        .atlas
        .cell(face, face_id, glyph, size, ascent, &member)?;

    Some(PlacedGlyph {
        face: face_id,
        glyph,
        member,
        size,
        cell,
    })
}

pub struct TextLayout {
    pub scale_x: f32,
    pub character_space: f32,
    pub line_height: f32,

    /// The font's height and ascent (already scaled)
    pub font_height: f32,

    pub ascent: f32,
    pub position: TextPosition,
    pub alignment: TextAlignment,
    pub center_ceiling: bool,

    /// The text box top left corner and size, lines wrap at its width
    pub box_left: f32,

    pub box_top: f32,
    pub box_width: f32,
    pub box_height: f32,
}

pub struct LaidGlyph<'a> {
    pub glyph: &'a PlacedGlyph,
    pub x: f32,
    pub baseline: f32,
}

impl TextLayout {
    pub fn lay_out<'a>(
        &self,
        fonts: &FontCache,
        paragraphs: &'a [Vec<Option<PlacedGlyph>>],
    ) -> Vec<LaidGlyph<'a>> {
        let lines: Vec<&[Option<PlacedGlyph>]> = paragraphs
            .iter()
            .flat_map(|paragraph| self.wrap(fonts, paragraph))
            .collect();

        let widths: Vec<f32> = lines.iter().map(|line| self.width(fonts, line)).collect();
        let text_width = widths.iter().copied().fold(0.0, f32::max);

        let text_height =
            self.line_height * lines.len().saturating_sub(1) as f32 + self.font_height;

        let half = |v: f32| {
            let half = v / 2.0;

            if self.center_ceiling {
                half.ceil()
            } else {
                half
            }
        };

        let left = self.box_left
            + match self.position.horizontal {
                HorizontalPosition::Left => 0.0,
                HorizontalPosition::Center => self.box_width / 2.0 - half(text_width),
                HorizontalPosition::Right => self.box_width - text_width,
            };

        let top = self.box_top
            + match self.position.vertical {
                VerticalPosition::Top => 0.0,
                VerticalPosition::Center => self.box_height / 2.0 - half(text_height),
                VerticalPosition::Bottom => self.box_height - text_height,
            };

        let alignment = match self.alignment {
            TextAlignment::Synchronous => match self.position.horizontal {
                HorizontalPosition::Left => TextAlignment::Left,
                HorizontalPosition::Center => TextAlignment::Center,
                HorizontalPosition::Right => TextAlignment::Right,
            },
            alignment => alignment,
        };

        let mut laid = Vec::new();

        for (index, (line, width)) in lines.into_iter().zip(widths).enumerate() {
            let mut x = left
                + match alignment {
                    TextAlignment::Center => half(text_width) - half(width),
                    TextAlignment::Right => text_width - width,
                    TextAlignment::Left | TextAlignment::Synchronous => 0.0,
                };

            let baseline = top + index as f32 * self.line_height + self.ascent;
            let mut prev: Option<&PlacedGlyph> = None;

            for placed in line {
                let Some(placed) = placed else {
                    prev = None;
                    continue;
                };

                if let Some(prev) = prev {
                    x += self.character_space + kerning(fonts, prev, placed) * self.scale_x;
                }

                laid.push(LaidGlyph {
                    glyph: placed,
                    x,
                    baseline,
                });

                x += placed.cell.advance * self.scale_x;
                prev = Some(placed);
            }
        }

        laid
    }

    fn wrap<'a>(
        &self,
        fonts: &FontCache,
        paragraph: &'a [Option<PlacedGlyph>],
    ) -> Vec<&'a [Option<PlacedGlyph>]> {
        let mut lines = Vec::new();
        let mut start = 0;
        let mut width = 0.0;
        let mut prev: Option<&PlacedGlyph> = None;
        let mut has_glyph = false;

        for (index, placed) in paragraph.iter().enumerate() {
            let Some(placed) = placed else {
                prev = None;
                continue;
            };

            let mut next = width + placed.cell.advance * self.scale_x;
            if let Some(prev) = prev {
                next += self.character_space + kerning(fonts, prev, placed) * self.scale_x;
            }

            if has_glyph && next > self.box_width {
                lines.push(&paragraph[start..index]);
                start = index;
                next = placed.cell.advance * self.scale_x;
            }

            width = next;
            prev = Some(placed);
            has_glyph = true;
        }

        lines.push(&paragraph[start..]);
        lines
    }

    fn width(&self, fonts: &FontCache, line: &[Option<PlacedGlyph>]) -> f32 {
        let mut width = 0.0;
        let mut prev: Option<&PlacedGlyph> = None;

        for placed in line {
            let Some(placed) = placed else {
                prev = None;
                continue;
            };

            if let Some(prev) = prev {
                width += self.character_space + kerning(fonts, prev, placed) * self.scale_x;
            }

            width += placed.cell.advance * self.scale_x;
            prev = Some(placed);
        }

        width.max(0.0)
    }
}

fn kerning(fonts: &FontCache, prev: &PlacedGlyph, next: &PlacedGlyph) -> f32 {
    let fixed = |m: &MemberMetrics| m.override_bearing || m.char_width.is_some();

    if prev.face != next.face || fixed(&prev.member) || fixed(&next.member) {
        return 0.0;
    }

    (fonts
        .face(next.face)
        .kerning(prev.glyph, next.glyph, next.size)
        * next.member.width_scale)
        .trunc()
}

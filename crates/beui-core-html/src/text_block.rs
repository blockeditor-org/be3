use std::ops::Range;
use std::rc::Rc;

use beui_types::text::{FontId, LineBox, TextAlign, TextLayout};
use beui_types::{Rect, Vec2, pos2, vec2};

#[cfg(not(target_arch = "wasm32"))]
mod faces;
#[cfg(not(target_arch = "wasm32"))]
use faces::{covers, extents, width};
#[cfg(target_arch = "wasm32")]
mod canvas;
#[cfg(target_arch = "wasm32")]
use canvas::{covers, extents, width};

const WRAP_SLACK: f32 = 1.0 / 1024.0;

#[derive(Clone, PartialEq, Debug)]
pub struct TextBlock(Rc<Block>);

#[derive(PartialEq, Debug)]
struct Block {
    text: String,
    font: FontId,
    size: Vec2,
    line_height: f32,
    baseline: f32,
    lines: Vec<Line>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Line {
    pub bytes: Range<usize>,
    pub shown: Range<usize>,
    pub left: f32,
    pub width: f32,
    pub top: f32,
}

impl TextBlock {
    pub fn empty(text: &str, font: FontId, layout: TextLayout) -> Self {
        let line = line_height_of(font, layout);
        Self(Rc::new(Block {
            text: text.to_owned(),
            font,
            size: vec2(0.0, line),
            line_height: line,
            baseline: line,
            lines: Vec::new(),
        }))
    }

    pub fn text(&self) -> &str {
        &self.0.text
    }

    pub fn font(&self) -> FontId {
        self.0.font
    }

    pub fn size(&self) -> Vec2 {
        self.0.size
    }

    pub fn line_height(&self) -> f32 {
        self.0.line_height
    }

    pub fn baseline(&self) -> f32 {
        self.0.baseline
    }

    pub fn lines(&self) -> &[Line] {
        &self.0.lines
    }

    pub fn line_boxes(&self) -> Vec<LineBox> {
        self.0
            .lines
            .iter()
            .map(|line| LineBox {
                bytes: line.bytes.clone(),
                rect: Rect::from_min_size(
                    pos2(line.left, line.top),
                    vec2(line.width, self.0.line_height),
                ),
                baseline: line.top + self.0.baseline,
            })
            .collect()
    }

    pub fn has_every_glyph(&self) -> bool {
        covers(&self.0.text, self.0.font)
    }
}

pub fn line_height_of(font: FontId, layout: TextLayout) -> f32 {
    let line = layout
        .line_height
        .unwrap_or_else(|| crate::text::line_height(font.size))
        .round()
        .max(1.0);
    (line * layout.line_spacing.max(0.1)).round().max(1.0)
}

pub fn lay_out_text(text: &str, font: FontId, layout: TextLayout) -> Option<TextBlock> {
    let (ascent, descent) = extents(font)?;
    let line_height = line_height_of(font, layout);
    let baseline = ((line_height - (ascent + descent)) / 2.0).round() + ascent;
    let wrap = layout.wrap_width + WRAP_SLACK;
    let mut lines: Vec<Line> = Vec::new();
    let mut start = 0;
    loop {
        let end = text[start..].find('\n').map_or(text.len(), |at| start + at);
        let first = lines.len();
        let mut from = start;
        loop {
            let (to, width) = match width(text[from..end].trim_end(), font)? {
                whole if whole <= wrap => (end, whole),
                _ => broken(text, from..end, font, wrap)?,
            };
            let shown = from..from + text[from..to].trim_end().len();
            lines.push(Line {
                bytes: from..to,
                shown,
                left: 0.0,
                width,
                top: lines.len() as f32 * line_height,
            });
            from = to;
            if from >= end {
                break;
            }
        }
        if end == text.len() {
            break;
        }
        if let Some(last) = lines[first..].last_mut() {
            last.bytes.end = end + 1;
        }
        start = end + 1;
    }
    let widest = lines.iter().map(|line| line.width).fold(0.0, f32::max);
    for line in &mut lines {
        line.left = match layout.align {
            TextAlign::Start => 0.0,
            TextAlign::Center => ((widest - line.width) / 2.0).round(),
            TextAlign::End => (widest - line.width).round(),
        };
    }
    Some(TextBlock(Rc::new(Block {
        text: text.to_owned(),
        font,
        size: vec2(widest.ceil(), lines.len() as f32 * line_height),
        line_height,
        baseline,
        lines,
    })))
}

fn broken(text: &str, range: Range<usize>, font: FontId, wrap: f32) -> Option<(usize, f32)> {
    let mut chosen = None;
    for at in good_breaks(text, range.clone()) {
        let width = width(text[range.start..at].trim_end(), font)?;
        if width > wrap {
            break;
        }
        chosen = Some((at, width));
    }
    if chosen.is_some() {
        return chosen;
    }
    for (at, ch) in text[range.clone()].char_indices() {
        let to = range.start + at + ch.len_utf8();
        let width = width(&text[range.start..to], font)?;
        if width > wrap && chosen.is_some() {
            break;
        }
        chosen = Some((to, width));
    }
    chosen
}

fn good_breaks(text: &str, range: Range<usize>) -> impl Iterator<Item = usize> + '_ {
    text[range.clone()]
        .char_indices()
        .filter(|(_, ch)| ch.is_whitespace() || matches!(ch, '-' | '/' | '\\'))
        .map(move |(at, ch)| range.start + at + ch.len_utf8())
        .filter(move |at| *at < range.end)
}

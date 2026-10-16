use std::collections::HashMap;
use std::ops::Range;

use text_editor_core::{
    MarkdownTableAlignment, SynHlColorScope, SynHlFontFamily, SynHlStyle, SynHlTextSize, TextRun,
};

use beui_core::base::text::DEFAULT_FONT_SIZE;
use beui_core::color::Color32;
use beui_core::font::{FontId, Galley, TextAlign, TextLayout};
use beui_core::geometry::Vec2;
use beui_core::rich::{Piece, RichLayout, RichOptions, SpanStyle};
use beui_view::reactive::*;

use super::colors::TextAreaColors;
use super::state::Snapshot;

pub const BODY_SIZE: f32 = DEFAULT_FONT_SIZE;
pub const CODE_SIZE: f32 = BODY_SIZE;
pub const CHECKBOX_WIDTH: f32 = 18.0;
pub const INLINE_WIDGET_HEIGHT: f32 = 24.0;
pub const INLINE_WIDGET_ICON_INSET: f32 = 13.0;
pub const RUN_CHIP_HEIGHT: f32 = 16.0;
pub const RUN_CHIP_PADDING: f32 = 4.0;
pub const RUN_CHIP_SIZE: f32 = 10.0;
pub const DOCUMENT_PADDING: Vec2 = Vec2::new(24.0, 16.0);
pub const LINE_PADDING: (f32, f32) = (3.0, 4.0);

const MASK: &str = "*";
const OBJECT: &str = "\u{fffc}";
const NEWLINE_MARKER: &str = "\u{23ce}";
const ELLIPSIS: &str = "...";
const ELLIPSIS_GAP: f32 = 6.0;
const DELETED_PREVIEW: usize = 48;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextWidget {
    pub range: Range<usize>,
    pub label: String,
    pub icon: Option<&'static str>,
    pub italic: bool,
    pub broken: bool,
    pub block_size: Option<Vec2>,
}

impl TextWidget {
    pub fn block(&self) -> bool {
        self.block_size.is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Composition {
    pub range: Range<usize>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Prose {
    pub bold: bool,
    pub align: TextAlign,
    pub vertical_align: TextAlign,
    pub line_padding: (f32, f32),
    pub wrap: bool,
}

impl Default for Prose {
    fn default() -> Self {
        Self {
            bold: false,
            align: TextAlign::Start,
            vertical_align: TextAlign::Start,
            line_padding: (0.0, 0.0),
            wrap: true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RowOptions {
    pub body_size: f32,
    pub mask: bool,
    pub single_line: bool,
    pub prose: Option<Prose>,
}

impl RowOptions {
    pub fn padding(&self) -> (f32, f32) {
        match (self.single_line, self.prose) {
            (true, _) => (0.0, 0.0),
            (false, Some(prose)) => prose.line_padding,
            (false, None) => LINE_PADDING,
        }
    }

    fn bold(&self) -> bool {
        self.prose.is_some_and(|prose| prose.bold)
    }

    fn invisibles(&self) -> bool {
        !self.single_line && self.prose.is_none()
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Inline {
    Checkbox { line_start: usize, checked: bool },
    Widget(usize),
    Run,
}

#[derive(Clone, PartialEq, Debug)]
pub struct InlineItem {
    pub inline: Inline,
    pub size: Vec2,
    pub style: SpanStyle,
    pub label: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Segment {
    pub source: Range<usize>,
    pub display: Range<usize>,
    pub mapped: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SpanKind {
    Text,
    Space(f32),
    Inline(usize),
}

#[derive(Clone, PartialEq, Debug)]
pub struct RowSpan {
    pub range: Range<usize>,
    pub style: SpanStyle,
    pub kind: SpanKind,
    pub break_after: bool,
    pub invisible: Option<usize>,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Row {
    pub line: usize,
    pub start: usize,
    pub end: usize,
    pub display: String,
    pub spans: Vec<RowSpan>,
    pub segments: Vec<Segment>,
    pub inline: Vec<InlineItem>,
    pub line_height: f32,
    pub block: Option<(usize, Vec2)>,
    pub code: Vec<Range<usize>>,
    pub code_block: bool,
    pub placeholder: bool,
}

impl Row {
    pub fn moved(&self, line: usize, start: usize) -> Row {
        let shift = |at: usize| at + start - self.start;
        Row {
            line,
            start,
            end: shift(self.end),
            segments: self
                .segments
                .iter()
                .map(|segment| Segment {
                    source: shift(segment.source.start)..shift(segment.source.end),
                    ..segment.clone()
                })
                .collect(),
            inline: self
                .inline
                .iter()
                .map(|item| match item.inline {
                    Inline::Checkbox {
                        line_start,
                        checked,
                    } => InlineItem {
                        inline: Inline::Checkbox {
                            line_start: shift(line_start),
                            checked,
                        },
                        ..item.clone()
                    },
                    _ => item.clone(),
                })
                .collect(),
            ..self.clone()
        }
    }

    pub fn to_display(&self, byte: usize) -> usize {
        if self.placeholder {
            return 0;
        }
        for segment in &self.segments {
            if segment.source.is_empty() {
                continue;
            }
            if segment.source.contains(&byte) {
                return match segment.mapped {
                    true => segment.display.start + (byte - segment.source.start),
                    false => segment.display.start,
                };
            }
            if segment.source.start > byte {
                return segment.display.start;
            }
        }
        self.display.len()
    }

    pub fn to_source(&self, display: usize) -> usize {
        let source = match self
            .segments
            .iter()
            .find(|segment| segment.display.contains(&display))
        {
            Some(segment) if segment.mapped => {
                segment.source.start + (display - segment.display.start)
            }
            Some(segment) => {
                let into = display - segment.display.start;
                match into * 2 < segment.display.len() {
                    true => segment.source.start,
                    false => segment.source.end,
                }
            }
            None => self
                .segments
                .iter()
                .rev()
                .find(|segment| segment.display.end <= display)
                .map_or(self.start, |segment| segment.source.end),
        };
        source.clamp(self.start, self.end)
    }

    pub fn display_range(&self, range: &Range<usize>) -> Range<usize> {
        let start = self.to_display(range.start.max(self.start));
        let end = match range.end > self.end {
            true => self.newline_display_end(),
            false => self.to_display(range.end),
        };
        start..end.max(start)
    }

    fn newline_display_end(&self) -> usize {
        self.segments
            .iter()
            .find(|segment| segment.source.start == self.end && segment.source.len() == 1)
            .map_or(self.display_end_of_source(), |segment| segment.display.end)
    }

    fn display_end_of_source(&self) -> usize {
        self.segments
            .iter()
            .rev()
            .find(|segment| !segment.source.is_empty())
            .map_or(0, |segment| segment.display.end)
    }

    pub fn pieces(&self) -> Vec<Piece<'_>> {
        let mut flow = 0;
        self.spans
            .iter()
            .map(|span| {
                let size = match span.kind {
                    SpanKind::Text => {
                        return Piece::Text {
                            text: self.display.get(span.range.clone()).unwrap_or_default(),
                            style: span.style,
                            break_after: span.break_after,
                        };
                    }
                    SpanKind::Space(width) => return Piece::Gap(width),
                    SpanKind::Inline(index) => {
                        self.inline.get(index).map_or(Vec2::ZERO, |item| item.size)
                    }
                };
                flow += 1;
                Piece::Inline {
                    index: flow - 1,
                    size,
                }
            })
            .collect()
    }

    pub fn flow_inline(&self, flow: usize) -> Option<&InlineItem> {
        let index = self
            .spans
            .iter()
            .filter(|span| matches!(span.kind, SpanKind::Inline(_)))
            .nth(flow)
            .and_then(|span| match span.kind {
                SpanKind::Inline(index) => Some(index),
                _ => None,
            })?;
        self.inline.get(index)
    }
}

pub struct RowInputs<'a> {
    pub snapshot: &'a Snapshot,
    pub widgets: &'a [TextWidget],
    pub client_colors: &'a [(u64, Color32)],
    pub composition: Option<&'a Composition>,
    pub colors: &'a TextAreaColors,
    pub options: RowOptions,
    pub spacers: &'a [(usize, f32)],
    pub placeholder: Option<&'a str>,
}

pub fn style_size(style: SynHlStyle, body_size: f32) -> f32 {
    if style.family == SynHlFontFamily::Monospace {
        return CODE_SIZE * body_size / BODY_SIZE;
    }
    match style.size {
        SynHlTextSize::Body => body_size,
        SynHlTextSize::Heading(1) => 32.0,
        SynHlTextSize::Heading(2) => 28.0,
        SynHlTextSize::Heading(3) => 24.0,
        SynHlTextSize::Heading(4) => 21.0,
        SynHlTextSize::Heading(5) => 19.0,
        SynHlTextSize::Heading(_) => 18.0,
    }
}

pub fn style_font(style: SynHlStyle, body_size: f32) -> FontId {
    let size = style_size(style, body_size);
    let font = match style.family {
        SynHlFontFamily::Monospace => FontId::monospace(size),
        SynHlFontFamily::Proportional => FontId::proportional(size),
    };
    font.bold(style.bold).italic(style.italic)
}

pub fn mask(text: &str) -> String {
    MASK.repeat(text.chars().count())
}

pub fn galley(text: &str, font: FontId) -> Option<Galley> {
    layout_text(text, font, TextLayout::DEFAULT)
}

pub fn line_starts(bytes: &[u8]) -> Vec<usize> {
    std::iter::once(0)
        .chain(memchr::memchr_iter(b'\n', bytes).map(|at| at + 1))
        .collect()
}

pub fn line_range(bytes: &[u8], starts: &[usize], line: usize) -> Option<(usize, usize, bool)> {
    let start = *starts.get(line)?;
    match starts.get(line + 1) {
        Some(next) => Some((start, next - 1, true)),
        None => Some((start, bytes.len(), false)),
    }
}

pub fn line_of(starts: &[usize], byte: usize) -> usize {
    starts
        .partition_point(|start| *start <= byte)
        .saturating_sub(1)
}

fn next_character(bytes: &[u8]) -> Option<(char, usize)> {
    let valid = match std::str::from_utf8(bytes) {
        Ok(valid) => valid,
        Err(error) if error.valid_up_to() > 0 => {
            std::str::from_utf8(&bytes[..error.valid_up_to()]).ok()?
        }
        Err(_) => return None,
    };
    let character = valid.chars().next()?;
    Some((character, character.len_utf8()))
}

fn invisible_marker(byte: u8) -> Option<&'static str> {
    match byte {
        b' ' => Some("\u{00b7}"),
        b'\t' => Some("\u{21e5}"),
        b'\r' => Some("\u{240d}"),
        _ => None,
    }
}

struct Builder<'a> {
    inputs: &'a RowInputs<'a>,
    row: Row,
    fonts: Vec<FontId>,
}

impl Builder<'_> {
    fn span_style(&self, style: SynHlStyle) -> SpanStyle {
        SpanStyle {
            font: style_font(style, self.inputs.options.body_size)
                .bold(style.bold || self.inputs.options.bold()),
            color: self.inputs.colors.syntax.scope(style.color),
            underline: style.underline,
            strikethrough: style.strikethrough,
        }
    }

    fn push(
        &mut self,
        text: &str,
        source: Range<usize>,
        mapped: bool,
        style: SpanStyle,
        kind: SpanKind,
    ) {
        self.push_breaking(text, source, mapped, style, kind, false);
    }

    fn push_breaking(
        &mut self,
        text: &str,
        source: Range<usize>,
        mapped: bool,
        style: SpanStyle,
        kind: SpanKind,
        break_after: bool,
    ) {
        let start = self.row.display.len();
        self.row.display.push_str(text);
        let display = start..self.row.display.len();
        if kind == SpanKind::Text && !self.fonts.contains(&style.font) {
            self.fonts.push(style.font);
        }
        match self.row.spans.last_mut() {
            Some(last)
                if kind == SpanKind::Text
                    && last.kind == SpanKind::Text
                    && last.style == style
                    && last.break_after == break_after
                    && last.invisible.is_none()
                    && last.range.end == start =>
            {
                last.range.end = display.end;
            }
            _ => self.row.spans.push(RowSpan {
                range: display.clone(),
                style,
                kind,
                break_after,
                invisible: None,
            }),
        }
        match self.row.segments.last_mut() {
            Some(last)
                if mapped
                    && last.mapped
                    && last.source.end == source.start
                    && last.display.end == display.start =>
            {
                last.source.end = source.end;
                last.display.end = display.end;
            }
            _ => self.row.segments.push(Segment {
                source,
                display,
                mapped,
            }),
        }
    }

    fn invisible(&mut self, byte: usize, trailing: bool) {
        if let Some(last) = self.row.spans.last_mut()
            && !trailing
        {
            last.invisible = Some(byte);
        }
    }

    fn spacers(&mut self, at: usize, style: SpanStyle) {
        let widths: Vec<f32> = self
            .inputs
            .spacers
            .iter()
            .filter(|(byte, width)| *byte == at && *width > 0.0)
            .map(|(_, width)| *width)
            .collect();
        for width in widths {
            self.gap(width, style);
        }
    }

    fn gap(&mut self, width: f32, style: SpanStyle) {
        let at = self.row.display.len();
        self.row.spans.push(RowSpan {
            range: at..at,
            style,
            kind: SpanKind::Space(width),
            break_after: false,
            invisible: None,
        });
    }

    fn client_color(&self, client: u64) -> Option<Color32> {
        self.inputs
            .client_colors
            .iter()
            .find(|(named, _)| *named == client)
            .map(|(_, color)| *color)
    }

    fn author_color(&self, index: usize) -> Option<Color32> {
        if self.inputs.client_colors.is_empty() {
            return None;
        }
        let runs = self.inputs.snapshot.runs.as_deref()?;
        let before = runs.partition_point(|run| run.at <= index);
        let run = runs[..before].iter().rev().find(|run| run.visible)?;
        (run.at + run.len > index)
            .then(|| self.client_color(run.pos.client))
            .flatten()
    }

    fn runs(&mut self, runs: &[TextRun], at: usize, style: SpanStyle) {
        let gutter = self.inputs.colors.gutter_text;
        let first = runs.partition_point(|run| run.at < at);
        for run in runs[first..].iter().take_while(|run| run.at == at) {
            let color = self.client_color(run.pos.client).unwrap_or(gutter);
            let mut label = format!("@{}+{}", run.pos.client, run.pos.offset);
            if !run.visible && run.deleted.is_empty() {
                label.push_str(&format!(" -{}", run.len));
            }
            let chip = SpanStyle::new(FontId::monospace(RUN_CHIP_SIZE), color);
            self.inline(Inline::Run, at..at, chip, label);
            if !run.visible && !run.deleted.is_empty() {
                let struck = SpanStyle {
                    color,
                    strikethrough: true,
                    underline: false,
                    ..style
                };
                self.push(
                    &deleted_preview(&run.deleted),
                    at..at,
                    false,
                    struck,
                    SpanKind::Text,
                );
            }
        }
    }

    fn inline(&mut self, inline: Inline, source: Range<usize>, style: SpanStyle, label: String) {
        let size = inline_size(inline, &label, style);
        let index = self.row.inline.len();
        self.row.inline.push(InlineItem {
            inline,
            size,
            style,
            label,
        });
        self.push(OBJECT, source, false, style, SpanKind::Inline(index));
    }
}

fn deleted_preview(deleted: &[u8]) -> String {
    let text = String::from_utf8_lossy(deleted);
    let mut preview: String = text
        .chars()
        .take(DELETED_PREVIEW)
        .map(|character| match character {
            '\n' => NEWLINE_MARKER.to_owned(),
            other => other.to_string(),
        })
        .collect();
    let shown: usize = text.chars().take(DELETED_PREVIEW).map(char::len_utf8).sum();
    if shown < text.len() {
        preview.push_str(&format!("{ELLIPSIS} +{} bytes", text.len() - shown));
    }
    preview
}

pub fn inline_size(inline: Inline, label: &str, style: SpanStyle) -> Vec2 {
    match inline {
        Inline::Checkbox { .. } => Vec2::splat(CHECKBOX_WIDTH),
        Inline::Widget(_) => Vec2::new(
            galley(label, style.font).map_or(0.0, |galley| galley.size().x),
            INLINE_WIDGET_HEIGHT,
        ),
        Inline::Run => Vec2::new(
            galley(label, style.font).map_or(0.0, |galley| galley.size().x)
                + RUN_CHIP_PADDING * 2.0,
            RUN_CHIP_HEIGHT,
        ),
    }
}

pub fn widget_label(widget: &TextWidget) -> String {
    match widget.icon {
        Some(_) => format!("   {} ", widget.label),
        None => format!(" {} ", widget.label),
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct RowKey {
    bytes: Vec<u8>,
    newline: bool,
    styles: Vec<SynHlStyle>,
    styled_from_start: bool,
    code_block: bool,
    collapsed: bool,
    checkboxes: Vec<(Range<usize>, bool)>,
    widgets: Vec<(usize, Range<usize>, TextWidget)>,
    spacers: Vec<(usize, f32)>,
    composition: Option<Range<usize>>,
    placeholder: Option<String>,
}

impl RowKey {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub fn row_key(inputs: &RowInputs, start: usize, end: usize, newline: bool) -> Option<RowKey> {
    let snapshot = inputs.snapshot;
    if snapshot.runs.is_some() {
        return None;
    }
    let bytes = &snapshot.bytes;
    let highlight = snapshot.highlight();
    let held = start.min(bytes.len().saturating_sub(1));
    let relative =
        |range: &Range<usize>| range.start.saturating_sub(start)..range.end.saturating_sub(start);
    Some(RowKey {
        bytes: bytes[start..end].to_vec(),
        newline,
        styles: highlight.styles_in(held..(end + 1).min(bytes.len())),
        styled_from_start: held == start,
        code_block: highlight.in_code_block(start..end),
        collapsed: snapshot
            .sections
            .iter()
            .any(|section| section.collapsed && section.line_start == start),
        checkboxes: snapshot
            .checkboxes
            .iter()
            .filter(|checkbox| (start..=end).contains(&checkbox.marker.start))
            .map(|checkbox| (relative(&checkbox.marker), checkbox.checked))
            .collect(),
        widgets: inputs
            .widgets
            .iter()
            .enumerate()
            .filter(|(_, widget)| widget.range.start <= end && widget.range.end >= start)
            .map(|(index, widget)| {
                let range = relative(&widget.range);
                (
                    index,
                    range.clone(),
                    TextWidget {
                        range,
                        ..widget.clone()
                    },
                )
            })
            .collect(),
        spacers: inputs
            .spacers
            .iter()
            .map(|(at, width)| (at.saturating_sub(start), *width))
            .collect(),
        composition: inputs
            .composition
            .filter(|composition| composition.range.start < end && composition.range.end > start)
            .map(|composition| relative(&composition.range)),
        placeholder: inputs.placeholder.map(str::to_owned),
    })
}

pub fn build_row(inputs: &RowInputs, line: usize, start: usize, end: usize, newline: bool) -> Row {
    let snapshot = inputs.snapshot;
    let bytes = &snapshot.bytes;
    let highlight = snapshot.highlight();
    let options = inputs.options;
    let invisibles = options.invisibles();
    let composition = inputs
        .composition
        .filter(|composition| composition.range.start < end && composition.range.end > start);
    let composed =
        |index: usize| composition.is_some_and(|composition| composition.range.contains(&index));
    let held = start.min(bytes.len().saturating_sub(1));
    let held_styles = highlight.styles_in(held..(end + 1).min(bytes.len()));
    let style_at = |index: usize| {
        let index_read = index.min(bytes.len().saturating_sub(1));
        let mut style = match index_read.checked_sub(held) {
            Some(offset) if offset < held_styles.len() => held_styles[offset],
            _ => highlight.style_at(index_read),
        };
        if inputs
            .widgets
            .iter()
            .any(|widget| widget.italic && widget.range.contains(&index))
        {
            style.italic = true;
        }
        style
    };
    let mut builder = Builder {
        inputs,
        row: Row {
            line,
            start,
            end,
            ..Row::default()
        },
        fonts: Vec::new(),
    };
    let body = SpanStyle::new(
        FontId::proportional(options.body_size).bold(options.bold()),
        inputs
            .colors
            .syntax
            .scope(SynHlColorScope::MarkdownPlainText),
    );
    if let Some(placeholder) = inputs.placeholder {
        let style = SpanStyle {
            color: inputs.colors.placeholder,
            ..body
        };
        builder.push(placeholder, start..start, false, style, SpanKind::Text);
        builder.row.placeholder = true;
        return finish(builder, body);
    }
    let trailing_from = bytes[start..end]
        .iter()
        .rposition(|byte| !matches!(*byte, b' ' | b'\t' | b'\r'))
        .map_or(start, |index| start + index + 1);
    let invisible = inputs.colors.syntax.scope(SynHlColorScope::Invisible);
    let runs = match options.mask {
        true => &[][..],
        false => snapshot.runs_in(start, end),
    };
    let run_starts = |at: usize| runs.binary_search_by_key(&at, |run| run.at).is_ok();
    let mut index = start;
    while index < end {
        let base = builder.span_style(style_at(index));
        builder.spacers(index, base);
        builder.runs(runs, index, base);
        let base = builder.span_style(style_at(index));
        let base = SpanStyle {
            underline: base.underline || composed(index),
            ..base
        };
        if let Some(checkbox) = snapshot
            .checkboxes
            .iter()
            .find(|checkbox| checkbox.marker.start == index && checkbox.marker.end <= end)
        {
            let inline = Inline::Checkbox {
                line_start: checkbox.line_start,
                checked: checkbox.checked,
            };
            builder.inline(inline, checkbox.marker.clone(), base, String::new());
            index = checkbox.marker.end;
            continue;
        }
        if let Some((widget_index, widget)) = inputs
            .widgets
            .iter()
            .enumerate()
            .find(|(_, widget)| widget.range.start == index && widget.range.end <= end)
        {
            if widget.block_size.is_some() {
                builder.row.block = Some((widget_index, widget.block_size.unwrap_or_default()));
            }
            let label = widget_label(widget);
            builder.inline(
                Inline::Widget(widget_index),
                widget.range.clone(),
                base,
                label,
            );
            index = widget.range.end;
            continue;
        }
        if options.mask {
            let len = next_character(&bytes[index..end]).map_or(1, |(_, len)| len);
            builder.push(MASK, index..index + len, false, base, SpanKind::Text);
            index += len;
            continue;
        }
        let byte = bytes[index];
        if invisibles && let Some(marker) = invisible_marker(byte) {
            let trailing = index >= trailing_from;
            let color = match trailing {
                true => invisible,
                false => Color32::TRANSPARENT,
            };
            let style = SpanStyle { color, ..base };
            builder.push_breaking(marker, index..index + 1, false, style, SpanKind::Text, true);
            builder.invisible(index, trailing);
            index += 1;
            continue;
        }
        let Some((_, len)) = next_character(&bytes[index..end]) else {
            builder.push("\u{fffd}", index..index + 1, false, base, SpanKind::Text);
            index += 1;
            continue;
        };
        let style = style_at(index);
        let mut stop = index + len;
        while stop < end {
            if composition.is_some_and(|composition| {
                composition.range.start == stop || composition.range.end == stop
            }) || inputs.spacers.iter().any(|(at, _)| *at == stop)
                || snapshot
                    .checkboxes
                    .iter()
                    .any(|checkbox| checkbox.marker.start == stop)
                || inputs
                    .widgets
                    .iter()
                    .any(|widget| widget.range.start == stop)
                || style_at(stop) != style
                || run_starts(stop)
                || (invisibles && invisible_marker(bytes[stop]).is_some())
            {
                break;
            }
            let Some((_, len)) = next_character(&bytes[stop..end]) else {
                break;
            };
            stop += len;
        }
        let text = String::from_utf8_lossy(&bytes[index..stop]).into_owned();
        let base = match builder.author_color(index) {
            Some(color) => SpanStyle { color, ..base },
            None => base,
        };
        builder.push(&text, index..stop, true, base, SpanKind::Text);
        index = stop;
    }
    builder.spacers(end, body);
    builder.runs(runs, end, builder.span_style(style_at(end)));
    if newline && invisibles {
        let style = SpanStyle {
            color: Color32::TRANSPARENT,
            underline: false,
            strikethrough: false,
            ..builder.span_style(style_at(end))
        };
        builder.push(NEWLINE_MARKER, end..end + 1, false, style, SpanKind::Text);
        builder.invisible(end, false);
    }
    if snapshot
        .sections
        .iter()
        .any(|section| section.collapsed && section.line_start == start)
    {
        builder.gap(ELLIPSIS_GAP, body);
        let style = SpanStyle::new(FontId::monospace(BODY_SIZE), inputs.colors.gutter_arrow);
        builder.push(ELLIPSIS, end..end, false, style, SpanKind::Text);
    }
    finish(builder, body)
}

fn finish(mut builder: Builder, body: SpanStyle) -> Row {
    let padding = builder.inputs.options.padding();
    let mut height = galley("", body.font).map_or(0.0, |galley| galley.line_height());
    for font in &builder.fonts {
        if let Some(galley) = galley("", *font) {
            height = height.max(galley.line_height());
        }
    }
    builder.row.line_height = height + padding.0 + padding.1;
    let highlight = builder.inputs.snapshot.highlight();
    let (start, end) = (builder.row.start, builder.row.end);
    builder.row.code_block = !builder.row.placeholder && highlight.in_code_block(start..end);
    if builder.row.code_block {
        return builder.row;
    }
    let code = |byte: usize| highlight.style_at(byte).color == SynHlColorScope::MarkdownCode;
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut extend = |display: Range<usize>| match ranges.last_mut() {
        Some(last) if last.end == display.start => last.end = display.end,
        _ => ranges.push(display),
    };
    for segment in &builder.row.segments {
        if segment.source.is_empty() {
            continue;
        }
        if !segment.mapped {
            if segment.source.clone().all(code) {
                extend(segment.display.clone());
            }
            continue;
        }
        for byte in segment.source.clone() {
            if code(byte) {
                let display = segment.display.start + (byte - segment.source.start);
                extend(display..display + 1);
            }
        }
    }
    builder.row.code = ranges;
    builder.row
}

pub fn rich_layout(
    row: &Row,
    font: FontId,
    padding: (f32, f32),
    wrap_width: f32,
) -> Option<RichLayout> {
    let options = RichOptions {
        wrap_width,
        padding,
        style: SpanStyle::new(font, Color32::WHITE),
    };
    beui_view::reactive::rich_layout(&row.pieces(), options)
}

pub type TableSpacers = HashMap<usize, Vec<(usize, f32)>>;

type Cell = (Range<usize>, f32, f32);

pub struct TableInputs<'a> {
    pub wrap_width: f32,
    pub snapshot: &'a Snapshot,
    pub widgets: &'a [TextWidget],
    pub colors: &'a TextAreaColors,
    pub options: RowOptions,
    pub starts: &'a [usize],
    pub scale: f32,
    pub attached: u64,
    pub cache: &'a std::cell::RefCell<TableCache>,
}

type MeasuredRow = Option<(Vec<(Range<usize>, f32, f32)>, f32)>;

#[derive(Clone, PartialEq)]
struct TableEnv {
    colors: TextAreaColors,
    options: RowOptions,
    wrap_width: f32,
    scale: f32,
    attached: u64,
}

type TableRowKey = (RowKey, Vec<Range<usize>>);

#[derive(Default)]
pub struct TableCache {
    env: Option<TableEnv>,
    rows: Vec<(TableRowKey, MeasuredRow)>,
    used: Vec<(TableRowKey, MeasuredRow)>,
}

impl TableCache {
    fn start(&mut self, env: TableEnv) {
        if self.env.as_ref() != Some(&env) {
            self.env = Some(env);
            self.rows.clear();
        }
        self.used.clear();
    }

    fn finish(&mut self) {
        self.rows = std::mem::take(&mut self.used);
    }

    fn take(&mut self, key: &TableRowKey) -> Option<MeasuredRow> {
        if let Some(found) = self.rows.iter().position(|(held, _)| held == key) {
            let (key, measured) = self.rows.swap_remove(found);
            self.used.push((key, measured.clone()));
            return Some(measured);
        }
        self.used
            .iter()
            .find(|(held, _)| held == key)
            .map(|(_, measured)| measured.clone())
    }
}

fn measure_table_row(
    inputs: &TableInputs,
    line: usize,
    cells: &[Range<usize>],
) -> Option<(usize, MeasuredRow)> {
    let snapshot = inputs.snapshot;
    let (start, end, newline) = line_range(&snapshot.bytes, inputs.starts, line)?;
    let row_inputs = RowInputs {
        snapshot,
        widgets: inputs.widgets,
        client_colors: &[],
        composition: None,
        colors: inputs.colors,
        options: inputs.options,
        spacers: &[],
        placeholder: None,
    };
    let relative: Vec<Range<usize>> = cells
        .iter()
        .map(|cell| cell.start.saturating_sub(start)..cell.end.saturating_sub(start))
        .collect();
    let key = row_key(&row_inputs, start, end, newline).map(|key| (key, relative.clone()));
    if let Some(key) = &key
        && let Some(measured) = inputs.cache.borrow_mut().take(key)
    {
        return Some((start, measured));
    }
    let row = build_row(&row_inputs, line, start, end, newline);
    let measured = rich_layout(
        &row,
        FontId::proportional(inputs.options.body_size),
        (0.0, 0.0),
        inputs.wrap_width,
    )
    .and_then(|layout| {
        let first = layout.lines.first()?;
        let cells: Vec<(Range<usize>, f32, f32)> = cells
            .iter()
            .zip(&relative)
            .map(|(cell, relative)| {
                (
                    relative,
                    row.to_display(cell.start),
                    row.to_display(cell.end),
                )
            })
            .take_while(|(_, _, end)| *end <= first.range.end)
            .map(|(relative, start, end)| (relative.clone(), first.x_of(start), first.x_of(end)))
            .collect();
        (!cells.is_empty()).then_some((cells, first.width))
    });
    if let Some(key) = key {
        inputs.cache.borrow_mut().used.push((key, measured.clone()));
    }
    Some((start, measured))
}

pub fn table_spacers(inputs: &TableInputs) -> TableSpacers {
    inputs.cache.borrow_mut().start(TableEnv {
        colors: *inputs.colors,
        options: inputs.options,
        wrap_width: inputs.wrap_width,
        scale: inputs.scale,
        attached: inputs.attached,
    });
    let spacers = lay_out_tables(inputs);
    inputs.cache.borrow_mut().finish();
    spacers
}

fn lay_out_tables(inputs: &TableInputs) -> TableSpacers {
    let mut spacers = HashMap::new();
    let snapshot = inputs.snapshot;
    for table in &snapshot.highlight().markdown_tables() {
        let rows: Vec<(usize, Vec<Cell>, f32)> = table
            .rows
            .iter()
            .filter_map(|table_row| {
                let line = line_of(inputs.starts, table_row.range.start);
                let (start, measured) = measure_table_row(inputs, line, &table_row.cells)?;
                let (cells, width) = measured?;
                let cells = cells
                    .into_iter()
                    .map(|(cell, left, right)| (start + cell.start..start + cell.end, left, right))
                    .collect();
                Some((start, cells, width))
            })
            .collect();
        if rows.is_empty() {
            continue;
        }
        let columns = rows
            .iter()
            .map(|(_, cells, _)| cells.len())
            .max()
            .unwrap_or(0);
        let mut widths = vec![0.0_f32; columns];
        let mut gaps = vec![0.0_f32; columns.saturating_sub(1)];
        let mut prefix = 0.0_f32;
        for (_, cells, _) in &rows {
            prefix = prefix.max(cells[0].1);
            for (column, (_, start, end)) in cells.iter().enumerate() {
                widths[column] = widths[column].max((end - start).max(0.0));
                if let Some((_, next, _)) = cells.get(column + 1) {
                    gaps[column] = gaps[column].max((next - end).max(0.0));
                }
            }
        }
        for (line_start, cells, _) in rows {
            let mut inserted = 0.0_f32;
            let mut placed = Vec::new();
            let mut insert = |at: usize, natural: f32, wanted: f32, inserted: &mut f32| {
                let width = wanted - (natural + *inserted);
                if width > 0.0 {
                    placed.push((at, width));
                    *inserted += width;
                }
            };
            let mut target = prefix;
            for (column, (cell, start, end)) in cells.iter().enumerate() {
                let width = (end - start).max(0.0);
                let spare = (widths[column] - width).max(0.0);
                let leading = match table
                    .alignments
                    .get(column)
                    .copied()
                    .unwrap_or(MarkdownTableAlignment::Left)
                {
                    MarkdownTableAlignment::Left => 0.0,
                    MarkdownTableAlignment::Center => spare / 2.0,
                    MarkdownTableAlignment::Right => spare,
                };
                insert(cell.start, *start, target + leading, &mut inserted);
                target += widths[column];
                insert(cell.end, *end, target, &mut inserted);
                if let Some((_, next, _)) = cells.get(column + 1) {
                    target += gaps[column].max((next - end).max(0.0));
                }
            }
            spacers.insert(line_start, placed);
        }
    }
    spacers
}

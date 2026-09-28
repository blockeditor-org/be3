use std::collections::HashMap;
use std::ops::Range;

use text_editor_core::{
    MarkdownTableAlignment, SynHlColorScope, SynHlFontFamily, SynHlStyle, SynHlTextSize,
};

use beui_core::color::Color32;
use beui_core::font::{FontId, Galley, TextLayout};
use beui_core::geometry::Vec2;
use beui_core::rich::{RichLayout, RichOptions, SpanKind, SpanStyle, TextSpan};
use beui_view::reactive::layout_text;

use super::colors::TextAreaColors;
use super::state::Snapshot;

pub const BODY_SIZE: f32 = 12.0;
pub const CODE_SIZE: f32 = 12.0;
pub const CHECKBOX_WIDTH: f32 = 18.0;
pub const INLINE_WIDGET_HEIGHT: f32 = 24.0;
pub const INLINE_WIDGET_ICON_INSET: f32 = 13.0;
pub const DOCUMENT_PADDING: Vec2 = Vec2::new(24.0, 16.0);
pub const LINE_PADDING: (f32, f32) = (3.0, 4.0);

const MASK: &str = "*";
const OBJECT: &str = "\u{fffc}";
const SPACER: &str = "\u{200b}";
const NEWLINE_MARKER: &str = "\u{23ce}";
const ELLIPSIS: &str = "...";
const ELLIPSIS_SIZE: f32 = 12.0;
const ELLIPSIS_GAP: f32 = 6.0;

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
    pub at: usize,
    pub text: String,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RowOptions {
    pub body_size: f32,
    pub mask: bool,
    pub single_line: bool,
}

impl RowOptions {
    pub fn padding(&self) -> (f32, f32) {
        match self.single_line {
            true => (0.0, 0.0),
            false => LINE_PADDING,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Inline {
    Checkbox { line_start: usize, checked: bool },
    Widget(usize),
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

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Row {
    pub line: usize,
    pub start: usize,
    pub end: usize,
    pub display: String,
    pub spans: Vec<TextSpan>,
    pub segments: Vec<Segment>,
    pub inline: Vec<InlineItem>,
    pub line_height: f32,
    pub block: Option<(usize, Vec2)>,
    pub code: Vec<Range<usize>>,
}

impl Row {
    pub fn to_display(&self, byte: usize) -> usize {
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
            Some(segment) if segment.mapped => segment.source.start + (display - segment.display.start),
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

    pub fn inline_sizes(&self) -> impl Fn(usize) -> Vec2 + '_ {
        move |index| {
            self.inline
                .get(index)
                .map_or(Vec2::ZERO, |item| item.size)
        }
    }
}

pub struct RowInputs<'a> {
    pub snapshot: &'a Snapshot,
    pub widgets: &'a [TextWidget],
    pub composition: Option<&'a Composition>,
    pub selection: &'a [Range<usize>],
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
        .chain(
            bytes
                .iter()
                .enumerate()
                .filter(|(_, byte)| **byte == b'\n')
                .map(|(at, _)| at + 1),
        )
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
    starts.partition_point(|start| *start <= byte).saturating_sub(1)
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
            font: style_font(style, self.inputs.options.body_size),
            color: self.inputs.colors.syntax.scope(style.color),
            underline: style.underline,
            strikethrough: style.strikethrough,
        }
    }

    fn push(&mut self, text: &str, source: Range<usize>, mapped: bool, style: SpanStyle, kind: SpanKind) {
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
                    && last.range.end == start =>
            {
                last.range.end = display.end;
            }
            _ => self.row.spans.push(TextSpan {
                range: display.clone(),
                style,
                kind,
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

    fn spacers(&mut self, at: usize, style: SpanStyle) {
        let widths: Vec<f32> = self
            .inputs
            .spacers
            .iter()
            .filter(|(byte, width)| *byte == at && *width > 0.0)
            .map(|(_, width)| *width)
            .collect();
        for width in widths {
            self.push(SPACER, at..at, false, style, SpanKind::Space(width));
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

pub fn inline_size(inline: Inline, label: &str, style: SpanStyle) -> Vec2 {
    match inline {
        Inline::Checkbox { .. } => Vec2::splat(CHECKBOX_WIDTH),
        Inline::Widget(_) => Vec2::new(
            galley(label, style.font).map_or(0.0, |galley| galley.size().x),
            INLINE_WIDGET_HEIGHT,
        ),
    }
}

pub fn widget_label(widget: &TextWidget) -> String {
    match widget.icon {
        Some(_) => format!("   {} ", widget.label),
        None => format!(" {} ", widget.label),
    }
}

pub fn build_row(inputs: &RowInputs, line: usize, start: usize, end: usize, newline: bool) -> Row {
    let snapshot = inputs.snapshot;
    let bytes = &snapshot.bytes;
    let highlight = snapshot.highlight();
    let options = inputs.options;
    let invisibles = !options.single_line;
    let composition = inputs
        .composition
        .filter(|composition| composition.at >= start && composition.at <= end);
    let selected = |byte: usize| {
        inputs
            .selection
            .iter()
            .any(|range| range.start < range.end && range.contains(&byte))
    };
    let style_at = |index: usize| {
        let mut style = highlight.style_at(index.min(bytes.len().saturating_sub(1)));
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
        FontId::proportional(options.body_size),
        inputs.colors.syntax.scope(SynHlColorScope::MarkdownPlainText),
    );
    if let Some(placeholder) = inputs.placeholder {
        let style = SpanStyle {
            color: inputs.colors.placeholder,
            ..body
        };
        builder.push(placeholder, start..start, false, style, SpanKind::Text);
        return finish(builder, body);
    }
    let trailing_from = bytes[start..end]
        .iter()
        .rposition(|byte| !matches!(*byte, b' ' | b'\t' | b'\r'))
        .map_or(start, |index| start + index + 1);
    let invisible = inputs.colors.syntax.scope(SynHlColorScope::Invisible);
    let mut index = start;
    while index < end {
        let base = builder.span_style(style_at(index));
        builder.spacers(index, base);
        if let Some(composition) = composition.filter(|composition| composition.at == index) {
            compose(&mut builder, composition, index, bytes, style_at(index));
        }
        let base = builder.span_style(style_at(index));
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
            builder.inline(Inline::Widget(widget_index), widget.range.clone(), base, label);
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
            let color = match index >= trailing_from || selected(index) {
                true => invisible,
                false => Color32::TRANSPARENT,
            };
            let style = SpanStyle { color, ..base };
            builder.push(marker, index..index + 1, false, style, SpanKind::Text);
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
            if composition.is_some_and(|composition| composition.at == stop)
                || inputs.spacers.iter().any(|(at, _)| *at == stop)
                || snapshot
                    .checkboxes
                    .iter()
                    .any(|checkbox| checkbox.marker.start == stop)
                || inputs.widgets.iter().any(|widget| widget.range.start == stop)
                || style_at(stop) != style
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
        builder.push(&text, index..stop, true, base, SpanKind::Text);
        index = stop;
    }
    builder.spacers(end, body);
    if let Some(composition) = composition.filter(|composition| composition.at == end) {
        compose(&mut builder, composition, end, bytes, style_at(end));
    }
    if newline && invisibles {
        let color = match selected(end) {
            true => invisible,
            false => Color32::TRANSPARENT,
        };
        let style = SpanStyle {
            color,
            underline: false,
            strikethrough: false,
            ..builder.span_style(style_at(end))
        };
        builder.push(NEWLINE_MARKER, end..end + 1, false, style, SpanKind::Text);
    }
    if snapshot
        .sections
        .iter()
        .any(|section| section.collapsed && section.line_start == start)
    {
        builder.push(SPACER, end..end, false, body, SpanKind::Space(ELLIPSIS_GAP));
        let style = SpanStyle::new(FontId::monospace(ELLIPSIS_SIZE), inputs.colors.gutter_arrow);
        builder.push(ELLIPSIS, end..end, false, style, SpanKind::Text);
    }
    finish(builder, body)
}

fn compose(
    builder: &mut Builder,
    composition: &Composition,
    at: usize,
    bytes: &[u8],
    style: SynHlStyle,
) {
    let style = match at
        .checked_sub(1)
        .filter(|before| bytes.get(*before) != Some(&b'\n'))
    {
        Some(before) => builder.inputs.snapshot.highlight().style_at(before),
        None => style,
    };
    let style = SpanStyle {
        underline: true,
        ..builder.span_style(style)
    };
    builder.push(&composition.text, at..at, false, style, SpanKind::Text);
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
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for segment in &builder.row.segments {
        if !segment.mapped {
            continue;
        }
        for byte in segment.source.clone() {
            if highlight.style_at(byte).color != SynHlColorScope::MarkdownCode {
                continue;
            }
            let display = segment.display.start + (byte - segment.source.start);
            match ranges.last_mut() {
                Some(last) if last.end == display => last.end = display + 1,
                _ => ranges.push(display..display + 1),
            }
        }
    }
    builder.row.code = ranges;
    builder.row
}

pub fn rich_layout(
    row: &Row,
    body_size: f32,
    padding: (f32, f32),
    wrap_width: f32,
) -> Option<RichLayout> {
    galley("", FontId::proportional(body_size))?;
    let options = RichOptions {
        wrap_width,
        padding,
        style: SpanStyle::new(FontId::proportional(body_size), Color32::WHITE),
    };
    let sizes = row.inline_sizes();
    let mut shaper = |text: &str, font: FontId| {
        galley(text, font).unwrap_or_else(|| empty_galley(font))
    };
    Some(RichLayout::new(&row.display, &row.spans, &sizes, options, &mut shaper))
}

fn empty_galley(font: FontId) -> Galley {
    Galley::new("", font, Vec2::ZERO, 0.0, 0.0, Vec::new(), Vec::new())
}

pub struct TableInputs<'a> {
    pub snapshot: &'a Snapshot,
    pub widgets: &'a [TextWidget],
    pub colors: &'a TextAreaColors,
    pub options: RowOptions,
    pub starts: &'a [usize],
}

pub fn table_spacers(inputs: &TableInputs) -> HashMap<usize, Vec<(usize, f32)>> {
    let mut spacers = HashMap::new();
    let snapshot = inputs.snapshot;
    for table in snapshot.highlight().markdown_tables() {
        let rows: Vec<(usize, Vec<(Range<usize>, f32, f32)>, f32)> = table
            .rows
            .iter()
            .filter_map(|table_row| {
                let line = line_of(inputs.starts, table_row.range.start);
                let (start, end, newline) = line_range(&snapshot.bytes, inputs.starts, line)?;
                let row = build_row(
                    &RowInputs {
                        snapshot,
                        widgets: inputs.widgets,
                        composition: None,
                        selection: &[],
                        colors: inputs.colors,
                        options: inputs.options,
                        spacers: &[],
                        placeholder: None,
                    },
                    line,
                    start,
                    end,
                    newline,
                );
                let layout = rich_layout(&row, inputs.options.body_size, (0.0, 0.0), f32::INFINITY)?;
                let first = layout.lines.first()?;
                let cells: Vec<(Range<usize>, f32, f32)> = table_row
                    .cells
                    .iter()
                    .map(|cell| {
                        (
                            cell.clone(),
                            first.x_of(row.to_display(cell.start)),
                            first.x_of(row.to_display(cell.end)),
                        )
                    })
                    .collect();
                (!cells.is_empty()).then_some((start, cells, first.width))
            })
            .collect();
        if rows.is_empty() {
            continue;
        }
        let columns = rows.iter().map(|(_, cells, _)| cells.len()).max().unwrap_or(0);
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

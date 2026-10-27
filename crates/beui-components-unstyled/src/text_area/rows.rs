use std::ops::Range;

use text_editor_core::{
    LineLook, ListMarker, MarkSet, SynHlColorScope, SynHlFontFamily, SynHlStyle, SynHlTextSize,
    TableRow, TextLanguage, TextRun, VOID,
};

use beui_view::reactive::*;
use beui_view::types::Color;
use beui_view::types::SpanStyle;
use beui_view::types::Vec2;
use beui_view::types::{DEFAULT_FONT_SIZE, Piece};
use beui_view::types::{FontId, TextAlign};

use super::colors::TextAreaColors;
use super::look::{BlockGeometry, TextAreaGeometry};
use super::state::Snapshot;

mod table;

pub use table::{CellAlign, RowTable, TableColumns, table_columns};

pub const BODY_SIZE: f32 = DEFAULT_FONT_SIZE;
pub const CODE_SIZE: f32 = BODY_SIZE;

const MASK: &str = "*";
const OBJECT: &str = "\u{fffc}";
const NEWLINE_MARKER: &str = "\u{23ce}";
const NEWLINE_SLOT: &str = "mm";
const CARRIAGE_RETURN: &str = "\u{200b}";
const ELLIPSIS: &str = "...";
const DELETED_PREVIEW: usize = 48;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextWidget {
    pub range: Range<usize>,
    pub label: String,
    pub icon: Option<beui_view::types::icons::Glyph>,
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
    pub geometry: TextAreaGeometry,
}

impl RowOptions {
    pub fn padding(&self) -> (f32, f32) {
        match (self.single_line, self.prose) {
            (true, _) => (0.0, 0.0),
            (false, Some(prose)) => prose.line_padding,
            (false, None) => self.geometry.line_padding,
        }
    }

    pub fn scale(&self) -> f32 {
        self.body_size / BODY_SIZE
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
    Marker(ListMarker),
    Quote,
    Widget(usize),
    Run,
    Void,
}

#[derive(Clone, PartialEq, Debug)]
pub struct InlineItem {
    pub inline: Inline,
    pub style: SpanStyle,
    pub label: String,
    pub props: MarkSet,
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
    pub leaf: Option<SynHlStyle>,
    pub kind: SpanKind,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Invisible {
    pub source: usize,
    pub display: Range<usize>,
    pub glyph: &'static str,
    pub font: FontId,
    pub trailing: bool,
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
    pub look: LineLook,
    pub hang: usize,
    pub outdent: f32,
    pub space: (f32, f32),
    pub placeholder: bool,
    pub props: MarkSet,
    pub invisibles: Vec<Invisible>,
    pub table: Option<RowTable>,
}

impl Row {
    pub fn moved(&self, line: usize, start: usize) -> Row {
        let shift = |at: usize| at + start - self.start;
        Row {
            line,
            start,
            end: shift(self.end),
            look: self.look.shifted(start as isize - self.start as isize),
            segments: self
                .segments
                .iter()
                .map(|segment| Segment {
                    source: shift(segment.source.start)..shift(segment.source.end),
                    ..segment.clone()
                })
                .collect(),
            invisibles: self
                .invisibles
                .iter()
                .map(|invisible| Invisible {
                    source: shift(invisible.source),
                    ..invisible.clone()
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

    pub fn inline_source(&self, item: usize) -> Option<Range<usize>> {
        let span = self
            .spans
            .iter()
            .find(|span| span.kind == SpanKind::Inline(item))?;
        Some(self.to_source(span.range.start)..self.to_source(span.range.end))
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

    pub fn pieces(&self, item: Vec2) -> Vec<Piece<'_>> {
        let mut flow = 0;
        self.spans
            .iter()
            .map(|span| {
                let text = self.display.get(span.range.clone()).unwrap_or_default();
                match span.kind {
                    SpanKind::Text => Piece::text(text, span.style),
                    SpanKind::Space(width) => Piece::Gap(width),
                    SpanKind::Inline(_) => {
                        flow += 1;
                        Piece::Inline {
                            index: flow - 1,
                            size: item,
                            text,
                        }
                    }
                }
            })
            .collect()
    }

    pub fn flow_item(&self, flow: usize) -> Option<usize> {
        self.spans
            .iter()
            .filter(|span| matches!(span.kind, SpanKind::Inline(_)))
            .nth(flow)
            .and_then(|span| match span.kind {
                SpanKind::Inline(index) => Some(index),
                _ => None,
            })
    }

    pub fn flow_inline(&self, flow: usize) -> Option<&InlineItem> {
        self.inline.get(self.flow_item(flow)?)
    }
}

pub struct RowInputs<'a> {
    pub snapshot: &'a Snapshot,
    pub widgets: &'a [TextWidget],
    pub client_colors: &'a [(u64, Color)],
    pub composition: Option<&'a Composition>,
    pub colors: &'a TextAreaColors,
    pub options: RowOptions,
    pub placeholder: Option<&'a str>,
    pub columns: Option<std::rc::Rc<TableColumns>>,
}

pub fn style_size(style: SynHlStyle, body_size: f32) -> f32 {
    if style.family == SynHlFontFamily::Monospace {
        return CODE_SIZE * body_size / BODY_SIZE;
    }
    match style.size {
        SynHlTextSize::Body => body_size,
        SynHlTextSize::Heading(1) => body_size * 24.0 / BODY_SIZE,
        SynHlTextSize::Heading(2) => body_size * 19.0 / BODY_SIZE,
        SynHlTextSize::Heading(3) => body_size * 16.0 / BODY_SIZE,
        SynHlTextSize::Heading(_) => body_size,
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

fn line_start_of(bytes: &[u8], at: usize) -> usize {
    memchr::memrchr(b'\n', &bytes[..at.min(bytes.len())]).map_or(0, |newline| newline + 1)
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

fn invisibles_of(
    row: &Row,
    bytes: &[u8],
    skip: &Range<usize>,
    trailing_from: usize,
) -> Vec<Invisible> {
    let mut found = Vec::new();
    for (at, byte) in bytes.iter().enumerate().take(row.end).skip(row.start) {
        let Some(glyph) = invisible_marker(*byte) else {
            continue;
        };
        if skip.contains(&at) {
            continue;
        }
        let display = row.to_display(at)..row.to_display(at + 1);
        let shown = row.display.get(display.clone()).unwrap_or_default();
        let real = match byte {
            b'\r' => shown == CARRIAGE_RETURN,
            _ => shown.as_bytes() == [*byte],
        };
        let Some(span) = row
            .spans
            .iter()
            .find(|span| span.kind == SpanKind::Text && span.range.contains(&display.start))
            .filter(|_| real)
        else {
            continue;
        };
        found.push(Invisible {
            source: at,
            display,
            glyph,
            font: span.style.font,
            trailing: at >= trailing_from,
        });
    }
    found
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
                    && last.leaf.is_none()
                    && last.range.end == start =>
            {
                last.range.end = display.end;
            }
            _ => self.row.spans.push(RowSpan {
                range: display.clone(),
                style,
                leaf: None,
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

    fn leaf(&mut self, syntax: SynHlStyle) {
        if let Some(last) = self.row.spans.last_mut() {
            last.leaf = Some(syntax);
        }
    }

    fn marker(&mut self, look: &LineLook, source: Range<usize>, body: SpanStyle) {
        let shown =
            String::from_utf8_lossy(&self.inputs.snapshot.bytes[source.clone()]).into_owned();
        let (text, mapped) = match source.is_empty() {
            true => (OBJECT, false),
            false => (shown.as_str(), true),
        };
        let inline = match look.list {
            Some(list) => Inline::Marker(list),
            None => Inline::Quote,
        };
        let label = marker_label(look.list, &shown);
        let item = self.push_inline(inline, body, label);
        self.push(text, source, mapped, body, SpanKind::Inline(item));
        self.row.hang = self.row.display.len();
    }

    fn gap(&mut self, width: f32, style: SpanStyle) {
        let at = self.row.display.len();
        self.row.spans.push(RowSpan {
            range: at..at,
            style,
            leaf: None,
            kind: SpanKind::Space(width),
        });
    }

    fn client_color(&self, client: u64) -> Option<Color> {
        self.inputs
            .client_colors
            .iter()
            .find(|(named, _)| *named == client)
            .map(|(_, color)| *color)
    }

    fn author_color(&self, index: usize) -> Option<Color> {
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
            let chip = SpanStyle::new(
                FontId::monospace(self.inputs.options.geometry.run_chip.font_size),
                color,
            );
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
        let index = self.push_inline(inline, style, label);
        self.push(OBJECT, source, false, style, SpanKind::Inline(index));
    }

    fn push_inline(&mut self, inline: Inline, style: SpanStyle, label: String) -> usize {
        self.row.inline.push(InlineItem {
            inline,
            style,
            label,
            props: MarkSet::default(),
        });
        self.row.inline.len() - 1
    }
}

fn marker_label(list: Option<ListMarker>, shown: &str) -> String {
    let written = shown
        .trim_end()
        .rsplit([' ', '\t', '>'])
        .next()
        .unwrap_or_default();
    match list {
        Some(ListMarker::Bullet) if matches!(written, "-" | "*" | "+") => written.to_owned(),
        Some(ListMarker::Number(_))
            if written.ends_with(['.', ')'])
                && written[..written.len() - 1]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit()) =>
        {
            written.to_owned()
        }
        _ => String::new(),
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

pub fn widget_label(widget: &TextWidget) -> String {
    format!(" {} ", widget.label)
}

#[derive(Clone, PartialEq, Debug)]
pub struct RowKey {
    bytes: Vec<u8>,
    newline: bool,
    styles: Vec<SynHlStyle>,
    styled_from_start: bool,
    collapsed: bool,
    widgets: Vec<(usize, Range<usize>, TextWidget)>,
    composition: Option<Range<usize>>,
    placeholder: Option<String>,
    props: MarkSet,
    look: LineLook,
    voids: Vec<MarkSet>,
    columns: Option<std::rc::Rc<TableColumns>>,
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
        collapsed: snapshot
            .sections
            .iter()
            .any(|section| section.collapsed && section.line_start == start),
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
        composition: inputs
            .composition
            .filter(|composition| composition.range.start < end && composition.range.end > start)
            .map(|composition| relative(&composition.range)),
        placeholder: inputs.placeholder.map(str::to_owned),
        props: snapshot
            .formats
            .as_deref()
            .and_then(|formats| formats.block_at(start))
            .map(|block| block.props.clone())
            .unwrap_or_default(),
        look: highlight.line_look(start).shifted(-(start as isize)),
        voids: snapshot
            .formats
            .as_deref()
            .map_or_else(Vec::new, |formats| {
                memchr::memmem::find_iter(&bytes[start..end], VOID.as_bytes())
                    .map(|at| formats.marks_at(start + at).cloned().unwrap_or_default())
                    .collect()
            }),
        columns: inputs.columns.clone(),
    })
}

pub fn build_row(inputs: &RowInputs, line: usize, start: usize, end: usize, newline: bool) -> Row {
    let snapshot = inputs.snapshot;
    let bytes = &snapshot.bytes;
    let highlight = snapshot.highlight();
    let options = inputs.options;
    let invisibles = options.invisibles() && snapshot.language != TextLanguage::RichText;
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
    let formats = snapshot.formats.as_deref();
    let mut builder = Builder {
        inputs,
        row: Row {
            line,
            start,
            end,
            props: formats
                .and_then(|formats| formats.block_at(start))
                .map(|block| block.props.clone())
                .unwrap_or_default(),
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
    let look = match options.mask {
        true => LineLook::at(start),
        false => highlight.line_look(start),
    };
    builder.row.look = look.clone();
    let marker = look.marker.start.clamp(start, end)..look.marker.end.clamp(start, end);
    let mut marked = look.list.is_some() || !marker.is_empty();
    if let (Some(table), Some(columns)) = (look.table, inputs.columns.clone())
        && !options.mask
    {
        if marked {
            builder.marker(&look, marker.clone(), body);
        }
        builder.table_row(table, marker.end..end, &columns, body);
        builder.row.space = line_space(&look, line, &options);
        let delimiter = table.row == TableRow::Delimiter;
        let mut row = finish(builder, body);
        if delimiter {
            let padding = options.padding();
            row.line_height = table::delimiter_height(inputs) + padding.0 + padding.1;
        }
        return row;
    }
    let trailing_from = bytes[start..end]
        .iter()
        .rposition(|byte| !matches!(*byte, b' ' | b'\t' | b'\r'))
        .map_or(start, |index| start + index + 1);
    let runs = match options.mask {
        true => &[][..],
        false => snapshot.runs_in(start, end),
    };
    let run_starts = |at: usize| runs.binary_search_by_key(&at, |run| run.at).is_ok();
    let mut index = start;
    while index < end || marked {
        if marked && index >= marker.start {
            builder.marker(&look, marker.clone(), body);
            marked = false;
            index = marker.end;
            continue;
        }
        let base = builder.span_style(style_at(index));
        builder.runs(runs, index, base);
        let base = builder.span_style(style_at(index));
        let base = SpanStyle {
            underline: base.underline || composed(index),
            ..base
        };
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
        if let Some(formats) = formats
            && bytes[index..end].starts_with(VOID.as_bytes())
        {
            let props = formats.marks_at(index).cloned().unwrap_or_default();
            let item = builder.push_inline(Inline::Void, base, String::new());
            builder.row.inline[item].props = props;
            builder.push(
                OBJECT,
                index..index + VOID.len(),
                false,
                base,
                SpanKind::Inline(item),
            );
            index += VOID.len();
            continue;
        }
        if options.mask {
            let len = next_character(&bytes[index..end]).map_or(1, |(_, len)| len);
            builder.push(MASK, index..index + len, false, base, SpanKind::Text);
            index += len;
            continue;
        }
        let byte = bytes[index];
        if invisibles && byte == b'\r' {
            builder.push(
                CARRIAGE_RETURN,
                index..index + 1,
                false,
                base,
                SpanKind::Text,
            );
            builder.leaf(style_at(index));
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
            }) || (marked && stop == marker.start)
                || inputs
                    .widgets
                    .iter()
                    .any(|widget| widget.range.start == stop)
                || style_at(stop) != style
                || run_starts(stop)
                || (formats.is_some() && bytes[stop..end].starts_with(VOID.as_bytes()))
                || (invisibles && bytes[stop] == b'\r')
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
        builder.leaf(style);
        index = stop;
    }
    builder.runs(runs, end, builder.span_style(style_at(end)));
    let ending = builder.span_style(style_at(end)).font;
    if !builder.fonts.contains(&ending) {
        builder.fonts.push(ending);
    }
    if invisibles {
        builder.row.invisibles = invisibles_of(&builder.row, bytes, &marker, trailing_from);
    }
    hang_into_margin(&mut builder.row, &look, &options);
    builder.row.space = line_space(&look, line, &options);
    if newline && invisibles {
        let style = SpanStyle {
            color: Color::TRANSPARENT,
            underline: false,
            strikethrough: false,
            ..builder.span_style(style_at(end))
        };
        let at = builder.row.display.len();
        builder.push(NEWLINE_SLOT, end..end + 1, false, style, SpanKind::Text);
        builder.row.invisibles.push(Invisible {
            source: end,
            display: at..builder.row.display.len(),
            glyph: NEWLINE_MARKER,
            font: style.font,
            trailing: false,
        });
    }
    if snapshot
        .sections
        .iter()
        .any(|section| section.collapsed && section.line_start == start)
    {
        builder.gap(options.geometry.ellipsis_gap, body);
        let style = SpanStyle::new(
            FontId::monospace(options.body_size),
            inputs.colors.gutter_arrow,
        );
        builder.push(ELLIPSIS, end..end, false, style, SpanKind::Text);
    }
    finish(builder, body)
}

fn hang_into_margin(row: &mut Row, look: &LineLook, options: &RowOptions) {
    if look.hang.is_empty() || !look.marker.is_empty() {
        return;
    }
    let end = row.to_display(look.hang.end);
    let width: f32 = row
        .spans
        .iter()
        .filter(|span| span.kind == SpanKind::Text && span.range.start < end)
        .filter_map(|span| {
            let text = row.display.get(span.range.start..span.range.end.min(end))?;
            measure_line(text, span.style.font).map(|line| line.x)
        })
        .sum();
    row.outdent = match look.rule {
        true => width + options.geometry.blocks.rule_gap * options.scale(),
        false => width,
    };
    row.hang = end;
}

fn line_space(look: &LineLook, line: usize, options: &RowOptions) -> (f32, f32) {
    let BlockGeometry {
        heading_space,
        quote_space,
        code_space,
        table_space,
        ..
    } = options.geometry.blocks;
    let scale = options.scale();
    let mut above = match look.heading {
        1..=3 if line > 0 => heading_space[usize::from(look.heading) - 1],
        _ => 0.0,
    };
    let mut below = 0.0;
    if look.quote > 0 && look.quote_above == 0 {
        above += quote_space;
    }
    if look.quote > 0 && look.quote_below == 0 {
        below += quote_space;
    }
    if look.code_block && !look.code_above {
        above += code_space;
    }
    if look.code_block && !look.code_below {
        below += code_space;
    }
    if look
        .table
        .is_some_and(|table| table.row != TableRow::Delimiter)
    {
        above += table_space;
        below += table_space;
    }
    (above * scale, below * scale)
}

fn finish(mut builder: Builder, body: SpanStyle) -> Row {
    let padding = builder.inputs.options.padding();
    let mut height = measure_line("", body.font).map_or(0.0, |line| line.y);
    for font in &builder.fonts {
        if let Some(line) = measure_line("", *font) {
            height = height.max(line.y);
        }
    }
    builder.row.line_height = height + padding.0 + padding.1;
    builder.row
}

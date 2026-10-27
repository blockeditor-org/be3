use std::ops::Range;

use beui_view::types::Color;
use beui_view::types::FontId;
use beui_view::types::SpanStyle;
use text_editor_core::{SynHlColorScope, SynHlStyle, TableLine, TableRow};

use beui_view::measure::measure_line;

use super::{Builder, RowInputs, SpanKind, next_character, style_font};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellAlign {
    Start,
    Center,
    End,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct TableColumns {
    pub widths: Vec<f32>,
    pub align: Vec<CellAlign>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct RowTable {
    pub row: TableRow,
    pub last: bool,
    pub edges: Vec<(f32, bool)>,
}

struct Cell {
    pipe: Option<usize>,
    range: Range<usize>,
}

struct Cells {
    cells: Vec<Cell>,
    closing: Option<usize>,
}

fn unescaped_pipes(bytes: &[u8], range: Range<usize>) -> Vec<usize> {
    let mut pipes = Vec::new();
    let mut at = range.start;
    while at < range.end {
        match bytes[at] {
            b'\\' => at += 2,
            b'|' => {
                pipes.push(at);
                at += 1;
            }
            _ => at += 1,
        }
    }
    pipes
}

fn cells(bytes: &[u8], range: Range<usize>) -> Cells {
    let first = range.start
        + bytes[range.clone()]
            .iter()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count();
    let mut cells = Vec::new();
    let mut cursor = range.start;
    let mut pipe = None;
    for at in unescaped_pipes(bytes, range.clone()) {
        if at != first {
            cells.push(Cell {
                pipe,
                range: cursor..at,
            });
        }
        pipe = Some(at);
        cursor = at + 1;
    }
    let rest = cursor..range.end;
    match bytes[rest.clone()].trim_ascii().is_empty() && pipe.is_some() {
        true => Cells {
            cells,
            closing: pipe,
        },
        false => {
            cells.push(Cell { pipe, range: rest });
            Cells {
                cells,
                closing: None,
            }
        }
    }
}

fn trimmed(bytes: &[u8], range: &Range<usize>) -> Range<usize> {
    let lead = bytes[range.clone()]
        .iter()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count();
    let trail = bytes[range.clone()]
        .iter()
        .rev()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count();
    let start = range.start + lead;
    start..(range.end - trail).max(start)
}

fn text_width(text: &str, font: FontId) -> f32 {
    let probe = format!("{text}|");
    let with = measure_line(&probe, font).map_or(0.0, |line| line.x);
    let bar = measure_line("|", font).map_or(0.0, |line| line.x);
    (with - bar).max(0.0)
}

fn delimiter_font(inputs: &RowInputs) -> FontId {
    let size = inputs.options.geometry.blocks.delimiter_size * inputs.options.scale();
    FontId::monospace(size.max(1.0))
}

pub(super) fn delimiter_height(inputs: &RowInputs) -> f32 {
    measure_line("", delimiter_font(inputs)).map_or(0.0, |line| line.y)
}

fn font_of(inputs: &RowInputs, row: TableRow, style: SynHlStyle) -> FontId {
    match row {
        TableRow::Delimiter => delimiter_font(inputs),
        _ => style_font(style, inputs.options.body_size).bold(style.bold),
    }
}

fn runs(inputs: &RowInputs, range: Range<usize>) -> Vec<(Range<usize>, SynHlStyle)> {
    let bytes = &inputs.snapshot.bytes;
    let highlight = inputs.snapshot.highlight();
    let styles = highlight.styles_in(range.clone());
    let style_at = |at: usize| {
        styles
            .get(at - range.start)
            .copied()
            .unwrap_or_else(|| highlight.style_at(at))
    };
    let mut runs: Vec<(Range<usize>, SynHlStyle)> = Vec::new();
    let mut at = range.start;
    while at < range.end {
        let len = next_character(&bytes[at..range.end]).map_or(1, |(_, len)| len);
        let style = style_at(at);
        match runs.last_mut() {
            Some((held, same)) if *same == style && held.end == at => held.end = at + len,
            _ => runs.push((at..at + len, style)),
        }
        at += len;
    }
    runs
}

fn width_of(inputs: &RowInputs, row: TableRow, range: Range<usize>) -> f32 {
    let bytes = &inputs.snapshot.bytes;
    runs(inputs, range)
        .into_iter()
        .map(|(range, style)| {
            text_width(
                &String::from_utf8_lossy(&bytes[range]),
                font_of(inputs, row, style),
            )
        })
        .sum()
}

fn alignment(bytes: &[u8]) -> CellAlign {
    let text = bytes.trim_ascii();
    match (text.first() == Some(&b':'), text.last() == Some(&b':')) {
        (true, true) => CellAlign::Center,
        (false, true) => CellAlign::End,
        _ => CellAlign::Start,
    }
}

pub fn table_columns(inputs: &RowInputs, start: usize) -> TableColumns {
    let bytes = &inputs.snapshot.bytes;
    let highlight = inputs.snapshot.highlight();
    let mut columns = TableColumns::default();
    let mut line = super::line_start_of(bytes, start);
    loop {
        let look = highlight.line_look(line);
        let Some(table) = look.table.filter(|table| table.start == start) else {
            break;
        };
        let end = memchr::memchr(b'\n', &bytes[line..]).map_or(bytes.len(), |at| line + at);
        let content = look.marker.end.clamp(line, end)..end;
        for (index, cell) in cells(bytes, content).cells.iter().enumerate() {
            let shown = trimmed(bytes, &cell.range);
            let width = width_of(inputs, table.row, shown.clone());
            if columns.widths.len() <= index {
                columns.widths.resize(index + 1, 0.0);
                columns.align.resize(index + 1, CellAlign::Start);
            }
            columns.widths[index] = columns.widths[index].max(width);
            if table.row == TableRow::Delimiter {
                columns.align[index] = alignment(&bytes[shown]);
            }
        }
        if table.last || end >= bytes.len() {
            break;
        }
        line = end + 1;
    }
    columns
}

impl Builder<'_> {
    fn table_text(&mut self, range: Range<usize>, row: TableRow, color: Option<Color>) -> f32 {
        let inputs = self.inputs;
        let bytes = &inputs.snapshot.bytes;
        let mut width = 0.0;
        for (range, style) in runs(inputs, range) {
            let text = String::from_utf8_lossy(&bytes[range.clone()]).into_owned();
            let base = self.span_style(style);
            let shown = SpanStyle {
                font: font_of(inputs, row, style),
                color: color.unwrap_or(base.color),
                ..base
            };
            width += text_width(&text, shown.font);
            self.push(&text, range, true, shown, SpanKind::Text);
            self.leaf(style);
        }
        width
    }

    fn table_gap(&mut self, width: f32, style: SpanStyle) {
        if width > 0.0 {
            self.gap(width, style);
        }
    }

    pub(super) fn table_row(
        &mut self,
        table: TableLine,
        content: Range<usize>,
        columns: &TableColumns,
        body: SpanStyle,
    ) {
        let inputs = self.inputs;
        let bytes = &inputs.snapshot.bytes;
        let padding = inputs.options.geometry.blocks.cell_padding * inputs.options.scale();
        let symbol = SynHlStyle::plain(SynHlColorScope::MarkdownSymbol);
        let pipe = text_width("|", font_of(inputs, table.row, symbol));
        let body_pipe = text_width("|", font_of(inputs, TableRow::Body, symbol));
        let parsed = cells(bytes, content.clone());
        let count = columns.widths.len().max(parsed.cells.len());
        let mut edges = Vec::with_capacity(count + 1);
        let mut edge = body_pipe / 2.0;
        for index in 0..count {
            edges.push(edge);
            edge += padding * 2.0 + columns.widths.get(index).copied().unwrap_or(0.0);
        }
        edges.push(edge);
        let hidden = Some(Color::TRANSPARENT);
        let mut x = 0.0;
        for (index, cell) in parsed.cells.iter().enumerate() {
            let at = edges[index] - pipe / 2.0;
            if let Some(pipe_at) = cell.pipe {
                self.table_gap(at - x, body);
                x = x.max(at);
                x += self.table_text(pipe_at..pipe_at + 1, table.row, hidden);
            }
            let shown = trimmed(bytes, &cell.range);
            let width = width_of(inputs, table.row, shown.clone());
            let room = columns.widths.get(index).copied().unwrap_or(width) - width;
            let offset = match columns.align.get(index) {
                Some(CellAlign::Center) => room / 2.0,
                Some(CellAlign::End) => room,
                _ => 0.0,
            };
            let lead = cell.range.start..shown.start;
            let lead_width = width_of(inputs, table.row, lead.clone());
            self.table_gap(edges[index] + padding + offset - lead_width - x, body);
            x = x.max(edges[index] + padding + offset - lead_width);
            x += self.table_text(lead, table.row, None);
            x += self.table_text(shown.clone(), table.row, None);
            x += self.table_text(shown.end..cell.range.end, table.row, None);
        }
        let mut tail = parsed
            .cells
            .last()
            .map_or(content.start, |cell| cell.range.end);
        if let Some(closing) = parsed.closing {
            let at = edges[count] - pipe / 2.0;
            self.table_gap(at - x, body);
            self.table_text(closing..closing + 1, table.row, hidden);
            tail = closing + 1;
        }
        self.table_text(tail..content.end, table.row, None);
        let leading = parsed.cells.first().is_some_and(|cell| cell.pipe.is_some());
        self.row.table = Some(RowTable {
            row: table.row,
            last: table.last,
            edges: edges
                .into_iter()
                .enumerate()
                .map(|(index, edge)| {
                    let drawn = match index {
                        0 => leading,
                        _ if index == count => parsed.closing.is_some(),
                        _ => parsed
                            .cells
                            .get(index)
                            .is_some_and(|cell| cell.pipe.is_some()),
                    };
                    (edge, drawn)
                })
                .collect(),
        });
    }
}

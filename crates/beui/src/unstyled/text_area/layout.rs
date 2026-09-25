use std::ops::Range;

use crate::font::{FontId, Galley, TextLayout};
use crate::geometry::{Pos2, Rect, Vec2};
use crate::reactive::layout_text;
use text_editor_core::{
    MarkdownTable, MarkdownTableAlignment, SynHlFontFamily, SynHlStyle, SynHlTextSize,
    SyntaxHighlight,
};

pub(crate) const BODY_SIZE: f32 = 12.0;
pub(crate) const CODE_SIZE: f32 = 12.0;
pub(crate) const CHECKBOX_WIDTH: f32 = 18.0;
pub(crate) const INLINE_WIDGET_HEIGHT: f32 = 24.0;
pub(crate) const INLINE_WIDGET_ICON_INSET: f32 = 13.0;
pub(crate) const DOCUMENT_PADDING: Vec2 = Vec2::new(24.0, 16.0);

const WRAP_FALLBACK_REMAINING_WIDTH: f32 = 0.15;
const MASK: &str = "*";
const LINE_PADDING_TOP: f32 = 3.0;
const LINE_PADDING_BOTTOM: f32 = 4.0;

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
    pub(crate) fn block(&self) -> bool {
        self.block_size.is_some()
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct LayoutOptions {
    pub wrap_width: f32,
    pub mask: bool,
    pub body_size: f32,
    pub single_line: bool,
}

impl LayoutOptions {
    pub(crate) fn wrapped(wrap_width: f32) -> Self {
        Self {
            wrap_width,
            mask: false,
            body_size: BODY_SIZE,
            single_line: false,
        }
    }

    pub(crate) fn single_line(body_size: f32) -> Self {
        Self {
            wrap_width: f32::INFINITY,
            mask: false,
            body_size,
            single_line: true,
        }
    }

    fn line_padding(&self) -> (f32, f32) {
        match self.single_line {
            true => (0.0, 0.0),
            false => (LINE_PADDING_TOP, LINE_PADDING_BOTTOM),
        }
    }

    fn document_padding(&self) -> Vec2 {
        match self.single_line {
            true => Vec2::ZERO,
            false => DOCUMENT_PADDING,
        }
    }
}

#[derive(Clone)]
pub(crate) struct WidgetLayout {
    pub index: usize,
    pub range: Range<usize>,
    pub icon: Option<&'static str>,
    pub broken: bool,
    pub block: bool,
    pub rect: Rect,
}

#[derive(Clone, Copy)]
pub(crate) struct BytePosition {
    pub line: usize,
    pub x: f32,
}

#[derive(Clone)]
pub(crate) struct Run {
    pub range: Range<usize>,
    pub x: f32,
    pub width: f32,
    pub galley: Option<Galley>,
    pub style: SynHlStyle,
    pub font_size: f32,
    pub invisible: bool,
    pub show_when_trailing: bool,
    mapped: bool,
}

#[derive(Clone)]
pub(crate) struct LineLayout {
    pub start: usize,
    pub end: usize,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
    pub document_line: usize,
    pub show_line_number: bool,
    pub runs: Vec<Run>,
}

#[derive(Clone, Default)]
pub(crate) struct DocumentLayout {
    pub size: Vec2,
    pub lines: Vec<LineLayout>,
    pub positions: Vec<Option<BytePosition>>,
    pub widgets: Vec<WidgetLayout>,
}

pub(crate) fn style_size(style: SynHlStyle, body_size: f32) -> f32 {
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

pub(crate) fn style_font(style: SynHlStyle, body_size: f32) -> FontId {
    let size = style_size(style, body_size);
    let font = match style.family {
        SynHlFontFamily::Monospace => FontId::monospace(size),
        SynHlFontFamily::Proportional => FontId::proportional(size),
    };
    font.bold(style.bold).italic(style.italic)
}

fn galley(text: &str, font: FontId) -> Option<Galley> {
    layout_text(text, font, TextLayout::DEFAULT)
}

fn body_metrics(body_size: f32) -> Option<(f32, f32)> {
    let empty = galley("", FontId::proportional(body_size))?;
    Some((empty.baseline(), empty.line_height()))
}

struct LineSource<'a> {
    bytes: &'a [u8],
    highlight: &'a SyntaxHighlight,
    widgets: &'a [&'a TextWidget],
    checkboxes: &'a [&'a Range<usize>],
    end: usize,
    has_newline: bool,
    trailing_from: usize,
    mask: bool,
    body_size: f32,
    invisibles: bool,
}

impl LineSource<'_> {
    fn style_at(&self, index: usize) -> SynHlStyle {
        let mut style = self.highlight.style_at(index);
        if self
            .widgets
            .iter()
            .any(|widget| widget.italic && widget.range.contains(&index))
        {
            style.italic = true;
        }
        style
    }

    fn text_run(&self, style: SynHlStyle, text: &str, range: Range<usize>) -> Option<Run> {
        let mapped = text.len() == range.len();
        let galley = galley(text, style_font(style, self.body_size))?;
        let width = galley.size().x;
        Some(Run {
            range,
            x: 0.0,
            width,
            galley: Some(galley),
            style,
            font_size: style_size(style, self.body_size),
            invisible: false,
            show_when_trailing: false,
            mapped,
        })
    }

    fn build(&self, from: usize) -> Option<Vec<Run>> {
        let mut runs: Vec<Run> = Vec::new();
        let mut index = from;
        while index < self.end {
            if let Some(checkbox) = self
                .checkboxes
                .iter()
                .find(|checkbox| checkbox.start == index)
            {
                runs.push(Run {
                    range: (*checkbox).clone(),
                    x: 0.0,
                    width: CHECKBOX_WIDTH,
                    galley: None,
                    style: self.style_at(index),
                    font_size: self.body_size,
                    invisible: false,
                    show_when_trailing: false,
                    mapped: false,
                });
                index = checkbox.end;
                continue;
            }
            if let Some(widget) = self
                .widgets
                .iter()
                .find(|widget| widget.range.start == index)
            {
                let style = self.style_at(index);
                let label = match widget.icon {
                    Some(_) => format!("   {} ", widget.label),
                    None => format!(" {} ", widget.label),
                };
                runs.push(self.text_run(style, &label, widget.range.clone())?);
                index = widget.range.end;
                continue;
            }
            let style = self.style_at(index);
            if self.mask {
                let len = next_character(&self.bytes[index..]).map_or(1, |(_, len)| len);
                let mut run = self.text_run(style, MASK, index..index + len)?;
                run.mapped = false;
                runs.push(run);
                index += len;
                continue;
            }
            let byte = self.bytes[index];
            if let Some(marker) = self.invisible_marker(byte) {
                let mut run = self.text_run(style, marker, index..index + 1)?;
                run.invisible = true;
                run.show_when_trailing = index >= self.trailing_from;
                run.mapped = false;
                runs.push(run);
                index += 1;
                continue;
            }
            let Some((character, len)) = next_character(&self.bytes[index..]) else {
                let mut run = self.text_run(style, "\u{fffd}", index..index + 1)?;
                run.mapped = false;
                runs.push(run);
                index += 1;
                continue;
            };
            let mut end = index + len;
            let mut text = String::new();
            text.push(character);
            while end < self.end {
                if self.checkboxes.iter().any(|checkbox| checkbox.start == end)
                    || self.widgets.iter().any(|widget| widget.range.start == end)
                    || self.style_at(end) != style
                    || self.invisible_marker(self.bytes[end]).is_some()
                {
                    break;
                }
                let Some((next, len)) = next_character(&self.bytes[end..]) else {
                    break;
                };
                text.push(next);
                end += len;
            }
            runs.push(self.text_run(style, &text, index..end)?);
            index = end;
        }
        if self.has_newline && self.invisibles {
            let style = self.style_at(self.end);
            let mut run = self.text_run(style, "\u{23ce}", self.end..self.end + 1)?;
            run.invisible = true;
            run.mapped = false;
            runs.push(run);
        }
        Some(place(runs))
    }
}

pub(crate) fn mask(text: &str) -> String {
    MASK.repeat(text.chars().count())
}

fn place(mut runs: Vec<Run>) -> Vec<Run> {
    let mut x = 0.0;
    for run in &mut runs {
        run.x = x;
        x += run.width;
    }
    runs
}

impl LineSource<'_> {
    fn invisible_marker(&self, byte: u8) -> Option<&'static str> {
        match self.invisibles {
            true => invisible_marker(byte),
            false => None,
        }
    }
}

fn invisible_marker(byte: u8) -> Option<&'static str> {
    match byte {
        b' ' => Some("\u{00b7}"),
        b'\t' => Some("\u{21e5}"),
        b'\r' => Some("\u{240d}"),
        _ => None,
    }
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

fn run_positions(run: &Run, into: &mut Vec<(usize, f32)>) {
    if run.mapped
        && let Some(galley) = &run.galley
    {
        let mut byte = run.range.start;
        while byte <= run.range.end {
            into.push((
                byte,
                run.x + galley.cursor_pos(Pos2::ZERO, byte - run.range.start).x,
            ));
            byte += 1;
        }
        return;
    }
    let count = run.range.len().max(1);
    for offset in 0..=count {
        let amount = offset as f32 / count as f32;
        into.push((run.range.start + offset, run.x + run.width * amount));
    }
}

fn positions_of(runs: &[Run]) -> Vec<(usize, f32)> {
    let mut positions = Vec::new();
    for run in runs {
        run_positions(run, &mut positions);
    }
    positions
}

fn runs_width(runs: &[Run]) -> f32 {
    runs.last().map_or(0.0, |run| run.x + run.width)
}

fn truncate(source: &LineSource, runs: Vec<Run>, breakpoint: usize) -> Option<Vec<Run>> {
    let mut kept = Vec::new();
    for run in runs {
        if run.range.end <= breakpoint {
            kept.push(run);
            continue;
        }
        if run.range.start >= breakpoint || !run.mapped {
            break;
        }
        let text = std::str::from_utf8(&source.bytes[run.range.start..breakpoint]).ok()?;
        kept.push(source.text_run(run.style, text, run.range.start..breakpoint)?);
        break;
    }
    Some(place(kept))
}

fn good_wrap_breakpoint(bytes: &[u8], breakpoint: usize) -> bool {
    bytes
        .get(breakpoint.saturating_sub(1))
        .is_some_and(|byte| byte.is_ascii_whitespace() || matches!(*byte, b'-' | b'/' | b'\\'))
}

fn utf8_boundary(bytes: &[u8], byte: usize) -> bool {
    byte == 0 || byte == bytes.len() || bytes[byte] & 0b1100_0000 != 0b1000_0000
}

fn choose_breakpoint(
    source: &LineSource,
    from: usize,
    positions: &[(usize, f32)],
    wrap_width: f32,
) -> Option<usize> {
    let mut candidates = positions
        .iter()
        .copied()
        .filter(|(byte, x)| {
            *byte > from
                && *byte < source.end
                && *x <= wrap_width
                && utf8_boundary(source.bytes, *byte)
                && !source
                    .widgets
                    .iter()
                    .any(|widget| widget.range.start < *byte && *byte < widget.range.end)
                && !source
                    .checkboxes
                    .iter()
                    .any(|checkbox| checkbox.start < *byte && *byte < checkbox.end)
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable_by_key(|(byte, _)| *byte);
    let good = candidates
        .iter()
        .rev()
        .find(|(byte, _)| good_wrap_breakpoint(source.bytes, *byte))
        .copied();
    let fallback = candidates
        .last()
        .copied()
        .filter(|(_, x)| wrap_width - *x < wrap_width * WRAP_FALLBACK_REMAINING_WIDTH);
    good.or(fallback).map(|(byte, _)| byte)
}

fn wrap(
    source: &LineSource,
    start: usize,
    wrap_width: f32,
) -> Option<Vec<(Range<usize>, Vec<Run>)>> {
    let mut wrapped = Vec::new();
    let mut from = start;
    loop {
        let runs = source.build(from)?;
        if runs_width(&runs) <= wrap_width {
            wrapped.push((from..source.end, runs));
            break;
        }
        let positions = positions_of(&runs);
        let Some(breakpoint) = choose_breakpoint(source, from, &positions, wrap_width) else {
            wrapped.push((from..source.end, runs));
            break;
        };
        wrapped.push((from..breakpoint, truncate(source, runs, breakpoint)?));
        from = breakpoint;
    }
    Some(wrapped)
}

fn line_metrics(runs: &[Run], body: (f32, f32), padding: (f32, f32)) -> (f32, f32) {
    let mut baseline = body.0;
    let mut height = body.1;
    for run in runs {
        let Some(galley) = &run.galley else {
            continue;
        };
        baseline = baseline.max(galley.baseline());
        height = height.max(galley.line_height());
    }
    (baseline + padding.0, height + padding.0 + padding.1)
}

pub(crate) fn layout_document(
    bytes: &[u8],
    highlight: &SyntaxHighlight,
    widgets: &[TextWidget],
    checkboxes: &[Range<usize>],
    hidden: &[Range<usize>],
    options: &LayoutOptions,
) -> Option<DocumentLayout> {
    let wrap_width = options.wrap_width;
    let body = body_metrics(options.body_size)?;
    let mut lines: Vec<LineLayout> = Vec::new();
    let mut widget_layouts = Vec::new();
    let mut positions: Vec<Option<BytePosition>> = vec![None; bytes.len() + 1];
    let mut start = 0;
    let mut document_line = 0;
    let mut y = 0.0;

    loop {
        let newline = bytes[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| start + offset);
        let end = newline.unwrap_or(bytes.len());
        if hidden.iter().any(|range| range.contains(&start)) {
            let Some(newline) = newline else {
                break;
            };
            start = newline + 1;
            document_line += 1;
            continue;
        }
        let line_widgets = widgets
            .iter()
            .enumerate()
            .filter(|(_, widget)| widget.range.start >= start && widget.range.end <= end)
            .collect::<Vec<_>>();
        let line_sources = line_widgets
            .iter()
            .map(|(_, widget)| *widget)
            .collect::<Vec<_>>();
        let line_checkboxes = checkboxes
            .iter()
            .filter(|checkbox| checkbox.start >= start && checkbox.end <= end)
            .collect::<Vec<_>>();
        let trailing_from = bytes[start..end]
            .iter()
            .rposition(|byte| !matches!(*byte, b' ' | b'\t' | b'\r'))
            .map_or(start, |index| start + index + 1);
        let source = LineSource {
            bytes,
            highlight,
            widgets: &line_sources,
            checkboxes: &line_checkboxes,
            end,
            has_newline: newline.is_some(),
            trailing_from,
            mask: options.mask,
            body_size: options.body_size,
            invisibles: !options.single_line,
        };
        for (range, runs) in wrap(&source, start, wrap_width)? {
            let line_index = lines.len();
            let width = runs_width(&runs);
            let (baseline, height) = line_metrics(&runs, body, options.line_padding());
            for (byte, x) in positions_of(&runs) {
                if let Some(position) = positions.get_mut(byte) {
                    position.get_or_insert(BytePosition {
                        line: line_index,
                        x,
                    });
                }
            }
            positions[range.start] = Some(BytePosition {
                line: line_index,
                x: 0.0,
            });
            positions[range.end].get_or_insert(BytePosition {
                line: line_index,
                x: width,
            });
            lines.push(LineLayout {
                start: range.start,
                end: range.end,
                y,
                width,
                height,
                baseline,
                document_line,
                show_line_number: range.start == start,
                runs,
            });
            y += height;
        }
        for (index, widget) in &line_widgets {
            let (Some(left), Some(right)) =
                (positions[widget.range.start], positions[widget.range.end])
            else {
                continue;
            };
            let Some(line) = lines.get(left.line) else {
                continue;
            };
            if right.line != left.line {
                continue;
            }
            widget_layouts.push(WidgetLayout {
                index: *index,
                range: widget.range.clone(),
                icon: widget.icon,
                broken: widget.broken,
                block: false,
                rect: Rect::from_min_max(
                    Pos2::new(left.x, line.y + (line.height - INLINE_WIDGET_HEIGHT) * 0.5),
                    Pos2::new(
                        right.x.max(left.x + 1.0),
                        line.y + (line.height + INLINE_WIDGET_HEIGHT) * 0.5,
                    ),
                ),
            });
        }
        if let Some((index, widget)) = line_widgets.iter().find(|(_, widget)| widget.block())
            && let Some(size) = widget.block_size
        {
            widget_layouts.push(WidgetLayout {
                index: *index,
                range: widget.range.clone(),
                icon: widget.icon,
                broken: widget.broken,
                block: true,
                rect: Rect::from_min_size(Pos2::new(0.0, y), size),
            });
            y += size.y;
        }

        let Some(newline) = newline else {
            break;
        };
        start = newline + 1;
        document_line += 1;
    }

    align_markdown_tables(&mut lines, &mut positions, highlight.markdown_tables());
    let width = lines
        .iter()
        .map(|line| line.width)
        .chain(widget_layouts.iter().map(|widget| widget.rect.max.x))
        .fold(0.0_f32, f32::max);
    Some(DocumentLayout {
        size: Vec2::new(width, y) + options.document_padding(),
        lines,
        positions,
        widgets: widget_layouts,
    })
}

struct LayoutSegment {
    position_range: Range<usize>,
    run_range: Range<usize>,
    offset: f32,
}

fn byte_x(positions: &[Option<BytePosition>], byte: usize, line: usize) -> Option<f32> {
    positions
        .get(byte)
        .and_then(|position| *position)
        .filter(|position| position.line == line)
        .map(|position| position.x)
}

fn align_markdown_tables(
    lines: &mut [LineLayout],
    positions: &mut [Option<BytePosition>],
    tables: &[MarkdownTable],
) {
    for table in tables {
        let rows = table
            .rows
            .iter()
            .filter_map(|row| {
                let line_index = positions
                    .get(row.range.start)
                    .and_then(|position| *position)?
                    .line;
                let line = lines.get(line_index)?;
                let cells = row
                    .cells
                    .iter()
                    .filter_map(|cell| {
                        let start = byte_x(positions, cell.start, line_index)?;
                        let end = byte_x(positions, cell.end, line_index)?;
                        Some((cell.clone(), start, end))
                    })
                    .collect::<Vec<_>>();
                (!cells.is_empty()).then_some((line_index, line.start, line.end, line.width, cells))
            })
            .collect::<Vec<_>>();
        if rows.is_empty() {
            continue;
        }

        let column_count = rows
            .iter()
            .map(|(_, _, _, _, cells)| cells.len())
            .max()
            .unwrap_or(0);
        let mut column_widths = vec![0.0_f32; column_count];
        let mut gap_widths = vec![0.0_f32; column_count.saturating_sub(1)];
        let mut prefix_width = 0.0_f32;
        for (_, _, _, _, cells) in &rows {
            prefix_width = prefix_width.max(cells[0].1);
            for (column, (_, start, end)) in cells.iter().enumerate() {
                column_widths[column] = column_widths[column].max((end - start).max(0.0));
                if let Some((_, next_start, _)) = cells.get(column + 1) {
                    gap_widths[column] = gap_widths[column].max((next_start - end).max(0.0));
                }
            }
        }

        for (line_index, line_start, line_end, natural_line_width, cells) in rows {
            let mut segments = Vec::new();
            let first_start = cells[0].0.start;
            segments.push(LayoutSegment {
                position_range: line_start..first_start,
                run_range: line_start..first_start,
                offset: prefix_width - cells[0].1,
            });

            let mut target_x = prefix_width;
            for (column, (cell, natural_start, natural_end)) in cells.iter().enumerate() {
                let cell_width = (natural_end - natural_start).max(0.0);
                let spare = (column_widths[column] - cell_width).max(0.0);
                let alignment = table
                    .alignments
                    .get(column)
                    .copied()
                    .unwrap_or(MarkdownTableAlignment::Left);
                let leading = match alignment {
                    MarkdownTableAlignment::Left => 0.0,
                    MarkdownTableAlignment::Center => spare / 2.0,
                    MarkdownTableAlignment::Right => spare,
                };
                segments.push(LayoutSegment {
                    position_range: cell.start..cell.end.saturating_add(1),
                    run_range: cell.clone(),
                    offset: target_x + leading - natural_start,
                });
                target_x += column_widths[column];

                if let Some((next, next_start, _)) = cells.get(column + 1) {
                    segments.push(LayoutSegment {
                        position_range: cell.end.saturating_add(1)..next.start,
                        run_range: cell.end..next.start,
                        offset: target_x - natural_end,
                    });
                    let natural_gap = (next_start - natural_end).max(0.0);
                    target_x += gap_widths[column].max(natural_gap);
                }
            }

            let (_, _, last_natural_end) = cells.last().expect("table row has a cell");
            let last_end = cells.last().expect("table row has a cell").0.end;
            let suffix_offset = target_x - last_natural_end;
            segments.push(LayoutSegment {
                position_range: last_end.saturating_add(1)..line_end.saturating_add(1),
                run_range: last_end..line_end.saturating_add(1),
                offset: suffix_offset,
            });

            for segment in &segments {
                for position in positions
                    .get_mut(segment.position_range.clone())
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    if position.line == line_index {
                        position.x += segment.offset;
                    }
                }
            }
            let line = &mut lines[line_index];
            for run in &mut line.runs {
                if let Some(segment) = segments
                    .iter()
                    .find(|segment| segment.run_range.contains(&run.range.start))
                {
                    run.x += segment.offset;
                }
            }
            line.width = natural_line_width + suffix_offset;
        }
    }
}

pub(crate) fn hit_test(layout: &DocumentLayout, point: Vec2) -> usize {
    if layout.lines.is_empty() {
        return 0;
    }
    let line = layout
        .lines
        .iter()
        .position(|line| point.y < line.y + line.height)
        .unwrap_or(layout.lines.len() - 1);
    let line_layout = &layout.lines[line];
    let inline_widgets = layout
        .widgets
        .iter()
        .filter(|widget| !widget.block && widget.rect.center().y >= line_layout.y)
        .filter(|widget| widget.rect.center().y < line_layout.y + line_layout.height)
        .collect::<Vec<_>>();
    (line_layout.start..=line_layout.end)
        .filter(|byte| {
            !inline_widgets
                .iter()
                .any(|widget| *byte > widget.range.start && *byte < widget.range.end)
        })
        .filter_map(|byte| {
            layout
                .positions
                .get(byte)
                .copied()
                .flatten()
                .filter(|position| position.line == line)
                .map(|position| (byte, (position.x - point.x).abs()))
        })
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map_or(line_layout.start, |(byte, _)| byte)
}

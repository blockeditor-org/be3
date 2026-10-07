use std::f32::consts::{FRAC_PI_4, SQRT_2};
use std::ops::Range;

use crate::color::Color32;
use crate::font::{FontId, Galley, TextAlign};
use crate::geometry::{Rect, Vec2, pos2, vec2};
use crate::painter::Corners;

const WRAP_FALLBACK_REMAINING_WIDTH: f32 = 0.15;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SpanStyle {
    pub font: FontId,
    pub color: Color32,
    pub underline: bool,
    pub strikethrough: bool,
}

impl SpanStyle {
    pub fn new(font: FontId, color: Color32) -> Self {
        Self {
            font,
            color,
            underline: false,
            strikethrough: false,
        }
    }
}

pub const OBJECT: &str = "\u{fffc}";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Piece<'a> {
    Text {
        text: &'a str,
        style: SpanStyle,
        break_after: bool,
    },
    Inline {
        index: usize,
        size: Vec2,
    },
}

impl<'a> Piece<'a> {
    pub fn text(text: &'a str, style: SpanStyle) -> Self {
        Piece::Text {
            text,
            style,
            break_after: false,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Piece::Text { text, .. } => text.len(),
            Piece::Inline { .. } => OBJECT.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SpanKind {
    Text,
    Inline(usize, Vec2),
}

#[derive(Clone, PartialEq, Debug)]
struct TextSpan {
    piece: usize,
    range: Range<usize>,
    style: SpanStyle,
    kind: SpanKind,
    break_after: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub struct TextMark {
    pub range: Range<usize>,
    pub color: Color32,
    pub radius: f32,
    pub outset: Vec2,
}

impl TextMark {
    pub fn new(range: Range<usize>, color: Color32) -> Self {
        Self {
            range,
            color,
            radius: 0.0,
            outset: Vec2::ZERO,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CaretHandle {
    Start,
    End,
    Middle,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextCaret {
    pub at: usize,
    pub color: Color32,
    pub width: f32,
    pub blink: bool,
    pub flag: bool,
    pub handle: Option<CaretHandle>,
}

impl TextCaret {
    pub fn new(at: usize, color: Color32, width: f32) -> Self {
        Self {
            at,
            color,
            width,
            blink: false,
            flag: false,
            handle: None,
        }
    }
}

pub const HANDLE_RADIUS: f32 = 9.0;
pub const HANDLE_GAP: f32 = 4.0;

pub fn handle_center(caret: Rect, handle: CaretHandle) -> Vec2 {
    let anchor = vec2(caret.min.x, caret.max.y + HANDLE_GAP);
    match handle {
        CaretHandle::Start => anchor + vec2(-HANDLE_RADIUS, HANDLE_RADIUS),
        CaretHandle::End => anchor + vec2(HANDLE_RADIUS, HANDLE_RADIUS),
        CaretHandle::Middle => anchor + vec2(caret.width() / 2.0, HANDLE_RADIUS * SQRT_2),
    }
}

pub fn handle_shape(caret: Rect, handle: CaretHandle) -> (Rect, Corners, f32) {
    let center = handle_center(caret, handle);
    let rect = Rect::from_center_size(pos2(center.x, center.y), Vec2::splat(HANDLE_RADIUS * 2.0));
    let round = Corners::all(HANDLE_RADIUS);
    match handle {
        CaretHandle::Start => (
            rect,
            Corners {
                top_right: 0.0,
                ..round
            },
            0.0,
        ),
        CaretHandle::End => (
            rect,
            Corners {
                top_left: 0.0,
                ..round
            },
            0.0,
        ),
        CaretHandle::Middle => (
            rect,
            Corners {
                top_left: 0.0,
                ..round
            },
            FRAC_PI_4,
        ),
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RichOptions {
    pub wrap_width: f32,
    pub padding: (f32, f32),
    pub style: SpanStyle,
}

#[derive(Clone)]
pub struct RichRun {
    pub piece: usize,
    pub range: Range<usize>,
    pub style: SpanStyle,
    pub kind: SpanKind,
    pub x: f32,
    pub width: f32,
    pub height: f32,
    pub galley: Option<Galley>,
}

impl RichRun {
    fn text(&self) -> bool {
        self.galley.is_some()
    }

    fn x_of(&self, index: usize) -> f32 {
        match &self.galley {
            Some(galley) if index > self.range.start => {
                self.x
                    + galley
                        .cursor_pos(pos2(0.0, 0.0), index - self.range.start)
                        .x
            }
            _ if index > self.range.start => self.x + self.width,
            _ => self.x,
        }
    }

    fn stops(&self, into: &mut Vec<(usize, f32)>) {
        into.push((self.range.start, self.x));
        if let Some(galley) = &self.galley {
            for line in galley.lines() {
                for (index, x) in &line.cursors {
                    into.push((self.range.start + index, self.x + x));
                }
            }
        }
        into.push((self.range.end, self.x + self.width));
    }
}

#[derive(Clone)]
pub struct RichLine {
    pub range: Range<usize>,
    pub top: f32,
    pub left: f32,
    pub height: f32,
    pub baseline: f32,
    pub width: f32,
    pub runs: Vec<RichRun>,
}

impl RichLine {
    pub fn x_of(&self, index: usize) -> f32 {
        if index >= self.range.end {
            return self.left + self.width;
        }
        self.left
            + self
                .runs
                .iter()
                .find(|run| run.range.contains(&index))
                .map_or(0.0, |run| run.x_of(index))
    }

    pub fn run_left(&self, run: &RichRun) -> f32 {
        self.left + run.x
    }

    pub fn run_top(&self, run: &RichRun) -> f32 {
        match &run.galley {
            Some(galley) => self.top + self.baseline - galley.baseline(),
            None => self.top + (self.height - run.height) / 2.0,
        }
    }
}

#[derive(Clone, Default)]
pub struct RichLayout {
    pub lines: Vec<RichLine>,
    pub size: Vec2,
    broken: bool,
}

pub trait Shaper {
    fn galley(&mut self, text: &str, font: FontId) -> Galley;
}

impl<F: FnMut(&str, FontId) -> Galley> Shaper for F {
    fn galley(&mut self, text: &str, font: FontId) -> Galley {
        self(text, font)
    }
}

struct Builder<'a> {
    text: &'a str,
    spans: &'a [TextSpan],
    shaper: &'a mut dyn Shaper,
}

impl Builder<'_> {
    fn runs(&mut self, from: usize, to: usize) -> Vec<RichRun> {
        let mut runs = Vec::new();
        let mut x = 0.0;
        let first = self
            .spans
            .partition_point(|span| span.range.end <= from)
            .min(self.spans.len());
        for index in first..self.spans.len() {
            let span = &self.spans[index];
            if span.range.start >= to {
                break;
            }
            let start = span.range.start.max(from);
            let end = span.range.end.min(to);
            if start >= end {
                continue;
            }
            let run = self.run(index, start..end, x);
            x += run.width;
            runs.push(run);
        }
        runs
    }

    fn run(&mut self, span: usize, range: Range<usize>, x: f32) -> RichRun {
        let TextSpan {
            piece, style, kind, ..
        } = self.spans[span].clone();
        match kind {
            SpanKind::Text => {
                let text = self.text.get(range.clone()).unwrap_or("");
                let galley = self.shaper.galley(text, style.font);
                RichRun {
                    piece,
                    range,
                    style,
                    kind,
                    x,
                    width: galley.size().x,
                    height: galley.line_height(),
                    galley: Some(galley),
                }
            }
            SpanKind::Inline(_, size) => RichRun {
                piece,
                range,
                style,
                kind,
                x,
                width: size.x,
                height: size.y,
                galley: None,
            },
        }
    }

    fn stops(&self, runs: &[RichRun]) -> Vec<(usize, f32)> {
        let mut stops = Vec::new();
        for run in runs {
            match run.text() {
                true => run.stops(&mut stops),
                false => {
                    stops.push((run.range.start, run.x));
                    stops.push((run.range.end, run.x + run.width));
                }
            }
        }
        stops.retain(|(byte, _)| self.text.is_char_boundary(*byte));
        stops.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        stops
    }

    fn breakpoint(
        &self,
        stops: &[(usize, f32)],
        from: usize,
        to: usize,
        origin: f32,
        wrap: f32,
    ) -> Option<usize> {
        let bytes = self.text.as_bytes();
        let first = stops.partition_point(|(byte, _)| *byte <= from);
        let candidates: Vec<(usize, f32)> = stops[first..]
            .iter()
            .take_while(|(byte, _)| *byte < to)
            .map(|(byte, x)| (*byte, x - origin))
            .filter(|(_, x)| *x <= wrap)
            .collect();
        let good = candidates
            .iter()
            .rev()
            .find(|(byte, _)| {
                self.breaks_after(*byte)
                    || bytes.get(byte - 1).is_some_and(|before| {
                        before.is_ascii_whitespace() || matches!(*before, b'-' | b'/' | b'\\')
                    })
            })
            .copied();
        let fallback = candidates
            .last()
            .copied()
            .filter(|(_, x)| wrap - *x < wrap * WRAP_FALLBACK_REMAINING_WIDTH);
        good.or(fallback).map(|(byte, _)| byte)
    }

    fn breaks_after(&self, byte: usize) -> bool {
        let index = self.spans.partition_point(|span| span.range.end < byte);
        self.spans[index..]
            .iter()
            .take_while(|span| span.range.end == byte)
            .any(|span| span.break_after)
    }
}

fn spans_of(pieces: &[Piece], fallback: SpanStyle) -> (String, Vec<TextSpan>) {
    let mut text = String::with_capacity(pieces.iter().map(Piece::len).sum());
    let mut spans = Vec::with_capacity(pieces.len());
    for (index, piece) in pieces.iter().enumerate() {
        let start = text.len();
        let (style, kind, break_after) = match *piece {
            Piece::Text {
                text: piece,
                style,
                break_after,
            } => {
                text.push_str(piece);
                (style, SpanKind::Text, break_after)
            }
            Piece::Inline {
                index: inline,
                size,
            } => {
                text.push_str(OBJECT);
                (fallback, SpanKind::Inline(inline, size), false)
            }
        };
        if text.len() > start {
            spans.push(TextSpan {
                piece: index,
                range: start..text.len(),
                style,
                kind,
                break_after,
            });
        }
    }
    (text, spans)
}

impl RichLayout {
    pub fn new(pieces: &[Piece], options: RichOptions, shaper: &mut dyn Shaper) -> Self {
        Self::build(pieces, options, shaper, None)
    }

    pub fn resume(
        &self,
        changed: usize,
        pieces: &[Piece],
        options: RichOptions,
        shaper: &mut dyn Shaper,
    ) -> Self {
        Self::build(pieces, options, shaper, Some((self, changed)))
    }

    fn build(
        pieces: &[Piece],
        options: RichOptions,
        shaper: &mut dyn Shaper,
        previous: Option<(&RichLayout, usize)>,
    ) -> Self {
        let (text, spans) = spans_of(pieces, options.style);
        let text = text.as_str();
        let strut = shaper.galley("", options.style.font);
        let strut = (strut.baseline(), strut.line_height());
        let mut builder = Builder {
            text,
            spans: &spans,
            shaper,
        };
        let kept = previous.map_or(0, |(previous, changed)| {
            previous.line_of(changed).saturating_sub(1)
        });
        let mut lines: Vec<RichLine> = match previous {
            Some((previous, _)) => previous.lines[..kept].to_vec(),
            None => Vec::new(),
        };
        let mut broken = previous.is_some_and(|(previous, _)| previous.broken) && kept > 0;
        let (mut top, mut resume) =
            match previous.and_then(|(previous, _)| previous.lines.get(kept)) {
                Some(line) if kept > 0 => (line.top, Some(line.range.start.min(text.len()))),
                _ => (0.0, None),
            };
        let mut start = resume.map_or(0, |at| text[..at].rfind('\n').map_or(0, |found| found + 1));
        loop {
            let end = text[start..].find('\n').map_or(text.len(), |at| start + at);
            let mut from = resume.take().unwrap_or(start);
            let whole = builder.runs(start, end);
            let total = whole.last().map_or(0.0, |run| run.x + run.width);
            let stops = match total > options.wrap_width || from != start {
                true => builder.stops(&whole),
                false => Vec::new(),
            };
            let mut whole = Some(whole);
            loop {
                let origin = match from == start {
                    true => 0.0,
                    false => stops
                        .get(stops.partition_point(|(byte, _)| *byte < from))
                        .map_or(0.0, |(_, x)| *x),
                };
                let broke = match total - origin > options.wrap_width {
                    true => builder.breakpoint(&stops, from, end, origin, options.wrap_width),
                    false => None,
                };
                let (to, runs) = match (broke, from == start) {
                    (Some(to), _) => (to, builder.runs(from, to)),
                    (None, true) => (end, whole.take().unwrap_or_default()),
                    (None, false) => (end, builder.runs(from, end)),
                };
                let line = line(from..to, runs, top, strut, options.padding);
                top += line.height;
                lines.push(line);
                if broke.is_none() {
                    break;
                }
                broken = true;
                from = to;
            }
            if end == text.len() {
                break;
            }
            start = end + 1;
        }
        let width = lines.iter().map(|line| line.width).fold(0.0, f32::max);
        Self {
            lines,
            size: vec2(width, top),
            broken,
        }
    }

    pub fn fits(&self, wrap_width: f32, laid_at: f32) -> bool {
        wrap_width >= self.size.x && (wrap_width <= laid_at || !self.broken)
    }

    pub fn truncate(&mut self, width: f32, shaper: &mut dyn Shaper) {
        for line in &mut self.lines {
            if line.width <= width {
                continue;
            }
            let Some(style) = line.runs.last().map(|run| run.style) else {
                continue;
            };
            let ellipsis = shaper.galley(ELLIPSIS, style.font);
            let budget = width - ellipsis.size().x;
            let mut runs = Vec::with_capacity(line.runs.len() + 1);
            let mut cut = line.range.start;
            let mut style = style;
            let mut piece = line.runs.last().map_or(0, |run| run.piece);
            for run in &line.runs {
                if run.x + run.width <= budget {
                    cut = run.range.end;
                    style = run.style;
                    piece = run.piece;
                    runs.push(run.clone());
                    continue;
                }
                if let Some(galley) = &run.galley {
                    let kept = galley.lines().first().map_or(0, |first| {
                        first
                            .cursors
                            .iter()
                            .filter(|(_, x)| run.x + x <= budget)
                            .map(|(at, _)| *at)
                            .max()
                            .unwrap_or(0)
                    });
                    let kept = galley.text().get(..kept).unwrap_or_default().trim_end();
                    if !kept.is_empty() {
                        let shaped = shaper.galley(kept, run.style.font);
                        cut = run.range.start + kept.len();
                        style = run.style;
                        piece = run.piece;
                        runs.push(RichRun {
                            range: run.range.start..cut,
                            width: shaped.size().x,
                            galley: Some(shaped),
                            ..run.clone()
                        });
                    }
                }
                break;
            }
            let joined = match runs.last() {
                Some(RichRun {
                    galley: Some(last), ..
                }) => Some(shaper.galley(&format!("{}{ELLIPSIS}", last.text()), style.font)),
                _ => None,
            };
            match (joined, runs.last_mut()) {
                (Some(joined), Some(last)) => {
                    last.range.end = line.range.end;
                    last.width = joined.size().x;
                    last.galley = Some(joined);
                }
                _ => {
                    let x = runs.last().map_or(0.0, |run: &RichRun| run.x + run.width);
                    runs.push(RichRun {
                        piece,
                        range: cut..line.range.end,
                        style,
                        kind: SpanKind::Text,
                        x,
                        width: ellipsis.size().x,
                        height: ellipsis.line_height(),
                        galley: Some(ellipsis),
                    });
                }
            }
            line.width = runs.last().map_or(0.0, |run| run.x + run.width);
            line.runs = runs;
        }
        self.size.x = self.lines.iter().map(|line| line.width).fold(0.0, f32::max);
    }

    pub fn align(&mut self, width: f32, align: TextAlign, snap: impl Fn(f32) -> f32) {
        for line in &mut self.lines {
            line.left = match align {
                TextAlign::Start => 0.0,
                TextAlign::Center => snap((width - line.width) / 2.0),
                TextAlign::End => snap(width - line.width),
            };
        }
    }

    pub fn line_of(&self, index: usize) -> usize {
        self.lines
            .partition_point(|line| line.range.start <= index)
            .saturating_sub(1)
    }

    pub fn caret_rect(&self, index: usize, width: f32) -> Rect {
        let Some(line) = self.lines.get(self.line_of(index)) else {
            return Rect::ZERO;
        };
        Rect::from_min_size(pos2(line.x_of(index), line.top), vec2(width, line.height))
    }

    pub fn index_at(&self, point: crate::geometry::Pos2) -> usize {
        let Some(line) = self
            .lines
            .iter()
            .find(|line| point.y < line.top + line.height)
            .or(self.lines.last())
        else {
            return 0;
        };
        let mut stops = vec![(line.range.start, 0.0)];
        for run in &line.runs {
            match run.text() {
                true => run.stops(&mut stops),
                false => {
                    stops.push((run.range.start, run.x));
                    stops.push((run.range.end, run.x + run.width));
                }
            }
        }
        let x = point.x - line.left;
        stops
            .into_iter()
            .filter(|(index, _)| line.range.contains(index) || *index == line.range.end)
            .min_by(|left, right| (left.1 - x).abs().total_cmp(&(right.1 - x).abs()))
            .map_or(line.range.start, |(index, _)| index)
    }

    pub fn selection_rects(&self, range: Range<usize>) -> Vec<Rect> {
        self.lines
            .iter()
            .filter_map(|line| {
                let start = range.start.max(line.range.start);
                let end = range.end.min(line.range.end);
                if start >= end {
                    return None;
                }
                let left = line.x_of(start);
                let right = line.x_of(end).max(left + 1.0);
                Some(Rect::from_min_max(
                    pos2(left, line.top),
                    pos2(right, line.top + line.height),
                ))
            })
            .collect()
    }

    pub fn inline_at(&self, point: crate::geometry::Pos2) -> Option<usize> {
        self.inline_rects()
            .into_iter()
            .find(|(_, rect)| rect.contains(point))
            .map(|(index, _)| index)
    }

    pub fn inline_rects(&self) -> Vec<(usize, Rect)> {
        let mut rects = Vec::new();
        for line in &self.lines {
            for run in &line.runs {
                if let SpanKind::Inline(child, _) = run.kind {
                    rects.push((
                        child,
                        Rect::from_min_size(
                            pos2(line.run_left(run), line.run_top(run)),
                            vec2(run.width, run.height),
                        ),
                    ));
                }
            }
        }
        rects
    }
}

const ELLIPSIS: &str = "\u{2026}";

fn line(
    range: Range<usize>,
    runs: Vec<RichRun>,
    top: f32,
    strut: (f32, f32),
    padding: (f32, f32),
) -> RichLine {
    let mut baseline = strut.0;
    let mut height = strut.1;
    for run in &runs {
        if let Some(galley) = &run.galley {
            baseline = baseline.max(galley.baseline());
        }
        height = height.max(run.height);
    }
    let width = runs.last().map_or(0.0, |run| run.x + run.width);
    RichLine {
        range,
        top,
        left: 0.0,
        height: height + padding.0 + padding.1,
        baseline: baseline + padding.0,
        width,
        runs,
    }
}

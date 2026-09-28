use std::ops::Range;

use crate::color::Color32;
use crate::font::{FontId, Galley};
use crate::geometry::{Rect, Vec2, pos2, vec2};

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

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SpanKind {
    Text,
    Space(f32),
    Inline(usize),
}

#[derive(Clone, PartialEq, Debug)]
pub struct TextSpan {
    pub range: Range<usize>,
    pub style: SpanStyle,
    pub kind: SpanKind,
    pub break_after: bool,
}

impl TextSpan {
    pub fn text(range: Range<usize>, style: SpanStyle) -> Self {
        Self {
            range,
            style,
            kind: SpanKind::Text,
            break_after: false,
        }
    }
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
        CaretHandle::Middle => anchor + vec2(0.0, HANDLE_RADIUS),
    }
}

pub fn handle_shapes(caret: Rect, handle: CaretHandle) -> [(Rect, f32); 2] {
    let anchor = vec2(caret.min.x, caret.max.y + HANDLE_GAP);
    let center = handle_center(caret, handle);
    let round = Rect::from_center_size(pos2(center.x, center.y), Vec2::splat(HANDLE_RADIUS * 2.0));
    let corner = match handle {
        CaretHandle::Start => pos2(anchor.x - HANDLE_RADIUS, anchor.y),
        CaretHandle::End => pos2(anchor.x, anchor.y),
        CaretHandle::Middle => pos2(anchor.x - HANDLE_RADIUS / 2.0, anchor.y),
    };
    [
        (round, HANDLE_RADIUS),
        (Rect::from_min_size(corner, Vec2::splat(HANDLE_RADIUS)), 0.0),
    ]
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RichOptions {
    pub wrap_width: f32,
    pub padding: (f32, f32),
    pub style: SpanStyle,
}

#[derive(Clone)]
pub struct RichRun {
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
    pub height: f32,
    pub baseline: f32,
    pub width: f32,
    pub runs: Vec<RichRun>,
}

impl RichLine {
    pub fn x_of(&self, index: usize) -> f32 {
        if index >= self.range.end {
            return self.width;
        }
        self.runs
            .iter()
            .find(|run| run.range.contains(&index))
            .map_or(0.0, |run| run.x_of(index))
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
    inline: &'a dyn Fn(usize) -> Vec2,
    shaper: &'a mut dyn Shaper,
}

impl Builder<'_> {
    fn runs(&mut self, from: usize, to: usize) -> Vec<RichRun> {
        let mut runs = Vec::new();
        let mut x = 0.0;
        for (index, span) in self.spans.iter().enumerate() {
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
        let TextSpan { style, kind, .. } = self.spans[span].clone();
        match kind {
            SpanKind::Text => {
                let text = self.text.get(range.clone()).unwrap_or("");
                let galley = self.shaper.galley(text, style.font);
                RichRun {
                    range,
                    style,
                    kind,
                    x,
                    width: galley.size().x,
                    height: galley.line_height(),
                    galley: Some(galley),
                }
            }
            SpanKind::Space(width) => RichRun {
                range,
                style,
                kind,
                x,
                width,
                height: 0.0,
                galley: None,
            },
            SpanKind::Inline(child) => {
                let size = (self.inline)(child);
                RichRun {
                    range,
                    style,
                    kind,
                    x,
                    width: size.x,
                    height: size.y,
                    galley: None,
                }
            }
        }
    }

    fn breakpoint(&self, runs: &[RichRun], from: usize, to: usize, wrap: f32) -> Option<usize> {
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
        let bytes = self.text.as_bytes();
        let mut candidates: Vec<(usize, f32)> = stops
            .into_iter()
            .filter(|(byte, x)| {
                *byte > from && *byte < to && *x <= wrap && self.text.is_char_boundary(*byte)
            })
            .collect();
        candidates.sort_by_key(|(byte, _)| *byte);
        let good = candidates
            .iter()
            .rev()
            .find(|(byte, _)| {
                self.spans
                    .iter()
                    .any(|span| span.break_after && span.range.end == *byte)
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
}

impl RichLayout {
    pub fn new(
        text: &str,
        spans: &[TextSpan],
        inline: &dyn Fn(usize) -> Vec2,
        options: RichOptions,
        shaper: &mut dyn Shaper,
    ) -> Self {
        let strut = shaper.galley("", options.style.font);
        let strut = (strut.baseline(), strut.line_height());
        let filled = fill(text.len(), spans, options.style);
        let mut builder = Builder {
            text,
            spans: &filled,
            inline,
            shaper,
        };
        let mut lines = Vec::new();
        let mut top = 0.0;
        let mut start = 0;
        loop {
            let end = text[start..].find('\n').map_or(text.len(), |at| start + at);
            let mut from = start;
            loop {
                let runs = builder.runs(from, end);
                let width = runs.last().map_or(0.0, |run| run.x + run.width);
                let broken = match width > options.wrap_width {
                    true => builder.breakpoint(&runs, from, end, options.wrap_width),
                    false => None,
                };
                let (to, runs) = match broken {
                    Some(to) => (to, builder.runs(from, to)),
                    None => (end, runs),
                };
                let line = line(from..to, runs, top, strut, options.padding);
                top += line.height;
                lines.push(line);
                if broken.is_none() {
                    break;
                }
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
        stops
            .into_iter()
            .filter(|(index, _)| line.range.contains(index) || *index == line.range.end)
            .min_by(|left, right| {
                (left.1 - point.x)
                    .abs()
                    .total_cmp(&(right.1 - point.x).abs())
            })
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
                if let SpanKind::Inline(child) = run.kind {
                    rects.push((
                        child,
                        Rect::from_min_size(
                            pos2(run.x, line.run_top(run)),
                            vec2(run.width, run.height),
                        ),
                    ));
                }
            }
        }
        rects
    }
}

fn fill(length: usize, spans: &[TextSpan], style: SpanStyle) -> Vec<TextSpan> {
    let mut filled = Vec::with_capacity(spans.len() + 1);
    let mut at = 0;
    let mut last = style;
    for span in spans {
        let start = span.range.start.min(length);
        if start > at {
            filled.push(TextSpan::text(at..start, last));
        }
        let end = span.range.end.clamp(start, length);
        if end > start.max(at) {
            filled.push(TextSpan {
                range: start.max(at)..end.max(at),
                ..span.clone()
            });
        }
        at = at.max(end);
        last = span.style;
    }
    if at < length || filled.is_empty() {
        filled.push(TextSpan::text(at..length, last));
    }
    filled
}

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
        height: height + padding.0 + padding.1,
        baseline: baseline + padding.0,
        width,
        runs,
    }
}

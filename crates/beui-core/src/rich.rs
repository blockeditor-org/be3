use std::f32::consts::{FRAC_PI_4, SQRT_2};
use std::ops::Range;

use crate::color::Color32;
use crate::font::{FontId, Galley, TextAlign, Wraps};
use crate::geometry::{Rect, Vec2, pos2, vec2};
use crate::painter::{Corners, Painter};

const SPAN_UNDERLINE_OFFSET: f32 = 0.12;
const SPAN_STRIKE_OFFSET: f32 = 0.32;
const SPAN_LINE_THICKNESS: f32 = 1.0 / 16.0;
pub const TAB_COLUMNS: f32 = 4.0;
const WRAP_SLACK: f32 = 1.0 / 1024.0;

pub use beui_types::text::{
    CaretHandle, SpanBackground, SpanStyle, TextCaret, TextGlyph, TextMark,
};

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
        text: &'a str,
    },
    Gap(f32),
    Break,
    Icon {
        glyph: &'a str,
        style: SpanStyle,
        spacing: f32,
        text_size: f32,
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
            Piece::Inline { text, .. } => text.len(),
            Piece::Gap(_) | Piece::Break | Piece::Icon { .. } => 0,
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
    Gap(f32),
    Break,
    Icon(f32),
    Tab,
}

impl SpanKind {
    fn spacing(self) -> f32 {
        match self {
            SpanKind::Icon(spacing) => spacing,
            _ => 0.0,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
struct TextSpan {
    piece: usize,
    range: Range<usize>,
    style: SpanStyle,
    kind: SpanKind,
    break_after: bool,
}

pub const HANDLE_RADIUS: f32 = 9.0;
pub const HANDLE_GAP: f32 = 4.0;
pub const TOOLBAR_GAP: f32 = 8.0;

pub fn toolbar_anchor(carets: Rect, scale: f32) -> Rect {
    Rect::from_min_max(
        pos2(carets.min.x, carets.min.y - TOOLBAR_GAP),
        pos2(
            carets.max.x,
            carets.max.y + (HANDLE_GAP + HANDLE_RADIUS * (1.0 + SQRT_2)) * scale + TOOLBAR_GAP,
        ),
    )
}

pub fn handle_center(caret: Rect, handle: CaretHandle, scale: f32) -> Vec2 {
    let (gap, radius) = (HANDLE_GAP * scale, HANDLE_RADIUS * scale);
    let anchor = vec2(caret.min.x, caret.max.y + gap);
    match handle {
        CaretHandle::Start => anchor + vec2(-radius, radius),
        CaretHandle::End => anchor + vec2(radius, radius),
        CaretHandle::Middle => anchor + vec2(caret.width() / 2.0, radius * SQRT_2),
    }
}

pub fn handle_shape(caret: Rect, handle: CaretHandle) -> (Rect, Corners, f32) {
    let center = handle_center(caret, handle, 1.0);
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
    pub hang: usize,
    pub anywhere: bool,
    pub least: bool,
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
    pub inner: Vec<usize>,
}

impl RichRun {
    fn text(&self) -> bool {
        self.galley.is_some() && self.kind == SpanKind::Text
    }

    fn box_height(&self) -> f32 {
        match &self.galley {
            Some(galley) => galley.line_height(),
            None => self.height,
        }
    }

    fn x_of(&self, index: usize) -> f32 {
        match &self.galley {
            Some(galley) if index > self.range.start && self.text() => {
                self.x
                    + galley
                        .cursor_pos(pos2(0.0, 0.0), index - self.range.start)
                        .x
            }
            _ if index > self.range.start => {
                let into = (index - self.range.start) as f32 / self.range.len().max(1) as f32;
                self.x + self.width * into.min(1.0)
            }
            _ => self.x,
        }
    }

    fn inner_stops(&self, into: &mut Vec<(usize, f32)>) {
        into.push((self.range.start, self.x));
        for index in &self.inner {
            into.push((*index, self.x_of(*index)));
        }
        into.push((self.range.end, self.x + self.width));
    }

    fn stops(&self, into: &mut Vec<(usize, f32)>) {
        into.push((self.range.start, self.x));
        if let Some(galley) = self.galley.as_ref().filter(|_| self.text()) {
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
    reach: f32,
    limit: f32,
    least: f32,
    pub top: f32,
    pub left: f32,
    pub height: f32,
    pub baseline: f32,
    pub middle: f32,
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

    pub fn x_after(&self, index: usize) -> f32 {
        self.runs
            .iter()
            .rev()
            .find(|run| run.range.start < index)
            .map_or_else(
                || self.x_of(index),
                |run| self.left + run.x_of(index.min(run.range.end)),
            )
    }

    fn ellipsize(&mut self, width: f32, shaper: &mut dyn Shaper) {
        let Some(style) = self
            .runs
            .iter()
            .rev()
            .find(|run| run.kind == SpanKind::Text)
            .or(self.runs.last())
            .map(|run| run.style)
        else {
            return;
        };
        let ellipsis = shaper.galley(ELLIPSIS, style.font);
        let budget = width - ellipsis.size().x;
        let mut runs = Vec::with_capacity(self.runs.len() + 1);
        let mut cut = self.range.start;
        let mut style = style;
        let mut piece = self.runs.last().map_or(0, |run| run.piece);
        for run in &self.runs {
            if run.x + run.width <= budget {
                cut = run.range.end;
                if run.kind == SpanKind::Text {
                    style = run.style;
                    piece = run.piece;
                }
                runs.push(run.clone());
                continue;
            }
            if let Some(galley) = run.galley.as_ref().filter(|_| run.text()) {
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
                galley: Some(last),
                kind: SpanKind::Text,
                ..
            }) => Some(shaper.galley(&format!("{}{ELLIPSIS}", last.text().trim_end()), style.font)),
            _ => None,
        };
        match (joined, runs.last_mut()) {
            (Some(joined), Some(last)) => {
                last.range.end = self.range.end;
                last.width = joined.size().x;
                last.galley = Some(joined);
            }
            _ => {
                let x = runs.last().map_or(0.0, |run: &RichRun| run.x + run.width);
                runs.push(RichRun {
                    piece,
                    range: cut..self.range.end,
                    style,
                    kind: SpanKind::Text,
                    x,
                    width: ellipsis.size().x,
                    height: ellipsis.line_height(),
                    galley: Some(ellipsis),
                    inner: Vec::new(),
                });
            }
        }
        self.width = runs.last().map_or(0.0, |run| run.x + run.width);
        self.runs = runs;
    }

    pub fn run_left(&self, run: &RichRun) -> f32 {
        self.left + run.x
    }

    pub fn run_top(&self, run: &RichRun) -> f32 {
        match &run.galley {
            Some(galley) if run.text() => self.top + self.baseline - galley.baseline(),
            _ => self.top + self.middle - run.box_height() / 2.0,
        }
    }
}

#[derive(Clone, Default)]
pub struct RichLayout {
    pub lines: Vec<RichLine>,
    pub size: Vec2,
    reach: f32,
    limit: f32,
}

pub trait Shaper {
    fn galley(&mut self, text: &str, font: FontId) -> Galley;

    fn slice(&mut self, galley: &Galley, range: Range<usize>, font: FontId) -> Galley {
        self.galley(galley.text().get(range).unwrap_or_default(), font)
    }
}

impl<F: FnMut(&str, FontId) -> Galley> Shaper for F {
    fn galley(&mut self, text: &str, font: FontId) -> Galley {
        self(text, font)
    }
}

struct Builder<'a> {
    text: &'a str,
    pieces: &'a [Piece<'a>],
    spans: &'a [TextSpan],
    shaper: &'a mut dyn Shaper,
    paragraph: Range<usize>,
    segment: Range<usize>,
    anywhere: bool,
    whole: Vec<Option<(Range<usize>, Galley)>>,
}

struct Segment {
    bytes: Range<usize>,
    spans: Range<usize>,
}

fn segments(text: &str, spans: &[TextSpan]) -> Vec<Segment> {
    let mut breaks = spans
        .iter()
        .enumerate()
        .filter(|(_, span)| span.kind == SpanKind::Break)
        .map(|(index, span)| (span.range.start, index))
        .peekable();
    let mut segments = Vec::new();
    let (mut start, mut lower) = (0, 0);
    let mut newline = text.find('\n');
    loop {
        let end = newline.unwrap_or(text.len());
        if let Some((byte, index)) = breaks.next_if(|(byte, _)| *byte <= end) {
            segments.push(Segment {
                bytes: start..byte,
                spans: lower..index,
            });
            (start, lower) = (byte, index + 1);
            continue;
        }
        segments.push(Segment {
            bytes: start..end,
            spans: lower..spans.len(),
        });
        let Some(at) = newline else {
            break;
        };
        start = at + 1;
        newline = text[start..].find('\n').map(|found| start + found);
    }
    segments
}

impl Builder<'_> {
    fn runs(&mut self, from: usize, to: usize) -> Vec<RichRun> {
        let mut runs: Vec<RichRun> = Vec::new();
        let mut x = 0.0;
        let spaced = |runs: &[RichRun], kind: SpanKind| {
            runs.last().map_or(0.0, |last: &RichRun| {
                last.kind.spacing().max(kind.spacing())
            })
        };
        let first = self
            .spans
            .partition_point(|span| {
                span.range.end < from || (span.range.end == from && !span.range.is_empty())
            })
            .max(self.segment.start)
            .min(self.spans.len());
        for index in first..self.segment.end {
            let span = &self.spans[index];
            if span.kind == SpanKind::Break {
                continue;
            }
            if span.range.is_empty() {
                let at = span.range.start;
                let last = at == to && to == self.paragraph.end;
                if at >= from && (at < to || last) {
                    x += spaced(&runs, span.kind);
                    let run = self.run(index, at..at, x);
                    x += run.width;
                    runs.push(run);
                    continue;
                }
                if at >= to {
                    break;
                }
                continue;
            }
            if span.range.start >= to {
                break;
            }
            let start = span.range.start.max(from);
            let end = span.range.end.min(to);
            if start >= end {
                continue;
            }
            x += spaced(&runs, span.kind);
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
                let spanned = &self.spans[span].range;
                let whole =
                    spanned.start.max(self.paragraph.start)..spanned.end.min(self.paragraph.end);
                let shaped = match &self.whole[span] {
                    Some((shaped, galley)) if *shaped == whole => galley.clone(),
                    _ => {
                        let shaped = self
                            .shaper
                            .galley(self.text.get(whole.clone()).unwrap_or(""), style.font);
                        self.whole[span] = Some((whole.clone(), shaped.clone()));
                        shaped
                    }
                };
                let galley = match whole == range {
                    true => shaped,
                    false => {
                        let within = range.start - whole.start..range.end - whole.start;
                        self.shaper.slice(&shaped, within, style.font)
                    }
                };
                RichRun {
                    piece,
                    range,
                    style,
                    kind,
                    x,
                    width: galley.cursor_pos(pos2(0.0, 0.0), galley.text().len()).x,
                    height: galley.line_height(),
                    galley: Some(galley),
                    inner: Vec::new(),
                }
            }
            SpanKind::Inline(_, size) => RichRun {
                inner: self
                    .text
                    .get(range.clone())
                    .unwrap_or_default()
                    .char_indices()
                    .skip(1)
                    .map(|(at, _)| range.start + at)
                    .collect(),
                piece,
                range,
                style,
                kind,
                x,
                width: size.x,
                height: size.y,
                galley: None,
            },
            SpanKind::Gap(_) | SpanKind::Break => RichRun {
                piece,
                range,
                style,
                kind,
                x,
                width: match kind {
                    SpanKind::Gap(width) => width,
                    _ => 0.0,
                },
                height: 0.0,
                galley: None,
                inner: Vec::new(),
            },
            SpanKind::Tab => {
                let stop = self.shaper.galley(" ", style.font).size().x * TAB_COLUMNS;
                let width = match stop > 0.0 {
                    true => ((x / stop).floor() + 1.0) * stop - x,
                    false => 0.0,
                };
                RichRun {
                    piece,
                    range,
                    style,
                    kind,
                    x,
                    width,
                    height: 0.0,
                    galley: None,
                    inner: Vec::new(),
                }
            }
            SpanKind::Icon(_) => {
                let (glyph, text_size) = match self.pieces[piece] {
                    Piece::Icon {
                        glyph, text_size, ..
                    } => (glyph, text_size),
                    _ => ("", 0.0),
                };
                let galley = self.shaper.galley(glyph, style.font);
                let height = self
                    .shaper
                    .galley("", FontId::proportional(text_size))
                    .line_height();
                RichRun {
                    piece,
                    range,
                    style,
                    kind,
                    x,
                    width: galley.size().x,
                    height,
                    galley: Some(galley),
                    inner: Vec::new(),
                }
            }
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
        if !stops.is_sorted_by(|a, b| (a.0, a.1) <= (b.0, b.1)) {
            stops.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        }
        stops
    }

    fn breakpoint(
        &self,
        stops: &[(usize, f32)],
        from: usize,
        to: usize,
        origin: f32,
        wrap: f32,
    ) -> Option<(usize, f32, f32)> {
        let first = stops.partition_point(|(byte, _)| *byte <= from);
        let candidates = stops[first..]
            .iter()
            .take_while(|(byte, x)| *byte < to && x - origin <= wrap);
        let count = candidates.clone().count();
        let fits = &stops[first..first + count];
        let good = fits.iter().rposition(|(byte, _)| self.good(*byte));
        let Some(chosen) = good.or(fits.len().checked_sub(1)) else {
            let (byte, x) = stops[first..]
                .iter()
                .find(|(byte, x)| *byte < to && *x > origin)?;
            return Some((*byte, 0.0, x - origin));
        };
        let (byte, x) = fits[chosen];
        let reach = x - origin;
        let limit = stops[first + chosen + 1..]
            .iter()
            .take_while(|(next, _)| *next <= to)
            .find(|(next, _)| *next > byte && (good.is_none() || *next == to || self.good(*next)))
            .map_or(f32::INFINITY, |(next, x)| match *next == to {
                true => stops[stops.partition_point(|(byte, _)| *byte <= to) - 1].1 - origin,
                false => x - origin,
            });
        Some((byte, reach, limit))
    }

    fn least(&self, stops: &[(usize, f32)], start: usize, end: usize) -> f32 {
        let (mut longest, mut from) = (0.0f32, 0.0f32);
        for &(byte, x) in stops {
            if byte > start && (byte >= end || self.good(byte)) {
                longest = longest.max(x - from);
                from = x;
            }
        }
        longest
    }

    fn good(&self, byte: usize) -> bool {
        self.anywhere
            || self.breaks_after(byte)
            || self
                .text
                .as_bytes()
                .get(byte.wrapping_sub(1))
                .is_some_and(|before| {
                    before.is_ascii_whitespace() || matches!(*before, b'-' | b'/' | b'\\')
                })
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
        let mut start = text.len();
        let (style, kind, break_after) = match *piece {
            Piece::Text {
                text: piece,
                style,
                break_after,
            } => {
                let mut rest = piece;
                while let Some(tab) = rest.find('\t') {
                    for (part, kind) in [(&rest[..tab], SpanKind::Text), ("\t", SpanKind::Tab)] {
                        if part.is_empty() {
                            continue;
                        }
                        let at = text.len();
                        text.push_str(part);
                        spans.push(TextSpan {
                            piece: index,
                            range: at..text.len(),
                            style,
                            kind,
                            break_after: false,
                        });
                    }
                    rest = &rest[tab + 1..];
                }
                start = text.len();
                text.push_str(rest);
                if rest.is_empty() {
                    if let Some(last) = spans.last_mut().filter(|last| last.piece == index) {
                        last.break_after = break_after;
                    }
                    continue;
                }
                (style, SpanKind::Text, break_after)
            }
            Piece::Inline {
                index: inline,
                size,
                text: stands_for,
            } => {
                text.push_str(stands_for);
                (fallback, SpanKind::Inline(inline, size), false)
            }
            Piece::Gap(width) => (fallback, SpanKind::Gap(width), false),
            Piece::Break => (fallback, SpanKind::Break, false),
            Piece::Icon { style, spacing, .. } => (style, SpanKind::Icon(spacing), false),
        };
        let shown = match *piece {
            Piece::Icon { glyph, .. } => !glyph.is_empty(),
            Piece::Gap(_) | Piece::Break => true,
            _ => text.len() > start,
        };
        if shown {
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
            pieces,
            spans: &spans,
            shaper,
            paragraph: 0..0,
            segment: 0..0,
            anywhere: options.anywhere,
            whole: vec![None; spans.len()],
        };
        let segments = segments(text, &spans);
        let kept = previous.map_or(0, |(previous, changed)| {
            let mut kept = previous.line_of(changed).saturating_sub(1);
            while kept > 0
                && (previous.lines[kept].range.start >= changed
                    || previous.lines[kept - 1].range.start == previous.lines[kept].range.start)
            {
                kept -= 1;
            }
            kept
        });
        let mut lines: Vec<RichLine> = match previous {
            Some((previous, _)) => previous.lines[..kept].to_vec(),
            None => Vec::new(),
        };
        let (mut top, mut resume) =
            match previous.and_then(|(previous, _)| previous.lines.get(kept)) {
                Some(line) if kept > 0 => (line.top, Some(line.range.start.min(text.len()))),
                _ => (0.0, None),
            };
        let first = resume.map_or(0, |at| {
            segments
                .iter()
                .position(|segment| {
                    segment.bytes.start <= at
                        && (at < segment.bytes.end || segment.bytes.start == at)
                })
                .unwrap_or(segments.len() - 1)
        });
        for segment in &segments[first..] {
            let (start, end) = (segment.bytes.start, segment.bytes.end);
            let mut from = resume.take().unwrap_or(start);
            builder.paragraph = start..end;
            builder.segment = segment.spans.clone();
            let whole = builder.runs(start, end);
            let least = match options.least {
                true => builder.least(&builder.stops(&whole), start, end),
                false => 0.0,
            };
            let first_line = lines.len();
            let total = whole.last().map_or(0.0, |run| run.x + run.width);
            let stops = match total > options.wrap_width + WRAP_SLACK || from != start {
                true => builder.stops(&whole),
                false => Vec::new(),
            };
            let hang = hang_indent(&whole, start..end, options.hang, options.wrap_width);
            let mut whole = Some(whole);
            loop {
                let (origin, indent) = match from == start {
                    true => (0.0, 0.0),
                    false => (
                        stops
                            .get(stops.partition_point(|(byte, _)| *byte < from))
                            .map_or(0.0, |(_, x)| *x),
                        hang,
                    ),
                };
                let wrap = options.wrap_width - indent + WRAP_SLACK;
                let broke = match total - origin > wrap {
                    true => builder.breakpoint(&stops, from, end, origin, wrap),
                    false => None,
                };
                let (to, mut runs) = match (broke, from == start) {
                    (Some((to, _, _)), _) => (to, builder.runs(from, to)),
                    (None, true) => (end, whole.take().unwrap_or_default()),
                    (None, false) => (end, builder.runs(from, end)),
                };
                for run in &mut runs {
                    run.x += indent;
                }
                let mut line = line(from..to, runs, top, strut, options.padding);
                top += line.height;
                let Some((_, reach, limit)) = broke else {
                    line.reach = total - origin + indent - WRAP_SLACK;
                    lines.push(line);
                    break;
                };
                line.reach = reach + indent - WRAP_SLACK;
                line.limit = limit + indent - WRAP_SLACK;
                lines.push(line);
                from = to;
            }
            if let Some(line) = lines.get_mut(first_line) {
                line.least = least;
            }
        }
        let width = lines.iter().map(|line| line.width).fold(0.0, f32::max);
        let reach = lines.iter().map(|line| line.reach).fold(0.0, f32::max);
        let limit = lines
            .iter()
            .map(|line| line.limit)
            .fold(f32::INFINITY, f32::min);
        Self {
            lines,
            size: vec2(width, top),
            reach,
            limit,
        }
    }

    pub fn paint_runs(
        &self,
        painter: &Painter,
        origin: Vec2,
        paint_of: impl Fn(&RichRun) -> (Color32, bool, bool),
    ) {
        for line in &self.lines {
            for run in &line.runs {
                let Some(galley) = &run.galley else {
                    continue;
                };
                let left = origin.x + line.run_left(run);
                let (color, underline, strikethrough) = paint_of(run);
                let decorated = run.kind == SpanKind::Text;
                painter.galley(
                    pos2(left, origin.y + line.run_top(run)),
                    galley.clone(),
                    color,
                );
                let size = run.style.font.size;
                let thickness = (size * SPAN_LINE_THICKNESS).max(1.0);
                let baseline = origin.y + line.top + line.baseline;
                let decorate = |y: f32| {
                    painter.rect_filled(
                        Rect::from_min_size(pos2(left, y), Vec2::new(run.width, thickness)),
                        0.0,
                        color,
                    );
                };
                if underline && decorated {
                    decorate(baseline + (size * SPAN_UNDERLINE_OFFSET).max(1.0));
                }
                if strikethrough && decorated {
                    decorate(baseline - size * SPAN_STRIKE_OFFSET);
                }
            }
        }
    }

    pub fn wraps(&self) -> Wraps {
        Wraps::between(self.reach, self.limit)
    }

    pub fn fits(&self, wrap_width: f32) -> bool {
        wrap_width >= self.reach && wrap_width < self.limit
    }

    pub fn least_width(&self) -> f32 {
        self.lines.iter().map(|line| line.least).fold(0.0, f32::max)
    }

    pub fn truncate(&mut self, width: f32, shaper: &mut dyn Shaper) {
        for line in &mut self.lines {
            if line.width > width {
                line.ellipsize(width, shaper);
            }
        }
        self.size.x = self.lines.iter().map(|line| line.width).fold(0.0, f32::max);
    }

    pub fn clamp(&mut self, max_lines: usize, width: f32, shaper: &mut dyn Shaper) {
        let max_lines = max_lines.max(1);
        let Some(end) = self.lines.last().map(|line| line.range.end) else {
            return;
        };
        if self.lines.len() <= max_lines {
            return;
        }
        self.lines.truncate(max_lines);
        if let Some(last) = self.lines.last_mut() {
            last.range.end = end;
            last.ellipsize(width, shaper);
            self.size.y = last.top + last.height;
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
                false => run.inner_stops(&mut stops),
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
        self.range_boxes(range)
            .into_iter()
            .map(|(rect, _)| {
                Rect::from_min_max(rect.min, pos2(rect.max.x.max(rect.min.x + 1.0), rect.max.y))
            })
            .collect()
    }

    pub fn range_boxes(&self, range: Range<usize>) -> Vec<(Rect, f32)> {
        self.lines
            .iter()
            .filter_map(|line| {
                let start = range.start.max(line.range.start);
                let end = range.end.min(line.range.end);
                if start >= end {
                    return None;
                }
                let left = line.x_of(start);
                let right = line.x_after(end).max(left);
                Some((
                    Rect::from_min_max(pos2(left, line.top), pos2(right, line.top + line.height)),
                    line.top + line.baseline,
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

fn hang_indent(runs: &[RichRun], paragraph: Range<usize>, hang: usize, wrap: f32) -> f32 {
    if hang <= paragraph.start || hang > paragraph.end {
        return 0.0;
    }
    let x = match runs.iter().find(|run| run.range.contains(&hang)) {
        Some(run) => run.x_of(hang),
        None => runs.last().map_or(0.0, |run| run.x + run.width),
    };
    x.min(wrap / 2.0)
}

fn line(
    range: Range<usize>,
    runs: Vec<RichRun>,
    top: f32,
    strut: (f32, f32),
    padding: (f32, f32),
) -> RichLine {
    let (mut ascent, mut descent) = (strut.0, strut.1 - strut.0);
    let mut tallest = strut.1;
    let mut middle = strut.1 / 2.0 - strut.0;
    for run in &runs {
        if let Some(galley) = run.galley.as_ref().filter(|_| run.text()) {
            ascent = ascent.max(galley.baseline());
            descent = descent.max(galley.line_height() - galley.baseline());
            if galley.line_height() > tallest {
                tallest = galley.line_height();
                middle = galley.line_height() / 2.0 - galley.baseline();
            }
        }
    }
    for run in runs.iter().filter(|run| !run.text()) {
        let half = run.height / 2.0;
        ascent = ascent.max(half - middle);
        descent = descent.max(half + middle);
    }
    let width = runs.last().map_or(0.0, |run| run.x + run.width);
    RichLine {
        range,
        reach: 0.0,
        limit: f32::INFINITY,
        least: 0.0,
        top,
        left: 0.0,
        height: ascent + descent + padding.0 + padding.1,
        baseline: ascent + padding.0,
        middle: ascent + padding.0 + middle,
        width,
        runs,
    }
}

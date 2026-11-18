use std::ops::Range;

use crate::font::{Galley, Wraps};
use crate::painter::Painter;
use be_lib_graphemes::grapheme_boundaries;
use beui_types::Color32;
use beui_types::{FontId, TextAlign};
use beui_types::{Rect, Vec2, pos2, vec2};

const SPAN_UNDERLINE_OFFSET: f32 = 0.12;
const SPAN_STRIKE_OFFSET: f32 = 0.32;
const SPAN_LINE_THICKNESS: f32 = 1.0 / 16.0;
pub const TAB_COLUMNS: f32 = 4.0;
const WRAP_SLACK: f32 = 1.0 / 1024.0;
/// How wide the break zone at the end of a wrapped line is, in ems of the text's font.
pub const BREAK_ZONE_EMS: f32 = 6.0;

/// A place a line could end: its byte, its x, the x it must fit by (before any spaces it
/// follows, which hang) and how good a break it is, if the line can break there at all.
struct Stop {
    byte: usize,
    x: f32,
    fit: f32,
    tier: Option<Tier>,
}

/// How good a place to break a line is, best first (guides/beui/text.md, "Wrapping").
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Tier {
    /// A Unicode line break opportunity (UAX #14), or the end of a piece that breaks after it.
    Good,
    /// After `/ . _ : , ; ) ] } -` or before `( [ {` inside a word, as in code, paths and URLs.
    LessGood,
    /// Any other grapheme cluster boundary.
    LastResort,
}

use beui_types::{GapWidth, Piece, RichOptions, SpanStyle};

pub const OBJECT: &str = "\u{fffc}";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SpanKind {
    Text,
    Inline(usize, Vec2),
    Gap(GapWidth),
    Blank(f32),
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
    /// Where the spaces that end the line begin, and the width they hang past: a caret or
    /// selection among them stops at `edge` rather than running on past the text.
    hung: usize,
    edge: f32,
    pub top: f32,
    pub left: f32,
    pub height: f32,
    pub baseline: f32,
    pub middle: f32,
    pub width: f32,
    pub runs: Vec<RichRun>,
}

impl RichLine {
    /// Where a caret at `index` stands: before the character there, past any gap, icon or
    /// break that holds no bytes before it; at the line's end, after its last character and the
    /// gaps right after that (room the text keeps), but before an icon that ends the line. A
    /// selection runs between the places of its ends, so it is drawn where its carets are.
    pub fn x_of(&self, index: usize) -> f32 {
        if index >= self.range.end {
            return self.hang(index, self.left + self.end_x());
        }
        let x = self.left
            + self
                .runs
                .iter()
                .find(|run| run.range.contains(&index))
                .map_or(0.0, |run| run.x_of(index));
        self.hang(index, x)
    }

    fn end_x(&self) -> f32 {
        let last = self.runs.iter().rposition(|run| !run.range.is_empty());
        let mut end = last.map_or(0.0, |at| self.runs[at].x + self.runs[at].width);
        for run in &self.runs[last.map_or(0, |at| at + 1)..] {
            match run.kind {
                SpanKind::Gap(_) => end = run.x + run.width,
                _ => break,
            }
        }
        end
    }

    /// `x` for a byte among the spaces that hang past the line's end, kept at the edge, but
    /// for the width of each `Piece::Blank` among them before it, which stays its own past the
    /// edge so a mark or glyph over a blank has room.
    fn hang(&self, index: usize, x: f32) -> f32 {
        if index <= self.hung {
            return x;
        }
        let blanks: f32 = self
            .runs
            .iter()
            .filter(|run| run.range.start >= self.hung && run.range.end <= index)
            .filter_map(|run| match run.kind {
                SpanKind::Blank(width) => Some(width),
                _ => None,
            })
            .sum();
        x.min(self.edge + blanks)
    }

    pub fn x_after(&self, index: usize) -> f32 {
        self.runs
            .iter()
            .rev()
            .find(|run| run.range.start < index)
            .map_or_else(
                || self.x_of(index),
                |run| self.hang(index, self.left + run.x_of(index.min(run.range.end))),
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
                let ellipsis = match budget < 0.0 {
                    true => shaper.galley("", style.font),
                    false => ellipsis,
                };
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
    /// The byte ranges of the isolated text pieces (`Piece::Text::isolate`), in order. Nothing
    /// reads them yet, since layout has no bidi: the bidi step of plans/deferred/i18n.md resolves
    /// each as a paragraph of its own and treats it as one neutral in the text around it.
    pub isolates: Vec<Range<usize>>,
    reach: f32,
    limit: f32,
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
    pieces: &'a [Piece<'a>],
    spans: &'a [TextSpan],
    shaper: &'a mut dyn Shaper,
    paragraph: Range<usize>,
    segment: Range<usize>,
    anywhere: bool,
    /// What a gap's share of the width is a share of.
    wrap_width: f32,
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
                // The part of a span on one wrapped line is shaped on its own
                // rather than cut from the paragraph's shaping: its glyphs'
                // subpixel phases then start afresh at the line's start, so an
                // edit earlier in the paragraph leaves a line whose text did
                // not change drawn exactly as it was.
                let galley = match whole == range {
                    true => shaped,
                    false => self
                        .shaper
                        .galley(self.text.get(range.clone()).unwrap_or(""), style.font),
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
            // A blank's galley is empty: it measures the line as text in its font would, and
            // draws nothing.
            SpanKind::Blank(width) => RichRun {
                piece,
                range,
                style,
                kind,
                x,
                width,
                height: 0.0,
                galley: Some(self.shaper.galley("", style.font)),
                inner: Vec::new(),
            },
            SpanKind::Gap(_) | SpanKind::Break => RichRun {
                piece,
                range,
                style,
                kind,
                x,
                width: match kind {
                    SpanKind::Gap(width) => width.of(self.wrap_width),
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

    /// Classifies each stop of the paragraph as a place to break (guides/beui/text.md,
    /// "Wrapping"), in one pass: its tier, and the x it must fit by, which for a stop after
    /// spaces is where the spaces begin, so trailing spaces hang past the edge.
    fn classify(&self, stops: Vec<(usize, f32)>) -> Vec<Stop> {
        let Range { start, end } = self.paragraph.clone();
        let paragraph = self.text.get(start..end).unwrap_or_default();
        const GOOD: u8 = 1;
        const BOUNDARY: u8 = 2;
        let mut flags = vec![0u8; paragraph.len() + 1];
        for (at, _) in unicode_linebreak::linebreaks(paragraph) {
            flags[at] |= GOOD;
        }
        for at in grapheme_boundaries(paragraph.as_bytes()) {
            flags[at] |= BOUNDARY;
        }
        let bytes = self.text.as_bytes();
        let mut span = self.spans.partition_point(|span| span.range.end < start);
        let mut ink = f32::NEG_INFINITY;
        stops
            .into_iter()
            .map(|(byte, x)| {
                while self.spans.get(span).is_some_and(|it| it.range.end < byte) {
                    span += 1;
                }
                let gap_or_break_after = self.spans[span.min(self.spans.len())..]
                    .iter()
                    .take_while(|it| it.range.end == byte)
                    .any(|it| it.break_after || matches!(it.kind, SpanKind::Gap(_)));
                let at = byte.saturating_sub(start).min(paragraph.len());
                let tier = match () {
                    _ if gap_or_break_after || flags[at] & GOOD != 0 => Some(Tier::Good),
                    _ if flags[at] & BOUNDARY == 0 => None,
                    _ if self.anywhere => Some(Tier::Good),
                    _ if matches!(
                        bytes.get(byte.wrapping_sub(1)),
                        Some(b'/' | b'.' | b'_' | b':' | b',' | b';' | b')' | b']' | b'}' | b'-')
                    ) || matches!(bytes.get(byte), Some(b'(' | b'[' | b'{')) =>
                    {
                        Some(Tier::LessGood)
                    }
                    _ => Some(Tier::LastResort),
                };
                let spaced = byte > start
                    && self.text[start..byte]
                        .chars()
                        .next_back()
                        .is_some_and(char::is_whitespace);
                if !spaced || self.anywhere || ink == f32::NEG_INFINITY {
                    ink = x;
                }
                Stop {
                    byte,
                    x,
                    fit: ink,
                    tier,
                }
            })
            .collect()
    }

    /// Where the line from `from` breaks when the rest of the paragraph does not fit in
    /// `wrap` (guides/beui/text.md, "Wrapping"): the rightmost break point of the best tier
    /// inside the break zone, the last `zone` before `wrap`, else the rightmost grapheme
    /// boundary that fits. Returns the byte, the least wrap that breaks there the same way,
    /// the least wrap that does not, and how wide the line is without the spaces that hang
    /// past its end. One pass forward from `from`, and on past the line at most `zone`.
    fn breakpoint(
        &self,
        stops: &[Stop],
        from: usize,
        to: usize,
        origin: f32,
        wrap: f32,
        zone: f32,
    ) -> Option<(usize, f32, f32, f32)> {
        let first = stops.partition_point(|stop| stop.byte <= from);
        let edge = wrap - zone;
        let mut best = [None::<(usize, f32)>; 3];
        let mut fitting = None::<(usize, f32)>;
        let mut before = [f32::NEG_INFINITY; 2];
        let mut index = first;
        while let Some(stop) = stops.get(index) {
            let x = stop.fit - origin;
            if stop.byte >= to || x > wrap {
                break;
            }
            index += 1;
            let Some(tier) = stop.tier else {
                continue;
            };
            fitting = Some((stop.byte, x));
            match x >= edge {
                true => best[tier as usize] = Some((stop.byte, x)),
                false => {
                    for (better, rightmost) in before.iter_mut().enumerate() {
                        if tier as usize <= better {
                            *rightmost = x;
                        }
                    }
                }
            }
        }
        let chosen = match best {
            [Some(good), ..] => Some((good, Tier::Good)),
            [None, Some(less), _] => Some((less, Tier::LessGood)),
            _ => fitting.map(|fitting| (fitting, Tier::LastResort)),
        };
        let Some(((byte, x), tier)) = chosen else {
            let stop = stops[first..]
                .iter()
                .find(|stop| stop.byte < to && stop.x > origin && stop.tier.is_some())?;
            let x = stop.fit.max(origin) - origin;
            return Some((stop.byte, 0.0, x, x));
        };
        let mut reach = x;
        if let Some(better) = (tier as usize).checked_sub(1) {
            reach = reach.max(before[better] + zone + WRAP_SLACK);
        }
        // Wider, the line breaks elsewhere once a break point as good as this one fits, or once
        // this one leaves the zone and something breakable after it fits.
        let mut alone = fitting.is_some_and(|(last, _)| last == byte);
        let mut limit = match tier {
            Tier::LastResort => f32::INFINITY,
            _ => x + zone,
        };
        for stop in &stops[index..] {
            let at = stop.fit - origin;
            if stop.byte >= to {
                break;
            }
            let Some(found) = stop.tier.filter(|_| stop.byte > byte) else {
                if at >= limit && !alone {
                    break;
                }
                continue;
            };
            if alone {
                limit = limit.max(at);
                alone = false;
            }
            if found <= tier {
                limit = limit.min(at);
                break;
            }
            if at >= limit {
                break;
            }
        }
        let rest = stops[stops.partition_point(|stop| stop.byte <= to) - 1].fit - origin;
        let limit = match alone {
            true => rest,
            false => limit.min(rest),
        };
        match reach <= wrap && wrap < limit {
            true => Some((byte, reach, limit, x)),
            false => Some((byte, wrap, wrap + WRAP_SLACK, x)),
        }
    }
}

fn isolates_of(pieces: &[Piece]) -> Vec<Range<usize>> {
    let mut at = 0;
    let mut isolates = Vec::new();
    for piece in pieces {
        let end = at + piece.len();
        if let Piece::Text { isolate: true, .. } = piece
            && end > at
        {
            isolates.push(at..end);
        }
        at = end;
    }
    isolates
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
                ..
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
            Piece::Blank { width, style } => {
                text.push_str(beui_types::text::BLANK);
                (style, SpanKind::Blank(width), false)
            }
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
            wrap_width: options.wrap_width,
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
            let total = whole.last().map_or(0.0, |run| run.x + run.width);
            let hangs = text[start..end].ends_with(char::is_whitespace);
            let breaks = total > options.wrap_width + WRAP_SLACK || from != start || hangs;
            let stops = match breaks {
                true => builder.classify(builder.stops(&whole)),
                false => Vec::new(),
            };
            let hang = hang_indent(&whole, start..end, options.hang, options.wrap_width);
            let zone = options.style.font.size * BREAK_ZONE_EMS;
            let inked = stops.last().map_or(total, |stop| stop.fit);
            let mut whole = Some(whole);
            loop {
                let (origin, indent) = match from == start {
                    true => (0.0, 0.0),
                    false => (
                        stops
                            .get(stops.partition_point(|stop| stop.byte < from))
                            .map_or(0.0, |stop| stop.x),
                        hang,
                    ),
                };
                let wrap = options.wrap_width - indent + WRAP_SLACK;
                let broke = match inked - origin > wrap {
                    true => builder.breakpoint(&stops, from, end, origin, wrap, zone),
                    false => None,
                };
                let (to, mut runs) = match (broke, from == start) {
                    (Some((to, ..)), _) => (to, builder.runs(from, to)),
                    (None, true) => (end, whole.take().unwrap_or_default()),
                    (None, false) => (end, builder.runs(from, end)),
                };
                for run in &mut runs {
                    run.x += indent;
                }
                let mut line = line(from..to, runs, top, strut, options.padding);
                line.hung = from + text[from..to].trim_end().len();
                line.edge = options.wrap_width;
                top += line.height;
                let Some((_, reach, limit, ink)) = broke else {
                    if line.width > options.wrap_width {
                        line.width = line.width.min(inked - origin + indent);
                    }
                    line.reach = inked - origin + indent - WRAP_SLACK;
                    lines.push(line);
                    break;
                };
                if line.width > options.wrap_width {
                    line.width = line.width.min(ink + indent);
                }
                line.reach = reach + indent - WRAP_SLACK;
                line.limit = limit + indent - WRAP_SLACK;
                lines.push(line);
                from = to;
            }
        }
        let width = lines
            .iter()
            .map(|line| line.width.min(options.wrap_width))
            .fold(0.0, f32::max);
        let reach = lines.iter().map(|line| line.reach).fold(0.0, f32::max);
        let limit = lines
            .iter()
            .map(|line| line.limit)
            .fold(f32::INFINITY, f32::min);
        // A gap with a share of the width is as wide as the width it was laid out at, so the
        // layout holds at that width alone.
        let shared = spans
            .iter()
            .any(|span| matches!(span.kind, SpanKind::Gap(gap) if gap.share != 0.0));
        let (reach, limit) = match shared {
            true => (options.wrap_width, options.wrap_width + WRAP_SLACK),
            false => (reach, limit),
        };
        Self {
            lines,
            size: vec2(width, top),
            isolates: isolates_of(pieces),
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
                let Some(galley) = run
                    .galley
                    .as_ref()
                    .filter(|_| !matches!(run.kind, SpanKind::Blank(_)))
                else {
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
        let x = line.x_of(index).min(line.edge - width);
        Rect::from_min_size(pos2(x, line.top), vec2(width, line.height))
    }

    pub fn index_at(&self, point: beui_types::Pos2) -> usize {
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

    /// The rectangles a selection or mark over `range` fills, a line at a time: from where a
    /// caret at its start stands to where one at its end does (`RichLine::x_of`), so it covers a
    /// gap between its characters, and one its end caret is drawn past, as the carets show.
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

    /// The rectangles a span's background fills: its own characters, not a gap or icon beside
    /// them.
    pub fn span_rects(&self, range: Range<usize>) -> Vec<Rect> {
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

    pub fn inline_at(&self, point: beui_types::Pos2) -> Option<usize> {
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
    let metric = |run: &RichRun| run.text() || matches!(run.kind, SpanKind::Blank(_));
    for run in &runs {
        if let Some(galley) = run.galley.as_ref().filter(|_| metric(run)) {
            ascent = ascent.max(galley.baseline());
            descent = descent.max(galley.line_height() - galley.baseline());
            if galley.line_height() > tallest {
                tallest = galley.line_height();
                middle = galley.line_height() / 2.0 - galley.baseline();
            }
        }
    }
    for run in runs.iter().filter(|run| !metric(run)) {
        let half = run.height / 2.0;
        ascent = ascent.max(half - middle);
        descent = descent.max(half + middle);
    }
    let width = runs.last().map_or(0.0, |run| run.x + run.width);
    RichLine {
        range: range.clone(),
        reach: 0.0,
        limit: f32::INFINITY,
        hung: range.end,
        edge: f32::INFINITY,
        top,
        left: 0.0,
        height: ascent + descent + padding.0 + padding.1,
        baseline: ascent + padding.0,
        middle: ascent + padding.0 + middle,
        width,
        runs,
    }
}

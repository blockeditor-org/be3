use be_lib_stable_hash::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

use beui_html::Node;
use beui_reactive::{Get, untrack};
use beui_types::text::{FontId, LineBox, handle_shape};
use beui_types::{Rect, Vec2, pos2, vec2};
use js_sys::{Function, Reflect};
use wasm_bindgen::{JsCast, JsValue};

use super::{Piece, Stop, TextModel, TextSlot, after, byte_at, ending_at, model, text_stops};
use crate::css;
use crate::node::NodeId;

const CARET_FLAG: Vec2 = Vec2::new(6.0, 4.0);
/// The attribute a span with a background carries (`beui-view`'s `Span`).
const BACKGROUND: &str = "data-beui-background";

#[derive(Default)]
pub(crate) struct Shown {
    hang: Option<String>,
    boxes: Vec<(&'static str, String)>,
}

struct Frame {
    rect: web_sys::DomRect,
    origin: (f64, f64),
    top: f64,
    bottom: f64,
    left: f64,
    right: f64,
    line: f64,
    start: f64,
}

impl Frame {
    fn line_of(&self, rect: &web_sys::DomRect) -> i64 {
        let middle = rect.top() + rect.height() / 2.0;
        ((middle - self.top) / self.line).floor().max(0.0) as i64
    }

    fn line_top(&self, line: i64) -> f64 {
        self.top + line as f64 * self.line
    }

    fn window(&self, left: f64, top: f64, width: f64, height: f64) -> Rect {
        Rect::from_min_size(
            pos2((left - self.origin.0) as f32, (top - self.origin.1) as f32),
            vec2(width as f32, height as f32),
        )
    }
}

fn pixels(style: &web_sys::CssStyleDeclaration, property: &str) -> Option<f64> {
    style
        .get_property_value(property)
        .ok()?
        .trim_end_matches("px")
        .parse()
        .ok()
}

fn frame(node: NodeId) -> Option<Frame> {
    let element: web_sys::Element = crate::dom::dom(node)?.dyn_into().ok()?;
    if !element.is_connected() {
        return None;
    }
    let rect = element.get_bounding_client_rect();
    let style = crate::dom::window()?.get_computed_style(&element).ok()??;
    let padding = |side: &str| pixels(&style, &format!("padding-{side}")).unwrap_or(0.0);
    let line = pixels(&style, "line-height")
        .or_else(|| {
            pixels(&style, "font-size").map(|size| f64::from(super::line_height(size as f32)))
        })
        .filter(|line| *line > 0.0)
        .unwrap_or(16.0);
    let origin = crate::run::root()
        .and_then(crate::dom::dom)
        .and_then(|dom| dom.dyn_into::<web_sys::Element>().ok())
        .map_or((0.0, 0.0), |root| {
            let origin = root.get_bounding_client_rect();
            (origin.left(), origin.top())
        });
    let left = rect.left() + padding("left");
    let right = rect.right() - padding("right");
    let start = match style
        .get_property_value("text-align")
        .unwrap_or_default()
        .as_str()
    {
        "center" => (left + right) / 2.0,
        "end" | "right" => right,
        _ => rect.left(),
    };
    Some(Frame {
        top: rect.top() + padding("top"),
        bottom: rect.bottom() - padding("bottom"),
        left,
        right,
        line,
        start,
        origin,
        rect,
    })
}

type Stops = Vec<Stop<web_sys::Node>>;

fn stops(model: &TextModel) -> Stops {
    let mut stops = Vec::new();
    let mut at = 0;
    for (node, piece) in model.ordered() {
        match piece {
            Piece::Run(run) => {
                let text = run.text.borrow();
                let start = run.start.get();
                for held in run.held.borrow().iter() {
                    let range = held.segment.range();
                    if let Some(dom) = held.text.dom() {
                        text_stops(start + range.start, &text[range], &dom, &mut stops);
                    }
                }
                at = start + text.len();
            }
            Piece::Item(slot) => {
                let slot = untrack(|| slot.get());
                if let TextSlot::Anchored(_) = slot {
                    continue;
                }
                let len = slot.bytes();
                if let Some(dom) = crate::dom::dom(node) {
                    stops.push(Stop::Atom {
                        byte: at,
                        len,
                        node: dom,
                        newline: false,
                    });
                }
                at += len;
            }
            Piece::Gap | Piece::Skip { .. } => {
                if let Some(stop) = crate::dom::dom(node)
                    .and_then(|dom| super::empty_stop(&piece, at, dom))
                {
                    stops.push(stop);
                }
            }
        }
    }
    stops
}

fn stop_rect(stop: &Stop<web_sys::Node>) -> Option<web_sys::DomRect> {
    let rect = match stop {
        Stop::Char {
            node, utf16, ch, ..
        } => {
            let range = crate::dom::document()?.create_range().ok()?;
            range.set_start(node, *utf16).ok()?;
            range.set_end(node, utf16 + ch.len_utf16() as u32).ok()?;
            range.get_bounding_client_rect()
        }
        Stop::Atom { node, .. } | Stop::Room { node, .. } => node
            .dyn_ref::<web_sys::Element>()?
            .get_bounding_client_rect(),
    };
    // An empty gap's inline block is no taller than nothing, but has its width.
    (rect.height() > 0.0 || matches!(stop, Stop::Room { .. })).then_some(rect)
}

/// Where a caret at `index` stands, its x and line: before the character there, else (at the
/// end of a line) after the one before it and the gaps right after that, as natively
/// (`RichLine::x_of`). A mark is drawn between the spots of its ends.
fn spot(frame: &Frame, stops: &Stops, index: usize) -> (f64, i64) {
    if let Some(stop) = after(stops, index).filter(|stop| !stop.breaks())
        && let Some(rect) = stop_rect(stop)
    {
        return (rect.left(), frame.line_of(&rect));
    }
    if let Some((stop, rooms)) = ending_at(stops, index).filter(|(stop, _)| !stop.breaks())
        && let Some(rect) = stop_rect(stop)
    {
        let line = frame.line_of(&rect);
        let right = rooms
            .iter()
            .filter_map(|stop| stop_rect(stop))
            .take_while(|room| frame.line_of(room) == line)
            .last()
            .map_or(rect.right(), |room| room.right());
        return (right, line);
    }
    let mut breaks = 0;
    for stop in stops
        .iter()
        .rev()
        .filter(|stop| stop.byte() + stop.len() <= index)
    {
        if stop.breaks() {
            breaks += 1;
            continue;
        }
        if let Some(rect) = stop_rect(stop) {
            return (frame.start, frame.line_of(&rect) + breaks);
        }
    }
    (frame.start, breaks)
}

pub(crate) fn caret_rect(node: NodeId, index: usize, width: f32) -> Option<Rect> {
    let model = model(node)?;
    let frame = frame(node)?;
    let (x, line) = spot(&frame, &stops(&model), index);
    Some(frame.window(x, frame.line_top(line), f64::from(width), frame.line))
}

fn caret_from_point(x: f64, y: f64) -> Option<(web_sys::Node, u32)> {
    let document: JsValue = crate::dom::document()?.into();
    let call = |name: &str| -> Option<JsValue> {
        let function: Function = Reflect::get(&document, &name.into())
            .ok()?
            .dyn_into()
            .ok()?;
        let found = function.call2(&document, &x.into(), &y.into()).ok()?;
        (!found.is_null() && !found.is_undefined()).then_some(found)
    };
    if let Some(position) = call("caretPositionFromPoint") {
        let node = Reflect::get(&position, &"offsetNode".into()).ok()?;
        let offset = Reflect::get(&position, &"offset".into()).ok()?.as_f64()?;
        return Some((node.dyn_into().ok()?, offset as u32));
    }
    let range: web_sys::Range = call("caretRangeFromPoint")?.dyn_into().ok()?;
    Some((range.start_container().ok()?, range.start_offset().ok()?))
}

pub(crate) fn index_at(node: NodeId, pos: beui_types::Pos2) -> Option<usize> {
    let model = model(node)?;
    let frame = frame(node)?;
    let stops = stops(&model);
    if stops.is_empty() {
        return Some(0);
    }
    let x =
        (f64::from(pos.x) + frame.origin.0).clamp(frame.left, (frame.right - 0.5).max(frame.left));
    let y =
        (f64::from(pos.y) + frame.origin.1).clamp(frame.top, (frame.bottom - 0.5).max(frame.top));
    let line = ((y - frame.top) / frame.line).floor().max(0.0) as i64;
    let middle = (frame.line_top(line) + frame.line / 2.0).min(frame.bottom - 0.5);
    if let Some((dom, offset)) = caret_from_point(x, middle)
        && dom.node_type() == web_sys::Node::TEXT_NODE
        && let Some(index) = byte_at(&stops, |held| held.is_same_node(Some(&dom)), offset)
    {
        return Some(index);
    }
    let mut nearest: Option<(f64, usize)> = None;
    let mut last_line = 0;
    for stop in &stops {
        if stop.is_empty() {
            continue;
        }
        let Some(rect) = stop_rect(stop) else {
            continue;
        };
        let at = frame.line_of(&rect);
        last_line = last_line.max(at);
        if at != line {
            continue;
        }
        for (index, edge) in [
            (stop.byte(), rect.left()),
            (stop.byte() + stop.len(), rect.right()),
        ] {
            let distance = (edge - x).abs();
            if nearest.is_none_or(|(held, _)| distance < held) {
                nearest = Some((distance, index));
            }
        }
    }
    let end = stops.last().map_or(0, |stop| stop.byte() + stop.len());
    Some(match nearest {
        Some((_, index)) => index,
        None if line > last_line => end,
        None => 0,
    })
}

pub(crate) fn inline_at(node: NodeId, pos: beui_types::Pos2) -> Option<usize> {
    let model = model(node)?;
    let frame = frame(node)?;
    let x = f64::from(pos.x) + frame.origin.0;
    let y = f64::from(pos.y) + frame.origin.1;
    stops(&model)
        .iter()
        .filter(|stop| matches!(stop, Stop::Atom { len, .. } if *len > 0))
        .position(|stop| {
            stop_rect(stop).is_some_and(|rect| {
                x >= rect.left() && x < rect.right() && y >= rect.top() && y < rect.bottom()
            })
        })
}

pub(crate) fn lines(node: NodeId) -> Vec<LineBox> {
    let (Some(model), Some(frame)) = (model(node), frame(node)) else {
        return Vec::new();
    };
    let ascent = model
        .decor
        .borrow()
        .font
        .and_then(ascent)
        .unwrap_or(frame.line * 0.8);
    struct Line {
        number: i64,
        bytes: std::ops::Range<usize>,
        left: f64,
        right: f64,
        baseline: Option<f64>,
    }
    let blank = |number: i64, at: usize| Line {
        number,
        bytes: at..at,
        left: frame.start,
        right: frame.start,
        baseline: None,
    };
    let mut lines: Vec<Line> = Vec::new();
    let mut broken: Option<usize> = None;
    for stop in &stops(&model) {
        if stop.breaks() {
            let end = stop.byte() + stop.len();
            match (broken, lines.last_mut()) {
                (Some(at), _) => {
                    let number = lines.last().map_or(0, |line| line.number + 1);
                    lines.push(blank(number, at));
                    if let Some(line) = lines.last_mut() {
                        line.bytes.end = end;
                    }
                }
                (None, Some(line)) => line.bytes.end = end,
                (None, None) => lines.push(Line {
                    bytes: 0..end,
                    ..blank(0, 0)
                }),
            }
            broken = Some(end);
            continue;
        }
        if stop.is_empty() {
            continue;
        }
        let Some(rect) = stop_rect(stop) else {
            continue;
        };
        let number = frame.line_of(&rect);
        let end = stop.byte() + stop.len();
        broken = None;
        match lines.last_mut() {
            Some(line) if line.number == number => {
                line.bytes.end = end;
                line.left = line.left.min(rect.left());
                line.right = line.right.max(rect.right());
                line.baseline.get_or_insert(rect.top() + ascent);
            }
            _ => lines.push(Line {
                number,
                bytes: stop.byte()..end,
                left: rect.left(),
                right: rect.right(),
                baseline: Some(rect.top() + ascent),
            }),
        }
    }
    if let Some(at) = broken {
        let number = lines.last().map_or(0, |line| line.number + 1);
        lines.push(blank(number, at));
    }
    if lines.is_empty() {
        lines.push(blank(0, 0));
    }
    let mut start = 0;
    lines
        .into_iter()
        .map(|line| {
            let top = frame.line_top(line.number);
            let bytes = start.min(line.bytes.start)..line.bytes.end;
            start = line.bytes.end;
            let baseline = line
                .baseline
                .unwrap_or(top + (frame.line - ascent / 0.8) / 2.0 + ascent);
            LineBox {
                bytes,
                rect: frame.window(line.left, top, line.right - line.left, frame.line),
                baseline: (baseline - frame.origin.1) as f32,
            }
        })
        .collect()
}

thread_local! {
    static CANVAS: RefCell<Option<web_sys::CanvasRenderingContext2d>> = const { RefCell::new(None) };
}

pub(crate) fn canvas<R>(read: impl FnOnce(&web_sys::CanvasRenderingContext2d) -> R) -> Option<R> {
    CANVAS.with(|held| {
        let mut held = held.borrow_mut();
        if held.is_none() {
            let canvas: web_sys::HtmlCanvasElement = crate::dom::document()?
                .create_element("canvas")
                .ok()?
                .dyn_into()
                .ok()?;
            *held = canvas.get_context("2d").ok()??.dyn_into().ok();
        }
        held.as_ref().map(read)
    })
}

fn ascent(font: FontId) -> Option<f64> {
    canvas(|context| {
        context.set_font(&css::font(font));
        context
            .measure_text("x")
            .ok()
            .map(|metrics| metrics.font_bounding_box_ascent())
    })
    .flatten()
}

fn boundary(stops: &Stops, index: usize, start: bool) -> Option<(web_sys::Node, u32)> {
    let chars = stops.iter().filter_map(|stop| match stop {
        Stop::Char {
            byte,
            ch,
            node,
            utf16,
        } => Some((*byte, *ch, node, *utf16)),
        Stop::Atom { .. } | Stop::Room { .. } => None,
    });
    if start {
        chars
            .into_iter()
            .find(|(byte, ..)| *byte >= index)
            .map(|(_, _, node, utf16)| (node.clone(), utf16))
    } else {
        chars
            .filter(|(byte, ch, ..)| byte + ch.len_utf8() <= index)
            .last()
            .map(|(_, ch, node, utf16)| (node.clone(), utf16 + ch.len_utf16() as u32))
    }
}

fn dom_range(stops: &Stops, bytes: std::ops::Range<usize>) -> Option<web_sys::Range> {
    let (start, from) = boundary(stops, bytes.start, true)?;
    let (end, to) = boundary(stops, bytes.end, false)?;
    let range = crate::dom::document()?.create_range().ok()?;
    range.set_start(&start, from).ok()?;
    range.set_end(&end, to).ok()?;
    (!range.collapsed()).then_some(range)
}

fn apply_hang(model: &TextModel, frame: &Frame, stops: &Stops) -> bool {
    let hang = model.decor.borrow().hang;
    let wanted = (hang > 0)
        .then(|| {
            let (x, line) = spot(frame, stops, hang);
            let (start, _) = spot(frame, stops, 0);
            (line == 0)
                .then(|| css::px(((x - start).max(0.0).min(frame.rect.width() / 2.0)) as f32))
        })
        .flatten();
    let mut decor = model.decor.borrow_mut();
    if decor.shown.hang == wanted || (hang > 0 && wanted.is_none() && decor.shown.hang.is_some()) {
        return false;
    }
    decor.shown.hang = wanted.clone();
    drop(decor);
    crate::document::set_style(model.node, "padding-left", wanted.clone());
    crate::document::set_style(model.node, "text-indent", wanted.map(|x| format!("-{x}")));
    model.bump();
    true
}

/// The boxes a mark over `bytes` fills, as (left, line, right): from where a caret at its start
/// stands to where one at its end does, a line at a time, so it covers a gap between its
/// characters (which the browser's own highlights would leave bare) and is drawn where its
/// carets are. A line between its first and last spans the characters on it, at least a pixel
/// wide as natively (an empty line's newline); a last line it holds nothing of (it ends where
/// a line wraps) gets none.
fn mark_boxes(
    frame: &Frame,
    stops: &Stops,
    bytes: std::ops::Range<usize>,
) -> Vec<(f64, i64, f64)> {
    let (left, first) = spot(frame, stops, bytes.start);
    let (right, last) = spot(frame, stops, bytes.end);
    if first >= last {
        return vec![(left, first, right.max(left + 1.0))];
    }
    let mut lines: HashMap<i64, (f64, f64)> = HashMap::default();
    if let Some(rects) = dom_range(stops, bytes).and_then(|range| range.get_client_rects()) {
        for index in 0..rects.length() {
            let Some(rect) = rects.get(index).filter(|rect| rect.height() > 0.0) else {
                continue;
            };
            let held = lines
                .entry(frame.line_of(&rect))
                .or_insert((rect.left(), rect.right()));
            *held = (held.0.min(rect.left()), held.1.max(rect.right()));
        }
    }
    (first..=last)
        .filter_map(|line| {
            let span = lines.get(&line).copied();
            let from = match line == first {
                true => left,
                false => span?.0,
            };
            let to = match line == last {
                true => right,
                false => span.map_or(from, |span| span.1),
            };
            (line == first || to > from || line != last).then_some((from, line, to.max(from + 1.0)))
        })
        .collect()
}

fn boxes(model: &TextModel, frame: &Frame, stops: &Stops) -> Vec<(&'static str, String)> {
    let decor = model.decor.borrow();
    let local = |left: f64, top: f64, width: f64, height: f64| {
        format!(
            "left: {}; top: {}; width: {}; height: {};",
            css::px((left - frame.rect.left()) as f32),
            css::px((top - frame.rect.top()) as f32),
            css::px(width as f32),
            css::px(height as f32)
        )
    };
    let mut boxes = Vec::new();
    // A span's background is drawn here while the text is decorated (the span's own is hidden
    // by `run.rs`'s style), before the marks: the text is a stacking context then, and a mark
    // under it would go under an inline background too. So a mark over inline code shows, in
    // the order the native core paints: backgrounds, marks, glyphs.
    for (node, piece) in model.ordered() {
        let Piece::Run(_) = piece else {
            continue;
        };
        let Some(span) = crate::dom::dom(node).and_then(|dom| dom.dyn_into::<web_sys::HtmlElement>().ok())
        else {
            continue;
        };
        if !span.has_attribute(BACKGROUND) {
            continue;
        }
        let style = span.style();
        let color = style.get_property_value("background-color").unwrap_or_default();
        let radius = style.get_property_value("border-radius").unwrap_or_default();
        let rects = span.get_client_rects();
        for index in 0..rects.length() {
            let Some(rect) = rects.get(index) else {
                continue;
            };
            boxes.push((
                "beui-span-background",
                format!(
                    "{} background-color: {color}; border-radius: {radius};",
                    local(rect.left(), rect.top(), rect.width(), rect.height()),
                ),
            ));
        }
    }
    for mark in &decor.all_marks() {
        if mark.range.is_empty() {
            continue;
        }
        for (left, line, right) in mark_boxes(frame, stops, mark.range.clone()) {
            boxes.push((
                "beui-mark",
                format!(
                    "{} background-color: {};",
                    local(left, frame.line_top(line), right - left, frame.line),
                    css::color(mark.color)
                ),
            ));
        }
    }
    for caret in decor.carets.iter().chain(&decor.handles) {
        let (x, line) = spot(frame, stops, caret.at);
        let top = frame.line_top(line);
        let color = css::color(caret.color);
        if let Some(handle) = caret.handle {
            let area = Rect::from_min_size(
                pos2(
                    (x - frame.rect.left()) as f32,
                    (top - frame.rect.top()) as f32,
                ),
                vec2(caret.width, frame.line as f32),
            );
            let (shape, corners, angle) = handle_shape(area, handle);
            boxes.push((
                "beui-caret-handle",
                format!(
                    "left: {}; top: {}; width: {}; height: {}; border-radius: {}; transform: rotate({}rad); background-color: {color};",
                    css::px(shape.min.x),
                    css::px(shape.min.y),
                    css::px(shape.width()),
                    css::px(shape.height()),
                    css::corners(corners),
                    css::number(angle)
                ),
            ));
            continue;
        }
        boxes.push((
            if caret.blink {
                "beui-caret beui-caret-blink"
            } else {
                "beui-caret"
            },
            format!(
                "{} background-color: {color};",
                local(x, top, f64::from(caret.width), frame.line)
            ),
        ));
        if caret.flag {
            boxes.push((
                "beui-caret-flag",
                format!(
                    "{} background-color: {color};",
                    local(
                        x,
                        top - CARET_FLAG.y as f64,
                        CARET_FLAG.x as f64,
                        CARET_FLAG.y as f64
                    )
                ),
            ));
        }
    }
    boxes
}

fn restyle(node: NodeId, property: &'static str, value: Option<String>) -> bool {
    if crate::document::style_value(node, property) == value {
        return false;
    }
    crate::document::set_style(node, property, value);
    true
}

pub(crate) fn place(model: &TextModel) -> bool {
    let Some(frame) = frame(model.node) else {
        return false;
    };
    let stops = stops(model);
    let mut changed = false;
    for (node, piece) in model.ordered() {
        let Piece::Item(slot) = piece else {
            continue;
        };
        let TextSlot::Anchored(at) = untrack(|| slot.get()) else {
            continue;
        };
        let spot = at.map(|at| spot(&frame, &stops, at));
        changed |= restyle(node, "display", spot.is_none().then(|| "none".to_owned()));
        let (left, top, height) = match spot {
            Some((x, line)) => (
                Some(css::px((x - frame.rect.left()) as f32)),
                Some(css::px((frame.line_top(line) - frame.rect.top()) as f32)),
                Some(css::px(frame.line as f32)),
            ),
            None => (None, None, None),
        };
        changed |= restyle(node, "left", left);
        changed |= restyle(node, "top", top);
        changed |= restyle(node, "height", height);
    }
    changed
}

pub(crate) fn decorate(model: &Rc<TextModel>) -> bool {
    let Some(frame) = frame(model.node) else {
        return false;
    };
    let stops = stops(model);
    if apply_hang(model, &frame, &stops) {
        return true;
    }
    let wanted = boxes(model, &frame, &stops);
    let mut decor = model.decor.borrow_mut();
    if decor.shown.boxes == wanted {
        return false;
    }
    let nodes = wanted
        .iter()
        .map(|(class, style)| {
            let node = Node::element("div");
            node.set_attribute("class", Some((*class).to_owned()));
            node.set_attribute("style", Some(style.clone()));
            node
        })
        .collect();
    decor.shown.boxes = wanted;
    drop(decor);
    if let Some((host, slot)) = model.overlay.borrow().as_ref() {
        host.fill(*slot, nodes);
    }
    false
}

pub(crate) fn decorated() -> bool {
    super::MODELS.with(|models| {
        models
            .borrow()
            .values()
            .any(|model| model.overlay.borrow().is_some())
    })
}

pub(crate) fn bounds(node: NodeId, rect: Rect) -> Rect {
    let (Some(model), Some(frame)) = (model(node), frame(node)) else {
        return rect;
    };
    let stops = stops(&model);
    let mut right = frame.rect.left();
    let first = stops.iter().find(|stop| matches!(stop, Stop::Char { .. }));
    let last = stops
        .iter()
        .rev()
        .find(|stop| matches!(stop, Stop::Char { .. }));
    if let (
        Some(Stop::Char {
            node: start,
            utf16: from,
            ..
        }),
        Some(Stop::Char {
            node: end,
            utf16: to,
            ch,
            ..
        }),
    ) = (first, last)
        && let Some(range) =
            crate::dom::document().and_then(|document| document.create_range().ok())
        && range.set_start(start, *from).is_ok()
        && range.set_end(end, to + ch.len_utf16() as u32).is_ok()
        && let Some(rects) = range.get_client_rects()
    {
        for index in 0..rects.length() {
            if let Some(line) = rects.get(index) {
                right = right.max(line.right());
            }
        }
    }
    for stop in &stops {
        if let Stop::Atom { node, .. } = stop
            && let Some(element) = node.dyn_ref::<web_sys::Element>()
        {
            right = right.max(element.get_bounding_client_rect().right());
        }
    }
    let right = right + frame.rect.right() - frame.right;
    Rect::from_min_size(
        rect.min,
        vec2((right - frame.rect.left()) as f32, rect.height()),
    )
}

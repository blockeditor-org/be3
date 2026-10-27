use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use beui_html::Node;
use beui_tree::reactive::Prop;
use beui_tree::reactive::{ChildList, IntoSlotHost, SlotHost, SlotId};
use beui_types::Color;
use beui_types::text::{FontId, TextCaret, TextGlyph, TextMark};
use reactive::{Get, Set, Signal, create_signal, on_cleanup};

use crate::document::element;
use crate::node::NodeId;

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub(crate) use web::{bounds, caret_rect, decorated, index_at, inline_at, lines, measure_line};

pub const OBJECT_BYTES: usize = '\u{FFFC}'.len_utf8();

pub const LINE_HEIGHT: f32 = 1.15;

pub fn line_height(size: f32) -> f32 {
    (size * LINE_HEIGHT).max(1.0)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextSlot {
    Anchored,
    Inline(usize),
}

impl TextSlot {
    pub fn inline(text: Option<&str>) -> Self {
        TextSlot::Inline(text.map_or(OBJECT_BYTES, str::len))
    }

    pub fn bytes(self) -> usize {
        match self {
            TextSlot::Anchored => 0,
            TextSlot::Inline(bytes) => bytes,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Segment {
    Plain(Range<usize>),
    Glyph {
        range: Range<usize>,
        glyph: String,
        font: FontId,
        color: Color,
    },
}

impl Segment {
    pub fn range(&self) -> Range<usize> {
        match self {
            Segment::Plain(range) | Segment::Glyph { range, .. } => range.clone(),
        }
    }

    fn same_shape(&self, other: &Segment) -> bool {
        match (self, other) {
            (Segment::Plain(_), Segment::Plain(_)) => true,
            (
                Segment::Glyph {
                    glyph, font, color, ..
                },
                Segment::Glyph {
                    glyph: other_glyph,
                    font: other_font,
                    color: other_color,
                    ..
                },
            ) => glyph == other_glyph && font == other_font && color == other_color,
            _ => false,
        }
    }
}

fn floor_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

pub fn segments(text: &str, start: usize, glyphs: &[TextGlyph]) -> Vec<Segment> {
    let end = start + text.len();
    let mut cuts: Vec<(Range<usize>, &TextGlyph)> = glyphs
        .iter()
        .filter(|glyph| glyph.range.start >= start && glyph.range.start < end)
        .filter_map(|glyph| {
            let from = glyph.range.start - start;
            let to = floor_boundary(text, glyph.range.end.min(end) - start);
            (text.is_char_boundary(from) && from < to).then_some((from..to, glyph))
        })
        .collect();
    cuts.sort_by_key(|(range, _)| range.start);
    let mut out = Vec::new();
    let mut at = 0;
    for (range, glyph) in cuts {
        if range.start < at {
            continue;
        }
        if range.start > at {
            out.push(Segment::Plain(at..range.start));
        }
        at = range.end;
        out.push(Segment::Glyph {
            range,
            glyph: glyph.glyph.clone(),
            font: glyph.font,
            color: glyph.color,
        });
    }
    if at < text.len() || out.is_empty() {
        out.push(Segment::Plain(at..text.len()));
    }
    out
}

struct Held {
    segment: Segment,
    text: Node,
    outer: Option<Node>,
}

pub struct Run {
    source: Prop<String>,
    host: Rc<dyn SlotHost<Node>>,
    slot: Option<SlotId>,
    held: RefCell<Vec<Held>>,
    start: Cell<usize>,
    text: RefCell<String>,
}

impl Run {
    pub fn new(holder: &Node, source: Prop<String>, glyphs: bool) -> Rc<Self> {
        let host = IntoSlotHost::<Node>::into_slot_host(holder);
        let (slot, held) = match glyphs {
            true => (Some(host.open()), Vec::new()),
            false => {
                let text = Node::text("");
                host.append(text.clone());
                let held = Held {
                    segment: Segment::Plain(0..0),
                    text,
                    outer: None,
                };
                (None, vec![held])
            }
        };
        Rc::new(Self {
            source,
            host,
            slot,
            held: RefCell::new(held),
            start: Cell::new(0),
            text: RefCell::default(),
        })
    }

    fn render(&self, start: usize, text: String, glyphs: &[TextGlyph]) {
        let wanted = segments(&text, start, glyphs);
        self.start.set(start);
        let mut held = self.held.borrow_mut();
        let same = held.len() == wanted.len()
            && held
                .iter()
                .zip(&wanted)
                .all(|(held, wanted)| held.segment.same_shape(wanted));
        if same {
            for (held, wanted) in held.iter_mut().zip(wanted) {
                held.text.set_text(text[wanted.range()].to_owned());
                held.segment = wanted;
            }
        } else if let Some(slot) = self.slot {
            *held = wanted
                .into_iter()
                .map(|segment| glyph_node(&text, segment))
                .collect();
            let nodes = held
                .iter()
                .map(|held| held.outer.clone().unwrap_or_else(|| held.text.clone()))
                .collect();
            drop(held);
            self.host.fill(slot, nodes);
        }
        *self.text.borrow_mut() = text;
    }
}

fn glyph_node(text: &str, segment: Segment) -> Held {
    let run = Node::text(&text[segment.range()]);
    let outer = match &segment {
        Segment::Plain(_) => None,
        Segment::Glyph {
            glyph, font, color, ..
        } => {
            let outer = Node::element("span");
            outer.set_attribute("class", Some("beui-glyph".to_owned()));
            outer.set_attribute("data-glyph", Some(glyph.clone()));
            outer.set_attribute(
                "style",
                Some(format!(
                    "--beui-glyph-font: {}; --beui-glyph-color: {};",
                    crate::css::font(*font),
                    crate::css::color(*color)
                )),
            );
            IntoSlotHost::<Node>::into_slot_host(&outer).append(run.clone());
            Some(outer)
        }
    };
    Held {
        segment,
        text: run,
        outer,
    }
}

#[derive(Clone)]
pub enum Piece {
    Run(Rc<Run>),
    Item(Prop<TextSlot>),
    Skip { newline: bool },
}

#[derive(Default)]
pub(crate) struct Decor {
    pub(crate) hang: usize,
    pub(crate) marks: Vec<TextMark>,
    pub(crate) carets: Vec<TextCaret>,
    pub(crate) font: Option<FontId>,
    #[cfg(target_arch = "wasm32")]
    pub(crate) shown: web::Shown,
}

pub struct TextModel {
    pub(crate) node: NodeId,
    host: Rc<dyn SlotHost<NodeId>>,
    order: RefCell<ChildList<NodeId>>,
    slots: RefCell<Vec<(SlotId, SlotId)>>,
    pieces: RefCell<HashMap<NodeId, Piece>>,
    single: RefCell<Option<Rc<Run>>>,
    structure: Signal<u64>,
    revision: Cell<u64>,
    pub(crate) decor: RefCell<Decor>,
    pub(crate) overlay: RefCell<Option<(Rc<dyn SlotHost<Node>>, SlotId)>>,
}

thread_local! {
    static MODELS: RefCell<HashMap<NodeId, Rc<TextModel>>> = RefCell::default();
}

pub fn model(node: NodeId) -> Option<Rc<TextModel>> {
    MODELS.with(|models| models.borrow().get(&node).cloned())
}

pub fn revision(node: NodeId) -> u64 {
    model(node).map_or(0, |model| model.revision.get())
}

impl TextModel {
    pub fn new(node: NodeId) -> Rc<Self> {
        let model = Rc::new(Self {
            node,
            host: crate::tree::node_host(node),
            order: RefCell::default(),
            slots: RefCell::default(),
            pieces: RefCell::default(),
            single: RefCell::default(),
            structure: create_signal(0),
            revision: Cell::new(0),
            decor: RefCell::default(),
            overlay: RefCell::default(),
        });
        MODELS.with(|models| models.borrow_mut().insert(node, model.clone()));
        on_cleanup(move || {
            MODELS.with(|models| models.borrow_mut().remove(&node));
        });
        model
    }

    pub fn set_single(&self, run: Rc<Run>) {
        *self.single.borrow_mut() = Some(run);
    }

    pub fn insert(&self, node: NodeId, piece: Piece) {
        self.pieces.borrow_mut().insert(node, piece);
    }

    fn changed(&self) {
        self.structure
            .set(self.structure.get_untracked().wrapping_add(1));
    }

    pub fn append(&self, node: NodeId) {
        self.order.borrow_mut().push(node);
        self.host.append(node);
        self.changed();
    }

    pub fn open(&self) -> SlotId {
        let outer = self.host.open();
        let mirror = self.order.borrow_mut().open();
        self.slots.borrow_mut().push((outer, mirror));
        outer
    }

    pub fn fill(&self, slot: SlotId, nodes: Vec<NodeId>) {
        let mirror = self
            .slots
            .borrow()
            .iter()
            .find(|(outer, _)| *outer == slot)
            .map(|(_, mirror)| *mirror);
        if let Some(mirror) = mirror {
            let mut order = self.order.borrow_mut();
            order.fill(mirror, nodes.clone());
            let kept: std::collections::HashSet<NodeId> = order.iter().copied().collect();
            self.pieces
                .borrow_mut()
                .retain(|node, _| kept.contains(node));
        }
        self.host.fill(slot, nodes);
        self.changed();
    }

    pub(crate) fn ordered(&self) -> Vec<(NodeId, Piece)> {
        if let Some(run) = self.single.borrow().clone() {
            return vec![(self.node, Piece::Run(run))];
        }
        let pieces = self.pieces.borrow();
        self.order
            .borrow()
            .iter()
            .filter_map(|node| Some((*node, pieces.get(node)?.clone())))
            .collect()
    }

    pub fn render(&self, glyphs: &[TextGlyph]) {
        self.structure.get();
        let mut at = 0;
        for (_, piece) in self.ordered() {
            match piece {
                Piece::Run(run) => {
                    let text = run.source.get();
                    let len = text.len();
                    run.render(at, text, glyphs);
                    at += len;
                }
                Piece::Item(slot) => at += slot.get().bytes(),
                Piece::Skip { .. } => {}
            }
        }
        self.bump();
    }

    pub(crate) fn bump(&self) {
        self.revision.set(self.revision.get().wrapping_add(1));
    }

    pub fn decorate(self: &Rc<Self>) {
        if self.overlay.borrow().is_some() {
            return;
        }
        let overlay = Node::element("div");
        overlay.set_attribute("class", Some("beui-text-decor".to_owned()));
        overlay.set_attribute("aria-hidden", Some("true".to_owned()));
        if let Some(text) = element(self.node) {
            IntoSlotHost::<Node>::into_slot_host(&text).append(overlay.clone());
        }
        let host = IntoSlotHost::<Node>::into_slot_host(&overlay);
        let slot = host.open();
        *self.overlay.borrow_mut() = Some((host, slot));
        #[cfg(target_arch = "wasm32")]
        {
            let model = Rc::downgrade(self);
            crate::frame::on_layout(Rc::new(move || {
                model.upgrade().is_some_and(|model| web::decorate(&model))
            }));
        }
    }

    pub fn set_hang(&self, hang: usize) {
        self.decor.borrow_mut().hang = hang;
    }

    pub fn set_marks(&self, marks: Vec<TextMark>) {
        self.decor.borrow_mut().marks = marks;
    }

    pub fn set_carets(&self, carets: Vec<TextCaret>) {
        self.decor.borrow_mut().carets = carets;
    }

    pub fn set_font(&self, font: FontId) {
        self.decor.borrow_mut().font = Some(font);
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Stop<D> {
    Char {
        byte: usize,
        ch: char,
        node: D,
        utf16: u32,
    },
    Atom {
        byte: usize,
        len: usize,
        node: D,
        newline: bool,
    },
}

impl<D> Stop<D> {
    pub fn byte(&self) -> usize {
        match self {
            Stop::Char { byte, .. } | Stop::Atom { byte, .. } => *byte,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Stop::Char { ch, .. } => ch.len_utf8(),
            Stop::Atom { len, .. } => *len,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn breaks(&self) -> bool {
        match self {
            Stop::Char { ch, .. } => *ch == '\n',
            Stop::Atom { newline, .. } => *newline,
        }
    }
}

pub fn text_stops<D: Clone>(byte: usize, text: &str, node: &D, stops: &mut Vec<Stop<D>>) {
    let mut utf16 = 0;
    for (offset, ch) in text.char_indices() {
        stops.push(Stop::Char {
            byte: byte + offset,
            ch,
            node: node.clone(),
            utf16,
        });
        utf16 += ch.len_utf16() as u32;
    }
}

pub fn after<D>(stops: &[Stop<D>], index: usize) -> Option<&Stop<D>> {
    stops
        .iter()
        .find(|stop| !stop.is_empty() && stop.byte() == index)
}

pub fn before<D>(stops: &[Stop<D>], index: usize) -> Option<&Stop<D>> {
    stops
        .iter()
        .rev()
        .find(|stop| !stop.is_empty() && stop.byte() + stop.len() == index)
}

pub fn byte_at<D>(stops: &[Stop<D>], same: impl Fn(&D) -> bool, utf16: u32) -> Option<usize> {
    let mut last = None;
    for stop in stops {
        if let Stop::Char {
            byte,
            ch,
            node,
            utf16: at,
        } = stop
            && same(node)
        {
            if *at >= utf16 {
                return Some(*byte);
            }
            last = Some(byte + ch.len_utf8());
        }
    }
    last
}

#[cfg(test)]
mod tests;

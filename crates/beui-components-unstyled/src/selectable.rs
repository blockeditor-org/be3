use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui::reactive::{
    Callback, Child, Interactive, NodeRef, Prop, copy_text, create_effect, set_component_state,
    untrack, with_document,
};
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::geometry::{Pos2, Rect, Vec2};
use beui_core::input::{Key, KeyPress, PointerPress};
use beui_core::node::NodeId;
use beui_core::rich::{CaretHandle, handle_center};
use beui_macros::{component, view};

const HANDLE_HIT_RADIUS: f32 = 24.0;
const CARET_WIDTH: f32 = 2.0;

#[derive(Clone, Copy, PartialEq, Debug)]
struct Caret {
    text: NodeId,
    index: usize,
}

#[derive(Clone, Copy)]
struct Grab {
    fixed: Caret,
    offset: Vec2,
}

#[derive(Clone, Default)]
pub struct SelectableState(Rc<Inner>);

#[derive(Default)]
struct Inner {
    region: NodeRef,
    anchor: Cell<Option<Caret>>,
    focus: Cell<Option<Caret>>,
    color: Cell<Color32>,
    handle_color: Cell<Color32>,
    touch: Cell<bool>,
    grab: Cell<Option<Grab>>,
    painted: RefCell<Vec<NodeId>>,
    handled: RefCell<Vec<NodeId>>,
}

impl SelectableState {
    fn texts(&self, document: &Document) -> Vec<NodeId> {
        self.0
            .region
            .try_get()
            .map_or_else(Vec::new, |region| document.texts_within(region))
    }

    fn caret_at(&self, document: &Document, pos: Pos2) -> Option<Caret> {
        let text = self
            .texts(document)
            .into_iter()
            .filter_map(|text| Some((text, distance(document.node_rect(text)?, pos))))
            .min_by(|(_, left), (_, right)| left.total_cmp(right))?
            .0;
        let index = document.text_index_at(text, pos)?;
        Some(Caret { text, index })
    }

    fn word_at(&self, document: &Document, pos: Pos2) -> Option<(Caret, Caret)> {
        let caret = self.caret_at(document, pos)?;
        let (start, end) = word_bounds(document.text(caret.text), caret.index);
        Some((
            Caret {
                text: caret.text,
                index: start,
            },
            Caret {
                text: caret.text,
                index: end,
            },
        ))
    }

    fn ordered(&self, texts: &[NodeId]) -> Option<(Caret, Caret, usize, usize)> {
        let (anchor, focus) = (self.0.anchor.get()?, self.0.focus.get()?);
        let at = |caret: Caret| texts.iter().position(|text| *text == caret.text);
        let (from, to) = (at(anchor)?, at(focus)?);
        match (from, anchor.index) <= (to, focus.index) {
            true => Some((anchor, focus, from, to)),
            false => Some((focus, anchor, to, from)),
        }
    }

    fn selected(&self, document: &Document, pos: Pos2) -> bool {
        let texts = self.texts(document);
        let (Some((start, end, from, to)), Some(caret)) =
            (self.ordered(&texts), self.caret_at(document, pos))
        else {
            return false;
        };
        let Some(at) = texts.iter().position(|text| *text == caret.text) else {
            return false;
        };
        (from, start.index) <= (at, caret.index)
            && (at, caret.index) <= (to, end.index)
            && (start != end)
    }

    fn ranges(&self, document: &Document) -> Vec<(NodeId, std::ops::Range<usize>)> {
        let texts = self.texts(document);
        let Some((start, end, from, to)) = self.ordered(&texts) else {
            return Vec::new();
        };
        texts[from..=to]
            .iter()
            .map(|text| {
                let begin = if *text == start.text { start.index } else { 0 };
                let finish = match *text == end.text {
                    true => end.index,
                    false => document.text(*text).len(),
                };
                (*text, begin..finish)
            })
            .filter(|(_, range)| !range.is_empty())
            .collect()
    }

    fn handles(&self, document: &Document) -> Vec<(CaretHandle, Caret)> {
        if !self.0.touch.get() {
            return Vec::new();
        }
        let texts = self.texts(document);
        match self.ordered(&texts) {
            Some((start, end, ..)) if start != end => {
                vec![(CaretHandle::Start, start), (CaretHandle::End, end)]
            }
            _ => Vec::new(),
        }
    }

    fn handle_at(&self, document: &Document, pos: Pos2) -> Option<(CaretHandle, Caret)> {
        self.handles(document)
            .into_iter()
            .filter_map(|(handle, caret)| {
                let rect = document.text_caret_rect(caret.text, caret.index, CARET_WIDTH)?;
                if pos.y < rect.max.y {
                    return None;
                }
                let distance = (pos.to_vec2() - handle_center(rect, handle)).length();
                (distance <= HANDLE_HIT_RADIUS).then_some((handle, caret, distance))
            })
            .min_by(|left, right| left.2.total_cmp(&right.2))
            .map(|(handle, caret, _)| (handle, caret))
    }

    fn paint(&self, document: &mut Document) {
        let ranges = self.ranges(document);
        let color = self.0.color.get();
        let previous = std::mem::take(&mut *self.0.painted.borrow_mut());
        for text in previous {
            if document.contains(text) && !ranges.iter().any(|(held, _)| *held == text) {
                document.set_text_selection(text, None);
            }
        }
        for (text, range) in &ranges {
            document.set_text_selection(*text, Some((range.clone(), color)));
        }
        *self.0.painted.borrow_mut() = ranges.into_iter().map(|(text, _)| text).collect();

        let handles = self.handles(document);
        let handle_color = self.0.handle_color.get();
        let previous = std::mem::take(&mut *self.0.handled.borrow_mut());
        for text in previous {
            if document.contains(text) {
                document.set_text_selection_handles(text, Vec::new());
            }
        }
        let mut handled: Vec<NodeId> = Vec::new();
        for (_, caret) in &handles {
            if !handled.contains(&caret.text) {
                handled.push(caret.text);
            }
        }
        for text in &handled {
            let on_text = handles
                .iter()
                .filter(|(_, caret)| caret.text == *text)
                .map(|(handle, caret)| (caret.index, *handle, handle_color))
                .collect();
            document.set_text_selection_handles(*text, on_text);
        }
        *self.0.handled.borrow_mut() = handled;
    }

    fn select(&self, document: &mut Document, anchor: Option<Caret>, focus: Option<Caret>) {
        self.0.anchor.set(anchor);
        self.0.focus.set(focus);
        self.paint(document);
    }

    fn select_all(&self, document: &mut Document) {
        let texts = self.texts(document);
        let (Some(first), Some(last)) = (texts.first(), texts.last()) else {
            return;
        };
        let anchor = Caret {
            text: *first,
            index: 0,
        };
        let focus = Caret {
            text: *last,
            index: document.text(*last).len(),
        };
        self.select(document, Some(anchor), Some(focus));
    }

    fn select_word(&self, document: &mut Document, pos: Pos2) {
        let Some((start, end)) = self.word_at(document, pos) else {
            return;
        };
        self.0.touch.set(true);
        self.select(document, Some(start), Some(end));
    }

    pub fn selected_text(&self, document: &Document) -> String {
        let mut copied = String::new();
        let mut above: Option<Rect> = None;
        for (text, range) in self.ranges(document) {
            let rect = document.node_rect(text).unwrap_or(Rect::NOTHING);
            if let Some(above) = above {
                copied.push(match rect.top() >= above.bottom() - 1.0 {
                    true => '\n',
                    false => ' ',
                });
            }
            copied.push_str(document.text(text).get(range).unwrap_or_default());
            above = Some(rect);
        }
        copied
    }
}

fn distance(rect: Rect, pos: Pos2) -> f32 {
    let x = (rect.left() - pos.x).max(pos.x - rect.right()).max(0.0);
    let y = (rect.top() - pos.y).max(pos.y - rect.bottom()).max(0.0);
    y * 1000.0 + x
}

fn word_bounds(text: &str, index: usize) -> (usize, usize) {
    let index = index.min(text.len());
    let word =
        |character: char| character.is_alphanumeric() || character == '_' || character == '\'';
    let after = text[index..].chars().next();
    let before = text[..index].chars().next_back();
    let at_word = after.is_some_and(word) || before.is_some_and(word);
    if !at_word {
        return match after {
            Some(character) => (index, index + character.len_utf8()),
            None => match before {
                Some(character) => (index - character.len_utf8(), index),
                None => (index, index),
            },
        };
    }
    let start = text[..index]
        .char_indices()
        .rev()
        .take_while(|(_, character)| word(*character))
        .last()
        .map_or(index, |(at, _)| at);
    let end = text[index..]
        .char_indices()
        .take_while(|(_, character)| word(*character))
        .last()
        .map_or(index, |(at, character)| index + at + character.len_utf8());
    (start, end)
}

fn key(state: &SelectableState, press: KeyPress) -> bool {
    if !press.pressed || !press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    match press.key {
        Key::C => {
            let text = with_document(|document| state.selected_text(document));
            if text.is_empty() {
                return false;
            }
            copy_text(text);
            true
        }
        Key::A => {
            with_document(|document| state.select_all(document));
            true
        }
        _ => false,
    }
}

pub fn copy_selection(state: &SelectableState) {
    let text = with_document(|document| state.selected_text(document));
    if !text.is_empty() {
        copy_text(text);
    }
}

pub fn select_all(state: &SelectableState) {
    with_document(|document| state.select_all(document));
}

pub fn selectable_text(document: &Document, selectable: NodeId) -> String {
    document
        .component_state::<SelectableState>(selectable)
        .selected_text(document)
}

fn press(state: &SelectableState, focusable: &NodeRef, press: PointerPress) {
    with_document(|document| {
        if press.touch {
            state.0.grab.set(None);
            let Some((handle, moving)) = state.handle_at(document, press.pos) else {
                return;
            };
            let texts = state.texts(document);
            let Some((start, end, ..)) = state.ordered(&texts) else {
                return;
            };
            let fixed = match handle {
                CaretHandle::Start => end,
                _ => start,
            };
            let offset = document
                .text_caret_rect(moving.text, moving.index, CARET_WIDTH)
                .map_or(Vec2::ZERO, |caret| press.pos - caret.min);
            state.0.grab.set(Some(Grab { fixed, offset }));
            return;
        }
        if let Some(target) = focusable.try_get() {
            document.focus_focusable(target);
        }
        state.0.touch.set(false);
        let caret = state.caret_at(document, press.pos);
        let anchor = match press.modifiers.shift {
            true => state.0.anchor.get().or(caret),
            false => caret,
        };
        state.select(document, anchor, caret);
    });
}

fn drag(state: &SelectableState, press: PointerPress) {
    with_document(|document| {
        if let Some(grab) = state.0.grab.get() {
            if let Some(caret) = state.caret_at(document, press.pos - grab.offset) {
                state.select(document, Some(grab.fixed), Some(caret));
            }
            return;
        }
        if press.touch {
            return;
        }
        let caret = state.caret_at(document, press.pos);
        if caret.is_some() {
            let anchor = state.0.anchor.get();
            state.select(document, anchor, caret);
        }
    });
}

fn tap(state: &SelectableState, on_menu: &Callback<Pos2>, press: PointerPress) {
    if !press.touch {
        return;
    }
    if state.0.grab.take().is_some() {
        return;
    }
    with_document(|document| {
        if press.clicks >= 2 {
            state.select_word(document, press.pos);
            return;
        }
        if state.selected(document, press.pos) {
            on_menu.call(press.pos);
            return;
        }
        state.0.touch.set(false);
        state.select(document, None, None);
    });
}

fn long_press(state: &SelectableState, press: PointerPress) {
    if !press.touch {
        return;
    }
    with_document(|document| {
        if !state.selected(document, press.pos) {
            state.select_word(document, press.pos);
        }
    });
}

#[component]
pub fn Selectable(
    children: Child,
    #[prop(default = SelectableState::default())] state: SelectableState,
    #[prop(default = Color32::from_rgba_unmultiplied(80, 140, 255, 90))] color: Prop<Color32>,
    #[prop(default = Color32::from_rgb(80, 140, 255))] handle_color: Prop<Color32>,
    on_menu: Callback<Pos2>,
) -> NodeId {
    set_component_state(state.clone());
    state.0.region.fill(children);
    let focusable = NodeRef::new();
    let painted = state.clone();
    create_effect(move || {
        let color = color.get();
        let handle_color = handle_color.get();
        untrack(|| {
            painted.0.color.set(color);
            painted.0.handle_color.set(handle_color);
            with_document(|document| painted.paint(document));
        });
    });
    let (pressed, dragged, keyed, ancestor) =
        (state.clone(), state.clone(), state.clone(), state.clone());
    let (tapped, held, captured, released) =
        (state.clone(), state.clone(), state.clone(), state.clone());
    let focused = focusable.clone();
    view! {
        <Interactive
            focusable=true
            @node_ref=&focusable
            tab_stop=false
            on_key={move |press: KeyPress| key(&keyed, press)}
            on_ancestor_key={move |press: KeyPress| key(&ancestor, press)}
            claims_touch=false
            capture_at={move |pos: Pos2| {
                with_document(|document| captured.handle_at(document, pos).is_some())
            }}
            on_press={move |event: PointerPress| press(&pressed, &focused, event)}
            on_drag={move |event: PointerPress| drag(&dragged, event)}
            on_click_at={move |event: PointerPress| tap(&tapped, &on_menu, event)}
            on_secondary_press={move |event: PointerPress| long_press(&held, event)}
            on_active_change={move |active: bool| {
                if !active {
                    released.0.grab.set(None);
                }
            }}
        >
            {children}
        </Interactive>
    }
}

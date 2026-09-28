use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui::reactive::{
    Child, ClickCatcher, Focusable, NodeRef, Prop, copy_text, create_effect, set_component_state,
    untrack, with_document,
};
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::geometry::{Pos2, Rect};
use beui_core::input::{Key, KeyPress, PointerPress};
use beui_core::node::NodeId;
use beui_macros::{component, view};

#[derive(Clone, Copy, PartialEq, Debug)]
struct Caret {
    text: NodeId,
    index: usize,
}

#[derive(Clone, Default)]
pub struct SelectableState(Rc<Inner>);

#[derive(Default)]
struct Inner {
    region: NodeRef,
    anchor: Cell<Option<Caret>>,
    focus: Cell<Option<Caret>>,
    color: Cell<Color32>,
    painted: RefCell<Vec<NodeId>>,
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

    fn ordered(&self, texts: &[NodeId]) -> Option<(Caret, Caret, usize, usize)> {
        let (anchor, focus) = (self.0.anchor.get()?, self.0.focus.get()?);
        let at = |caret: Caret| texts.iter().position(|text| *text == caret.text);
        let (from, to) = (at(anchor)?, at(focus)?);
        match (from, anchor.index) <= (to, focus.index) {
            true => Some((anchor, focus, from, to)),
            false => Some((focus, anchor, to, from)),
        }
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

#[component]
pub fn Selectable(
    children: Child,
    #[prop(default = SelectableState::default())] state: SelectableState,
    #[prop(default = Color32::from_rgba_unmultiplied(80, 140, 255, 90))] color: Prop<Color32>,
) -> NodeId {
    set_component_state(state.clone());
    state.0.region.fill(children);
    let focusable = NodeRef::new();
    let painted = state.clone();
    create_effect(move || {
        let color = color.get();
        untrack(|| {
            painted.0.color.set(color);
            with_document(|document| painted.paint(document));
        });
    });
    let (pressed, dragged, keyed, ancestor) =
        (state.clone(), state.clone(), state.clone(), state.clone());
    let focused = focusable.clone();
    view! {
        <Focusable
            @node_ref=&focusable
            tab_stop=false
            on_key={move |press: KeyPress| key(&keyed, press)}
            on_ancestor_key={move |press: KeyPress| key(&ancestor, press)}
        >
            <ClickCatcher
                claims_touch=false
                on_press={move |press: PointerPress| {
                    if press.touch {
                        return;
                    }
                    with_document(|document| {
                        if let Some(target) = focused.try_get() {
                            document.focus_focusable(target);
                        }
                        let caret = pressed.caret_at(document, press.pos);
                        let anchor = match press.modifiers.shift {
                            true => pressed.0.anchor.get().or(caret),
                            false => caret,
                        };
                        pressed.select(document, anchor, caret);
                    });
                }}
                on_drag={move |press: PointerPress| {
                    if press.touch {
                        return;
                    }
                    with_document(|document| {
                        let caret = dragged.caret_at(document, press.pos);
                        if caret.is_some() {
                            let anchor = dragged.0.anchor.get();
                            dragged.select(document, anchor, caret);
                        }
                    });
                }}
            >
                {children}
            </ClickCatcher>
        </Focusable>
    }
}

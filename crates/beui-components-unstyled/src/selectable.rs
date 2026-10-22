use std::cell::{Cell, RefCell};

use beui::primitives::*;
use beui::reactive::*;
use beui_core::color::Color32;
use beui_core::geometry::{Pos2, Rect, Vec2};
use beui_core::input::{Key, KeyPress, PointerPress};
use beui_core::keys::{KeyAt, KeyPhase};
use beui_core::node::NodeId;
use beui_core::rich::{CaretHandle, handle_center, toolbar_anchor};
use beui_macros::{component, view};

const HANDLE_HIT_RADIUS: f32 = 24.0;
const CARET_WIDTH: f32 = 2.0;

#[derive(Clone, Copy, PartialEq, Debug)]
struct Caret {
    text: NodeId,
    index: usize,
}

#[derive(Clone, Copy, Default, PartialEq, Debug)]
enum Unit {
    #[default]
    Character,
    Word,
    Text,
}

#[derive(Clone, Copy)]
struct Grab {
    fixed: Caret,
    offset: Vec2,
}

#[derive(Clone, Copy)]
pub struct SelectableState(Stored<Inner>);

impl Default for SelectableState {
    fn default() -> Self {
        Self(store(Inner::default()))
    }
}

#[derive(Default)]
struct Inner {
    region: NodeRef,
    anchor: Cell<Option<Caret>>,
    focus: Cell<Option<Caret>>,
    color: Cell<Color32>,
    handle_color: Cell<Color32>,
    touch: Cell<bool>,
    grab: Cell<Option<Grab>>,
    unit: Cell<Unit>,
    origin: Cell<Option<(Caret, Caret)>>,
    painted: RefCell<Vec<NodeId>>,
    handled: RefCell<Vec<NodeId>>,
    copied: RefCell<String>,
}

impl SelectableState {
    fn texts(&self) -> Vec<NodeId> {
        self.0
            .value()
            .region
            .try_get()
            .map_or_else(Vec::new, |region| region.texts_within())
    }

    fn caret_at(&self, pos: Pos2) -> Option<Caret> {
        let text = self
            .texts()
            .into_iter()
            .filter_map(|text| Some((text, distance(placements().rect(text)?, pos))))
            .min_by(|(_, left), (_, right)| left.total_cmp(right))?
            .0;
        let index = text.text_index_at(pos)?;
        Some(Caret { text, index })
    }

    fn word_at(&self, pos: Pos2) -> Option<(Caret, Caret)> {
        let caret = self.caret_at(pos)?;
        let (start, end) = word_bounds(&caret.text.text(), caret.index);
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

    fn unit_at(&self, pos: Pos2, unit: Unit) -> Option<(Caret, Caret)> {
        match unit {
            Unit::Character => self.caret_at(pos).map(|caret| (caret, caret)),
            Unit::Word => self.word_at(pos),
            Unit::Text => {
                let caret = self.caret_at(pos)?;
                let end = caret.text.text().len();
                Some((
                    Caret {
                        text: caret.text,
                        index: 0,
                    },
                    Caret {
                        text: caret.text,
                        index: end,
                    },
                ))
            }
        }
    }

    fn toolbar_rect(&self) -> Option<Rect> {
        let texts = self.texts();
        let (start, end, ..) = self.ordered(&texts)?;
        if start == end {
            return None;
        }
        let first = start.text.caret_rect(start.index, CARET_WIDTH)?;
        let last = end.text.caret_rect(end.index, CARET_WIDTH)?;
        Some(toolbar_anchor(first.union(last)))
    }

    fn ordered(&self, texts: &[NodeId]) -> Option<(Caret, Caret, usize, usize)> {
        let (anchor, focus) = (self.0.value().anchor.get()?, self.0.value().focus.get()?);
        let at = |caret: Caret| texts.iter().position(|text| *text == caret.text);
        let (from, to) = (at(anchor)?, at(focus)?);
        match (from, anchor.index) <= (to, focus.index) {
            true => Some((anchor, focus, from, to)),
            false => Some((focus, anchor, to, from)),
        }
    }

    fn selected(&self, pos: Pos2) -> bool {
        let texts = self.texts();
        let (Some((start, end, from, to)), Some(caret)) =
            (self.ordered(&texts), self.caret_at(pos))
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

    fn ranges(&self) -> Vec<(NodeId, std::ops::Range<usize>)> {
        let texts = self.texts();
        let Some((start, end, from, to)) = self.ordered(&texts) else {
            return Vec::new();
        };
        texts[from..=to]
            .iter()
            .map(|text| {
                let begin = if *text == start.text { start.index } else { 0 };
                let finish = match *text == end.text {
                    true => end.index,
                    false => text.text().len(),
                };
                (*text, begin..finish)
            })
            .filter(|(_, range)| !range.is_empty())
            .collect()
    }

    fn handles(&self) -> Vec<(CaretHandle, Caret)> {
        if !self.0.value().touch.get() {
            return Vec::new();
        }
        let texts = self.texts();
        match self.ordered(&texts) {
            Some((start, end, ..)) if start != end => {
                vec![(CaretHandle::Start, start), (CaretHandle::End, end)]
            }
            _ => Vec::new(),
        }
    }

    fn handle_at(&self, pos: Pos2) -> Option<(CaretHandle, Caret)> {
        self.handles()
            .into_iter()
            .filter_map(|(handle, caret)| {
                let rect = caret.text.caret_rect(caret.index, CARET_WIDTH)?;
                if pos.y < rect.max.y {
                    return None;
                }
                let distance = (pos.to_vec2() - handle_center(rect, handle)).length();
                (distance <= HANDLE_HIT_RADIUS).then_some((handle, caret, distance))
            })
            .min_by(|left, right| left.2.total_cmp(&right.2))
            .map(|(handle, caret, _)| (handle, caret))
    }

    fn paint(&self) {
        let ranges = self.ranges();
        let color = self.0.value().color.get();
        let previous = std::mem::take(&mut *self.0.value().painted.borrow_mut());
        for text in previous {
            if text.exists() && !ranges.iter().any(|(held, _)| *held == text) {
                text.set_text_selection(None);
            }
        }
        for (text, range) in &ranges {
            text.set_text_selection(Some((range.clone(), color)));
        }
        *self.0.value().painted.borrow_mut() = ranges.into_iter().map(|(text, _)| text).collect();

        let handles = self.handles();
        let handle_color = self.0.value().handle_color.get();
        let previous = std::mem::take(&mut *self.0.value().handled.borrow_mut());
        for text in previous {
            if text.exists() {
                text.set_text_selection_handles(Vec::new());
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
            text.set_text_selection_handles(on_text);
        }
        *self.0.value().handled.borrow_mut() = handled;
        *self.0.value().copied.borrow_mut() = self.copied();
    }

    fn select(&self, anchor: Option<Caret>, focus: Option<Caret>) {
        self.0.value().anchor.set(anchor);
        self.0.value().focus.set(focus);
        self.paint();
    }

    fn select_all(&self) {
        let texts = self.texts();
        let (Some(first), Some(last)) = (texts.first(), texts.last()) else {
            return;
        };
        let anchor = Caret {
            text: *first,
            index: 0,
        };
        let focus = Caret {
            text: *last,
            index: last.text().len(),
        };
        self.select(Some(anchor), Some(focus));
    }

    fn select_word(&self, pos: Pos2) {
        let Some((start, end)) = self.word_at(pos) else {
            return;
        };
        self.0.value().touch.set(true);
        self.select(Some(start), Some(end));
    }

    pub fn selected_text(&self) -> String {
        self.0.value().copied.borrow().clone()
    }

    fn copied(&self) -> String {
        let mut copied = String::new();
        let mut above: Option<Rect> = None;
        for (text, range) in self.ranges() {
            let rect = placements().rect(text).unwrap_or(Rect::NOTHING);
            if let Some(above) = above {
                copied.push(match rect.top() >= above.bottom() - 1.0 {
                    true => '\n',
                    false => ' ',
                });
            }
            copied.push_str(text.text().get(range).unwrap_or_default());
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

fn key(state: SelectableState, press: KeyPress) -> bool {
    if !press.pressed || !press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    match press.key {
        Key::C => {
            let text = state.selected_text();
            if text.is_empty() {
                return false;
            }
            copy_text(text);
            true
        }
        Key::A => {
            state.select_all();
            true
        }
        _ => false,
    }
}

pub fn copy_selection(state: SelectableState) {
    let text = state.selected_text();
    if !text.is_empty() {
        copy_text(text);
    }
}

pub fn select_all(state: SelectableState) {
    state.select_all();
}

pub fn selectable_text(tree: &impl Inspect, selectable: NodeId) -> String {
    tree.component_state::<SelectableState>(selectable)
        .selected_text()
}

fn precedes(texts: &[NodeId], left: Caret, right: Caret) -> bool {
    let at = |caret: Caret| texts.iter().position(|text| *text == caret.text);
    (at(left), left.index) < (at(right), right.index)
}

fn press(
    state: SelectableState,
    focusable: &NodeRef,
    on_toolbar: Callback<Option<Rect>>,
    press: PointerPress,
) {
    if press.touch {
        state.0.value().grab.set(None);
        let Some((handle, moving)) = state.handle_at(press.pos) else {
            return;
        };
        let texts = state.texts();
        let Some((start, end, ..)) = state.ordered(&texts) else {
            return;
        };
        let fixed = match handle {
            CaretHandle::Start => end,
            _ => start,
        };
        let offset = moving
            .text
            .caret_rect(moving.index, CARET_WIDTH)
            .map_or(Vec2::ZERO, |caret| press.pos - caret.min);
        state.0.value().grab.set(Some(Grab { fixed, offset }));
        on_toolbar.call(None);
        return;
    }
    if let Some(target) = focusable.try_get() {
        target.focus();
    }
    state.0.value().touch.set(false);
    on_toolbar.call(None);
    if press.modifiers.shift {
        let caret = state.caret_at(press.pos);
        let anchor = state.0.value().anchor.get().or(caret);
        state.0.value().unit.set(Unit::Character);
        state
            .0
            .value()
            .origin
            .set(anchor.map(|anchor| (anchor, anchor)));
        state.select(anchor, caret);
        return;
    }
    let unit = match press.clicks {
        0 | 1 => Unit::Character,
        2 => Unit::Word,
        _ => Unit::Text,
    };
    let origin = state.unit_at(press.pos, unit);
    state.0.value().unit.set(unit);
    state.0.value().origin.set(origin);
    state.select(origin.map(|(start, _)| start), origin.map(|(_, end)| end));
}

fn drag(state: SelectableState, press: PointerPress) {
    if let Some(grab) = state.0.value().grab.get() {
        if let Some(caret) = state.caret_at(press.pos - grab.offset) {
            state.select(Some(grab.fixed), Some(caret));
        }
        return;
    }
    if press.touch {
        return;
    }
    let (Some((start, end)), Some(reached)) = (
        state.0.value().origin.get(),
        state.unit_at(press.pos, state.0.value().unit.get()),
    ) else {
        return;
    };
    let texts = state.texts();
    let (anchor, focus) = match precedes(&texts, reached.0, start) {
        true => (end, reached.0),
        false => (start, reached.1),
    };
    state.select(Some(anchor), Some(focus));
}

fn show_toolbar(state: SelectableState, on_toolbar: Callback<Option<Rect>>) {
    on_toolbar.call(state.toolbar_rect());
}

fn tap(state: SelectableState, on_toolbar: Callback<Option<Rect>>, press: PointerPress) {
    if !press.touch {
        return;
    }
    if state.0.value().grab.take().is_some() {
        show_toolbar(state, on_toolbar);
        return;
    }
    if press.clicks >= 2 {
        state.select_word(press.pos);
        show_toolbar(state, on_toolbar);
        return;
    }
    if state.selected(press.pos) {
        show_toolbar(state, on_toolbar);
        return;
    }
    state.0.value().touch.set(false);
    state.select(None, None);
    on_toolbar.call(None);
}

fn long_press(state: SelectableState, on_toolbar: Callback<Option<Rect>>, press: PointerPress) {
    if !press.touch {
        return;
    }
    if !state.selected(press.pos) {
        state.select_word(press.pos);
    }
    show_toolbar(state, on_toolbar);
}

#[component]
pub fn Selectable(
    children: Child,
    #[prop(default = SelectableState::default())] state: SelectableState,
    #[prop(default = Color32::from_rgba_unmultiplied(80, 140, 255, 90))] color: Prop<Color32>,
    #[prop(default = Color32::from_rgb(80, 140, 255))] handle_color: Prop<Color32>,
    on_toolbar: Callback<Option<Rect>>,
) -> NodeId {
    set_component_state(state);
    state.0.value().region.fill(children);
    let focusable = NodeRef::new();
    create_effect(move || {
        let color = color.get();
        let handle_color = handle_color.get();
        untrack(|| {
            state.0.value().color.set(color);
            state.0.value().handle_color.set(handle_color);
            state.paint();
        });
    });
    on_key(KeyAt::Node(focusable), KeyPhase::Bubble, move |event| {
        event.press().is_some_and(|press| key(state, press))
    });
    view! {
        <Interactive
            focusable=true
            @node_ref=&focusable
            tab_stop=false
            claims_touch=false
            claim_at={move |(pos, _): (Pos2, beui_core::input::Modifiers)| {
                state.handle_at(pos).is_some()
            }}
            on_press={move |event: PointerPress| press(state, &focusable, on_toolbar, event)}
            on_drag={move |event: PointerPress| drag(state, event)}
            on_click_at={move |event: PointerPress| tap(state, on_toolbar, event)}
            on_secondary_press={move |event: PointerPress| long_press(state, on_toolbar, event)}
            on_active_change={move |active: bool| {
                if !active && state.0.value().grab.take().is_some() {
                    show_toolbar(state, on_toolbar);
                }
            }}
        >
            {children}
        </Interactive>
    }
}

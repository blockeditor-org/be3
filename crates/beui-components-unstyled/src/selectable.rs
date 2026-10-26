use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use beui::actions::Chord;
use beui::primitives::*;
use beui::reactive::*;
use beui::selection::{
    RowKey, RowWatcher, SelectableRows, SelectionItem, rows_of, scroll_toward, selection_items,
    texts_under,
};
use beui_core::rich::{handle_center, toolbar_anchor};
use beui_macros::{component, view};
use beui_view::reactive::KeyAt;
use beui_view::reactive::NodeId;
use beui_view::types::CaretHandle;
use beui_view::types::Color32;
use beui_view::types::KeyPhase;
use beui_view::types::{Key, KeyPress, PointerPress};
use beui_view::types::{Pos2, Rect, Vec2};

const HANDLE_HIT_RADIUS: f32 = 24.0;
const END: usize = usize::MAX;
const AUTOSCROLL_RATE: f32 = 0.5;

pub const COPY_KEY: Chord = Chord::ctrl(Key::C);
pub const SELECT_ALL_KEY: Chord = Chord::ctrl(Key::A);

#[derive(Clone, PartialEq, Debug)]
enum Spot {
    Text(NodeId),
    Row {
        list: NodeId,
        key: RowKey,
        part: usize,
    },
}

#[derive(Clone, PartialEq, Debug)]
struct Caret {
    spot: Spot,
    index: usize,
}

impl Caret {
    fn text(text: NodeId, index: usize) -> Self {
        Self {
            spot: Spot::Text(text),
            index,
        }
    }

    fn row(list: NodeId, key: RowKey, part: usize, index: usize) -> Self {
        Self {
            spot: Spot::Row { list, key, part },
            index,
        }
    }

    fn at(&self, index: usize) -> Self {
        Self {
            spot: self.spot.clone(),
            index,
        }
    }
}

type Order = (usize, usize, usize, usize);

type Ordered = (Caret, Caret, Order, Order);

type Bound = (RowKey, usize, usize);

#[derive(Clone, Copy, Default, PartialEq, Debug)]
enum Unit {
    #[default]
    Character,
    Word,
    Text,
}

#[derive(Clone)]
struct Grab {
    fixed: Caret,
    offset: Vec2,
}

#[derive(Clone)]
struct Coverage {
    from: Option<Bound>,
    to: Option<Bound>,
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
    anchor: RefCell<Option<Caret>>,
    focus: RefCell<Option<Caret>>,
    color: Cell<Color32>,
    handle_color: Cell<Color32>,
    caret_width: Cell<f32>,
    touch: Cell<bool>,
    grab: RefCell<Option<Grab>>,
    unit: Cell<Unit>,
    origin: RefCell<Option<(Caret, Caret)>>,
    painted: RefCell<Vec<NodeId>>,
    handled: RefCell<Vec<NodeId>>,
    covered: RefCell<HashMap<NodeId, Option<Coverage>>>,
    watcher: RefCell<Option<Rc<dyn RowWatcher>>>,
}

struct Watcher(SelectableState);

impl RowWatcher for Watcher {
    fn row_shown(&self, list: NodeId, row: NodeId, key: RowKey) {
        if self.0.0.try_value().is_some() {
            self.0.paint_row(list, row, &key);
        }
    }

    fn keys_changed(&self, list: NodeId) {
        if self.0.0.try_value().is_some() {
            self.0.clamp(list);
        }
    }
}

struct Candidate {
    text: NodeId,
    spot: Spot,
}

impl SelectableState {
    fn items(&self) -> Vec<SelectionItem> {
        let items = self
            .0
            .value()
            .region
            .try_get()
            .map_or_else(Vec::new, selection_items);
        for item in &items {
            if let SelectionItem::Rows(_, rows) = item {
                rows.watch(&self.watcher());
            }
        }
        items
    }

    fn watcher(&self) -> Rc<dyn RowWatcher> {
        let inner = self.0.value();
        let mut watcher = inner.watcher.borrow_mut();
        Rc::clone(watcher.get_or_insert_with(|| Rc::new(Watcher(*self)) as Rc<dyn RowWatcher>))
    }

    fn anchor(&self) -> Option<Caret> {
        self.0.value().anchor.borrow().clone()
    }

    fn focus(&self) -> Option<Caret> {
        self.0.value().focus.borrow().clone()
    }

    fn candidates(items: &[SelectionItem]) -> Vec<Candidate> {
        let mut found = Vec::new();
        for item in items {
            match item {
                SelectionItem::Text(text) => found.push(Candidate {
                    text: *text,
                    spot: Spot::Text(*text),
                }),
                SelectionItem::Rows(list, rows) => {
                    for (key, row) in rows.shown() {
                        for (part, text) in texts_under(row).into_iter().enumerate() {
                            found.push(Candidate {
                                text,
                                spot: Spot::Row {
                                    list: *list,
                                    key: key.clone(),
                                    part,
                                },
                            });
                        }
                    }
                }
            }
        }
        found
    }

    fn caret_at(&self, pos: Pos2) -> Option<Caret> {
        let items = self.items();
        let nearest = Self::candidates(&items)
            .into_iter()
            .filter_map(|candidate| {
                Some((
                    distance(measure(candidate.text).get_untracked()?, pos),
                    candidate,
                ))
            })
            .min_by(|(left, _), (right, _)| left.total_cmp(right))?
            .1;
        let index = measure_text(nearest.text)
            .get_untracked()?
            .index_at(pos)?
            .min(nearest.text.text().len());
        Some(Caret {
            spot: nearest.spot,
            index,
        })
    }

    fn resolve(caret: &Caret) -> Option<(NodeId, usize)> {
        let text = match &caret.spot {
            Spot::Text(text) => *text,
            Spot::Row { list, key, part } => {
                let (_, row) = rows_of(*list)?
                    .shown()
                    .into_iter()
                    .find(|(shown, _)| shown == key)?;
                let texts = texts_under(row);
                match *part {
                    END => *texts.last()?,
                    part => *texts.get(part)?,
                }
            }
        };
        let content = text.text();
        Some((text, floor(&content, caret.index)))
    }

    fn content(caret: &Caret) -> Option<String> {
        Self::resolve(caret).map(|(text, _)| text.text())
    }

    fn word_at(&self, pos: Pos2) -> Option<(Caret, Caret)> {
        let caret = self.caret_at(pos)?;
        let (start, end) = word_bounds(&Self::content(&caret)?, caret.index);
        Some((caret.at(start), caret.at(end)))
    }

    fn unit_at(&self, pos: Pos2, unit: Unit) -> Option<(Caret, Caret)> {
        match unit {
            Unit::Character => self.caret_at(pos).map(|caret| (caret.clone(), caret)),
            Unit::Word => self.word_at(pos),
            Unit::Text => {
                let caret = self.caret_at(pos)?;
                let end = Self::content(&caret)?.len();
                Some((caret.at(0), caret.at(end)))
            }
        }
    }

    fn caret_rect(&self, caret: &Caret) -> Option<Rect> {
        let (text, index) = Self::resolve(caret)?;
        measure_text(text)
            .get_untracked()?
            .caret_rect(index, self.0.value().caret_width.get())
    }

    fn caret_scale(caret: &Caret) -> f32 {
        Self::resolve(caret)
            .and_then(|(text, _)| measure_text(text).get_untracked())
            .map_or(1.0, |geometry| geometry.scale())
    }

    fn toolbar_rect(&self) -> Option<Rect> {
        let items = self.items();
        let (start, end, ..) = self.ordered(&items)?;
        if start == end {
            return None;
        }
        let first = self.caret_rect(&start)?;
        let last = self.caret_rect(&end)?;
        Some(toolbar_anchor(first.union(last), Self::caret_scale(&end)))
    }

    fn order(items: &[SelectionItem], caret: &Caret) -> Option<Order> {
        match &caret.spot {
            Spot::Text(text) => items
                .iter()
                .position(|item| matches!(item, SelectionItem::Text(held) if held == text))
                .map(|at| (at, 0, 0, caret.index)),
            Spot::Row { list, key, part } => {
                items.iter().enumerate().find_map(|(at, item)| match item {
                    SelectionItem::Rows(held, rows) if held == list => {
                        Some((at, rows.index_of(key)?, *part, caret.index))
                    }
                    _ => None,
                })
            }
        }
    }

    fn ordered(&self, items: &[SelectionItem]) -> Option<Ordered> {
        let (anchor, focus) = (self.anchor()?, self.focus()?);
        let (from, to) = (Self::order(items, &anchor)?, Self::order(items, &focus)?);
        match from <= to {
            true => Some((anchor, focus, from, to)),
            false => Some((focus, anchor, to, from)),
        }
    }

    fn selected(&self, pos: Pos2) -> bool {
        let items = self.items();
        let (Some((start, end, from, to)), Some(caret)) =
            (self.ordered(&items), self.caret_at(pos))
        else {
            return false;
        };
        let Some(at) = Self::order(&items, &caret) else {
            return false;
        };
        from <= at && at <= to && start != end
    }

    fn text_range(text: NodeId, ordered: &Ordered) -> Range<usize> {
        let (start, end, ..) = ordered;
        let begin = match start.spot == Spot::Text(text) {
            true => start.index,
            false => 0,
        };
        let finish = match end.spot == Spot::Text(text) {
            true => end.index,
            false => text.text().len(),
        };
        begin..finish
    }

    fn plain_ranges(
        items: &[SelectionItem],
        ordered: Option<&Ordered>,
    ) -> Vec<(NodeId, Range<usize>)> {
        let Some(ordered) = ordered.filter(|(start, end, ..)| start != end) else {
            return Vec::new();
        };
        let (_, _, from, to) = ordered;
        items
            .iter()
            .enumerate()
            .filter(|(at, _)| (from.0..=to.0).contains(at))
            .filter_map(|(_, item)| match item {
                SelectionItem::Text(text) => Some((*text, Self::text_range(*text, ordered))),
                SelectionItem::Rows(..) => None,
            })
            .filter(|(_, range)| !range.is_empty())
            .collect()
    }

    fn coverage(at: usize, ordered: Option<&Ordered>) -> Option<Coverage> {
        let (start, end, from, to) = ordered?;
        if start == end || at < from.0 || at > to.0 {
            return None;
        }
        let bound = |caret: &Caret, order: &Order| match &caret.spot {
            Spot::Row { key, part, .. } if order.0 == at => Some((key.clone(), *part, caret.index)),
            _ => None,
        };
        Some(Coverage {
            from: bound(start, from),
            to: bound(end, to),
        })
    }

    fn bounds(rows: &dyn SelectableRows, coverage: &Coverage) -> Option<(Order, Order)> {
        let place = |bound: &Bound| {
            rows.index_of(&bound.0)
                .map(|row| (0, row, bound.1, bound.2))
        };
        let from = match &coverage.from {
            Some(bound) => place(bound)?,
            None => (0, 0, 0, 0),
        };
        let to = match &coverage.to {
            Some(bound) => place(bound)?,
            None => (0, END, END, END),
        };
        Some((from, to))
    }

    fn row_range(
        rows: &dyn SelectableRows,
        coverage: &Coverage,
        row: usize,
        part: usize,
    ) -> Option<Range<usize>> {
        let ((_, from_row, from_part, from_index), (_, to_row, to_part, to_index)) =
            Self::bounds(rows, coverage)?;
        let at = (row, part);
        if at < (from_row, from_part) || at > (to_row, to_part) {
            return None;
        }
        let begin = match at == (from_row, from_part) {
            true => from_index,
            false => 0,
        };
        let finish = match at == (to_row, to_part) {
            true => to_index,
            false => END,
        };
        (begin < finish).then_some(begin..finish)
    }

    fn unbuilt_range(rows: &dyn SelectableRows, coverage: &Coverage, row: usize) -> Range<usize> {
        let Some(((_, from_row, from_part, from_index), (_, to_row, to_part, to_index))) =
            Self::bounds(rows, coverage)
        else {
            return 0..0;
        };
        let begin = match (from_row == row, from_part) {
            (true, 0) => from_index,
            (true, _) => END,
            (false, _) => 0,
        };
        let finish = match (to_row == row, to_part) {
            (true, 0) => to_index,
            _ => END,
        };
        begin..finish
    }

    fn paint_row(&self, list: NodeId, row: NodeId, key: &RowKey) {
        let coverage = self
            .0
            .value()
            .covered
            .borrow()
            .get(&list)
            .cloned()
            .flatten();
        let color = self.0.value().color.get();
        let rows = coverage.as_ref().and_then(|_| rows_of(list));
        let index = rows.as_ref().and_then(|rows| rows.index_of(key));
        for (part, text) in texts_under(row).into_iter().enumerate() {
            let range = match (&coverage, &rows, index) {
                (Some(coverage), Some(rows), Some(index)) => {
                    Self::row_range(rows.as_ref(), coverage, index, part)
                }
                _ => None,
            };
            text.set_text_selection(range.map(|range| (range, color)));
        }
    }

    fn paint(&self) {
        let items = self.items();
        let ordered = self.ordered(&items);
        let ranges = Self::plain_ranges(&items, ordered.as_ref());
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

        let covered = items
            .iter()
            .enumerate()
            .filter_map(|(at, item)| match item {
                SelectionItem::Rows(list, _) => Some((*list, Self::coverage(at, ordered.as_ref()))),
                SelectionItem::Text(_) => None,
            })
            .collect();
        *self.0.value().covered.borrow_mut() = covered;
        for item in &items {
            if let SelectionItem::Rows(list, rows) = item {
                for (key, row) in rows.shown() {
                    self.paint_row(*list, row, &key);
                }
            }
        }

        let handle_color = self.0.value().handle_color.get();
        let previous = std::mem::take(&mut *self.0.value().handled.borrow_mut());
        for text in previous {
            if text.exists() {
                text.set_text_selection_handles(Vec::new());
            }
        }
        let resolved: Vec<(CaretHandle, NodeId, usize)> = self
            .handles(&items)
            .iter()
            .filter_map(|(handle, caret)| {
                let (text, index) = Self::resolve(caret)?;
                Some((*handle, text, index))
            })
            .collect();
        let mut handled: Vec<NodeId> = Vec::new();
        for (_, text, _) in &resolved {
            if !handled.contains(text) {
                handled.push(*text);
            }
        }
        for text in &handled {
            let on_text = resolved
                .iter()
                .filter(|(_, held, _)| held == text)
                .map(|(handle, _, index)| (*index, *handle, handle_color))
                .collect();
            text.set_text_selection_handles(on_text);
        }
        *self.0.value().handled.borrow_mut() = handled;
    }

    fn handles(&self, items: &[SelectionItem]) -> Vec<(CaretHandle, Caret)> {
        if !self.0.value().touch.get() {
            return Vec::new();
        }
        match self.ordered(items) {
            Some((start, end, ..)) if start != end => {
                vec![(CaretHandle::Start, start), (CaretHandle::End, end)]
            }
            _ => Vec::new(),
        }
    }

    fn handle_at(&self, pos: Pos2) -> Option<(CaretHandle, Caret)> {
        let items = self.items();
        self.handles(&items)
            .into_iter()
            .filter_map(|(handle, caret)| {
                let rect = self.caret_rect(&caret)?;
                if pos.y < rect.max.y {
                    return None;
                }
                let center = handle_center(rect, handle, Self::caret_scale(&caret));
                let distance = (pos.to_vec2() - center).length();
                (distance <= HANDLE_HIT_RADIUS).then_some((handle, caret, distance))
            })
            .min_by(|left, right| left.2.total_cmp(&right.2))
            .map(|(handle, caret, _)| (handle, caret))
    }

    fn select(&self, anchor: Option<Caret>, focus: Option<Caret>) {
        *self.0.value().anchor.borrow_mut() = anchor;
        *self.0.value().focus.borrow_mut() = focus;
        self.paint();
    }

    fn clamp(&self, list: NodeId) {
        let Some(rows) = rows_of(list) else {
            return;
        };
        let (Some(anchor), Some(focus)) = (self.anchor(), self.focus()) else {
            return;
        };
        let items = self.items();
        let item_of = |caret: &Caret| {
            items.iter().position(|item| match (item, &caret.spot) {
                (SelectionItem::Text(text), Spot::Text(held)) => text == held,
                (SelectionItem::Rows(held, _), Spot::Row { list, .. }) => held == list,
                _ => false,
            })
        };
        let was = |caret: &Caret| match &caret.spot {
            Spot::Row {
                list: held,
                key,
                part,
            } if *held == list => Some((rows.previous_index(key)?, *part, caret.index)),
            _ => None,
        };
        let starts = |caret: &Caret, other: &Caret| match (item_of(caret), item_of(other)) {
            (Some(left), Some(right)) if left != right => left < right,
            _ => match (was(caret), was(other)) {
                (Some(left), Some(right)) => left <= right,
                _ => true,
            },
        };
        let moved = |caret: &Caret, other: &Caret| -> Option<Option<Caret>> {
            let Spot::Row {
                list: held, key, ..
            } = &caret.spot
            else {
                return None;
            };
            if *held != list || rows.index_of(key).is_some() {
                return None;
            }
            Some(
                rows.replacement(key, starts(caret, other))
                    .map(|(key, after)| match after {
                        true => Caret::row(list, key, 0, 0),
                        false => Caret::row(list, key, END, END),
                    }),
            )
        };
        let (moved_anchor, moved_focus) = (moved(&anchor, &focus), moved(&focus, &anchor));
        if moved_anchor.is_none() && moved_focus.is_none() {
            self.paint();
            return;
        }
        match (
            moved_anchor.unwrap_or(Some(anchor)),
            moved_focus.unwrap_or(Some(focus)),
        ) {
            (Some(anchor), Some(focus)) => self.select(Some(anchor), Some(focus)),
            _ => self.select(None, None),
        }
    }

    fn select_all(&self) {
        let items = self.items();
        let first = items.iter().find_map(|item| match item {
            SelectionItem::Text(text) => Some(Caret::text(*text, 0)),
            SelectionItem::Rows(list, rows) => Some(Caret::row(*list, rows.key_at(0)?, 0, 0)),
        });
        let last = items.iter().rev().find_map(|item| match item {
            SelectionItem::Text(text) => Some(Caret::text(*text, text.text().len())),
            SelectionItem::Rows(list, rows) => Some(Caret::row(
                *list,
                rows.key_at(rows.count().checked_sub(1)?)?,
                END,
                END,
            )),
        });
        if let (Some(first), Some(last)) = (first, last) {
            self.select(Some(first), Some(last));
        }
    }

    fn select_word(&self, pos: Pos2) {
        let Some((start, end)) = self.word_at(pos) else {
            return;
        };
        self.0.value().touch.set(true);
        self.select(Some(start), Some(end));
    }

    pub fn selected_text(&self) -> String {
        let items = self.items();
        let Some(ordered) = self.ordered(&items).filter(|(start, end, ..)| start != end) else {
            return String::new();
        };
        let (_, _, from, to) = &ordered;
        let mut copied = String::new();
        let mut above: Option<Option<Rect>> = None;
        let mut join = |copied: &mut String, rect: Option<Rect>| {
            if let Some(above) = above {
                copied.push(match (above, rect) {
                    (Some(above), Some(rect)) if rect.top() < above.bottom() - 1.0 => ' ',
                    _ => '\n',
                });
            }
            above = Some(rect);
        };
        for (at, item) in items.iter().enumerate() {
            if at < from.0 || at > to.0 {
                continue;
            }
            match item {
                SelectionItem::Text(text) => {
                    let range = Self::text_range(*text, &ordered);
                    if range.is_empty() {
                        continue;
                    }
                    join(
                        &mut copied,
                        Some(measure(*text).get_untracked().unwrap_or(Rect::NOTHING)),
                    );
                    copied.push_str(slice(&text.text(), range.start, range.end));
                }
                SelectionItem::Rows(_, rows) => {
                    let Some(coverage) = Self::coverage(at, Some(&ordered)) else {
                        continue;
                    };
                    let count = rows.count();
                    let shown: HashMap<usize, NodeId> = rows
                        .shown()
                        .into_iter()
                        .filter_map(|(key, row)| Some((rows.index_of(&key)?, row)))
                        .collect();
                    let first = if at == from.0 { from.1 } else { 0 };
                    let last = if at == to.0 { to.1 } else { END };
                    for row in first..=last.min(count.saturating_sub(1)) {
                        if row >= count {
                            break;
                        }
                        join(&mut copied, None);
                        match shown.get(&row) {
                            Some(node) => {
                                let parts: Vec<String> = texts_under(*node)
                                    .into_iter()
                                    .enumerate()
                                    .filter_map(|(part, text)| {
                                        let range =
                                            Self::row_range(rows.as_ref(), &coverage, row, part)?;
                                        Some(slice(&text.text(), range.start, range.end).to_owned())
                                    })
                                    .collect();
                                copied.push_str(&parts.join("\n"));
                            }
                            None => {
                                let content = rows
                                    .key_at(row)
                                    .and_then(|key| rows.text(&key))
                                    .unwrap_or_default();
                                let range = Self::unbuilt_range(rows.as_ref(), &coverage, row);
                                copied.push_str(slice(&content, range.start, range.end));
                            }
                        }
                    }
                }
            }
        }
        copied
    }
}

fn floor(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn slice(text: &str, start: usize, end: usize) -> &str {
    let (start, end) = (floor(text, start), floor(text, end));
    text.get(start..end.max(start)).unwrap_or_default()
}

fn distance(rect: Rect, pos: Pos2) -> f32 {
    let x = (rect.left() - pos.x).max(pos.x - rect.right()).max(0.0);
    let y = (rect.top() - pos.y).max(pos.y - rect.bottom()).max(0.0);
    y * 1000.0 + x
}

fn word_bounds(text: &str, index: usize) -> (usize, usize) {
    let index = floor(text, index);
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

fn key(
    state: SelectableState,
    copy_key: Option<Chord>,
    select_all_key: Option<Chord>,
    press: KeyPress,
) -> bool {
    if copy_key.is_some_and(|chord| chord.matches(press)) {
        let text = state.selected_text();
        if text.is_empty() {
            return false;
        }
        copy_text(text);
        return true;
    }
    if select_all_key.is_some_and(|chord| chord.matches(press)) {
        state.select_all();
        return true;
    }
    false
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

fn precedes(items: &[SelectionItem], left: &Caret, right: &Caret) -> bool {
    SelectableState::order(items, left) < SelectableState::order(items, right)
}

fn press(
    state: SelectableState,
    focusable: &NodeRef,
    on_toolbar: Callback<Option<Rect>>,
    press: PointerPress,
) {
    if press.touch {
        *state.0.value().grab.borrow_mut() = None;
        let Some((handle, moving)) = state.handle_at(press.pos) else {
            return;
        };
        let items = state.items();
        let Some((start, end, ..)) = state.ordered(&items) else {
            return;
        };
        let fixed = match handle {
            CaretHandle::Start => end,
            _ => start,
        };
        let offset = state
            .caret_rect(&moving)
            .map_or(Vec2::ZERO, |caret| press.pos - caret.min);
        *state.0.value().grab.borrow_mut() = Some(Grab { fixed, offset });
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
        let anchor = state.anchor().or_else(|| caret.clone());
        state.0.value().unit.set(Unit::Character);
        *state.0.value().origin.borrow_mut() = anchor
            .as_ref()
            .map(|anchor| (anchor.clone(), anchor.clone()));
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
    *state.0.value().origin.borrow_mut() = origin.clone();
    match origin {
        Some((start, end)) => state.select(Some(start), Some(end)),
        None => state.select(None, None),
    }
}

fn drag(state: SelectableState, autoscroll: Signal<bool>, press: PointerPress) {
    let grab = state.0.value().grab.borrow().clone();
    if let Some(grab) = grab {
        if let Some(caret) = state.caret_at(press.pos - grab.offset) {
            state.select(Some(grab.fixed), Some(caret));
        }
        return;
    }
    if press.touch {
        return;
    }
    let pos = match state.0.value().region.try_get() {
        Some(region) => {
            let beyond = scroll_toward(region, press.pos, AUTOSCROLL_RATE);
            if autoscroll.get_untracked() != beyond.scrolled {
                autoscroll.set(beyond.scrolled);
            }
            beyond.inside
        }
        None => press.pos,
    };
    let origin = state.0.value().origin.borrow().clone();
    let (Some((start, end)), Some(reached)) =
        (origin, state.unit_at(pos, state.0.value().unit.get()))
    else {
        return;
    };
    let items = state.items();
    let (anchor, focus) = match precedes(&items, &reached.0, &start) {
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
    if state.0.value().grab.borrow_mut().take().is_some() {
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
    #[prop(default = ComponentRef::new())] api: ComponentRef<SelectableState>,
    children: Child,
    #[prop(default = SelectableState::default())] state: SelectableState,
    color: Prop<Color32>,
    handle_color: Prop<Color32>,
    caret_width: Prop<f32>,
    #[prop(default = Some(COPY_KEY))] copy_key: Prop<Option<Chord>>,
    #[prop(default = Some(SELECT_ALL_KEY))] select_all_key: Prop<Option<Chord>>,
    on_toolbar: Callback<Option<Rect>>,
) -> NodeId {
    api.fill(state);
    state.0.value().region.fill(children);
    let focusable = NodeRef::new();
    let autoscroll = create_signal(false);
    create_effect(move || {
        let color = color.get();
        let handle_color = handle_color.get();
        let caret_width = caret_width.get();
        untrack(|| {
            state.0.value().caret_width.set(caret_width);
            state.0.value().color.set(color);
            state.0.value().handle_color.set(handle_color);
            state.paint();
        });
    });
    on_key(KeyAt::Node(focusable), KeyPhase::Bubble, move |event| {
        event.press().is_some_and(|press| {
            key(
                state,
                copy_key.get_untracked(),
                select_all_key.get_untracked(),
                press,
            )
        })
    });
    view! {
        <Interactive
            focusable=true
            @node_ref=&focusable
            tab_stop=false
            claims_touch=false
            repeat_drag={autoscroll}
            claim_at={move |(pos, _): (Pos2, beui_view::types::Modifiers)| {
                state.handle_at(pos).is_some()
            }}
            on_press={move |event: PointerPress| press(state, &focusable, on_toolbar, event)}
            on_drag={move |event: PointerPress| drag(state, autoscroll, event)}
            on_click_at={move |event: PointerPress| tap(state, on_toolbar, event)}
            on_secondary_press={move |event: PointerPress| long_press(state, on_toolbar, event)}
            on_active_change={move |active: bool| {
                if !active {
                    autoscroll.set(false);
                    if state.0.value().grab.borrow_mut().take().is_some() {
                        show_toolbar(state, on_toolbar);
                    }
                }
            }}
        >
            {children}
        </Interactive>
    }
}

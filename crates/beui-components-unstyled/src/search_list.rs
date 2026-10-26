use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;

use accesskit::{Node, Role};
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::KeyAt;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::KeyPhase;
use beui_view::types::{Key, KeyPress};

use crate::button::{Button, ButtonHandle};
use crate::scroll::{Scroll, ScrollbarStyle};
use crate::text_input::{TextInput, TextInputHandle, TextInputStyle};

const EXACT: u32 = 100;
const PREFIX: u32 = 90;
const WORD_PREFIX: u32 = 75;
const CONTAINS: u32 = 50;
const SCATTERED: u32 = 30;
const TITLE_WEIGHT: u32 = 4;
const TERM_WEIGHT: u32 = 2;
const DETAIL_WEIGHT: u32 = 1;

pub struct SearchRowHandle<K> {
    pub key: K,
    pub highlighted: Memo<bool>,
    pub enabled: Memo<bool>,
    pub hovered: ReadSignal<bool>,
}

struct State<K: 'static> {
    search: NodeRef,
    shown: Memo<Vec<K>>,
    enabled: Func<K, bool>,
    highlighted: Signal<Option<K>>,
    rows: Stored<RefCell<HashMap<K, NodeRef>>>,
    page: usize,
    on_choose: Callback<K>,
}

impl<K: 'static> Clone for State<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: 'static> Copy for State<K> {}

impl<K: Clone + Eq + Hash + 'static> State<K> {
    fn choosable(&self) -> Vec<K> {
        let enabled = self.enabled;
        self.shown
            .get_untracked()
            .into_iter()
            .filter(|key| untrack(|| enabled.call(key.clone())))
            .collect()
    }

    fn choose(&self, key: K) {
        if untrack(|| self.enabled.call(key.clone())) {
            self.on_choose.call(key);
        }
    }

    fn submit(&self) {
        if let Some(key) = self.highlighted.get_untracked() {
            self.choose(key);
        }
    }

    fn navigate(&self, press: KeyPress) -> bool {
        if !press.pressed || press.modifiers.ctrl || press.modifiers.alt {
            return false;
        }
        let moves = matches!(
            press.key,
            Key::ArrowDown | Key::ArrowUp | Key::PageDown | Key::PageUp | Key::Home | Key::End
        );
        let choosable = self.choosable();
        if !moves || choosable.is_empty() {
            return moves;
        }
        let at = self
            .highlighted
            .get_untracked()
            .and_then(|key| choosable.iter().position(|other| *other == key));
        let last = choosable.len() - 1;
        let next = match press.key {
            Key::ArrowDown => at.map_or(0, |at| (at + 1).min(last)),
            Key::ArrowUp => at.map_or(0, |at| at.saturating_sub(1)),
            Key::PageDown => at.map_or(0, |at| (at + self.page).min(last)),
            Key::PageUp => at.map_or(0, |at| at.saturating_sub(self.page)),
            Key::Home => 0,
            _ => last,
        };
        self.highlighted.set(Some(choosable[next].clone()));
        true
    }
}

#[component]
pub fn SearchList<K>(
    #[prop(default = ComponentRef::new())] api: ComponentRef<SearchListApi<K>>,
    query: Signal<String>,
    highlighted: Signal<Option<K>>,
    rank: Func<String, Vec<K>>,
    #[prop(default = None)] enabled: Option<Func<K, bool>>,
    #[prop(default = true)] highlight_first: bool,
    #[prop(default = 8)] page: usize,
    on_choose: Callback<K>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = true)] keyboard_on_focus: bool,
    on_focus_change: Callback<bool>,
    placeholder: Prop<String>,
    style: TextInputStyle,
    search_accessibility: Option<Prop<Node>>,
    field: RenderFn<TextInputHandle>,
    list_height: Prop<f32>,
    spacing: f32,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    accessibility: Func<K, Node>,
    row: RenderFn<SearchRowHandle<K>>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let enabled = enabled.unwrap_or_else(|| Func::new(|_| true));
    let shown = create_memo(move || {
        let query = query.get();
        rank.call(query)
    });
    let state = State {
        search: NodeRef::new(),
        shown,
        enabled,
        highlighted,
        rows: store(RefCell::default()),
        page,
        on_choose,
    };
    api.fill(SearchListApi(state));

    create_effect(move || {
        let shown = shown.get();
        let empty = query.with(|query| query.trim().is_empty());
        let current = highlighted.get();
        untrack(|| {
            let choosable = |key: &K| shown.contains(key) && enabled.call(key.clone());
            if current.as_ref().is_some_and(choosable) {
                return;
            }
            if current.is_none() && empty && !highlight_first {
                return;
            }
            let first = shown.iter().find(|key| choosable(key)).cloned();
            if current != first {
                highlighted.set(first);
            }
        });
    });

    let highlight = create_selector(move || highlighted.get());
    let reveal = create_memo(move || {
        let highlighted = highlighted.get()?;
        shown.with(|shown| shown.iter().position(|key| *key == highlighted))
    });
    on_key(KeyAt::Node(state.search), KeyPhase::Capture, move |event| {
        event.press().is_some_and(|press| state.navigate(press))
    });
    let search_accessibility = crate::labelled_node(
        Role::TextInput,
        search_accessibility,
        Prop::Static(String::new()),
    );
    view! {
        <List spacing>
            <TextInput
                @node_ref={&state.search}
                value={query}
                focused
                keyboard_on_focus
                placeholder
                style
                accessibility={search_accessibility}
                content={Render::new(move |handle: TextInputHandle| field.call(handle))}
                on_focus_change={move |has_focus: bool| on_focus_change.call(has_focus)}
                on_submit={move |_: String| state.submit()}
            />
            <Scroll
                @sizing={list_height.map(|height| ItemSize::Intrinsic.max(height).shrink(1.0))}
                fit=true
                reveal
                scrollbar
            >
                <ForEach keys={shown}>|key: K|
                    <SearchRow state key accessibility row highlight />
                </ForEach>
            </Scroll>
        </List>
    }
}

#[component]
fn SearchRow<K>(
    state: State<K>,
    key: K,
    accessibility: Func<K, Node>,
    row: RenderFn<SearchRowHandle<K>>,
    highlight: Selector<Option<K>>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let button = NodeRef::new();
    state.rows.value().borrow_mut().insert(key.clone(), button);
    on_cleanup({
        let key = key.clone();
        move || {
            if let Some(rows) = state.rows.try_value() {
                rows.borrow_mut().remove(&key);
            }
        }
    });
    let node = create_memo({
        let key = key.clone();
        move || accessibility.call(key.clone())
    });
    let enabled = create_memo({
        let key = key.clone();
        move || state.enabled.call(key.clone())
    });
    let (hover_key, click_key) = (key.clone(), key.clone());
    view! {
        <Button
            @node_ref={&button}
            accessibility={node}
            tab_stop=false
            press_focus=false
            on_click={move || state.choose(click_key.clone())}
        >|handle: ButtonHandle| {
            let hovered = handle.hovered;
            let hover_key = hover_key.clone();
            create_effect(move || {
                if hovered.get() && enabled.get_untracked() {
                    state.highlighted.set(Some(hover_key.clone()));
                }
            });
            row.call(SearchRowHandle {
                key: key.clone(),
                highlighted: highlight.memo(Some(key.clone())),
                enabled,
                hovered,
            })
        }</Button>
    }
}

pub struct SearchListApi<K: 'static>(State<K>);

impl<K: 'static> Clone for SearchListApi<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: 'static> Copy for SearchListApi<K> {}

impl<K: Clone + Eq + Hash + 'static> SearchListApi<K> {
    pub fn search(&self) -> NodeId {
        self.0.search.get()
    }

    pub fn shown(&self) -> Vec<K> {
        self.0.shown.get_untracked()
    }

    pub fn highlighted(&self) -> Option<K> {
        self.0.highlighted.get_untracked()
    }

    pub fn row(&self, key: &K) -> Option<NodeId> {
        let rows = self.0.rows.value();
        let rows = rows.borrow();
        rows.get(key).and_then(NodeRef::try_get)
    }
}

pub fn rank<T>(items: &[T], query: &str, score: impl Fn(&T, &str) -> Option<u32>) -> Vec<usize> {
    let mut scored: Vec<(u32, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| Some((score(item, query)?, index)))
        .collect();
    scored.sort_by(|(a, first), (b, second)| b.cmp(a).then(first.cmp(second)));
    scored.into_iter().map(|(_, index)| index).collect()
}

pub fn substring_score(text: &str, query: &str) -> Option<u32> {
    let text = text.to_lowercase();
    query
        .split_whitespace()
        .all(|word| text.contains(&word.to_lowercase()))
        .then_some(0)
}

pub fn fuzzy_score(title: &str, terms: &[String], detail: &str, query: &str) -> Option<u32> {
    let query = query.trim().to_lowercase();
    let mut total = match word_score(title, &query, false) {
        Some(whole) if query.contains(' ') => whole * TITLE_WEIGHT,
        _ => 0,
    };
    for word in query.split_whitespace() {
        let title = word_score(title, word, true).map(|score| score * TITLE_WEIGHT);
        let terms = terms
            .iter()
            .filter_map(|term| word_score(term, word, false))
            .max()
            .map(|score| score * TERM_WEIGHT);
        let detail = word_score(detail, word, false).map(|score| score * DETAIL_WEIGHT);
        total += [title, terms, detail].into_iter().flatten().max()?;
    }
    Some(total)
}

fn word_score(field: &str, word: &str, scattered: bool) -> Option<u32> {
    let field = field.to_lowercase();
    if field == word {
        return Some(EXACT);
    }
    if field.starts_with(word) {
        return Some(PREFIX);
    }
    let starts_a_word = field.match_indices(word).any(|(at, _)| {
        field[..at]
            .chars()
            .next_back()
            .is_none_or(|before| !before.is_alphanumeric())
    });
    if starts_a_word {
        return Some(WORD_PREFIX);
    }
    if field.contains(word) {
        return Some(CONTAINS);
    }
    if !scattered {
        return None;
    }
    let mut wanted = word.chars().peekable();
    let (mut first, mut last) = (None, 0);
    for (at, letter) in field.chars().enumerate() {
        if wanted.peek() == Some(&letter) {
            wanted.next();
            first.get_or_insert(at);
            last = at;
        }
    }
    if wanted.peek().is_some() {
        return None;
    }
    let span = (last + 1 - first.unwrap_or(0)) as u32;
    let length = word.chars().count() as u32;
    Some(SCATTERED * length / span.max(length))
}

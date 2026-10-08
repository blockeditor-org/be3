use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::scroll::scrollbar_style;
use crate::text::{Caption, Icon};
use crate::text_input::text_input_style;
use crate::theme::{BORDER_WIDTH, CARD_RADIUS, FONT_BODY, RADIUS, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ButtonHandle, Picture, TextInputHandle};
use beui_core::base::TextAlign;
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color32;
use beui_core::geometry::Pos2;
use beui_core::icons::{ICON_APPS, ICON_SEARCH, ICON_TERMINAL};
use beui_core::image::Image;
use beui_core::input::{Key, KeyPress};
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Align, Callback, ClickCallback, Direction, ForEach, Frame, ItemSize, List, Memo, Prop,
    ReadSignal, Show, Text, WriteSignal, clone, create_effect, create_memo, create_signal, untrack,
};

const WIDTH: f32 = 560.0;
const LIST_HEIGHT: f32 = 380.0;
const MARGIN: f32 = 12.0;
const PADDING: f32 = 6.0;
const SEARCH_HEIGHT: f32 = 40.0;
const ROW_PADDING_HORIZONTAL: f32 = 10.0;
const ROW_PADDING_VERTICAL: f32 = 6.0;
const ROW_SPACING: f32 = 12.0;
const PICTURE_SIZE: f32 = 32.0;
const PAGE: usize = 8;
const SCRIM: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 90);

const EXACT: u32 = 100;
const PREFIX: u32 = 90;
const WORD_PREFIX: u32 = 75;
const CONTAINS: u32 = 50;
const SCATTERED: u32 = 30;
const TITLE_WEIGHT: u32 = 4;
const TERM_WEIGHT: u32 = 2;
const DETAIL_WEIGHT: u32 = 1;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LauncherItem {
    pub key: String,
    pub title: String,
    pub detail: String,
    pub terms: Vec<String>,
    pub image: Option<Image>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Row {
    Item(String),
    Run,
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

pub fn launcher_score(item: &LauncherItem, query: &str) -> Option<u32> {
    let query = query.trim().to_lowercase();
    let mut total = match word_score(&item.title, &query, false) {
        Some(whole) if query.contains(' ') => whole * TITLE_WEIGHT,
        _ => 0,
    };
    for word in query.split_whitespace() {
        let title = word_score(&item.title, word, true).map(|score| score * TITLE_WEIGHT);
        let terms = item
            .terms
            .iter()
            .filter_map(|term| word_score(term, word, false))
            .max()
            .map(|score| score * TERM_WEIGHT);
        let detail = word_score(&item.detail, word, false).map(|score| score * DETAIL_WEIGHT);
        total += [title, terms, detail].into_iter().flatten().max()?;
    }
    Some(total)
}

pub fn launcher_rank(items: &[LauncherItem], query: &str) -> Vec<usize> {
    let mut scored: Vec<(u32, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| Some((launcher_score(item, query)?, index)))
        .collect();
    scored.sort_by(|(a, first), (b, second)| b.cmp(a).then(first.cmp(second)));
    scored.into_iter().map(|(_, index)| index).collect()
}

#[derive(Clone)]
struct State {
    query: ReadSignal<String>,
    set_query: WriteSignal<String>,
    rows: Memo<Vec<Row>>,
    highlighted: ReadSignal<Option<Row>>,
    set_highlighted: WriteSignal<Option<Row>>,
    on_launch: Callback<String>,
    on_run: Callback<String>,
}

impl State {
    fn activate(&self, row: Row) {
        match row {
            Row::Item(key) => self.on_launch.call(key),
            Row::Run => self
                .on_run
                .call(self.query.get_untracked().trim().to_owned()),
        }
    }

    fn submit(&self) {
        if let Some(row) = self.highlighted.get_untracked() {
            self.activate(row);
        }
    }

    fn navigate(&self, press: KeyPress) -> bool {
        if !press.pressed || press.modifiers.ctrl || press.modifiers.alt {
            return false;
        }
        let rows = self.rows.get_untracked();
        if rows.is_empty() {
            return matches!(press.key, Key::ArrowDown | Key::ArrowUp);
        }
        let at = self
            .highlighted
            .get_untracked()
            .and_then(|row| rows.iter().position(|other| *other == row));
        let last = rows.len() - 1;
        let next = match press.key {
            Key::ArrowDown => at.map_or(0, |at| (at + 1).min(last)),
            Key::ArrowUp => at.map_or(0, |at| at.saturating_sub(1)),
            Key::PageDown => at.map_or(0, |at| (at + PAGE).min(last)),
            Key::PageUp => at.map_or(0, |at| at.saturating_sub(PAGE)),
            _ => return false,
        };
        self.set_highlighted.set(Some(rows[next].clone()));
        true
    }
}

#[component]
pub fn Launcher(
    open: Prop<bool>,
    items: Prop<Rc<Vec<LauncherItem>>>,
    #[prop(default = "Search".to_owned())] placeholder: Prop<String>,
    #[prop(default = true)] runs_commands: Prop<bool>,
    on_launch: Callback<String>,
    on_run: Callback<String>,
    on_close: ClickCallback,
) -> NodeId {
    let (query, set_query) = create_signal(String::new());
    let (searching, set_searching) = create_signal(false);
    let (highlighted, set_highlighted) = create_signal(None::<Row>);
    let dismissing = open.clone();
    create_effect(clone!(open set_query set_searching -> move || {
        let opened = open.get();
        untrack(|| {
            if opened {
                set_query.set(String::new());
            }
            set_searching.set(opened);
        });
    }));
    let ranked = create_memo(clone!(items query -> move || {
        let items = items.get();
        let query = query.get();
        launcher_rank(&items, &query)
            .into_iter()
            .map(|index| items[index].key.clone())
            .collect::<Vec<_>>()
    }));
    let rows = create_memo(clone!(ranked query -> move || {
        let command = runs_commands.get() && !query.get().trim().is_empty();
        ranked
            .get()
            .into_iter()
            .map(Row::Item)
            .chain(command.then_some(Row::Run))
            .collect::<Vec<_>>()
    }));
    create_effect(clone!(rows highlighted set_highlighted -> move || {
        let rows = rows.get();
        let kept = untrack(|| highlighted.get()).filter(|row| rows.contains(row));
        let next = kept.or_else(|| rows.first().cloned());
        if untrack(|| highlighted.get()) != next {
            set_highlighted.set(next);
        }
    }));
    let reveal = create_memo(clone!(rows highlighted -> move || {
        let highlighted = highlighted.get()?;
        rows.get().iter().position(|row| *row == highlighted)
    }));
    let state = State {
        query: query.clone(),
        set_query,
        rows: rows.clone(),
        highlighted,
        set_highlighted,
        on_launch,
        on_run,
    };
    let (typing, submitting, keys, building) = (state.clone(), state.clone(), state.clone(), state);
    let accessibility = create_memo(|| {
        let mut node = Node::new(Role::SearchInput);
        node.set_label("Search programs");
        node
    });
    let theme = use_theme();
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            placement=Placement::Center
            scrim=SCRIM
            open
            on_dismiss={move || {
                if dismissing.peek() {
                    on_close.call();
                }
            }}
        >
            <Frame padding_horizontal=MARGIN padding_vertical=MARGIN>
                <Frame
                    max_width=WIDTH
                    color={theme.surface_raised.clone()}
                    outline={theme.border.clone()}
                    outline_width=BORDER_WIDTH
                    outline_visible=true
                    radius=CARD_RADIUS
                    padding_horizontal=PADDING
                    padding_vertical=PADDING
                    @test_id={"launcher"}
                >
                    <List spacing=PADDING>
                        <unstyled::TextInput
                            @test_id={"launcher.search"}
                            value={query}
                            focused={searching}
                            placeholder
                            style={text_input_style()}
                            accessibility
                            content={|handle| view! {
                                <LauncherSearch handle />
                            }}
                            on_change={move |text: String| typing.set_query.set(text)}
                            on_submit={move |_: String| submitting.submit()}
                            on_key_override={move |press: KeyPress| keys.navigate(press)}
                        />
                        <unstyled::Scroll
                            @sizing=ItemSize::Fixed(LIST_HEIGHT)
                            reveal
                            scrollbar={scrollbar_style()}
                        >
                            <ForEach keys={rows}>
                                {move |row: Row| view! {
                                    <LauncherRow
                                        state={building.clone()}
                                        items={items.clone()}
                                        row
                                    />
                                }}
                            </ForEach>
                        </unstyled::Scroll>
                    </List>
                </Frame>
            </Frame>
        </Overlay>
    }
}

#[component]
fn LauncherSearch(handle: TextInputHandle) -> NodeId {
    let TextInputHandle { field, .. } = handle;
    let theme = use_theme();
    view! {
        <Frame
            height=SEARCH_HEIGHT
            color={theme.surface.clone()}
            outline={theme.accent.clone()}
            outline_width=BORDER_WIDTH
            radius=RADIUS
            outline_visible=true
            padding_horizontal=ROW_PADDING_HORIZONTAL
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Icon glyph={ICON_SEARCH.to_owned()} color={theme.text_muted.clone()} />
                <Frame @sizing=ItemSize::Percent(100.0)>{field}</Frame>
            </List>
        </Frame>
    }
}

#[component]
fn LauncherRow(state: State, items: Prop<Rc<Vec<LauncherItem>>>, row: Row) -> NodeId {
    let item = create_memo(clone!(row -> move || match &row {
        Row::Item(key) => items.get().iter().find(|item| item.key == *key).cloned(),
        Row::Run => None,
    }));
    let query = state.query.clone();
    let title = create_memo(clone!(item row query -> move || match &row {
        Row::Item(_) => item.get().map(|item| item.title).unwrap_or_default(),
        Row::Run => format!("Run \u{201c}{}\u{201d}", query.get().trim()),
    }));
    let detail = create_memo(clone!(item row -> move || match &row {
        Row::Item(_) => item.get().map(|item| item.detail).unwrap_or_default(),
        Row::Run => "Run it as a command".to_owned(),
    }));
    let image = create_memo(clone!(item -> move || item.get().and_then(|item| item.image)));
    let test_id = match &row {
        Row::Item(key) => format!("launcher.item.{key}"),
        Row::Run => "launcher.run".to_owned(),
    };
    let glyph = match &row {
        Row::Item(_) => ICON_APPS,
        Row::Run => ICON_TERMINAL,
    };
    let highlighted = state.highlighted.clone();
    let lit = create_memo(clone!(row -> move || highlighted.get().as_ref() == Some(&row)));
    let accessibility = create_memo(clone!(title -> move || {
        let mut node = Node::new(Role::ListBoxOption);
        node.set_label(title.get());
        node
    }));
    let (hovering, clicking) = (state.clone(), state);
    let (hover_row, click_row) = (row.clone(), row);
    view! {
        <unstyled::Button
            @test_id={test_id}
            accessibility
            tab_stop=false
            press_focus=false
            content={move |handle: ButtonHandle| {
                let hovered = handle.hovered.clone();
                create_effect(clone!(hovering hover_row -> move || {
                    if hovered.get() {
                        hovering.set_highlighted.set(Some(hover_row.clone()));
                    }
                }));
                view! {
                    <LauncherRowFace
                        title={title.clone()}
                        detail={detail.clone()}
                        image={image.clone()}
                        glyph={glyph.to_owned()}
                        lit={lit.clone()}
                    />
                }
            }}
            on_click={move || clicking.activate(click_row.clone())}
        />
    }
}

#[component]
fn LauncherRowFace(
    title: Memo<String>,
    detail: Memo<String>,
    image: Memo<Option<Image>>,
    glyph: String,
    lit: Memo<bool>,
) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(clone!(theme lit -> move || match lit.get() {
        true => theme.accent_soft.get(),
        false => Color32::TRANSPARENT,
    }));
    let pictured = create_memo(clone!(image -> move || image.get().is_some()));
    let plain = create_memo(clone!(pictured -> move || !pictured.get()));
    let described = create_memo(clone!(detail -> move || !detail.get().is_empty()));
    view! {
        <Frame
            color={fill}
            radius=RADIUS
            padding_horizontal=ROW_PADDING_HORIZONTAL
            padding_vertical=ROW_PADDING_VERTICAL
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Frame width=PICTURE_SIZE height=PICTURE_SIZE>
                    <List spacing=0.0 align=Align::Center>
                        <Show condition={pictured}>
                            <LauncherPicture image={image.clone()} />
                        </Show>
                        <Show condition={plain}>
                            <Icon glyph={glyph.clone()} color={theme.text_muted.clone()} />
                        </Show>
                    </List>
                </Frame>
                <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                    <Text
                        string={title}
                        font_size=FONT_BODY
                        color={theme.text.clone()}
                        align=TextAlign::Start
                        ellipsis=true
                    />
                    <Show condition={described}>
                        <Caption content={detail.clone()} ellipsis=true />
                    </Show>
                </List>
            </List>
        </Frame>
    }
}

#[component]
fn LauncherPicture(image: Memo<Option<Image>>) -> NodeId {
    view! {
        <Frame width=PICTURE_SIZE height=PICTURE_SIZE>
            <Picture image />
        </Frame>
    }
}

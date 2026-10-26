use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::font::Overflow;
use beui_core::icons::Glyph;
use beui_core::layer::Dismiss;
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::scroll::scrollbar_style;
use crate::text::{Icon, Text, TextColor, TextStyle};
use crate::text_input::text_input_style;
use crate::theme::use_theme;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ButtonHandle, Picture, TextInputHandle};
use beui_core::base::TextAlign;
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color32;
use beui_core::geometry::Pos2;
use beui_core::icons::{ICON_LAYOUT_GRID, ICON_SEARCH, ICON_TERMINAL};
use beui_core::image::Image;
use beui_core::input::{Key, KeyPress};
use beui_core::keys::{KeyAt, KeyPhase};
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

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

#[derive(Clone, Copy)]
struct State {
    query: Signal<String>,
    rows: Memo<Vec<Row>>,
    highlighted: Signal<Option<Row>>,
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
        self.highlighted.set(Some(rows[next].clone()));
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
    let theme = use_theme();
    let scrim = create_memo(move || theme.scrim_soft.get());
    let query = create_signal(String::new());
    let searching = create_signal(false);
    let highlighted = create_signal(None::<Row>);
    create_effect(move || {
        let opened = open.get();
        untrack(|| {
            if opened {
                query.set(String::new());
            }
            searching.set(opened);
        });
    });
    let ranked = create_memo({
        let items = items.clone();
        move || {
            let items = items.get();
            let query = query.get();
            launcher_rank(&items, &query)
                .into_iter()
                .map(|index| items[index].key.clone())
                .collect::<Vec<_>>()
        }
    });
    let rows = create_memo(move || {
        let command = runs_commands.get() && !query.get().trim().is_empty();
        ranked
            .get()
            .into_iter()
            .map(Row::Item)
            .chain(command.then_some(Row::Run))
            .collect::<Vec<_>>()
    });
    create_effect(move || {
        let rows = rows.get();
        let kept = highlighted.get_untracked().filter(|row| rows.contains(row));
        let next = kept.or_else(|| rows.first().cloned());
        if highlighted.get_untracked() != next {
            highlighted.set(next);
        }
    });
    let reveal = create_memo(move || {
        let highlighted = highlighted.get()?;
        rows.get().iter().position(|row| *row == highlighted)
    });
    let state = State {
        query,
        rows,
        highlighted,
        on_launch,
        on_run,
    };
    let search = NodeRef::new();
    on_key(KeyAt::Node(search), KeyPhase::Capture, move |event| {
        event.press().is_some_and(|press| state.navigate(press))
    });

    let accessibility = create_memo(|| {
        let mut node = Node::new(Role::SearchInput);
        node.set_label("Search programs");
        node
    });
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            placement=Placement::Center
            scrim={scrim}
            open
            on_dismiss={move |_: Dismiss| on_close.call()}
            content={move || { let items = items.clone(); let placeholder = placeholder.clone(); view! {
                <Frame padding=MARGIN>
                    <Frame
                        width={Extent::at_most(WIDTH)}
                        fill=ColorRole::SurfaceRaised
                        border={Border::DEFAULT}
                        radius=Radius::Lg
                        padding=PADDING
                        @test_id={"launcher"}
                    >
                        <List spacing=PADDING>
                            <unstyled::TextInput
                                @node_ref=&search
                                @test_id={"launcher.search"}
                                value={query}
                                focused={searching}
                                placeholder
                                style={text_input_style()}
                                accessibility
                                content={|handle| view! {
                                    <LauncherSearch handle />
                                }}
                                on_submit={move |_: String| state.submit()}
                            />
                            <unstyled::Scroll
                                @sizing=ItemSize::Fixed(LIST_HEIGHT)
                                reveal
                                scrollbar={scrollbar_style()}
                            >
                                <ForEach keys={rows}>|row: Row|
                                    <LauncherRow state={state} items={items.clone()} row />
                                </ForEach>
                            </unstyled::Scroll>
                        </List>
                    </Frame>
                </Frame>
            } }}
        />
    }
}

#[component]
fn LauncherSearch(handle: TextInputHandle) -> NodeId {
    let TextInputHandle { field, .. } = handle;
    view! {
        <Frame
            height=SEARCH_HEIGHT
            fill=ColorRole::Surface
            border={Border::new(ColorRole::Accent)}
            radius=Radius::Md
            padding={Sides::horizontal(ROW_PADDING_HORIZONTAL)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Text color=TextColor::Muted align=TextAlign::Center>
                    <Icon glyph=ICON_SEARCH />
                </Text>
                <Frame @sizing=ItemSize::Percent(100.0)>{field}</Frame>
            </List>
        </Frame>
    }
}

#[component]
fn LauncherRow(state: State, items: Prop<Rc<Vec<LauncherItem>>>, row: Row) -> NodeId {
    let item = create_memo({
        let row = row.clone();
        move || match &row {
            Row::Item(key) => items.get().iter().find(|item| item.key == *key).cloned(),
            Row::Run => None,
        }
    });
    let query = state.query;
    let title = create_memo({
        let row = row.clone();
        move || match &row {
            Row::Item(_) => item.get().map(|item| item.title).unwrap_or_default(),
            Row::Run => format!("Run \u{201c}{}\u{201d}", query.get().trim()),
        }
    });
    let detail = create_memo({
        let row = row.clone();
        move || match &row {
            Row::Item(_) => item.get().map(|item| item.detail).unwrap_or_default(),
            Row::Run => "Run it as a command".to_owned(),
        }
    });
    let image = create_memo(move || item.get().and_then(|item| item.image));
    let row_test_id = match &row {
        Row::Item(key) => format!("launcher.item.{key}"),
        Row::Run => "launcher.run".to_owned(),
    };
    let glyph = match &row {
        Row::Item(_) => ICON_LAYOUT_GRID,
        Row::Run => ICON_TERMINAL,
    };
    let highlighted = state.highlighted;
    let lit = create_memo({
        let row = row.clone();
        move || highlighted.get().as_ref() == Some(&row)
    });
    let accessibility = create_memo(move || {
        let mut node = Node::new(Role::ListBoxOption);
        node.set_label(title.get());
        node
    });
    let (hover_row, click_row) = (row.clone(), row);
    view! {
        <unstyled::Button
            @test_id={row_test_id}
            accessibility
            tab_stop=false
            press_focus=false
            on_click={move || state.activate(click_row.clone())}
        >|handle: ButtonHandle| {
            let hovered = handle.hovered;
            create_effect(move || {
                if hovered.get() {
                    state.highlighted.set(Some(hover_row.clone()));
                }
            });
            view! {
                <LauncherRowFace title={title} detail={detail} image={image} glyph lit={lit} />
            }
        }</unstyled::Button>
    }
}

#[component]
fn LauncherRowFace(
    title: Memo<String>,
    detail: Memo<String>,
    image: Memo<Option<Image>>,
    glyph: Glyph,
    lit: Memo<bool>,
) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(move || match lit.get() {
        true => theme.accent_soft.get(),
        false => Color32::TRANSPARENT,
    });
    let pictured = create_memo(move || image.get().is_some());
    let plain = create_memo(move || !pictured.get());
    let described = create_memo(move || !detail.get().is_empty());
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            radius=Radius::Md
            padding={Sides::symmetric(ROW_PADDING_HORIZONTAL, ROW_PADDING_VERTICAL)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Frame width=PICTURE_SIZE height=PICTURE_SIZE>
                    <List spacing=0.0 align=Align::Center>
                        <Show condition={pictured}>
                            <LauncherPicture image={image} />
                        </Show>
                        <Show condition={plain}>
                            <Text color=TextColor::Muted align=TextAlign::Center>
                                <Icon glyph={glyph} />
                            </Text>
                        </Show>
                    </List>
                </Frame>
                <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                    <Text align=TextAlign::Start overflow=Overflow::Ellipsis>{title}</Text>
                    <Show condition={described}>
                        <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>{detail}</Text>
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

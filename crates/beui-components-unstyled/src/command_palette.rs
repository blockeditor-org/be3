use std::cell::{Cell, RefCell};
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_core::base::overlay::{OverlayAnchor, OverlayNode, Placement};
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::geometry::Pos2;
use beui_core::input::{Key, KeyPress};
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Action, Child, ClickCallback, ForEach, ItemSize, List, Memo, NodeRef, Prop, ReadSignal, Render,
    RenderFn, Selector, WriteSignal, active_actions_from, clone, create_effect, create_memo,
    create_selector, create_signal, set_component_state, untrack, with_document,
};

use crate as unstyled;
use crate::button::ButtonHandle;
use crate::scroll::ScrollbarStyle;
use crate::text_input::TextInputHandle;
use crate::text_menu::TextMenu;

pub struct CommandRowHandle {
    pub label: Prop<String>,
    pub glyph: String,
    pub shortcut: Option<String>,
    pub disabled: Memo<bool>,
    pub highlighted: Memo<bool>,
    pub hovered: ReadSignal<bool>,
}

struct State {
    overlay: NodeRef,
    search: NodeRef,
    actions: RefCell<Vec<Action>>,
    query: ReadSignal<String>,
    set_query: WriteSignal<String>,
    shown: ReadSignal<Vec<u64>>,
    set_shown: WriteSignal<Vec<u64>>,
    highlighted: ReadSignal<Option<u64>>,
    set_highlighted: WriteSignal<Option<u64>>,
    searching: ReadSignal<bool>,
    set_searching: WriteSignal<bool>,
    returning: Cell<Option<NodeId>>,
    rows: RefCell<Vec<(u64, NodeRef)>>,
    on_close: ClickCallback,
}

type Handle = Rc<State>;

#[component]
pub fn CommandPalette(
    open: Prop<bool>,
    on_close: ClickCallback,
    list_height: Prop<f32>,
    #[prop(default = Color32::TRANSPARENT)] scrim: Prop<Color32>,
    search_placeholder: Prop<String>,
    search_font_size: Prop<f32>,
    search_color: Prop<Color32>,
    search_placeholder_color: Prop<Color32>,
    search_selection_color: Prop<Color32>,
    search_caret_color: Prop<Color32>,
    search_padding_horizontal: Prop<f32>,
    search_content: Option<Render<TextInputHandle>>,
    #[prop(default = TextMenu::default())] search_menu: TextMenu,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    row: Option<RenderFn<CommandRowHandle>>,
    #[prop(children)] panel: Option<Render<Child>>,
) -> NodeId {
    let row = row.unwrap_or_else(|| {
        RenderFn::new(|_| {
            view! {
                <List spacing=0.0 />
            }
        })
    });
    let panel = panel.unwrap_or_else(|| Render::new(|content| content));
    let (query, set_query) = create_signal(String::new());
    let (shown, set_shown) = create_signal(Vec::<u64>::new());
    let (highlighted, set_highlighted) = create_signal(None::<u64>);
    let (searching, set_searching) = create_signal(false);
    let state: Handle = Rc::new(State {
        overlay: NodeRef::new(),
        search: NodeRef::new(),
        actions: RefCell::new(Vec::new()),
        query: query.clone(),
        set_query,
        shown: shown.clone(),
        set_shown,
        highlighted: highlighted.clone(),
        set_highlighted,
        searching: searching.clone(),
        set_searching,
        returning: Cell::new(None),
        rows: RefCell::new(Vec::new()),
        on_close,
    });
    set_component_state(state.clone());

    let opened = open.clone();
    create_effect(clone!(state -> move || {
        if opened.get() {
            untrack(|| start(&state));
        }
    }));

    let highlight = create_selector(clone!(highlighted -> move || highlighted.get()));
    let keys = create_memo(clone!(shown -> move || shown.get()));
    let reveal = reveal_reader(&state);
    let (dismiss_state, filter_state, submit_state, key_state, rows_state) = (
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
    );
    let accessibility = create_memo(|| {
        let mut node = Node::new(Role::Dialog);
        node.set_label("Command palette");
        node
    });
    view! {
        <Overlay
            @node_ref={&state.overlay}
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            placement=Placement::Center
            scrim
            open
            on_dismiss={move || dismiss(&dismiss_state)}
        >
            {panel.call(view! {
                <List spacing=6.0>
                    <unstyled::TextInput
                        @node_ref={&state.search}
                        @test_id={"command-palette.search"}
                        value={query}
                        focused={searching}
                        placeholder={search_placeholder}
                        font_size={search_font_size}
                        color={search_color}
                        placeholder_color={search_placeholder_color}
                        selection_color={search_selection_color}
                        caret_color={search_caret_color}
                        padding_horizontal={search_padding_horizontal}
                        menu={search_menu}
                        accessibility={accessibility}
                        content={search_content.unwrap_or_else(|| Render::new(|handle: TextInputHandle| handle.field))}
                        on_change={move |text: String| filter(&filter_state, &text)}
                        on_submit={move |_text: String| {
                            if let Some(key) = submit_state.highlighted.get_untracked() {
                                confirm(&submit_state, key);
                            }
                        }}
                        on_key_override={move |press: KeyPress| navigate(&key_state, press)}
                    />
                    <unstyled::Scroll
                        @sizing={list_height.map(ItemSize::Fixed)}
                        reveal
                        scrollbar={scrollbar}
                    >
                        <ForEach keys={keys}>
                            {move |key: u64| {
                                view! {
                                    <CommandRow
                                        state={rows_state.clone()}
                                        key
                                        row={row.clone()}
                                        highlight={highlight.clone()}
                                    />
                                }
                            }}
                        </ForEach>
                    </unstyled::Scroll>
                </List>
            })}
        </Overlay>
    }
}

#[component]
fn CommandRow(
    state: Handle,
    key: u64,
    row: RenderFn<CommandRowHandle>,
    highlight: Selector<Option<u64>>,
) -> NodeId {
    let action = find(&state, key).expect("a shown row has an action");
    let button = NodeRef::new();
    state.rows.borrow_mut().push((key, button.clone()));
    let enabled = action.enabled();
    let disabled = create_memo(move || !enabled.get());
    let label = action.label();
    let accessibility = create_memo(clone!(label disabled -> move || {
        let mut node = Node::new(Role::ListBoxOption);
        node.set_label(label.get());
        if disabled.get() {
            node.set_disabled();
        }
        node
    }));
    let (hover_state, click_state) = (state.clone(), state);
    let glyph = action.glyph().to_owned();
    let shortcut = action.shortcut_label();
    let face_disabled = disabled.clone();
    view! {
        <unstyled::Button
            @node_ref={&button}
            accessibility
            tab_stop=false
            press_focus=false
            content={move |handle: ButtonHandle| {
                let hovered = handle.hovered.clone();
                let hovering = clone!(hover_state -> move || {
                    if hovered.get() && find(&hover_state, key).is_some_and(|action| action.is_enabled()) {
                        hover_state.set_highlighted.set(Some(key));
                    }
                });
                create_effect(hovering);
                row.call(CommandRowHandle {
                    label: label.clone(),
                    glyph: glyph.clone(),
                    shortcut: shortcut.clone(),
                    disabled: face_disabled.clone(),
                    highlighted: highlight.memo(Some(key)),
                    hovered: handle.hovered,
                })
            }}
            on_click={move || confirm(&click_state, key)}
        />
    }
}

fn find(state: &State, key: u64) -> Option<Action> {
    state
        .actions
        .borrow()
        .iter()
        .find(|action| action.key() == key)
        .cloned()
}

fn start(state: &State) {
    let (focused, path) =
        with_document(|document| (document.focused_node(), document.focus_ancestry()));
    state.returning.set(focused);
    let mut actions = active_actions_from(&path);
    actions.sort_by_key(|action| !action.is_enabled());
    *state.actions.borrow_mut() = actions;
    state.rows.borrow_mut().clear();
    state.set_query.set(String::new());
    filter(state, "");
    state.set_searching.set(true);
}

fn close(state: &State) {
    state.set_searching.set(false);
    state.on_close.call();
    let returning = state.returning.take();
    let overlay = state.overlay.try_get();
    with_document(|document| {
        if let Some(overlay) = overlay.and_then(|id| document.arena.kind_of::<OverlayNode>(id)) {
            document.close_overlay(overlay);
        }
        if returning.is_some_and(|node| document.contains(node)) {
            document.update_focus(returning);
        } else {
            document.update_focus(None);
        }
    });
}

fn dismiss(state: &State) {
    if state.searching.get_untracked() {
        close(state);
    }
}

fn confirm(state: &State, key: u64) {
    let Some(action) = find(state, key) else {
        return;
    };
    if !action.is_enabled() {
        return;
    }
    close(state);
    action.run();
}

fn matches(label: &str, query: &str) -> bool {
    let label = label.to_lowercase();
    query
        .split_whitespace()
        .all(|word| label.contains(&word.to_lowercase()))
}

fn filter(state: &State, text: &str) {
    if state.query.get_untracked() != text {
        state.set_query.set(text.to_owned());
    }
    let actions = state.actions.borrow().clone();
    let shown: Vec<u64> = actions
        .iter()
        .filter(|action| matches(&action.label().peek(), text))
        .map(Action::key)
        .collect();
    let first = actions
        .iter()
        .find(|action| shown.contains(&action.key()) && action.is_enabled())
        .map(Action::key);
    let kept = state
        .highlighted
        .get_untracked()
        .filter(|key| shown.contains(key));
    state.set_shown.set(shown);
    state.set_highlighted.set(kept.or(first));
}

fn navigate(state: &State, press: KeyPress) -> bool {
    if !press.pressed || press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let actions = state.actions.borrow().clone();
    let shown = state.shown.get_untracked();
    let selectable: Vec<u64> = shown
        .iter()
        .copied()
        .filter(|key| {
            actions
                .iter()
                .any(|action| action.key() == *key && action.is_enabled())
        })
        .collect();
    if selectable.is_empty() {
        return matches!(press.key, Key::ArrowDown | Key::ArrowUp);
    }
    let position = state
        .highlighted
        .get_untracked()
        .and_then(|key| selectable.iter().position(|other| *other == key));
    let last = selectable.len() - 1;
    let next = match press.key {
        Key::ArrowDown => selectable[position.map_or(0, |at| (at + 1).min(last))],
        Key::ArrowUp => selectable[position.map_or(0, |at| at.saturating_sub(1))],
        Key::PageDown => selectable[position.map_or(0, |at| (at + 8).min(last))],
        Key::PageUp => selectable[position.map_or(0, |at| at.saturating_sub(8))],
        _ => return false,
    };
    state.set_highlighted.set(Some(next));
    true
}

fn reveal_reader(state: &Handle) -> Prop<Option<usize>> {
    let state = state.clone();
    Prop::Dynamic(Rc::new(move || {
        let shown = state.shown.get();
        let highlighted = state.highlighted.get()?;
        shown.iter().position(|key| *key == highlighted)
    }))
}

pub fn command_palette_search(document: &Document, palette: NodeId) -> NodeId {
    document.component_state::<Handle>(palette).search.get()
}

pub fn command_palette_shown(document: &Document, palette: NodeId) -> Vec<String> {
    let state = document.component_state::<Handle>(palette);
    let actions = state.actions.borrow();
    state
        .shown
        .get_untracked()
        .iter()
        .filter_map(|key| actions.iter().find(|action| action.key() == *key))
        .map(|action| action.id().to_owned())
        .collect()
}

pub fn command_palette_highlighted(document: &Document, palette: NodeId) -> Option<String> {
    let state = document.component_state::<Handle>(palette);
    let key = state.highlighted.get_untracked()?;
    find(state, key).map(|action| action.id().to_owned())
}

pub fn command_palette_row(document: &Document, palette: NodeId, id: &str) -> Option<NodeId> {
    let state = document.component_state::<Handle>(palette);
    let key = state
        .actions
        .borrow()
        .iter()
        .find(|action| action.id() == id)
        .map(Action::key)?;
    let rows = state.rows.borrow();
    rows.iter()
        .rev()
        .find(|(other, _)| *other == key)
        .and_then(|(_, node)| node.try_get())
}

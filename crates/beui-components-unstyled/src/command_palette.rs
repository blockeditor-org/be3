use beui_core::layer::Dismiss;
use std::cell::RefCell;

use accesskit::{Node, Role, Toggled};
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color32;
use beui_core::geometry::Pos2;
use beui_core::input::{Key, KeyPress};
use beui_core::keys::{KeyAt, KeyPhase};
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

use crate as unstyled;
use crate::button::ButtonHandle;
use crate::scroll::ScrollbarStyle;
use crate::text_input::{TextInputHandle, TextInputStyle};

pub struct CommandRowHandle {
    pub label: Prop<String>,
    pub glyph: Option<beui_core::icons::Glyph>,
    pub shortcut: Option<String>,
    pub checked: Memo<Option<bool>>,
    pub disabled: Memo<bool>,
    pub highlighted: Memo<bool>,
    pub hovered: ReadSignal<bool>,
}

#[derive(Clone, Copy)]
struct State {
    overlay: NodeRef,
    search: NodeRef,
    actions: Stored<RefCell<Vec<Action>>>,
    query: Signal<String>,
    shown: Signal<Vec<u64>>,
    highlighted: Signal<Option<u64>>,
    searching: Signal<bool>,
    rows: Stored<RefCell<Vec<(u64, NodeRef)>>>,
    open: Binding<bool>,
}

#[component]
pub fn CommandPalette(
    #[prop(default = ComponentRef::new())] api: ComponentRef<CommandPaletteApi>,
    open: Binding<bool>,
    list_height: Prop<f32>,
    #[prop(default = Color32::TRANSPARENT)] scrim: Prop<Color32>,
    search_placeholder: Prop<String>,
    search_content: Option<RenderFn<TextInputHandle>>,
    #[prop(default = TextInputStyle::default())] search_style: TextInputStyle,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    row: Option<RenderFn<CommandRowHandle>>,
    #[prop(children)] panel: Option<RenderFn<Child>>,
) -> NodeId {
    let row = row.unwrap_or_else(|| {
        RenderFn::new(|_| {
            view! {
                <List spacing=0.0 />
            }
        })
    });
    let panel = panel.unwrap_or_else(|| RenderFn::new(|content| content));
    let query = create_signal(String::new());
    let shown = create_signal(Vec::<u64>::new());
    let highlighted = create_signal(None::<u64>);
    let searching = create_signal(false);
    let state = State {
        overlay: NodeRef::new(),
        search: NodeRef::new(),
        actions: store(RefCell::new(Vec::new())),
        query,
        shown,
        highlighted,
        searching,
        rows: store(RefCell::new(Vec::new())),
        open,
    };
    api.fill(CommandPaletteApi(state));

    create_effect(move || {
        if open.get() {
            untrack(|| start(state));
        }
    });

    let search_placeholder = create_memo(move || search_placeholder.get());
    let highlight = create_selector(move || highlighted.get());
    let keys = create_memo(move || shown.get());
    let reveal = reveal_reader(state);
    on_key(KeyAt::Node(state.search), KeyPhase::Capture, move |event| {
        event.press().is_some_and(|press| navigate(state, press))
    });
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
            on_dismiss={move |_: Dismiss| dismiss(state)}
        >
            {panel.call(view! {
                <List spacing=6.0>
                    <unstyled::TextInput
                        @node_ref={&state.search}
                        @test_id={"command-palette.search"}
                        value={Binding::new(query, move |text: String| filter(state, &text))}
                        focused={searching}
                        placeholder={search_placeholder}
                        style={search_style}
                        accessibility={accessibility}
                        content={Render::new(move |handle: TextInputHandle| match search_content {
                            Some(search) => search.call(handle),
                            None => handle.field,
                        })}
                        on_submit={move |_text: String| {
                            if let Some(key) = state.highlighted.get_untracked() {
                                confirm(state, key);
                            }
                        }}
                    />
                    <unstyled::Scroll
                        @sizing={list_height.map(ItemSize::Fixed)}
                        reveal
                        scrollbar={scrollbar}
                    >
                        <ForEach keys={keys}>|key: u64|
                            <CommandRow state={state} key row={row} highlight={highlight} />
                        </ForEach>
                    </unstyled::Scroll>
                </List>
            })}
        </Overlay>
    }
}

#[component]
fn CommandRow(
    state: State,
    key: u64,
    row: RenderFn<CommandRowHandle>,
    highlight: Selector<Option<u64>>,
) -> NodeId {
    let action = find(state, key).expect("a shown row has an action");
    let button = NodeRef::new();
    state.rows.value().borrow_mut().push((key, button));
    let enabled = action.enabled();
    let disabled = create_memo(move || !enabled.get());
    let label = action.label();
    let checked = action.checked();
    let checked = create_memo(move || checked.map(|checked| checked.get()));
    let accessibility = create_memo({
        let label = label.clone();
        move || {
            let mut node = Node::new(Role::ListBoxOption);
            node.set_label(label.get());
            if let Some(on) = checked.get() {
                node.set_toggled(Toggled::from(on));
            }
            if disabled.get() {
                node.set_disabled();
            }
            node
        }
    });
    let glyph = action.glyph();
    let shortcut = action.shortcut_label();
    view! {
        <unstyled::Button
            @node_ref={&button}
            accessibility
            tab_stop=false
            press_focus=false
            on_click={move || confirm(state, key)}
        >|handle: ButtonHandle| {
            let hovered = handle.hovered;
            let hovering = move || {
                if hovered.get() && find(state, key).is_some_and(|action| action.is_enabled()) {
                    state.highlighted.set(Some(key));
                }
            };
            create_effect(hovering);
            row.call(CommandRowHandle {
                label: label.clone(),
                glyph,
                shortcut: shortcut.clone(),
                checked,
                disabled,
                highlighted: highlight.memo(Some(key)),
                hovered: handle.hovered,
            })
        }</unstyled::Button>
    }
}

fn find(state: State, key: u64) -> Option<Action> {
    state
        .actions
        .value()
        .borrow()
        .iter()
        .find(|action| action.key() == key)
        .cloned()
}

fn start(state: State) {
    let path = focus_ancestry();
    let mut actions = active_actions_from(&path);
    actions.sort_by_key(|action| !action.is_enabled());
    *state.actions.value().borrow_mut() = actions;
    state.rows.value().borrow_mut().clear();
    state.query.set(String::new());
    filter(state, "");
    state.searching.set(true);
}

fn close(state: State) {
    state.searching.set(false);
    state.open.set(false);
    if let Some(overlay) = state.overlay.try_get() {
        overlay.close_overlay();
    }
}

fn dismiss(state: State) {
    if state.searching.get_untracked() {
        close(state);
    }
}

fn confirm(state: State, key: u64) {
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

fn filter(state: State, text: &str) {
    if state.query.get_untracked() != text {
        state.query.set(text.to_owned());
    }
    let actions = state.actions.value().borrow().clone();
    let shown: Vec<u64> = actions
        .iter()
        .filter(|action| matches(&action.label().get_untracked(), text))
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
    state.shown.set(shown);
    state.highlighted.set(kept.or(first));
}

fn navigate(state: State, press: KeyPress) -> bool {
    if !press.pressed || press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let actions = state.actions.value().borrow().clone();
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
    state.highlighted.set(Some(next));
    true
}

fn reveal_reader(state: State) -> Prop<Option<usize>> {
    Prop::dynamic(move || {
        let shown = state.shown.get();
        let highlighted = state.highlighted.get()?;
        shown.iter().position(|key| *key == highlighted)
    })
}

#[derive(Clone, Copy)]
pub struct CommandPaletteApi(State);

impl CommandPaletteApi {
    pub fn search(&self) -> NodeId {
        self.0.search.get()
    }

    pub fn shown(&self) -> Vec<String> {
        let state = self.0;
        let actions = state.actions.value();
        let actions = actions.borrow();
        state
            .shown
            .get_untracked()
            .iter()
            .filter_map(|key| actions.iter().find(|action| action.key() == *key))
            .map(|action| action.id().to_owned())
            .collect()
    }

    pub fn highlighted(&self) -> Option<String> {
        let state = self.0;
        let key = state.highlighted.get_untracked()?;
        find(state, key).map(|action| action.id().to_owned())
    }

    pub fn row(&self, id: &str) -> Option<NodeId> {
        let state = self.0;
        let key = state
            .actions
            .value()
            .borrow()
            .iter()
            .find(|action| action.id() == id)
            .map(Action::key)?;
        let rows = state.rows.value();
        let rows = rows.borrow();
        rows.iter()
            .rev()
            .find(|(other, _)| *other == key)
            .and_then(|(_, node)| node.try_get())
    }
}

use crate as unstyled;
use crate::ChoiceOption;
use crate::button::ButtonHandle;
use crate::scroll::ScrollbarStyle;
use crate::text_input::{TextInputHandle, TextInputStyle};
use beui_core::base::overlay::Placement;
use beui_core::document::Document;
use beui_core::input::{Key, KeyPress};
use beui_core::keys::{KeyAt, KeyPhase};
use beui_core::layer::Dismiss;
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Callback, Child, Children, Frame, IntoProp, ItemSize, List, Memo, NodeRef, Prop, ReadSignal,
    Render, RenderFn, Run, Selector, Stored, WriteSignal, create_effect, create_memo,
    create_selector, create_signal, on_key, set_component_state, store,
};
use std::cell::RefCell;

use accesskit::{Node, Role};

const OPTIONS_MAX_HEIGHT: f32 = 240.0;

pub struct SelectTriggerHandle {
    pub selected: ReadSignal<Option<usize>>,
    pub label: Memo<Option<String>>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
}

pub struct SelectOptionHandle {
    pub index: usize,
    pub label: Prop<String>,
    pub highlighted: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Focus {
    Away,
    Search,
}

struct Row {
    button: NodeRef,
    label: Memo<String>,
    visible: ReadSignal<bool>,
    set_visible: WriteSignal<bool>,
}

#[derive(Clone, Copy)]
struct State {
    trigger: NodeRef,
    search: NodeRef,
    options: Stored<Run<ChoiceOption>>,
    rows: Stored<RefCell<Vec<Row>>>,
    open: ReadSignal<bool>,
    set_open: WriteSignal<bool>,
    focus: ReadSignal<Focus>,
    set_focus: WriteSignal<Focus>,
    set_search_text: WriteSignal<String>,
    selected: ReadSignal<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
    highlighted: ReadSignal<Option<usize>>,
    set_highlighted: WriteSignal<Option<usize>>,
    on_change: Callback<Option<usize>>,
}

#[component]
pub fn Select(
    options: Children<ChoiceOption>,
    selected: Prop<Option<usize>>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_change: Callback<Option<usize>>,
    search_placeholder: Prop<String>,
    search_content: Option<Render<TextInputHandle>>,
    #[prop(default = TextInputStyle::default())] search_style: TextInputStyle,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    trigger: Option<Render<SelectTriggerHandle>>,
    option: Option<RenderFn<SelectOptionHandle>>,
    #[prop(children)] popup: Option<Render<Child>>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let selected_prop = selected;
    let options = options.into_run();
    let initial = selected_prop
        .peek()
        .filter(|index| *index < options.peek().len());
    let option = option.unwrap_or_else(|| {
        RenderFn::new(|_| {
            view! {
                <List spacing=0.0 />
            }
        })
    });
    let popup = popup.unwrap_or_else(|| Render::new(|content| content));

    let (highlighted, set_highlighted) = create_signal(initial);
    let highlight = create_selector(move || highlighted.get());
    let (selected, set_selected) = create_signal(initial);
    let (is_open, set_open) = create_signal(false);
    let (focus, set_focus) = create_signal(Focus::Away);
    let focused = create_selector(move || focus.get());
    let (search_text, set_search_text) = create_signal(String::new());
    let disabled = create_memo(move || disabled.get());

    let state = State {
        trigger: NodeRef::new(),
        search: NodeRef::new(),
        options: store(options.clone()),
        rows: store(RefCell::default()),
        open: is_open,
        set_open,
        focus,
        set_focus,
        set_search_text,
        selected,
        set_selected,
        highlighted,
        set_highlighted,
        on_change,
    };
    set_component_state(state);

    let trigger_view = trigger.unwrap_or_else(|| {
        Render::new(|_| {
            view! {
                <List spacing=0.0 />
            }
        })
    });
    let accessibility = crate::labelled_node(Role::ComboBox, accessibility, label);
    let trigger_accessibility = create_memo(move || {
        let mut node = accessibility.get();
        if let Some(label) = state
            .selected
            .get()
            .and_then(|index| state.rows.get().borrow().get(index).map(|row| row.label))
        {
            node.set_value(label.get());
        }
        node.set_expanded(state.open.get());
        node
    });
    let trigger_disabled = disabled;
    let labelled = options.clone();
    let chosen = selected;
    let label = create_memo(move || {
        let index = chosen.get()?;
        labelled
            .items()
            .get(index)
            .map(|option| option.label().get())
    });
    let trigger_content = move |handle: ButtonHandle| {
        trigger_view.call(SelectTriggerHandle {
            selected,
            label,
            hovered: handle.hovered,
            active: handle.active,
            focused: handle.focused,
            disabled: trigger_disabled,
        })
    };

    let items = options.build_each(
        state.rows,
        |_, option| {
            let (visible, set_visible) = create_signal(true);
            Row {
                button: NodeRef::new(),
                label: option.label(),
                visible,
                set_visible,
            }
        },
        move |index| {
            view! {
                <SelectRow state={state} index option={option} highlight={highlight} />
            }
        },
    );

    create_effect(move || {
        state.options.get().len();
        apply_requested_selection(state, selected_prop.get());
    });

    let reveal = reveal_reader(state);
    on_key(KeyAt::Node(state.search), KeyPhase::Capture, move |event| {
        event
            .press()
            .is_some_and(|press| navigate(state, press))
    });

    view! {
        <List spacing=0.0>
            <unstyled::Button
                @node_ref={&state.trigger}
                accessibility={trigger_accessibility}
                disabled={disabled}
                content={trigger_content}
                on_click={move || open(state)}
                on_key={move |press: KeyPress| trigger_key(state, press)}
            />
            <Overlay
                anchor={state.trigger}
                placement=Placement::BelowStart
                open={is_open}
                on_dismiss={move |_: Dismiss| dismiss(state)}
            >
                {popup.call(view! {
                    <List spacing=6.0>
                        <unstyled::TextInput
                            @node_ref={&state.search}
                            value={search_text}
                            focused={focused.memo(Focus::Search)}
                            keyboard_on_focus=false
                            placeholder={search_placeholder}
                            style={search_style}
                            content={search_content.unwrap_or_else(|| Render::new(|handle: TextInputHandle| handle.field))}
                            on_focus_change={move |has_focus: bool| blur(state, has_focus, Focus::Search)}
                            on_change={move |text: String| filter(state, &text)}
                            on_submit={move |_text: String| {
                                if let Some(index) = state.highlighted.get_untracked() {
                                    confirm(state, index);
                                }
                            }}
                        />
                        <unstyled::Scroll
                            @sizing=ItemSize::Fixed(OPTIONS_MAX_HEIGHT)
                            reveal
                            scrollbar={scrollbar}
                            children={items}
                        />
                    </List>
                })}
            </Overlay>
        </List>
    }
}

#[component]
fn SelectRow(
    state: State,
    index: usize,
    option: RenderFn<SelectOptionHandle>,
    highlight: Selector<Option<usize>>,
) -> NodeId {
    let (visible, button, label) = {
        let rows = state.rows.get();
        let row = &rows.borrow()[index];
        (row.visible, row.button, row.label)
    };
    let accessibility_label = label;
    let accessibility = create_memo(move || {
        let mut node = Node::new(Role::ListBoxOption);
        node.set_label(accessibility_label.get());
        node.set_selected(state.selected.get() == Some(index));
        node
    });
    view! {
        <Frame visible>
            <unstyled::Button
                @node_ref={&button}
                accessibility
                tab_stop=false
                content={move |button: ButtonHandle| {
                    let hovered = button.hovered;
                    create_effect(move || {
                        if hovered.get() {
                            state.set_highlighted.set(Some(index));
                        }
                    });
                    option.call(SelectOptionHandle {
                        index,
                        label: label.into_prop(),
                        highlighted: highlight.memo(Some(index)),
                        hovered: button.hovered,
                        focused: button.focused,
                    })
                }}
                on_click={move || confirm(state, index)}
            />
        </Frame>
    }
}

fn reveal_reader(state: State) -> Prop<Option<usize>> {
    Prop::dynamic(move || {
        let visible: Vec<_> = state.rows.get().borrow().iter().map(|row| row.visible).collect();
        for row in visible {
            row.get();
        }
        state.highlighted.get()
    })
}

fn blur(state: State, has_focus: bool, target: Focus) {
    if !has_focus && state.focus.get_untracked() == target {
        state.set_focus.set(Focus::Away);
    }
}

pub fn select_selected(document: &Document, select: NodeId) -> Option<usize> {
    document.component_state::<State>(select).selected.get()
}

pub fn select_open(document: &Document, select: NodeId) -> bool {
    document.component_state::<State>(select).open.get()
}

pub fn select_trigger(document: &Document, select: NodeId) -> NodeId {
    document.component_state::<State>(select).trigger.get()
}

pub fn select_search(document: &Document, select: NodeId) -> NodeId {
    document.component_state::<State>(select).search.get()
}

pub fn select_option_button(document: &Document, select: NodeId, index: usize) -> NodeId {
    document.component_state::<State>(select).rows.get().borrow()[index]
        .button
        .get()
}

pub fn select_highlighted(document: &Document, select: NodeId) -> Option<usize> {
    document
        .component_state::<State>(select)
        .highlighted
        .get_untracked()
}

fn apply_requested_selection(state: State, selected: Option<usize>) {
    let selected = selected.filter(|index| *index < state.options.get().peek().len());
    if state.selected.get_untracked() != selected {
        state.set_selected.set(selected);
    }
}

fn trigger_key(state: State, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    if !matches!(
        press.key,
        Key::ArrowDown | Key::ArrowUp | Key::Home | Key::End
    ) {
        return false;
    }
    if state.open.get_untracked() {
        return false;
    }
    if press.pressed {
        open(state);
        navigate(state, press);
    }
    true
}

fn open(state: State) {
    state.set_open.set(true);
    filter(state, "");
    state.set_highlighted.set(state.selected.get_untracked());
    state.set_focus.set(Focus::Search);
}

fn dismiss(state: State) {
    state.set_open.set(false);
    state.set_focus.set(Focus::Away);
}

fn confirm(state: State, index: usize) {
    apply_selection(state, Some(index));
    dismiss(state);
}

fn apply_selection(state: State, selected: Option<usize>) {
    state.set_selected.set(selected);
    state.on_change.call(selected);
}

fn filter(state: State, text: &str) {
    state.set_search_text.set(text.to_owned());
    let query = text.to_lowercase();
    let mut first_visible = None;
    let rows: Vec<_> = state
        .rows.get()
        .borrow()
        .iter()
        .map(|row| (row.label.get_untracked(), row.set_visible))
        .collect();
    for (index, (label, set_visible)) in rows.into_iter().enumerate() {
        let visible = query.is_empty() || label.to_lowercase().contains(&query);
        set_visible.set(visible);
        if visible && first_visible.is_none() {
            first_visible = Some(index);
        }
    }
    let still_visible = state.highlighted.get_untracked().is_some_and(|index| {
        state
            .rows.get()
            .borrow()
            .get(index)
            .is_some_and(|row| row.visible.get_untracked())
    });
    if !still_visible {
        state.set_highlighted.set(first_visible);
    }
}

fn navigate(state: State, press: KeyPress) -> bool {
    if !press.pressed || press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let visible: Vec<usize> = state
        .rows.get()
        .borrow()
        .iter()
        .enumerate()
        .filter(|(_, row)| row.visible.get_untracked())
        .map(|(index, _)| index)
        .collect();
    if visible.is_empty() {
        return false;
    }
    let current = state.highlighted.get_untracked();
    let position = current.and_then(|index| visible.iter().position(|&i| i == index));
    let next = match press.key {
        Key::ArrowDown => visible[position.map_or(0, |p| (p + 1).min(visible.len() - 1))],
        Key::ArrowUp => visible[position.map_or(0, |p| p.saturating_sub(1))],
        Key::Home => visible[0],
        Key::End => *visible.last().expect("checked non-empty"),
        _ => return false,
    };
    state.set_highlighted.set(Some(next));
    true
}

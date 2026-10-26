use crate as unstyled;
use crate::ChoiceOption;
use crate::button::ButtonHandle;
use crate::scroll::ScrollbarStyle;
use crate::search_list::{SearchList, SearchListApi, SearchRowHandle};
use crate::text_input::{TextInputHandle, TextInputStyle};
use beui_core::base::overlay::Placement;
use beui_core::input::{Key, KeyPress};
use beui_core::layer::Dismiss;
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

use accesskit::{Node, Role};

pub struct SelectTriggerHandle {
    pub selected: Binding<Option<usize>>,
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
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Focus {
    Away,
    Search,
}

#[derive(Clone, Copy)]
struct State {
    trigger: NodeRef,
    options: Stored<Run<ChoiceOption>>,
    list: ComponentRef<SearchListApi<usize>>,
    open: Signal<bool>,
    focus: Signal<Focus>,
    query: Signal<String>,
    selected: Binding<Option<usize>>,
    highlighted: Signal<Option<usize>>,
}

#[component]
pub fn Select(
    #[prop(default = ComponentRef::new())] api: ComponentRef<SelectApi>,
    options: Children<ChoiceOption>,
    #[prop(default = None)] selected: Binding<Option<usize>>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    search_placeholder: Prop<String>,
    search_content: RenderFn<TextInputHandle>,
    #[prop(default = TextInputStyle::default())] search_style: TextInputStyle,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    list_height: Prop<f32>,
    spacing: f32,
    trigger: Render<SelectTriggerHandle>,
    option: RenderFn<SelectOptionHandle>,
    #[prop(children)] popup: Option<RenderFn<Child>>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let options = options.into_run();
    let initial = selected
        .get_untracked()
        .filter(|index| *index < options.get_untracked().len());
    let popup = popup.unwrap_or_else(|| RenderFn::new(|content| content));

    let search_placeholder = create_memo(move || search_placeholder.get());
    let is_open = create_signal(false);
    let focus = create_signal(Focus::Away);
    let focused = create_selector(move || focus.get());
    let disabled = create_memo(move || disabled.get());

    let state = State {
        trigger: NodeRef::new(),
        options: store(options.clone()),
        list: ComponentRef::new(),
        open: is_open,
        focus,
        query: create_signal(String::new()),
        selected,
        highlighted: create_signal(initial),
    };
    api.fill(SelectApi(state));

    let accessibility = crate::labelled_node(Role::ComboBox, accessibility, label);
    let labelled = options.clone();
    let label = create_memo(move || {
        let index = selected.get()?;
        labelled
            .items()
            .get(index)
            .map(|option| option.label().get())
    });
    let trigger_accessibility = create_memo(move || {
        let mut node = accessibility.get();
        if let Some(label) = label.get() {
            node.set_value(label);
        }
        node.set_expanded(state.open.get());
        node
    });
    let trigger_content = move |handle: ButtonHandle| {
        trigger.call(SelectTriggerHandle {
            selected,
            label,
            hovered: handle.hovered,
            active: handle.active,
            focused: handle.focused,
            disabled,
        })
    };

    let rank = move |query: String| {
        let options = state.options.value().items();
        crate::search_list::rank(&options, &query, |option, query| {
            crate::search_list::substring_score(&option.label().get(), query)
        })
    };
    let row_accessibility = move |index: usize| {
        let mut node = Node::new(Role::ListBoxOption);
        if let Some(option) = state.options.value().items().get(index) {
            node.set_label(option.label().get());
        }
        node.set_selected(state.selected.get() == Some(index));
        node
    };
    let row = move |handle: SearchRowHandle<usize>| {
        let index = handle.key;
        let label = state
            .options
            .value()
            .get_untracked()
            .get(index)
            .map(|option| option.label())
            .unwrap_or_else(|| create_memo(String::new));
        option.call(SelectOptionHandle {
            index,
            label: label.into_prop(),
            highlighted: handle.highlighted,
            hovered: handle.hovered,
        })
    };

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
                content={move || popup.call(view! {
                    <SearchList
                        api={state.list}
                        query={state.query}
                        highlighted={state.highlighted}
                        rank
                        highlight_first=false
                        on_choose={move |index: usize| confirm(state, index)}
                        focused={focused.memo(Focus::Search)}
                        keyboard_on_focus=false
                        on_focus_change={move |has_focus: bool| blur(state, has_focus)}
                        placeholder={search_placeholder}
                        style={search_style}
                        field={search_content}
                        list_height
                        spacing
                        scrollbar
                        accessibility={row_accessibility}
                        row
                    />
                })}
            />
        </List>
    }
}

fn blur(state: State, has_focus: bool) {
    if !has_focus && state.focus.get_untracked() == Focus::Search {
        state.focus.set(Focus::Away);
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
        let count = state.options.value().get_untracked().len();
        let current = state.highlighted.get_untracked();
        let next = match press.key {
            Key::ArrowDown => current.map_or(0, |at| (at + 1).min(count.saturating_sub(1))),
            Key::ArrowUp => current.map_or(0, |at| at.saturating_sub(1)),
            Key::Home => 0,
            _ => count.saturating_sub(1),
        };
        state.highlighted.set((count > 0).then_some(next));
    }
    true
}

fn open(state: State) {
    state.query.set(String::new());
    state.highlighted.set(state.selected.get_untracked());
    state.open.set(true);
    state.focus.set(Focus::Search);
}

fn dismiss(state: State) {
    state.open.set(false);
    state.focus.set(Focus::Away);
}

fn confirm(state: State, index: usize) {
    state.selected.update(|selected| *selected = Some(index));
    dismiss(state);
}

#[derive(Clone, Copy)]
pub struct SelectApi(State);

impl SelectApi {
    pub fn selected(&self) -> Option<usize> {
        let state = self.0;
        state
            .selected
            .get_untracked()
            .filter(|index| *index < state.options.value().get_untracked().len())
    }

    pub fn open(&self) -> bool {
        self.0.open.get()
    }

    pub fn trigger(&self) -> NodeId {
        self.0.trigger.get()
    }

    pub fn search(&self) -> NodeId {
        self.0.list.get().search()
    }

    pub fn option_button(&self, index: usize) -> Option<NodeId> {
        self.0.list.try_get()?.row(&index)
    }

    pub fn highlighted(&self) -> Option<usize> {
        self.0.highlighted.get_untracked()
    }
}

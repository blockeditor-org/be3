use crate as unstyled;
use crate::ChoiceOption;
use crate::anchored::{Anchored, AnchoredHandle, AnchoredStyle};
use crate::button::ButtonHandle;
use crate::choice::{position, value_at};
use crate::scroll::ScrollbarStyle;
use crate::search_list::{SearchList, SearchListApi, SearchRowHandle};
use crate::text_input::{TextInputHandle, TextInputStyle};
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{Key, KeyPress};

use accesskit::{Node, Role};

pub struct SelectTriggerHandle {
    pub label: Memo<Option<String>>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
}

pub struct SelectOptionHandle {
    pub label: Prop<String>,
    pub highlighted: Memo<bool>,
    pub hovered: ReadSignal<bool>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Focus {
    Away,
    Search,
}

struct State<T: 'static> {
    trigger: NodeRef,
    options: Stored<Run<ChoiceOption<T>>>,
    lists: [ComponentRef<SearchListApi<usize>>; 2],
    sheet: Signal<bool>,
    open: Signal<bool>,
    focus: Signal<Focus>,
    query: Signal<String>,
    selected: Binding<Option<T>>,
    shown: Memo<Option<usize>>,
    highlighted: Signal<Option<usize>>,
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for State<T> {}

#[component]
pub fn Select<T>(
    #[prop(default = ComponentRef::new())] api: ComponentRef<SelectApi<T>>,
    options: Children<ChoiceOption<T>>,
    #[prop(default = None)] selected: Binding<Option<T>>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    search_placeholder: Prop<String>,
    search_content: RenderFn<TextInputHandle>,
    search_style: TextInputStyle,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    list_height: Prop<f32>,
    spacing: f32,
    trigger: Render<SelectTriggerHandle>,
    option: RenderFn<SelectOptionHandle>,
    surface: AnchoredStyle,
    accessibility: Option<Prop<Node>>,
) -> NodeId
where
    T: Clone + PartialEq + 'static,
{
    let options = options.into_run();
    let among = options.clone();
    let shown = create_memo(move || {
        selected.with(|selected| selected.as_ref().and_then(|value| position(&among, value)))
    });

    let search_placeholder = create_memo(move || search_placeholder.get());
    let is_open = create_signal(false);
    let focus = create_signal(Focus::Away);
    let focused = create_selector(move || focus.get());
    let disabled = create_memo(move || disabled.get());

    let state = State {
        trigger: NodeRef::new(),
        options: store(options.clone()),
        lists: [ComponentRef::new(), ComponentRef::new()],
        sheet: create_signal(false),
        open: is_open,
        focus,
        query: create_signal(String::new()),
        selected,
        shown,
        highlighted: create_signal(shown.get_untracked()),
    };
    api.fill(SelectApi(state));

    let accessibility = crate::labelled_node(Role::ComboBox, accessibility, label);
    let labelled = options.clone();
    let label = create_memo(move || {
        let index = shown.get()?;
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
        node.set_selected(state.shown.get() == Some(index));
        node
    };
    let row = move |handle: SearchRowHandle<usize>| {
        let index = handle.key;
        let held = state.options.value().get_untracked().get(index).cloned();
        let label = held
            .as_ref()
            .map(|option| option.label())
            .unwrap_or_else(|| create_memo(String::new));
        let built = option.call(SelectOptionHandle {
            label: label.into_prop(),
            highlighted: handle.highlighted,
            hovered: handle.hovered,
        });
        if let Some(test_id) = held.and_then(|option| option.test_id()) {
            bind_test_id(built, test_id);
        }
        built
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
            <Anchored
                style={surface}
                anchor={state.trigger}
                open={is_open}
                on_dismiss={move || dismiss(state)}
            >|handle: AnchoredHandle| {
                let AnchoredHandle { sheet, shown } = handle;
                create_effect(move || {
                    if shown.get() {
                        state.sheet.set(sheet);
                    }
                });
                view! {
                    <SearchList
                        api={state.lists[usize::from(sheet)]}
                        query={state.query}
                        highlighted={state.highlighted}
                        rank
                        highlight_first=false
                        on_choose={move |index: usize| confirm(state, index)}
                        focused={create_memo(move || shown.get() && focused.is_selected(&Focus::Search))}
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
                }
            }</Anchored>
        </List>
    }
}

fn blur<T>(state: State<T>, has_focus: bool) {
    if !has_focus && state.focus.get_untracked() == Focus::Search {
        state.focus.set(Focus::Away);
    }
}

fn trigger_key<T>(state: State<T>, press: KeyPress) -> bool {
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

fn open<T>(state: State<T>) {
    state.query.set(String::new());
    state.highlighted.set(state.shown.get_untracked());
    state.open.set(true);
    state.focus.set(Focus::Search);
}

fn dismiss<T>(state: State<T>) {
    state.open.set(false);
    state.focus.set(Focus::Away);
}

fn confirm<T: Clone + 'static>(state: State<T>, index: usize) {
    if let Some(value) = value_at(&state.options.value(), index) {
        state.selected.update(|selected| *selected = Some(value));
    }
    dismiss(state);
}

pub struct SelectApi<T: 'static>(State<T>);

impl<T> Clone for SelectApi<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for SelectApi<T> {}

impl<T: Clone + PartialEq + 'static> SelectApi<T> {
    fn list(&self) -> ComponentRef<SearchListApi<usize>> {
        self.0.lists[usize::from(self.0.sheet.get_untracked())]
    }

    pub fn selected(&self) -> Option<T> {
        let state = self.0;
        value_at(&state.options.value(), state.shown.get_untracked()?)
    }

    pub fn open(&self) -> bool {
        self.0.open.get()
    }

    pub fn trigger(&self) -> NodeId {
        self.0.trigger.get()
    }

    pub fn search(&self) -> NodeId {
        self.list().get().search()
    }

    pub fn option_button(&self, value: &T) -> Option<NodeId> {
        let index = self
            .0
            .options
            .value()
            .get_untracked()
            .iter()
            .position(|option| option.value() == value)?;
        self.list().try_get()?.row(&index)
    }

    pub fn highlighted(&self) -> Option<T> {
        value_at(&self.0.options.value(), self.0.highlighted.get_untracked()?)
    }
}

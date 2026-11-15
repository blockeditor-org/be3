use crate as unstyled;
use crate::ChoiceOption;
use crate::anchored::{Anchored, AnchoredHandle, AnchoredStyle};
use crate::button::ButtonHandle;
use crate::form::Posted;
use crate::scroll::ScrollStyle;
use crate::search_list::{SearchList, SearchListApi, SearchRowHandle};
use crate::text_input::{TextInputHandle, TextInputStyle};
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{Key, KeyPress, Placement};

use accesskit::{Node, Role};

pub struct SelectTriggerHandle {
    pub label: Memo<Option<String>>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
}

pub struct SelectOptionHandle<T> {
    pub value: T,
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
    chosen: Memo<Option<usize>>,
    highlighted: Signal<Option<usize>>,
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for State<T> {}

/// A trigger that opens a popup with a search box over the options.
///
/// Opening it focuses the search box; typing filters the options to those whose
/// label contains every typed word, ignoring case. Up/Down/Home/End on the
/// closed trigger open it with the highlight moved that way. In the popup,
/// Up/Down move the highlight without moving the search box's caret, Page
/// Up/Down move eight options, and Home/End reach the first and last shown
/// option; Enter picks the highlighted one and closes. Escape or a press outside
/// closes without changing the selection and returns the focus to the trigger.
///
/// `selected` holds the chosen option's value; a value no option holds shows
/// nothing chosen.
#[component]
pub fn Select<T>(
    #[prop(default = ComponentRef::new())] api: ComponentRef<SelectApi<T>>,
    options: Children<ChoiceOption<T>>,
    #[prop(default = None)] selected: Binding<Option<T>>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = String::new())] name: Prop<String>,
    search_placeholder: Prop<String>,
    search_content: RenderFn<TextInputHandle>,
    search_style: TextInputStyle,
    #[prop(default = ScrollStyle::default())] scroll: ScrollStyle,
    list_height: Prop<f32>,
    spacing: f32,
    trigger: Render<SelectTriggerHandle>,
    option: RenderFn<SelectOptionHandle<T>>,
    surface: AnchoredStyle,
    accessibility: Option<Prop<Node>>,
) -> NodeId
where
    T: Clone + PartialEq + 'static,
{
    let options = options.into_run();
    let among = options.clone();
    let chosen = create_memo(move || {
        selected.with(|selected| {
            selected
                .as_ref()
                .and_then(|value| crate::choice::position(&among, value))
        })
    });
    let initial = chosen.get_untracked();

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
        chosen,
        highlighted: create_signal(initial),
    };
    api.fill(SelectApi(state));

    let accessibility = crate::labelled_node(Role::ComboBox, accessibility, label);
    let labelled = options.clone();
    let label = create_memo(move || {
        let index = chosen.get()?;
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
        node.set_selected(state.chosen.get() == Some(index));
        node
    };
    let row = move |handle: SearchRowHandle<usize>| {
        let index = handle.key;
        let Some(shown) = state.options.value().get_untracked().get(index).cloned() else {
            return view! {
                <Frame />
            };
        };
        let built = option.call(SelectOptionHandle {
            value: shown.value().clone(),
            label: shown.label().into_prop(),
            highlighted: handle.highlighted,
            hovered: handle.hovered,
        });
        if let Some(test_id) = shown.test_id() {
            bind_test_id(built, test_id);
        }
        built
    };

    let valued = options.clone();
    let posted = create_memo(move || {
        let index = chosen.get()?;
        valued.items().get(index).map(|option| option.posted().get())
    });
    let select = view! {
        <List spacing=0.0>
            <unstyled::Button
                @node_ref={&state.trigger}
                accessibility={trigger_accessibility}
                disabled={disabled}
                content={trigger_content}
                on_click={move || toggle(state)}
                on_key={move |press: KeyPress| trigger_key(state, press)}
            />
            <Anchored
                style={surface}
                anchor={state.trigger}
                placement=Placement::Menu
                trigger={state.trigger}
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
                        scroll
                        accessibility={row_accessibility}
                        row
                    />
                }
            }</Anchored>
        </List>
    };
    view! {
        <Posted name value={posted} disabled content={select} />
    }
}

fn blur<T>(state: State<T>, has_focus: bool) {
    if !has_focus && state.focus.get_untracked() == Focus::Search {
        state.focus.set(Focus::Away);
    }
}

fn trigger_key<T: Clone + PartialEq + 'static>(state: State<T>, press: KeyPress) -> bool {
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

fn toggle<T: Clone + PartialEq + 'static>(state: State<T>) {
    match state.open.get_untracked() {
        true => dismiss(state),
        false => open(state),
    }
}

fn open<T: Clone + PartialEq + 'static>(state: State<T>) {
    state.query.set(String::new());
    state.highlighted.set(state.chosen.get_untracked());
    state.open.set(true);
    state.focus.set(Focus::Search);
}

fn dismiss<T>(state: State<T>) {
    state.open.set(false);
    state.focus.set(Focus::Away);
}

fn confirm<T: Clone + PartialEq + 'static>(state: State<T>, index: usize) {
    if let Some(value) = crate::choice::value_at(&state.options.value(), index) {
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

    /// The chosen option's value, or `None` while no option holds the bound value.
    pub fn selected(&self) -> Option<T> {
        let state = self.0;
        let index = state.chosen.get_untracked()?;
        crate::choice::value_at(&state.options.value(), index)
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

    /// The row the open popup shows for the option holding `value`.
    pub fn option_button(&self, value: &T) -> Option<NodeId> {
        let index = crate::choice::position(&self.0.options.value(), value)?;
        self.list().try_get()?.row(&index)
    }

    /// The value of the option the popup highlights.
    pub fn highlighted(&self) -> Option<T> {
        crate::choice::value_at(&self.0.options.value(), self.0.highlighted.get_untracked()?)
    }
}

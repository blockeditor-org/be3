use crate as unstyled;
use crate::ChoiceOption;
use crate::anchored::{Anchored, AnchoredHandle, AnchoredStyle};
use crate::button::ButtonHandle;
use crate::choice::{keyed, option_key};
use crate::form::Posted;
use crate::scroll::ScrollStyle;
use crate::search_list::{SearchList, SearchListApi, SearchRowHandle};
use crate::text_input::{TextInputHandle, TextInputStyle};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{Key, KeyPress, Placement, Pos2, Rect};

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
    lists: [ComponentRef<SearchListApi<u64>>; 2],
    sheet: Signal<bool>,
    open: Binding<bool>,
    /// Whether `open` has reset the search and highlight for this opening.
    prepared: Signal<bool>,
    focus: Signal<Focus>,
    query: Signal<String>,
    selected: Binding<Option<T>>,
    chosen: Memo<Option<u64>>,
    highlighted: Signal<Option<u64>>,
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
    selected: Binding<Option<T>>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = String::new())] name: Prop<String>,
    search_placeholder: Prop<String>,
    search_content: RenderFn<TextInputHandle, NodeId>,
    search_style: TextInputStyle,
    scroll: ScrollStyle,
    list_height: Prop<f32>,
    spacing: f32,
    trigger: Render<SelectTriggerHandle, NodeId>,
    option: RenderFn<SelectOptionHandle<T>, NodeId>,
    surface: AnchoredStyle,
    #[prop(default = false)] open: Binding<bool>,
    #[prop(default = None)] at: Binding<Option<Pos2>>,
    #[prop(default = Placement::BELOW_START)] placement: Prop<Placement>,
    #[prop(default = None)] bounds: Prop<Option<Rect>>,
    #[prop(default = false)] keep_alive: bool,
    accessibility: Option<Prop<Node>>,
) -> NodeId
where
    T: Clone + PartialEq + 'static,
{
    let options = options.into_run();
    let among = options.clone();
    let chosen = create_memo(move || {
        selected.with(|selected| {
            let selected = selected.as_ref()?;
            among
                .items()
                .iter()
                .find(|option| option.value() == selected)
                .map(|option| option_key(option))
        })
    });
    let initial = chosen.get_untracked();

    let search_placeholder = create_memo(move || search_placeholder.get());
    let focus = create_signal(Focus::Away);
    let focused = create_selector(move || focus.get());
    let disabled = create_memo(move || disabled.get());

    let state = State {
        trigger: NodeRef::new(),
        options: store(options.clone()),
        lists: [ComponentRef::new(), ComponentRef::new()],
        sheet: create_signal(false),
        open,
        prepared: create_signal(false),
        focus,
        query: create_signal(String::new()),
        selected,
        chosen,
        highlighted: create_signal(initial),
    };
    api.fill(SelectApi(state));
    let mut was_open = false;
    create_effect(move || {
        let now = open.get();
        untrack(|| match (was_open, now) {
            (false, true) if !state.prepared.get_untracked() => prepare(state),
            (true, false) => {
                state.prepared.set(false);
                state.focus.set(Focus::Away);
            }
            _ => {}
        });
        was_open = now;
    });

    let accessibility = crate::labelled_node(Role::ComboBox, accessibility, label);
    let labelled = options.clone();
    let label = create_memo(move || Some(keyed(&labelled, chosen.get()?)?.label().get()));
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
        .into_iter()
        .map(|index| option_key(&options[index]))
        .collect()
    };
    let row_accessibility = move |key: u64| {
        let mut node = Node::new(Role::ListBoxOption);
        if let Some(option) = keyed(&state.options.value(), key) {
            node.set_label(option.label().get());
        }
        node.set_selected(state.chosen.get() == Some(key));
        node
    };
    let row = move |handle: SearchRowHandle<u64>| {
        let Some(shown) = untrack(|| keyed(&state.options.value(), handle.key)) else {
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
    let posted = create_memo(move || Some(keyed(&valued, chosen.get()?)?.posted().get()));
    let select = view! {
        <List spacing=0.0>
            <unstyled::Button
                @node_ref={&state.trigger}
                accessibility={move || {
                    let mut node = accessibility.get();
                    if let Some(label) = label.get() {
                        node.set_value(label);
                    }
                    node.set_expanded(state.open.get());
                    node
                }}
                disabled={disabled}
                content={trigger_content}
                on_click={move || toggle(state)}
                on_key={move |press: KeyPress| trigger_key(state, press)}
            />
            <Anchored
                style={surface}
                anchor={state.trigger}
                at
                placement
                bounds
                keep_alive
                trigger={Some(state.trigger)}
                open
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
                        on_choose={move |key: u64| confirm(state, key)}
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
        let options = state.options.value().get_untracked();
        let count = options.len();
        let current = state
            .highlighted
            .get_untracked()
            .and_then(|key| options.iter().position(|option| option_key(option) == key));
        let next = match press.key {
            Key::ArrowDown => current.map_or(0, |at| (at + 1).min(count.saturating_sub(1))),
            Key::ArrowUp => current.map_or(0, |at| at.saturating_sub(1)),
            Key::Home => 0,
            _ => count.saturating_sub(1),
        };
        state
            .highlighted
            .set(options.get(next).map(|option| option_key(option)));
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
    prepare(state);
    state.open.set(true);
}

/// Clears the search, highlights the chosen option and focuses the search box, once for
/// each opening, however it opened.
fn prepare<T: Clone + PartialEq + 'static>(state: State<T>) {
    state.prepared.set(true);
    state.query.set(String::new());
    state.highlighted.set(state.chosen.get_untracked());
    state.focus.set(Focus::Search);
}

fn dismiss<T>(state: State<T>) {
    state.open.set(false);
}

/// Chooses the option the row was built for, wherever it now sits among the options.
fn confirm<T: Clone + PartialEq + 'static>(state: State<T>, key: u64) {
    if let Some(option) = untrack(|| keyed(&state.options.value(), key)) {
        let value = option.value().clone();
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
    fn list(&self) -> ComponentRef<SearchListApi<u64>> {
        self.0.lists[usize::from(self.0.sheet.get_untracked())]
    }

    /// The chosen option's value, or `None` while no option holds the bound value.
    pub fn selected(&self) -> Option<T> {
        let state = self.0;
        let key = state.chosen.get_untracked()?;
        untrack(|| keyed(&state.options.value(), key)).map(|option| option.value().clone())
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
        let key = self
            .0
            .options
            .value()
            .get_untracked()
            .iter()
            .find(|option| option.value() == value)
            .map(|option| option_key(option))?;
        self.list().try_get()?.row(&key)
    }

    /// The value of the option the popup highlights.
    pub fn highlighted(&self) -> Option<T> {
        let key = self.0.highlighted.get_untracked()?;
        untrack(|| keyed(&self.0.options.value(), key)).map(|option| option.value().clone())
    }
}

use std::cell::RefCell;
use std::rc::Rc;

use accesskit::{Node, Role, Toggled};

use crate as unstyled;
use crate::ButtonHandle;
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Direction;
use beui_view::types::{CursorIcon, Key, KeyPress};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChoiceKind {
    Tabs,
    Radio,
}

pub struct ChoiceOptionHandle<T> {
    pub value: T,
    pub label: Prop<String>,
    pub selected: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

/// One option of a `Choice` or `Select`: the `value` it stands for, which the
/// control's binding holds when it is chosen, and the label it shows.
pub struct ChoiceOption<T> {
    key: u64,
    value: T,
    label: Memo<String>,
    posted: Memo<String>,
    test_id: Option<Prop<String>>,
}

impl<T> TakesTestId for ChoiceOption<T> {
    type Named = Self;

    fn take_test_id(mut self, test_id: Prop<String>) -> Self {
        self.test_id = Some(test_id);
        self
    }
}

impl<T> ChoiceOption<T> {
    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn label(&self) -> Memo<String> {
        self.label
    }

    /// What a `Form` posts for the option: its `posted`, or its label without one, as
    /// `<option>`'s `value` does.
    pub fn posted(&self) -> Memo<String> {
        self.posted
    }

    /// The id `@test_id` gave the option, which its control binds on the row it builds.
    pub fn test_id(&self) -> Option<Prop<String>> {
        self.test_id.clone()
    }
}

beui_view::value_child_type!(<T> ChoiceOption<T>);

#[component]
pub fn ChoiceOption<T>(
    value: T,
    label: Prop<String>,
    #[prop(default = String::new())] posted: Prop<String>,
) -> ChoiceOption<T>
where
    T: 'static,
{
    let label = create_memo(move || label.get());
    ChoiceOption {
        key: next_key(),
        value,
        label,
        posted: create_memo(move || {
            Some(posted.get())
                .filter(|posted| !posted.is_empty())
                .unwrap_or_else(|| label.get())
        }),
        test_id: None,
    }
}

thread_local! {
    static NEXT_KEY: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn next_key() -> u64 {
    NEXT_KEY.with(|next| {
        let key = next.get();
        next.set(key + 1);
        key
    })
}

/// Names one option for as long as it lives, wherever it moves among its siblings, so
/// rows, focus and highlights follow it rather than a position.
pub(crate) fn option_key<T>(option: &ChoiceOption<T>) -> u64 {
    option.key
}

/// The option `key` names among `options`, read tracked.
pub(crate) fn keyed<T: 'static>(
    options: &Run<ChoiceOption<T>>,
    key: u64,
) -> Option<std::rc::Rc<ChoiceOption<T>>> {
    options.items().into_iter().find(|option| option.key == key)
}

/// Where the option holding `value` sits among `options`, read tracked.
pub(crate) fn position<T: PartialEq + 'static>(
    options: &Run<ChoiceOption<T>>,
    value: &T,
) -> Option<usize> {
    options
        .items()
        .iter()
        .position(|option| option.value == *value)
}

pub(crate) fn value_at<T: Clone + 'static>(
    options: &Run<ChoiceOption<T>>,
    index: usize,
) -> Option<T> {
    options
        .get_untracked()
        .get(index)
        .map(|option| option.value.clone())
}

pub struct ChoiceItem<T> {
    pub value: T,
    pub button: NodeId,
}

struct State<T: 'static> {
    options: Stored<Run<ChoiceOption<T>>>,
    focus: Signal<Option<u64>>,
    selected: Binding<Option<T>>,
    shown: Memo<Option<usize>>,
    kind: ChoiceKind,
    direction: Direction,
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for State<T> {}

/// Tabs or a radio group, by `kind`.
///
/// One Tab stop, at the selected option or the first. Moving selects. Radio
/// arrows go either way and wrap. Tabs wrap along their `direction` and ignore
/// the other arrows. Home/End select the first and last option.
///
/// `selected` holds the chosen option's value. A value no option holds shows
/// nothing selected, so a list that changes between a frame and a click never
/// selects whatever moved into a position.
///
/// Given a `handle`, it tells it the options' values and the node each option's `item`
/// built (`ForEachRef`), as a `ForEach` does for its rows.
#[component]
pub fn Choice<T>(
    #[prop(default = ComponentRef::new())] api: ComponentRef<ChoiceApi<T>>,
    options: Children<ChoiceOption<T>>,
    #[prop(default = None)] selected: Binding<Option<T>>,
    kind: ChoiceKind,
    direction: Option<Direction>,
    #[prop(default = false)] wrap: bool,
    #[prop(default = false)] shrink: Prop<bool>,
    #[prop(default = CursorIcon::PointingHand)] cursor: CursorIcon,
    spacing: f32,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = None)] item: Option<RenderFn<ChoiceItem<T>, NodeId>>,
    #[prop(default = None)] handle: Option<ForEachRef<T, NodeId>>,
    #[prop(children)] option: Option<RenderFn<ChoiceOptionHandle<T>, NodeId>>,
) -> NodeId
where
    T: Clone + PartialEq + 'static,
{
    let option = option.expect("choice requires an `option` builder");
    let options = options.into_run();
    let direction = direction.unwrap_or(match kind {
        ChoiceKind::Tabs => Direction::Horizontal,
        ChoiceKind::Radio => Direction::Vertical,
    });
    let among = options.clone();
    let shown = create_memo(move || {
        selected.with(|selected| selected.as_ref().and_then(|value| position(&among, value)))
    });
    let selection = create_selector(move || shown.get());
    let tab_stop_owner = create_selector(move || shown.get().unwrap_or(0));
    let focus = create_signal(None);
    let focused = create_selector(move || focus.get());

    component_accessibility(crate::labelled_node(
        match kind {
            ChoiceKind::Tabs => Role::TabList,
            ChoiceKind::Radio => Role::RadioGroup,
        },
        None,
        label,
    ));

    let state = State {
        options: store(options.clone()),
        focus,
        selected,
        shown,
        kind,
        direction,
    };
    api.fill(ChoiceApi(state));
    // Each option's value, place and the node its `item` built, for `handle`.
    let built_rows = Rc::new(RefCell::new(Vec::<(T, ReadSignal<usize>, NodeId)>::new()));
    let rows_changed = create_trigger();
    if let Some(fill) = handle.map(|handle| handle.claim()) {
        let built_rows = Rc::clone(&built_rows);
        let among = options.clone();
        create_effect(move || {
            rows_changed.track();
            fill.keys(
                among
                    .items()
                    .iter()
                    .map(|option| option.value.clone())
                    .collect(),
            );
            let mut rows: Vec<ShownRow<T, NodeId>> = built_rows
                .borrow()
                .iter()
                .map(|(value, index, node)| ShownRow {
                    key: value.clone(),
                    index: *index,
                    nodes: vec![*node],
                })
                .collect();
            rows.sort_by_key(|row| row.index.get_untracked());
            fill.rows(rows);
        });
    }
    let buttons = options.build_keyed(
        move |built: Rc<ChoiceOption<T>>, index: ReadSignal<usize>| {
            let label = built.label;
            let value = built.value.clone();
            let is_selected = create_memo(move || selection.is_selected(&Some(index.get())));
            let id = option_key(&built);
            let tab_stop = create_memo(move || tab_stop_owner.is_selected(&index.get()));
            let focus_here = focused.memo(Some(id));
            let button = view! {
                <unstyled::Button
                    cursor
                    tab_stop={tab_stop}
                    focused={focus_here}
                    accessibility={move || {
                        let mut node = Node::new(match kind {
                            ChoiceKind::Tabs => Role::Tab,
                            ChoiceKind::Radio => Role::RadioButton,
                        });
                        if label.is_alive() {
                            node.set_label(label.get());
                        }
                        if kind == ChoiceKind::Radio {
                            node.set_toggled(Toggled::from(is_selected.get()));
                        } else {
                            node.set_selected(is_selected.get());
                        }
                        node
                    }}
                    on_focus_change={move |has_focus: bool| track_focus(state, id, has_focus)}
                    on_click={move || select(state, index.get_untracked())}
                    on_key={move |press: KeyPress| key(state, index.get_untracked(), press)}
                >|button: ButtonHandle| {
                    option.call(ChoiceOptionHandle {
                        value: value.clone(),
                        label: label.into_prop(),
                        selected: is_selected,
                        hovered: button.hovered,
                        active: button.active,
                        focused: button.focused,
                    })
                }</unstyled::Button>
            };
            if let Some(test_id) = built.test_id() {
                bind_test_id(button, test_id);
            }
            let node = match item {
                Some(item) => item.call(ChoiceItem {
                    value: built.value.clone(),
                    button,
                }),
                None => button,
            };
            if handle.is_some() {
                built_rows
                    .borrow_mut()
                    .push((built.value.clone(), index, node));
                rows_changed.notify();
                let built_rows = Rc::clone(&built_rows);
                on_cleanup(move || {
                    built_rows.borrow_mut().retain(|(.., shown)| *shown != node);
                    rows_changed.notify();
                });
            }
            view! {
                <ListItem
                    shrink={move || match shrink.get() {
                        true => 1.0,
                        false => 0.0,
                    }}
                >
                    {node}
                </ListItem>
            }
        },
    );

    view! {
        <List direction wrap spacing children={buttons} />
    }
}

fn id_at<T: 'static>(state: State<T>, index: usize) -> Option<u64> {
    state
        .options
        .value()
        .get_untracked()
        .get(index)
        .map(|option| option_key(option))
}

fn track_focus<T>(state: State<T>, id: u64, has_focus: bool) {
    if has_focus {
        state.focus.set(Some(id));
    } else if state.focus.get_untracked() == Some(id) {
        state.focus.set(None);
    }
}

fn option_count<T>(state: State<T>) -> usize {
    state.options.value().get_untracked().len()
}

fn select<T: Clone + PartialEq + 'static>(state: State<T>, index: usize) {
    let Some(value) = value_at(&state.options.value(), index) else {
        return;
    };
    if state.shown.get_untracked() == Some(index) {
        return;
    }
    let focused = state.focus.get_untracked().is_some();
    state.selected.set(Some(value));
    if focused {
        state.focus.set(id_at(state, index));
    }
}

fn key<T: Clone + PartialEq + 'static>(state: State<T>, index: usize, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let count = option_count(state);
    let wraps = match state.kind {
        ChoiceKind::Tabs => state.direction == Direction::Horizontal,
        ChoiceKind::Radio => true,
    };
    let vertical_tabs = state.kind == ChoiceKind::Tabs && state.direction == Direction::Vertical;
    let next = match press.key {
        Key::ArrowLeft if wraps => (index + count - 1) % count,
        Key::ArrowRight if wraps => (index + 1) % count,
        Key::ArrowUp if state.kind == ChoiceKind::Radio || vertical_tabs => {
            (index + count - 1) % count
        }
        Key::ArrowDown if state.kind == ChoiceKind::Radio || vertical_tabs => (index + 1) % count,
        Key::Home => 0,
        Key::End => count - 1,
        _ => return false,
    };
    if press.pressed {
        state.focus.set(id_at(state, next));
        select(state, next);
    }
    true
}

pub struct ChoiceApi<T: 'static>(State<T>);

impl<T> Clone for ChoiceApi<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ChoiceApi<T> {}

impl<T: Clone + 'static> ChoiceApi<T> {
    /// The selected option's value, or `None` while no option holds the bound value.
    pub fn selected(&self) -> Option<T> {
        let state = self.0;
        let index = state.shown.get_untracked()?;
        value_at(&state.options.value(), index)
    }
}

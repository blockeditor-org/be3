use std::cell::RefCell;
use std::rc::Rc;

use accesskit::{Node, Role, Toggled};

use crate as unstyled;
use crate::ButtonHandle;
use crate::typeahead::Typeahead;
use beui_core::base::Direction;
use beui_core::input::{Key, KeyPress};
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChoiceKind {
    Tabs,
    Radio,
    Listbox,
}

pub struct ChoiceOptionHandle<T> {
    pub value: T,
    pub label: Prop<String>,
    pub selected: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub struct ChoiceOption<T> {
    value: T,
    label: Memo<String>,
    test_id: Option<Prop<String>>,
}

impl<T> ChoiceOption<T> {
    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn label(&self) -> Memo<String> {
        self.label
    }

    pub fn test_id(&self) -> Option<Prop<String>> {
        self.test_id.clone()
    }
}

impl<T> TakesTestId for ChoiceOption<T> {
    fn take_test_id(mut self, test_id: Prop<String>) -> Self {
        self.test_id = Some(test_id);
        self
    }
}

beui_view::value_child_type!(<T> ChoiceOption<T>);

#[component]
pub fn ChoiceOption<T>(value: T, label: Prop<String>) -> ChoiceOption<T>
where
    T: 'static,
{
    ChoiceOption {
        value,
        label: create_memo(move || label.get()),
        test_id: None,
    }
}

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

struct State<T: 'static> {
    options: Stored<Run<ChoiceOption<T>>>,
    focus: Signal<Option<usize>>,
    selected: Binding<Option<T>>,
    shown: Memo<Option<usize>>,
    kind: ChoiceKind,
    direction: Direction,
    typeahead: Stored<RefCell<Typeahead>>,
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for State<T> {}

#[component]
pub fn Choice<T>(
    #[prop(default = ComponentRef::new())] api: ComponentRef<ChoiceApi<T>>,
    options: Children<ChoiceOption<T>>,
    #[prop(default = None)] selected: Binding<Option<T>>,
    kind: ChoiceKind,
    direction: Option<Direction>,
    #[prop(default = false)] wrap: bool,
    spacing: f32,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(children)] option: Option<RenderFn<ChoiceOptionHandle<T>>>,
) -> NodeId
where
    T: Clone + PartialEq + 'static,
{
    let option = option.expect("choice requires an `option` builder");
    let options = options.into_run();
    let direction = direction.unwrap_or(match kind {
        ChoiceKind::Tabs => Direction::Horizontal,
        ChoiceKind::Radio | ChoiceKind::Listbox => Direction::Vertical,
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
            ChoiceKind::Listbox => Role::ListBox,
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
        typeahead: store(RefCell::default()),
    };
    api.fill(ChoiceApi(state));
    let built_options: Stored<RefCell<Vec<Rc<ChoiceOption<T>>>>> = store(RefCell::default());
    let buttons = options.build_each(
        built_options,
        |_, option| option,
        move |index| {
            let (label, value, test_id) = {
                let built = built_options.value();
                let built = built.borrow();
                let option = &built[index];
                (option.label, option.value.clone(), option.test_id.clone())
            };
            let is_selected = selection.memo(Some(index));
            let accessibility = create_memo(move || {
                let mut node = Node::new(match kind {
                    ChoiceKind::Tabs => Role::Tab,
                    ChoiceKind::Radio => Role::RadioButton,
                    ChoiceKind::Listbox => Role::ListBoxOption,
                });
                node.set_label(label.get());
                if kind == ChoiceKind::Radio {
                    node.set_toggled(Toggled::from(is_selected.get()));
                } else {
                    node.set_selected(is_selected.get());
                }
                node
            });
            let built = view! {
                <unstyled::Button
                    tab_stop={tab_stop_owner.memo(index)}
                    focused={focused.memo(Some(index))}
                    accessibility
                    on_focus_change={move |has_focus: bool| track_focus(state, index, has_focus)}
                    on_click={move || select(state, index)}
                    on_key={move |press: KeyPress| key(state, index, press)}
                    on_text={move |typed: String| {
                        if state.kind == ChoiceKind::Listbox {
                            typeahead(state, index, &typed);
                        }
                    }}
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
            if let Some(test_id) = test_id {
                bind_test_id(built, test_id);
            }
            built
        },
    );

    if kind == ChoiceKind::Listbox {
        create_effect(move || {
            if state.focus.get().is_none() {
                state.typeahead.value().borrow_mut().clear();
            }
        });
    }

    view! {
        <List direction wrap spacing children={buttons} />
    }
}

fn track_focus<T>(state: State<T>, index: usize, has_focus: bool) {
    if has_focus {
        state.focus.set(Some(index));
    } else if state.focus.get_untracked() == Some(index) {
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
        state.focus.set(Some(index));
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
        ChoiceKind::Listbox => false,
    };
    let vertical_tabs = state.kind == ChoiceKind::Tabs && state.direction == Direction::Vertical;
    let next = match press.key {
        Key::ArrowLeft if wraps => (index + count - 1) % count,
        Key::ArrowRight if wraps => (index + 1) % count,
        Key::ArrowUp if state.kind == ChoiceKind::Radio || vertical_tabs => {
            (index + count - 1) % count
        }
        Key::ArrowDown if state.kind == ChoiceKind::Radio || vertical_tabs => (index + 1) % count,
        Key::ArrowUp if state.kind == ChoiceKind::Listbox => index.saturating_sub(1),
        Key::ArrowDown if state.kind == ChoiceKind::Listbox => (index + 1).min(count - 1),
        Key::Home => 0,
        Key::End => count - 1,
        _ => return false,
    };
    if press.pressed {
        state.focus.set(Some(next));
        select(state, next);
    }
    true
}

fn typeahead<T: Clone + PartialEq + 'static>(state: State<T>, index: usize, text: &str) {
    let labels: Vec<String> = state
        .options
        .value()
        .get_untracked()
        .iter()
        .map(|option| option.label.get_untracked())
        .collect();
    let matched =
        state
            .typeahead
            .value()
            .borrow_mut()
            .matched(text, index, labels.len(), |option| labels[option].clone());
    if let Some(next) = matched {
        state.focus.set(Some(next));
        select(state, next);
    }
}

pub struct ChoiceApi<T: 'static>(State<T>);

impl<T> Clone for ChoiceApi<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ChoiceApi<T> {}

impl<T: Clone + 'static> ChoiceApi<T> {
    pub fn selected(&self) -> Option<T> {
        let state = self.0;
        let index = state.shown.get_untracked()?;
        value_at(&state.options.value(), index)
    }
}

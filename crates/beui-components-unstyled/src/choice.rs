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

pub struct ChoiceOptionHandle {
    pub index: ReadSignal<usize>,
    pub label: Prop<String>,
    pub selected: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub struct ChoiceOption {
    label: Memo<String>,
}

impl ChoiceOption {
    pub fn label(&self) -> Memo<String> {
        self.label
    }
}

beui_view::value_child_type!(ChoiceOption);

#[component]
pub fn ChoiceOption(label: Prop<String>) -> ChoiceOption {
    ChoiceOption {
        label: create_memo(move || label.get()),
    }
}

pub struct ChoiceItem {
    pub index: ReadSignal<usize>,
    pub button: NodeId,
}

#[derive(Clone, Copy)]
struct State {
    options: Stored<Run<ChoiceOption>>,
    focus: Signal<Option<usize>>,
    selected: Binding<Option<usize>>,
    kind: ChoiceKind,
    direction: Direction,
    typeahead: Stored<RefCell<Typeahead>>,
}

#[component]
pub fn Choice(
    #[prop(default = ComponentRef::new())] api: ComponentRef<ChoiceApi>,
    options: Children<ChoiceOption>,
    #[prop(default = None)] selected: Binding<Option<usize>>,
    kind: ChoiceKind,
    direction: Option<Direction>,
    #[prop(default = false)] wrap: bool,
    spacing: f32,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = None)] item: Option<RenderFn<ChoiceItem>>,
    #[prop(children)] option: Option<RenderFn<ChoiceOptionHandle>>,
) -> NodeId {
    let option = option.expect("choice requires an `option` builder");
    let options = options.into_run();
    let direction = direction.unwrap_or(match kind {
        ChoiceKind::Tabs => Direction::Horizontal,
        ChoiceKind::Radio | ChoiceKind::Listbox => Direction::Vertical,
    });
    let selection = create_selector(move || selected.get());
    let counted = options.clone();
    let tab_stop_owner = create_selector(move || {
        selected
            .get()
            .filter(|index| *index < counted.len())
            .unwrap_or(0)
    });
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
        kind,
        direction,
        typeahead: store(RefCell::default()),
    };
    api.fill(ChoiceApi(state));
    let buttons = options.build_keyed(move |built: Rc<ChoiceOption>, index: ReadSignal<usize>| {
        let label = built.label;
        let is_selected = create_memo(move || selection.is_selected(&Some(index.get())));
        let accessibility = create_memo(move || {
            let mut node = Node::new(match kind {
                ChoiceKind::Tabs => Role::Tab,
                ChoiceKind::Radio => Role::RadioButton,
                ChoiceKind::Listbox => Role::ListBoxOption,
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
        });
        let id = option_id(&built);
        let tab_stop = create_memo(move || tab_stop_owner.is_selected(&index.get()));
        let focus_here = focused.memo(Some(id));
        let button = view! {
            <unstyled::Button
                tab_stop={tab_stop}
                focused={focus_here}
                accessibility
                on_focus_change={move |has_focus: bool| track_focus(state, id, has_focus)}
                on_click={move || select(state, Some(index.get_untracked()))}
                on_key={move |press: KeyPress| key(state, index.get_untracked(), press)}
                on_text={move |typed: String| {
                    if state.kind == ChoiceKind::Listbox {
                        typeahead(state, index.get_untracked(), &typed);
                    }
                }}
            >|button: ButtonHandle| {
                option.call(ChoiceOptionHandle {
                    index,
                    label: label.into_prop(),
                    selected: is_selected,
                    hovered: button.hovered,
                    active: button.active,
                    focused: button.focused,
                })
            }</unstyled::Button>
        };
        match item {
            Some(item) => item.call(ChoiceItem { index, button }),
            None => button,
        }
    });

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

fn option_id(option: &Rc<ChoiceOption>) -> usize {
    Rc::as_ptr(option) as usize
}

fn id_at(state: State, index: usize) -> Option<usize> {
    state
        .options
        .value()
        .get_untracked()
        .get(index)
        .map(option_id)
}

fn track_focus(state: State, id: usize, has_focus: bool) {
    if has_focus {
        state.focus.set(Some(id));
    } else if state.focus.get_untracked() == Some(id) {
        state.focus.set(None);
    }
}

fn option_count(state: State) -> usize {
    state.options.value().get_untracked().len()
}

fn select(state: State, selected: Option<usize>) {
    if selected.is_some_and(|index| index >= option_count(state))
        || state.selected.get_untracked() == selected
    {
        return;
    }
    let focused = state.focus.get_untracked().is_some();
    state.selected.set(selected);
    if focused && selected.unwrap_or(0) < option_count(state) {
        state.focus.set(id_at(state, selected.unwrap_or(0)));
    }
}

fn key(state: State, index: usize, press: KeyPress) -> bool {
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
        state.focus.set(id_at(state, next));
        select(state, Some(next));
    }
    true
}

fn typeahead(state: State, index: usize, text: &str) {
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
        state.focus.set(id_at(state, next));
        select(state, Some(next));
    }
}

#[derive(Clone, Copy)]
pub struct ChoiceApi(State);

impl ChoiceApi {
    pub fn selected(&self) -> Option<usize> {
        let state = self.0;
        state
            .selected
            .get_untracked()
            .filter(|index| *index < option_count(state))
    }
}

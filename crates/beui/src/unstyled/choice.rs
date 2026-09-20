use std::cell::RefCell;
use std::rc::Rc;

use accesskit::{Node, Role, Toggled};

use crate::base::Direction;
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use crate::reactive::{
    Callback, ChildScope, ChildValue, Children, IntoProp, List, Memo, Prop, ReadSignal, RenderFn,
    Run, Scope, WriteSignal, clone, component_accessibility, create_effect, create_memo,
    create_selector, create_signal, set_component_state,
};
use crate::unstyled;
use crate::unstyled::ButtonHandle;
use crate::unstyled::typeahead::Typeahead;
use beui_macros::{component, view};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChoiceKind {
    Tabs,
    Radio,
    Listbox,
}

pub struct ChoiceOptionHandle {
    pub index: usize,
    pub label: Prop<String>,
    pub selected: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub struct ChoiceOption {
    label: Memo<String>,
    scope: ChildScope,
}

impl ChildValue for ChoiceOption {
    fn anchor(&self) -> Option<NodeId> {
        None
    }

    fn adopt_scope(&mut self, scope: Scope) {
        self.scope.adopt(scope);
    }
}

impl ChoiceOption {
    pub fn label(&self) -> Memo<String> {
        self.label.clone()
    }
}

crate::value_child_type!(ChoiceOption);

#[component]
pub fn ChoiceOption(label: Prop<String>) -> ChoiceOption {
    ChoiceOption {
        label: create_memo(move || label.get()),
        scope: ChildScope::default(),
    }
}

struct State {
    options: Run<ChoiceOption>,
    focus: ReadSignal<Option<usize>>,
    set_focus: WriteSignal<Option<usize>>,
    selected: ReadSignal<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
    kind: ChoiceKind,
    typeahead: RefCell<Typeahead>,
    on_change: Callback<Option<usize>>,
}

type Handle = Rc<State>;

#[component]
pub fn Choice(
    options: Children<ChoiceOption>,
    selected: Prop<Option<usize>>,
    kind: ChoiceKind,
    on_change: Callback<Option<usize>>,
    #[prop(children)] option: Option<RenderFn<ChoiceOptionHandle>>,
) -> NodeId {
    let selected_prop = selected;
    let option = option.expect("choice requires an `option` builder");
    let options = options.into_run();
    let (selected, set_selected) = create_signal(None);
    let selection = create_selector(clone!(selected -> move || selected.get()));
    let tab_stop_owner = create_selector(clone!(selected -> move || selected.get().unwrap_or(0)));
    let (focus, set_focus) = create_signal(None);
    let focused = create_selector(clone!(focus -> move || focus.get()));

    component_accessibility(Node::new(match kind {
        ChoiceKind::Tabs => Role::TabList,
        ChoiceKind::Radio => Role::RadioGroup,
        ChoiceKind::Listbox => Role::ListBox,
    }));

    let state: Handle = Rc::new(State {
        options: options.clone(),
        focus: focus.clone(),
        set_focus,
        selected: selected.clone(),
        set_selected,
        kind,
        typeahead: RefCell::default(),
        on_change,
    });
    set_component_state(state.clone());
    let buttons_state = state.clone();
    let buttons = options.build(move |built: Vec<Rc<ChoiceOption>>| {
        let state = buttons_state.clone();
        built
            .into_iter()
            .enumerate()
            .map(|(index, built)| {
                let option = option.clone();
                let label = built.label.clone();
                let accessibility_label = label.clone();
                let is_selected = selection.memo(Some(index));
                let selected_accessibility = is_selected.clone();
                let accessibility = create_memo(move || {
                    let mut node = Node::new(match kind {
                        ChoiceKind::Tabs => Role::Tab,
                        ChoiceKind::Radio => Role::RadioButton,
                        ChoiceKind::Listbox => Role::ListBoxOption,
                    });
                    node.set_label(accessibility_label.get());
                    if kind == ChoiceKind::Radio {
                        node.set_toggled(Toggled::from(selected_accessibility.get()));
                    } else {
                        node.set_selected(selected_accessibility.get());
                    }
                    node
                });
                let (blur, click, key_press, text) =
                    (state.clone(), state.clone(), state.clone(), state.clone());
                view! {
                    <unstyled::Button
                        tab_stop={tab_stop_owner.memo(index)}
                        focused={focused.memo(Some(index))}
                        accessibility
                        on_focus_change={move |has_focus: bool| track_focus(&blur, index, has_focus)}
                        content={move |button: ButtonHandle| {
                            option.call(ChoiceOptionHandle {
                                index,
                                label: label.into_prop(),
                                selected: is_selected,
                                hovered: button.hovered,
                                active: button.active,
                                focused: button.focused,
                            })
                        }}
                        on_click={move || select(&click, Some(index))}
                        on_key={move |press: KeyPress| key(&key_press, index, press)}
                        on_text={move |typed: String| {
                            if text.kind == ChoiceKind::Listbox {
                                typeahead(&text, index, &typed);
                            }
                        }}
                    />
                }
            })
            .collect()
    });

    if kind == ChoiceKind::Listbox {
        let state = state.clone();
        create_effect(move || {
            if state.focus.get().is_none() {
                state.typeahead.borrow_mut().clear();
            }
        });
    }

    create_effect(clone!(state -> move || {
        state.options.len();
        sync_selected(&state, selected_prop.get());
    }));

    let direction = if kind == ChoiceKind::Tabs {
        Direction::Horizontal
    } else {
        Direction::Vertical
    };
    view! {
        <List direction spacing=6.0 children={buttons} />
    }
}

pub fn choice_selected(document: &Document, choice: NodeId) -> Option<usize> {
    document.component_state::<Handle>(choice).selected.get()
}

fn track_focus(state: &State, index: usize, has_focus: bool) {
    if has_focus {
        state.set_focus.set(Some(index));
    } else if state.focus.get_untracked() == Some(index) {
        state.set_focus.set(None);
    }
}

fn sync_selected(state: &State, selected: Option<usize>) {
    if selected.is_some_and(|index| index >= option_count(state)) {
        return;
    }
    state.set_selected.set(selected);
}

fn option_count(state: &State) -> usize {
    state.options.peek().len()
}

fn select(state: &State, selected: Option<usize>) {
    if selected.is_some_and(|index| index >= option_count(state))
        || state.selected.get_untracked() == selected
    {
        return;
    }
    let focused = state.focus.get_untracked().is_some();
    sync_selected(state, selected);
    if focused && selected.unwrap_or(0) < option_count(state) {
        state.set_focus.set(Some(selected.unwrap_or(0)));
    }
    state.on_change.call(selected);
}

fn key(state: &State, index: usize, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let count = option_count(state);
    let next = match press.key {
        Key::ArrowLeft if state.kind != ChoiceKind::Listbox => (index + count - 1) % count,
        Key::ArrowRight if state.kind != ChoiceKind::Listbox => (index + 1) % count,
        Key::ArrowUp if state.kind == ChoiceKind::Radio => (index + count - 1) % count,
        Key::ArrowDown if state.kind == ChoiceKind::Radio => (index + 1) % count,
        Key::ArrowUp if state.kind == ChoiceKind::Listbox => index.saturating_sub(1),
        Key::ArrowDown if state.kind == ChoiceKind::Listbox => (index + 1).min(count - 1),
        Key::Home => 0,
        Key::End => count - 1,
        _ => return false,
    };
    if press.pressed {
        state.set_focus.set(Some(next));
        select(state, Some(next));
    }
    true
}

fn typeahead(state: &State, index: usize, text: &str) {
    let labels: Vec<String> = state
        .options
        .peek()
        .iter()
        .map(|option| option.label.get_untracked())
        .collect();
    let matched = state
        .typeahead
        .borrow_mut()
        .matched(text, index, labels.len(), |option| labels[option].clone());
    if let Some(next) = matched {
        state.set_focus.set(Some(next));
        select(state, Some(next));
    }
}

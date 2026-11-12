use std::rc::Rc;

use accesskit::{Node, Role, Toggled};

use crate as unstyled;
use crate::ButtonHandle;
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Direction;
use beui_view::types::{Key, KeyPress};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChoiceKind {
    Tabs,
    Radio,
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
    value: Memo<String>,
}

impl ChoiceOption {
    pub fn label(&self) -> Memo<String> {
        self.label
    }

    /// What a `Form` posts for the option: its `value`, or its label without one, as
    /// `<option>` does.
    pub fn value(&self) -> Memo<String> {
        self.value
    }
}

beui_view::value_child_type!(ChoiceOption);

#[component]
pub fn ChoiceOption(
    label: Prop<String>,
    #[prop(default = String::new())] value: Prop<String>,
) -> ChoiceOption {
    let label = create_memo(move || label.get());
    ChoiceOption {
        label,
        value: create_memo(move || {
            Some(value.get())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| label.get())
        }),
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
}

/// Tabs or a radio group, by `kind`.
///
/// One Tab stop, at the selected option or the first. Moving selects. Radio
/// arrows go either way and wrap. Tabs wrap along their `direction` and ignore
/// the other arrows. Home/End select the first and last option.
#[component]
pub fn Choice(
    #[prop(default = ComponentRef::new())] api: ComponentRef<ChoiceApi>,
    options: Children<ChoiceOption>,
    #[prop(default = None)] selected: Binding<Option<usize>>,
    kind: ChoiceKind,
    direction: Option<Direction>,
    #[prop(default = false)] wrap: bool,
    #[prop(default = false)] shrink: Prop<bool>,
    spacing: f32,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = None)] item: Option<RenderFn<ChoiceItem>>,
    #[prop(children)] option: Option<RenderFn<ChoiceOptionHandle>>,
) -> NodeId {
    let option = option.expect("choice requires an `option` builder");
    let options = options.into_run();
    let direction = direction.unwrap_or(match kind {
        ChoiceKind::Tabs => Direction::Horizontal,
        ChoiceKind::Radio => Direction::Vertical,
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
    };
    api.fill(ChoiceApi(state));
    let option_shrink = create_memo(move || match shrink.get() {
        true => 1.0,
        false => 0.0,
    });
    let buttons = options.build_keyed(move |built: Rc<ChoiceOption>, index: ReadSignal<usize>| {
        let label = built.label;
        let is_selected = create_memo(move || selection.is_selected(&Some(index.get())));
        let accessibility = create_memo(move || {
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
        let node = match item {
            Some(item) => item.call(ChoiceItem { index, button }),
            None => button,
        };
        view! {
            <ListItem shrink=option_shrink>{node}</ListItem>
        }
    });

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
        select(state, Some(next));
    }
    true
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

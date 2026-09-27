use accesskit::{HasPopup, Node, Role};
use beui_macros::{component, view};

use crate::base::overlay::{Overlay, Placement};
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    Callback, Child, List, Memo, NodeRef, Prop, ReadSignal, Render, Show, clone,
    component_accessibility, create_effect, create_memo, create_signal, set_component_state,
};
use crate::unstyled;
use crate::unstyled::ButtonHandle;

pub struct PopoverTriggerHandle {
    pub open: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
}

#[derive(Clone)]
pub struct PopoverHandle {
    pub open: ReadSignal<bool>,
    pub close: Callback<()>,
}

#[derive(Clone)]
struct State {
    open: ReadSignal<bool>,
    trigger: NodeRef,
}

#[component]
pub fn Popover(
    trigger: Render<PopoverTriggerHandle>,
    #[prop(children)] content: Render<PopoverHandle>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = String::new())] label: Prop<String>,
    accessibility: Option<Prop<Node>>,
    on_open_change: Callback<bool>,
) -> NodeId {
    let (open, set_open) = create_signal(false);
    let (refocus, set_refocus) = create_signal(false);
    let disabled = create_memo(move || disabled.get());
    let label = create_memo(move || label.get());
    let trigger_ref = NodeRef::new();
    set_component_state(State {
        open: open.clone(),
        trigger: trigger_ref.clone(),
    });

    create_effect(clone!(open -> move || on_open_change.call(open.get())));
    create_effect(clone!(disabled set_open -> move || {
        if disabled.get() {
            set_open.set(false);
        }
    }));

    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::Button)));
    let trigger_accessibility = create_memo(clone!(open label -> move || {
        let mut node = accessibility.get();
        if node.label().is_none() && !label.get().is_empty() {
            node.set_label(label.get());
        }
        node.set_has_popup(HasPopup::Dialog);
        node.set_expanded(open.get());
        node
    }));

    let (built, set_built) = create_signal(false);
    create_effect(clone!(open -> move || {
        if open.get() {
            set_built.set(true);
        }
    }));

    let close = Callback::new(clone!(set_open -> move |()| set_open.set(false)));
    let handle = PopoverHandle {
        open: open.clone(),
        close,
    };
    let trigger_disabled = disabled.clone();
    let trigger_open = open.clone();
    let toggle = clone!(set_open disabled -> move || {
        if !disabled.get_untracked() {
            set_open.update(|open| *open = !*open);
        }
    });
    view! {
        <List spacing=0.0>
            <unstyled::Button
                @node_ref=&trigger_ref
                disabled={disabled}
                focused={refocus}
                accessibility={trigger_accessibility}
                on_focus_change={clone!(set_refocus -> move |focused: bool| {
                    if !focused {
                        set_refocus.set(false);
                    }
                })}
                on_click={toggle}
                content={move |button: ButtonHandle| {
                    trigger.call(PopoverTriggerHandle {
                        open: trigger_open,
                        hovered: button.hovered,
                        active: button.active,
                        focused: button.focused,
                        disabled: trigger_disabled,
                    })
                }}
            />
            <Overlay
                anchor=&trigger_ref
                placement=Placement::BelowStart
                open={open.clone()}
                on_dismiss={clone!(open set_open -> move || {
                    let was_open = open.get_untracked();
                    set_open.set(false);
                    if was_open {
                        set_refocus.set(true);
                    }
                })}
            >
                <PopoverSurface label>
                    <List spacing=0.0>
                        <Show condition={built}>{move || content.call(handle)}</Show>
                    </List>
                </PopoverSurface>
            </Overlay>
        </List>
    }
}

#[component]
fn PopoverSurface(label: Memo<String>, children: Child) -> NodeId {
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Dialog);
        let label = label.get();
        if !label.is_empty() {
            node.set_label(label);
        }
        node
    }));
    view! {
        <List spacing=0.0>{children}</List>
    }
}

pub fn popover_open(document: &Document, popover: NodeId) -> bool {
    document
        .component_state::<State>(popover)
        .open
        .get_untracked()
}

pub fn popover_trigger(document: &Document, popover: NodeId) -> NodeId {
    document.component_state::<State>(popover).trigger.get()
}

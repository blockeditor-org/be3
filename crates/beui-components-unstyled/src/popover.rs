use accesskit::{HasPopup, Node, Role};
use beui_macros::{component, view};

use crate as unstyled;
use crate::ButtonHandle;
use beui_core::base::overlay::Placement;
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Callback, Child, List, Memo, NodeRef, Prop, ReadSignal, Render, ShowKeepAlive, clone,
    component_accessibility, create_effect, create_memo, create_signal, set_component_state,
};

pub struct PopoverTriggerHandle {
    pub open: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PopoverPlacement {
    #[default]
    Below,
    Over(u16),
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
    #[prop(default = false)] open: Prop<bool>,
    anchor: Option<NodeRef>,
    #[prop(default = PopoverPlacement::Below)] placement: PopoverPlacement,
    #[prop(default = true)] refocus_trigger: Prop<bool>,
    accessibility: Option<Prop<Node>>,
    on_open_change: Callback<bool>,
) -> NodeId {
    let requested = open;
    let (open, set_open) = create_signal(false);
    create_effect(clone!(set_open -> move || {
        if requested.get() {
            set_open.set(true);
        }
    }));
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

    let refocus_trigger = create_memo(move || refocus_trigger.get());
    let close = Callback::new(
        clone!(open set_open set_refocus refocus_trigger -> move |()| {
            if open.get_untracked() {
                if refocus_trigger.get_untracked() {
                    set_refocus.set(true);
                }
                set_open.set(false);
            }
        }),
    );
    let anchor = anchor.unwrap_or_else(|| trigger_ref.clone());
    let placement = match placement {
        PopoverPlacement::Below => Placement::BelowStart,
        PopoverPlacement::Over(inset) => Placement::Over(inset),
    };
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
                anchor=&anchor
                placement
                light=true
                open={open.clone()}
                on_dismiss={clone!(open set_open -> move || {
                    let was_open = open.get_untracked();
                    set_open.set(false);
                    if was_open && refocus_trigger.get_untracked() {
                        set_refocus.set(true);
                    }
                })}
            >
                <PopoverSurface label>
                    <List spacing=0.0>
                        <ShowKeepAlive condition={built}>
                            {move || content.call(handle)}
                        </ShowKeepAlive>
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

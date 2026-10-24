use beui_core::icons::Glyph;
use accesskit::{HasPopup, Node, Role};
use beui_core::layer::{Dismiss, LayerPolicy};
use beui_macros::{component, view};

use crate as unstyled;
use crate::ButtonHandle;
use beui_core::base::overlay::Placement;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

pub struct PopoverTriggerHandle {
    pub open: Binding<bool>,
    pub button: ButtonHandle,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PopoverPlacement {
    #[default]
    Below,
    Over(u16),
}

#[derive(Clone, Copy)]
pub struct PopoverHandle {
    pub open: Binding<bool>,
    pub close: Callback<()>,
}

#[derive(Clone, Copy)]
struct State {
    open: Binding<bool>,
    trigger: NodeRef,
}

#[component]
pub fn Popover(
    #[prop(default = ComponentRef::new())] api: ComponentRef<PopoverApi>,
    trigger: Render<PopoverTriggerHandle>,
    #[prop(children)] content: Render<PopoverHandle>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] open: Binding<bool>,
    anchor: Option<NodeRef>,
    #[prop(default = PopoverPlacement::Below)] placement: PopoverPlacement,
    #[prop(default = true)] refocus_trigger: Prop<bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let refocus = create_signal(false);
    let disabled = create_memo(move || disabled.get());
    let label = create_memo(move || label.get());
    let trigger_ref = NodeRef::new();
    api.fill(PopoverApi(State {
        open,
        trigger: trigger_ref,
    }));

    create_effect(move || {
        if disabled.get() {
            open.set(false);
        }
    });

    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::Button)));
    let trigger_accessibility = create_memo(move || {
        let mut node = accessibility.get();
        if node.label().is_none() && !label.get().is_empty() {
            node.set_label(label.get());
        }
        node.set_has_popup(HasPopup::Dialog);
        node.set_expanded(open.get());
        node
    });

    let content = content.once();

    let refocus_trigger = create_memo(move || refocus_trigger.get());
    let close = Callback::new(move |()| {
        if open.get_untracked() {
            if refocus_trigger.get_untracked() {
                refocus.set(true);
            }
            open.set(false);
        }
    });
    let anchor = anchor.unwrap_or(trigger_ref);
    let placement = match placement {
        PopoverPlacement::Below => Placement::BelowStart,
        PopoverPlacement::Over(inset) => Placement::Over(inset),
    };
    let handle = PopoverHandle { open, close };
    let toggle = move || {
        if !disabled.get_untracked() {
            open.update(|open| *open = !*open);
        }
    };
    view! {
        <List spacing=0.0>
            <unstyled::Button
                @node_ref=&trigger_ref
                label={label}
                glyph
                disabled={disabled}
                focused={refocus}
                accessibility={trigger_accessibility}
                on_focus_change={move |focused: bool| {
                    if !focused {
                        refocus.set(false);
                    }
                }}
                on_click={toggle}
            >|button: ButtonHandle| {
                trigger.call(PopoverTriggerHandle {
                    open,
                    button,
                })
            }</unstyled::Button>
            <Overlay
                anchor=anchor
                placement
                policy=LayerPolicy::Pass
                trigger={trigger_ref}
                open={open}
                keep_alive=true
                on_dismiss={move |_: Dismiss| {
                    open.set(false);
                    if refocus_trigger.get_untracked() {
                        refocus.set(true);
                    }
                }}
            >
                <PopoverSurface label>{content.call(handle)}</PopoverSurface>
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

#[derive(Clone, Copy)]
pub struct PopoverApi(State);

impl PopoverApi {
    pub fn open(&self) -> bool {
        self.0.open.get_untracked()
    }

    pub fn trigger(&self) -> NodeId {
        self.0.trigger.get()
    }
}

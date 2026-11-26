use accesskit::{HasPopup, Node, Role};
use beui_view::components::overlay::use_modal;
use beui_view::types::LayerPolicy;
use beui_view::types::icons::Glyph;

use crate::anchored::{Anchored, AnchoredHandle, AnchoredStyle, SheetWhen};
use crate::popup_trigger::{PopupTrigger, PopupTriggerHandle};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::Placement;
use beui_view::types::{Pos2, Rect};

#[derive(Clone, Copy)]
pub struct PopoverHandle {
    pub open: Binding<bool>,
    pub shown: Memo<bool>,
    pub close: Callback<()>,
}

#[derive(Clone, Copy)]
struct State {
    open: Binding<bool>,
    trigger: NodeRef,
}

/// A trigger and an anchored panel, built each time it opens (a form that keeps what was typed in it
/// while closed is wrapped in a `Kept`), that traps Tab; its layer gives the focus back on close.
///
/// Its layer passes input through (`LayerPolicy::Pass`): a press outside asks it
/// to close and then lands on what was pressed, except a press on its own trigger,
/// which is left to the trigger so a second click closes rather than reopens it.
/// Content takes focus by binding `focused` to the handle's `shown`.
///
/// Without a `trigger` there is no button: it hangs from `anchor`, which then names
/// something outside it, and opens and closes only through `open`. It takes any core
/// `placement`, and `beak` points an arrow at its anchor from the frame its surface binds
/// to `Dropdown::pointer` while it is above or below it.
#[component]
pub fn Popover(
    #[prop(default = ComponentRef::new())] api: ComponentRef<PopoverApi>,
    trigger: Option<Render<PopupTriggerHandle, NodeId>>,
    surface: AnchoredStyle,
    #[prop(children)] content: RenderFn<PopoverHandle, NodeId>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] open: Binding<bool>,
    #[prop(default = None)] at: Binding<Option<Pos2>>,
    anchor: Option<Prop<OverlayAnchor>>,
    band: Option<NodeRef>,
    #[prop(default = Placement::BELOW_START)] placement: Prop<Placement>,
    #[prop(default = None)] bounds: Prop<Option<Rect>>,
    #[prop(default = 0.0)] beak: Prop<f32>,
    #[prop(default = SheetWhen::Touched)] sheet: Prop<SheetWhen>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let disabled = create_memo(move || disabled.get());
    let label = create_memo(move || label.get());
    let trigger_ref = NodeRef::new();
    api.fill(PopoverApi(State {
        open,
        trigger: trigger_ref,
    }));

    create_effect(move || {
        if disabled.get() {
            untrack(|| open.set(false));
        }
    });

    let close = Callback::new(move |()| open.set(false));
    let anchor = anchor.unwrap_or(Prop::Static(OverlayAnchor::Node(trigger_ref)));
    // A press on the button, or on the node it hangs from, is left to that to toggle it.
    let has_button = trigger.is_some();
    let held = anchor.clone();
    let pressed = create_memo(move || match has_button {
        true => Some(trigger_ref),
        false => held.get().node(),
    });
    let anchor = match band {
        Some(band) => Prop::dynamic(move || match anchor.get() {
            OverlayAnchor::Node(node) => OverlayAnchor::Band { node, band },
            anchor => anchor,
        }),
        None => anchor,
    };
    let button = trigger.map(|trigger| {
        view! {
            <PopupTrigger
                @node_ref=&trigger_ref
                open
                popup=HasPopup::Dialog
                label={label}
                glyph
                icon_only
                disabled={disabled}
                accessibility={accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::Button)))}
                content={trigger}
            />
        }
    });
    view! {
        <List spacing=0.0>
            {button}
            <Anchored
                style={surface}
                anchor
                at
                placement
                bounds
                beak
                sheet
                policy=LayerPolicy::Pass
                trigger={pressed}
                open
            >|handle: AnchoredHandle|
                <PopoverSurface label>
                    {content.call(PopoverHandle {
                        open,
                        shown: handle.shown,
                        close,
                    })}
                </PopoverSurface>
            </Anchored>
        </List>
    }
}

#[component]
fn PopoverSurface(label: Memo<String>, children: Child) -> NodeId {
    let modal = use_modal();
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Dialog);
        let label = label.get();
        if !label.is_empty() {
            node.set_label(label);
        }
        if modal.get() {
            node.set_modal();
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

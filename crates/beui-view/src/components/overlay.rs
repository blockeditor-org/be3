use beui_core::color::Color32;

use std::cell::RefCell;

use crate::reactive::{
    Callback, Get, Memo, NodeRef, Prop, RenderFn, Scope, Set, Signal, create_binding,
    create_effect, create_memo, create_signal, owner_scope, provide_context, store, untrack,
    use_context, with_document,
};
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::layer::{Dismiss, LayerLevel, LayerPolicy};
use beui_core::node::NodeId;
use beui_core::window::WindowId;
use beui_macros::component;

#[derive(Clone, Copy)]
pub struct OverlayPresence {
    open: Memo<bool>,
    lingering: Signal<Vec<Memo<bool>>>,
}

impl OverlayPresence {
    pub fn open(self) -> Memo<bool> {
        self.open
    }

    pub fn linger_while(self, showing: Memo<bool>) {
        self.lingering.update(|lingering| lingering.push(showing));
    }
}

pub fn use_overlay_presence() -> Option<OverlayPresence> {
    use_context::<OverlayPresence>()
}

struct Mounted {
    content: NodeId,
    scope: Scope,
}

#[component]
pub fn Overlay(
    anchor: Prop<OverlayAnchor>,
    #[prop(default = Placement::BelowStart)] placement: Prop<Placement>,
    #[prop(default = Color32::TRANSPARENT)] scrim: Prop<Color32>,
    #[prop(default = LayerPolicy::BlocksAll)] policy: Prop<LayerPolicy>,
    #[prop(default = LayerLevel::Normal)] level: Prop<LayerLevel>,
    #[prop(default = true)] traps_focus: Prop<bool>,
    #[prop(default = None)] shown_in: Prop<Option<WindowId>>,
    trigger: Option<NodeRef>,
    pointer: Option<NodeRef>,
    open: Prop<bool>,
    #[prop(default = false)] keep_alive: bool,
    on_dismiss: Callback<Dismiss>,
    #[prop(children)] content: RenderFn<()>,
) -> NodeId {
    let overlay = with_document(|document| {
        let overlay = document.create_overlay(anchor.get_untracked(), Placement::BelowStart);
        document.set_overlay_trigger(overlay, trigger);
        document.set_overlay_pointer(overlay, pointer);
        document.set_overlay_on_dismiss(overlay, on_dismiss);
        overlay
    });
    create_binding(move || {
        let anchor = anchor.get();
        with_document(|document| document.set_overlay_anchor(overlay, anchor))
    });
    create_binding(move || {
        let shown_in = shown_in.get();
        with_document(|document| document.set_overlay_shown_in(overlay, shown_in))
    });
    create_binding(move || {
        let (placement, scrim, traps_focus) = (placement.get(), scrim.get(), traps_focus.get());
        let (policy, level) = (policy.get(), level.get());
        with_document(|document| {
            document.set_overlay_placement(overlay, placement);
            document.set_overlay_scrim(overlay, scrim);
            document.set_overlay_traps_focus(overlay, traps_focus);
            document.set_overlay_policy(overlay, policy);
            document.set_overlay_level(overlay, level);
        })
    });
    let presence = OverlayPresence {
        open: create_memo(move || open.get()),
        lingering: create_signal(Vec::new()),
    };
    let shown = create_memo(move || {
        presence.open.get()
            || presence
                .lingering
                .with(|lingering| lingering.iter().any(Get::get))
    });
    let owner = owner_scope().expect("an overlay is built inside a scope");
    let mounted = store(RefCell::new(None::<Mounted>));
    create_effect(move || {
        let shown = shown.get();
        untrack(|| {
            let held = mounted.value();
            let mut held = held.borrow_mut();
            if shown {
                let content = match held.as_ref() {
                    Some(held) => {
                        if keep_alive {
                            held.scope.resume();
                        }
                        held.content
                    }
                    None => {
                        let scope = owner.run(|| match keep_alive {
                            true => Scope::detached().pausable(),
                            false => Scope::detached(),
                        });
                        let built = scope.context().run(|| {
                            provide_context(presence);
                            content.call(())
                        });
                        *held = Some(Mounted {
                            content: built,
                            scope,
                        });
                        built
                    }
                };
                with_document(|document| {
                    document.set_overlay_content(overlay, Some(content));
                    document.open_overlay(overlay);
                });
                return;
            }
            with_document(|document| {
                document.close_overlay(overlay);
                document.set_overlay_content(overlay, None);
            });
            match keep_alive {
                true => {
                    if let Some(held) = held.as_ref() {
                        held.scope.pause();
                    }
                }
                false => {
                    presence.lingering.update(Vec::clear);
                    drop(held.take());
                }
            }
        });
    });
    overlay.id()
}

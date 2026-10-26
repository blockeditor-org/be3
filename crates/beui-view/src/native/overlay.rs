use crate::reactive::Callback;
use crate::reactive::Get;
use crate::reactive::Memo;
use crate::reactive::NodeRef;
use crate::reactive::Prop;
use crate::reactive::RenderFn;
use crate::reactive::Scope;
use crate::reactive::Set;
use crate::reactive::Signal;
use crate::reactive::create_binding;
use crate::reactive::create_effect;
use crate::reactive::create_memo;
use crate::reactive::create_signal;
use crate::reactive::owner_scope;
use crate::reactive::provide_context;
use crate::reactive::store;
use crate::reactive::untrack;
use crate::reactive::use_context;
use crate::reactive::with_document;
use beui_core::base::overlay::OverlayAnchor;
use beui_core::base::overlay::Placement;
use beui_core::base::overlay::WindowSpec;
use beui_core::color::Color32;
use beui_core::layer::Dismiss;
use beui_core::layer::LayerLevel;
use beui_core::layer::LayerPolicy;
use beui_core::node::NodeId;
use beui_core::window::WindowId;
use std::cell::RefCell;

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

pub fn overlay(
    anchor: Prop<OverlayAnchor>,
    placement: Prop<Placement>,
    scrim: Prop<Color32>,
    policy: Prop<LayerPolicy>,
    level: Prop<LayerLevel>,
    traps_focus: Prop<bool>,
    shown_in: Prop<Option<WindowId>>,
    trigger: Option<NodeRef>,
    pointer: Option<NodeRef>,
    beak: Prop<f32>,
    window: Prop<Option<WindowSpec>>,
    open: Prop<bool>,
    keep_alive: bool,
    on_dismiss: Callback<Dismiss>,
    content: RenderFn<()>,
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
        let beak = beak.get();
        with_document(|document| document.set_overlay_beak(overlay, beak))
    });
    create_binding(move || {
        let window = window.get();
        with_document(|document| document.set_overlay_window(overlay, window))
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

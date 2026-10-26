use std::cell::RefCell;
use std::rc::Rc;

use crate::reactive::Callback;
use crate::reactive::Get;
use crate::reactive::Memo;
use crate::reactive::NodeId;
use crate::reactive::NodeRef;
use crate::reactive::OverlayAnchor;
use crate::reactive::Prop;
use crate::reactive::RenderFn;
use crate::reactive::Scope;
use crate::reactive::Set;
use crate::reactive::Signal;
use crate::reactive::create_binding;
use crate::reactive::create_memo;
use crate::reactive::create_signal;
use crate::reactive::provide_context;
use crate::reactive::untrack;
use crate::reactive::use_context;
use crate::types::Color32;
use crate::types::Dismiss;
use crate::types::LayerLevel;
use crate::types::LayerPolicy;
use crate::types::Placement;
use crate::types::WindowId;
use crate::types::WindowSpec;
use beui_core_html::document::{create, set_attribute, set_style};

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
    let _ = (
        anchor,
        placement,
        scrim,
        policy,
        level,
        traps_focus,
        shown_in,
        trigger,
        pointer,
        beak,
        window,
        on_dismiss,
    );
    let overlay = create("Overlay", "div");
    set_style(overlay, "position", Some("relative".to_owned()));
    let host = beui_core_html::tree::node_host(overlay);
    let slot = host.open();
    let lingering = create_signal(Vec::<Memo<bool>>::new());
    let open = create_memo(move || open.get());
    let shown = create_memo(move || open.get() || lingering.get().iter().any(|memo| memo.get()));
    let held: Rc<RefCell<Option<(Scope, NodeId)>>> = Rc::default();
    create_binding(move || {
        let shown = shown.get();
        let mut held = held.borrow_mut();
        match (shown, held.as_ref()) {
            (true, Some((_, content))) => {
                set_attribute(*content, "hidden", None);
            }
            (true, None) => {
                let scope = Scope::detached();
                let built = untrack(|| {
                    scope.run(|| {
                        provide_context(OverlayPresence { open, lingering });
                        let built = content.call(());
                        let frame = super::stand_in("OverlayContent");
                        for (property, value) in [
                            ("position", "absolute"),
                            ("top", "100%"),
                            ("left", "0"),
                            ("z-index", "1000"),
                        ] {
                            set_style(frame, property, Some(value.to_owned()));
                        }
                        beui_core_html::document::append(frame, built);
                        frame
                    })
                });
                host.fill(slot, vec![built]);
                *held = Some((scope, built));
            }
            (false, Some((_, content))) if keep_alive => {
                set_attribute(*content, "hidden", Some(String::new()));
            }
            (false, Some(_)) => {
                host.fill(slot, Vec::new());
                *held = None;
            }
            (false, None) => {}
        }
    });
    overlay
}

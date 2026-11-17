use crate::components::overlay::OverlayPresence;
use std::cell::RefCell;

use crate::reactive::BuildFn;
use crate::reactive::Callback;
use crate::reactive::Get;
use crate::reactive::Memo;
use crate::reactive::NodeId;
use crate::reactive::NodeRef;
use crate::reactive::OverlayAnchor;
use crate::reactive::Prop;
use crate::reactive::Scope;
use crate::reactive::Set;
use crate::reactive::create_effect;
use crate::reactive::owner_scope;
use crate::reactive::store;
use crate::reactive::untrack;
use crate::types::Color;
use crate::types::Dismiss;
use crate::types::LayerLevel;
use crate::types::LayerPolicy;
use crate::types::Placement;
use crate::types::Rect;
use crate::types::WindowId;
use crate::types::WindowSpec;
use beui_core_html::document::create;
use beui_core_html::overlay::Parts;

use super::bind;

struct Mounted {
    /// Owns the content while it is shown; dropping it disposes the content.
    _scope: Scope,
}

pub(crate) fn overlay(
    anchor: Prop<OverlayAnchor>,
    placement: Prop<Placement>,
    bounds: Prop<Option<Rect>>,
    scrim: Prop<Color>,
    scrim_radius: Prop<f32>,
    policy: Prop<LayerPolicy>,
    level: Prop<LayerLevel>,
    traps_focus: Prop<bool>,
    shown_in: Prop<Option<WindowId>>,
    trigger: Prop<Option<NodeRef>>,
    pointer: Option<NodeRef>,
    beak: Prop<f32>,
    window: Prop<Option<WindowSpec>>,
    dismissable: Prop<bool>,
    on_dismiss: Callback<Dismiss>,
    presence: OverlayPresence,
    content: BuildFn<NodeId>,
) -> NodeId {
    let _ = (shown_in, window, scrim_radius, bounds);
    let host = create("Overlay", "div");
    let dialog = create("OverlayLayer", "dialog");
    let proxy = create("OverlayAnchor", "div");
    let pointing = create("OverlayBeak", "div");
    let hosted = beui_core_html::tree::node_host(host);
    hosted.append(proxy);
    hosted.append(dialog);
    let layered = beui_core_html::tree::node_host(dialog);
    let slot = layered.open();
    layered.append(pointing);
    let layer = beui_core_html::overlay::layer(
        Parts {
            host,
            dialog,
            proxy,
            beak: pointing,
        },
        pointer,
        on_dismiss,
    );
    let held = layer.clone();
    bind(anchor, move |anchor| held.set_anchor(anchor));
    let held = layer.clone();
    bind(placement, move |placement| held.set_placement(placement));
    let held = layer.clone();
    bind(scrim, move |scrim| held.set_scrim(scrim));
    let held = layer.clone();
    bind(policy, move |policy| held.set_policy(policy));
    let held = layer.clone();
    bind(level, move |level| held.set_level(level));
    let held = layer.clone();
    bind(traps_focus, move |traps| held.set_traps_focus(traps));
    let held = layer.clone();
    bind(dismissable, move |dismissable| {
        held.set_dismissable(dismissable)
    });
    let held = layer.clone();
    bind(beak, move |reach| held.set_beak(reach));
    let held = layer.clone();
    bind(trigger, move |trigger| held.set_trigger(trigger));
    let shown = presence.shown();
    let owner = owner_scope().expect("an overlay is built inside a scope");
    let mounted = store(RefCell::new(None::<Mounted>));
    create_effect(move || {
        let shown = shown.get();
        untrack(|| {
            let held = mounted.value();
            let mut held = held.borrow_mut();
            if shown {
                if held.is_none() {
                    let scope = owner.run(Scope::detached);
                    let built = scope.context().run(|| {
                        presence.provide();
                        content.call()
                    });
                    layered.fill(slot, vec![built]);
                    *held = Some(Mounted { _scope: scope });
                }
                layer.show(true);
                return;
            }
            layer.show(false);
            layered.fill(slot, Vec::new());
            drop(held.take());
            // What content a `Kept` keeps registered goes on lingering each time it closes.
            presence
                .lingering()
                .update(|lingering| lingering.retain(Memo::is_alive));
        });
    });
    host
}

use crate::components::overlay::OverlayPresence;
use std::cell::RefCell;

use crate::reactive::Callback;
use crate::reactive::Get;
use crate::reactive::NodeId;
use crate::reactive::NodeRef;
use crate::reactive::OverlayAnchor;
use crate::reactive::Prop;
use crate::reactive::RenderFn;
use crate::reactive::Scope;
use crate::reactive::Set;
use crate::reactive::create_effect;
use crate::reactive::owner_scope;
use crate::reactive::provide_context;
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
    scope: Scope,
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
    trigger: Option<NodeRef>,
    pointer: Option<NodeRef>,
    beak: Prop<f32>,
    window: Prop<Option<WindowSpec>>,
    keep_alive: bool,
    dismissable: Prop<bool>,
    on_dismiss: Callback<Dismiss>,
    presence: OverlayPresence,
    content: RenderFn<()>,
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
        trigger,
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
    let shown = presence.shown();
    let owner = owner_scope().expect("an overlay is built inside a scope");
    let mounted = store(RefCell::new(None::<Mounted>));
    create_effect(move || {
        let shown = shown.get();
        untrack(|| {
            let held = mounted.value();
            let mut held = held.borrow_mut();
            if shown {
                match held.as_ref() {
                    Some(held) => {
                        if keep_alive {
                            held.scope.resume();
                        }
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
                        layered.fill(slot, vec![built]);
                        *held = Some(Mounted { scope });
                    }
                }
                layer.show(true);
                return;
            }
            layer.show(false);
            match keep_alive {
                true => {
                    if let Some(held) = held.as_ref() {
                        held.scope.pause();
                    }
                }
                false => {
                    presence.lingering().update(Vec::clear);
                    layered.fill(slot, Vec::new());
                    drop(held.take());
                }
            }
        });
    });
    host
}

use beui_core::color::Color32;

use crate::reactive::{Callback, Child, NodeRef, Prop, create_effect, with_document};
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::layer::{Dismiss, LayerLevel, LayerPolicy};
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Overlay(
    anchor: Prop<OverlayAnchor>,
    #[prop(default = Placement::BelowStart)] placement: Prop<Placement>,
    #[prop(default = Color32::TRANSPARENT)] scrim: Prop<Color32>,
    #[prop(default = LayerPolicy::BlocksAll)] policy: Prop<LayerPolicy>,
    #[prop(default = LayerLevel::Normal)] level: Prop<LayerLevel>,
    #[prop(default = true)] traps_focus: Prop<bool>,
    trigger: Option<NodeRef>,
    open: Prop<bool>,
    on_dismiss: Callback<Dismiss>,
    children: Option<Child>,
) -> NodeId {
    let content = children;
    let overlay = with_document(|document| {
        let overlay = document.create_overlay(anchor.peek(), Placement::BelowStart);
        if let Some(content) = content {
            document.set_overlay_content(overlay, content);
        }
        document.set_overlay_trigger(overlay, trigger);
        document.set_overlay_on_dismiss(overlay, on_dismiss);
        overlay
    });
    create_effect(move || {
        with_document(|document| document.set_overlay_anchor(overlay, anchor.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_overlay_placement(overlay, placement.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_overlay_scrim(overlay, scrim.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_overlay_traps_focus(overlay, traps_focus.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_overlay_policy(overlay, policy.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_overlay_level(overlay, level.get()))
    });
    create_effect(move || {
        let open = open.get();
        with_document(|document| match open {
            true => document.open_overlay(overlay),
            false => document.close_overlay(overlay),
        });
    });
    overlay.id()
}

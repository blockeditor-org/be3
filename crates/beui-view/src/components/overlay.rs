use beui_core::color::Color32;

use crate::reactive::{
    Child, ClickCallback, IntoProp, NodeRef, Prop, create_effect, with_document,
};
use beui_core::base::overlay::{OverlayAnchor, OverlayMode, Placement};
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Overlay(
    anchor: Prop<OverlayAnchor>,
    #[prop(default = Placement::BelowStart)] placement: Prop<Placement>,
    #[prop(default = Color32::TRANSPARENT)] scrim: Prop<Color32>,
    #[prop(default = true)] traps_focus: Prop<bool>,
    #[prop(default = false)] light: Prop<bool>,
    trigger: Option<NodeRef>,
    #[prop(default = OverlayMode::Modal)] mode: Prop<OverlayMode>,
    open: Prop<bool>,
    on_dismiss: ClickCallback,
    children: Option<Child>,
) -> NodeId {
    let content = children;
    let overlay = with_document(|document| {
        let overlay = document.create_overlay(anchor.peek(), Placement::BelowStart);
        if let Some(content) = content {
            document.set_overlay_content(overlay, content);
        }
        document.set_overlay_trigger(overlay, trigger);
        if !on_dismiss.is_empty() {
            document.set_overlay_on_dismiss(overlay, move || on_dismiss.call());
        }
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
        with_document(|document| document.set_overlay_light(overlay, light.get()))
    });
    create_effect(move || with_document(|document| document.set_overlay_mode(overlay, mode.get())));
    create_effect(move || {
        let open = open.get();
        with_document(|document| match open {
            true => document.open_overlay(overlay),
            false => document.close_overlay(overlay),
        });
    });
    overlay.id()
}

impl IntoProp<OverlayAnchor> for &NodeRef {
    fn into_prop(self) -> Prop<OverlayAnchor> {
        Prop::Static(OverlayAnchor::Node(self.clone()))
    }
}

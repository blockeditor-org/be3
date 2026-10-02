use beui_macros::{component, view};

use crate::dialog::close_overlay;
use beui_components_unstyled::BackSlide;
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color32;
use beui_core::geometry::Pos2;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{Child, ClickCallback, NodeRef, Prop, create_memo, create_signal};

const SCRIM: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 230);

#[component]
pub fn Fullscreen(open: Prop<bool>, on_dismiss: ClickCallback, children: Child) -> NodeId {
    let overlay = NodeRef::new();
    let closing = overlay.clone();
    let (presence, set_presence) = create_signal(1.0_f32);
    let scrim = create_memo(move || SCRIM.scale_alpha(presence.get()));
    view! {
        <Overlay
            @node_ref=&overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            open={open}
            placement=Placement::Fill
            scrim={scrim}
            on_dismiss={move || on_dismiss.call()}
        >
            <BackSlide
                enters=false
                on_back={move || close_overlay(&closing)}
                on_presence={move |presence: f32| set_presence.set(presence)}
            >
                {children}
            </BackSlide>
        </Overlay>
    }
}

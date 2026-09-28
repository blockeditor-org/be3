use beui_macros::{component, view};

use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color32;
use beui_core::geometry::Pos2;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{Child, ClickCallback, Prop};

const SCRIM: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 230);

#[component]
pub fn Fullscreen(open: Prop<bool>, on_dismiss: ClickCallback, children: Child) -> NodeId {
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            open={open}
            placement=Placement::Fill
            scrim=SCRIM
            on_dismiss={move || on_dismiss.call()}
        >
            {children}
        </Overlay>
    }
}

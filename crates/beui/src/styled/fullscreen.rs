use beui_macros::{component, view};

use crate::base::overlay::{Overlay, OverlayAnchor, Placement};
use crate::color::Color32;
use crate::geometry::Pos2;
use crate::node::NodeId;
use crate::reactive::{Child, ClickCallback, Prop};

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

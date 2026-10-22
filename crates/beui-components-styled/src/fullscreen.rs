use beui_core::layer::Dismiss;
use beui_macros::{component, view};

use beui_components_unstyled::BackSlide;
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::geometry::Pos2;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::*;

#[component]
pub fn Fullscreen(
    open: Prop<bool>,
    on_dismiss: ClickCallback,
    #[prop(children)] content: Render,
) -> NodeId {
    let content = content.once();
    let presence = create_signal(1.0_f32);
    let theme = crate::theme::use_theme();
    let scrim = create_memo(move || theme.scrim_strong.get().scale_alpha(presence.get()));
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            open={open}
            placement=Placement::Fill
            scrim={scrim}
            keep_alive=true
            on_dismiss={move |_: Dismiss| on_dismiss.call()}
        >
            <BackSlide enters=false on_back={on_dismiss} on_presence={presence}>
                {content.call(())}
            </BackSlide>
        </Overlay>
    }
}

use beui_macros::{component, view};

use crate::frame::{ColorRole, Radius};
use crate::theme::use_theme;
use crate::tokens::Stroke;
use beui_view::primitives;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

#[component]
pub fn FocusRing(
    focused: Prop<bool>,
    #[prop(default = Radius::Md)] radius: Radius,
    #[prop(default = Stroke::None)] offset: Prop<Stroke>,
    children: Child,
) -> NodeId {
    let theme = use_theme();
    let ring = focus_ring(focused);
    let outline = create_memo(move || {
        ring.get().then(|| {
            Outline::new(ColorRole::Accent.resolve(&theme), Stroke::Focus.px())
                .offset(offset.get().px())
        })
    });
    let corners = offset.map(move |offset| radius.around(offset));
    view! {
        <primitives::Frame outline radius=corners>{children}</primitives::Frame>
    }
}

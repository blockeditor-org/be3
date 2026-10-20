use crate::frame::{Border, ColorRole, Frame, Radius};
use accesskit::{Node, Role};
use beui_core::layer::Dismiss;
use beui_macros::{component, view};

use crate::text::{Text, TextStyle};
use beui_components_unstyled::BackSlide;
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color32;
use beui_core::geometry::Pos2;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

const PADDING_HORIZONTAL: f32 = 20.0;
const PADDING_VERTICAL: f32 = 18.0;
const SPACING: f32 = 12.0;
const MARGIN: f32 = 12.0;
const SCRIM: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 150);

#[component]
pub fn Dialog(
    open: Prop<bool>,
    #[prop(default = String::new())] title: Prop<String>,
    #[prop(default = 360.0)] width: Prop<f32>,
    on_dismiss: ClickCallback,
    #[prop(children)] content: Render,
) -> NodeId {
    let closing = on_dismiss;
    let presence = create_signal(1.0_f32);
    let scrim = create_memo(move || SCRIM.scale_alpha(presence.get()));
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            open={open}
            placement=Placement::Center
            scrim={scrim}
            on_dismiss={move |_: Dismiss| on_dismiss.call()}
            content={move || view! {
                <BackSlide
                    enters=false
                    on_back={closing}
                    on_presence={move |next: f32| presence.set(next)}
                >
                    <DialogSurface width={width} title={title}>{content.call(())}</DialogSurface>
                </BackSlide>
            }}
        />
    }
}

#[component]
fn DialogSurface(width: Prop<f32>, title: Prop<String>, children: Child) -> NodeId {
    let label = create_memo(move || title.get());
    let titled = create_memo(move || !label.get().is_empty());
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Dialog);
        let title = label.get();
        if !title.is_empty() {
            node.set_label(title);
        }
        node
    }));
    view! {
        <Frame padding=MARGIN>
            <Frame
                width={width.map(Extent::at_most)}
                fill=ColorRole::SurfaceRaised
                border={Border::DEFAULT}
                radius=Radius::Lg
                padding={Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
            >
                <List spacing=SPACING>
                    <Show condition={titled}>
                        <Text style=TextStyle::TITLE string={label} />
                    </Show>
                    {children}
                </List>
            </Frame>
        </Frame>
    }
}

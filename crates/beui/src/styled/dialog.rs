use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::base::overlay::{Overlay, OverlayAnchor, Placement};
use crate::color::Color32;
use crate::geometry::Pos2;
use crate::node::NodeId;
use crate::reactive::{
    Child, ClickCallback, Frame, List, Prop, Show, clone, component_accessibility, create_memo,
};
use crate::styled::text::Title;
use crate::styled::theme::{BORDER_WIDTH, CARD_RADIUS, use_theme};

const PADDING_HORIZONTAL: f32 = 20.0;
const PADDING_VERTICAL: f32 = 18.0;
const SPACING: f32 = 12.0;
const SCRIM: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 150);

#[component]
pub fn Dialog(
    open: Prop<bool>,
    #[prop(default = String::new())] title: Prop<String>,
    #[prop(default = 360.0)] width: Prop<f32>,
    on_dismiss: ClickCallback,
    children: Child,
) -> NodeId {
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            open={open}
            placement=Placement::Center
            scrim=SCRIM
            on_dismiss={move || on_dismiss.call()}
        >
            <DialogSurface width={width} title={title}>{children}</DialogSurface>
        </Overlay>
    }
}

#[component]
fn DialogSurface(width: Prop<f32>, title: Prop<String>, children: Child) -> NodeId {
    let label = create_memo(move || title.get());
    let titled = create_memo(clone!(label -> move || !label.get().is_empty()));
    component_accessibility(create_memo(clone!(label -> move || {
        let mut node = Node::new(Role::Dialog);
        let title = label.get();
        if !title.is_empty() {
            node.set_label(title);
        }
        node
    })));
    let theme = use_theme();
    view! {
        <Frame
            width={width}
            color={theme.surface_raised.clone()}
            outline={theme.border.clone()}
            outline_width=BORDER_WIDTH
            outline_visible=true
            radius=CARD_RADIUS
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=PADDING_VERTICAL
        >
            <List spacing=SPACING>
                <Show condition={titled}>
                    <Title content={label} />
                </Show>
                {children}
            </List>
        </Frame>
    }
}

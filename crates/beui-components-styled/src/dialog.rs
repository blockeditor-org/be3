use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::text::Title;
use crate::theme::{BORDER_WIDTH, CARD_RADIUS, use_theme};
use beui_components_unstyled::BackSlide;
use beui_core::base::overlay::{OverlayAnchor, OverlayNode, Placement};
use beui_core::color::Color32;
use beui_core::geometry::Pos2;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Child, ClickCallback, Frame, List, NodeRef, Prop, Show, clone, component_accessibility,
    create_memo, create_signal, with_document,
};

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
    children: Child,
) -> NodeId {
    let overlay = NodeRef::new();
    let closing = overlay.clone();
    let (presence, set_presence) = create_signal(1.0_f32);
    let scrim = create_memo(move || SCRIM.scale_alpha(presence.get()));
    view! {
        <Overlay
            @node_ref=&overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            open={open}
            placement=Placement::Center
            scrim={scrim}
            on_dismiss={move || on_dismiss.call()}
        >
            <BackSlide
                enters=false
                on_back={move || dismiss_overlay(&closing)}
                on_presence={move |presence: f32| set_presence.set(presence)}
            >
                <DialogSurface width={width} title={title}>{children}</DialogSurface>
            </BackSlide>
        </Overlay>
    }
}

pub(crate) fn dismiss_overlay(overlay: &NodeRef) {
    let Some(id) = overlay.try_get() else {
        return;
    };
    with_document(|document| {
        if let Some(overlay) = document.arena.kind_of::<OverlayNode>(id) {
            document.dismiss_overlay(overlay);
        }
    });
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
        <Frame padding_horizontal=MARGIN padding_vertical=MARGIN>
            <Frame
                max_width={width.map(Some)}
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
                        <Title content={label.clone()} />
                    </Show>
                    {children}
                </List>
            </Frame>
        </Frame>
    }
}

use beui_macros::{component, view};

use crate::dialog::{Modal, Scrim};
use beui_components_unstyled::ModalPlacement;
use beui_core::node::NodeId;
use beui_view::reactive::*;

#[component]
pub fn Fullscreen(
    open: Prop<bool>,
    on_dismiss: ClickCallback,
    #[prop(children)] content: Render,
) -> NodeId {
    view! {
        <Modal open scrim=Scrim::Strong placement=ModalPlacement::Fill on_dismiss={on_dismiss}>
            {content.call(())}
        </Modal>
    }
}

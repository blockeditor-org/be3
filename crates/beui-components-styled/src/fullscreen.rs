use crate::dialog::{Modal, Scrim};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

#[component]
pub fn Fullscreen(
    open: Prop<bool>,
    on_dismiss: Callback<()>,
    #[prop(children)] content: BuildFn<NodeId>,
) -> NodeId {
    view! {
        <Modal open scrim=Scrim::Strong on_dismiss={on_dismiss}>{content.call()}</Modal>
    }
}

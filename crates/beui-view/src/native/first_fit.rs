use crate::reactive::Children;
use crate::reactive::Placed;
use crate::reactive::with_document;
use beui_core::base::first_fit::FirstFitNode;
use beui_core::document::Document;
use beui_core::node::NodeId;

pub type Fitting = Placed<FirstFitNode>;

pub fn first_fit(
    children: Children<Fitting>
) -> NodeId {
    let fit = with_document(Document::create_first_fit);
    children.mount(fit);
    fit.id()
}

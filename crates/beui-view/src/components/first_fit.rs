use crate::reactive::{Children, Placed, with_document};
use beui_core::base::first_fit::FirstFitNode;
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_macros::component;

pub type Fitting = Placed<FirstFitNode>;

#[component]
pub fn FirstFit(children: Children<Fitting>) -> NodeId {
    let fit = with_document(Document::create_first_fit);
    children.mount(fit);
    fit.id()
}

pub use crate::core::first_fit::Fitting;
use crate::reactive::Children;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn FirstFit(children: Children<Fitting>) -> NodeId {
    crate::core::first_fit(children)
}

use beui_core::node::NodeId;
use beui_macros::component;
use crate::reactive::Children;
pub use crate::core::first_fit::{Fitting};

#[component]
pub fn FirstFit(
    children: Children<Fitting>
) -> NodeId {
    crate::core::first_fit(children)
}

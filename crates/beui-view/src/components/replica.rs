use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::component;

/// Draws `of` here instead of where it is laid out, so nothing around `of` clips it: `of`
/// keeps its place, its size and its input there, and is not drawn there while a replica
/// draws it. Natively the replica paints `of`'s own retained paint, as it changes; on HTML it
/// holds a copy of `of`'s elements, made again as they change, and `of` is drawn at
/// opacity 0. A carried item's face floats as one (guides/beui/components.md, Carries and
/// drop sites). Give it `of`'s size: it fills what it is given.
#[component]
pub fn Replica(#[prop(default = None)] of: Prop<Option<NodeId>>) -> NodeId {
    crate::core::replica(of)
}

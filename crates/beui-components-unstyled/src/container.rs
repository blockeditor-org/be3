use beui_macros::{component, view};

use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Vec2;

#[derive(Clone, Copy)]
pub struct ContainerSize(pub ReadSignal<Vec2>);

#[component]
pub fn Container(#[prop(children)] content: Render<ReadSignal<Vec2>>) -> NodeId {
    view! {
        <LayoutBuilder>|size: ReadSignal<Vec2>| {
            provide_context(ContainerSize(size));
            content.call(size)
        }</LayoutBuilder>
    }
}

pub fn container_size() -> Option<ReadSignal<Vec2>> {
    use_context::<ContainerSize>().map(|ContainerSize(size)| size)
}

fn nearest_container(query: &str) -> Option<ReadSignal<Vec2>> {
    let size = container_size();
    debug_assert!(
        size.is_some(),
        "`{query}` answers for the nearest `Container` above it, and there is none. \
         Build the code that asks inside a `Container`'s content; children written between \
         a wrapper's tags are built before the wrapper's body runs, so a wrapper that \
         provides one takes them as a `#[prop(children)] Render`"
    );
    size
}

pub fn narrower_than(width: f32) -> Memo<bool> {
    let size = nearest_container("narrower_than");
    create_memo(move || {
        size.as_ref().is_some_and(|size| {
            let measured = size.get().x;
            measured > 0.0 && measured < width
        })
    })
}

pub fn shorter_than(height: f32) -> Memo<bool> {
    let size = nearest_container("shorter_than");
    create_memo(move || {
        size.as_ref().is_some_and(|size| {
            let measured = size.get().y;
            measured > 0.0 && measured < height
        })
    })
}

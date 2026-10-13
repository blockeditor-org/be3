use beui_macros::{component, view};

use beui_core::geometry::Vec2;
use beui_core::node::NodeId;
use beui_view::reactive::{
    LayoutBuilder, Memo, ReadSignal, Render, create_memo, provide_context, use_context,
};

#[derive(Clone)]
pub struct ContainerSize(pub ReadSignal<Vec2>);

#[component]
pub fn Container(#[prop(children)] content: Render<ReadSignal<Vec2>>) -> NodeId {
    view! {
        <LayoutBuilder>
            {move |size: ReadSignal<Vec2>| {
                provide_context(ContainerSize(size));
                content.call(size)
            }}
        </LayoutBuilder>
    }
}

pub fn container_size() -> Option<ReadSignal<Vec2>> {
    use_context::<ContainerSize>().map(|ContainerSize(size)| size)
}

pub fn narrower_than(width: f32) -> Memo<bool> {
    let size = container_size();
    create_memo(move || {
        size.as_ref().is_some_and(|size| {
            let measured = size.get().x;
            measured > 0.0 && measured < width
        })
    })
}

pub fn shorter_than(height: f32) -> Memo<bool> {
    let size = container_size();
    create_memo(move || {
        size.as_ref().is_some_and(|size| {
            let measured = size.get().y;
            measured > 0.0 && measured < height
        })
    })
}

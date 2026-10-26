use beui_macros::{component, view};

use beui_core::geometry::Rect;
use beui_core::node::NodeId;
use beui_core::screens::bounds;
use beui_view::primitives::*;
use beui_view::reactive::*;

#[derive(Clone, Copy)]
pub struct ScreenSlot {
    pub index: usize,
    pub rect: Memo<Rect>,
}

#[component]
pub fn EveryScreen(
    #[prop(default = Align2::CENTER)] align: Prop<Align2>,
    #[prop(children)] content: RenderFn<ScreenSlot>,
) -> NodeId {
    let screens = use_screens();
    let all = create_memo(move || screens.with(|screens| bounds(screens)));
    let places = create_memo(move || (0..screens.with(Vec::len)).collect::<Vec<usize>>());
    let width = create_memo(move || Extent::px(all.get().width()));
    let height = create_memo(move || Extent::px(all.get().height()));
    view! {
        <Frame width height>
            <Layers>
                <ForEach keys={places}>|index: usize| {
                    let rect = create_memo(move || {
                        let origin = all.get().min.to_vec2();
                        screens.with(|screens| {
                            screens
                                .get(index)
                                .map_or(Rect::ZERO, |screen| screen.rect.translate(-origin))
                        })
                    });
                    let inset = create_memo(move || Sides::new(rect.get().left(), rect.get().top(), 0.0, 0.0));
                    let width = create_memo(move || Extent::px(rect.get().width()));
                    let height = create_memo(move || Extent::px(rect.get().height()));
                    let slot = content.call(ScreenSlot { index, rect });
                    let layer: Layer = view! {
                        <Frame padding=inset align=Align::Start>
                            <Frame width height align>{slot}</Frame>
                        </Frame>
                    }
                    .into_child();
                    layer
                }</ForEach>
            </Layers>
        </Frame>
    }
}

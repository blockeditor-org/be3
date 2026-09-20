use beui_macros::{component, view};

use crate::base::{Direction, ItemSize, ScrollPosition};
use crate::color::Color32;
use crate::node::NodeId;
use crate::reactive;
use crate::reactive::{
    Callback, Children, List, ListChild, Memo, Prop, ReadSignal, RenderFn, create_memo,
    create_signal,
};

pub struct ScrollHandle {
    pub position: ReadSignal<ScrollPosition>,
    pub direction: Prop<Direction>,
}

#[derive(Clone, Default)]
pub struct ScrollbarStyle(Option<(f32, RenderFn<ScrollHandle, ListChild>)>);

impl ScrollbarStyle {
    pub fn new(spacing: f32, bar: impl Fn(ScrollHandle) -> ListChild + 'static) -> Self {
        Self(Some((spacing, RenderFn::new(bar))))
    }

    fn spacing(&self) -> f32 {
        self.0.as_ref().map_or(0.0, |(spacing, _)| *spacing)
    }

    fn beside(
        &self,
        position: ReadSignal<ScrollPosition>,
        direction: Prop<Direction>,
    ) -> Children<ListChild> {
        match &self.0 {
            None => Children::default(),
            Some((_, bar)) => Children::from(bar.call(ScrollHandle {
                position,
                direction,
            })),
        }
    }
}

#[component]
pub fn Scroll(
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = None)] reveal: Prop<Option<usize>>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = Color32::TRANSPARENT)] focus_color: Prop<Color32>,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    on_change: Callback<ScrollPosition>,
    children: Children<NodeId>,
) -> NodeId {
    let (position, set_position) = create_signal(ScrollPosition::ZERO);
    let across = across(&direction);
    let content_direction = direction.clone();
    view! {
        <List direction={across} spacing={scrollbar.spacing()}>
            <reactive::Scroll
                @sizing=ItemSize::Percent(100.0)
                offset
                reveal
                direction={content_direction}
                focus_color
                on_change={move |reported: ScrollPosition| {
                    set_position.set(reported);
                    on_change.call(reported);
                }}
            >
                {children}
            </reactive::Scroll>
            {scrollbar.beside(position, direction)}
        </List>
    }
}

#[component]
pub fn VirtualList(
    count: Prop<usize>,
    item_size: Prop<f32>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = Color32::TRANSPARENT)] focus_color: Prop<Color32>,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    on_change: Callback<ScrollPosition>,
    #[prop(children)] item: RenderFn<usize>,
) -> NodeId {
    let (position, set_position) = create_signal(ScrollPosition::ZERO);
    let across = across(&direction);
    let content_direction = direction.clone();
    view! {
        <List direction={across} spacing={scrollbar.spacing()}>
            <reactive::VirtualList
                @sizing=ItemSize::Percent(100.0)
                count
                item_size
                direction={content_direction}
                focus_color
                item={item}
                on_change={move |reported: ScrollPosition| {
                    set_position.set(reported);
                    on_change.call(reported);
                }}
            />
            {scrollbar.beside(position, direction)}
        </List>
    }
}

fn across(direction: &Prop<Direction>) -> Memo<Direction> {
    let direction = direction.clone();
    create_memo(move || match direction.get() {
        Direction::Horizontal => Direction::Vertical,
        Direction::Vertical => Direction::Horizontal,
    })
}

use beui_macros::{component, view};

use crate::base::{Direction, ItemSize, ScrollPosition};
use crate::color::Color32;
use crate::node::NodeId;
use crate::reactive::{Callback, Children, Memo, Prop, RenderFn, create_memo};
use crate::styled::Scrollbar;
use crate::styled::theme::{SCROLLBAR_SPACING, SCROLLBAR_WIDTH, use_theme};
use crate::unstyled;
use crate::unstyled::{ScrollHandle, ScrollbarStyle};

#[component]
pub fn Scroll(
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = None)] reveal: Prop<Option<usize>>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = None)] focus_color: Prop<Option<Color32>>,
    on_change: Callback<ScrollPosition>,
    children: Children<NodeId>,
) -> NodeId {
    let focus = focus_ring(focus_color);
    view! {
        <unstyled::Scroll
            offset
            reveal
            direction
            focus_color={focus}
            scrollbar={scrollbar_style()}
            on_change={move |reported: ScrollPosition| on_change.call(reported)}
        >
            {children}
        </unstyled::Scroll>
    }
}

#[component]
pub fn VirtualList(
    count: Prop<usize>,
    item_size: Prop<f32>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = None)] focus_color: Prop<Option<Color32>>,
    on_change: Callback<ScrollPosition>,
    #[prop(children)] item: RenderFn<usize>,
) -> NodeId {
    let focus = focus_ring(focus_color);
    view! {
        <unstyled::VirtualList
            count
            item_size
            direction
            focus_color={focus}
            scrollbar={scrollbar_style()}
            item={item}
            on_change={move |reported: ScrollPosition| on_change.call(reported)}
        />
    }
}

pub(crate) fn scrollbar_style() -> ScrollbarStyle {
    ScrollbarStyle::new(SCROLLBAR_SPACING, |handle: ScrollHandle| {
        let ScrollHandle {
            position,
            direction,
        } = handle;
        view! {
            <Scrollbar @sizing=ItemSize::Fixed(SCROLLBAR_WIDTH) position direction />
        }
    })
}

fn focus_ring(color: Prop<Option<Color32>>) -> Memo<Color32> {
    let theme = use_theme();
    create_memo(move || color.get().unwrap_or_else(|| theme.accent.get()))
}

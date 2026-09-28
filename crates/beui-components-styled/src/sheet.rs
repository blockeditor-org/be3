use std::cell::Cell;
use std::rc::Rc;

use beui_macros::{component, view};

use crate::theme::{BORDER_WIDTH, use_theme};
use beui_core::input::{CursorIcon, PointerPress};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, BackHandler, Child, ClickCallback, ClickCatcher, Direction, Frame, ItemSize, List, Prop,
    Spacer, clone, create_memo, create_signal,
};

pub const SHEET_STOPS: [f32; 3] = [0.3, 0.5, 0.9];

const CLOSE_BELOW: f32 = 0.18;
const HANDLE_HEIGHT: f32 = 24.0;
const GRIP_WIDTH: f32 = 36.0;
const GRIP_HEIGHT: f32 = 4.0;

#[component]
pub fn Sheet(
    extent: Prop<f32>,
    #[prop(default = true)] open: Prop<bool>,
    on_close: ClickCallback,
    children: Child,
) -> NodeId {
    let theme = use_theme();
    let extent = create_memo(move || extent.get().max(1.0));
    let (share, set_share) = create_signal(SHEET_STOPS[1]);
    let height = create_memo(clone!(extent share -> move || {
        Some((share.get() * extent.get()).max(HANDLE_HEIGHT))
    }));
    let grab: Rc<Cell<Option<(f32, f32)>>> = Rc::default();
    let pressed = clone!(grab share -> move |press: PointerPress| {
        grab.set(Some((press.pos.y, share.get_untracked())));
    });
    let dragged = clone!(grab extent set_share -> move |press: PointerPress| {
        let Some((from, start)) = grab.get() else {
            return;
        };
        let moved = (from - press.pos.y) / extent.get_untracked();
        set_share.set((start + moved).clamp(0.0, SHEET_STOPS[SHEET_STOPS.len() - 1]));
    });
    let cancelled = clone!(grab set_share -> move || {
        if let Some((_, start)) = grab.take() {
            set_share.set(start);
        }
    });
    let closing = on_close.clone();
    let released = clone!(grab share set_share -> move |active: bool| {
        if active || grab.take().is_none() {
            return;
        }
        let held = share.get_untracked();
        if held < CLOSE_BELOW {
            set_share.set(SHEET_STOPS[1]);
            closing.call();
            return;
        }
        let nearest = SHEET_STOPS
            .into_iter()
            .min_by(|a, b| (a - held).abs().total_cmp(&(b - held).abs()))
            .unwrap_or(SHEET_STOPS[1]);
        set_share.set(nearest);
    });
    view! {
        <BackHandler enabled={open} on_back={move || on_close.call()}>
            <Frame height={height} color={theme.surface.clone()}>
                <List spacing=0.0>
                    <Frame height=BORDER_WIDTH color={theme.border.clone()} />
                    <ClickCatcher
                        @test_id={"sheet.handle"}
                        cursor=CursorIcon::ResizeVertical
                        touch_drags=true
                        on_press={pressed}
                        on_drag={dragged}
                        on_cancel={cancelled}
                        on_active_change={released}
                    >
                        <Frame height=HANDLE_HEIGHT>
                            <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                                <Spacer @sizing=ItemSize::Percent(100.0) />
                                <Frame
                                    width=GRIP_WIDTH
                                    height=GRIP_HEIGHT
                                    radius=2
                                    color={theme.text_muted.clone()}
                                />
                                <Spacer @sizing=ItemSize::Percent(100.0) />
                            </List>
                        </Frame>
                    </ClickCatcher>
                    {children} @sizing=ItemSize::Percent(100.0)
                </List>
            </Frame>
        </BackHandler>
    }
}

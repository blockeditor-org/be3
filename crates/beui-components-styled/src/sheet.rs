use std::cell::Cell;
use std::rc::Rc;

use beui_macros::{component, view};

use crate::theme::{BORDER_WIDTH, use_theme};
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color32;
use beui_core::geometry::Pos2;
use beui_core::input::{CursorIcon, PointerPress};
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Align, BackHandler, Child, ClickCallback, Frame, Interactive, ItemSize, List, Prop, Spacer,
    clone, component_size, create_memo, create_signal,
};

pub const SHEET_STOPS: [f32; 3] = [0.3, 0.5, 0.9];

const CLOSE_BELOW: f32 = 0.18;
const FIT_CLOSE_DISTANCE: f32 = 64.0;
const HANDLE_HEIGHT: f32 = 24.0;
const GRIP_WIDTH: f32 = 36.0;
const GRIP_HEIGHT: f32 = 4.0;
const SCRIM: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 150);

#[component]
pub fn Sheet(
    extent: Prop<f32>,
    #[prop(default = true)] open: Prop<bool>,
    #[prop(default = SHEET_STOPS[1])] rest: f32,
    #[prop(default = SHEET_STOPS.to_vec())] stops: Vec<f32>,
    #[prop(default = false)] fit: bool,
    on_close: ClickCallback,
    children: Child,
) -> NodeId {
    let theme = use_theme();
    let stops: Rc<[f32]> = match stops.is_empty() {
        true => Rc::from(SHEET_STOPS.as_slice()),
        false => Rc::from(stops),
    };
    let highest = stops.iter().copied().fold(0.0_f32, f32::max);
    let extent = create_memo(move || extent.get().max(1.0));
    let (share, set_share) = create_signal(rest);
    let height = create_memo(clone!(extent share -> move || match fit {
        true => None,
        false => Some((share.get() * extent.get()).max(HANDLE_HEIGHT)),
    }));
    let grab: Rc<Cell<Option<(f32, f32)>>> = Rc::default();
    let pulled = Rc::new(Cell::new(0.0_f32));
    let pressed = clone!(grab share pulled -> move |press: PointerPress| {
        pulled.set(0.0);
        grab.set(Some((press.pos.y, share.get_untracked())));
    });
    let dragged = clone!(grab extent set_share pulled -> move |press: PointerPress| {
        let Some((from, start)) = grab.get() else {
            return;
        };
        pulled.set(press.pos.y - from);
        if fit {
            return;
        }
        let moved = (from - press.pos.y) / extent.get_untracked();
        set_share.set((start + moved).clamp(0.0, highest));
    });
    let cancelled = clone!(grab set_share -> move || {
        if let Some((_, start)) = grab.take() {
            set_share.set(start);
        }
    });
    let closing = on_close.clone();
    let released = clone!(grab share set_share stops -> move |active: bool| {
        if active || grab.take().is_none() {
            return;
        }
        if fit {
            if pulled.get() > FIT_CLOSE_DISTANCE {
                closing.call();
            }
            return;
        }
        let held = share.get_untracked();
        if held < CLOSE_BELOW {
            set_share.set(rest);
            closing.call();
            return;
        }
        let nearest = stops
            .iter()
            .copied()
            .min_by(|a, b| (a - held).abs().total_cmp(&(b - held).abs()))
            .unwrap_or(rest);
        set_share.set(nearest);
    });
    let body = match fit {
        true => ItemSize::Intrinsic,
        false => ItemSize::Percent(100.0),
    };
    view! {
        <BackHandler enabled={open} on_back={move || on_close.call()}>
            <Frame height={height} color={theme.surface.clone()}>
                <List spacing=0.0>
                    <Frame height=BORDER_WIDTH color={theme.border.clone()} />
                    <Interactive
                        @test_id={"sheet.handle"}
                        cursor=CursorIcon::ResizeVertical
                        touch_drags=true
                        on_press={pressed}
                        on_drag={dragged}
                        on_cancel={cancelled}
                        on_active_change={released}
                    >
                        <Frame
                            height=HANDLE_HEIGHT
                            align_horizontal=Align::Center
                            align_vertical=Align::Center
                        >
                            <Frame
                                width=GRIP_WIDTH
                                height=GRIP_HEIGHT
                                radius=2
                                color={theme.text_muted.clone()}
                            />
                        </Frame>
                    </Interactive>
                    {children} @sizing={body}
                </List>
            </Frame>
        </BackHandler>
    }
}

#[component]
pub fn ModalSheet(
    open: Prop<bool>,
    #[prop(default = SHEET_STOPS[2])] rest: f32,
    #[prop(default = SHEET_STOPS.to_vec())] stops: Vec<f32>,
    #[prop(default = false)] fit: bool,
    on_close: ClickCallback,
    children: Child,
) -> NodeId {
    let dismiss = on_close.clone();
    let shown = open.clone();
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            open={shown}
            placement=Placement::Fill
            scrim=SCRIM
            on_dismiss={move || dismiss.call()}
        >
            <ModalSheetBody open rest stops fit on_close={move || on_close.call()}>
                {children}
            </ModalSheetBody>
        </Overlay>
    }
}

#[component]
fn ModalSheetBody(
    open: Prop<bool>,
    rest: f32,
    stops: Vec<f32>,
    fit: bool,
    on_close: ClickCallback,
    children: Child,
) -> NodeId {
    let size = component_size();
    let extent = create_memo(move || size.get().y);
    let outside = on_close.clone();
    let closing = on_close;
    view! {
        <List spacing=0.0>
            <Interactive
                @sizing=ItemSize::Percent(100.0)
                @test_id={"sheet.outside"}
                on_click={move || outside.call()}
            >
                <Spacer />
            </Interactive>
            <Sheet extent open rest stops fit on_close={move || closing.call()}>{children}</Sheet>
        </List>
    }
}

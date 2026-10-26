use beui_view::types::{LayerLevel, LayerPolicy};
use std::any::Any;
use std::rc::Rc;

use beui_macros::{component, view};

use beui_core::drag_board::{DragPoint, Pending, Target};
use beui_core::input::TOUCH_DRAG_THRESHOLD;
use beui_core::tree::component_node;
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::CursorIcon;
use beui_view::types::Direction;
use beui_view::types::Placement;
use beui_view::types::PointerPress;
use beui_view::types::{Pos2, Vec2};

pub const DRAG_THRESHOLD: f32 = 4.0;
pub const DRAG_PREVIEW_OFFSET: Vec2 = Vec2::new(12.0, 14.0);
const TOUCH_PREVIEW_OFFSET: Vec2 = Vec2::new(16.0, -64.0);

#[derive(Clone, Copy)]
pub struct DragHandle {
    pub dragging: Memo<bool>,
}

#[derive(Clone, Copy)]
pub struct DropHandle {
    pub carrying: Memo<bool>,
    pub over: Memo<bool>,
    pub pointer: ReadSignal<Option<DragPoint>>,
}

#[component]
pub fn Draggable<P>(
    payload: Prop<Option<P>>,
    #[prop(default = DRAG_THRESHOLD)] threshold: f32,
    #[prop(default = CursorIcon::Default)] cursor: Prop<CursorIcon>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = false)] touch_drags: Prop<bool>,
    #[prop(default = None)] touch_drag_axis: Prop<Option<Direction>>,
    on_click: ClickCallback,
    on_drag_change: Callback<bool>,
    on_drop_outside: Callback<DragPoint>,
    preview: Option<RenderFn<P>>,
    #[prop(children)] content: Render<DragHandle>,
) -> NodeId
where
    P: Clone + PartialEq + 'static,
{
    let board = store(drag_board());
    let pressed_at = StoredCell::new(None::<Pos2>);
    let carried = create_signal(None::<P>);
    let pointer = create_signal(None::<DragPoint>);
    let touched = create_signal(false);
    let dragging = create_memo(move || carried.with(Option::is_some));

    let moved = move |point: DragPoint| {
        let Some(origin) = pressed_at.get() else {
            return;
        };
        let threshold = match touched.get_untracked() {
            true => threshold.max(TOUCH_DRAG_THRESHOLD),
            false => threshold,
        };
        if carried.with_untracked(Option::is_some) {
            board.value().track(point);
            return;
        }
        if (point.pos - origin).length() < threshold || board.value().carrying() {
            return;
        }
        let Some(payload) = payload.get_untracked() else {
            return;
        };
        carried.set(Some(payload.clone()));
        on_drag_change.call(true);
        board.value().begin(
            Rc::new(payload),
            point,
            Rc::new(move |point| pointer.set(Some(point))),
            Rc::new(move |point| on_drop_outside.call(point)),
        );
    };
    let moved: Pending = Rc::new(moved);
    let pending = Rc::clone(&moved);
    let pressed = move |press: PointerPress| {
        pressed_at.set(Some(press.pos));
        touched.set(press.touch);
        if !press.touch {
            board.value().press(Rc::clone(&pending));
        }
    };
    let dragged = move |press: PointerPress| moved(DragPoint::of(press));
    let clicked = move |_: PointerPress| {
        if carried.with_untracked(Option::is_none) {
            on_click.call();
        }
    };
    let released = move |active: bool| {
        if active {
            return;
        }
        pressed_at.set(None);
        let board = board.value();
        board.release();
        if carried.with_untracked(Option::is_none) {
            return;
        }
        board.finish();
        carried.set(None);
        on_drag_change.call(false);
    };
    on_cleanup(move || {
        if carried.with_untracked(Option::is_some) {
            board.value().end(None);
        }
    });

    let anchor = create_memo(move || {
        let offset = match touched.get() {
            true => TOUCH_PREVIEW_OFFSET,
            false => DRAG_PREVIEW_OFFSET,
        };
        pointer.get().map_or(Pos2::ZERO, |point| point.pos) + offset
    })
    .into_prop()
    .map(OverlayAnchor::Point);
    let shown_in = create_memo(move || pointer.get().map(|point| point.window));
    let face = content.call(DragHandle { dragging });
    let preview = preview.unwrap_or_else(|| {
        RenderFn::new(|_: P| {
            view! {
                <Frame />
            }
        })
    });
    view! {
        <Interactive
            cursor
            capture_presses
            touch_drags
            touch_drag_axis
            on_press={pressed}
            on_drag={dragged}
            on_click_at={clicked}
            on_active_change={released}
        >
            <List spacing=0.0>
                {face}
                <Overlay
                    anchor={anchor}
                    placement=Placement::BelowStart
                    policy=LayerPolicy::Inert
                    level=LayerLevel::Top
                    traps_focus=false
                    shown_in={shown_in}
                    open={dragging}
                >
                    <List spacing=0.0>
                        <Dynamic value={carried}>|carried: Option<P>| {
                            match carried {
                                Some(carried) => preview.call(carried),
                                None => view! {
                                    <Frame />
                                },
                            }
                        }</Dynamic>
                    </List>
                </Overlay>
            </List>
        </Interactive>
    }
}

#[component]
pub fn DropTarget<P>(
    #[prop(default = None)] accepts: Option<Func<P, bool>>,
    on_over: Callback<Option<(P, DragPoint)>>,
    on_drop: Callback<(P, DragPoint)>,
    #[prop(children)] content: Render<DropHandle>,
) -> NodeId
where
    P: Clone + 'static,
{
    let accepts = accepts.unwrap_or_else(|| Func::new(|_| true));
    let carrying = create_signal(false);
    let pointer = create_signal(None::<DragPoint>);
    let board = drag_board();
    let id = board.register(Target {
        id: 0,
        node: component_node(),
        accepts: Rc::new(move |payload: &dyn Any| {
            payload
                .downcast_ref::<P>()
                .is_some_and(|payload| accepts.call(payload.clone()))
        }),
        carrying: Rc::new(move |next| carrying.set(next)),
        over: Rc::new(move |over: Option<(&dyn Any, DragPoint)>| {
            let over = over.and_then(|(payload, point)| {
                payload
                    .downcast_ref::<P>()
                    .map(|payload| (payload.clone(), point))
            });
            pointer.set(over.as_ref().map(|(_, point)| *point));
            on_over.call(over);
        }),
        dropped: Rc::new(move |payload: &dyn Any, point| {
            if let Some(payload) = payload.downcast_ref::<P>() {
                on_drop.call((payload.clone(), point));
            }
        }),
    });
    on_cleanup(move || board.unregister(id));
    let carrying = create_memo(move || carrying.get());
    let over = create_memo(move || pointer.with(Option::is_some));
    let face = content.call(DropHandle {
        carrying,
        over,
        pointer: pointer.read_only(),
    });
    view! {
        <Frame>{face}</Frame>
    }
}

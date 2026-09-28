use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

use beui_macros::{component, view};

use beui_core::base::Direction;
use beui_core::base::overlay::{OverlayAnchor, OverlayMode, Placement};
use beui_core::drag_board::{Board, DragPoint, Pending, Target};
use beui_core::geometry::{Pos2, Vec2};
use beui_core::input::CursorIcon;
use beui_core::input::{PointerPress, TOUCH_DRAG_THRESHOLD};
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Callback, ClickCallback, ClickCatcher, Dynamic, Frame, Func, IntoProp, List, Memo, Prop,
    ReadSignal, Render, RenderFn, clone, component_rect, create_memo, create_signal, on_cleanup,
    provide_context, use_context, with_document,
};

pub const DRAG_THRESHOLD: f32 = 4.0;
pub const DRAG_PREVIEW_OFFSET: Vec2 = Vec2::new(12.0, 14.0);
const TOUCH_PREVIEW_OFFSET: Vec2 = Vec2::new(16.0, -64.0);

#[derive(Clone)]
pub struct DragHandle {
    pub dragging: Memo<bool>,
}

#[derive(Clone)]
pub struct DropHandle {
    pub carrying: Memo<bool>,
    pub over: Memo<bool>,
    pub pointer: ReadSignal<Option<DragPoint>>,
}

fn board() -> Rc<Board> {
    with_document(|document| document.drag_board())
}

#[derive(Clone, Copy)]
struct DropDepth(usize);

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
    preview: Option<RenderFn<P>>,
    #[prop(children)] content: Render<DragHandle>,
) -> NodeId
where
    P: Clone + PartialEq + 'static,
{
    let board = board();
    let pressed_at = Rc::new(Cell::new(None::<Pos2>));
    let (carried, set_carried) = create_signal(None::<P>);
    let (pointer, set_pointer) = create_signal(Pos2::ZERO);
    let (touched, set_touched) = create_signal(false);
    let dragging = create_memo(clone!(carried -> move || carried.with(Option::is_some)));

    let moved = clone!(board pressed_at carried set_carried set_pointer on_drag_change payload touched -> move |point: DragPoint| {
        let Some(origin) = pressed_at.get() else {
            return;
        };
        let threshold = match touched.get_untracked() {
            true => threshold.max(TOUCH_DRAG_THRESHOLD),
            false => threshold,
        };
        if carried.with_untracked(Option::is_some)
            || (point.pos - origin).length() < threshold
            || board.carrying()
        {
            return;
        }
        let Some(payload) = payload.peek() else {
            return;
        };
        let follow = set_pointer.clone();
        set_carried.set(Some(payload.clone()));
        on_drag_change.call(true);
        board.begin(
            Rc::new(payload),
            point,
            Rc::new(move |pos| follow.set(pos)),
        );
    });
    let moved: Pending = Rc::new(moved);
    let pressed = clone!(board pressed_at moved -> move |press: PointerPress| {
        pressed_at.set(Some(press.pos));
        set_touched.set(press.touch);
        if !press.touch {
            board.press(Rc::clone(&moved));
        }
    });
    let dragged = move |press: PointerPress| moved(DragPoint::of(press));
    let clicked = clone!(carried -> move |_: PointerPress| {
        if carried.with_untracked(Option::is_none) {
            on_click.call();
        }
    });
    let released = clone!(board pressed_at carried set_carried on_drag_change -> move |active: bool| {
        if active {
            return;
        }
        pressed_at.set(None);
        board.release();
        if carried.with_untracked(Option::is_none) {
            return;
        }
        board.finish();
        set_carried.set(None);
        on_drag_change.call(false);
    });
    on_cleanup(clone!(board carried -> move || {
        if carried.with_untracked(Option::is_some) {
            board.end(None);
        }
    }));

    let anchor = create_memo(clone!(pointer -> move || {
        let offset = match touched.get() {
            true => TOUCH_PREVIEW_OFFSET,
            false => DRAG_PREVIEW_OFFSET,
        };
        pointer.get() + offset
    }))
    .into_prop()
    .map(OverlayAnchor::Point);
    let face = content.call(DragHandle {
        dragging: dragging.clone(),
    });
    let preview = preview.unwrap_or_else(|| {
        RenderFn::new(|_: P| {
            view! {
                <Frame />
            }
        })
    });
    view! {
        <ClickCatcher
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
                    mode=OverlayMode::Passive
                    traps_focus=false
                    open={dragging}
                >
                    <List spacing=0.0>
                        <Dynamic value={carried}>
                            {move |carried: Option<P>| match carried {
                                Some(carried) => preview.call(carried),
                                None => view! {
                                    <Frame />
                                },
                            }}
                        </Dynamic>
                    </List>
                </Overlay>
            </List>
        </ClickCatcher>
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
    let depth = use_context::<DropDepth>().map_or(0, |DropDepth(depth)| depth + 1);
    provide_context(DropDepth(depth));
    let accepts = accepts.unwrap_or_else(|| Func::new(|_| true));
    let (carrying, set_carrying) = create_signal(false);
    let (pointer, set_pointer) = create_signal(None::<DragPoint>);
    let reported = on_over.clone();
    let board = board();
    let id = board.register(Target {
        id: 0,
        depth,
        rect: component_rect(),
        accepts: Rc::new(move |payload: &dyn Any| {
            payload
                .downcast_ref::<P>()
                .is_some_and(|payload| accepts.call(payload.clone()))
        }),
        carrying: Rc::new(move |carrying| set_carrying.set(carrying)),
        over: Rc::new(move |over: Option<(&dyn Any, DragPoint)>| {
            let over = over.and_then(|(payload, point)| {
                payload
                    .downcast_ref::<P>()
                    .map(|payload| (payload.clone(), point))
            });
            set_pointer.set(over.as_ref().map(|(_, point)| *point));
            reported.call(over);
        }),
        dropped: Rc::new(move |payload: &dyn Any, point| {
            if let Some(payload) = payload.downcast_ref::<P>() {
                on_drop.call((payload.clone(), point));
            }
        }),
    });
    on_cleanup(clone!(board -> move || board.unregister(id)));
    let carrying = create_memo(move || carrying.get());
    let over = create_memo(clone!(pointer -> move || pointer.with(Option::is_some)));
    let face = content.call(DropHandle {
        carrying,
        over,
        pointer,
    });
    view! {
        <Frame>{face}</Frame>
    }
}

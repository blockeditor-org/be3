use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_macros::{component, view};

use crate::CursorIcon;
use crate::base::overlay::{Overlay, OverlayAnchor, OverlayMode, Placement};
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::{Modifiers, PointerPress};
use crate::node::NodeId;
use crate::reactive::{
    Callback, ClickCallback, ClickCatcher, Dynamic, Frame, Func, IntoProp, List, Memo, Prop,
    ReadSignal, Render, RenderFn, clone, component_rect, create_memo, create_signal, on_cleanup,
    provide_context, use_context, with_document,
};

pub const DRAG_THRESHOLD: f32 = 4.0;
pub const DRAG_PREVIEW_OFFSET: Vec2 = Vec2::new(12.0, 14.0);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DragPoint {
    pub pos: Pos2,
    pub modifiers: Modifiers,
}

impl DragPoint {
    fn of(press: PointerPress) -> Self {
        Self {
            pos: press.pos,
            modifiers: press.modifiers,
        }
    }
}

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

type Accepts = Rc<dyn Fn(&dyn Any) -> bool>;
type Dropped = Rc<dyn Fn(&dyn Any, DragPoint)>;
type Over = Rc<dyn Fn(Option<(&dyn Any, DragPoint)>)>;
type Pending = Rc<dyn Fn(DragPoint)>;

struct Target {
    id: u64,
    depth: usize,
    rect: ReadSignal<Rect>,
    accepts: Accepts,
    carrying: Rc<dyn Fn(bool)>,
    over: Over,
    dropped: Dropped,
}

struct Carried {
    payload: Rc<dyn Any>,
    over: Option<u64>,
    last: Option<DragPoint>,
    follow: Rc<dyn Fn(Pos2)>,
}

#[derive(Default)]
pub(crate) struct Board {
    pressed: RefCell<Option<Pending>>,
    carried: RefCell<Option<Carried>>,
    targets: RefCell<Vec<Target>>,
    next: Cell<u64>,
}

impl Board {
    fn register(&self, mut target: Target) -> u64 {
        let id = self.next.get();
        self.next.set(id + 1);
        target.id = id;
        let carrying = self
            .carried
            .borrow()
            .as_ref()
            .is_some_and(|carried| (target.accepts)(carried.payload.as_ref()));
        let mark = Rc::clone(&target.carrying);
        self.targets.borrow_mut().push(target);
        if carrying {
            mark(true);
        }
        id
    }

    fn unregister(&self, id: u64) {
        self.targets.borrow_mut().retain(|target| target.id != id);
        if let Some(carried) = self.carried.borrow_mut().as_mut()
            && carried.over == Some(id)
        {
            carried.over = None;
        }
    }

    fn carrying(&self) -> bool {
        self.carried.borrow().is_some()
    }

    fn begin(&self, payload: Rc<dyn Any>, point: DragPoint, follow: Rc<dyn Fn(Pos2)>) {
        let marks = self
            .targets
            .borrow()
            .iter()
            .filter(|target| (target.accepts)(payload.as_ref()))
            .map(|target| Rc::clone(&target.carrying))
            .collect::<Vec<_>>();
        *self.carried.borrow_mut() = Some(Carried {
            payload,
            over: None,
            last: None,
            follow,
        });
        for mark in marks {
            mark(true);
        }
        self.track(point);
    }

    fn press(&self, pending: Pending) {
        *self.pressed.borrow_mut() = Some(pending);
    }

    fn release(&self) {
        self.pressed.borrow_mut().take();
    }

    pub(crate) fn track(&self, point: DragPoint) {
        if !self.carrying() {
            let pending = self.pressed.borrow().clone();
            if let Some(pending) = pending {
                pending(point);
            }
            return;
        }
        let follow = {
            let mut carried = self.carried.borrow_mut();
            let Some(carried) = carried.as_mut() else {
                return;
            };
            if carried.last == Some(point) {
                return;
            }
            carried.last = Some(point);
            Rc::clone(&carried.follow)
        };
        follow(point.pos);
        self.move_to(point);
    }

    fn finish(&self) {
        let last = self
            .carried
            .borrow()
            .as_ref()
            .and_then(|carried| carried.last);
        self.end(last);
    }

    fn under(&self, pos: Pos2) -> Option<(u64, Over)> {
        let carried = self.carried.borrow();
        let payload = carried.as_ref()?.payload.as_ref();
        self.targets
            .borrow()
            .iter()
            .filter(|target| target.rect.get_untracked().contains(pos))
            .filter(|target| (target.accepts)(payload))
            .max_by_key(|target| (target.depth, target.id))
            .map(|target| (target.id, Rc::clone(&target.over)))
    }

    fn over(&self, id: u64) -> Option<Over> {
        self.targets
            .borrow()
            .iter()
            .find(|target| target.id == id)
            .map(|target| Rc::clone(&target.over))
    }

    fn move_to(&self, point: DragPoint) {
        if !self.carrying() {
            return;
        }
        let under = self.under(point.pos);
        let previous = self.carried.borrow_mut().as_mut().and_then(|carried| {
            std::mem::replace(&mut carried.over, under.as_ref().map(|(id, _)| *id))
        });
        if previous.is_some()
            && previous != under.as_ref().map(|(id, _)| *id)
            && let Some(left) = previous.and_then(|id| self.over(id))
        {
            left(None);
        }
        let payload = self
            .carried
            .borrow()
            .as_ref()
            .map(|carried| Rc::clone(&carried.payload));
        if let (Some((_, entered)), Some(payload)) = (under, payload) {
            entered(Some((payload.as_ref(), point)));
        }
    }

    fn end(&self, point: Option<DragPoint>) {
        if let Some(point) = point {
            self.move_to(point);
        }
        let Some(carried) = self.carried.borrow_mut().take() else {
            return;
        };
        let targets = self
            .targets
            .borrow()
            .iter()
            .map(|target| {
                (
                    target.id,
                    Rc::clone(&target.carrying),
                    Rc::clone(&target.over),
                    Rc::clone(&target.dropped),
                )
            })
            .collect::<Vec<_>>();
        let mut drop = None;
        for (id, carrying, over, dropped) in targets {
            if carried.over == Some(id) {
                drop = Some(dropped);
            }
            over(None);
            carrying(false);
        }
        if let (Some(dropped), Some(point)) = (drop, point) {
            dropped(carried.payload.as_ref(), point);
        }
    }
}

impl Document {
    pub(crate) fn drag_board(&self) -> Rc<Board> {
        Rc::clone(&self.drags)
    }

    pub fn dragging(&self) -> bool {
        self.drags.carrying()
    }
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
    let dragging = create_memo(clone!(carried -> move || carried.with(Option::is_some)));

    let moved = clone!(board pressed_at carried set_carried set_pointer on_drag_change payload -> move |point: DragPoint| {
        let Some(origin) = pressed_at.get() else {
            return;
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
        board.press(Rc::clone(&moved));
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

    let anchor = create_memo(clone!(pointer -> move || pointer.get() + DRAG_PREVIEW_OFFSET))
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

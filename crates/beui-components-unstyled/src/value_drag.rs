//! How a value control (`NumberInput`'s scrub, `Slider`, the colour controls) tells its owner
//! about a drag. The control writes `value` on every move, and reports the drag around those
//! writes, so an owner can hold them until release: in a tentative gesture where it writes block
//! content (decisions/tentative_edits_wait_for_release.md), or with `HeldValue` where it cannot.

use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use beui_view::drag::{Carry, free};
use beui_view::reactive::*;
use beui_view::types::{PointerPress, Pos2, Rect, Vec2};

use crate::drag_board::drag_board;
use crate::drop_site::use_drop_site;

/// Where a value control's drag stands. `Started` comes before the drag's first write; `Kept`
/// or `Cancelled` once, after its last. A cancelled drag has written back the value it started
/// from first, and a control removed mid-drag reports `Cancelled` as it goes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueDrag {
    Started,
    Kept,
    Cancelled,
}

/// A control's side of a drag: reports `Started` once, then `Kept` or `Cancelled` once, and
/// `Cancelled` if the control's scope is disposed while it drags.
#[derive(Clone, Copy)]
pub(crate) struct DragReport {
    on_drag: Callback<ValueDrag>,
    dragging: StoredCell<bool>,
}

impl DragReport {
    pub(crate) fn new(on_drag: Callback<ValueDrag>) -> Self {
        let dragging = StoredCell::new(false);
        on_cleanup(move || {
            if dragging.get() {
                dragging.set(false);
                on_drag.call(ValueDrag::Cancelled);
            }
        });
        Self { on_drag, dragging }
    }

    pub(crate) fn dragging(&self) -> bool {
        self.dragging.get()
    }

    /// Reports `Started` unless the drag already started; whether this started it.
    pub(crate) fn start(&self) -> bool {
        if self.dragging.get() {
            return false;
        }
        self.dragging.set(true);
        self.on_drag.call(ValueDrag::Started);
        true
    }

    /// Reports the drag's end, if it is dragging.
    pub(crate) fn end(&self, kept: bool) {
        if !self.dragging.get() {
            return;
        }
        self.dragging.set(false);
        self.on_drag.call(match kept {
            true => ValueDrag::Kept,
            false => ValueDrag::Cancelled,
        });
    }
}

/// A control's thumb: its drag, reported as `ValueDrag`, and carried on the drag board. A press
/// `begin`s it (`Started`, before any write, a press's jump included); lifted then at a
/// mouse's or pen's press (a finger's past the threshold), its place is the point under the
/// pointer in the control's own node (`free`, unbounded), which `moved` writes as it goes,
/// given the press it was lifted by, moved there. A drop writes its place and is `Kept`; a
/// put-down (Escape, a cancelled press) writes back the start (`read` at the press, written
/// with `write`) and is `Cancelled`, as is a press `cancel`led or a control dropped mid-drag.
/// A press that lifts nothing (a finger's tap) ends with its `active(false)`. Its
/// `Interactive` sets `lifts_by_keys` off: a control's keys step it themselves.
#[derive(Clone, Copy)]
pub(crate) struct ThumbCarry {
    /// The node the thumb moves over: the control's `Interactive`.
    pub(crate) node: NodeRef,
    dragging: Signal<bool>,
    report: DragReport,
    payload: Thumb,
    pressed: StoredCell<Option<PointerPress>>,
    dropped: StoredCell<bool>,
    ends: Stored<Ends>,
}

/// What a thumb does at a drag's ends: whether it takes no drag now, records where it starts,
/// and writes that back.
struct Ends {
    disabled: Box<dyn Fn() -> bool>,
    record: Box<dyn Fn()>,
    restore: Box<dyn Fn()>,
}

/// What a thumb's carry holds: which control's it is.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Thumb(u64);

static THUMBS: AtomicU64 = AtomicU64::new(0);

impl ThumbCarry {
    pub(crate) fn new<T: Copy + 'static>(
        on_drag: Callback<ValueDrag>,
        disabled: impl Fn() -> bool + 'static,
        (read, write): (impl Fn() -> T + 'static, impl Fn(T) + 'static),
        moved: impl Fn(PointerPress) + 'static,
    ) -> Self {
        let started = StoredCell::new(None::<T>);
        let dragging = create_signal(false);
        let carry = Self {
            node: NodeRef::new(),
            dragging,
            report: DragReport::new(on_drag),
            payload: Thumb(THUMBS.fetch_add(1, Ordering::Relaxed)),
            pressed: StoredCell::new(None),
            dropped: StoredCell::new(false),
            ends: store(Ends {
                disabled: Box::new(disabled),
                record: Box::new(move || started.set(Some(read()))),
                restore: Box::new(move || {
                    if let Some(start) = started.get() {
                        write(start);
                    }
                }),
            }),
        };
        let moved = Rc::new(moved);
        // A place is the lifting press, moved to the point.
        let at = move |local: Pos2| {
            carry.pressed.get().map(|press| {
                let size = press.size.max(Vec2::splat(f32::MIN_POSITIVE));
                PointerPress {
                    local,
                    fraction: Vec2::new(local.x / size.x, local.y / size.y),
                    ..press
                }
            })
        };
        let site = use_drop_site(
            Func::new(move |thumb: Thumb| thumb == carry.payload),
            Func::new(move |_: Thumb| free(carry.node, |rect: Rect| rect.min).unbounded()),
            // The drop writes its place: the effect below may not have seen it yet.
            Callback::new({
                let moved = Rc::clone(&moved);
                move |(_, local): (Thumb, Pos2)| {
                    carry.dropped.set(true);
                    if let Some(press) = at(local) {
                        moved(press);
                    }
                    carry.end(true);
                }
            }),
        );
        create_effect_on(
            move || site.over.get(),
            move |over: &Option<Pos2>, _| {
                if let Some(press) = over.and_then(at) {
                    moved(press);
                }
            },
        );
        create_effect_on_change(
            move || drag_board().carries(&carry.payload),
            // Another carry's lift or put-down runs this too: only this one's end counts.
            move |held: &bool, was: &bool| {
                if *was && !*held && !carry.dropped.replace(false) {
                    carry.cancel();
                }
            },
        );
        carry
    }

    /// A press begins a drag, unless the control is disabled: `Started` and the start
    /// recorded, once.
    pub(crate) fn begin(self) {
        let Some(ends) = self.ends.try_value() else {
            return;
        };
        if (ends.disabled)() {
            return;
        }
        if self.report.start() {
            (ends.record)();
        }
        self.set_dragging(true);
    }

    /// Ends the drag called off, writing back where it started.
    pub(crate) fn cancel(self) {
        if self.report.dragging()
            && let Some(ends) = self.ends.try_value()
        {
            (ends.restore)();
        }
        self.end(false);
    }

    /// The press's own end: kept, unless the thumb is carried, whose end is the board's.
    pub(crate) fn active(self, active: bool) {
        match active {
            true => self.begin(),
            false if untrack(|| drag_board().carries(&self.payload)) => {}
            false => self.end(true),
        }
    }

    fn end(self, kept: bool) {
        self.set_dragging(false);
        self.report.end(kept);
    }

    fn set_dragging(self, next: bool) {
        if self.dragging.is_alive() && self.dragging.get_untracked() != next {
            self.dragging.set(next);
        }
    }

    /// Whether a drag is under way, from its press to its end.
    pub(crate) fn dragging(self) -> ReadSignal<bool> {
        self.dragging.read_only()
    }

    /// `Interactive`'s `drag`: the thumb's carry, for a press that began a drag.
    pub(crate) fn drag(self) -> Func<PointerPress, Option<Carry>> {
        Func::new(move |press: PointerPress| {
            if !self.report.dragging() {
                return None;
            }
            self.pressed.set(Some(press));
            self.dropped.set(false);
            Some(
                Carry::new(self.payload)
                    .item(Rect::ZERO)
                    .drawn_by_site()
                    .scrolls_nothing()
                    .reads_outside(),
            )
        })
    }
}

/// Holds what a value control writes during a drag outside `value`, for an owner whose writes
/// cannot wait in a tentative gesture (a graph definition, a library handing values to its
/// app): the held value shows through `value` here, and is written once when the drag is kept.
/// Pass `value` to the control and `on_drag` to its drag callback.
pub struct HeldValue<T: 'static> {
    pub value: Binding<T>,
    pub on_drag: Callback<ValueDrag>,
}

impl<T: 'static> Clone for HeldValue<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for HeldValue<T> {}

impl<T: Clone + PartialEq + 'static> HeldValue<T> {
    pub fn new(value: Binding<T>) -> Self {
        let held = create_signal(None::<T>);
        let dragging = StoredCell::new(false);
        let shown = create_memo(move || held.get().unwrap_or_else(|| value.get()));
        let on_drag = Callback::new(move |change: ValueDrag| match change {
            ValueDrag::Started => dragging.set(true),
            ValueDrag::Kept => {
                dragging.set(false);
                if let Some(kept) = held.get_untracked() {
                    value.set(kept);
                }
                held.set(None);
            }
            ValueDrag::Cancelled => {
                dragging.set(false);
                held.set(None);
            }
        });
        Self {
            value: Binding::new(shown, move |next: T| match dragging.get() {
                true => held.set(Some(next)),
                false => value.set(next),
            }),
            on_drag,
        }
    }
}

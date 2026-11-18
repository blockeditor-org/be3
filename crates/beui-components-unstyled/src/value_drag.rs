//! How a value control (`NumberInput`'s scrub, `Slider`, the colour controls) tells its owner
//! about a drag. The control writes `value` on every move, and reports the drag around those
//! writes, so an owner can hold them until release: in a tentative gesture where it writes block
//! content (decisions/tentative_edits_wait_for_release.md), or with `HeldValue` where it cannot.

use beui_view::reactive::*;

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

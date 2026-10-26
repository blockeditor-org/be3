use std::cell::{Cell, RefCell};

use beui::reactive::*;
use beui::{Rect, WindowPlacement};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShownDisplay {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) rect: Rect,
    pub(crate) scale: f32,
    pub(crate) placement: WindowPlacement,
}

thread_local! {
    static SHOWN: Cell<Option<Signal<Vec<ShownDisplay>>>> = const { Cell::new(None) };
    static PENDING: RefCell<Option<Vec<ShownDisplay>>> = const { RefCell::new(None) };
}

pub(crate) fn create_handle() {
    SHOWN.set(Some(create_signal(Vec::new())));
}

pub(crate) fn shown() -> Signal<Vec<ShownDisplay>> {
    SHOWN
        .get()
        .expect("the displays are created while the document is built")
}

#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
pub(crate) fn offer(displays: Vec<ShownDisplay>) {
    PENDING.with(|pending| *pending.borrow_mut() = Some(displays));
    crate::host::wake();
}

pub(crate) fn apply() {
    let Some(displays) = PENDING.with(|pending| pending.borrow_mut().take()) else {
        return;
    };
    let shown = shown();
    if shown.with_untracked(|held| *held != displays) {
        shown.set(displays);
    }
}

pub(crate) fn owner(displays: &[Rect], rect: Rect) -> Option<usize> {
    let area = |display: &Rect| {
        let shared = display.intersect(rect);
        shared.width().max(0.0) * shared.height().max(0.0)
    };
    displays
        .iter()
        .enumerate()
        .filter(|(_, display)| area(display) > 0.0)
        .max_by(|(_, left), (_, right)| area(left).total_cmp(&area(right)))
        .map(|(index, _)| index)
}

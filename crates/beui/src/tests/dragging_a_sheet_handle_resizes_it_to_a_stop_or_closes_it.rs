use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::input::BackGesture;
use crate::reactive::{Frame, List, build, view};
use crate::styled::Sheet;

#[test]
fn dragging_a_sheet_handle_resizes_it_to_a_stop_or_closes_it() {
    let sheet = NodeRef::new();
    let closed = Rc::new(Cell::new(0));
    let document = build({
        let (sheet, closed) = (sheet.clone(), closed.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Frame @sizing=ItemSize::Percent(100.0) />
                    <Sheet
                        @node_ref=&sheet
                        extent=600.0
                        on_close={move || closed.set(closed.get() + 1)}
                    >
                        <Frame />
                    </Sheet>
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.frame(Vec::new());
    let height = |harness: &Harness| {
        harness
            .document()
            .node_rect(sheet.get())
            .map(|rect| rect.height())
    };
    let handle = |harness: &Harness| {
        let id = harness
            .document()
            .find_test_id("sheet.handle")
            .expect("the handle");
        harness.center(id)
    };
    assert_eq!(
        height(&harness),
        Some(300.0),
        "a sheet opens at half the extent"
    );

    let from = handle(&harness);
    harness.drag(from, from - Vec2::new(0.0, 200.0));
    harness.settle();
    assert_eq!(
        height(&harness),
        Some(540.0),
        "dragged most of the way up, it snaps to the top stop"
    );

    let from = handle(&harness);
    harness.drag(from, from + Vec2::new(0.0, 40.0));
    harness.settle();
    assert_eq!(
        height(&harness),
        Some(540.0),
        "a small drag settles back on the nearest stop"
    );
    assert_eq!(closed.get(), 0);

    let from = handle(&harness);
    harness.drag(from, from + Vec2::new(0.0, 500.0));
    harness.settle();
    assert_eq!(
        closed.get(),
        1,
        "dragged nearly to the bottom, the sheet closes"
    );

    harness.frame(vec![Event::Back(BackGesture::Invoked)]);
    assert_eq!(closed.get(), 2, "going back closes an open sheet");
}

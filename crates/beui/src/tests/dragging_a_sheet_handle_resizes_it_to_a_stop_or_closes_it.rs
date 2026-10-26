use super::*;
use crate::input::BackGesture;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn dragging_a_sheet_handle_resizes_it_to_a_stop_or_closes_it() {
    let _root = crate::reactive::RootScope::new();
    let sheet = NodeRef::new();
    let closed = create_signal(0);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame @sizing=ItemSize::Percent(100.0) />
                <Sheet
                    @node_ref=&sheet
                    extent=600.0
                    on_close={move || closed.update(|closed| *closed += 1)}
                >
                    <Frame height=1000.0 />
                </Sheet>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.settle();
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
    assert_eq!(closed.get_untracked(), 0);

    let from = handle(&harness);
    harness.drag(from, from + Vec2::new(0.0, 500.0));
    harness.settle();
    assert_eq!(
        closed.get_untracked(),
        1,
        "dragged nearly to the bottom, the sheet closes"
    );

    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    assert_eq!(closed.get_untracked(), 2, "going back closes an open sheet");
}

use super::*;
use crate::primitives::Frame;
use crate::reactive::*;

/// The wheel steps a half-open sheet up to its top stop and then scrolls what it holds; scrolled
/// back, it stops at the top stop, and it never lowers or closes the sheet.
#[test]
fn a_wheel_raises_a_sheet_and_scrolls_what_it_holds_but_never_lowers_it() {
    let _root = RootScope::new();
    let sheet = NodeRef::new();
    let closed = create_signal(0);
    let document = build_node(move || {
        view! {
            <ModalSheet
                @node_ref=&sheet
                open=true
                rest=0.5
                on_close={move || closed.update(|n| *n += 1)}
            >
                <Frame height=2000.0 />
            </ModalSheet>
        }
    });
    let mut harness = Harness::sized(document, vec2(400.0, 600.0));
    harness.settle();
    let offset = |harness: &Harness| harness.document().scroll_offset(sheet.get());
    let wheel = |harness: &mut Harness, by: f32| {
        harness.scroll_at(pos2(200.0, 500.0), vec2(0.0, by), Modifiers::NONE);
        harness.settle();
    };
    assert_eq!(offset(&harness), 300.0, "it opens at its rest");

    wheel(&mut harness, 40.0);
    assert_eq!(offset(&harness), 300.0, "a wheel down short of the top stop leaves it");
    wheel(&mut harness, -40.0);
    assert_eq!(offset(&harness), 540.0, "a wheel up steps it to the next stop");
    wheel(&mut harness, -100.0);
    assert_eq!(offset(&harness), 640.0, "and at the top, scrolls what it holds");
    wheel(&mut harness, 60.0);
    assert_eq!(offset(&harness), 580.0);
    wheel(&mut harness, 200.0);
    assert_eq!(
        offset(&harness),
        540.0,
        "scrolled back past the start of what it holds, it stops at the top stop"
    );
    wheel(&mut harness, 200.0);
    assert_eq!(offset(&harness), 540.0, "and the wheel lowers it no further");
    assert_eq!(closed.get_untracked(), 0, "nor closes it");
}

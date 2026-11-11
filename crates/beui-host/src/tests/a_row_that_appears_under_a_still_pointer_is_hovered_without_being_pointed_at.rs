use super::*;
use crate::primitives::*;
use crate::reactive::*;

const ROW: f32 = 20.0;

struct Rows {
    _root: crate::reactive::RootScope,
    harness: Harness,
    shown: Signal<bool>,
    hovered: Signal<bool>,
    pointed: Signal<bool>,
}

/// A header row and, below it, a row that `shown` brings: what a menu does when it opens under
/// the pointer that pressed the button which opened it.
fn rows() -> Rows {
    let root = crate::reactive::RootScope::new();
    let shown = create_signal(false);
    let hovered = create_signal(false);
    let pointed = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height=ROW />
                <Show condition={shown}>
                    <Interactive on_hover_change={hovered} on_pointed_change={pointed}>
                        <Frame height=ROW />
                    </Interactive>
                </Show>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    Rows {
        _root: root,
        harness,
        shown,
        hovered,
        pointed,
    }
}

fn under_the_row() -> Pos2 {
    pos2(10.0, ROW + 5.0)
}

/// The row is hovered, as it is by anything under the pointer, but not pointed at: a pointer
/// that did not move has not chosen it, so a menu does not highlight it.
fn assert_hovered_not_pointed(rows: &Rows, what: &str) {
    assert!(rows.hovered.get_untracked(), "{what}: the row is hovered");
    assert!(
        !rows.pointed.get_untracked(),
        "{what}: the pointer did not move, so it is not pointed at"
    );
}

#[test]
fn a_row_that_appears_under_a_still_pointer_is_hovered_without_being_pointed_at() {
    // The pointer is there before the row, whichever way it got there.
    let mut rows = rows();
    rows.harness.frame(vec![Input::PointerMoved(under_the_row())]);
    rows.shown.set(true);
    rows.harness.frame(Vec::new());
    assert_hovered_not_pointed(&rows, "a row that appears under a pointer that moved there");

    // Moving it is what points at the row.
    rows.harness
        .frame(vec![Input::PointerMoved(under_the_row() + vec2(1.0, 0.0))]);
    assert!(rows.pointed.get_untracked(), "a pointer that moves points at it");
}

#[test]
fn the_order_a_pointer_rests_and_a_row_appears_in_does_not_decide_what_is_pointed_at() {
    // The host reports a pointer that stays where it is while what is under it changes as
    // `PointerRested`: before the row exists, once it does, and in the frame it appears in.
    let mut first = rows();
    first.harness.frame(vec![Input::PointerRested(under_the_row())]);
    first.shown.set(true);
    first.harness.frame(Vec::new());
    assert_hovered_not_pointed(&first, "the rest came before the row");

    let mut after = rows();
    after.shown.set(true);
    after.harness.frame(Vec::new());
    after.harness.frame(vec![Input::PointerRested(under_the_row())]);
    assert_hovered_not_pointed(&after, "the rest came after the row");

    let mut together = rows();
    together.shown.set(true);
    together
        .harness
        .frame(vec![Input::PointerRested(under_the_row())]);
    assert_hovered_not_pointed(&together, "the rest came in the frame the row appeared in");
}

use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

/// A sheet slides up by moving its content (`Frame::translate`), which places the rows where they
/// are drawn: a tap on a row while the sheet is still on its way lands on that row.
#[test]
fn a_tap_on_a_sheet_still_sliding_up_lands_on_the_row_drawn_under_it() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let tapped = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
                    <Frame />
                </ListItem>
                <ModalSheet open={open} fit=true on_close={move || open.set(false)}>
                    <Button label="Text" @test_id="row" on_click={move || tapped.set(true)} />
                </ModalSheet>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.frame(Vec::new());
    open.set(true);
    // The sheet is measured, then sets out: a few frames until the row is up on screen.
    for _ in 0..4 {
        harness.frame(Vec::new());
        if harness.document().find_test_id("row").is_some() {
            break;
        }
    }
    harness.frame(Vec::new());
    let sliding = harness.rect(harness.find("row"));

    harness.touch(TouchPhase::Start, sliding.center());
    harness.touch(TouchPhase::End, sliding.center());
    harness.settle();
    let resting = harness.rect(harness.find("row"));
    assert!(
        resting.top() < sliding.top() - 20.0,
        "the tap came while the sheet was still sliding up: at {} on its way to {}",
        sliding.top(),
        resting.top()
    );
    assert!(
        tapped.get_untracked(),
        "the row drawn under the tap took it"
    );
}

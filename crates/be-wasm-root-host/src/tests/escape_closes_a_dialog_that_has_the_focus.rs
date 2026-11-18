use be_lib_ui_types::{Key, Modifiers};
use be_sdk_root_testing::DIALOG_LAYER;

use super::*;

#[test]
fn escape_closes_a_dialog_that_has_the_focus() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);
    open_dialog(&mut root, &mut time);
    let closed = until(
        &mut root,
        &mut time,
        chord(Key::Escape, Modifiers::NONE),
        "Escape, which the dialog has while it has the focus, closes it",
        |done| layer_of(done, DIALOG_LAYER).is_none(),
    );
    assert!(closed.ime.is_none(), "the field went with the dialog");

    // Closed, Escape is not the root's: it opens nothing, and F2 opens a dialog afresh.
    root.input(
        None,
        be_lib_ui_types::input::encode(&chord(Key::Escape, Modifiers::NONE)),
    )
    .expect("the root takes input");
    for _ in 0..3 {
        time += 1;
        assert!(layer_of(&step(&mut root, time), DIALOG_LAYER).is_none());
    }
    open_dialog(&mut root, &mut time);
}

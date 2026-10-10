use super::*;
use crate::reactive::{List, Text, build, view};

#[test]
fn a_driver_resizes_a_headless_window_and_reads_its_state() {
    let document = build(|| {
        view! {
            <List spacing=0.0>
                <Text string="Hello" />
            </List>
        }
    });
    let mut driven = Driven::headless(document);

    driven
        .ask(&["resize", "200x100@2"])
        .expect("a headless window resizes");
    let tree = driven.ask(&["tree"]).expect("the tree is read");
    assert!(
        tree.starts_with("Window \"Driven\" focused at 0,0 size 400x200"),
        "the window is 200 by 100 points at twice the pixels:\n{tree}"
    );
    driven.ask(&["blur"]).expect("the window loses focus");
    let state = driven.ask(&["state"]).expect("the state is read");
    assert!(
        state.starts_with("window 200x100 at scale 2 (headless; windowed 200x100, screen 1000x600), fullscreen no, focused no"),
        "{state}"
    );
    driven.ask(&["clipboard", "copied"]).expect("the clipboard is set");
    assert_eq!(driven.ask(&["clipboard"]), Ok("copied\n".to_owned()));
}

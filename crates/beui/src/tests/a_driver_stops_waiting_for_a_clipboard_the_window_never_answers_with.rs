use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_driver_stops_waiting_for_a_clipboard_the_window_never_answers_with() {
    let document = build_node(|| {
        view! {
            <List spacing=0.0>
                <Text>"Hello"</Text>
            </List>
        }
    });
    let mut driven = stepped(crate::testing::Showing(document));

    let read = driven.ask(&["screen", "clipboard"]);
    assert!(
        read.as_ref()
            .is_err_and(|error| error.contains("did not answer with its clipboard")),
        "a window that never answers is given up on rather than holding every later command: {read:?}"
    );
    assert!(driven.ask(&["app", "tree"]).is_ok(), "the next command runs");
}

use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_driver_stops_waiting_for_a_clipboard_the_window_never_answers_with() {
    let document = build(|| {
        view! {
            <List spacing=0.0>
                <Text string="Hello" />
            </List>
        }
    });
    let mut driven = Driven::new(document);

    let read = driven.ask(&["clipboard"]);
    assert!(
        read.as_ref()
            .is_err_and(|error| error.contains("did not answer with its clipboard")),
        "a window that never answers is given up on rather than holding every later command: {read:?}"
    );
    assert!(driven.ask(&["tree"]).is_ok(), "the next command runs");
}

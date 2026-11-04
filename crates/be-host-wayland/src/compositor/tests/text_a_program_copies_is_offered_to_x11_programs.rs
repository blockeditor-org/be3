use super::*;
use crate::selection::client;

#[test]
fn text_a_program_copies_is_offered_to_x11_programs() {
    let mut harness = Harness::new();
    let control = harness.client.data_control();
    harness.settle();

    harness
        .client
        .copy_with_control(&control, "copied in a program");
    harness.settle();

    let offered = harness
        .compositor()
        .server
        .state
        .xwayland
        .offered(SelectionTarget::Clipboard)
        .map(<[String]>::to_vec);
    assert_eq!(offered, Some(vec![client::TEXT.to_owned()]));
    let pasted = paste_in_x11(&mut harness, SelectionTarget::Clipboard, client::TEXT);
    assert_eq!(pasted, "copied in a program");
}

use super::*;

#[test]
fn a_key_without_exec_offers_reconnect_when_connecting() {
    let mut boat = Boat::listing(SANDBOXES);
    boat.connect_terminal(Uuid::new_v4());
    let uname = boat.command();
    boat.answer(&uname.fetch, 403, FORBIDDEN);

    assert!(boat.test.shown("boat.reconnect"));
    assert!(
        boat.label("boat.key-notice").contains("exec and file.write"),
        "{}",
        boat.label("boat.key-notice")
    );
    assert!(
        boat.test
            .shown(&format!("boat.sandbox.{RUNNING}.terminal.retry"))
    );
}

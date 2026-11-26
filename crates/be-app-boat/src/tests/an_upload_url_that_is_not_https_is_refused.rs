use super::*;

#[test]
fn an_upload_url_that_is_not_https_is_refused() {
    let mut boat = Boat::listing(SANDBOXES);
    boat.connect_terminal(Uuid::new_v4());
    boat.ran("uname -m", UNAME);
    boat.answer_be(Boat::built_files());
    let held = boat.ran(
        "cd /home/user/.local/lib/be3/builds/tree ",
        &said("missing\n"),
    );
    assert!(held.contains("  terminal_types.wasm"));
    let asked = boat.fetch();
    boat.answer(&asked, 200, UPLOAD_URL_HTTP);
    assert!(!boat.test.has_fetches(), "nothing is sent to an http URL");
    assert!(
        boat.label(&format!("boat.sandbox.{RUNNING}.terminal.status"))
            .contains("not https")
    );
    assert!(
        !boat.test.shown("boat.reconnect"),
        "the key is not to blame"
    );
}

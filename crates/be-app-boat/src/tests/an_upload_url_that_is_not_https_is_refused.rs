use super::*;

#[test]
fn an_upload_url_that_is_not_https_is_refused() {
    let mut boat = Boat::listing(SANDBOXES);
    boat.connect_terminal(Uuid::new_v4());
    boat.ran("uname -m", UNAME);
    boat.answer_be(TerminalHostSource::Bytes(b"\x7fELF be".to_vec()));
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

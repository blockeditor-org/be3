use super::*;

#[test]
fn a_development_build_uploads_its_own_be() {
    let mut boat = Boat::listing(SANDBOXES);
    let block = boat.connect_terminal(Uuid::new_v4());
    boat.ran("uname -m", UNAME);
    boat.answer_be(TerminalHostSource::Bytes(b"\x7fELF be".to_vec()));

    let asked = boat.fetch();
    assert_eq!(
        asked.request.url,
        format!("https://boat.dev/api/v1/sandboxes/{RUNNING}/files/upload-url")
    );
    let body: serde_json::Value = serde_json::from_slice(&asked.request.body).unwrap();
    assert_eq!(body["path"], "/home/user/.local/lib/be3/builds/tree/be.part");
    boat.answer(&asked, 200, UPLOAD_URL);

    boat.test
        .settle_until("the upload", |test| test.has_fetches());
    let mut fetches = boat.test.take_fetches();
    let upload = fetches.remove(0);
    assert_eq!(upload.request.method, "PUT");
    assert!(upload.request.url.ends_with("token=signed-upload-token"));
    assert_eq!(
        upload.request.header_value("Authorization"),
        None,
        "the URL is the key"
    );
    assert_eq!(upload.request.body, b"\x7fELF be");
    boat.answer(&upload, 200, "{}");

    let checked = boat.ran("set -e", DONE);
    assert!(checked.contains("test -f be.part"));
    assert!(
        checked.contains(&format!(
            "echo '{}  be.part' | sha256sum -c",
            crate::root::sha256(b"\x7fELF be")
        )),
        "an uploaded be is checked before it runs: {checked}"
    );
    boat.write_answered("/home/user/.local/lib/be3/builds/tree/build-info.json");
    boat.install_answered(block, true);
}

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
    assert!(!boat.test.shown("boat.reconnect"), "the key is not to blame");
}

use super::*;

/// A development build's `be` goes up with the apps' manifests and type modules beside it,
/// which it reads blocks through, each checked against its SHA-256 before `be` can run.
#[test]
fn a_development_build_uploads_its_own_be() {
    let mut boat = Boat::listing(SANDBOXES);
    let block = boat.connect_terminal(Uuid::new_v4());
    boat.ran("uname -m", UNAME);
    boat.answer_be(Boat::built_files());
    let held = boat.ran(
        "cd /home/user/.local/lib/be3/builds/tree ",
        &said("missing\n"),
    );
    assert!(held.contains(&format!("{}  be", crate::root::sha256(b"\x7fELF be"))));
    assert!(held.contains("  terminal_types.wasm"));

    for (name, bytes) in [
        ("be", &b"\x7fELF be"[..]),
        ("terminal_types.wasm", &b"\0asm terminal"[..]),
    ] {
        let asked = boat.fetch();
        assert_eq!(
            asked.request.url,
            format!("https://boat.dev/api/v1/sandboxes/{RUNNING}/files/upload-url")
        );
        let body: serde_json::Value = serde_json::from_slice(&asked.request.body).unwrap();
        assert_eq!(
            body["path"],
            format!("/home/user/.local/lib/be3/builds/tree/{name}.part.{block}")
        );
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
        assert_eq!(upload.request.body, bytes);
        boat.answer(&upload, 200, "{}");
    }

    let checked = boat.ran("set -e", DONE);
    for (name, bytes) in [
        ("be", &b"\x7fELF be"[..]),
        ("terminal_types.wasm", &b"\0asm terminal"[..]),
    ] {
        assert!(checked.contains(&format!("test -f {name}.part.{block}")));
        assert!(
            checked.contains(&format!(
                "echo '{}  {name}.part.{block}' | sha256sum -c",
                crate::root::sha256(bytes)
            )),
            "an uploaded file is checked before be runs: {checked}"
        );
    }
    assert!(
        checked.ends_with(&format!("mv be.part.{block} be")),
        "be goes in place last: {checked}"
    );
    boat.write_answered("/home/user/.local/lib/be3/builds/tree/build-info.json");
    boat.install_answered(block, true);
}

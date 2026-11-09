use super::*;

/// Stops and starts the running sandbox as the list sees it.
fn resumed(boat: &mut Boat) {
    let stopped = SANDBOXES.replacen("\"state\": \"idle\"", "\"state\": \"archived\"", 1);
    boat.wait(Duration::from_secs(15));
    let list = boat.fetch();
    boat.answer(&list, 200, &stopped);
    boat.wait(Duration::from_secs(15));
    let list = boat.fetch();
    boat.answer(&list, 200, SANDBOXES);
}

#[test]
fn another_editors_token_or_another_build_is_put_right() {
    let mut boat = Boat::listing(SANDBOXES);
    let block = boat.connected(Uuid::new_v4());

    resumed(&mut boat);
    let theirs = crate::root::sha256(b"a token another editor rotated to");
    boat.checked(block, &said(&format!("{theirs}\ntree\n")));
    let credentials = boat.install_answered(block, false);
    assert_eq!(
        credentials["token"], "first-token",
        "the kept token is written again rather than rotated once more"
    );
    boat.test.settle();
    assert!(
        boat.test
            .take_requests_where(|call| match call {
                HostCall::RotateAgentToken { .. } => Ok(()),
                call => Err(call),
            })
            .is_empty()
    );

    resumed(&mut boat);
    let ours = crate::root::sha256(b"first-token");
    boat.checked(block, &said(&format!("{ours}\nolder-build\n")));
    assert_eq!(
        boat.label(&format!("boat.sandbox.{RUNNING}.terminal.status")),
        "Terminal offline, reconnecting…"
    );
    boat.ran("uname -m", UNAME);
    boat.answer_be(TerminalHostSource::Download(TerminalHostDownload {
        url: DEPLOYED_BE.to_owned(),
        sha256: BE_SHA256.to_owned(),
    }));
    boat.ran("set -e", DONE);
    boat.write_answered("/home/user/.local/lib/be3/builds/tree/build-info.json");
    let credentials = boat.install_answered(block, true);
    assert_eq!(credentials["token"], "first-token");
    assert_eq!(
        boat.label(&format!("boat.sandbox.{RUNNING}.terminal.status")),
        "Terminal"
    );
}

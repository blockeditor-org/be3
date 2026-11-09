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

fn rotations(boat: &mut Boat) -> usize {
    boat.test.settle();
    boat.test
        .take_requests_where(|call| match call {
            HostCall::RotateAgentToken { .. } => Ok(()),
            call => Err(call),
        })
        .len()
}

#[test]
fn a_running_terminal_is_left_alone_and_a_failing_one_is_put_right() {
    let mut boat = Boat::listing(SANDBOXES);
    let block = boat.connected(Uuid::new_v4());

    // Another device installed another build and token, and the host runs connected.
    resumed(&mut boat);
    boat.checked(block, &said("present\nanother-build\nconnected\nactive\n"));
    assert_eq!(rotations(&mut boat), 0);
    assert!(!boat.test.has_fetches(), "a working terminal is left alone");
    assert_eq!(
        boat.label(&format!("boat.sandbox.{RUNNING}.terminal.status")),
        "Terminal"
    );

    // The host runs but the server refuses its token: rotate, and install this build too.
    resumed(&mut boat);
    boat.checked(
        block,
        &said("present\nanother-build\noffline the server refused the token\nactive\n"),
    );
    let (reply, _) = boat.host_call("the token rotated", |call| match call {
        HostCall::RotateAgentToken { reply, agent } => Ok((reply, agent)),
        call => Err(call),
    });
    assert_eq!(
        boat.label(&format!("boat.sandbox.{RUNNING}.terminal.status")),
        "Terminal offline, reconnecting…"
    );
    boat.test.answer(reply, Ok("second-token".to_owned()));
    boat.test.run();
    boat.ran("uname -m", UNAME);
    boat.answer_be(TerminalHostSource::Download(TerminalHostDownload {
        url: DEPLOYED_BE.to_owned(),
        sha256: BE_SHA256.to_owned(),
    }));
    boat.ran("set -e", DONE);
    boat.write_answered("/home/user/.local/lib/be3/builds/tree/build-info.json");
    let credentials = boat.install_answered(block, true);
    assert_eq!(credentials["token"], "second-token");

    // A service that stopped trying (its start limit) is put right too.
    resumed(&mut boat);
    boat.checked(block, &said("present\ntree\nconnected\nfailed\n"));
    assert_eq!(rotations(&mut boat), 1);
}

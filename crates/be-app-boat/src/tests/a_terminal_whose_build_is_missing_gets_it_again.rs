use super::*;

#[test]
fn a_terminal_whose_build_is_missing_gets_it_again() {
    let mut boat = Boat::listing(SANDBOXES);
    let block = boat.connected(Uuid::new_v4());
    boat.resumed();
    // The link names this build, but its `be` is gone: the check says `none`.
    boat.checked(
        block,
        &said("present\nnone\nconnected\nfailed\n1800000000\n"),
    );
    let (reply, _) = boat.host_call("the token rotated", |call| match call {
        HostCall::RotateAgentToken { reply, agent } => Ok((reply, agent)),
        call => Err(call),
    });
    boat.test.answer(reply, Ok("second-token".to_owned()));
    boat.test.run();
    boat.ran("uname -m", UNAME);
    boat.answer_be(TerminalHostSource::Download(TerminalHostDownload {
        url: DEPLOYED_BE.to_owned(),
        sha256: BE_SHA256.to_owned(),
    }));
    let fetched = boat.ran("set -e", DONE);
    assert!(fetched.contains(&format!("curl -fsSL -o be.part.{block}")));
    boat.write_answered("/home/user/.local/lib/be3/builds/tree/build-info.json");
    boat.install_answered(block, true);
}

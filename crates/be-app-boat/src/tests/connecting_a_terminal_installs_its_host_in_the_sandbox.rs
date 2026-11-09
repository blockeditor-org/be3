use super::*;

use be_type_terminal::TerminalContent;

#[test]
fn connecting_a_terminal_installs_its_host_in_the_sandbox() {
    let mut boat = Boat::listing(SANDBOXES);
    assert!(
        !boat.test.shown("boat.sandbox.bx_r4t8w3ne.terminal.connect"),
        "a stopped sandbox runs no commands"
    );
    let agent = Uuid::new_v4();
    let block = boat.connect_terminal(agent);
    assert_eq!(
        boat.test.store().block(block).map(|info| info.block_type),
        Some(<TerminalContent as be_sdk_beui::be_block::BlockContent>::CONTENT_TYPE),
        "a terminal block was made"
    );
    assert!(
        boat.test
            .content::<BoatContent>(Some(boat.block))
            .root()
            .terminals_of(RUNNING)
            .contains(&block),
        "the block references it"
    );
    let kept = boat.kept_terminals();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].0, block.to_string());
    assert_eq!(kept[0].1.agent, agent.to_string());
    assert_eq!(kept[0].1.sandbox, RUNNING);
    assert!(kept[0].1.credentials.contains("abandon ability"));

    boat.ran("uname -m", UNAME);
    boat.test
        .snapshot("connecting_a_terminal_says_so_in_its_sandboxs_row");
    assert_eq!(
        boat.label(&format!("boat.sandbox.{RUNNING}.terminal.status")),
        "Connecting the terminal…"
    );
    boat.answer_be(TerminalHostSource::Download(TerminalHostDownload {
        url: DEPLOYED_BE.to_owned(),
        sha256: BE_SHA256.to_owned(),
    }));
    let download = boat.ran("set -e", DONE);
    assert!(download.contains("cd /home/user/.local/lib/be3/builds/tree"));
    assert!(download.contains(&format!(
        "curl -fsSL -o be.part.{block} '{DEPLOYED_BE}'"
    )));
    assert!(download.contains(&format!(
        "echo '{BE_SHA256}  be.part.{block}' | sha256sum -c"
    )));
    assert!(download.contains(&format!("mv be.part.{block} be")));
    assert!(
        download.contains(&format!("if echo '{BE_SHA256}  be' | sha256sum -c")),
        "a be already there is not fetched again"
    );
    let build_info = boat.write_answered("/home/user/.local/lib/be3/builds/tree/build-info.json");
    assert_eq!(build_info, BUILD_INFO);
    let credentials = boat.install_answered(block, true);
    assert_eq!(credentials["token"], "first-token");
    assert_eq!(credentials["boat_id"], RUNNING);
    assert_eq!(credentials["block"], block.to_string());
    assert_eq!(
        boat.label(&format!("boat.sandbox.{RUNNING}.terminal.status")),
        "Terminal"
    );
    assert!(!boat.test.has_fetches());

    boat.test
        .click(&format!("boat.sandbox.{RUNNING}.terminal.open"));
    boat.test.run();
    let opened = boat.test.take_opens();
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0].block_id, block);

    boat.test
        .click(&format!("boat.sandbox.{RUNNING}.terminal.disconnect"));
    boat.test.run();
    let (reply, removed) = boat.host_call("the agent removed", |call| match call {
        HostCall::RemoveAgent { reply, agent } => Ok((reply, agent)),
        call => Err(call),
    });
    assert_eq!(
        removed,
        agent.into_bytes(),
        "disconnecting removes the agent"
    );
    boat.test.answer(reply, Ok(()));
    let stop = boat.ran("sudo systemctl disable --now", DONE);
    assert!(stop.contains(&format!("be-terminal@{block}.service")));
    assert!(boat.kept_terminals().is_empty());
    assert!(
        boat.test
            .content::<BoatContent>(Some(boat.block))
            .root()
            .terminals
            .is_empty()
    );
    assert!(
        boat.test
            .shown(&format!("boat.sandbox.{RUNNING}.terminal.connect"))
    );
}

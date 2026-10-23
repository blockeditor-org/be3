use super::*;

use be_block::presence::{ChannelKind, PresenceKind};
use be_block::terminal::{TerminalContent, TerminalHost, TerminalMessage, TerminalSize};
use be_cli::terminal_host::{Host, HostOptions};

fn host_of(shared: &Shared, block: Uuid) -> Option<u64> {
    shared.presence.get(&block).and_then(|(_, peers)| {
        peers
            .iter()
            .find(|peer| peer.kind == TerminalHost::ID)
            .map(|peer| peer.client)
    })
}

fn heard(shared: &Shared, block: Uuid) -> Vec<TerminalMessage> {
    shared
        .peer_messages
        .get(&block)
        .into_iter()
        .flatten()
        .filter(|(_, message)| message.channel == TerminalMessage::ID)
        .filter_map(|(_, message)| TerminalMessage::decode(&message.payload))
        .collect()
}

fn output(shared: &Shared, block: Uuid) -> String {
    let bytes: Vec<u8> = heard(shared, block)
        .into_iter()
        .flat_map(|message| match message {
            TerminalMessage::Snapshot { screen, .. } => screen,
            TerminalMessage::Output { bytes, .. } => bytes,
            _ => Vec::new(),
        })
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[test]
fn a_terminal_host_answers_this_peer_through_the_blocks_session() {
    let harness = Harness::start();
    let block = Uuid::new_v4();
    let (ready, joined) = std::sync::mpsc::channel();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let config = harness.outside_config();
    let hosting = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("the host's runtime starts");
        runtime.block_on(async move {
            let peer = outside_peer(config).await;
            peer.ensure::<TerminalContent>(block, be_graph::BlockParent::Root)
                .await
                .expect("the terminal is created");
            let mut host = Host::new(HostOptions {
                command: vec!["sh".to_owned()],
                machine: "build-box".to_owned(),
                user: "agent".to_owned(),
            })
            .expect("the host starts");
            ready.send(()).expect("the test waits for the host");
            host.serve_until(peer, block, async {
                let _ = stopped.await;
            })
            .await
        })
    });
    joined.recv().expect("the host made the block");

    harness.connect();
    open(block, TerminalContent::CONTENT_TYPE);
    wait_until("saw the host in the block's session", |shared| {
        host_of(shared, block).is_some()
    });
    let host = with_shared(|shared| host_of(shared, block))
        .flatten()
        .expect("the host is there");

    let size = TerminalSize { cols: 60, rows: 12 };
    send_to_peers(
        block,
        Some(host),
        TerminalMessage::ID,
        TerminalMessage::Attach(size).encode(),
    );
    wait_until("was sent the host's screen", |shared| {
        heard(shared, block)
            .iter()
            .any(|message| matches!(message, TerminalMessage::Snapshot { size: sent, .. } if *sent == size))
    });

    send_to_peers(
        block,
        Some(host),
        TerminalMessage::ID,
        TerminalMessage::Input(b"echo from-the-app-$((40+2))\r".to_vec()).encode(),
    );
    wait_until("saw the shell run what it typed", |shared| {
        output(shared, block).contains("from-the-app-42")
    });

    stop.send(()).expect("the host is still serving");
    hosting
        .join()
        .expect("the host's thread ends")
        .expect("the host stops cleanly");
    wait_until("saw the host leave", |shared| host_of(shared, block).is_none());
}

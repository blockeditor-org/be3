use super::*;

use be_block::presence::{ChannelKind, PresenceKind};
use be_cli::terminal_host::{Host, HostOptions};
use terminal_types::{
    Key, Modifiers, TerminalContent, TerminalHost, TerminalKey, TerminalMessage, TerminalSize,
};

fn host_of(shared: &Shared, block: Uuid) -> Option<u64> {
    shared.presence.get(&block).and_then(|(_, peers)| {
        peers
            .iter()
            .find(|peer| peer.kind == TerminalHost::ID)
            .map(|peer| peer.client)
    })
}

fn terminal_text(shared: &Shared, block: Uuid) -> String {
    let Some(held) = shared.blocks.get(&block) else {
        return String::new();
    };
    let Ok(content) = TerminalContent::decode(held.bytes()) else {
        return String::new();
    };
    let root = content.root();
    root.history
        .iter()
        .chain(root.shown().iter())
        .map(|line| line.text.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

fn writer_of(shared: &Shared, block: Uuid) -> Option<u64> {
    let held = shared.blocks.get(&block)?;
    let content = TerminalContent::decode(held.bytes()).ok()?;
    content.root().host
}

fn send(block: Uuid, host: u64, message: TerminalMessage) {
    send_to_peers(block, Some(host), TerminalMessage::ID, message.encode());
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
        let local = tokio::task::LocalSet::new();
        local.block_on(&runtime, async move {
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
            let sources = super::super::types::sources().await;
            host.serve_until(peer, block, sources, async {
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

    wait_until("saw the host write the terminal", |shared| {
        writer_of(shared, block) == Some(host)
    });
    send(
        block,
        host,
        TerminalMessage::Resize(TerminalSize { cols: 60, rows: 12 }),
    );
    send(
        block,
        host,
        TerminalMessage::Paste("echo from-the-app-$((40+2))".to_owned()),
    );
    send(
        block,
        host,
        TerminalMessage::Key(TerminalKey {
            key: Key::Enter,
            modifiers: Modifiers::NONE,
            text: String::new(),
            pressed: true,
        }),
    );
    wait_until("read what the shell ran in the block's content", |shared| {
        terminal_text(shared, block)
            .lines()
            .any(|line| line.ends_with("from-the-app-42"))
    });

    stop.send(()).expect("the host is still serving");
    hosting
        .join()
        .expect("the host's thread ends")
        .expect("the host stops cleanly");
    wait_until("saw the host leave", |shared| {
        host_of(shared, block).is_none()
    });
}

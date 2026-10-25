use super::*;

use be_block::presence::ChannelKind;
use be_cli::terminal_host::{Host, HostOptions};
use terminal_types::{Key, Modifiers, TerminalContent, TerminalKey, TerminalMessage, TerminalSize};

fn writer_of(shared: &Shared, block: Uuid) -> Option<u64> {
    let held = shared.blocks.get(&block)?;
    let content = TerminalContent::decode(held.bytes()).ok()?;
    content.root().host
}

fn screen_of(shared: &Shared, block: Uuid) -> String {
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

#[test]
fn a_terminal_host_takes_over_a_session_this_peer_opened_first() {
    let harness = Harness::lapsing();
    let block = Uuid::new_v4();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts");
    runtime.block_on(async {
        harness
            .outside_peer()
            .await
            .ensure::<TerminalContent>(block, be_graph::BlockParent::Root)
            .await
            .expect("the terminal is created");
    });

    harness.connect();
    open(block, TerminalContent::CONTENT_TYPE);
    wait_until("opened the terminal", |shared| {
        shared.blocks.contains_key(&block)
    });

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
            let mut host = Host::new(HostOptions {
                command: vec!["sh".to_owned()],
                machine: "build-box".to_owned(),
                user: "agent".to_owned(),
            })
            .expect("the host starts");
            let sources = super::super::types::sources().await;
            host.serve_until(peer, block, sources, async {
                let _ = stopped.await;
            })
            .await
        })
    });

    wait_until("saw the host take the session and write", |shared| {
        writer_of(shared, block).is_some() && shared.owing == 0
    });
    let host = with_shared(|shared| writer_of(shared, block))
        .flatten()
        .expect("the host writes");
    for message in [
        TerminalMessage::Resize(TerminalSize { cols: 60, rows: 12 }),
        TerminalMessage::Paste("echo after-the-takeover-$((40+2))".to_owned()),
        TerminalMessage::Key(TerminalKey {
            key: Key::Enter,
            modifiers: Modifiers::NONE,
            text: String::new(),
            pressed: true,
        }),
    ] {
        send_to_peers(block, Some(host), TerminalMessage::ID, message.encode());
    }
    wait_until("kept seeing what the host writes", |shared| {
        let screen = screen_of(shared, block);
        screen
            .lines()
            .any(|line| line.ends_with("after-the-takeover-42"))
            && shared.owing == 0
    });

    stop.send(()).expect("the host is still serving");
    hosting
        .join()
        .expect("the host's thread ends")
        .expect("the host stops cleanly");
}

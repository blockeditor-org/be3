use std::sync::Arc;

use be_block::presence::{ChannelKind, PresenceKind};
use be_block::terminal::{TerminalContent, TerminalHost, TerminalMessage, TerminalSize};
use be_client::Live;
use be_graph::Access;
use ghostty_vt::{Renderer, Terminal};

use super::*;
use crate::session::connect_live;
use crate::terminal_host::{Host, HostOptions};

const PATIENCE: std::time::Duration = std::time::Duration::from_secs(30);
const SIZE: TerminalSize = TerminalSize { cols: 60, rows: 12 };

struct Viewer {
    live: Live<MemoryStore, TerminalContent>,
    screen: Terminal,
    renderer: Renderer,
    shell: Option<(u64, u64)>,
}

impl Viewer {
    async fn join(peer: &Arc<Peer<MemoryStore>>, block: Uuid) -> Self {
        Self {
            live: Live::join(Arc::clone(peer), block).await.unwrap(),
            screen: Terminal::new(SIZE.cols, SIZE.rows, 100).unwrap(),
            renderer: Renderer::new().unwrap(),
            shell: None,
        }
    }

    fn host(&self) -> Option<u64> {
        self.live
            .presence()
            .keys()
            .find(|(_, kind)| *kind == TerminalHost::ID)
            .map(|(client, _)| *client)
    }

    async fn send(&self, message: TerminalMessage) {
        let host = self.host().expect("a host is in the session");
        self.live
            .send_channel(Some(host), TerminalMessage::ID, message.encode())
            .await
            .unwrap();
    }

    fn text(&mut self) -> String {
        self.renderer.update(&mut self.screen).unwrap().text()
    }

    async fn until(&mut self, what: &str, ready: impl Fn(&mut Self) -> bool) {
        let reached = tokio::time::timeout(PATIENCE, async {
            loop {
                self.live.poll().await.unwrap();
                self.take();
                if ready(self) {
                    return;
                }
                self.live.wait().await.unwrap();
            }
        })
        .await;
        assert!(reached.is_ok(), "the viewer never {what}; it shows:\n{}", self.text());
    }

    fn take(&mut self) {
        for message in self.live.take_channel() {
            match TerminalMessage::decode(&message.payload) {
                Some(TerminalMessage::Snapshot {
                    shell,
                    sequence,
                    size,
                    screen,
                }) => {
                    self.screen.reset();
                    self.screen.resize(size.cols, size.rows, 1, 1).unwrap();
                    self.screen.write(&screen);
                    self.shell = Some((shell, sequence));
                }
                Some(TerminalMessage::Output {
                    shell,
                    sequence,
                    bytes,
                }) => {
                    if self.shell == Some((shell, sequence - 1)) {
                        self.screen.write(&bytes);
                        self.shell = Some((shell, sequence));
                    }
                }
                _ => {}
            }
        }
    }
}

#[tokio::test]
async fn a_terminal_host_runs_a_shell_for_its_viewers_and_repaints_it_for_one_that_returns() {
    let harness = Harness::start().await;
    let owner = Tracker::new(harness.owner().await);
    let block = Uuid::now_v7();
    owner
        .peer()
        .create_block(
            block,
            TerminalContent::CONTENT_TYPE,
            BlockParent::Root,
            &BlockMetadata::named("Shell"),
        )
        .await
        .unwrap();
    let (settings, account, public) = agent(&harness, &owner).await;
    owner
        .peer()
        .share(block, account, Access::Edit, &public)
        .await
        .unwrap();
    let agent = Tracker::new(connect_live(&settings).await.unwrap());
    let mut host = Host::new(HostOptions {
        command: vec!["sh".to_owned()],
        machine: "build-box".to_owned(),
        user: "agent".to_owned(),
    })
    .unwrap();
    let (stop, stopped) = oneshot::channel::<()>();
    let viewing = owner.shared_peer();

    let (served, ()) = tokio::join!(
        host.serve_until(agent.shared_peer(), block, async {
            let _ = stopped.await;
        }),
        async {
            let mut viewer = Viewer::join(&viewing, block).await;
            viewer
                .until("saw the host arrive", |viewer| viewer.host().is_some())
                .await;
            let shown = viewer
                .live
                .presence()
                .iter()
                .find(|((_, kind), _)| *kind == TerminalHost::ID)
                .map(|(_, value)| serde_json::from_slice::<TerminalHost>(value).unwrap());
            assert_eq!(shown.unwrap().machine, "build-box");

            viewer.send(TerminalMessage::Attach(SIZE)).await;
            viewer
                .until("was sent the screen", |viewer| viewer.shell.is_some())
                .await;
            viewer
                .send(TerminalMessage::Input(b"echo hello-$((6*7))\r".to_vec()))
                .await;
            viewer
                .until("saw the shell answer", |viewer| viewer.text().contains("hello-42"))
                .await;

            viewing.leave_session(block).await.unwrap();
            drop(viewer);
            let mut returned = Viewer::join(&viewing, block).await;
            returned
                .until("saw the host again", |viewer| viewer.host().is_some())
                .await;
            returned.send(TerminalMessage::Attach(SIZE)).await;
            returned
                .until("was repainted what the shell had shown", |viewer| {
                    viewer.text().contains("hello-42")
                })
                .await;

            stop.send(()).unwrap();
            returned
                .until("saw the host leave", |viewer| viewer.host().is_none())
                .await;
        }
    );
    served.unwrap();

    harness.stop().await;
}

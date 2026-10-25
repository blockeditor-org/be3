use std::sync::Arc;
use std::time::Duration;

use be_block::ObjectId;
use be_block::presence::{ChannelKind, PresenceKind};
use be_client::Peer;
use be_graph::Access;
use be_store::FileStore;
use terminal_types::{
    Key, Modifiers, Terminal, TerminalContent, TerminalHost, TerminalKey, TerminalLine,
    TerminalMessage, TerminalSize,
};

use super::*;
use crate::replica::Session;
use crate::session::connect_live;
use crate::terminal_host::{Host, HostOptions};

pub(super) const PATIENCE: Duration = Duration::from_secs(60);
pub(super) const SIZE: TerminalSize = TerminalSize { cols: 60, rows: 12 };

pub(super) struct Setup {
    pub harness: Harness,
    pub owner: Tracker<MemoryStore>,
    pub block: Uuid,
}

impl Setup {
    pub async fn start() -> Self {
        let harness = Harness::start().await;
        let owner = Tracker::new(harness.owner().await, sources());
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
        Self {
            harness,
            owner,
            block,
        }
    }

    pub async fn agent(&self) -> Arc<Peer<FileStore>> {
        let (settings, account, public) = agent(&self.harness, &self.owner).await;
        self.owner
            .peer()
            .share(self.block, account, Access::Edit, &public)
            .await
            .unwrap();
        Arc::new(connect_live(&settings).await.unwrap())
    }

    pub async fn viewer(&self) -> Viewer {
        Viewer {
            session: Session::open(self.owner.shared_peer(), self.block, sources()())
                .await
                .unwrap(),
        }
    }

    pub async fn stop(self) {
        self.harness.stop().await;
    }
}

pub(super) fn host(machine: &str) -> Host {
    Host::new(HostOptions {
        command: vec!["sh".to_owned()],
        machine: machine.to_owned(),
        user: "agent".to_owned(),
    })
    .unwrap()
}

pub(super) async fn serve(
    host: &mut Host,
    peer: Arc<Peer<FileStore>>,
    block: Uuid,
    stopped: oneshot::Receiver<()>,
) -> Result<(), String> {
    host.serve_until(peer, block, sources()(), async {
        let _ = stopped.await;
    })
    .await
}

pub(super) struct Viewer {
    pub session: Session<MemoryStore, TerminalContent>,
}

impl Viewer {
    pub fn root(&self) -> Terminal {
        self.session
            .content()
            .map(|content| content.root())
            .unwrap_or_default()
    }

    pub fn hosts_present(&self) -> usize {
        self.session
            .presence()
            .iter()
            .filter(|peer| peer.kind == TerminalHost::ID)
            .count()
    }

    fn shown(&self, host: u64) -> Option<&[u8]> {
        self.session
            .presence()
            .iter()
            .find(|peer| peer.client == host && peer.kind == TerminalHost::ID)
            .map(|peer| peer.value.as_slice())
    }

    pub fn host(&self) -> Option<u64> {
        let host = self
            .session
            .content()?
            .field(ObjectId::ROOT, Terminal::HOST)?;
        self.shown(host).map(|_| host)
    }

    pub fn machine(&self) -> Option<String> {
        let value = self.shown(self.host()?)?;
        serde_json::from_slice::<TerminalHost>(value)
            .ok()
            .map(|shown| shown.machine)
    }

    pub fn history(&self) -> Vec<TerminalLine> {
        self.root()
            .history
            .iter()
            .map(|line| (**line).clone())
            .collect()
    }

    pub fn text(&self) -> String {
        let root = self.root();
        root.history
            .iter()
            .chain(root.shown().iter())
            .map(|line| line.text.clone())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub async fn send(&mut self, message: TerminalMessage) {
        let host = self.host().expect("a host is writing the terminal");
        self.session
            .send(Some(host), TerminalMessage::ID, message.encode());
        self.session.settle().await;
    }

    pub fn answered(&self, answer: &str) -> bool {
        self.text()
            .lines()
            .any(|line| line.ends_with(answer) && !line.contains("echo"))
    }

    pub async fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.send(TerminalMessage::Key(TerminalKey {
            key,
            modifiers,
            text: String::new(),
            pressed: true,
        }))
        .await;
    }

    pub async fn run(&mut self, line: &str) {
        self.send(TerminalMessage::Paste(line.to_owned())).await;
        self.key(Key::Enter, Modifiers::NONE).await;
    }

    pub async fn until(&mut self, what: &str, ready: impl Fn(&Self) -> bool) {
        let reached = tokio::time::timeout(PATIENCE, async {
            loop {
                if ready(self) {
                    return;
                }
                let woken = self.session.next().await;
                self.session.handle(woken).await.unwrap();
            }
        })
        .await;
        assert!(
            reached.is_ok(),
            "the viewer never {what}; it is {:?} in {:?} with {:?}, and the terminal holds:\n{}",
            self.session.client(),
            self.session.state(),
            self.session.status(),
            self.text()
        );
    }

    pub async fn leave(self) {
        self.session.close().await;
    }
}

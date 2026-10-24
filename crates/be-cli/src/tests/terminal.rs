use std::sync::Arc;
use std::time::Duration;

use be_block::ObjectId;
use be_block::presence::{ChannelKind, PresenceKind};
use be_block::terminal::{
    Key, Modifiers, Terminal, TerminalContent, TerminalHost, TerminalKey, TerminalLine,
    TerminalMessage, TerminalSize,
};
use be_client::{Live, Peer};
use be_graph::Access;
use be_store::FileStore;

use super::*;
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
            live: Live::join(self.owner.shared_peer(), self.block)
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

pub(super) struct Viewer {
    pub live: Live<MemoryStore, TerminalContent>,
}

impl Viewer {
    pub fn root(&self) -> Terminal {
        self.live.content().root()
    }

    pub fn host(&self) -> Option<u64> {
        let host = self.live.content().field(ObjectId::ROOT, Terminal::HOST)?;
        self.live
            .presence()
            .contains_key(&(host, TerminalHost::ID))
            .then_some(host)
    }

    pub fn machine(&self) -> Option<String> {
        let host = self.host()?;
        let value = self.live.presence().get(&(host, TerminalHost::ID))?;
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

    pub async fn send(&self, message: TerminalMessage) {
        let host = self.host().expect("a host is writing the terminal");
        self.live
            .send_channel(Some(host), TerminalMessage::ID, message.encode())
            .await
            .unwrap();
    }

    pub fn answered(&self, answer: &str) -> bool {
        self.text()
            .lines()
            .any(|line| line.ends_with(answer) && !line.contains("echo"))
    }

    pub async fn key(&self, key: Key, modifiers: Modifiers) {
        self.send(TerminalMessage::Key(TerminalKey {
            key,
            modifiers,
            text: String::new(),
            pressed: true,
        }))
        .await;
    }

    pub async fn run(&self, line: &str) {
        self.send(TerminalMessage::Paste(line.to_owned())).await;
        self.key(Key::Enter, Modifiers::NONE).await;
    }

    pub async fn until(&mut self, what: &str, ready: impl Fn(&Self) -> bool) {
        let reached = tokio::time::timeout(PATIENCE, async {
            loop {
                self.live.poll().await.unwrap();
                if ready(self) {
                    return;
                }
                self.live.wait().await.unwrap();
            }
        })
        .await;
        assert!(
            reached.is_ok(),
            "the viewer never {what}; it is {} in {:?} at {:?}, and the terminal holds:\n{}",
            self.live.client(),
            self.live.state(),
            self.live.head(),
            self.text()
        );
    }

    pub async fn leave(self, setup: &Setup) {
        setup.owner.peer().leave_session(setup.block).await.unwrap();
    }
}

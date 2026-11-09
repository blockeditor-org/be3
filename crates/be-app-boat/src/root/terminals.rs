//! Terminals into sandboxes (guides/terminal.md, "A terminal host in a boat.dev sandbox"):
//! Connect makes a terminal block in Unfiled and an agent bound to it, and installs
//! `be terminal-host` in the sandbox as a `be-terminal@BLOCK` service; after a resume, which
//! leaves the sandbox without credentials, the agent's token is rotated and written again;
//! Disconnect removes the agent.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use be_sdk_beui::be_block::SettingsLayer;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::{BlockParent, BlockQuery, Failure, KeptEdit, TerminalHostSource};
use be_type_boat::{Boat, BoatSettings, TerminalAgent};
use be_type_terminal::TerminalContent;
use uuid::Uuid;

use super::state::State;
use crate::api::{self, Account, BoatError, Phase};

/// Where `be` and its `build-info.json` go in a sandbox.
pub(crate) const BE_DIRECTORY: &str = "/home/user/.local/lib/be3";
/// Where files wait before a command moves them where the files API can't write.
pub(crate) const STAGING: &str = "/home/user/.cache/be3";
/// The longest a command may run: a download of `be` takes the longest.
const COMMAND_TIMEOUT: u32 = 600;

/// How a terminal of the viewer's own stands in this editor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TerminalStatus {
    /// Asking the sandbox whether it still has the terminal's credentials.
    Checking,
    Connecting,
    /// The sandbox resumed without credentials; a new token is on its way to it.
    Reconnecting,
    Online,
    Failed(String),
}

/// Why connecting or reconnecting stopped.
#[derive(Debug)]
enum Trouble {
    Boat(BoatError),
    Host(String),
}

impl From<BoatError> for Trouble {
    fn from(error: BoatError) -> Self {
        Self::Boat(error)
    }
}

impl From<Failure> for Trouble {
    fn from(failure: Failure) -> Self {
        Self::Host(failure.to_string())
    }
}

/// Awaits a call answering `Option<Result<T, E>>`, ending the chain with its error.
macro_rules! step {
    ($call:expr) => {
        match $call.await? {
            Ok(value) => value,
            Err(error) => return Some(Err(Trouble::from(error))),
        }
    };
}

pub(crate) fn credentials_name(block: Uuid) -> String {
    format!("terminal-host-{block}.json")
}

/// The service that runs a terminal's host, one instance per terminal block, started by its
/// `.path` unit once the credentials are in `/run/be3`, which is in memory, so no snapshot of
/// the sandbox carries them. A key a gateway in front of the server wants goes in
/// `~/.config/be3/gateway` by hand, as `BE3_AUTHORIZATION=...`.
pub(crate) const SERVICE_UNIT: &str = "[Unit]
Description=be3 terminal host for block %i
After=network-online.target
Wants=network-online.target
ConditionPathExists=/run/be3/terminal-host-%i.json

[Service]
User=user
EnvironmentFile=-/home/user/.config/be3/gateway
PassEnvironment=BOAT_ID
ExecStart=/bin/sh -c 'BE3_AGENT_CREDENTIALS=\"$$(cat /run/be3/terminal-host-%i.json)\" exec /home/user/.local/lib/be3/be %i terminal-host'
Restart=on-failure
RestartSec=10
";

pub(crate) const PATH_UNIT: &str = "[Unit]
Description=be3 terminal host for block %i, once its credentials are in place

[Path]
PathExists=/run/be3/terminal-host-%i.json
Unit=be-terminal@%i.service

[Install]
WantedBy=paths.target
";

/// The command that moves the staged credentials (and, when connecting, the units) into
/// place and starts the terminal's service. The staged copy is deleted however it ends.
pub(crate) fn install_command(block: Uuid, units: bool) -> String {
    let name = credentials_name(block);
    let mut lines = vec![
        "set -e".to_owned(),
        format!("trap 'rm -f {STAGING}/{name}' EXIT"),
        "sudo install -d -m 0755 /run/be3".to_owned(),
        format!("sudo install -m 0600 -o user -g user {STAGING}/{name} /run/be3/{name}"),
    ];
    if units {
        lines.push(format!(
            "sudo install -m 0644 {STAGING}/be-terminal@.service {STAGING}/be-terminal@.path \
             /etc/systemd/system/"
        ));
        lines.push("sudo systemctl daemon-reload".to_owned());
        lines.push(format!("sudo systemctl enable --now be-terminal@{block}.path"));
    }
    lines.push(format!("sudo systemctl restart be-terminal@{block}.service"));
    lines.join("\n")
}

/// Whether the sandbox still has the terminal's credentials: `present` or `missing`.
pub(crate) fn check_command(block: Uuid) -> String {
    format!(
        "test -s /run/be3/{} && echo present || echo missing",
        credentials_name(block)
    )
}

/// Stops the terminal's service and deletes its credentials.
pub(crate) fn remove_command(block: Uuid) -> String {
    format!(
        "sudo systemctl disable --now be-terminal@{block}.path be-terminal@{block}.service; \
         sudo rm -f /run/be3/{}",
        credentials_name(block)
    )
}

/// Downloads a deployed `be` and checks its SHA-256 before it replaces the last one.
pub(crate) fn download_command(url: &str, sha256: &str) -> String {
    [
        "set -e".to_owned(),
        format!("mkdir -p {BE_DIRECTORY}"),
        format!("cd {BE_DIRECTORY}"),
        format!("curl -fsSL -o be.part '{url}'"),
        format!("echo '{sha256}  be.part' | sha256sum -c --quiet -"),
        "chmod 0755 be.part".to_owned(),
        "mv be.part be".to_owned(),
    ]
    .join("\n")
}

/// Makes an uploaded `be` the one the service runs.
pub(crate) fn uploaded_command() -> String {
    format!("cd {BE_DIRECTORY} && chmod 0755 be.part && mv be.part be")
}

/// The file the terminal host reads, as `BE3_AGENT_CREDENTIALS`: the agent's credentials with
/// the block and the sandbox it was installed for.
fn credentials_file(block: Uuid, terminal: &TerminalAgent) -> String {
    let mut credentials: serde_json::Value =
        serde_json::from_str(&terminal.credentials).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(object) = credentials.as_object_mut() {
        object.insert("block".to_owned(), block.to_string().into());
        object.insert("boat_id".to_owned(), terminal.sandbox.clone().into());
    }
    credentials.to_string()
}

/// The credentials with a rotated token.
fn with_token(credentials: &str, token: &str) -> String {
    let mut credentials: serde_json::Value =
        serde_json::from_str(credentials).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(object) = credentials.as_object_mut() {
        object.insert("token".to_owned(), token.into());
    }
    credentials.to_string()
}

/// `uname -m` as `terminal-host-binary` names it.
fn arch(machine: &str) -> String {
    match machine.trim() {
        "aarch64" | "arm64" => "arm64".to_owned(),
        machine => machine.to_owned(),
    }
}

/// Writes the credentials file and starts the service, with the units too when `units`.
async fn install_credentials(
    host: be_sdk_beui::EditorHost,
    account: Account,
    block: Uuid,
    terminal: TerminalAgent,
    units: bool,
) -> Option<Result<(), Trouble>> {
    let sandbox = terminal.sandbox.clone();
    if units {
        step!(api::write_file(
            &host,
            &account,
            &sandbox,
            &format!("{STAGING}/be-terminal@.service"),
            SERVICE_UNIT,
        ));
        step!(api::write_file(
            &host,
            &account,
            &sandbox,
            &format!("{STAGING}/be-terminal@.path"),
            PATH_UNIT,
        ));
    }
    step!(api::write_file(
        &host,
        &account,
        &sandbox,
        &format!("{STAGING}/{}", credentials_name(block)),
        &credentials_file(block, &terminal),
    ));
    step!(api::command(
        &host,
        &account,
        &sandbox,
        &install_command(block, units),
        COMMAND_TIMEOUT,
    ));
    Some(Ok(()))
}

/// Puts `be` of the host's own build in the sandbox, then the units and credentials.
async fn install(
    host: be_sdk_beui::EditorHost,
    account: Account,
    block: Uuid,
    terminal: TerminalAgent,
) -> Option<Result<(), Trouble>> {
    let sandbox = terminal.sandbox.clone();
    let machine = step!(api::command(&host, &account, &sandbox, "uname -m", 30));
    let be = step!(host.terminal_host_binary(arch(&machine)));
    match be.source {
        TerminalHostSource::Download(download) => {
            step!(api::command(
                &host,
                &account,
                &sandbox,
                &download_command(&download.url, &download.sha256),
                COMMAND_TIMEOUT,
            ));
        }
        TerminalHostSource::Bytes(bytes) => {
            step!(api::upload(
                &host,
                &account,
                &sandbox,
                &format!("{BE_DIRECTORY}/be.part"),
                bytes,
            ));
            step!(api::command(
                &host,
                &account,
                &sandbox,
                &uploaded_command(),
                COMMAND_TIMEOUT,
            ));
        }
    }
    step!(api::write_file(
        &host,
        &account,
        &sandbox,
        &format!("{BE_DIRECTORY}/build-info.json"),
        &be.build_info,
    ));
    install_credentials(host, account, block, terminal, true).await
}

/// A terminal's row: the viewer's own, or one shared with them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Shown {
    pub block: Uuid,
    pub own: bool,
}

impl State {
    /// The terminal the viewer sees for `sandbox`: their own first, else the first in the
    /// block's content they can open.
    pub(crate) fn terminal_of(&self, sandbox: &str) -> Option<Shown> {
        let own = self.own_terminals.with(|own| {
            own.iter()
                .find(|(_, terminal)| terminal.sandbox == sandbox)
                .map(|(block, _)| *block)
        });
        if let Some(block) = own {
            return Some(Shown { block, own: true });
        }
        let connecting = self
            .connecting
            .with(|connecting| connecting.get(sandbox).copied());
        if let Some(block) = connecting {
            return Some(Shown { block, own: true });
        }
        let visible = self.visible_terminals.get();
        self.content_terminals.with(|terminals| {
            terminals
                .iter()
                .filter(|(_, terminal)| terminal.sandbox == sandbox)
                .filter_map(|(block, _)| Uuid::parse_str(block).ok())
                .find(|block| visible.contains(block))
                .map(|block| Shown { block, own: false })
        })
    }

    /// The viewer's own terminals whose sandbox the list no longer shows.
    pub(crate) fn gone_terminals(&self) -> Vec<(Uuid, String)> {
        let Some(listed) = self.listed.get() else {
            return Vec::new();
        };
        if listed.cut_off {
            return Vec::new();
        }
        let organization = self.filters.with(|filters| filters.organization.clone());
        self.own_terminals.with(|own| {
            own.iter()
                .filter(|(_, terminal)| terminal.organization == organization)
                .filter(|(_, terminal)| {
                    !listed
                        .sandboxes
                        .iter()
                        .any(|sandbox| sandbox.id == terminal.sandbox)
                })
                .map(|(block, terminal)| (*block, terminal.sandbox.clone()))
                .collect()
        })
    }

    pub(super) fn watch_terminals(&self) {
        let state = *self;
        // A running sandbox the viewer has a terminal in is asked whether it still has the
        // credentials, once each time it is seen running: a resume leaves it without them.
        create_effect(move || {
            let Some(listed) = state.fetched.get() else {
                return;
            };
            let running: BTreeSet<String> = listed
                .sandboxes
                .iter()
                .filter(|sandbox| sandbox.phase() == Phase::Running)
                .map(|sandbox| sandbox.id.clone())
                .collect();
            let own = state.own_terminals.get();
            let organization = state.filters.with(|filters| filters.organization.clone());
            untrack(|| {
                state.terminal_status.update(|status| {
                    status.retain(|block, standing| {
                        matches!(standing, TerminalStatus::Connecting)
                            || own
                                .get(block)
                                .is_some_and(|terminal| running.contains(&terminal.sandbox))
                    });
                });
                if state.account().is_none() {
                    return;
                }
                let unchecked: Vec<Uuid> = own
                    .iter()
                    .filter(|(_, terminal)| {
                        terminal.organization == organization && running.contains(&terminal.sandbox)
                    })
                    .map(|(block, _)| *block)
                    .filter(|block| {
                        state
                            .terminal_status
                            .with(|status| !status.contains_key(block))
                    })
                    .collect();
                for block in unchecked {
                    state.check_terminal(block);
                }
            });
        });
    }

    fn set_status(&self, block: Uuid, status: Option<TerminalStatus>) {
        self.terminal_status.update(|statuses| match status {
            Some(status) => {
                statuses.insert(block, status);
            }
            None => {
                statuses.remove(&block);
            }
        });
    }

    /// Ends a chain: online, or why not, with a key lacking `exec` or `file.write` offering
    /// Reconnect.
    fn finished(&self, block: Uuid, answer: Result<(), Trouble>) {
        let status = match answer {
            Ok(()) => TerminalStatus::Online,
            Err(Trouble::Boat(BoatError::Refused { status: 403, .. })) => {
                self.lacking.set(true);
                TerminalStatus::Failed(
                    "This boat.dev key can't run commands or write files in sandboxes".to_owned(),
                )
            }
            Err(Trouble::Boat(error @ BoatError::Reconnect { .. })) => {
                let message = error.to_string();
                self.failure.set(Some(error));
                TerminalStatus::Failed(message)
            }
            Err(Trouble::Boat(error)) => TerminalStatus::Failed(error.to_string()),
            Err(Trouble::Host(message)) => TerminalStatus::Failed(message),
        };
        self.set_status(block, Some(status));
    }

    fn keep_terminal(&self, block: Uuid, terminal: Option<TerminalAgent>) -> bool {
        let edit: KeptEdit<BoatSettings> =
            Rc::new(move |_| BoatSettings::set_terminal(block, terminal.as_ref()));
        self.settings.write_all(SettingsLayer::EveryDevice, &[edit])
    }

    /// Makes a terminal block in Unfiled, references it from the block when the viewer may
    /// edit it, makes its agent and installs its host in `sandbox`.
    pub(crate) fn connect_terminal(&self, sandbox: String, name: String) {
        let Some(account) = untrack(|| self.account()) else {
            return;
        };
        self.notice.set(None);
        let block = self.editor.blocks().create_named(
            &TerminalContent::default(),
            BlockParent::Unfiled,
            format!("Terminal: {name}"),
        );
        self.connecting.update(|connecting| {
            connecting.insert(sandbox.clone(), block);
        });
        self.set_status(block, Some(TerminalStatus::Connecting));
        if !self.editor.read_only().get_untracked() {
            self.content.operate(Boat::add_terminal(block, sandbox.clone()));
        }
        let state = *self;
        let organization = self.filters.get_untracked().organization;
        let made = self
            .editor
            .host()
            .create_block_agent(block, format!("boat VM {sandbox}"));
        self.editor.then(made, move |made| {
            state.connecting.update(|connecting| {
                connecting.remove(&sandbox);
            });
            let made = match made {
                Ok(made) => made,
                Err(failure) => {
                    state.set_status(block, None);
                    if !state.editor.read_only().get_untracked() {
                        state.content.operate(Boat::remove_terminal(block));
                    }
                    state
                        .notice
                        .set(Some(format!("The terminal could not be made: {failure}")));
                    return;
                }
            };
            let terminal = TerminalAgent {
                agent: Uuid::from_bytes(made.agent).to_string(),
                credentials: made.credentials,
                sandbox,
                organization,
            };
            state.keep_terminal(block, Some(terminal.clone()));
            state.install_terminal(block, terminal, account);
        });
    }

    fn install_terminal(&self, block: Uuid, terminal: TerminalAgent, account: Account) {
        self.set_status(block, Some(TerminalStatus::Connecting));
        let state = *self;
        let installing = install(self.editor.host(), account, block, terminal);
        self.editor
            .then(installing, move |answer| state.finished(block, answer));
    }

    /// Connects again a terminal whose connecting failed.
    pub(crate) fn retry_terminal(&self, block: Uuid) {
        let Some(account) = untrack(|| self.account()) else {
            return;
        };
        let Some(terminal) = self
            .own_terminals
            .with_untracked(|own| own.get(&block).cloned())
        else {
            return;
        };
        self.install_terminal(block, terminal, account);
    }

    /// Asks the sandbox whether it has the terminal's credentials, and writes them again with
    /// a rotated token when it doesn't.
    fn check_terminal(&self, block: Uuid) {
        let Some(account) = untrack(|| self.account()) else {
            return;
        };
        let Some(terminal) = self
            .own_terminals
            .with_untracked(|own| own.get(&block).cloned())
        else {
            return;
        };
        self.set_status(block, Some(TerminalStatus::Checking));
        let state = *self;
        let asked = api::command(
            &self.editor.host(),
            &account,
            &terminal.sandbox,
            &check_command(block),
            30,
        );
        self.editor.then(asked, move |answer| match answer {
            Ok(said) if said.trim() == "present" => {
                state.set_status(block, Some(TerminalStatus::Online));
            }
            Ok(_) => state.reconnect_terminal(block, terminal, account),
            Err(error) => state.finished(block, Err(Trouble::Boat(error))),
        });
    }

    /// Rotates the agent's token, which revokes the one the sandbox had, keeps the new
    /// credentials and writes them to the sandbox.
    fn reconnect_terminal(&self, block: Uuid, terminal: TerminalAgent, account: Account) {
        self.set_status(block, Some(TerminalStatus::Reconnecting));
        let Ok(agent) = Uuid::parse_str(&terminal.agent) else {
            return self.finished(
                block,
                Err(Trouble::Host("the terminal's agent is unknown".to_owned())),
            );
        };
        let state = *self;
        let rotated = self.editor.host().rotate_agent_token(agent);
        self.editor.then(rotated, move |rotated| {
            let token = match rotated {
                Ok(token) => token,
                Err(failure) => return state.finished(block, Err(failure.into())),
            };
            let terminal = TerminalAgent {
                credentials: with_token(&terminal.credentials, &token),
                ..terminal
            };
            state.keep_terminal(block, Some(terminal.clone()));
            let writing = install_credentials(state.editor.host(), account, block, terminal, false);
            state
                .editor
                .then(writing, move |answer| state.finished(block, answer));
        });
    }

    /// Removes the terminal's agent at once, stops its host in the sandbox while it runs, and
    /// forgets the terminal; the block stays in the viewer's Unfiled.
    pub(crate) fn disconnect_terminal(&self, block: Uuid) {
        let Some(terminal) = self
            .own_terminals
            .with_untracked(|own| own.get(&block).cloned())
        else {
            return;
        };
        let state = *self;
        if let Ok(agent) = Uuid::parse_str(&terminal.agent) {
            let removed = self.editor.host().remove_agent(agent);
            self.editor.then(removed, move |removed| {
                if let Err(failure) = removed {
                    state.notice.set(Some(format!(
                        "The terminal's agent could not be removed: {failure}"
                    )));
                }
            });
        }
        let running = self.listed.with_untracked(|listed| {
            listed.as_ref().is_some_and(|listed| {
                listed
                    .sandboxes
                    .iter()
                    .any(|sandbox| sandbox.id == terminal.sandbox && self.phase(sandbox) == Phase::Running)
            })
        });
        if running && let Some(account) = untrack(|| self.account()) {
            let stopping = api::command(
                &self.editor.host(),
                &account,
                &terminal.sandbox,
                &remove_command(block),
                30,
            );
            self.editor.then(stopping, |_| {});
        }
        let referenced = self
            .content_terminals
            .with_untracked(|terminals| terminals.contains_key(&block.to_string()));
        if referenced && !self.editor.read_only().get_untracked() {
            self.content.operate(Boat::remove_terminal(block));
        }
        self.keep_terminal(block, None);
        self.set_status(block, None);
    }

    pub(crate) fn open_terminal(&self, block: Uuid) {
        self.editor
            .host()
            .open_block(block, <TerminalContent as be_sdk_beui::be_block::BlockContent>::CONTENT_TYPE);
    }
}

/// The viewer's own terminals, from their owner-only settings.
pub(super) fn own_terminals(
    effective: Memo<BoatSettings>,
) -> Memo<BTreeMap<Uuid, TerminalAgent>> {
    create_memo(move || {
        effective.with(|settings| {
            settings
                .terminals
                .iter()
                .filter_map(|(block, terminal)| {
                    Some((Uuid::parse_str(block).ok()?, terminal.clone()))
                })
                .collect()
        })
    })
}

/// The terminals the block references that the viewer can open.
pub(super) fn visible_terminals(editor: be_sdk_beui::Editor) -> Memo<BTreeSet<Uuid>> {
    let references = editor.watch_blocks(BlockQuery::References(editor.block_id()));
    create_memo(move || {
        references
            .get()
            .map(|blocks| {
                blocks
                    .into_iter()
                    .filter(|block| !block.no_access())
                    .map(|block| block.id)
                    .collect()
            })
            .unwrap_or_default()
    })
}

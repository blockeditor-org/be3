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

/// Where `be` goes in a sandbox: `builds/BUILD/` holds each build's `be` and its
/// `build-info.json`, and `blocks/BLOCK` links the build a terminal runs, so a terminal
/// connected from another build of be3 never replaces the `be` another one runs.
pub(crate) const BE_DIRECTORY: &str = "/home/user/.local/lib/be3";
/// Where files wait before a command moves them where the files API can't write.
pub(crate) const STAGING: &str = "/home/user/.cache/be3";
/// The longest a command may run: a download of `be` takes the longest.
const COMMAND_TIMEOUT: u32 = 600;

/// How a terminal of the viewer's own stands in this editor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TerminalStatus {
    /// Asking the sandbox whether it still has the terminal's credentials and build.
    Checking,
    Connecting,
    /// The sandbox lacks the credentials, a token this editor kept, or this build's `be`;
    /// they are on their way to it.
    Reconnecting,
    Online,
    Failed(String),
    /// Disconnecting could not remove the terminal's agent, so the terminal is kept.
    NotRemoved(String),
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
/// `~/.config/be3/gateway` by hand, as `BE3_AUTHORIZATION=...`. A host whose token was
/// revoked stops after a few tries rather than retrying for ever.
pub(crate) const SERVICE_UNIT: &str = "[Unit]
Description=be3 terminal host for block %i
After=network-online.target
Wants=network-online.target
ConditionPathExists=/run/be3/terminal-host-%i.json
StartLimitIntervalSec=600
StartLimitBurst=5

[Service]
User=user
EnvironmentFile=-/home/user/.config/be3/gateway
PassEnvironment=BOAT_ID
Environment=BE3_TERMINAL_STATUS=/run/be3/terminal-host-%i.status
ExecStart=/bin/sh -c 'BE3_AGENT_CREDENTIALS=\"$$(cat /run/be3/terminal-host-%i.json)\" exec /home/user/.local/lib/be3/blocks/%i/be %i terminal-host'
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

/// The SHA-256 of `bytes` in lower-case hex.
pub(crate) fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A build id from `build-info.json`, if it is one that is safe in a path and a command.
fn build_of(build_info: &str) -> Option<String> {
    let id = serde_json::from_str::<serde_json::Value>(build_info)
        .ok()?
        .get("id")?
        .as_str()?
        .to_owned();
    let safe = !id.is_empty()
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._+-".contains(character))
        && !id.starts_with('.');
    safe.then_some(id)
}

/// Deletes every build in the sandbox that no terminal's link names.
fn prune_command() -> String {
    format!(
        "used=$(for link in {BE_DIRECTORY}/blocks/*; do test -L \"$link\" && basename \"$(readlink \"$link\")\"; done); \
         for build in {BE_DIRECTORY}/builds/*; do test -d \"$build\" || continue; \
         printf '%s\\n' \"$used\" | grep -qxF \"$(basename \"$build\")\" || rm -rf \"$build\"; done"
    )
}

/// The command that moves the staged credentials into place, links the build the terminal
/// runs (pruning builds no terminal runs) and, when connecting, installs the units, and
/// starts the terminal's service. The staged copy is deleted however it ends.
pub(crate) fn install_command(block: Uuid, build: Option<&str>, units: bool) -> String {
    let name = credentials_name(block);
    let mut lines = vec![
        "set -e".to_owned(),
        format!("trap 'rm -f {STAGING}/{name}' EXIT"),
        "sudo install -d -m 0700 -o user -g user /run/be3".to_owned(),
        format!("sudo install -m 0600 -o user -g user {STAGING}/{name} /run/be3/{name}"),
        format!("rm -f /run/be3/terminal-host-{block}.status"),
    ];
    if let Some(build) = build {
        lines.push(format!("mkdir -p {BE_DIRECTORY}/blocks"));
        lines.push(format!(
            "ln -sfn ../builds/{build} {BE_DIRECTORY}/blocks/{block}"
        ));
        lines.push(format!("{} || true", prune_command()));
    }
    if units {
        lines.push(format!(
            "sudo install -m 0644 {STAGING}/be-terminal@.service {STAGING}/be-terminal@.path \
             /etc/systemd/system/"
        ));
        lines.push("sudo systemctl daemon-reload".to_owned());
        lines.push(format!(
            "sudo systemctl enable --now be-terminal@{block}.path"
        ));
    }
    lines.push(format!(
        "sudo systemctl reset-failed be-terminal@{block}.service || true"
    ));
    lines.push(format!(
        "sudo systemctl restart be-terminal@{block}.service"
    ));
    lines.join("\n")
}

/// Deletes credentials left staged on the sandbox's disk when installing them stopped.
pub(crate) fn unstage_command(block: Uuid) -> String {
    format!("rm -f {STAGING}/{}", credentials_name(block))
}

/// What the sandbox holds of a terminal, a line each: `present` or `missing` credentials, the
/// build its link names, the standing the host last wrote (`connected`, `offline ...`, or
/// `none`), and the service's state (`systemctl is-active`). Deletes credentials left staged
/// first.
pub(crate) fn check_command(block: Uuid) -> String {
    let name = credentials_name(block);
    [
        unstage_command(block),
        format!("test -s /run/be3/{name} && echo present || echo missing"),
        format!("basename \"$(readlink {BE_DIRECTORY}/blocks/{block})\""),
        format!("head -n 1 /run/be3/terminal-host-{block}.status 2>/dev/null || echo none"),
        format!("systemctl is-active be-terminal@{block}.service || true"),
    ]
    .join("\n")
}

/// What `check_command` answered.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Held {
    pub credentials: bool,
    pub build: String,
    /// What the host last wrote of itself.
    pub standing: String,
    /// The service's `ActiveState`.
    pub active: String,
}

impl Held {
    pub(crate) fn parse(said: &str) -> Self {
        let mut lines = said.lines().map(str::trim);
        let mut next = || lines.next().unwrap_or_default().to_owned();
        Self {
            credentials: next() == "present",
            build: next(),
            standing: next(),
            active: next(),
        }
    }

    /// Whether the host runs and hasn't said it lost the server: such a terminal is left
    /// alone, whatever token or build another device would have written.
    pub(crate) fn healthy(&self) -> bool {
        self.credentials && self.active == "active" && !self.standing.starts_with("offline")
    }
}

/// Stops the terminal's service and deletes its credentials and link.
pub(crate) fn remove_command(block: Uuid) -> String {
    format!(
        "sudo systemctl disable --now be-terminal@{block}.path be-terminal@{block}.service; \
         sudo rm -f /run/be3/{name} /run/be3/terminal-host-{block}.status; \
         rm -f {STAGING}/{name} {BE_DIRECTORY}/blocks/{block}; {}",
        prune_command(),
        name = credentials_name(block)
    )
}

/// Puts `be` of `build` in place unless it is there: downloaded from `url`, or uploaded
/// already, and checked against `sha256` either way before it can run.
pub(crate) fn be_command(build: &str, url: Option<&str>, sha256: &str) -> Result<String, String> {
    if url.is_some_and(|url| !url.starts_with("https://") || url.contains(['\'', '\\'])) {
        return Err("the host named a download of be that is not a plain https URL".to_owned());
    }
    if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("the host named no SHA-256 for be".to_owned());
    }
    let fetch = match url {
        Some(url) => format!("curl -fsSL -o be.part '{url}'"),
        None => "test -f be.part".to_owned(),
    };
    Ok([
        "set -e".to_owned(),
        format!("mkdir -p {BE_DIRECTORY}/builds/{build}"),
        format!("cd {BE_DIRECTORY}/builds/{build}"),
        fetch,
        format!("echo '{sha256}  be.part' | sha256sum -c --quiet -"),
        "chmod 0755 be.part".to_owned(),
        "mv be.part be".to_owned(),
    ]
    .join("\n"))
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

/// Writes the credentials file and starts the service, with the units too when `units`, and
/// the link to `build` when given. Credentials left staged are deleted when it stops early.
async fn install_credentials(
    host: be_sdk_beui::EditorHost,
    account: Account,
    block: Uuid,
    terminal: TerminalAgent,
    build: Option<String>,
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
    let written = api::write_file(
        &host,
        &account,
        &sandbox,
        &format!("{STAGING}/{}", credentials_name(block)),
        &credentials_file(block, &terminal),
    )
    .await?;
    let installed = match written {
        Ok(()) => {
            api::command(
                &host,
                &account,
                &sandbox,
                &install_command(block, build.as_deref(), units),
                COMMAND_TIMEOUT,
            )
            .await?
        }
        Err(error) => Err(error),
    };
    if let Err(error) = installed {
        // A write that failed may still have landed; nothing secret stays on the disk.
        let _ = api::command(&host, &account, &sandbox, &unstage_command(block), 30).await;
        return Some(Err(error.into()));
    }
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
    let Some(build) = build_of(&be.build_info) else {
        return Some(Err(Trouble::Host(
            "the host named no build for be".to_owned(),
        )));
    };
    let command = match be.source {
        TerminalHostSource::Download(download) => {
            be_command(&build, Some(&download.url), &download.sha256)
        }
        TerminalHostSource::Bytes(bytes) => {
            let hash = sha256(&bytes);
            step!(api::upload(
                &host,
                &account,
                &sandbox,
                &format!("{BE_DIRECTORY}/builds/{build}/be.part"),
                bytes,
            ));
            be_command(&build, None, &hash)
        }
    };
    let command = match command {
        Ok(command) => command,
        Err(message) => return Some(Err(Trouble::Host(message))),
    };
    step!(api::command(
        &host,
        &account,
        &sandbox,
        &command,
        COMMAND_TIMEOUT
    ));
    step!(api::write_file(
        &host,
        &account,
        &sandbox,
        &format!("{BE_DIRECTORY}/builds/{build}/build-info.json"),
        &be.build_info,
    ));
    install_credentials(host, account, block, terminal, Some(build), true).await
}

/// What the sandbox holds of a terminal, and the host's own build.
async fn check(
    host: be_sdk_beui::EditorHost,
    account: Account,
    block: Uuid,
    sandbox: String,
) -> Option<Result<(Held, String), Trouble>> {
    let said = step!(api::command(
        &host,
        &account,
        &sandbox,
        &check_command(block),
        30
    ));
    let build_info = step!(host.terminal_host_build());
    Some(Ok((
        Held::parse(&said),
        build_of(&build_info).unwrap_or_default(),
    )))
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
        // A filtered list hides sandboxes that still run; only an unfiltered one says a
        // sandbox is gone.
        let filtered = self
            .filters
            .with(|filters| !filters.name_prefix.is_empty() || !filters.label.trim().is_empty());
        if listed.cut_off || filtered || listed.filters != self.filters.get() {
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
                        matches!(
                            standing,
                            TerminalStatus::Connecting | TerminalStatus::NotRemoved(_)
                        ) || own
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

    /// Ends a chain: online, or why not. boat.dev's API refusing a command or a file (403)
    /// means the key lacks `exec` or `file.write`, which Reconnect fixes.
    fn finished(&self, block: Uuid, answer: Result<(), Trouble>) {
        if !self.owns(block) {
            return self.set_status(block, None);
        }
        let status = match answer {
            Ok(()) => TerminalStatus::Online,
            Err(Trouble::Boat(BoatError::Refused { status: 403, .. })) => {
                self.lacking.set(true);
                TerminalStatus::Failed(
                    "boat.dev refused to run a command or write a file in the sandbox with this \
                     key"
                    .to_owned(),
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

    /// Whether the viewer still has the terminal: an answer for one disconnected meanwhile is
    /// dropped.
    fn owns(&self, block: Uuid) -> bool {
        self.own_terminals
            .with_untracked(|own| own.contains_key(&block))
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
        if !self.read_only.get_untracked() {
            self.content
                .operate(Boat::add_terminal(block, sandbox.clone()));
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
                    state.unmake(block);
                    state
                        .notice
                        .set(Some(format!("The terminal could not be made: {failure}")));
                    return;
                }
            };
            let agent = Uuid::from_bytes(made.agent);
            let terminal = TerminalAgent {
                agent: agent.to_string(),
                credentials: made.credentials,
                sandbox,
                organization,
            };
            if !state.keep_terminal(block, Some(terminal.clone())) {
                // Nothing would keep the agent's credentials, so nothing could disconnect it.
                let removed = state.editor.host().remove_agent(agent);
                state.editor.then(removed, |_| {});
                state.unmake(block);
                state.notice.set(Some(
                    "The terminal could not be made: your settings have not loaded yet".to_owned(),
                ));
                return;
            }
            state.install_terminal(block, terminal, account);
        });
    }

    /// Takes back a terminal block whose connecting never got as far as an agent.
    fn unmake(&self, block: Uuid) {
        self.set_status(block, None);
        let referenced = self
            .content_terminals
            .with_untracked(|terminals| terminals.contains_key(&block.to_string()));
        if referenced && !self.read_only.get_untracked() {
            self.content.operate(Boat::remove_terminal(block));
        }
        self.editor.host().delete_block(
            block,
            <TerminalContent as be_sdk_beui::be_block::BlockContent>::CONTENT_TYPE,
            BlockParent::Unfiled,
        );
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

    /// Asks the sandbox what it holds of the terminal and how its host stands. A host that
    /// runs connected is left alone; otherwise (a resume left no credentials, the token was
    /// revoked, the server refuses the build) the token is rotated and written again, with
    /// this build's `be` when the sandbox runs another.
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
        let asked = check(
            self.editor.host(),
            account.clone(),
            block,
            terminal.sandbox.clone(),
        );
        self.editor.then(asked, move |answer| {
            if !state.owns(block) {
                return;
            }
            let (held, build) = match answer {
                Ok(answer) => answer,
                Err(trouble) => return state.finished(block, Err(trouble)),
            };
            if held.healthy() {
                return state.set_status(block, Some(TerminalStatus::Online));
            }
            // Only a host that isn't running, or has lost the server, is put right: a new
            // token (the old one may be revoked) and, when it runs another build, this one's.
            state.reconnect_terminal(block, terminal, account, held.build != build);
        });
    }

    /// Writes the kept credentials to the sandbox again, with this build's `be` too when
    /// `refresh`.
    fn rewrite_terminal(
        &self,
        block: Uuid,
        terminal: TerminalAgent,
        account: Account,
        refresh: bool,
    ) {
        self.set_status(block, Some(TerminalStatus::Reconnecting));
        let state = *self;
        match refresh {
            true => {
                let writing = install(self.editor.host(), account, block, terminal);
                self.editor
                    .then(writing, move |answer| state.finished(block, answer));
            }
            false => {
                let writing =
                    install_credentials(self.editor.host(), account, block, terminal, None, false);
                self.editor
                    .then(writing, move |answer| state.finished(block, answer));
            }
        }
    }

    /// Rotates the agent's token, which revokes the one the sandbox had, keeps the new
    /// credentials and writes them to the sandbox.
    fn reconnect_terminal(
        &self,
        block: Uuid,
        terminal: TerminalAgent,
        account: Account,
        refresh: bool,
    ) {
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
            if !state.owns(block) {
                // Disconnected meanwhile: its agent is going, and nothing brings it back.
                return;
            }
            let terminal = TerminalAgent {
                credentials: with_token(&terminal.credentials, &token),
                ..terminal
            };
            if !state.keep_terminal(block, Some(terminal.clone())) {
                return state.finished(
                    block,
                    Err(Trouble::Host(
                        "your settings have not loaded yet".to_owned(),
                    )),
                );
            }
            state.rewrite_terminal(block, terminal, account, refresh);
        });
    }

    /// Stops the terminal's host in the sandbox while it runs and removes its agent; once the
    /// agent is gone the terminal is forgotten, and the block stays in the viewer's Unfiled.
    /// When the agent can't be removed the terminal is kept, to disconnect again.
    pub(crate) fn disconnect_terminal(&self, block: Uuid) {
        let Some(terminal) = self
            .own_terminals
            .with_untracked(|own| own.get(&block).cloned())
        else {
            return;
        };
        let running = self.listed.with_untracked(|listed| {
            listed.as_ref().is_some_and(|listed| {
                listed.sandboxes.iter().any(|sandbox| {
                    sandbox.id == terminal.sandbox && self.phase(sandbox) == Phase::Running
                })
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
        let Ok(agent) = Uuid::parse_str(&terminal.agent) else {
            return self.forget_terminal(block);
        };
        let state = *self;
        let removed = self.editor.host().remove_agent(agent);
        self.editor.then(removed, move |removed| match removed {
            Ok(()) => state.forget_terminal(block),
            // The server no longer has the agent (its block was collected, say).
            Err(Failure::Failed(message)) if message.contains("not an agent") => {
                state.forget_terminal(block);
            }
            Err(failure) => {
                let message = format!("The terminal's agent could not be removed: {failure}");
                state.notice.set(Some(message.clone()));
                state.set_status(block, Some(TerminalStatus::NotRemoved(message)));
            }
        });
    }

    fn forget_terminal(&self, block: Uuid) {
        let referenced = self
            .content_terminals
            .with_untracked(|terminals| terminals.contains_key(&block.to_string()));
        if referenced && !self.read_only.get_untracked() {
            self.content.operate(Boat::remove_terminal(block));
        }
        self.keep_terminal(block, None);
        self.set_status(block, None);
    }

    pub(crate) fn open_terminal(&self, block: Uuid) {
        self.editor.host().open_block(
            block,
            <TerminalContent as be_sdk_beui::be_block::BlockContent>::CONTENT_TYPE,
        );
    }
}

/// The viewer's own terminals, from their owner-only settings.
pub(super) fn own_terminals(effective: Memo<BoatSettings>) -> Memo<BTreeMap<Uuid, TerminalAgent>> {
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

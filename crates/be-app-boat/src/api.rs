//! boat.dev's HTTP API (`https://boat.dev/openapi.yaml`): the calls the editor makes, the
//! shapes it reads, and its errors.

use std::collections::BTreeMap;
use std::future::Future;

use be_sdk_beui::EditorHost;
use be_sdk_beui::network::Request;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub(crate) const API: &str = "https://boat.dev/api/v1";
/// The dashboard's API Keys tab, where a person makes a key.
pub(crate) const API_KEYS: &str = "https://boat.dev/dashboard?tab=api-keys";
/// What a key needs for the list, start and stop, `GET /me`, and a terminal's commands and
/// files, named as boat's key scopes name them (`https://docs.boat.dev/api-keys/scopes.md`).
pub(crate) const ACTIONS: [&str; 6] = [
    "sandbox.read",
    "sandbox.resume",
    "sandbox.stop",
    "account.read",
    "exec",
    "file.write",
];
/// The longest a key may live, and what the sign-in page asks for.
pub(crate) const KEY_DAYS: i64 = 365;
/// The most sandboxes one page lists, boat's own limit.
pub(crate) const PAGE: usize = 200;
/// Pages read before the list stops, so a runaway cursor ends; the list says it was cut off.
pub(crate) const PAGES: usize = 10;

/// The `boat` command that makes the key the sign-in page describes.
pub(crate) fn create_command() -> String {
    format!(
        "boat api-key create be3 --ttl {KEY_DAYS}d --actions {}",
        ACTIONS.join(",")
    )
}

/// A sandbox as `GET /sandboxes` lists it, as much of it as the editor shows. Also what the
/// viewer's view state keeps of it.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Sandbox {
    pub id: String,
    pub name: String,
    pub state: String,
    #[serde(rename = "type")]
    pub size: Option<String>,
    pub vcpu: Option<u32>,
    #[serde(rename = "memoryGB")]
    pub memory_gb: Option<u32>,
    pub ip: Option<String>,
    pub ssh_endpoint: Option<String>,
    pub error: Option<String>,
    pub labels: BTreeMap<String, String>,
}

/// Where a sandbox is, in the words the editor uses: boat's `archived` is stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Phase {
    Running,
    Starting,
    Stopping,
    Stopped,
    Failed,
}

impl Sandbox {
    pub(crate) fn phase(&self) -> Phase {
        match self.state.as_str() {
            "ready" | "idle" | "running" => Phase::Running,
            "init" | "provisioning" | "provisioned" | "cloning" => Phase::Starting,
            "archiving" => Phase::Stopping,
            "archived" => Phase::Stopped,
            _ => Phase::Failed,
        }
    }
}

impl Phase {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Starting => "Starting",
            Self::Stopping => "Stopping",
            Self::Stopped => "Stopped",
            Self::Failed => "Failed",
        }
    }

    /// Whether the sandbox is on its way somewhere, so the list is worth reading sooner.
    pub(crate) fn moving(self) -> bool {
        matches!(self, Self::Starting | Self::Stopping)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SandboxPage {
    sandboxes: Vec<Sandbox>,
    #[serde(default)]
    page_info: Option<PageInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    next_cursor: Option<String>,
    #[serde(default)]
    has_more: bool,
}

#[derive(Deserialize)]
struct Me {
    user: User,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct User {
    #[serde(default)]
    login: String,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct Ignored {}

#[derive(Default, Deserialize)]
#[serde(default)]
struct ErrorEnvelope {
    code: String,
    message: String,
    error: Option<ErrorDetail>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct ErrorDetail {
    code: String,
    message: String,
}

/// Why boat didn't answer as asked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BoatError {
    /// The request never reached boat, or its answer never arrived.
    Offline(String),
    /// boat no longer takes the key: a 401, or `api_key_expired`. Reconnect fixes it.
    Reconnect { expired: bool },
    /// boat refused or failed the request.
    Refused { status: u16, message: String },
    /// The sandbox refused a file sent to an upload URL, or the URL was not one to send it to.
    Upload(String),
}

impl std::fmt::Display for BoatError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Offline(why) => write!(formatter, "boat.dev can't be reached: {why}"),
            Self::Reconnect { expired: true } => formatter.write_str("This boat.dev key expired"),
            Self::Reconnect { expired: false } => {
                formatter.write_str("boat.dev no longer accepts this key")
            }
            Self::Refused { message, .. } | Self::Upload(message) => formatter.write_str(message),
        }
    }
}

fn refused(status: u16, body: &[u8]) -> BoatError {
    let envelope: ErrorEnvelope = serde_json::from_slice(body).unwrap_or_default();
    let detail = envelope.error.unwrap_or_default();
    let code = [envelope.code, detail.code]
        .into_iter()
        .find(|code| !code.is_empty())
        .unwrap_or_default();
    if status == 401 || code == "api_key_expired" {
        return BoatError::Reconnect {
            expired: code == "api_key_expired",
        };
    }
    let message = [envelope.message, detail.message]
        .into_iter()
        .find(|message| !message.is_empty())
        .unwrap_or_else(|| format!("boat.dev answered HTTP {status}"));
    BoatError::Refused { status, message }
}

/// The key and account every call is made with.
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct Account {
    pub key: String,
    pub organization: String,
}

impl std::fmt::Debug for Account {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Account")
            .field("organization", &self.organization)
            .finish_non_exhaustive()
    }
}

impl Account {
    fn request(&self, request: Request) -> Request {
        let request = request
            .bearer(&self.key)
            .header("Accept", "application/json");
        match self.organization.is_empty() {
            true => request,
            false => request.header("X-Boat-Org", &self.organization),
        }
    }
}

async fn send<T: DeserializeOwned>(
    host: EditorHost,
    request: Request,
) -> Option<Result<T, BoatError>> {
    let response = match host.fetch(request).await? {
        Ok(response) => response,
        Err(failure) => return Some(Err(BoatError::Offline(failure.to_string()))),
    };
    let status = response.head.status;
    let body = match response.bytes().await? {
        Ok(body) => body,
        Err(failure) => return Some(Err(BoatError::Offline(failure.to_string()))),
    };
    if !(200..300).contains(&status) {
        return Some(Err(refused(status, &body)));
    }
    Some(
        serde_json::from_slice(&body).map_err(|error| BoatError::Refused {
            status,
            message: format!("boat.dev answered something unexpected: {error}"),
        }),
    )
}

/// Whose key it is: `GET /me`, answered with the login and display name.
pub(crate) fn me(
    host: &EditorHost,
    account: &Account,
) -> impl Future<Output = Option<Result<String, BoatError>>> + 'static {
    let request = account.request(Request::get(format!("{API}/me")));
    let host = host.clone();
    async move {
        let me: Me = match send(host, request).await? {
            Ok(me) => me,
            Err(error) => return Some(Err(error)),
        };
        let user = me.user;
        Some(Ok(
            match user.display_name.filter(|name| !name.is_empty()) {
                Some(name) if name != user.login => format!("{name} ({})", user.login),
                _ => user.login,
            },
        ))
    }
}

/// Every sandbox the account shows, through the pages of `GET /sandboxes`, with the block's
/// filters, and whether more were listed than `PAGES` pages hold.
pub(crate) fn sandboxes(
    host: &EditorHost,
    account: &Account,
    name_prefix: &str,
    label: Option<(&str, &str)>,
) -> impl Future<Output = Option<Result<(Vec<Sandbox>, bool), BoatError>>> + 'static {
    let mut query = format!("limit={PAGE}");
    if !name_prefix.is_empty() {
        query.push_str(&format!("&namePrefix={}", encoded(name_prefix)));
    }
    if let Some((key, value)) = label {
        query.push_str(&format!("&label={}", encoded(&format!("{key}={value}"))));
    }
    let host = host.clone();
    let account = account.clone();
    async move {
        let mut listed = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..PAGES {
            let after = cursor
                .as_deref()
                .map(|cursor| format!("&cursor={}", encoded(cursor)))
                .unwrap_or_default();
            let request = account.request(Request::get(format!("{API}/sandboxes?{query}{after}")));
            let page: SandboxPage = match send(host.clone(), request).await? {
                Ok(page) => page,
                Err(error) => return Some(Err(error)),
            };
            listed.extend(page.sandboxes);
            cursor = page
                .page_info
                .filter(|info| info.has_more)
                .and_then(|info| info.next_cursor);
            if cursor.is_none() {
                return Some(Ok((listed, false)));
            }
        }
        Some(Ok((listed, true)))
    }
}

/// What the editor asks of one sandbox.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Action {
    Resume,
    Stop,
}

/// `POST /sandboxes/{id}/resume` or `/stop`. boat answers once it has started either; the
/// list shows how it goes.
pub(crate) fn act(
    host: &EditorHost,
    account: &Account,
    sandbox: &str,
    action: Action,
) -> impl Future<Output = Option<Result<(), BoatError>>> + 'static {
    let verb = match action {
        Action::Resume => "resume",
        Action::Stop => "stop",
    };
    let request = account
        .request(Request::post(format!(
            "{API}/sandboxes/{}/{verb}",
            encoded(sandbox)
        )))
        .header("Content-Type", "application/json")
        .with_body(b"{}".to_vec());
    let host = host.clone();
    async move { Some(send::<Ignored>(host, request).await?.map(|_| ())) }
}

/// What a command run in a sandbox answered.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Ran {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl Ran {
    /// The command's output if it succeeded, and otherwise why it failed.
    pub(crate) fn succeeded(self) -> Result<String, BoatError> {
        if self.exit_code == Some(0) && !self.timed_out {
            return Ok(self.stdout);
        }
        let said = [self.stderr.trim(), self.stdout.trim()]
            .into_iter()
            .find(|said| !said.is_empty())
            .and_then(|said| said.lines().last())
            .unwrap_or_default()
            .to_owned();
        let message = match (self.timed_out, said.is_empty()) {
            (true, _) => "A command in the sandbox took too long".to_owned(),
            (false, true) => "A command in the sandbox failed".to_owned(),
            (false, false) => format!("A command in the sandbox failed: {said}"),
        };
        Err(BoatError::Refused {
            status: 200,
            message,
        })
    }
}

/// `POST /sandboxes/{id}/commands`: runs `command` with `sh` as the sandbox's `user`, waiting
/// at most `timeout` seconds. boat keeps every command and its output, so no secret goes in
/// one: secrets go in files (`write_file`).
pub(crate) fn command(
    host: &EditorHost,
    account: &Account,
    sandbox: &str,
    command: &str,
    timeout: u32,
) -> impl Future<Output = Option<Result<String, BoatError>>> + 'static {
    let body = serde_json::json!({ "command": command, "timeoutSeconds": timeout });
    let request = account
        .request(Request::post(format!(
            "{API}/sandboxes/{}/commands",
            encoded(sandbox)
        )))
        .header("Content-Type", "application/json")
        .with_body(body.to_string().into_bytes());
    let host = host.clone();
    async move { Some(send::<Ran>(host, request).await?.and_then(Ran::succeeded)) }
}

/// `PUT /sandboxes/{id}/files`: writes `content` to `path`, under `/home/user` or `/tmp`.
pub(crate) fn write_file(
    host: &EditorHost,
    account: &Account,
    sandbox: &str,
    path: &str,
    content: &str,
) -> impl Future<Output = Option<Result<(), BoatError>>> + 'static {
    let body = serde_json::json!({ "path": path, "content": content, "encoding": "utf8" });
    let request = account
        .request(Request::put(format!(
            "{API}/sandboxes/{}/files",
            encoded(sandbox)
        )))
        .header("Content-Type", "application/json")
        .with_body(body.to_string().into_bytes());
    let host = host.clone();
    async move { Some(send::<Ignored>(host, request).await?.map(|_| ())) }
}

#[derive(Deserialize)]
struct UploadUrl {
    url: String,
}

/// Writes `bytes` to `path` however large: `POST /sandboxes/{id}/files/upload-url`, then the
/// bytes `PUT` to the URL it answers, which takes no key.
pub(crate) fn upload(
    host: &EditorHost,
    account: &Account,
    sandbox: &str,
    path: &str,
    bytes: Vec<u8>,
) -> impl Future<Output = Option<Result<(), BoatError>>> + 'static {
    let body = serde_json::json!({ "path": path });
    let request = account
        .request(Request::post(format!(
            "{API}/sandboxes/{}/files/upload-url",
            encoded(sandbox)
        )))
        .header("Content-Type", "application/json")
        .with_body(body.to_string().into_bytes());
    let host = host.clone();
    async move {
        let upload: UploadUrl = match send(host.clone(), request).await? {
            Ok(upload) => upload,
            Err(error) => return Some(Err(error)),
        };
        if !upload.url.starts_with("https://") {
            return Some(Err(BoatError::Upload(
                "boat.dev answered an upload URL that is not https".to_owned(),
            )));
        }
        let put = Request::put(upload.url)
            .header("Content-Type", "application/octet-stream")
            .with_body(bytes);
        let response = match host.fetch(put).await? {
            Ok(response) => response,
            Err(failure) => return Some(Err(BoatError::Offline(failure.to_string()))),
        };
        let status = response.head.status;
        if let Err(failure) = response.bytes().await? {
            return Some(Err(BoatError::Offline(failure.to_string())));
        }
        Some(match (200..300).contains(&status) {
            true => Ok(()),
            false => Err(BoatError::Upload(format!(
                "The sandbox refused the upload of be: HTTP {status}"
            ))),
        })
    }
}

/// `text` as a URL's query value or path segment.
fn encoded(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

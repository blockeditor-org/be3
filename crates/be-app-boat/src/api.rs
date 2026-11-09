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
/// What a key needs for the list, start and stop, and for `GET /me`, named as boat's key
/// scopes name them (`https://docs.boat.dev/api-keys/scopes.md`).
pub(crate) const ACTIONS: [&str; 4] = [
    "sandbox.read",
    "sandbox.resume",
    "sandbox.stop",
    "account.read",
];
/// The longest a key may live, and what the sign-in page asks for.
pub(crate) const KEY_DAYS: i64 = 365;
/// The most sandboxes one page lists, boat's own limit.
const PAGE: usize = 200;
/// Pages read before the list stops, so a runaway cursor ends.
const PAGES: usize = 10;

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
}

impl std::fmt::Display for BoatError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Offline(why) => write!(formatter, "boat.dev can't be reached: {why}"),
            Self::Reconnect { expired: true } => formatter.write_str("This boat.dev key expired"),
            Self::Reconnect { expired: false } => {
                formatter.write_str("boat.dev no longer accepts this key")
            }
            Self::Refused { message, .. } => formatter.write_str(message),
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Account {
    pub key: String,
    pub organization: String,
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

/// Every sandbox the account shows, through every page of `GET /sandboxes`, with the
/// block's filters.
pub(crate) fn sandboxes(
    host: &EditorHost,
    account: &Account,
    name_prefix: &str,
    label: Option<(&str, &str)>,
) -> impl Future<Output = Option<Result<Vec<Sandbox>, BoatError>>> + 'static {
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
                break;
            }
        }
        Some(Ok(listed))
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

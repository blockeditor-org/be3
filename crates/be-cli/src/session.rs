use std::path::PathBuf;

use be_client::{ClientError, Connection, Credentials, Peer, PeerConfig, authorize, http_url};
use be_protocol::{ClientMessage, ServerMessage};
use be_store::FileStore;
use uuid::Uuid;

pub const DEFAULT_SERVER: &str = "https://api.be3.pfg.pw";

#[derive(Clone, Debug)]
pub struct Settings {
    pub server: String,
    pub organization: Option<Uuid>,
    pub token: String,
    pub phrase: String,
    pub authorization: Option<String>,
    pub cache: PathBuf,
}

impl Settings {
    pub fn from_env() -> Result<Self, String> {
        Self::from_values(|name| std::env::var(name).ok())
    }

    pub(crate) fn from_values(
        environment: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, String> {
        let env = |name: &str| {
            environment(name)
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        let bundle = env("BE3_AGENT_CREDENTIALS")
            .map(|text| {
                let value: serde_json::Value = serde_json::from_str(&text)
                    .map_err(|_| "BE3_AGENT_CREDENTIALS must be a JSON object".to_owned())?;
                if !value.is_object() {
                    return Err("BE3_AGENT_CREDENTIALS must be a JSON object".to_owned());
                }
                for key in ["server", "organization", "recovery_phrase"] {
                    if value
                        .get(key)
                        .and_then(|value| value.as_str())
                        .is_none_or(|value| value.trim().is_empty())
                    {
                        return Err(format!(
                            "BE3_AGENT_CREDENTIALS needs a nonempty {key} string"
                        ));
                    }
                }
                if value.get("authentication").and_then(|v| v.as_str()) != Some("proxy")
                    && value
                        .get("token")
                        .and_then(|v| v.as_str())
                        .is_none_or(|v| v.trim().is_empty())
                {
                    return Err(
                        "BE3_AGENT_CREDENTIALS needs a token or authentication set to proxy"
                            .to_owned(),
                    );
                }
                Ok(value)
            })
            .transpose()?;
        let read = |name: &str| {
            let key = match name {
                "BE3_SERVER" => Some("server"),
                "BE3_ORGANIZATION" => Some("organization"),
                "BE3_TOKEN" => Some("token"),
                "BE3_RECOVERY_PHRASE" => Some("recovery_phrase"),
                _ => None,
            };
            if let (Some(bundle), Some(key)) = (&bundle, key) {
                return bundle[key].as_str().map(|value| value.trim().to_owned());
            }
            env(name)
        };
        let organization = match read("BE3_ORGANIZATION") {
            Some(text) => Some(
                Uuid::parse_str(&text)
                    .map_err(|_| "BE3_ORGANIZATION must be an organization id".to_owned())?,
            ),
            None => None,
        };
        let proxy = bundle
            .as_ref()
            .is_some_and(|v| v["authentication"] == "proxy");
        let token = if proxy {
            Some(String::new())
        } else {
            read("BE3_TOKEN")
        }
        .ok_or("set BE3_TOKEN to the token the app showed when it added this agent")?;
        let phrase = read("BE3_RECOVERY_PHRASE").ok_or(
            "set BE3_RECOVERY_PHRASE to the recovery phrase the app showed when it added this agent",
        )?;
        Ok(Self {
            server: read("BE3_SERVER").unwrap_or_else(|| DEFAULT_SERVER.to_owned()),
            organization,
            token,
            phrase,
            authorization: read("BE3_AUTHORIZATION"),
            cache: read("BE3_CACHE")
                .map(PathBuf::from)
                .or_else(|| {
                    read("XDG_CACHE_HOME")
                        .map(PathBuf::from)
                        .or_else(|| read("HOME").map(|home| PathBuf::from(home).join(".cache")))
                        .map(|cache| cache.join("be3").join("be"))
                })
                .ok_or("set BE3_CACHE to a directory be can keep the objects it reads in")?,
        })
    }

    fn credentials(&self) -> Credentials {
        if self.token.is_empty() {
            Credentials::Header
        } else {
            Credentials::Token(self.token.clone())
        }
    }
}

fn secure_or_local(url: &str) -> bool {
    let Some(rest) = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("ws://"))
    else {
        return true;
    };
    let authority = rest.split('/').next().unwrap_or_default();
    let host = match authority.strip_prefix('[') {
        Some(bracketed) => bracketed.split(']').next().unwrap_or_default(),
        None => authority
            .rsplit_once(':')
            .map_or(authority, |(host, _)| host),
    };
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

pub async fn connect(settings: &Settings) -> Result<Peer<FileStore>, String> {
    connect_to(settings, http_url(&settings.server)).await
}

pub async fn connect_live(settings: &Settings) -> Result<Peer<FileStore>, String> {
    connect_to(settings, be_client::socket_url(&settings.server)).await
}

async fn connect_to(settings: &Settings, url: String) -> Result<Peer<FileStore>, String> {
    authorize(&url, settings.authorization.clone());
    if !secure_or_local(&url) {
        return Err(format!(
            "{} is not https, so BE3_TOKEN would cross the network unencrypted",
            settings.server
        ));
    }
    let connection = Connection::connect(&url, &be_client::Clock::real())
        .await
        .map_err(explain)?;
    let signed_in = connection
        .request(|request| {
            if settings.token.is_empty() {
                ClientMessage::AuthenticateHeader { request }
            } else {
                ClientMessage::Authenticate {
                    request,
                    token: settings.token.clone(),
                }
            }
        })
        .await
        .map_err(|error| {
            format!(
                "the server refused account authentication: {}",
                explain(error)
            )
        })?;
    let ServerMessage::Authenticated { account, token, .. } = signed_in else {
        return Err("the server did not sign this connection in".to_owned());
    };
    let organization = match settings.organization {
        Some(organization) => organization,
        None => only_organization(&connection).await?,
    };
    let identity = identity(&connection, &settings.phrase).await?;
    Peer::signed_in(
        connection,
        account,
        token,
        PeerConfig::new(url, identity, settings.credentials()).organization(Some(organization)),
        FileStore::open(settings.cache.join(organization.simple().to_string())).map_err(
            |error| {
                format!(
                    "cannot keep objects in {}: {error}",
                    settings.cache.display()
                )
            },
        )?,
    )
    .await
    .map_err(explain)
}

async fn only_organization(connection: &Connection) -> Result<Uuid, String> {
    let ServerMessage::Organizations { organizations, .. } = connection
        .request(|request| ClientMessage::ListOrganizations { request })
        .await
        .map_err(explain)?
    else {
        return Err("the server did not list organizations".to_owned());
    };
    match organizations.as_slice() {
        [only] => Ok(only.id),
        [] => Err("this account is not a member of any organization".to_owned()),
        several => Err(format!(
            "this account is in {} organizations; set BE3_ORGANIZATION to one of: {}",
            several.len(),
            several
                .iter()
                .map(|organization| format!("{} ({})", organization.id, organization.name))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

async fn identity(
    connection: &Connection,
    phrase: &str,
) -> Result<be_keys::RecoverySecret, String> {
    let secret = be_keys::RecoveryPhrase::parse(phrase)
        .map_err(|error| format!("BE3_RECOVERY_PHRASE: {error}"))?
        .secret();
    let ServerMessage::RecoveryKey { public, .. } = connection
        .request(|request| ClientMessage::GetRecoveryKey { request })
        .await
        .map_err(explain)?
    else {
        return Err("the server did not send the account's recovery key".to_owned());
    };
    match public {
        Some(public) if public == secret.public().0 => Ok(secret),
        Some(_) => Err("BE3_RECOVERY_PHRASE is not this account's recovery phrase".to_owned()),
        None => Err("this account has no recovery phrase yet".to_owned()),
    }
}

pub fn explain(error: ClientError) -> String {
    match error {
        ClientError::Refused(_, message) => message,
        ClientError::Incompatible { ours, theirs } => format!(
            "this be is build {ours} and the server is build {theirs}; ./scripts/be runs the release pinned in scripts/internal/pins, which must be the server's build"
        ),
        other => other.to_string(),
    }
}

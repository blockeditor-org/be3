use std::path::PathBuf;

use be_client::{ClientError, Connection, Credentials, Peer, PeerConfig, authorize, http_url};
use be_protocol::{ClientMessage, ServerMessage};
use be_store::FileStore;
use uuid::Uuid;

pub const DEFAULT_SERVER: &str = "https://blocks.pfg.pw";

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
        let read = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        let organization = match read("BE3_ORGANIZATION") {
            Some(text) => Some(
                Uuid::parse_str(&text)
                    .map_err(|_| format!("BE3_ORGANIZATION is not an organization id: {text}"))?,
            ),
            None => None,
        };
        let token = read("BE3_TOKEN")
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
        Credentials::Token(self.token.clone())
    }
}

fn secure_or_local(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
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
    let url = http_url(&settings.server);
    authorize(&url, settings.authorization.clone());
    if !secure_or_local(&url) {
        return Err(format!(
            "{} is not https, so BE3_TOKEN would cross the network unencrypted",
            settings.server
        ));
    }
    let connection = Connection::connect(&url).await.map_err(explain)?;
    let signed_in = connection
        .request(|request| ClientMessage::Authenticate {
            request,
            token: settings.token.clone(),
        })
        .await
        .map_err(|error| format!("the server refused BE3_TOKEN: {}", explain(error)))?;
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
            "this be is build {ours} and the server is build {theirs}; ./scripts/be runs the release pinned in scripts/internal/common.sh, which must be the server's build"
        ),
        other => other.to_string(),
    }
}

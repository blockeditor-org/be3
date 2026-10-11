use be_client::{ClientError, Connection, Credentials, Peer, PeerConfig, authorize, http_url};
use be_protocol::{ClientMessage, ServerMessage};
use be_store::{ContentKey, MemoryStore};
use uuid::Uuid;

pub const DEFAULT_SERVER: &str = "https://blocks.pfg.pw";

#[derive(Clone, Debug)]
pub struct Settings {
    pub server: String,
    pub workspace: Option<Uuid>,
    pub token: String,
    pub phrase: String,
    pub authorization: Option<String>,
}

impl Settings {
    pub fn from_env() -> Result<Self, String> {
        let read = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        let workspace = match read("BE3_WORKSPACE") {
            Some(text) => Some(
                Uuid::parse_str(&text)
                    .map_err(|_| format!("BE3_WORKSPACE is not a workspace id: {text}"))?,
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
            workspace,
            token,
            phrase,
            authorization: read("BE3_AUTHORIZATION"),
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

pub async fn connect(settings: &Settings) -> Result<Peer<MemoryStore>, String> {
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
    if !matches!(signed_in, ServerMessage::Authenticated { .. }) {
        return Err("the server did not sign this connection in".to_owned());
    }
    let workspace = match settings.workspace {
        Some(workspace) => workspace,
        None => only_workspace(&connection).await?,
    };
    let key = workspace_key(&connection, workspace, &settings.phrase).await?;
    Peer::connect(
        PeerConfig::new(url, ContentKey::from_bytes(key), settings.credentials())
            .workspace(Some(workspace)),
        MemoryStore::new(),
    )
    .await
    .map_err(explain)
}

async fn only_workspace(connection: &Connection) -> Result<Uuid, String> {
    let ServerMessage::Workspaces { workspaces, .. } = connection
        .request(|request| ClientMessage::ListWorkspaces { request })
        .await
        .map_err(explain)?
    else {
        return Err("the server did not list workspaces".to_owned());
    };
    match workspaces.as_slice() {
        [only] => Ok(only.id),
        [] => Err("this account is not a member of any workspace".to_owned()),
        several => Err(format!(
            "this account is in {} workspaces; set BE3_WORKSPACE to one of: {}",
            several.len(),
            several
                .iter()
                .map(|workspace| format!("{} ({})", workspace.id, workspace.name))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

async fn workspace_key(
    connection: &Connection,
    workspace: Uuid,
    phrase: &str,
) -> Result<[u8; 32], String> {
    let ServerMessage::Keys { sealed, .. } = connection
        .request(|request| ClientMessage::GetKeys { request })
        .await
        .map_err(explain)?
    else {
        return Err("the server did not send the keys".to_owned());
    };
    let sealed = sealed
        .into_iter()
        .find(|key| key.workspace == workspace)
        .ok_or("no workspace key has been sealed for this account yet; open the workspace in the app once so it can seal one")?;
    let phrase = be_keys::RecoveryPhrase::parse(phrase).map_err(|error| error.to_string())?;
    let key = phrase
        .secret()
        .open(&sealed.sealed)
        .map_err(|error| error.to_string())?;
    key.try_into()
        .map_err(|_| "the workspace key is not 32 bytes".to_owned())
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

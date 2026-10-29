use be_client::{ClientError, Connection};
use be_protocol::{ClientMessage, ErrorCode, Organization, OrganizationInvitation, ServerMessage};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Session {
    pub account: Uuid,
    pub email: String,
    pub display_name: String,
    pub token: String,
}

#[derive(Debug)]
pub struct AccountError {
    pub message: String,
    pub invalid_token: bool,
}

impl From<ClientError> for AccountError {
    fn from(error: ClientError) -> Self {
        let invalid_token = matches!(
            error,
            ClientError::Refused(
                ErrorCode::InvalidCredentials | ErrorCode::NotAuthenticated,
                _
            )
        );
        let message = match error {
            ClientError::Refused(_, message) => message,
            other => other.to_string(),
        };
        Self {
            message,
            invalid_token,
        }
    }
}

impl std::fmt::Display for AccountError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

pub use be_client::socket_url;

pub fn link_base(server_url: &str) -> String {
    match server_url.split_once("://") {
        Some(("ws", rest)) => format!("http://{rest}"),
        Some(("wss", rest)) => format!("https://{rest}"),
        _ => server_url.to_owned(),
    }
}

fn session_of(response: ServerMessage) -> Result<Session, AccountError> {
    match response {
        ServerMessage::Authenticated {
            account,
            email,
            display_name,
            token,
            ..
        } => Ok(Session {
            account,
            email,
            display_name,
            token,
        }),
        _ => Err(ClientError::Unexpected.into()),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrowserLogin {
    pub code: String,
    pub url: String,
}

pub enum LoginEvent {
    Browser(BrowserLogin),
    Finished(Result<Session, String>),
}

pub async fn create_local_profile(
    server_url: String,
    display_name: String,
) -> Result<Session, AccountError> {
    let connection = crate::link::connect(&server_url).await?;
    let session = session_of(
        connection
            .request(|request| ClientMessage::CreateLocalProfile {
                request,
                display_name,
            })
            .await?,
    )?;
    crate::link::adopt(&server_url, &session.token, session.account, connection).await;
    Ok(session)
}

pub async fn browser_login(
    server_url: String,
    auth_url: String,
    notify: impl FnOnce(BrowserLogin),
    cancelled: futures_util::future::BoxFuture<'static, ()>,
) -> Result<Session, String> {
    let login = async {
        let mut connection = crate::link::connect(&server_url)
            .await
            .map_err(|error| error.to_string())?;
        let mut events = connection.subscribe();
        let response = connection
            .request(|request| ClientMessage::BeginBrowserLogin { request })
            .await
            .map_err(|error| error.to_string())?;
        let ServerMessage::BrowserLogin { code, secret, .. } = response else {
            return Err("Unexpected sign-in response".into());
        };
        notify(BrowserLogin {
            code: code.clone(),
            url: format!("{auth_url}?code={code}"),
        });
        loop {
            if !approval_pushed(&mut events, connection.closed()).await {
                connection = loop {
                    be_client::Clock::real()
                        .sleep(std::time::Duration::from_secs(2))
                        .await;
                    match crate::link::connect(&server_url).await {
                        Ok(connection) => break connection,
                        Err(error) if error.is_unreachable() => {}
                        Err(error) => return Err(error.to_string()),
                    }
                };
                events = connection.subscribe();
            }
            match connection
                .request(|request| ClientMessage::FinishBrowserLogin {
                    request,
                    code: code.clone(),
                    secret: secret.clone(),
                })
                .await
            {
                Ok(ServerMessage::Ok { .. }) => {}
                Err(error) if error.is_unreachable() => {}
                Ok(response) => {
                    let session = session_of(response).map_err(|error| error.to_string())?;
                    crate::link::adopt(&server_url, &session.token, session.account, connection)
                        .await;
                    return Ok(session);
                }
                Err(error) => return Err(error.to_string()),
            }
        }
    };
    match futures_util::future::select(Box::pin(login), cancelled).await {
        futures_util::future::Either::Left((result, _)) => result,
        futures_util::future::Either::Right(_) => Err("Sign-in cancelled".into()),
    }
}

async fn approval_pushed(
    events: &mut tokio::sync::broadcast::Receiver<ServerMessage>,
    mut closed: tokio::sync::watch::Receiver<bool>,
) -> bool {
    let pushed = async {
        loop {
            match events.recv().await {
                Ok(ServerMessage::BrowserLoginFinished { .. })
                | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => return true,
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return false,
            }
        }
    };
    let closed = async {
        let _ = closed.wait_for(|closed| *closed).await;
    };
    matches!(
        futures_util::future::select(std::pin::pin!(pushed), std::pin::pin!(closed)).await,
        futures_util::future::Either::Left((true, _))
    )
}

async fn authenticated(
    server_url: &str,
    token: String,
) -> Result<std::sync::Arc<Connection>, AccountError> {
    Ok(
        crate::link::signed_in(server_url, &token, &be_client::Clock::real())
            .await?
            .connection,
    )
}

pub async fn logout(server_url: String, token: String) -> Result<(), AccountError> {
    let connection = authenticated(&server_url, token.clone()).await?;
    crate::link::forget(&server_url, &token).await;
    connection
        .request(|request| ClientMessage::Logout { request })
        .await?;
    Ok(())
}

pub async fn organizations(
    server_url: String,
    token: String,
) -> Result<Vec<Organization>, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    let ServerMessage::Organizations { organizations, .. } = connection
        .request(|request| ClientMessage::ListOrganizations { request })
        .await?
    else {
        return Err(ClientError::Unexpected.into());
    };
    Ok(organizations)
}

pub async fn create_organization(
    server_url: String,
    token: String,
    name: String,
) -> Result<Organization, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    match connection
        .request(|request| ClientMessage::CreateOrganization { request, name })
        .await?
    {
        ServerMessage::Organizations {
            mut organizations, ..
        } if !organizations.is_empty() => Ok(organizations.remove(0)),
        _ => Err(ClientError::Unexpected.into()),
    }
}

#[derive(Clone, Debug)]
pub struct Me {
    pub account: Uuid,
    pub identity: [u8; 32],
    pub display_name: String,
    pub email: String,
}

impl Me {
    pub fn secret(&self) -> be_keys::RecoverySecret {
        be_keys::RecoverySecret::from_bytes(self.identity)
    }

    pub fn card(&self) -> be_keys::ContactCard {
        be_client::contacts::card(
            self.account,
            &self.secret(),
            &self.display_name,
            &self.email,
        )
    }
}

pub async fn invitation(
    server_url: String,
    token: String,
    code: be_keys::InviteCode,
) -> Result<OrganizationInvitation, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    Ok(be_client::contacts::invitation(&connection, &code).await?.0)
}

pub async fn accept_invitation(
    server_url: String,
    token: String,
    me: Me,
    code: be_keys::InviteCode,
) -> Result<Uuid, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    let invitation =
        be_client::contacts::accept_invitation(&connection, &me.secret(), &me.card(), &code)
            .await?;
    Ok(invitation.organization)
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentCredentials {
    pub name: String,
    pub server: String,
    pub organization: Uuid,
    pub token: String,
    pub phrase: String,
}

pub async fn agents(
    server_url: String,
    token: String,
    organization: Uuid,
) -> Result<Vec<be_protocol::Agent>, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    let ServerMessage::Agents { agents, .. } = connection
        .request(|request| ClientMessage::ListAgents {
            request,
            organization,
        })
        .await?
    else {
        return Err(ClientError::Unexpected.into());
    };
    Ok(agents)
}

pub async fn remove_agent(
    server_url: String,
    token: String,
    organization: Uuid,
    account: Uuid,
) -> Result<(), AccountError> {
    let connection = authenticated(&server_url, token).await?;
    connection
        .request(|request| ClientMessage::RemoveAgent {
            request,
            organization,
            account,
        })
        .await?;
    Ok(())
}

#[derive(Clone, Debug, Default)]
pub struct Keys {
    pub account: Uuid,
    pub recovery: Option<be_keys::RecoveryPublic>,
    pub sealed: Vec<be_protocol::SealedKey>,
    pub own: Vec<be_protocol::OwnSealed>,
}

pub async fn keys(server_url: String, token: String) -> Result<Keys, AccountError> {
    let crate::link::Link {
        connection,
        account,
        ..
    } = crate::link::signed_in(&server_url, &token, &be_client::Clock::real()).await?;
    let ServerMessage::RecoveryKey {
        public,
        sealed,
        own,
        ..
    } = connection
        .request(|request| ClientMessage::GetRecoveryKey { request })
        .await?
    else {
        return Err(ClientError::Unexpected.into());
    };
    Ok(Keys {
        account,
        recovery: public.map(be_keys::RecoveryPublic),
        sealed,
        own,
    })
}

pub async fn set_recovery_key(
    server_url: String,
    token: String,
    public: be_keys::RecoveryPublic,
    resealed: be_client::contacts::NewIdentity,
) -> Result<(), AccountError> {
    let connection = authenticated(&server_url, token).await?;
    connection
        .request(|request| ClientMessage::SetRecoveryKey {
            request,
            public: public.0,
            sealed: resealed.sealed,
            own: resealed.own,
            changes: resealed.changes,
        })
        .await?;
    Ok(())
}

pub async fn pair(
    server_url: String,
    token: String,
    account: Uuid,
    code: String,
    device: String,
    cancelled: futures_util::future::BoxFuture<'static, ()>,
) -> Result<[u8; 32], AccountError> {
    let connection = authenticated(&server_url, token).await?;
    let mut events = connection.subscribe();
    let (pairing, message) = be_keys::Pairing::start(&code, account.as_bytes());
    let payload = be_keys::PairingMessage::Request { message, device }.encode();
    connection
        .request(|request| ClientMessage::Pair {
            request,
            to: None,
            payload,
        })
        .await?;
    let reply = async {
        loop {
            match events.recv().await {
                Ok(ServerMessage::Paired { payload, .. }) => {
                    if let Ok(be_keys::PairingMessage::Reply { message, sealed }) =
                        be_keys::PairingMessage::decode(&payload)
                    {
                        return Ok((message, sealed));
                    }
                }
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    return Err(AccountError::from(ClientError::Disconnected(
                        "the connection closed while waiting for another device".into(),
                    )));
                }
            }
        }
    };
    let (message, sealed) =
        match futures_util::future::select(std::pin::pin!(reply), cancelled).await {
            futures_util::future::Either::Left((reply, _)) => reply?,
            futures_util::future::Either::Right(_) => {
                return Err(AccountError {
                    message: "pairing was cancelled".into(),
                    invalid_token: false,
                });
            }
        };
    let wrong = || AccountError {
        message: "The code was typed wrong on the other device. Try again with a new code.".into(),
        invalid_token: false,
    };
    let key = pairing.finish(&message).map_err(|_| wrong())?;
    key.open(&sealed)
        .map_err(|_| wrong())?
        .try_into()
        .map_err(|_| wrong())
}

pub fn authorize(saved: &[block_app_base::app_state::SavedAccount]) {
    for account in saved {
        if let block_app_base::app_state::ServerLocation::Remote(url) = &account.server {
            be_client::authorize(url, account.authorization.clone());
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

use be_client::{ClientError, Connection, ask};
use be_protocol::{Authenticated, ErrorCode, Organization, OrganizationInvitation, client};
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

fn session_of(
    Authenticated {
        account,
        email,
        display_name,
        token,
    }: Authenticated,
) -> Session {
    Session {
        account,
        email,
        display_name,
        token,
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
        ask(client::account::create_local_profile(
            &connection,
            display_name,
        ))
        .await?,
    );
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
        let be_protocol::BrowserLogin { code, secret } =
            ask(client::account::begin_browser_login(&connection))
                .await
                .map_err(|error| error.to_string())?;
        notify(BrowserLogin {
            code: code.clone(),
            url: format!("{auth_url}?code={code}"),
        });
        loop {
            match ask(client::account::finish_browser_login(
                &connection,
                code.clone(),
                secret.clone(),
            ))
            .await
            {
                Ok(response) => {
                    let session = session_of(response);
                    crate::link::adopt(&server_url, &session.token, session.account, connection)
                        .await;
                    return Ok(session);
                }
                Err(error) if error.is_unreachable() => {}
                Err(error) => return Err(error.to_string()),
            }
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
        }
    };
    match futures_util::future::select(Box::pin(login), cancelled).await {
        futures_util::future::Either::Left((result, _)) => result,
        futures_util::future::Either::Right(_) => Err("Sign-in cancelled".into()),
    }
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
    ask(client::account::logout(&connection)).await?;
    Ok(())
}

pub async fn organizations(
    server_url: String,
    token: String,
) -> Result<Vec<Organization>, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    Ok(ask(client::organizations::list_organizations(&connection)).await?)
}

pub async fn create_organization(
    server_url: String,
    token: String,
    name: String,
) -> Result<Organization, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    Ok(ask(client::organizations::create_organization(
        &connection,
        name,
    ))
    .await?)
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
    Ok(ask(client::members::list_agents(&connection, organization)).await?)
}

pub async fn remove_agent(
    server_url: String,
    token: String,
    organization: Uuid,
    account: Uuid,
) -> Result<(), AccountError> {
    let connection = authenticated(&server_url, token).await?;
    ask(client::members::remove_agent(
        &connection,
        organization,
        account,
    ))
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
    let be_protocol::RecoveryKey {
        public,
        sealed,
        own,
    } = ask(client::keys::get_recovery_key(&connection)).await?;
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
    ask(client::keys::set_recovery_key(
        &connection,
        public.0,
        resealed.sealed,
        resealed.own,
        resealed.changes,
    ))
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
    let mut paired = client::account::paired(&connection);
    let (pairing, message) = be_keys::Pairing::start(&code, account.as_bytes());
    let payload = be_keys::PairingMessage::Request { message, device }.encode();
    ask(client::account::pair(&connection, None, payload)).await?;
    let reply = async {
        while let Some(heard) = paired.recv().await {
            if let Ok(be_keys::PairingMessage::Reply { message, sealed }) =
                be_keys::PairingMessage::decode(&heard.payload)
            {
                return Ok((message, sealed));
            }
        }
        Err(AccountError::from(ClientError::Disconnected(
            "the connection closed while waiting for another device".into(),
        )))
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

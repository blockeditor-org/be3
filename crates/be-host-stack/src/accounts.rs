use be_block_client::{ClientError, Connection, ask};
use be_block_protocol::{Authenticated, ErrorCode, Organization, OrganizationInvitation, client};
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

pub use be_block_client::socket_url;

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

/// How often a browser sign-in is polled while nobody asks to check now. A poll at once is
/// `browser_login`'s `check_now`; neither polls sooner than the server's interval.
pub const LOGIN_POLL_EVERY: std::time::Duration = std::time::Duration::from_secs(10);

/// Signs in through the browser: begins a sign-in with a one-shot call, tells `notify` its code
/// and link, and polls it until the person approves or denies it there, or it expires. Polls
/// come every `LOGIN_POLL_EVERY`, and as soon as the server allows after each `check_now`; a
/// poll the server finds too soon slows them down (RFC 8628). Once approved, the account's
/// connection is opened with the token.
pub async fn browser_login(
    server_url: String,
    auth_url: String,
    notify: impl FnOnce(BrowserLogin),
    mut check_now: tokio::sync::mpsc::UnboundedReceiver<()>,
    cancelled: futures_util::future::BoxFuture<'static, ()>,
) -> Result<Session, String> {
    use be_block_protocol::BrowserLoginState;
    use std::time::Duration;
    let login = async {
        let begun = be_block_client::call_once(&server_url, |peer| {
            client::account::begin_browser_login(peer)
        })
        .await
        .map_err(|error| error.to_string())?;
        notify(BrowserLogin {
            code: begun.code.clone(),
            url: format!("{auth_url}?code={}", begun.code),
        });
        let clock = be_block_client::Clock::real();
        let mut interval = Duration::from_secs(u64::from(begun.interval.max(1)));
        let mut polled = clock.now();
        let mut checking = false;
        let mut listening = true;
        loop {
            let at = match checking {
                true => polled + interval,
                false => polled + LOGIN_POLL_EVERY.max(interval),
            };
            if listening {
                let waited =
                    futures_util::future::select(clock.sleep_until(at), Box::pin(check_now.recv()))
                        .await;
                if let futures_util::future::Either::Right((asked, _)) = waited {
                    // Asked to check now, or nobody can ask any more: wait for the poll this
                    // allows, or the next on the schedule.
                    checking |= asked.is_some();
                    listening = asked.is_some();
                    continue;
                }
            } else {
                clock.sleep_until(at).await;
            }
            checking = false;
            polled = clock.now();
            let (code, secret) = (begun.code.clone(), begun.secret.clone());
            let answer = be_block_client::call_once(&server_url, move |peer| {
                client::account::poll_browser_login(peer, code, secret)
            })
            .await;
            match answer {
                Ok(BrowserLoginState::Pending) => {}
                Ok(BrowserLoginState::Approved(authenticated)) => {
                    let session = session_of(authenticated);
                    crate::link::signed_in(&server_url, &session.token, &clock)
                        .await
                        .map_err(|error| error.to_string())?;
                    return Ok(session);
                }
                Ok(BrowserLoginState::Denied) => {
                    return Err("Sign-in was denied in the browser.".into());
                }
                Ok(BrowserLoginState::Expired) => {
                    return Err("This sign-in expired. Start it again.".into());
                }
                Err(ClientError::Refused(ErrorCode::SlowDown, _)) => {
                    interval += Duration::from_secs(5);
                }
                Err(error) if error.is_unreachable() => {}
                Err(error) => return Err(error.to_string()),
            }
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
        crate::link::signed_in(server_url, &token, &be_block_client::Clock::real())
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
    pub fn secret(&self) -> be_block_keys::RecoverySecret {
        be_block_keys::RecoverySecret::from_bytes(self.identity)
    }

    pub fn card(&self) -> be_block_keys::ContactCard {
        be_block_client::contacts::card(
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
    code: be_block_keys::InviteCode,
) -> Result<OrganizationInvitation, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    Ok(be_block_client::contacts::invitation(&connection, &code)
        .await?
        .0)
}

pub async fn accept_invitation(
    server_url: String,
    token: String,
    me: Me,
    code: be_block_keys::InviteCode,
) -> Result<Uuid, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    let invitation =
        be_block_client::contacts::accept_invitation(&connection, &me.secret(), &me.card(), &code)
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

impl AgentCredentials {
    /// The JSON object `be` reads from `BE3_AGENT_CREDENTIALS`. With `proxy` it holds no token
    /// but `"authentication": "proxy"`, for a proxy that adds the token itself
    /// (guides/hosting.md, Proxy account credentials).
    pub fn bundle(&self, proxy: bool) -> String {
        let mut bundle = serde_json::json!({
            "server": self.server,
            "organization": self.organization,
            "recovery_phrase": self.phrase,
        });
        match proxy {
            true => bundle["authentication"] = "proxy".into(),
            false => bundle["token"] = self.token.clone().into(),
        }
        bundle.to_string()
    }
}

pub async fn agents(
    server_url: String,
    token: String,
    organization: Uuid,
) -> Result<Vec<be_block_protocol::Agent>, AccountError> {
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
    pub recovery: Option<be_block_keys::RecoveryPublic>,
    pub sealed: Vec<be_block_protocol::SealedKey>,
    pub own: Vec<be_block_protocol::OwnSealed>,
}

pub async fn keys(server_url: String, token: String) -> Result<Keys, AccountError> {
    let crate::link::Link {
        connection,
        account,
        ..
    } = crate::link::signed_in(&server_url, &token, &be_block_client::Clock::real()).await?;
    let be_block_protocol::RecoveryKey {
        public,
        sealed,
        own,
    } = ask(client::keys::get_recovery_key(&connection)).await?;
    Ok(Keys {
        account,
        recovery: public.map(be_block_keys::RecoveryPublic),
        sealed,
        own,
    })
}

pub async fn set_recovery_key(
    server_url: String,
    token: String,
    public: be_block_keys::RecoveryPublic,
    resealed: be_block_client::contacts::NewIdentity,
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
    let (pairing, message) = be_block_keys::Pairing::start(&code, account.as_bytes());
    let payload = be_block_keys::PairingMessage::Request { message, device }.encode();
    ask(client::account::pair(&connection, None, payload)).await?;
    let reply = async {
        while let Some(heard) = paired.recv().await {
            if let Ok(be_block_keys::PairingMessage::Reply { message, sealed }) =
                be_block_keys::PairingMessage::decode(&heard.payload)
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

pub fn authorize(saved: &[be_host_base::host_state::SavedAccount]) {
    for account in saved {
        if let be_host_base::host_state::ServerLocation::Remote(url) = &account.server {
            be_block_client::authorize(url, account.authorization.clone());
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

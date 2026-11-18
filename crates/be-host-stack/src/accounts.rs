use be_block_client::{ClientError, Connection, ask};
use be_block_protocol::{Authenticated, ErrorCode, Invitation, LinkKind, Organization, client};
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
        ..
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

/// Signs in with a token the server issued another way: a test's server hands one to each
/// device of an account (crates/be-host-tests).
pub async fn token_login(server_url: String, token: String) -> Result<Session, AccountError> {
    let (connection, authenticated) = crate::link::connect(&server_url, &token).await?;
    let mut session = session_of(authenticated);
    session.token = token;
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
            client::server::device_login(peer, be_block_protocol::DeviceLoginStep::Begin)
        })
        .await
        .map_err(|error| error.to_string())?;
        let be_block_protocol::DeviceLoginAnswer::Begun(begun) = begun else {
            return Err("The server answered a sign-in with something else.".into());
        };
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
                client::server::device_login(
                    peer,
                    be_block_protocol::DeviceLoginStep::Poll { code, secret },
                )
            })
            .await
            .and_then(|answer| match answer {
                be_block_protocol::DeviceLoginAnswer::Polled(state) => Ok(state),
                be_block_protocol::DeviceLoginAnswer::Begun(_) => Err(ClientError::Unexpected),
            });
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

/// The server's notices in a mailbox, by message id, oldest first.
pub fn notices(mailbox: Vec<be_block_protocol::Message>) -> Vec<(u64, String)> {
    mailbox
        .into_iter()
        .filter_map(|message| match (message.from, message.letter) {
            (None, be_block_protocol::Letter::Notice(text)) => Some((message.id, text)),
            _ => None,
        })
        .collect()
}

/// The server's notices in the account's mailbox (`query.mailbox`), by message id, whether or not
/// an organization is open.
pub async fn account_notices(
    server_url: String,
    token: String,
) -> Result<Vec<(u64, String)>, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    Ok(notices(
        be_block_client::contacts::mailbox(&connection).await?,
    ))
}

/// Deletes an account notice from the mailbox for the account's every device. The server
/// refuses it in its first week, when only this device hides it
/// (`be_host_base::notices::dismiss`).
pub async fn dismiss_account_notice(
    server_url: String,
    token: String,
    id: u64,
) -> Result<(), AccountError> {
    let connection = authenticated(&server_url, token).await?;
    Ok(ask(be_block_protocol::write::<(), _>(
        &connection,
        None,
        be_block_protocol::Op::DeleteMessage(id),
    ))
    .await?)
}

/// Ends the session `token` names; with `push_address`, this device's push endpoint, the
/// account forgets it in every organization too.
pub async fn logout(
    server_url: String,
    token: String,
    push_address: Option<String>,
) -> Result<(), AccountError> {
    let connection = authenticated(&server_url, token.clone()).await?;
    crate::link::forget(&server_url, &token).await;
    ask(be_block_protocol::write::<(), _>(
        &connection,
        None,
        be_block_protocol::Op::RevokeToken(push_address),
    ))
    .await?;
    Ok(())
}

pub async fn organizations(
    server_url: String,
    token: String,
) -> Result<Vec<Organization>, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    Ok(ask(be_block_protocol::read::<
        Vec<be_block_protocol::Organization>,
        _,
    >(
        &connection, None, be_block_protocol::Query::Organizations
    ))
    .await?)
}

/// Makes a company organization as `me`, who signs the root of its access log.
pub async fn create_organization(
    server_url: String,
    token: String,
    me: Me,
    name: String,
) -> Result<Organization, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    Ok(
        be_block_client::contacts::create_organization(&connection, &me.secret(), me.account, name)
            .await?,
    )
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

/// An invitation into an organization, as the invited account reads it.
#[derive(Clone, Debug)]
pub struct Invited {
    pub organization_name: String,
    pub invitation: Invitation,
    /// Who invited them: the name and email on the inviter's card, opened with the code.
    pub inviter: String,
}

/// How a person is named from their card: their name, and their email when it has one.
fn named(card: &be_block_keys::ContactCard) -> String {
    match card.email.is_empty() {
        true => card.display_name.clone(),
        false => format!("{} ({})", card.display_name, card.email),
    }
}

/// The invitation `code` opens; a contact link's code is refused as not one
/// (`contacts::Expected`).
pub async fn invitation(
    server_url: String,
    token: String,
    code: be_block_keys::InviteCode,
) -> Result<Invited, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    let (link, card) = be_block_client::contacts::open_link(
        &connection,
        &code,
        be_block_client::contacts::Expected::Invitation,
    )
    .await?;
    let organization_name = link.organization_name.clone();
    Ok(Invited {
        organization_name,
        invitation: invitation_of(link)?,
        inviter: named(&card),
    })
}

/// An invitation as anyone holding its link reads it before signing in: whose it is, into what,
/// and where it stands; an ended one names neither, so both are empty.
#[derive(Clone, Debug, PartialEq)]
pub struct InvitationPreview {
    pub organization_name: String,
    pub inviter: String,
    pub state: be_block_protocol::InvitationState,
}

/// Reads the invitation `code` opens at `server_url`, with no sign-in (a one-shot call).
pub async fn preview_invitation(
    server_url: String,
    code: be_block_keys::InviteCode,
) -> Result<InvitationPreview, AccountError> {
    let (preview, card) = be_block_client::contacts::preview_invitation(&server_url, &code).await?;
    Ok(InvitationPreview {
        organization_name: preview.organization_name,
        inviter: card.as_ref().map(named).unwrap_or_default(),
        state: preview.state,
    })
}

/// Accepts the invitation `code` opens, refusing a contact link's; answers the organization
/// joined.
pub async fn accept_invitation(
    server_url: String,
    token: String,
    me: Me,
    code: be_block_keys::InviteCode,
) -> Result<Uuid, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    let (link, _) = be_block_client::contacts::accept_link(
        &connection,
        &me.secret(),
        &me.card(),
        &code,
        be_block_client::contacts::Expected::Invitation,
    )
    .await?;
    Ok(invitation_of(link)?.organization)
}

/// The invitation a link opened as one carries.
fn invitation_of(link: be_block_protocol::Link) -> Result<Invitation, AccountError> {
    match link.kind {
        LinkKind::Invitation(invitation) => Ok(invitation),
        LinkKind::Contact(_) => Err(AccountError {
            message: "That is a contact link, not an invitation: open it from Contacts.".into(),
            invalid_token: false,
        }),
    }
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
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

#[derive(Clone, Debug, Default)]
pub struct Keys {
    pub account: Uuid,
    pub recovery: Option<be_block_keys::RecoveryPublic>,
    pub sealed: Vec<be_block_protocol::SealedKey>,
    pub logs: Vec<be_block_protocol::SignedLog>,
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
        logs,
    } = ask(
        be_block_protocol::read::<be_block_protocol::RecoveryKey, _>(
            &connection,
            None,
            be_block_protocol::Query::RecoveryKey,
        ),
    )
    .await?;
    Ok(Keys {
        account,
        recovery: public.map(be_block_keys::RecoveryPublic),
        sealed,
        logs,
    })
}

/// Moves the account to `fresh` (`be_block_client::contacts::set_recovery_key`), resealing with
/// `old` everything sealed to it or for itself; with no `old`, only an account with nothing
/// sealed yet. The contacts' waiting key changes are followed first, checked against the
/// device's `pins`.
pub async fn set_recovery_key(
    server_url: String,
    token: String,
    account: Uuid,
    old: Option<be_block_keys::RecoverySecret>,
    fresh: be_block_keys::RecoverySecret,
    pins: std::collections::HashMap<Uuid, be_block_client::contacts::OwnContact>,
) -> Result<(), AccountError> {
    let connection = authenticated(&server_url, token).await?;
    be_block_client::contacts::set_recovery_key(&connection, account, old.as_ref(), &fresh, &pins)
        .await?
        .map_err(|_| AccountError {
            message: CANNOT_RESEAL.to_owned(),
            invalid_token: false,
        })
}

/// Why a new phrase can't be taken on a device without the old one.
pub const CANNOT_RESEAL: &str = "This device cannot open what the old phrase sealed. Unlock this \
     account with the old phrase first, so the new one can seal it.";

pub async fn pair(
    server_url: String,
    token: String,
    account: Uuid,
    code: String,
    device: String,
    cancelled: futures_util::future::BoxFuture<'static, ()>,
) -> Result<[u8; 32], AccountError> {
    let connection = authenticated(&server_url, token).await?;
    let mut paired = client::server::events(&connection);
    let (pairing, message) = be_block_keys::Pairing::start(&code, account.as_bytes());
    let payload = be_block_keys::PairingMessage::Request { message, device }.encode();
    ask(be_block_protocol::client::server::send(
        &connection,
        None,
        be_block_protocol::Target::Devices(None),
        payload,
    ))
    .await?;
    let reply = async {
        while let Some(heard) = paired.recv().await {
            let be_block_protocol::Event::Paired(heard) = heard else {
                continue;
            };
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

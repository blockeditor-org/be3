use be_client::{ClientError, Connection};
use be_protocol::{ClientMessage, ErrorCode, Organization, OrganizationInvitation, ServerMessage};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) struct Session {
    pub(crate) account: Uuid,
    pub(crate) email: String,
    pub(crate) display_name: String,
    pub(crate) token: String,
}

#[derive(Debug)]
pub(crate) struct AccountError {
    pub(crate) message: String,
    pub(crate) invalid_token: bool,
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

pub(crate) use be_client::socket_url;

pub(crate) fn link_base(server_url: &str) -> String {
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

pub(crate) async fn register(
    server_url: String,
    email: String,
    display_name: String,
    password: String,
) -> Result<Session, AccountError> {
    let connection = Connection::connect(&socket_url(&server_url)).await?;
    let response = connection
        .request(|request| ClientMessage::Register {
            request,
            email,
            display_name,
            password,
        })
        .await?;
    session_of(response)
}

pub(crate) async fn login(
    server_url: String,
    email: String,
    password: String,
) -> Result<Session, AccountError> {
    let connection = Connection::connect(&socket_url(&server_url)).await?;
    let response = connection
        .request(|request| ClientMessage::Login {
            request,
            email,
            password,
        })
        .await?;
    session_of(response)
}

async fn authenticated(
    server_url: &str,
    token: String,
) -> Result<std::sync::Arc<Connection>, AccountError> {
    let connection = Connection::connect(&socket_url(server_url)).await?;
    connection
        .request(|request| ClientMessage::Authenticate { request, token })
        .await?;
    Ok(connection)
}

pub(crate) async fn logout(server_url: String, token: String) -> Result<(), AccountError> {
    let connection = authenticated(&server_url, token).await?;
    connection
        .request(|request| ClientMessage::Logout { request })
        .await?;
    Ok(())
}

pub(crate) async fn organizations(
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

pub(crate) async fn create_organization(
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
pub(crate) struct Me {
    pub(crate) account: Uuid,
    pub(crate) identity: [u8; 32],
    pub(crate) display_name: String,
    pub(crate) email: String,
}

impl Me {
    pub(crate) fn secret(&self) -> be_keys::RecoverySecret {
        be_keys::RecoverySecret::from_bytes(self.identity)
    }

    pub(crate) fn card(&self) -> be_keys::ContactCard {
        be_client::contacts::card(
            self.account,
            &self.secret(),
            &self.display_name,
            &self.email,
        )
    }
}

pub(crate) async fn invitation(
    server_url: String,
    token: String,
    code: be_keys::InviteCode,
) -> Result<OrganizationInvitation, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    Ok(be_client::contacts::invitation(&connection, &code).await?.0)
}

pub(crate) async fn accept_invitation(
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

pub(crate) async fn invite(
    server_url: String,
    token: String,
    me: Me,
    organization: Uuid,
    email: String,
    role: be_protocol::OrganizationRole,
) -> Result<String, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    let escrow =
        be_client::contacts::verified_escrow(&connection, &me.secret(), organization).await?;
    let code = be_client::contacts::invite(
        &connection,
        &me.secret(),
        &me.card(),
        organization,
        email,
        role,
        escrow.as_ref(),
    )
    .await?;
    Ok(crate::invite_link::link(&link_base(&server_url), &code))
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AgentCredentials {
    pub(crate) name: String,
    pub(crate) server: String,
    pub(crate) organization: Uuid,
    pub(crate) token: String,
    pub(crate) phrase: String,
}

pub(crate) async fn create_agent(
    server_url: String,
    token: String,
    me: Me,
    organization: Uuid,
    name: String,
) -> Result<AgentCredentials, AccountError> {
    let phrase = be_keys::RecoveryPhrase::generate();
    let secret = phrase.secret();
    let public = secret.public();
    let connection = authenticated(&server_url, token).await?;
    let escrow =
        be_client::contacts::verified_escrow(&connection, &me.secret(), organization).await?;
    let ServerMessage::AgentCreated { agent, token, .. } = connection
        .request(|request| ClientMessage::CreateAgent {
            request,
            organization,
            name,
            recovery: public.0,
            contact: me.card().seal_own(&secret),
            escrow: escrow.map_or_else(Vec::new, |escrow| {
                be_client::contacts::seal_pin(&secret, organization, &escrow)
            }),
        })
        .await?
    else {
        return Err(ClientError::Unexpected.into());
    };
    let card = be_keys::ContactCard {
        account: agent.account.into_bytes(),
        public,
        display_name: agent.name.clone(),
        email: agent.email.clone(),
    };
    connection
        .request(|request| ClientMessage::PutContact {
            request,
            contact: agent.account,
            sealed: card.seal_own(&me.secret()),
        })
        .await?;
    Ok(AgentCredentials {
        name: agent.name,
        server: server_url,
        organization,
        token,
        phrase: phrase.to_string(),
    })
}

pub(crate) async fn agents(
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

pub(crate) async fn remove_agent(
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
pub(crate) struct Keys {
    pub(crate) account: Uuid,
    pub(crate) recovery: Option<be_keys::RecoveryPublic>,
    pub(crate) sealed: Vec<be_protocol::SealedKey>,
    pub(crate) own: Vec<be_protocol::OwnSealed>,
}

pub(crate) async fn keys(server_url: String, token: String) -> Result<Keys, AccountError> {
    let connection = Connection::connect(&socket_url(&server_url)).await?;
    let ServerMessage::Authenticated { account, .. } = connection
        .request(|request| ClientMessage::Authenticate { request, token })
        .await?
    else {
        return Err(ClientError::Unexpected.into());
    };
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

pub(crate) async fn set_recovery_key(
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

pub(crate) async fn pair(
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

pub(crate) fn authorize(saved: &[crate::app_state::SavedAccount]) {
    for account in saved {
        if let crate::app_state::ServerLocation::Remote(url) = &account.server {
            be_client::authorize(url, account.authorization.clone());
        }
    }
}

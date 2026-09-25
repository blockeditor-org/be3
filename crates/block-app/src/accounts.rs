use be_client::{ClientError, Connection};
use be_protocol::{ClientMessage, ErrorCode, ServerMessage, Workspace, WorkspaceInvitation};
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

pub(crate) fn socket_url(server_url: &str) -> String {
    let base = match server_url.split_once("://") {
        Some(("http", host)) => format!("ws://{host}"),
        Some(("https", host)) => format!("wss://{host}"),
        _ => server_url.to_owned(),
    };
    format!("{}/api/be", base.trim_end_matches('/'))
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

pub(crate) async fn workspaces(
    server_url: String,
    token: String,
) -> Result<(Vec<Workspace>, Vec<WorkspaceInvitation>), AccountError> {
    let connection = authenticated(&server_url, token).await?;
    let ServerMessage::Workspaces { workspaces, .. } = connection
        .request(|request| ClientMessage::ListWorkspaces { request })
        .await?
    else {
        return Err(ClientError::Unexpected.into());
    };
    let ServerMessage::Invitations { invitations, .. } = connection
        .request(|request| ClientMessage::ListInvitations { request })
        .await?
    else {
        return Err(ClientError::Unexpected.into());
    };
    Ok((workspaces, invitations))
}

pub(crate) async fn create_workspace(
    server_url: String,
    token: String,
    name: String,
) -> Result<Workspace, AccountError> {
    let connection = authenticated(&server_url, token).await?;
    match connection
        .request(|request| ClientMessage::CreateWorkspace { request, name })
        .await?
    {
        ServerMessage::Workspaces { mut workspaces, .. } if !workspaces.is_empty() => {
            Ok(workspaces.remove(0))
        }
        _ => Err(ClientError::Unexpected.into()),
    }
}

pub(crate) async fn respond_invitation(
    server_url: String,
    token: String,
    invitation: Uuid,
    accept: bool,
) -> Result<(), AccountError> {
    let connection = authenticated(&server_url, token).await?;
    connection
        .request(|request| ClientMessage::RespondInvitation {
            request,
            invitation,
            accept,
        })
        .await?;
    Ok(())
}

pub(crate) async fn invite(
    server_url: String,
    token: String,
    workspace: Uuid,
    email: String,
    role: be_protocol::WorkspaceRole,
) -> Result<(), AccountError> {
    let connection = authenticated(&server_url, token).await?;
    connection
        .request(|request| ClientMessage::Invite {
            request,
            workspace,
            email,
            role,
        })
        .await?;
    Ok(())
}

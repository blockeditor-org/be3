mod debug;
mod dialogs;
mod onboarding;
mod picker;
mod share;
mod tools;
mod workspace;

use std::cell::RefCell;

use be_protocol::WorkspaceRole;
use beui::reactive::{Dynamic, Frame, List, Store, component, view};
use beui::styled::use_theme;
use beui::{ItemSize, NodeId};
use uuid::Uuid;

use crate::app_state::{SavedAccount, ServerLocation};
use crate::block_picker::{PickerCommand, PickerView};
use crate::share::{ShareCommand, ShareView};

pub(crate) use debug::{
    DebugCommand, DebugView, DebugWindow, Line, LineStyle, PerformanceRow, PluginsView, RunView,
    RuntimeView, VersionRuns, VersionView,
};

thread_local! {
    static COMMANDS: RefCell<Vec<UiCommand>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn send(command: UiCommand) {
    COMMANDS.with(|commands| commands.borrow_mut().push(command));
}

pub(crate) fn take_commands() -> Vec<UiCommand> {
    COMMANDS.with(|commands| std::mem::take(&mut *commands.borrow_mut()))
}

pub(crate) fn account_key(account: &SavedAccount) -> String {
    format!("{}\u{0}{}", account.server.key(), account.id)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Screen {
    Error,
    #[default]
    Accounts,
    Workspaces,
    Workspace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ErrorAction {
    DeleteClientDatabase,
    #[cfg(not(target_arch = "wasm32"))]
    DeleteServerDatabase,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ErrorView {
    pub(crate) message: String,
    pub(crate) pending: Option<ErrorAction>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AccountRow {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) email: String,
    pub(crate) server: String,
    pub(crate) local: bool,
    pub(crate) has_last_workspace: bool,
    pub(crate) current: bool,
}

impl AccountRow {
    pub(crate) fn of(account: &SavedAccount, current: bool) -> Self {
        Self {
            key: account_key(account),
            name: account.name.clone(),
            email: account.email.clone(),
            server: match &account.server {
                ServerLocation::Local => "Local server".to_owned(),
                ServerLocation::Remote(url) => url.clone(),
            },
            local: account.server == ServerLocation::Local,
            has_last_workspace: account.last_workspace_id.is_some(),
            current,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AddAccountView {
    pub(crate) open: bool,
    pub(crate) generation: u64,
    pub(crate) pending: bool,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct AccountForm {
    pub(crate) register: bool,
    pub(crate) remote: bool,
    pub(crate) remote_url: String,
    pub(crate) email: String,
    pub(crate) display_name: String,
    pub(crate) password: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum WorkspacesState {
    #[default]
    Loading,
    Loaded,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InvitationRow {
    pub(crate) id: Uuid,
    pub(crate) workspace: String,
    pub(crate) role: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct WorkspacesView {
    pub(crate) state: WorkspacesState,
    pub(crate) workspaces: Vec<(Uuid, String)>,
    pub(crate) invitations: Vec<InvitationRow>,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
    pub(crate) created: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ReauthView {
    pub(crate) email: String,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct StatusView {
    pub(crate) changes_saved: bool,
    pub(crate) frame: String,
    pub(crate) workspace: String,
    pub(crate) signed_in_as: String,
    pub(crate) accounts: Vec<AccountRow>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct InviteView {
    pub(crate) workspace: String,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
    pub(crate) sent: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DiscardView {
    pub(crate) title: String,
    pub(crate) message: String,
    pub(crate) button: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RenameView {
    pub(crate) id: Uuid,
    pub(crate) name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ArtifactSettingsView {
    pub(crate) id: Uuid,
    pub(crate) changed: bool,
    pub(crate) summary: Option<String>,
}

#[derive(Clone, Default, PartialEq, Store)]
pub(crate) struct AppView {
    pub(crate) screen: Screen,
    pub(crate) error: ErrorView,
    pub(crate) accounts: Vec<AccountRow>,
    pub(crate) account_error: Option<String>,
    pub(crate) add_account: AddAccountView,
    pub(crate) account: AccountRow,
    pub(crate) workspaces: WorkspacesView,
    pub(crate) reauth: Option<ReauthView>,
    pub(crate) status: StatusView,
    pub(crate) invite: Option<InviteView>,
    pub(crate) about: bool,
    pub(crate) discard: Option<DiscardView>,
    pub(crate) rename: Option<RenameView>,
    pub(crate) artifact_settings: Option<ArtifactSettingsView>,
    pub(crate) unlink: bool,
    pub(crate) share: Option<ShareView>,
    pub(crate) pickers: Vec<PickerView>,
    pub(crate) presenting: bool,
    pub(crate) debug: DebugView,
}

#[derive(Clone, Debug)]
pub(crate) enum UiCommand {
    Restart,
    AskErrorAction(ErrorAction),
    ConfirmErrorAction,
    CancelErrorAction,
    Exit,
    OpenAccount(String),
    ChooseWorkspace(String),
    LogOut(String),
    OpenAddAccount,
    SubmitAccount(AccountForm),
    CloseAddAccount,
    ReloadWorkspaces,
    OpenWorkspace(Uuid),
    RespondInvitation(Uuid, bool),
    CreateWorkspace(String),
    SwitchAccount,
    LogOutCurrent,
    ReauthSubmit(String),
    ReauthLogOut,
    ReauthClose,
    OpenSettings,
    OpenInspector,
    InviteMember,
    SwitchWorkspace,
    SwitchTo(String),
    ManageAccounts,
    About(bool),
    SendInvite(String, WorkspaceRole),
    CloseInvite,
    Discard,
    CancelDiscard,
    SubmitRename(String),
    CancelRename,
    ApplyArtifactSettings,
    CancelArtifactSettings,
    Unlink,
    CancelUnlink,
    Share(ShareCommand),
    Picker(PickerCommand),
    Debug(DebugCommand),
}

pub(crate) fn root(view: AppViewStore) -> NodeId {
    view! {
        <Root view />
    }
}

#[component]
fn Root(view: AppViewStore) -> NodeId {
    let theme = use_theme();
    let screen = view.screen.clone();
    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=0.0>
                <Dynamic value={screen}>
                    {move |screen: Screen| {
                        let view = view.clone();
                        match screen {
                            Screen::Error => view! {
                                <onboarding::ErrorScreen @sizing=ItemSize::Percent(100.0) view />
                            },
                            Screen::Accounts => view! {
                                <onboarding::AccountsScreen @sizing=ItemSize::Percent(100.0) view />
                            },
                            Screen::Workspaces => view! {
                                <onboarding::WorkspacesScreen
                                    @sizing=ItemSize::Percent(100.0)
                                    view
                                />
                            },
                            Screen::Workspace => view! {
                                <workspace::WorkspaceScreen @sizing=ItemSize::Percent(100.0) view />
                            },
                        }
                    }}
                </Dynamic>
            </List>
        </Frame>
    }
}

mod debug;
mod dialogs;
mod keys;
mod lock;
mod onboarding;
mod workspace;

use std::cell::RefCell;

use be_protocol::WorkspaceRole;
use beui::reactive::{Dynamic, Frame, List, NodeRef, Store, component, view};
use beui::styled::{KeepChanges, Toast, Toasts, use_theme};
use beui::{ItemSize, NodeId};
use block_plugin_api::HostPanel;
use uuid::Uuid;

use crate::app_state::{SavedAccount, ServerLocation};

pub(crate) use debug::{
    DebugCommand, DebugView, HostPanelSurface, Line, LineStyle, PerformanceRow, PluginsView,
    RunView, RuntimeView, VersionRuns, VersionView,
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
    Recovery,
    Unlock,
    Profiles,
    Workspace,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PairingRow {
    pub(crate) from: u64,
    pub(crate) device: String,
    pub(crate) workspace: String,
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
    pub(crate) unsaved: usize,
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
    pub(crate) workspace: String,
    pub(crate) signed_in_as: String,
    pub(crate) profiles: Vec<ProfileRow>,
    pub(crate) profiles_loaded: bool,
    pub(crate) every_profile_type: bool,
    pub(crate) session_type: Uuid,
    pub(crate) can_close: bool,
    pub(crate) runs_programs: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProfileRow {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) kind: Option<String>,
    pub(crate) current: bool,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum LockCover {
    #[default]
    Blank,
    Desktop,
    BuiltIn,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LockView {
    pub(crate) available: bool,
    pub(crate) locked: bool,
    pub(crate) cover: LockCover,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
    pub(crate) user: String,
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
    pub(crate) recovery: crate::keys::RecoveryView,
    pub(crate) unlock: crate::keys::UnlockView,
    pub(crate) pairing: Vec<PairingRow>,
    pub(crate) reauth: Option<ReauthView>,
    pub(crate) status: StatusView,
    pub(crate) invite: Option<InviteView>,
    pub(crate) about: bool,
    pub(crate) launcher: bool,
    pub(crate) programs: std::rc::Rc<Vec<beui::styled::LauncherItem>>,
    pub(crate) app_menu: bool,
    pub(crate) discard: Option<DiscardView>,
    pub(crate) presenting: bool,
    pub(crate) debug: DebugView,
    pub(crate) toasts: Vec<Toast>,
    pub(crate) keep_display: bool,
    pub(crate) lock: LockView,
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
    ChooseProfile(Uuid),
    OpenProfile(Uuid),
    OpenNewProfile(Uuid),
    EveryProfileType(bool),
    RespondInvitation(Uuid, bool),
    CreateWorkspace(String),
    SwitchAccount,
    LogOutCurrent,
    ReauthSubmit(String),
    ReauthLogOut,
    ReauthClose,
    OpenInspector,
    InviteMember,
    SwitchWorkspace,
    ManageAccounts,
    About(bool),
    CloseApp,
    Launcher(bool),
    LaunchProgram(String),
    Launch(String),
    AppMenu(bool),
    SendInvite(String, WorkspaceRole),
    CloseInvite,
    Discard,
    CancelDiscard,
    Debug(DebugCommand),
    ShowPanel(HostPanel),
    ConfirmRecovery(Vec<String>),
    NewRecoveryPhrase,
    CancelRecovery,
    UnlockWithPhrase(String),
    StartPairing,
    CancelPairing,
    ApprovePairing(u64, String),
    DismissPairing(u64),
    DismissToast(u64),
    ToastAction(u64, String),
    ActivateToast(u64),
    KeepDisplay,
    RevertDisplay,
    LockScreen,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Unlock(crate::password::Password),
}

#[component]
pub(crate) fn Root(view: AppViewStore) -> NodeId {
    let theme = use_theme();
    let screen = view.screen.clone();
    let toasts = view.toasts.clone();
    let keep_display = view.keep_display.clone();
    let locking = view.clone();
    let area = NodeRef::new();
    view! {
        <Frame @node_ref=&area color={theme.background.clone()}>
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
                            Screen::Recovery => view! {
                                <keys::RecoveryScreen @sizing=ItemSize::Percent(100.0) view />
                            },
                            Screen::Unlock => view! {
                                <keys::UnlockScreen @sizing=ItemSize::Percent(100.0) view />
                            },
                            Screen::Profiles => view! {
                                <onboarding::ProfilesScreen @sizing=ItemSize::Percent(100.0) view />
                            },
                            Screen::Workspace => view! {
                                <workspace::WorkspaceScreen @sizing=ItemSize::Percent(100.0) view />
                            },
                        }
                    }}
                </Dynamic>
                <Toasts
                    anchor={area.clone()}
                    toasts={toasts}
                    on_dismiss={move |id: u64| send(UiCommand::DismissToast(id))}
                    on_action={move |(id, action): (u64, String)| send(UiCommand::ToastAction(id, action))}
                    on_activate={move |id: u64| send(UiCommand::ActivateToast(id))}
                />
                <KeepChanges
                    open={keep_display}
                    title="Keep these display settings?"
                    id="display.keep"
                    on_keep={|| send(UiCommand::KeepDisplay)}
                    on_revert={|| send(UiCommand::RevertDisplay)}
                />
                <lock::ScreenCover view={locking} />
            </List>
        </Frame>
    }
}

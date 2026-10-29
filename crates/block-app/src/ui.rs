mod debug;
mod dialogs;
mod keep_changes;
mod keys;
mod lock;
mod onboarding;
mod organization;
mod saving;

use std::cell::RefCell;

use be_protocol::OrganizationRole;
use beui::reactive::*;
use beui::styled::*;
use beui::{ItemSize, NodeId};
use block_plugin_api::HostPanel;
use uuid::Uuid;

use crate::compositor::{PluginLayers, ShellDisplay};
use block_app_base::app_state::{SavedAccount, ServerLocation};
use block_app_desktop::desktop_host::WindowActions;

pub(crate) use debug::{
    DebugCommand, DebugView, HostPanelSurface, Line, LineStyle, PluginsView, RuntimeView,
};

thread_local! {
    static COMMANDS: RefCell<Vec<UiCommand>> = const { RefCell::new(Vec::new()) };
}

const COPY_REPORT: &str = "copy-report";

pub(crate) fn notice_toasts() -> Vec<Toast> {
    block_app_base::notices::shown()
        .into_iter()
        .map(|notice| Toast {
            id: notice.id,
            message: notice.message,
            danger: true,
            actions: notice
                .report
                .iter()
                .map(|_| ToastAction {
                    key: COPY_REPORT.to_owned(),
                    label: "Copy report".to_owned(),
                })
                .collect(),
            ..Toast::default()
        })
        .collect()
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
    Organizations,
    Recovery,
    Unlock,
    Sessions,
    Organization,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PairingRow {
    pub(crate) from: u64,
    pub(crate) device: String,
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

pub(crate) use block_accounts_ui::AccountRow;

pub(crate) fn account_row(
    account: &SavedAccount,
    current: bool,
    companies: Vec<String>,
) -> AccountRow {
    AccountRow {
        key: account_key(account),
        name: account.name.clone(),
        email: account.email.clone(),
        server: match &account.server {
            ServerLocation::Local => "Local server".to_owned(),
            ServerLocation::Remote(url) => url.clone(),
        },
        local: account.server == ServerLocation::Local,
        has_last_organization: account.last_organization_id.is_some(),
        current,
        companies,
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AddAccountView {
    pub(crate) browser: Option<block_app_stack::accounts::BrowserLogin>,
    pub(crate) open: bool,
    pub(crate) generation: u64,
    pub(crate) pending: bool,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct AccountForm {
    pub(crate) remote: bool,
    pub(crate) remote_url: String,
    pub(crate) authorization: String,
    pub(crate) display_name: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum OrganizationsState {
    #[default]
    Loading,
    Loaded,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct JoiningRow {
    pub(crate) organization: String,
    pub(crate) role: String,
    pub(crate) email: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OrganizationsView {
    pub(crate) state: OrganizationsState,
    pub(crate) organizations: Vec<(Uuid, String)>,
    pub(crate) joining: Option<JoiningRow>,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
    pub(crate) created: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ReauthView {
    pub(crate) name: String,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum SyncState {
    #[default]
    Saved,
    Connecting,
    Syncing,
    Offline {
        waiting: usize,
    },
    Error(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct UnsentRow {
    pub(crate) block: Uuid,
    pub(crate) name: String,
    pub(crate) offerable: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct UnsentView {
    pub(crate) rows: Vec<UnsentRow>,
    pub(crate) discarding: Option<Uuid>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct StatusView {
    pub(crate) sync: SyncState,
    pub(crate) unsaved: bool,
    pub(crate) unsent: usize,
    pub(crate) saves: u64,
    pub(crate) organization: String,
    pub(crate) signed_in_as: String,
    pub(crate) sessions: Vec<SessionRow>,
    pub(crate) profiles: Vec<ProfileRow>,
    pub(crate) settings_loaded: bool,
    pub(crate) every_session_type: bool,
    pub(crate) session_type: Uuid,
    pub(crate) can_close: bool,
    pub(crate) company: bool,
    pub(crate) administrator: bool,
    pub(crate) escrow_warning: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SessionRow {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) kind: Option<String>,
    pub(crate) current: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProfileRow {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) current: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct InviteView {
    pub(crate) organization: String,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
    pub(crate) sent: u64,
    pub(crate) link: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ContactRow {
    pub(crate) account: Uuid,
    pub(crate) name: String,
    pub(crate) email: String,
    pub(crate) verified: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ContactsView {
    pub(crate) contacts: Vec<ContactRow>,
    pub(crate) link: Option<String>,
    pub(crate) offer: Option<String>,
    pub(crate) connected: Option<String>,
    pub(crate) removing: Option<String>,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct MemberRow {
    pub(crate) account: Uuid,
    pub(crate) name: String,
    pub(crate) detail: String,
    pub(crate) administrator: bool,
    pub(crate) me: bool,
    pub(crate) agent: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct MembersView {
    pub(crate) members: Vec<MemberRow>,
    pub(crate) removing: Option<String>,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AgentRow {
    pub(crate) account: Uuid,
    pub(crate) name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CreatedAgent {
    pub(crate) name: String,
    pub(crate) server: String,
    pub(crate) organization: String,
    pub(crate) token: String,
    pub(crate) phrase: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AgentsView {
    pub(crate) organization: String,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
    pub(crate) agents: Vec<AgentRow>,
    pub(crate) created: Option<CreatedAgent>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AppMenuView {
    pub(crate) opened: u64,
    pub(crate) at: Option<beui::Rect>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DiscardView {
    pub(crate) title: String,
    pub(crate) message: String,
    pub(crate) button: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LockView {
    pub(crate) available: bool,
    pub(crate) locked: bool,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
    pub(crate) user: String,
    pub(crate) power: Vec<block_plugin_api::PowerAction>,
}

#[derive(Clone, Default, PartialEq, Store)]
pub(crate) struct AppView {
    pub(crate) screen: Screen,
    pub(crate) error: ErrorView,
    pub(crate) accounts: Vec<AccountRow>,
    pub(crate) account_error: Option<String>,
    pub(crate) add_account: AddAccountView,
    pub(crate) account: AccountRow,
    pub(crate) organizations: OrganizationsView,
    pub(crate) recovery: block_app_stack::keys::RecoveryView,
    pub(crate) unlock: block_app_stack::keys::UnlockView,
    pub(crate) pairing: Vec<PairingRow>,
    pub(crate) reauth: Option<ReauthView>,
    pub(crate) status: StatusView,
    pub(crate) invite: Option<InviteView>,
    pub(crate) agents: Option<AgentsView>,
    pub(crate) contacts: Option<ContactsView>,
    pub(crate) members: Option<MembersView>,
    pub(crate) about: bool,
    pub(crate) unsent: Option<UnsentView>,
    pub(crate) app_menu: Option<AppMenuView>,
    pub(crate) discard: Option<DiscardView>,
    pub(crate) leaving: bool,
    pub(crate) presenting: bool,
    pub(crate) debug: DebugView,
    pub(crate) toasts: Vec<Toast>,
    pub(crate) keep_display: Option<u64>,
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
    ChooseOrganization(String),
    LogOut(String),
    OpenAddAccount,
    SubmitAccount(AccountForm),
    OpenLoginBrowser,
    RetryAccount,
    CloseAddAccount,
    ReloadOrganizations,
    OpenOrganization(Uuid),
    ChooseSession(Uuid),
    SwitchSession,
    CloseSessions,
    OpenSession(Uuid),
    OpenNewSession(Uuid),
    EverySessionType(bool),
    UseProfile(Uuid),
    NewProfile,
    OpenInvite(String),
    JoinInvited,
    CancelJoin,
    CreateOrganization(String),
    SwitchAccount,
    LogOutCurrent,
    ReauthSubmit,
    ReauthLogOut,
    ReauthClose,
    OpenInspector,
    InviteMember,
    SwitchOrganization,
    ManageAccounts,
    About(bool),
    Unsent(bool),
    RestoreUnsent(Uuid),
    DiscardUnsent(Uuid),
    ConfirmDiscardUnsent,
    CancelDiscardUnsent,
    CloseApp,
    AppMenu(bool),
    SendInvite(String, OrganizationRole),
    CloseInvite,
    OpenAgents,
    CloseAgents,
    OpenContacts,
    CloseContacts,
    NewContactLink,
    OpenContactLink(String),
    AcceptContact,
    DeclineContact,
    ConfirmLeave,
    CancelLeave,
    RemoveContact(Uuid),
    ConfirmRemoveContact,
    CancelRemoveContact,
    OpenMembers,
    CloseMembers,
    SetMemberRole(Uuid, OrganizationRole),
    RemoveMember(Uuid),
    ConfirmRemoveMember,
    CancelRemoveMember,
    CreateAgent(String),
    RemoveAgent(Uuid),
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
    KeepDisplay(u64),
    RevertDisplay(u64),
    LockScreen,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Unlock(block_app_base::password::Password),
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    LockPower(block_plugin_api::PowerAction),
}

#[component]
pub(crate) fn Displays(view: AppViewStore) -> NodeId {
    let shown = crate::display::shown();
    let windowed = create_memo(move || shown.with(|shown| !shown.is_empty()));
    view! {
        <Layers>
            <Dynamic value={windowed}>|windowed: bool| {
                match windowed {
                    false => view! {
                        <Layers>
                            <Root view />
                            <PluginLayers />
                        </Layers>
                    },
                    true => {
                        let keys = create_memo(move || {
                            shown.with(|shown| {
                                shown.iter().map(|display| display.id.clone()).collect::<Vec<_>>()
                            })
                        });
                        view! {
                            <Layers>
                                <ForEach keys={keys}>|id: String|
                                    <DisplayWindow view id />
                                </ForEach>
                            </Layers>
                        }
                    }
                }
            }</Dynamic>
        </Layers>
    }
}

#[component]
fn DisplayWindow(view: AppViewStore, id: String) -> NodeId {
    let shown = crate::display::shown();
    let key = id.clone();
    let display = create_memo(move || {
        shown.with(|shown| shown.iter().find(|display| display.id == key).cloned())
    });
    let key = id.clone();
    let primary = create_memo(move || {
        shown.with(|shown| shown.first().is_some_and(|display| display.id == key))
    });
    let rect = create_memo(move || {
        display
            .get()
            .map_or(beui::Rect::ZERO, |display| display.rect)
    });
    let size = create_memo(move || rect.get().size());
    let placement = display
        .get_untracked()
        .map_or(beui::WindowPlacement::AtPointer, |display| {
            display.placement
        });
    let title = display.get_untracked().map(|display| display.name);
    let index = create_memo({
        let key = id.clone();
        move || {
            shown.with(|shown| {
                shown
                    .iter()
                    .position(|display| display.id == key)
                    .unwrap_or_default()
            })
        }
    });
    let displays = create_memo(move || {
        shown.with(|shown| shown.iter().map(|display| display.rect).collect::<Vec<_>>())
    });
    view! {
        <Window size={size} placement={placement} title={title}>|_handle: WindowHandle| {
            provide_context(crate::compositor::DesktopPlace {
                display: rect,
                displays,
            });
            view! {
                <beui::styled::Root>
                    <Layers>
                        <Dynamic value={primary}>|primary: bool| {
                            match primary {
                                true => view! {
                                    <Root view />
                                },
                                false => view! {
                                    <SecondaryDisplay view index />
                                },
                            }
                        }</Dynamic>
                        <PluginLayers />
                    </Layers>
                </beui::styled::Root>
            }
            .child()
        }</Window>
    }
}

#[component]
fn SecondaryDisplay(view: AppViewStore, index: Memo<usize>) -> NodeId {
    let area = NodeRef::new();
    let screen = view.screen;
    let presenting = view.presenting;
    let shell = crate::compositor::shell();
    let desktop = create_memo(move || {
        screen.get() == Screen::Organization && !presenting.get() && shell.get().is_some()
    });
    view! {
        <Frame @node_ref=&area>
            <List spacing=Space::None>
                <Show condition={desktop}>
                    <Dynamic value={index}>|index: usize| {
                        let display = u32::try_from(index).unwrap_or(u32::MAX);
                        view! {
                            <ShellDisplay
                                @sizing=ItemSize::Percent(100.0)
                                shell={create_memo(move || shell.get())}
                                display
                            />
                        }
                    }</Dynamic>
                </Show>
                <DisplayChrome view area primary=false />
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn Root(view: AppViewStore) -> NodeId {
    let screen = view.screen;
    let area = NodeRef::new();
    view! {
        <Frame @node_ref=&area>
            <List spacing=Space::None>
                <Dynamic value={screen}>|screen: Screen| {
                    match screen {
                        Screen::Error => view! {
                            <onboarding::ErrorScreen @sizing=ItemSize::Percent(100.0) view />
                        },
                        Screen::Accounts => view! {
                            <onboarding::AccountsScreen @sizing=ItemSize::Percent(100.0) view />
                        },
                        Screen::Organizations => view! {
                            <onboarding::OrganizationsScreen
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
                        Screen::Sessions => view! {
                            <onboarding::SessionsScreen @sizing=ItemSize::Percent(100.0) view />
                        },
                        Screen::Organization => view! {
                            <organization::OrganizationScreen
                                @sizing=ItemSize::Percent(100.0)
                                view
                            />
                        },
                    }
                }</Dynamic>
                <onboarding::ReauthDialog view />
                <onboarding::AddAccountDialog view />
                <DisplayChrome view area primary=true />
                <WindowActions />
            </List>
        </Frame>
    }
}

#[component]
fn DisplayChrome(view: AppViewStore, area: NodeRef, primary: bool) -> NodeId {
    let slow_save = saving::SlowSave::new(view);
    let notices = view.toasts;
    let toasts = create_memo(move || slow_save.with_toast(notices.get()));
    let keep_display = view.keep_display;
    let asking = create_memo(move || keep_display.get().is_some());
    let round = create_memo(move || keep_display.get().unwrap_or_default());
    let toasts = create_memo(move || match primary {
        true => toasts.get(),
        false => Vec::new(),
    });
    view! {
        <List spacing=Space::None>
            <Toasts
                anchor={area}
                edge=Edge::TopEnd
                toasts={toasts}
                on_dismiss={move |id: u64| match id {
                    saving::TOAST_ID => slow_save.dismiss(),
                    id => send(UiCommand::DismissToast(id)),
                }}
                on_action={move |(id, key): (u64, String)| {
                    if key == COPY_REPORT
                        && let Some(report) = block_app_base::notices::report_of(id)
                    {
                        copy_text(report);
                    }
                }}
            />
            <keep_changes::KeepChanges
                open={asking}
                title="Keep these display settings?"
                timeout=crate::display::ANSWER_WITHIN
                round={round}
                id="display.keep"
                on_keep={move || send(UiCommand::KeepDisplay(round.get_untracked()))}
                on_revert={move || send(UiCommand::RevertDisplay(round.get_untracked()))}
            />
            <lock::SessionLock view={view} shortcut={primary} />
        </List>
    }
}

#[cfg(test)]
mod tests;

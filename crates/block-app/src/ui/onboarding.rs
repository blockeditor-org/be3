use beui::icons::{
    ICON_ADD, ICON_CHECK, ICON_CLOUD, ICON_COMPUTER, ICON_GROUP_ADD, ICON_KEYBOARD_ARROW_DOWN,
    ICON_MORE_HORIZ, ICON_REFRESH, ICON_SWITCH_ACCOUNT, ICON_WORKSPACES,
};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::{ChoiceOption, MenuItem};
use beui::{NodeId, TextAlign};
use uuid::Uuid;

use super::{AccountForm, AccountRow, AppViewStore, ErrorAction, UiCommand, WorkspacesState, send};
use crate::platform;
use crate::root_settings::{SESSION_TYPES, session_type_name};

const COLUMN_WIDTH: f32 = 460.0;

#[component]
pub(super) fn ErrorText(text: Memo<Option<String>>) -> NodeId {
    let shown = create_memo(move || text.get().is_some());
    let content = create_memo(move || text.get().unwrap_or_default());
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <Text
                    style=TextStyle::PARAGRAPH
                    string={content}
                    color=TextColor::Danger
                    wrap=true
                />
            </Show>
        </List>
    }
}

#[component]
pub(super) fn Column(children: beui::reactive::Children<beui::reactive::ListChild>) -> NodeId {
    view! {
        <Scroll>
            <List spacing=0.0 align=Align::Center>
                <Frame width={Extent::at_most(COLUMN_WIDTH)} padding={Sides::symmetric(16.0, 36.0)}>
                    <List spacing=12.0>{children}</List>
                </Frame>
            </List>
        </Scroll>
    }
}

#[component]
pub(super) fn ErrorScreen(view: AppViewStore) -> NodeId {
    let message = create_memo(move || view.error.get().message);
    let copied = message;
    let pending = create_memo(move || view.error.get().pending);
    let asking = create_memo(move || pending.get().is_some());
    let title = create_memo(move || match pending.get() {
        Some(ErrorAction::DeleteClientDatabase) => "Delete client database?".to_owned(),
        #[cfg(not(target_arch = "wasm32"))]
        Some(ErrorAction::DeleteServerDatabase) => "Delete local server database?".to_owned(),
        None => String::new(),
    });
    let unsaved = create_memo(move || view.error.get().unsaved);
    let confirmation = create_memo(move || {
        match pending.get() {
        Some(ErrorAction::DeleteClientDatabase) => {
            let accounts =
                "This removes every saved account and workspace key on this device. You will need to sign in again, and unlock each workspace with your recovery phrase or another device.";
            match unsaved.get() {
                0 => accounts.to_owned(),
                1 => format!("{accounts} One block has changes that have not reached the server yet, and they will be lost."),
                count => format!("{accounts} {count} blocks have changes that have not reached the server yet, and they will be lost."),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        Some(ErrorAction::DeleteServerDatabase) => {
            "This permanently deletes every workspace and block stored on this device's local server."
                .to_owned()
        }
        None => String::new(),
    }
    });
    view! {
        <Column>
            <Text style=TextStyle::TITLE string="Something went wrong" align=TextAlign::Center />
            <Frame height=240.0>
                <Scroll>
                    <Text style=TextStyle::PARAGRAPH string={message} wrap=true />
                </Scroll>
            </Frame>
            <List direction=Direction::Horizontal spacing=8.0 wrap=true>
                <Button
                    label="Restart"
                    variant=ButtonVariant::Primary
                    on_click={|| send(UiCommand::Restart)}
                />
                <Button
                    label="Copy"
                    variant=ButtonVariant::Secondary
                    on_click={move || copy_text(copied.get_untracked())}
                />
                <Button
                    label="Delete client database..."
                    variant=ButtonVariant::Secondary
                    on_click={|| send(UiCommand::AskErrorAction(ErrorAction::DeleteClientDatabase))}
                />
                <Show condition={platform::HAS_EMBEDDED_SERVER}>
                    <Button
                        label="Delete local server database..."
                        variant=ButtonVariant::Secondary
                        on_click={|| {
                            #[cfg(not(target_arch = "wasm32"))]
                            send(UiCommand::AskErrorAction(ErrorAction::DeleteServerDatabase));
                        }}
                    />
                </Show>
                <Button
                    label="Exit"
                    variant=ButtonVariant::Ghost
                    on_click={|| send(UiCommand::Exit)}
                />
            </List>
            <Dialog open={asking} title={title} on_dismiss={|| send(UiCommand::CancelErrorAction)}>
                <List spacing=12.0>
                    <Text style=TextStyle::PARAGRAPH string={confirmation} wrap=true />
                    <List direction=Direction::Horizontal spacing=8.0>
                        <Button
                            label="Delete"
                            variant=ButtonVariant::Primary
                            on_click={|| send(UiCommand::ConfirmErrorAction)}
                        />
                        <Button
                            label="Cancel"
                            variant=ButtonVariant::Secondary
                            on_click={|| send(UiCommand::CancelErrorAction)}
                        />
                    </List>
                </List>
            </Dialog>
        </Column>
    }
}

#[component]
pub(super) fn AccountsScreen(view: AppViewStore) -> NodeId {
    let keys = create_memo(move || {
        view.accounts
            .get()
            .into_iter()
            .map(|account| account.key)
            .collect::<Vec<_>>()
    });
    let empty = create_memo(move || view.accounts.get().is_empty());
    let error = create_memo(move || match view.add_account.get().open {
        true => None,
        false => view.account_error.get(),
    });
    let accounts = view.accounts;
    view! {
        <Column>
            <Text style=TextStyle::TITLE string="Block Editor" />
            <Text style=TextStyle::CAPTION string="Choose an account to continue." />
            <ForEach keys={keys}>
                {move |key: String| {
                    let accounts = accounts;
                    let account = create_memo(move || {
                        accounts
                            .get()
                            .into_iter()
                            .find(|account| account.key == key)
                            .unwrap_or_default()
                    });
                    view! {
                        <AccountCard account />
                    }
                }}
            </ForEach>
            <Show condition={empty}>
                <Card>
                    <Text
                        style=TextStyle::CAPTION
                        string="No accounts yet. Add one to get started."
                    />
                </Card>
            </Show>
            <Button
                label="Add account"
                glyph={ICON_ADD.to_owned()}
                variant=ButtonVariant::Secondary
                on_click={|| send(UiCommand::OpenAddAccount)}
            />
            <ErrorText text={error} />
            <AddAccountDialog view />
        </Column>
    }
}

#[component]
fn AccountCard(account: Memo<AccountRow>) -> NodeId {
    let name = create_memo(move || account.get().name);
    let key = create_memo(move || account.get().key);
    let open_hint = create_memo(move || match account.get().has_last_workspace {
        true => "Open the last workspace used".to_owned(),
        false => "Choose a workspace".to_owned(),
    });
    let log_out = key;
    let open = key;
    let choose = key;
    view! {
        <Card>
            <List spacing=6.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Text style=TextStyle::HEADING @sizing=ItemSize::Percent(100.0) string={name} />
                    <MenuButton
                        label="Account options"
                        glyph={ICON_MORE_HORIZ.to_owned()}
                        icon_only=true
                        arrow=false
                        on_select={move |_path: Vec<usize>| {
                            send(UiCommand::LogOut(log_out.get_untracked()));
                        }}
                        items={view! {
                            <MenuItem label="Log out" />
                        }}
                    />
                </List>
                <AccountDetails account />
                <List
                    direction=Direction::Horizontal
                    align=Align::Center
                    justify=Justify::End
                    spacing=4.0
                >
                    <Tooltip label={open_hint}>
                        <Button
                            label="Open"
                            variant=ButtonVariant::Primary
                            on_click={move || send(UiCommand::OpenAccount(open.get_untracked()))}
                        />
                    </Tooltip>
                    <IconButton
                        glyph={ICON_KEYBOARD_ARROW_DOWN.to_owned()}
                        label="Open a different workspace"
                        on_click={move || send(UiCommand::ChooseWorkspace(choose.get_untracked()))}
                    />
                </List>
            </List>
        </Card>
    }
}

#[component]
fn AccountDetails(account: Memo<AccountRow>) -> NodeId {
    let email = create_memo(move || account.get().email);
    let server = create_memo(move || account.get().server);
    let glyph = create_memo(move || match account.get().local {
        true => ICON_COMPUTER.to_owned(),
        false => ICON_CLOUD.to_owned(),
    });
    view! {
        <List spacing=2.0>
            <Text style=TextStyle::CAPTION string={email} />
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <Text size=TextSize::Sm align=TextAlign::Center>
                    <Icon glyph={glyph} />
                </Text>
                <Text style=TextStyle::CAPTION string={server} />
            </List>
        </List>
    }
}

#[component]
fn AddAccountDialog(view: AppViewStore) -> NodeId {
    let open = create_memo(move || view.add_account.get().open);
    let generation = create_memo(move || view.add_account.get().generation);
    let pending = create_memo(move || view.add_account.get().pending);
    let error = create_memo(move || view.add_account.get().error);
    let (register, set_register) = create_signal(false);
    let (remote, set_remote) = create_signal(!platform::HAS_EMBEDDED_SERVER);
    let (remote_url, set_remote_url) = create_signal(String::new());
    let (authorization, set_authorization) = create_signal(String::new());
    let (email, set_email) = create_signal(String::new());
    let (display_name, set_display_name) = create_signal(String::new());
    let (password, set_password) = create_signal(String::new());
    create_effect(move || {
        generation.get();
        untrack(|| {
            set_register.set(false);
            set_remote.set(!platform::HAS_EMBEDDED_SERVER);
            set_remote_url.set(String::new());
            set_authorization.set(String::new());
            set_email.set(String::new());
            set_display_name.set(String::new());
            set_password.set(String::new());
        });
    });
    let ready = create_memo(move || {
        !pending.get()
            && !email.get().trim().is_empty()
            && !password.get().is_empty()
            && (!register.get() || !display_name.get().trim().is_empty())
    });
    let not_ready = create_memo(move || !ready.get());
    let submit_label = create_memo(move || match register.get() {
        true => "Register".to_owned(),
        false => "Log in".to_owned(),
    });
    let register_index = create_memo(move || usize::from(register.get()));
    let remote_index = create_memo(move || usize::from(remote.get()));
    let submit = move || {
        if !ready.get_untracked() {
            return;
        }
        send(UiCommand::SubmitAccount(AccountForm {
            register: register.get_untracked(),
            remote: remote.get_untracked(),
            remote_url: remote_url.get_untracked(),
            authorization: authorization.get_untracked(),
            email: email.get_untracked(),
            display_name: display_name.get_untracked(),
            password: password.get_untracked(),
        }));
    };
    let submit_on_enter = submit;
    view! {
        <Dialog
            open={open}
            title="Add account"
            width=320.0
            on_dismiss={|| send(UiCommand::CloseAddAccount)}
        >
            <List spacing=10.0>
                <Tabs
                    selected={register_index}
                    on_change={move |index: usize| set_register.set(index == 1)}
                    options={view! {
                        <ChoiceOption label="Log in" />
                        <ChoiceOption label="Register" />
                    }}
                />
                <Text style=TextStyle::CAPTION string="Server" />
                <Show condition={platform::HAS_EMBEDDED_SERVER}>
                    {move || view! {
                        <Tabs
                            selected={remote_index}
                            on_change={move |index: usize| set_remote.set(index == 1)}
                            options={view! {
                                <ChoiceOption label="Local" />
                                <ChoiceOption label="Remote" />
                            }}
                        />
                    }}
                </Show>
                <Show condition={remote}>
                    {move || view! {
                        <List spacing=10.0>
                            <TextInput
                                value={remote_url}
                                placeholder="https://example.com"
                                label="Server address"
                                on_change={move |value: String| set_remote_url.set(value)}
                            />
                            <Show condition={platform::SENDS_HEADERS}>
                                {move || view! {
                                    <TextInput
                                        value={authorization}
                                        placeholder="Authorization header (optional)"
                                        label="Authorization header"
                                        password=true
                                        on_change={move |value: String| set_authorization.set(value)}
                                    />
                                }}
                            </Show>
                        </List>
                    }}
                </Show>
                <Text style=TextStyle::CAPTION string="Email address" />
                <TextInput
                    value={email}
                    placeholder="you@example.com"
                    label="Email address"
                    on_change={move |value: String| set_email.set(value)}
                />
                <Show condition={register}>
                    {move || view! {
                        <List spacing=10.0>
                            <Text style=TextStyle::CAPTION string="Display name" />
                            <TextInput
                                value={display_name}
                                label="Display name"
                                on_change={move |value: String| set_display_name.set(value)}
                            />
                        </List>
                    }}
                </Show>
                <Text style=TextStyle::CAPTION string="Password" />
                <TextInput
                    value={password}
                    label="Password"
                    password=true
                    on_change={move |value: String| set_password.set(value)}
                    on_submit={move |_value: String| submit_on_enter()}
                />
                <ErrorText text={error} />
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Show condition={pending}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Spinner />
                            <Text style=TextStyle::CAPTION string="Contacting server…" />
                        </List>
                    </Show>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Button
                        label={submit_label}
                        variant=ButtonVariant::Primary
                        disabled={not_ready}
                        on_click={submit}
                    />
                    <Button
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        on_click={|| send(UiCommand::CloseAddAccount)}
                    />
                </List>
            </List>
        </Dialog>
    }
}

#[component]
pub(super) fn WorkspacesScreen(view: AppViewStore) -> NodeId {
    let workspaces = create_memo(move || view.workspaces.get());
    let busy = create_memo(move || workspaces.get().busy);
    let keys = create_memo(move || {
        workspaces
            .get()
            .workspaces
            .into_iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>()
    });
    let invitation_keys = create_memo(move || {
        workspaces
            .get()
            .invitations
            .into_iter()
            .map(|invitation| invitation.id)
            .collect::<Vec<_>>()
    });
    let has_invitations = create_memo(move || !invitation_keys.get().is_empty());
    let empty = create_memo(move || keys.get().is_empty());
    let state = create_memo(move || workspaces.get().state);
    let error = create_memo(move || workspaces.get().error);
    let created = create_memo(move || workspaces.get().created);
    let account = create_memo(move || view.account.get());
    let name = create_memo(move || account.get().name);
    let rows = workspaces;
    let invitations = workspaces;
    let (reload_busy, row_busy) = (busy, busy);
    view! {
        <Column>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Text style=TextStyle::TITLE @sizing=ItemSize::Percent(100.0) string="Workspaces" />
                <MenuButton
                    label="Account options"
                    glyph={ICON_MORE_HORIZ.to_owned()}
                    icon_only=true
                    arrow=false
                    on_select={|_path: Vec<usize>| send(UiCommand::LogOutCurrent)}
                    items={view! {
                        <MenuItem label="Log out" />
                    }}
                />
                <IconButton
                    glyph={ICON_REFRESH.to_owned()}
                    label="Reload workspaces"
                    disabled={reload_busy}
                    on_click={|| send(UiCommand::ReloadWorkspaces)}
                />
            </List>
            <Card>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <List @sizing=ItemSize::Percent(100.0) spacing=4.0>
                        <Text style=TextStyle::HEADING string={name} />
                        <AccountDetails account />
                    </List>
                    <Button
                        label="Switch account"
                        glyph={ICON_SWITCH_ACCOUNT.to_owned()}
                        variant=ButtonVariant::Secondary
                        on_click={|| send(UiCommand::SwitchAccount)}
                    />
                </List>
            </Card>
            <Text style=TextStyle::HEADING string="Open a workspace" />
            <ForEach keys={keys}>
                {move |id: Uuid| {
                    let rows = rows;
                    let label = create_memo(move || {
                        rows.get()
                            .workspaces
                            .into_iter()
                            .find(|(workspace, _)| *workspace == id)
                            .map(|(_, name)| name)
                            .unwrap_or_default()
                    });
                    view! {
                        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                            <Button
                                @sizing=ItemSize::Percent(100.0)
                                label={label}
                                glyph={ICON_WORKSPACES.to_owned()}
                                variant=ButtonVariant::Secondary
                                on_click={move || send(UiCommand::OpenWorkspace(id))}
                            />
                            <IconButton
                                glyph={ICON_KEYBOARD_ARROW_DOWN.to_owned()}
                                label="Choose a session or profile"
                                on_click={move || send(UiCommand::ChooseSession(id))}
                            />
                        </List>
                    }
                }}
            </ForEach>
            <Show condition={empty}>
                {move || view! {
                    <Card>
                        <WorkspacesPlaceholder state />
                    </Card>
                }}
            </Show>
            <Show condition={has_invitations}>
                <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                    <Text align=TextAlign::Center>
                        <Icon glyph={ICON_GROUP_ADD.to_owned()} />
                    </Text>
                    <Text style=TextStyle::HEADING string="Invitations" />
                </List>
            </Show>
            <ForEach keys={invitation_keys}>
                {move |id: Uuid| {
                    let invitations = invitations;
                    let row = create_memo(move || {
                        invitations
                            .get()
                            .invitations
                            .into_iter()
                            .find(|invitation| invitation.id == id)
                    });
                    let workspace = create_memo(move || {
                        row.get().map(|row| row.workspace).unwrap_or_default()
                    });
                    let role = create_memo(move || {
                        row.get()
                            .map(|row| format!("Invited as {}", row.role))
                            .unwrap_or_default()
                    });
                    let busy = row_busy;
                    view! {
                        <Card>
                            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                                <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                                    <Text style=TextStyle::HEADING string={workspace} />
                                    <Text style=TextStyle::CAPTION string={role} />
                                </List>
                                <Button
                                    label="Accept"
                                    variant=ButtonVariant::Primary
                                    disabled={busy}
                                    on_click={move || send(UiCommand::RespondInvitation(id, true))}
                                />
                                <Button
                                    label="Decline"
                                    variant=ButtonVariant::Secondary
                                    disabled={busy}
                                    on_click={move || send(UiCommand::RespondInvitation(id, false))}
                                />
                            </List>
                        </Card>
                    }
                }}
            </ForEach>
            <Text style=TextStyle::HEADING string="Create a workspace" />
            <CreateWorkspace busy={busy} created />
            <ErrorText text={error} />
            <ReauthDialog view />
        </Column>
    }
}

#[component]
pub(super) fn SessionsScreen(view: AppViewStore) -> NodeId {
    let status = view.status;
    let session_keys = create_memo(move || {
        status
            .get()
            .sessions
            .into_iter()
            .map(|session| session.id)
            .collect::<Vec<_>>()
    });
    let profile_keys = create_memo(move || {
        status
            .get()
            .profiles
            .into_iter()
            .map(|profile| profile.id)
            .collect::<Vec<_>>()
    });
    let loaded = create_memo(move || status.get().settings_loaded);
    let loading = create_memo(move || !loaded.get());
    let title = create_memo(move || status.get().workspace);
    let every_type = create_memo(move || status.get().every_session_type);
    let session_type = create_memo(move || status.get().session_type);
    let new_label =
        create_memo(move || format!("New {} session", session_type_name(session_type.get())));
    let new_type = session_type;
    let (sessions, profiles) = (status, status);
    view! {
        <Column>
            <Text style=TextStyle::TITLE string={title} />
            <Show condition={loading}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Spinner />
                    <Text style=TextStyle::CAPTION string="Loading sessions and profiles…" />
                </List>
            </Show>
            <Text style=TextStyle::HEADING string="Settings profile" />
            <Text
                style=TextStyle::CAPTION
                string="The settings this device uses. Each device starts with a profile of its own."
            />
            <ForEach keys={profile_keys}>
                {move |id: Uuid| {
                    let row = create_memo(move || {
                        profiles.get().profiles.into_iter().find(|profile| profile.id == id)
                    });
                    let label = create_memo(move || match row.get() {
                        Some(profile) if profile.current => format!("{} (this device)", profile.name),
                        Some(profile) => profile.name,
                        None => String::new(),
                    });
                    let glyph = create_memo(move || match row.get().is_some_and(|profile| profile.current) {
                        true => ICON_CHECK.to_owned(),
                        false => String::new(),
                    });
                    view! {
                        <Button
                            label={label}
                            glyph={glyph}
                            variant=ButtonVariant::Secondary
                            @test_id={format!("sessions.profile.{id}")}
                            on_click={move || send(UiCommand::UseProfile(id))}
                        />
                    }
                }}
            </ForEach>
            <Button
                label="New settings profile"
                glyph={ICON_ADD.to_owned()}
                variant=ButtonVariant::Ghost
                disabled={loading}
                @test_id={"sessions.new-profile"}
                on_click={|| send(UiCommand::NewProfile)}
            />
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Text style=TextStyle::HEADING @sizing=ItemSize::Percent(100.0) string="Session" />
                <Text style=TextStyle::CAPTION string="Show every type" />
                <Switch
                    on={every_type}
                    label="Show sessions of every type"
                    on_change={|every: bool| send(UiCommand::EverySessionType(every))}
                />
            </List>
            <Text
                style=TextStyle::CAPTION
                string="The blocks you had open. Start a new session to begin again with the same settings."
            />
            <ForEach keys={session_keys}>
                {move |id: Uuid| {
                    let sessions = sessions;
                    let label = create_memo(move || {
                        sessions.get()
                            .sessions
                            .into_iter()
                            .find(|session| session.id == id)
                            .map(|session| match (session.kind, session.current) {
                                (Some(kind), true) => format!("{} ({kind}, last used)", session.name),
                                (Some(kind), false) => format!("{} ({kind})", session.name),
                                (None, true) => format!("{} (last used)", session.name),
                                (None, false) => session.name,
                            })
                            .unwrap_or_default()
                    });
                    view! {
                        <Button
                            label={label}
                            variant=ButtonVariant::Secondary
                            @test_id={format!("sessions.session.{id}")}
                            on_click={move || send(UiCommand::OpenSession(id))}
                        />
                    }
                }}
            </ForEach>
            <SplitButton
                label={new_label}
                glyph={ICON_ADD.to_owned()}
                variant=ButtonVariant::Secondary
                menu_label="New session of another type"
                disabled={loading}
                items={view! {
                    <ForEach keys={(0..SESSION_TYPES.len()).collect::<Vec<_>>()}>
                        {|index: usize| view! {
                            <MenuItem label={format!("New {} session", SESSION_TYPES[index].1)} />
                        }}
                    </ForEach>
                }}
                on_click={move || send(UiCommand::OpenNewSession(new_type.get_untracked()))}
                on_select={|path: Vec<usize>| {
                    if let Some((editor, _)) = path.first().and_then(|index| SESSION_TYPES.get(*index)) {
                        send(UiCommand::OpenNewSession(*editor));
                    }
                }}
            />
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Button
                    label="Keep the current session"
                    variant=ButtonVariant::Ghost
                    @test_id={"sessions.keep"}
                    on_click={|| send(UiCommand::CloseSessions)}
                />
                <Button
                    label="Back to workspaces"
                    variant=ButtonVariant::Ghost
                    on_click={|| send(UiCommand::SwitchWorkspace)}
                />
            </List>
        </Column>
    }
}

#[component]
fn WorkspacesPlaceholder(state: Memo<WorkspacesState>) -> NodeId {
    let loaded = create_memo(move || state.get() == WorkspacesState::Loaded);
    let failed = create_memo(move || state.get() == WorkspacesState::Failed);
    let loading = create_memo(move || state.get() == WorkspacesState::Loading);
    view! {
        <List spacing=0.0>
            <Show condition={loaded}>
                <Text
                    style=TextStyle::CAPTION
                    string="You do not have any workspaces yet. Create one below."
                />
            </Show>
            <Show condition={failed}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Text style=TextStyle::CAPTION string="Could not load workspaces." />
                    <Button
                        label="Retry"
                        variant=ButtonVariant::Secondary
                        on_click={|| send(UiCommand::ReloadWorkspaces)}
                    />
                </List>
            </Show>
            <Show condition={loading}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Spinner />
                    <Text style=TextStyle::CAPTION string="Loading workspaces…" />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn CreateWorkspace(busy: Memo<bool>, created: Memo<u64>) -> NodeId {
    let (name, set_name) = create_signal(String::new());
    create_effect(move || {
        created.get();
        untrack(|| set_name.set(String::new()));
    });
    let cannot = create_memo(move || busy.get() || name.get().trim().is_empty());
    let create = move || {
        if !cannot.get_untracked() {
            send(UiCommand::CreateWorkspace(name.get_untracked()));
        }
    };
    let submit = create;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
            <TextInput
                @sizing=ItemSize::Percent(100.0)
                value={name}
                placeholder="Workspace name"
                label="Workspace name"
                on_change={move |value: String| set_name.set(value)}
                on_submit={move |_value: String| submit()}
            />
            <Button
                label="Create"
                variant=ButtonVariant::Primary
                disabled={cannot}
                on_click={create}
            />
        </List>
    }
}

#[component]
fn ReauthDialog(view: AppViewStore) -> NodeId {
    let reauth = view.reauth;
    let open = create_memo(move || reauth.get().is_some());
    let busy = create_memo(move || reauth.get().is_some_and(|reauth| reauth.busy));
    let error = create_memo(move || reauth.get().and_then(|reauth| reauth.error));
    let message = create_memo(move || {
        format!(
            "Your session for {} is no longer valid. Enter your password to sign in again, \
             or close this to switch accounts.",
            reauth.get().map(|reauth| reauth.email).unwrap_or_default()
        )
    });
    let (password, set_password) = create_signal(String::new());
    create_effect(move || {
        if !open.get() {
            untrack(|| set_password.set(String::new()));
        }
    });
    let cannot = create_memo(move || busy.get() || password.get().is_empty());
    let submit = move || {
        if !cannot.get_untracked() {
            send(UiCommand::ReauthSubmit(password.get_untracked()));
        }
    };
    let submit_on_enter = submit;
    view! {
        <Dialog
            open={open}
            title="Session expired"
            width=320.0
            on_dismiss={|| send(UiCommand::ReauthClose)}
        >
            <List spacing=10.0>
                <Text style=TextStyle::PARAGRAPH string={message} wrap=true />
                <Text style=TextStyle::CAPTION string="Password" />
                <TextInput
                    value={password}
                    label="Password"
                    password=true
                    on_change={move |value: String| set_password.set(value)}
                    on_submit={move |_value: String| submit_on_enter()}
                />
                <ErrorText text={error} />
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Show condition={busy}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Spinner />
                            <Text style=TextStyle::CAPTION string="Signing in…" />
                        </List>
                    </Show>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Button
                        label="Sign in"
                        variant=ButtonVariant::Primary
                        disabled={cannot}
                        on_click={submit}
                    />
                    <Button
                        label="Log out"
                        variant=ButtonVariant::Secondary
                        disabled={busy}
                        on_click={|| send(UiCommand::ReauthLogOut)}
                    />
                </List>
            </List>
        </Dialog>
    }
}

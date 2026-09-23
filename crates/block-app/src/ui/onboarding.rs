use beui::icons::{
    ICON_ADD, ICON_CLOUD, ICON_COMPUTER, ICON_GROUP_ADD, ICON_KEYBOARD_ARROW_DOWN, ICON_MORE_HORIZ,
    ICON_REFRESH, ICON_SWITCH_ACCOUNT, ICON_WORKSPACES,
};
use beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, Spacer, clone,
    component, create_effect, create_memo, create_signal, untrack, view,
};
use beui::styled::{
    Button, ButtonVariant, Caption, Card, Dialog, Heading, Icon, IconButton, MenuButton,
    Paragraph, Scroll, Spinner, Tabs, TextInput, Title, Tooltip, use_theme,
};
use beui::unstyled::{ChoiceOption, MenuItem};
use beui::{NodeId, TextAlign};
use uuid::Uuid;

use super::{AccountForm, AccountRow, AppViewStore, ErrorAction, UiCommand, WorkspacesState, send};
use crate::platform;

const COLUMN_WIDTH: f32 = 460.0;

#[component]
pub(super) fn ErrorText(text: Memo<Option<String>>) -> NodeId {
    let theme = use_theme();
    let shown = create_memo(clone!(text -> move || text.get().is_some()));
    let content = create_memo(move || text.get().unwrap_or_default());
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <Paragraph content={content} color={theme.danger.clone()} />
            </Show>
        </List>
    }
}

#[component]
pub(super) fn Column(children: beui::reactive::Children<beui::reactive::ListChild>) -> NodeId {
    view! {
        <Scroll>
            <List spacing=0.0 align=Align::Center>
                <Frame max_width=COLUMN_WIDTH padding_horizontal=16.0 padding_vertical=36.0>
                    <List spacing=12.0>{children}</List>
                </Frame>
            </List>
        </Scroll>
    }
}

#[component]
pub(super) fn ErrorScreen(view: AppViewStore) -> NodeId {
    let message = create_memo(clone!(view -> move || view.error.get().message));
    let pending = create_memo(clone!(view -> move || view.error.get().pending));
    let asking = create_memo(clone!(pending -> move || pending.get().is_some()));
    let title = create_memo(clone!(pending -> move || match pending.get() {
        Some(ErrorAction::DeleteClientDatabase) => "Delete client database?".to_owned(),
        #[cfg(not(target_arch = "wasm32"))]
        Some(ErrorAction::DeleteServerDatabase) => "Delete local server database?".to_owned(),
        None => String::new(),
    }));
    let confirmation = create_memo(move || match pending.get() {
        Some(ErrorAction::DeleteClientDatabase) => {
            "This removes every saved account on this device. You will need to sign in again."
                .to_owned()
        }
        #[cfg(not(target_arch = "wasm32"))]
        Some(ErrorAction::DeleteServerDatabase) => {
            "This permanently deletes every workspace and block stored on this device's local server."
                .to_owned()
        }
        None => String::new(),
    });
    view! {
        <Column>
            <Title content="Something went wrong" align=TextAlign::Center />
            <Frame height=240.0>
                <Scroll>
                    <Paragraph content={message} />
                </Scroll>
            </Frame>
            <List direction=Direction::Horizontal spacing=8.0 wrap=true>
                <Button
                    label="Restart"
                    variant=ButtonVariant::Primary
                    on_click={|| send(UiCommand::Restart)}
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
                <Button label="Exit" variant=ButtonVariant::Ghost on_click={|| send(UiCommand::Exit)} />
            </List>
            <Dialog
                open={asking}
                title={title}
                on_dismiss={|| send(UiCommand::CancelErrorAction)}
            >
                <List spacing=12.0>
                    <Paragraph content={confirmation} />
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
    let keys = create_memo(clone!(view -> move || {
        view.accounts
            .get()
            .into_iter()
            .map(|account| account.key)
            .collect::<Vec<_>>()
    }));
    let empty = create_memo(clone!(view -> move || view.accounts.get().is_empty()));
    let error = create_memo(clone!(view -> move || {
        match view.add_account.get().open {
            true => None,
            false => view.account_error.get(),
        }
    }));
    let accounts = view.accounts.clone();
    view! {
        <Column>
            <Title content="Block Editor" />
            <Caption content="Choose an account to continue." />
            <ForEach keys={keys}>
                {move |key: String| {
                    let accounts = accounts.clone();
                    let account = create_memo(move || {
                        accounts
                            .get()
                            .into_iter()
                            .find(|account| account.key == key)
                            .unwrap_or_default()
                    });
                    view! { <AccountCard account /> }
                }}
            </ForEach>
            <Show condition={empty}>
                <Card>
                    <Caption content="No accounts yet. Add one to get started." />
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
    let name = create_memo(clone!(account -> move || account.get().name));
    let key = create_memo(clone!(account -> move || account.get().key));
    let open_hint = create_memo(clone!(account -> move || {
        match account.get().has_last_workspace {
            true => "Open the last workspace used".to_owned(),
            false => "Choose a workspace".to_owned(),
        }
    }));
    let log_out = key.clone();
    let open = key.clone();
    let choose = key;
    view! {
        <Card>
            <List spacing=6.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Heading @sizing=ItemSize::Percent(100.0) content={name} />
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
                <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
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
    let email = create_memo(clone!(account -> move || account.get().email));
    let server = create_memo(clone!(account -> move || account.get().server));
    let glyph = create_memo(move || match account.get().local {
        true => ICON_COMPUTER.to_owned(),
        false => ICON_CLOUD.to_owned(),
    });
    view! {
        <List spacing=2.0>
            <Caption content={email} />
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <Icon glyph={glyph} text_size=12.0 />
                <Caption content={server} />
            </List>
        </List>
    }
}

#[component]
fn AddAccountDialog(view: AppViewStore) -> NodeId {
    let open = create_memo(clone!(view -> move || view.add_account.get().open));
    let generation = create_memo(clone!(view -> move || view.add_account.get().generation));
    let pending = create_memo(clone!(view -> move || view.add_account.get().pending));
    let error = create_memo(clone!(view -> move || view.add_account.get().error));
    let (register, set_register) = create_signal(false);
    let (remote, set_remote) = create_signal(!platform::HAS_EMBEDDED_SERVER);
    let (remote_url, set_remote_url) = create_signal(String::new());
    let (email, set_email) = create_signal(String::new());
    let (display_name, set_display_name) = create_signal(String::new());
    let (password, set_password) = create_signal(String::new());
    create_effect(clone!(
        set_register set_remote set_remote_url set_email set_display_name set_password -> move || {
            generation.get();
            untrack(|| {
                set_register.set(false);
                set_remote.set(!platform::HAS_EMBEDDED_SERVER);
                set_remote_url.set(String::new());
                set_email.set(String::new());
                set_display_name.set(String::new());
                set_password.set(String::new());
            });
        }
    ));
    let ready = create_memo(clone!(pending register email password display_name -> move || {
        !pending.get()
            && !email.get().trim().is_empty()
            && !password.get().is_empty()
            && (!register.get() || !display_name.get().trim().is_empty())
    }));
    let not_ready = create_memo(clone!(ready -> move || !ready.get()));
    let submit_label = create_memo(clone!(register -> move || match register.get() {
        true => "Register".to_owned(),
        false => "Log in".to_owned(),
    }));
    let register_index = create_memo(clone!(register -> move || usize::from(register.get())));
    let remote_index = create_memo(clone!(remote -> move || usize::from(remote.get())));
    let submit = clone!(ready register remote remote_url email display_name password -> move || {
        if !ready.get_untracked() {
            return;
        }
        send(UiCommand::SubmitAccount(AccountForm {
            register: register.get_untracked(),
            remote: remote.get_untracked(),
            remote_url: remote_url.get_untracked(),
            email: email.get_untracked(),
            display_name: display_name.get_untracked(),
            password: password.get_untracked(),
        }));
    });
    let submit_on_enter = submit.clone();
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
                <Caption content="Server" />
                <Show condition={platform::HAS_EMBEDDED_SERVER}>
                    <Tabs
                        selected={remote_index}
                        on_change={move |index: usize| set_remote.set(index == 1)}
                        options={view! {
                            <ChoiceOption label="Local" />
                            <ChoiceOption label="Remote" />
                        }}
                    />
                </Show>
                <Show condition={remote}>
                    <TextInput
                        value={remote_url}
                        placeholder="https://example.com"
                        label="Server address"
                        on_change={move |value: String| set_remote_url.set(value)}
                    />
                </Show>
                <Caption content="Email address" />
                <TextInput
                    value={email}
                    placeholder="you@example.com"
                    label="Email address"
                    on_change={move |value: String| set_email.set(value)}
                />
                <Show condition={register}>
                    <List spacing=10.0>
                        <Caption content="Display name" />
                        <TextInput
                            value={display_name}
                            label="Display name"
                            on_change={move |value: String| set_display_name.set(value)}
                        />
                    </List>
                </Show>
                <Caption content="Password" />
                <TextInput
                    value={password}
                    label="Password"
                    password=true
                    on_change={move |value: String| set_password.set(value)}
                    on_submit={move |_value: String| submit_on_enter()}
                />
                <ErrorText text={error} />
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Show condition={pending.clone()}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Spinner />
                            <Caption content="Contacting server…" />
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
    let workspaces = create_memo(clone!(view -> move || view.workspaces.get()));
    let busy = create_memo(clone!(workspaces -> move || workspaces.get().busy));
    let keys = create_memo(clone!(workspaces -> move || {
        workspaces
            .get()
            .workspaces
            .into_iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>()
    }));
    let invitation_keys = create_memo(clone!(workspaces -> move || {
        workspaces
            .get()
            .invitations
            .into_iter()
            .map(|invitation| invitation.id)
            .collect::<Vec<_>>()
    }));
    let has_invitations = create_memo(clone!(invitation_keys -> move || !invitation_keys.get().is_empty()));
    let empty = create_memo(clone!(keys -> move || keys.get().is_empty()));
    let state = create_memo(clone!(workspaces -> move || workspaces.get().state));
    let error = create_memo(clone!(workspaces -> move || workspaces.get().error));
    let created = create_memo(clone!(workspaces -> move || workspaces.get().created));
    let account = create_memo(clone!(view -> move || view.account.get()));
    let name = create_memo(clone!(account -> move || account.get().name));
    let rows = workspaces.clone();
    let invitations = workspaces;
    let (reload_busy, row_busy) = (busy.clone(), busy.clone());
    view! {
        <Column>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Title @sizing=ItemSize::Percent(100.0) content="Workspaces" />
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
                        <Heading content={name} />
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
            <Heading content="Open a workspace" />
            <ForEach keys={keys}>
                {move |id: Uuid| {
                    let rows = rows.clone();
                    let label = create_memo(move || {
                        rows.get()
                            .workspaces
                            .into_iter()
                            .find(|(workspace, _)| *workspace == id)
                            .map(|(_, name)| name)
                            .unwrap_or_default()
                    });
                    view! {
                        <Button
                            label={label}
                            glyph={ICON_WORKSPACES.to_owned()}
                            variant=ButtonVariant::Secondary
                            on_click={move || send(UiCommand::OpenWorkspace(id))}
                        />
                    }
                }}
            </ForEach>
            <Show condition={empty}>
                <Card>
                    <WorkspacesPlaceholder state />
                </Card>
            </Show>
            <Show condition={has_invitations}>
                <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                    <Icon glyph={ICON_GROUP_ADD.to_owned()} />
                    <Heading content="Invitations" />
                </List>
            </Show>
            <ForEach keys={invitation_keys}>
                {move |id: Uuid| {
                    let invitations = invitations.clone();
                    let row = create_memo(move || {
                        invitations
                            .get()
                            .invitations
                            .into_iter()
                            .find(|invitation| invitation.id == id)
                    });
                    let workspace = create_memo(clone!(row -> move || {
                        row.get().map(|row| row.workspace).unwrap_or_default()
                    }));
                    let role = create_memo(move || {
                        row.get()
                            .map(|row| format!("Invited as {}", row.role))
                            .unwrap_or_default()
                    });
                    let busy = row_busy.clone();
                    view! {
                        <Card>
                            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                                <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                                    <Heading content={workspace} />
                                    <Caption content={role} />
                                </List>
                                <Button
                                    label="Accept"
                                    variant=ButtonVariant::Primary
                                    disabled={busy.clone()}
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
            <Heading content="Create a workspace" />
            <CreateWorkspace busy={busy} created />
            <ErrorText text={error} />
            <ReauthDialog view />
        </Column>
    }
}

#[component]
fn WorkspacesPlaceholder(state: Memo<WorkspacesState>) -> NodeId {
    let loaded = create_memo(clone!(state -> move || state.get() == WorkspacesState::Loaded));
    let failed = create_memo(clone!(state -> move || state.get() == WorkspacesState::Failed));
    let loading = create_memo(move || state.get() == WorkspacesState::Loading);
    view! {
        <List spacing=0.0>
            <Show condition={loaded}>
                <Caption content="You do not have any workspaces yet. Create one below." />
            </Show>
            <Show condition={failed}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Caption content="Could not load workspaces." />
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
                    <Caption content="Loading workspaces…" />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn CreateWorkspace(busy: Memo<bool>, created: Memo<u64>) -> NodeId {
    let (name, set_name) = create_signal(String::new());
    create_effect(clone!(set_name -> move || {
        created.get();
        untrack(|| set_name.set(String::new()));
    }));
    let cannot = create_memo(clone!(busy name -> move || busy.get() || name.get().trim().is_empty()));
    let create = clone!(name cannot -> move || {
        if !cannot.get_untracked() {
            send(UiCommand::CreateWorkspace(name.get_untracked()));
        }
    });
    let submit = create.clone();
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
    let reauth = view.reauth.clone();
    let open = create_memo(clone!(reauth -> move || reauth.get().is_some()));
    let busy = create_memo(clone!(reauth -> move || reauth.get().is_some_and(|reauth| reauth.busy)));
    let error = create_memo(clone!(reauth -> move || reauth.get().and_then(|reauth| reauth.error)));
    let message = create_memo(move || {
        format!(
            "Your session for {} is no longer valid. Enter your password to sign in again, \
             or close this to switch accounts.",
            reauth.get().map(|reauth| reauth.email).unwrap_or_default()
        )
    });
    let (password, set_password) = create_signal(String::new());
    create_effect(clone!(open set_password -> move || {
        if !open.get() {
            untrack(|| set_password.set(String::new()));
        }
    }));
    let cannot = create_memo(clone!(busy password -> move || busy.get() || password.get().is_empty()));
    let submit = clone!(password cannot -> move || {
        if !cannot.get_untracked() {
            send(UiCommand::ReauthSubmit(password.get_untracked()));
        }
    });
    let submit_on_enter = submit.clone();
    view! {
        <Dialog
            open={open}
            title="Session expired"
            width=320.0
            on_dismiss={|| send(UiCommand::ReauthClose)}
        >
            <List spacing=10.0>
                <Paragraph content={message} />
                <Caption content="Password" />
                <TextInput
                    value={password}
                    label="Password"
                    password=true
                    on_change={move |value: String| set_password.set(value)}
                    on_submit={move |_value: String| submit_on_enter()}
                />
                <ErrorText text={error} />
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Show condition={busy.clone()}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Spinner />
                            <Caption content="Signing in…" />
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

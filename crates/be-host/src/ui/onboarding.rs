use be_host_accounts_ui::{AccountDetails, Column, ErrorText};
use beui::icons::{
    ICON_BOXES, ICON_CHECK, ICON_CHEVRON_DOWN, ICON_CLOUD, ICON_COPY, ICON_ELLIPSIS, ICON_MONITOR,
    ICON_PLUS, ICON_REFRESH_CW, ICON_USER_PLUS, ICON_USERS_ROUND,
};
use beui::reactive::*;
use beui::styled::*;
use beui::{NodeId, TextAlign};
use uuid::Uuid;

use super::{
    AccountForm, ErrorAction, HostUiStore, JoiningRow, OrganizationsState, UiCommand, send,
};
use crate::root_settings::{SESSION_TYPES, session_type_name};
use be_host_base::platform;
use beui::{Align, Direction};

#[component]
pub(super) fn ErrorScreen(view: HostUiStore) -> NodeId {
    let message = create_memo(move || view.error.get().message);
    let pending = create_memo(move || view.error.get().pending);
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
                "This removes every saved account and its key on this device. You will need to sign in again, and unlock each account with its recovery phrase or another device.";
            match unsaved.get() {
                0 => accounts.to_owned(),
                1 => format!("{accounts} One block has changes that have not reached the server yet, and they will be lost."),
                count => format!("{accounts} {count} blocks have changes that have not reached the server yet, and they will be lost."),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        Some(ErrorAction::DeleteServerDatabase) => {
            "This permanently deletes every organization and block stored on this device's local server."
                .to_owned()
        }
        None => String::new(),
    }
    });
    view! {
        <Column>
            <Text style=TextStyle::TITLE align=TextAlign::Center>"Something went wrong"</Text>
            <Frame height={Size::Custom(240.0)}>
                <Scroll>
                    <Text style=TextStyle::PARAGRAPH>{message}</Text>
                </Scroll>
            </Frame>
            <List direction=Direction::Horizontal spacing=Space::Md wrap=true>
                <Button
                    label="Restart"
                    variant=ButtonVariant::Primary
                    on_click={|| send(UiCommand::Restart)}
                />
                <Button
                    label="Copy"
                    variant=ButtonVariant::Secondary
                    on_click={move || copy(Clip::text(message.get_untracked()))}
                />
                <Button
                    label="Delete client database..."
                    variant=ButtonVariant::Secondary
                    on_click={|| send(UiCommand::AskErrorAction(ErrorAction::DeleteClientDatabase))}
                />
                {platform::HAS_EMBEDDED_SERVER.then(|| view! {
                    <Button
                        label="Delete local server database..."
                        variant=ButtonVariant::Secondary
                        on_click={|| {
                            #[cfg(not(target_arch = "wasm32"))]
                            send(UiCommand::AskErrorAction(ErrorAction::DeleteServerDatabase));
                        }}
                    />
                })}
                <Button
                    label="Exit"
                    variant=ButtonVariant::Ghost
                    on_click={|| send(UiCommand::Exit)}
                />
            </List>
            <Dialog
                open={move || pending.with(Option::is_some)}
                kind=DialogKind::Alert
                title={title}
                actions={vec![
                    DialogAction::destructive("Delete", || send(UiCommand::ConfirmErrorAction)),
                    DialogAction::cancel("Cancel"),
                ]}
                on_dismiss={|| send(UiCommand::CancelErrorAction)}
            >
                <Text style=TextStyle::PARAGRAPH>{confirmation}</Text>
            </Dialog>
        </Column>
    }
}

#[component]
pub(super) fn AccountsScreen(view: HostUiStore) -> NodeId {
    let accounts = create_memo(move || view.accounts.get());
    let error = create_memo(move || match view.add_account.get().open {
        true => None,
        false => view.account_error.get(),
    });
    view! {
        <List spacing=Space::None>
            <InvitationBanner view />
            <ListItem grow=1.0>
                <be_host_accounts_ui::AccountsScreen
                    accounts
                    error
                    on_add={|| send(UiCommand::OpenAddAccount)}
                    on_open={|key: String| send(UiCommand::OpenAccount(key))}
                    on_choose_organization={|key: String| send(UiCommand::ChooseOrganization(key))}
                    on_log_out={|key: String| send(UiCommand::LogOut(key))}
                />
            </ListItem>
        </List>
    }
}

/// An invite link opened before signing in: whose invitation it is and into what. Its code is
/// held while the person signs in or makes an account, and the invitation waits for them in
/// Join an organization after.
#[component]
fn InvitationBanner(view: HostUiStore) -> NodeId {
    let joining = create_memo(move || view.organizations.get().joining);
    view! {
        <Show value={joining}>|row: ReadSignal<JoiningRow>| {
            let title = create_memo(move || {
                row.with(|row| format!("{} invited you to join {}", row.inviter, row.organization))
            });
            let lead = create_memo(move || {
                row.with(|row| match row.problem.is_empty() {
                    true => "Sign in, or add an account, to accept it. You join as an editor."
                        .to_owned(),
                    false => row.problem.clone(),
                })
            });
            view! {
                <Frame padding={Padding::all(Space::Lg)} @test_id={"host.invited"}>
                    <Card>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <Text size=TextSize::Lg color=TextColor::Accent>
                                <Icon glyph=ICON_USER_PLUS />
                            </Text>
                            <ListItem grow=1.0>
                                <List spacing=Space::Xs2>
                                    <Text weight=Weight::Bold>{title}</Text>
                                    <Text style=TextStyle::CAPTION>{lead}</Text>
                                </List>
                            </ListItem>
                        </List>
                    </Card>
                </Frame>
            }
        }</Show>
    }
}

#[component]
pub(super) fn AddAccountDialog(
    view: HostUiStore,
    #[prop(default = platform::default_account_server())] default_server: Option<
        be_lib_build_id::DefaultServer,
    >,
) -> NodeId {
    let open = create_memo(move || view.add_account.get().open);
    let generation = create_memo(move || view.add_account.get().generation);
    let pending = create_memo(move || view.add_account.get().pending);
    let error = create_memo(move || view.add_account.get().error);
    let browser = create_memo(move || view.add_account.get().browser);
    let waiting = create_memo(move || browser.with(Option::is_some));
    let browser_url = create_memo(move || browser.get().map(|login| login.url).unwrap_or_default());
    let browser_code =
        create_memo(move || browser.get().map(|login| login.code).unwrap_or_default());
    // The Copy handle the render closures below share: the server is not `Copy`.
    let default = create_memo(move || default_server.clone());
    let has_default = default.get_untracked().is_some();
    let default_name =
        create_memo(move || default.get().map(|server| server.name).unwrap_or_default());
    let selection = create_linked_signal(move || {
        generation.track();
        None::<u8>
    });
    let remote_url = create_linked_signal(move || {
        generation.track();
        String::new()
    });
    let authorization = create_linked_signal(move || {
        generation.track();
        String::new()
    });
    let display_name = create_linked_signal(move || {
        generation.track();
        String::new()
    });
    let editing = create_memo(move || selection.with(Option::is_some) && !pending.get());
    let local = create_memo(move || selection.get() == Some(1));
    let remote = create_memo(move || selection.with(Option::is_some) && !local.get());
    let gateway_required = create_memo(move || {
        selection.get() == Some(0) && default.get().is_some_and(|server| server.gateway_required)
    });
    let gateway_label = create_memo(move || {
        if gateway_required.get() {
            "Gateway token"
        } else {
            "Gateway token (optional)"
        }
        .to_owned()
    });
    let server_name = create_memo(move || match selection.get() {
        Some(0) => default
            .get()
            .map(|server| server.api_url)
            .unwrap_or_default(),
        Some(1) => "On this device".into(),
        _ => "Another server".into(),
    });
    let ready = create_memo(move || {
        !pending.get()
            && match selection.get() {
                Some(1) => !display_name.get().trim().is_empty(),
                Some(0) => {
                    !platform::SENDS_HEADERS
                        || !gateway_required.get()
                        || !authorization.get().trim().is_empty()
                }
                Some(2) => !remote_url.get().trim().is_empty(),
                _ => false,
            }
    });
    let submit_label = create_memo(move || {
        if local.get() {
            "Create account"
        } else {
            "Continue"
        }
        .to_owned()
    });
    let submit = move || {
        if ready.get_untracked() {
            send(UiCommand::SubmitAccount(AccountForm {
                remote: !local.get_untracked(),
                remote_url: if selection.get_untracked() == Some(0) {
                    default
                        .get_untracked()
                        .map(|server| server.api_url)
                        .unwrap_or_default()
                } else {
                    remote_url.get_untracked()
                },
                authorization: authorization.get_untracked(),
                display_name: display_name.get_untracked(),
            }));
        }
    };
    let title = create_memo(move || {
        if pending.get() {
            "Sign in with your browser"
        } else {
            match selection.get() {
                Some(1) => "Create local account",
                Some(_) => "Connect to server",
                None => "Add account",
            }
        }
        .to_owned()
    });
    let footer_primary = create_memo(move || {
        if waiting.get() {
            "Open browser".to_owned()
        } else if editing.get() {
            submit_label.get()
        } else {
            String::new()
        }
    });
    let connecting = create_memo(move || pending.get() && !waiting.get());
    view! {
        <Dialog
            open={open}
            title={title}
            width={Size::Custom(460.0)}
            id="account"
            actions={vec![
                DialogAction::secondary("Back", move || {
                    selection.set(None);
                    send(UiCommand::RetryAccount);
                })
                .shown(editing)
                .named("back"),
                DialogAction::cancel("Cancel"),
                DialogAction::secondary("Check now", || send(UiCommand::CheckLoginNow))
                    .shown(waiting)
                    .named("check-now"),
                DialogAction::primary(footer_primary, move || {
                    if waiting.get_untracked() {
                        send(UiCommand::OpenLoginBrowser);
                    } else {
                        submit();
                    }
                })
                .disabled(move || !waiting.get() && !ready.get())
                .busy(connecting)
                .shown(create_memo(move || !footer_primary.with(String::is_empty))),
            ]}
            on_dismiss={|| send(UiCommand::CloseAddAccount)}
        >
            <List spacing=Space::Xl2>
                <Show value={move || selection.with(Option::is_none) && !pending.get()}>
                    <List spacing=Space::Lg>
                        <Text style=TextStyle::PARAGRAPH>"Choose where to keep your account."</Text>
                        <Frame
                            fill=ColorRole::Surface
                            border={Border::DEFAULT}
                            radius=Radius::Md
                            padding=Space::Sm
                        >
                            <List spacing=Space::Xs>
                                {has_default.then(|| view! {
                                    <ActionRow
                                        label={default_name}
                                        detail={move || {
                                            default
                                                .get()
                                                .map(|server| server.api_url)
                                                .unwrap_or_default()
                                        }}
                                        glyph=ICON_CLOUD
                                        @test_id="account.default"
                                        on_click={move || selection.set(Some(0))}
                                    />
                                })}
                                {platform::HAS_EMBEDDED_SERVER.then(|| view! {
                                    <ActionRow
                                        label="On this device"
                                        detail="A local account, stored on this device"
                                        glyph=ICON_MONITOR
                                        @test_id="account.local"
                                        on_click={move || selection.set(Some(1))}
                                    />
                                })}
                                <ActionRow
                                    label="Another server"
                                    detail="Connect using a server address"
                                    glyph=ICON_PLUS
                                    @test_id="account.custom"
                                    on_click={move || selection.set(Some(2))}
                                />
                            </List>
                        </Frame>
                    </List>
                </Show>
                <Show value={editing}>
                    <List spacing=Space::Xl>
                        <Match value={selection}>
                            <Case is={Some(1)}>
                                <List spacing=Space::Xl>
                                    <Text style=TextStyle::PARAGRAPH>
                                        "Your data stays on this device. You only need a display name."
                                    </Text>
                                    <List spacing=Space::Sm>
                                        <Text style=TextStyle::CAPTION>"Display name"</Text>
                                        <TextInput
                                            value={display_name}
                                            label="Display name"
                                            placeholder="Your name"
                                            @test_id="account.name"
                                            on_submit={move |_| submit()}
                                        />
                                    </List>
                                </List>
                            </Case>
                            <Case is={Some(0)}>
                                <List spacing=Space::Xs>
                                    <Text size=TextSize::Lg>{default_name}</Text>
                                    <Text style=TextStyle::CAPTION>{server_name}</Text>
                                </List>
                            </Case>
                            <Case is={Some(2)}>
                                <List spacing=Space::Sm>
                                    <Text style=TextStyle::CAPTION>"Server address"</Text>
                                    <TextInput
                                        value={remote_url}
                                        label="Server address"
                                        placeholder="https://example.com"
                                        @test_id="account.server"
                                        on_submit={move |_| submit()}
                                    />
                                </List>
                            </Case>
                        </Match>
                        <Show value={move || remote.get() && platform::SENDS_HEADERS}>
                            <List spacing=Space::Sm>
                                <Text style=TextStyle::CAPTION>{gateway_label}</Text>
                                <TextInput
                                    value={authorization}
                                    label={gateway_label}
                                    kind={InputKind::Password}
                                    @test_id="account.gateway"
                                    on_submit={move |_| submit()}
                                />
                            </List>
                        </Show>
                        <Show value={remote}>
                            <Text style=TextStyle::CAPTION>
                                "Next, you’ll sign in or create an account in your browser."
                            </Text>
                        </Show>
                    </List>
                </Show>
                <Show value={waiting}>
                    <List spacing=Space::Xl>
                        <Text style=TextStyle::PARAGRAPH>
                            "Sign in and approve this code in your browser. You can also use a browser on another device. This is checked every few seconds; once you have approved it, choose Check now."
                        </Text>
                        <Card>
                            <List spacing=Space::Xl>
                                <List spacing=Space::Sm>
                                    <Text style=TextStyle::CAPTION>"Device code"</Text>
                                    <List
                                        direction=Direction::Horizontal
                                        align=Align::Center
                                        spacing=Space::Lg
                                    >
                                        <ListItem grow=1.0>
                                            <SelectableText child_grow=1.0>
                                                <Text
                                                    size=TextSize::Xl
                                                    @test_id="account.browser-code"
                                                >
                                                    {browser_code}
                                                </Text>
                                            </SelectableText>
                                        </ListItem>
                                        <Button
                                            icon_only=true
                                            label="Copy code"
                                            glyph=ICON_COPY
                                            variant=ButtonVariant::Ghost
                                            @test_id="account.copy-code"
                                            on_click={move || copy(Clip::text(browser_code.get_untracked()))}
                                        />
                                    </List>
                                </List>
                                <Separator />
                                <List spacing=Space::Sm>
                                    <Text style=TextStyle::CAPTION>"Sign-in link"</Text>
                                    <List
                                        direction=Direction::Horizontal
                                        align=Align::Center
                                        spacing=Space::Lg
                                    >
                                        <ListItem grow=1.0>
                                            <SelectableText child_grow=1.0>
                                                <Text
                                                    style=TextStyle::CAPTION
                                                    @test_id="account.browser-url"
                                                >
                                                    {browser_url}
                                                </Text>
                                            </SelectableText>
                                        </ListItem>
                                        <Button
                                            icon_only=true
                                            label="Copy link"
                                            glyph=ICON_COPY
                                            variant=ButtonVariant::Ghost
                                            @test_id="account.copy-link"
                                            on_click={move || copy(Clip::text(browser_url.get_untracked()))}
                                        />
                                    </List>
                                </List>
                            </List>
                        </Card>
                        <Text style=TextStyle::CAPTION>
                            "Waiting for approval. This window will continue automatically."
                        </Text>
                    </List>
                </Show>
                <Show value={connecting}>
                    <Text style=TextStyle::PARAGRAPH>"Connecting to the server…"</Text>
                </Show>
                <Show value={move || error.with(Option::is_some)}>
                    <ErrorText text={error} />
                </Show>
            </List>
        </Dialog>
    }
}

#[component]
pub(super) fn OrganizationsScreen(view: HostUiStore) -> NodeId {
    let organizations = create_memo(move || view.organizations.get());
    let busy = create_memo(move || organizations.get().busy);
    let keys = create_memo(move || {
        organizations
            .get()
            .organizations
            .into_iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>()
    });
    let state = create_memo(move || organizations.get().state);
    let error = create_memo(move || organizations.get().error);
    let created = create_memo(move || organizations.get().created);
    let account = create_memo(move || view.account.get());
    let name = create_memo(move || account.get().name);
    view! {
        <Column>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <ListItem grow=1.0>
                    <Text style=TextStyle::TITLE>"Organizations"</Text>
                </ListItem>
                <MenuButton
                    label="Account options"
                    glyph=ICON_ELLIPSIS
                    icon_only=true
                    arrow=false
                    items={view! {
                        <MenuItem
                            label={move || be_host_accounts_ui::leave_label(account.get().local).to_owned()}
                            on_click={|| send(UiCommand::LogOutCurrent)}
                        />
                    }}
                />
                <Button
                    icon_only=true
                    glyph=ICON_REFRESH_CW
                    label="Reload organizations"
                    disabled={busy}
                    on_click={|| send(UiCommand::ReloadOrganizations)}
                />
            </List>
            <Card>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <ListItem grow=1.0>
                        <List spacing=Space::Xs>
                            <Text size=TextSize::Lg>{name}</Text>
                            <AccountDetails account />
                        </List>
                    </ListItem>
                    <Button
                        label="Switch account"
                        glyph=ICON_USERS_ROUND
                        variant=ButtonVariant::Secondary
                        on_click={|| send(UiCommand::SwitchAccount)}
                    />
                </List>
            </Card>
            <Text style=TextStyle::HEADING>"Open an organization"</Text>
            <ForEach
                value={keys}
                fallback={move || view! {
                    <Card>
                        <OrganizationsPlaceholder state />
                    </Card>
                }}
            >|id: Uuid| {
                let label = create_memo(move || {
                    organizations.get()
                        .organizations
                        .into_iter()
                        .find(|(organization, _)| *organization == id)
                        .map(|(_, name)| name)
                        .unwrap_or_default()
                });
                view! {
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                        <ListItem grow=1.0>
                            <Button
                                label={label}
                                glyph=ICON_BOXES
                                variant=ButtonVariant::Secondary
                                on_click={move || send(UiCommand::OpenOrganization(id))}
                            />
                        </ListItem>
                        <Button
                            icon_only=true
                            glyph=ICON_CHEVRON_DOWN
                            label="Choose a session or profile"
                            on_click={move || send(UiCommand::ChooseSession(id))}
                        />
                    </List>
                }
            }</ForEach>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                <Text align=TextAlign::Center>
                    <Icon glyph=ICON_USER_PLUS />
                </Text>
                <Text style=TextStyle::HEADING>"Join an organization"</Text>
            </List>
            <JoinOrganization view busy={busy} />
            <Text style=TextStyle::HEADING>"Create an organization"</Text>
            <CreateOrganization busy={busy} created />
            <ErrorText text={error} />
        </Column>
    }
}

#[component]
fn JoinOrganization(view: HostUiStore, busy: Memo<bool>) -> NodeId {
    let joining = create_memo(move || view.organizations.get().joining);
    let link = create_signal(String::new());
    let cannot = create_memo(move || busy.get() || link.get().trim().is_empty());
    let open = move || {
        if !cannot.get_untracked() {
            send(UiCommand::OpenInvite(link.get_untracked()));
        }
    };
    view! {
        <List spacing=Space::Md @test_id={"host.join"}>
            <Show
                value={joining}
                fallback={move || view! {
                    <List spacing=Space::Md>
                        <Text style=TextStyle::CAPTION>
                            "Paste the invite link someone sent you."
                        </Text>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <ListItem grow=1.0>
                                <TextInput
                                    value={link}
                                    label="Invite link"
                                    placeholder="https://…/#invite/…"
                                    @test_id={"host.join.link"}
                                    on_submit={move |_value: String| open()}
                                />
                            </ListItem>
                            <Button
                                label="Open"
                                variant=ButtonVariant::Secondary
                                disabled={cannot}
                                @test_id={"host.join.open"}
                                on_click={open}
                            />
                        </List>
                    </List>
                }}
            >|row: ReadSignal<JoiningRow>| {
                let title = create_memo(move || row.with(|row| format!("Join {}?", row.organization)));
                let invited = create_memo(move || {
                    row.with(|row| format!("Invited by {}. You join as an editor.", row.inviter))
                });
                let problem = create_memo(move || row.with(|row| row.problem.clone()));
                let can_join = create_memo(move || row.with(|row| row.can_join));
                let refused = create_memo(move || !problem.with(String::is_empty));
                view! {
                    <Card>
                        <List spacing=Space::Md>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Md
                            >
                                <ListItem grow=1.0>
                                    <List spacing=Space::Xs2>
                                        <Text size=TextSize::Lg>{title}</Text>
                                        <Text style=TextStyle::CAPTION>{invited}</Text>
                                    </List>
                                </ListItem>
                                <Button
                                    label="Cancel"
                                    variant=ButtonVariant::Secondary
                                    disabled={busy}
                                    on_click={|| send(UiCommand::CancelJoin)}
                                />
                                <Show value={can_join}>
                                    <Button
                                        label="Join"
                                        variant=ButtonVariant::Primary
                                        disabled={busy}
                                        @test_id={"host.join.confirm"}
                                        on_click={|| send(UiCommand::JoinInvited)}
                                    />
                                </Show>
                            </List>
                            <Show value={refused}>
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Md
                                >
                                    <ListItem grow=1.0>
                                        <Text
                                            style=TextStyle::CAPTION
                                            @test_id={"host.join.problem"}
                                        >
                                            {problem}
                                        </Text>
                                    </ListItem>
                                    <Button
                                        label="Use another account"
                                        variant=ButtonVariant::Secondary
                                        @test_id={"host.join.another"}
                                        on_click={|| send(UiCommand::OpenAddAccount)}
                                    />
                                </List>
                            </Show>
                        </List>
                    </Card>
                }
            }</Show>
        </List>
    }
}

#[component]
pub(super) fn SessionsScreen(view: HostUiStore) -> NodeId {
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
    let title = create_memo(move || status.get().organization);
    let every_type = create_memo(move || status.get().every_session_type);
    let session_type = create_memo(move || status.get().session_type);
    view! {
        <Column>
            <Text style=TextStyle::TITLE>{title}</Text>
            <Show value={loading}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Progress width=INLINE_PROGRESS_WIDTH />
                    <Text style=TextStyle::CAPTION>"Loading sessions and profiles…"</Text>
                </List>
            </Show>
            <Text style=TextStyle::HEADING>"Settings profile"</Text>
            <Text style=TextStyle::CAPTION>
                "The settings this device uses. Each device starts with a profile of its own."
            </Text>
            <ForEach value={profile_keys}>|id: Uuid| {
                let row = create_memo(move || {
                    status.get().profiles.into_iter().find(|profile| profile.id == id)
                });
                let label = create_memo(move || match row.get() {
                    Some(profile) if profile.current => format!("{} (this device)", profile.name),
                    Some(profile) => profile.name,
                    None => String::new(),
                });
                let glyph = create_memo(move || match row.get().is_some_and(|profile| profile.current) {
                    true => Some(ICON_CHECK),
                    false => None,
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
            }</ForEach>
            <Button
                label="New settings profile"
                glyph=ICON_PLUS
                variant=ButtonVariant::Ghost
                disabled={loading}
                @test_id={"sessions.new-profile"}
                on_click={|| send(UiCommand::NewProfile)}
            />
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <ListItem grow=1.0>
                    <Text style=TextStyle::HEADING>"Session"</Text>
                </ListItem>
                <Text style=TextStyle::CAPTION>"Show every type"</Text>
                <Switch
                    on={Binding::new(every_type, |every: bool| send(UiCommand::EverySessionType(every)))}
                    label="Show sessions of every type"
                />
            </List>
            <Text style=TextStyle::CAPTION>
                "The blocks you had open. Start a new session to begin again with the same settings."
            </Text>
            <ForEach value={session_keys}>|id: Uuid| {
                let label = create_memo(move || {
                    status.get()
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
            }</ForEach>
            <SplitButton
                label={move || format!("New {} session", session_type_name(session_type.get()))}
                glyph=ICON_PLUS
                variant=ButtonVariant::Secondary
                menu_label="New session of another type"
                disabled={loading}
                items={view! {
                    <ForEach value={SESSION_TYPES.to_vec()}>|(kind, name): (Uuid, &'static str)|
                        <MenuItem
                            label={format!("New {name} session")}
                            on_click={move || send(UiCommand::OpenNewSession(kind))}
                        />
                    </ForEach>
                }}
                on_click={move || send(UiCommand::OpenNewSession(session_type.get_untracked()))}
            />
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button
                    label="Keep the current session"
                    variant=ButtonVariant::Ghost
                    @test_id={"sessions.keep"}
                    on_click={|| send(UiCommand::CloseSessions)}
                />
                <Button
                    label="Back to organizations"
                    variant=ButtonVariant::Ghost
                    on_click={|| send(UiCommand::SwitchOrganization)}
                />
            </List>
        </Column>
    }
}

#[component]
fn OrganizationsPlaceholder(state: Memo<OrganizationsState>) -> NodeId {
    view! {
        <List spacing=Space::None>
            <Match value={state}>
                <Case is={OrganizationsState::Loaded}>
                    <Text style=TextStyle::CAPTION>
                        "You do not have any organizations yet. Create one below."
                    </Text>
                </Case>
                <Case is={OrganizationsState::Failed}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <Text style=TextStyle::CAPTION>"Could not load organizations."</Text>
                        <Button
                            label="Retry"
                            variant=ButtonVariant::Secondary
                            on_click={|| send(UiCommand::ReloadOrganizations)}
                        />
                    </List>
                </Case>
                <Case is={OrganizationsState::Loading}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <Progress width=INLINE_PROGRESS_WIDTH />
                        <Text style=TextStyle::CAPTION>"Loading organizations…"</Text>
                    </List>
                </Case>
            </Match>
        </List>
    }
}

#[component]
fn CreateOrganization(busy: Memo<bool>, created: Memo<u64>) -> NodeId {
    let name = create_linked_signal(move || {
        created.track();
        String::new()
    });
    let cannot = create_memo(move || busy.get() || name.get().trim().is_empty());
    let create = move || {
        if !cannot.get_untracked() {
            send(UiCommand::CreateOrganization(name.get_untracked()));
        }
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
            <ListItem grow=1.0>
                <TextInput
                    value={name}
                    placeholder="Organization name"
                    label="Organization name"
                    on_submit={move |_value: String| create()}
                />
            </ListItem>
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
pub(super) fn ReauthDialog(view: HostUiStore) -> NodeId {
    let reauth = view.reauth;
    let open = create_memo(move || reauth.with(Option::is_some) && !view.add_account.get().open);
    let busy = create_memo(move || reauth.get().is_some_and(|reauth| reauth.busy));
    let error = create_memo(move || reauth.get().and_then(|reauth| reauth.error));
    let message = create_memo(move || {
        format!(
            "Your session for {} is no longer valid. Continue in your browser to sign in again.",
            reauth.get().map(|reauth| reauth.name).unwrap_or_default()
        )
    });
    view! {
        <Dialog
            open={open}
            title="Session expired"
            width={Size::Custom(460.0)}
            kind=DialogKind::Alert
            id="reauth"
            actions={vec![
                DialogAction::secondary("Log out", || send(UiCommand::ReauthLogOut)).named("log-out"),
                DialogAction::cancel("Not now"),
                DialogAction::primary("Continue in browser", || send(UiCommand::ReauthSubmit)).busy(busy),
            ]}
            on_dismiss={|| send(UiCommand::ReauthClose)}
        >
            <List spacing=Space::Xl2>
                <Text style=TextStyle::PARAGRAPH>{message}</Text>
                <Show value={move || error.with(Option::is_some)}>
                    <ErrorText text={error} />
                </Show>
            </List>
        </Dialog>
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

#[component]
pub(super) fn RemoveAccountDialog(view: HostUiStore) -> NodeId {
    let removing = view.removing_account;
    let open = create_memo(move || removing.with(Option::is_some));
    let title = create_memo(move || format!("Remove {}?", removing.get().unwrap_or_default()));
    view! {
        <Dialog
            open={open}
            kind=DialogKind::Alert
            title={title}
            id="host.remove-account"
            actions={vec![
                DialogAction::destructive("Remove", || send(UiCommand::ConfirmRemoveAccount)).named("remove"),
                DialogAction::cancel("Keep"),
            ]}
            on_dismiss={|| send(UiCommand::CancelRemoveAccount)}
        >
            <Text style=TextStyle::PARAGRAPH>
                "This account lives only on this device and has no password, so once it is removed nobody can open it again. Its blocks and settings are lost, except what it shared with other accounts."
            </Text>
        </Dialog>
    }
}

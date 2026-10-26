use beui::icons::{
    ICON_BOXES, ICON_CHECK, ICON_CHEVRON_DOWN, ICON_CLOUD, ICON_COPY, ICON_ELLIPSIS, ICON_MONITOR,
    ICON_PLUS, ICON_REFRESH_CW, ICON_USER_PLUS, ICON_USERS_ROUND,
};
use beui::reactive::*;
use beui::styled::*;
use beui::{NodeId, TextAlign};
use uuid::Uuid;

use super::{
    AccountForm, AccountRow, AppViewStore, ErrorAction, OrganizationsState, UiCommand, send,
};
use crate::platform;
use crate::root_settings::{SESSION_TYPES, session_type_name};

const COLUMN_WIDTH: f32 = 460.0;

#[component]
pub(super) fn ErrorText(text: Memo<Option<String>>) -> NodeId {
    let shown = create_memo(move || text.get().is_some());
    let content = create_memo(move || text.get().unwrap_or_default());
    view! {
        <List spacing=Space::None>
            <Show condition={shown}>
                <Text style=TextStyle::PARAGRAPH color=TextColor::Danger>{content}</Text>
            </Show>
        </List>
    }
}

#[component]
pub(super) fn Column(children: beui::reactive::Children<beui::reactive::ListChild>) -> NodeId {
    view! {
        <Scroll>
            <List spacing=Space::None align=Align::Center>
                <Frame
                    width={Extent::at_most(COLUMN_WIDTH)}
                    padding={Padding::symmetric(Space::Xl, Space::Xl3)}
                >
                    <List spacing=Space::Lg>{children}</List>
                </Frame>
            </List>
        </Scroll>
    }
}

#[component]
pub(super) fn ErrorScreen(view: AppViewStore) -> NodeId {
    let message = create_memo(move || view.error.get().message);
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
                    on_click={move || copy_text(message.get_untracked())}
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
            <Dialog
                open={asking}
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
            <Text style=TextStyle::TITLE>"Block Editor"</Text>
            <Text style=TextStyle::CAPTION>"Choose an account to continue."</Text>
            <ForEach keys={keys}>|key: String| {
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
            }</ForEach>
            <Show condition={empty}>
                <Card>
                    <Text style=TextStyle::CAPTION>"No accounts yet. Add one to get started."</Text>
                </Card>
            </Show>
            <Button
                label="Add account"
                glyph=ICON_PLUS
                variant=ButtonVariant::Secondary
                on_click={|| send(UiCommand::OpenAddAccount)}
            />
            <ErrorText text={error} />
        </Column>
    }
}

#[component]
fn AccountCard(account: Memo<AccountRow>) -> NodeId {
    let name = create_memo(move || account.get().name);
    let key = create_memo(move || account.get().key);
    let open_hint = create_memo(move || match account.get().has_last_organization {
        true => "Open the last organization used".to_owned(),
        false => "Choose an organization".to_owned(),
    });
    view! {
        <Card>
            <List spacing=Space::Sm>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Text style=TextStyle::HEADING @sizing=ItemSize::Percent(100.0)>{name}</Text>
                    <MenuButton
                        label="Account options"
                        glyph=ICON_ELLIPSIS
                        icon_only=true
                        arrow=false
                        on_select={move |_path: Vec<usize>| {
                            send(UiCommand::LogOut(key.get_untracked()));
                        }}
                        items={view! {
                            <MenuItem
                                label={create_memo(move || if account.get().local { "Close account" } else { "Log out" }.to_owned())}
                            />
                        }}
                    />
                </List>
                <AccountDetails account />
                <List
                    direction=Direction::Horizontal
                    align=Align::Center
                    justify=Justify::End
                    spacing=Space::Xs
                >
                    <Tooltip label={open_hint}>
                        <Button
                            label="Open"
                            variant=ButtonVariant::Primary
                            on_click={move || send(UiCommand::OpenAccount(key.get_untracked()))}
                        />
                    </Tooltip>
                    <Button
                        icon_only=true
                        glyph=ICON_CHEVRON_DOWN
                        label="Open a different organization"
                        on_click={move || send(UiCommand::ChooseOrganization(key.get_untracked()))}
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
        true => ICON_MONITOR,
        false => ICON_CLOUD,
    });
    view! {
        <List spacing=Space::Xs2>
            <Show condition={create_memo(move || !email.get().is_empty())}>
                <Text style=TextStyle::CAPTION>{email}</Text>
            </Show>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                <Text size=TextSize::Sm align=TextAlign::Center>
                    <Icon glyph={glyph} />
                </Text>
                <Text style=TextStyle::CAPTION>{server}</Text>
            </List>
        </List>
    }
}

#[component]
pub(super) fn AddAccountDialog(
    view: AppViewStore,
    #[prop(default = platform::default_account_server())] default_server: Option<
        build_id::DefaultServer,
    >,
) -> NodeId {
    let open = create_memo(move || view.add_account.get().open);
    let generation = create_memo(move || view.add_account.get().generation);
    let pending = create_memo(move || view.add_account.get().pending);
    let error = create_memo(move || view.add_account.get().error);
    let browser = create_memo(move || view.add_account.get().browser);
    let waiting = create_memo(move || browser.get().is_some());
    let browser_url = create_memo(move || browser.get().map(|login| login.url).unwrap_or_default());
    let browser_code =
        create_memo(move || browser.get().map(|login| login.code).unwrap_or_default());
    let default = create_memo(move || default_server.clone());
    let has_default = default.get_untracked().is_some();
    let default_name =
        create_memo(move || default.get().map(|server| server.name).unwrap_or_default());
    let selection = create_signal(None::<u8>);
    let remote_url = create_signal(String::new());
    let authorization = create_signal(String::new());
    let display_name = create_signal(String::new());
    create_effect(move || {
        generation.get();
        untrack(|| {
            selection.set(None);
            remote_url.set(String::new());
            authorization.set(String::new());
            display_name.set(String::new());
        });
    });
    let choosing = create_memo(move || selection.get().is_none() && !pending.get());
    let editing = create_memo(move || selection.get().is_some() && !pending.get());
    let local = create_memo(move || selection.get() == Some(1));
    let custom = create_memo(move || selection.get() == Some(2));
    let remote = create_memo(move || selection.get().is_some() && !local.get());
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
    let not_ready = create_memo(move || !ready.get());
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
    let primary_disabled = create_memo(move || !waiting.get() && not_ready.get());
    let default_address = create_memo(move || {
        default
            .get()
            .map(|server| server.api_url)
            .unwrap_or_default()
    });
    let is_default = create_memo(move || selection.get() == Some(0));
    let show_gateway = create_memo(move || remote.get() && platform::SENDS_HEADERS);
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
                DialogAction::primary(footer_primary, move || {
                    if waiting.get_untracked() {
                        send(UiCommand::OpenLoginBrowser);
                    } else {
                        submit();
                    }
                })
                .disabled(primary_disabled)
                .busy(connecting)
                .shown(create_memo(move || !footer_primary.get().is_empty())),
            ]}
            on_dismiss={|| send(UiCommand::CloseAddAccount)}
        >
            <List spacing=Space::Xl2>
                <Show condition={choosing}>
                    <List spacing=Space::Lg>
                        <Text style=TextStyle::PARAGRAPH>"Choose where to keep your account."</Text>
                        <Frame
                            fill=ColorRole::Surface
                            border={Border::DEFAULT}
                            radius=Radius::Md
                            padding=Space::Sm
                        >
                            <List spacing=Space::Xs>
                                <Show condition={has_default}>
                                    <ActionRow
                                        label={default_name}
                                        detail={default_address}
                                        glyph=ICON_CLOUD
                                        @test_id="account.default"
                                        on_click={move || selection.set(Some(0))}
                                    />
                                </Show>
                                <Show condition={platform::HAS_EMBEDDED_SERVER}>
                                    <ActionRow
                                        label="On this device"
                                        detail="A local account, stored on this device"
                                        glyph=ICON_MONITOR
                                        @test_id="account.local"
                                        on_click={move || selection.set(Some(1))}
                                    />
                                </Show>
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
                <Show condition={editing}>
                    <List spacing=Space::Xl>
                        <Show condition={local}>
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
                        </Show>
                        <Show condition={is_default}>
                            <List spacing=Space::Xs>
                                <Text style=TextStyle::HEADING>{default_name}</Text>
                                <Text style=TextStyle::CAPTION>{server_name}</Text>
                            </List>
                        </Show>
                        <Show condition={custom}>
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
                        </Show>
                        <Show condition={show_gateway}>
                            <List spacing=Space::Sm>
                                <Text style=TextStyle::CAPTION>{gateway_label}</Text>
                                <TextInput
                                    value={authorization}
                                    label={gateway_label}
                                    password=true
                                    @test_id="account.gateway"
                                    on_submit={move |_| submit()}
                                />
                            </List>
                        </Show>
                        <Show condition={remote}>
                            <Text style=TextStyle::CAPTION>
                                "Next, you’ll sign in or create an account in your browser."
                            </Text>
                        </Show>
                    </List>
                </Show>
                <Show condition={waiting}>
                    <List spacing=Space::Xl>
                        <Text style=TextStyle::PARAGRAPH>
                            "Sign in and approve this code in your browser. You can also use a browser on another device."
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
                                        <SelectableText
                                            @sizing=ItemSize::Percent(100.0)
                                            child_size=ItemSize::Percent(100.0)
                                        >
                                            <Text
                                                style=TextStyle::TITLE
                                                @test_id="account.browser-code"
                                            >
                                                {browser_code}
                                            </Text>
                                        </SelectableText>
                                        <Button
                                            icon_only=true
                                            label="Copy code"
                                            glyph=ICON_COPY
                                            variant=ButtonVariant::Ghost
                                            @test_id="account.copy-code"
                                            on_click={move || copy_text(browser_code.get_untracked())}
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
                                        <SelectableText
                                            @sizing=ItemSize::Percent(100.0)
                                            child_size=ItemSize::Percent(100.0)
                                        >
                                            <Text
                                                style=TextStyle::CAPTION
                                                @test_id="account.browser-url"
                                            >
                                                {browser_url}
                                            </Text>
                                        </SelectableText>
                                        <Button
                                            icon_only=true
                                            label="Copy link"
                                            glyph=ICON_COPY
                                            variant=ButtonVariant::Ghost
                                            @test_id="account.copy-link"
                                            on_click={move || copy_text(browser_url.get_untracked())}
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
                <Show condition={connecting}>
                    <Text style=TextStyle::PARAGRAPH>"Connecting to the server…"</Text>
                </Show>
                <Show condition={create_memo(move || error.get().is_some())}>
                    <ErrorText text={error} />
                </Show>
            </List>
        </Dialog>
    }
}

#[component]
pub(super) fn OrganizationsScreen(view: AppViewStore) -> NodeId {
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
    let empty = create_memo(move || keys.get().is_empty());
    let state = create_memo(move || organizations.get().state);
    let error = create_memo(move || organizations.get().error);
    let created = create_memo(move || organizations.get().created);
    let account = create_memo(move || view.account.get());
    let name = create_memo(move || account.get().name);
    view! {
        <Column>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Text style=TextStyle::TITLE @sizing=ItemSize::Percent(100.0)>"Organizations"</Text>
                <MenuButton
                    label="Account options"
                    glyph=ICON_ELLIPSIS
                    icon_only=true
                    arrow=false
                    on_select={|_path: Vec<usize>| send(UiCommand::LogOutCurrent)}
                    items={view! {
                        <MenuItem label="Log out" />
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
                    <List @sizing=ItemSize::Percent(100.0) spacing=Space::Xs>
                        <Text style=TextStyle::HEADING>{name}</Text>
                        <AccountDetails account />
                    </List>
                    <Button
                        label="Switch account"
                        glyph=ICON_USERS_ROUND
                        variant=ButtonVariant::Secondary
                        on_click={|| send(UiCommand::SwitchAccount)}
                    />
                </List>
            </Card>
            <Text style=TextStyle::HEADING>"Open an organization"</Text>
            <ForEach keys={keys}>|id: Uuid| {
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
                        <Button
                            @sizing=ItemSize::Percent(100.0)
                            label={label}
                            glyph=ICON_BOXES
                            variant=ButtonVariant::Secondary
                            on_click={move || send(UiCommand::OpenOrganization(id))}
                        />
                        <Button
                            icon_only=true
                            glyph=ICON_CHEVRON_DOWN
                            label="Choose a session or profile"
                            on_click={move || send(UiCommand::ChooseSession(id))}
                        />
                    </List>
                }
            }</ForEach>
            <Show condition={empty}>
                <Card>
                    <OrganizationsPlaceholder state />
                </Card>
            </Show>
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
fn JoinOrganization(view: AppViewStore, busy: Memo<bool>) -> NodeId {
    let joining = create_memo(move || view.organizations.get().joining);
    let asking = create_memo(move || joining.get().is_some());
    let pasting = create_memo(move || !asking.get());
    let title = create_memo(move || {
        joining
            .get()
            .map(|row| format!("Join {} as {}?", row.organization, row.role))
            .unwrap_or_default()
    });
    let invited = create_memo(move || {
        joining
            .get()
            .map(|row| format!("Invited as {}", row.email))
            .unwrap_or_default()
    });
    let link = create_signal(String::new());
    let cannot = create_memo(move || busy.get() || link.get().trim().is_empty());
    let open = move || {
        if !cannot.get_untracked() {
            send(UiCommand::OpenInvite(link.get_untracked()));
        }
    };
    view! {
        <List spacing=Space::Md @test_id={"app.join"}>
            <Show condition={asking}>
                <Card>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <List @sizing=ItemSize::Percent(100.0) spacing=Space::Xs2>
                            <Text style=TextStyle::HEADING>{title}</Text>
                            <Text style=TextStyle::CAPTION>{invited}</Text>
                        </List>
                        <Button
                            label="Cancel"
                            variant=ButtonVariant::Secondary
                            disabled={busy}
                            on_click={|| send(UiCommand::CancelJoin)}
                        />
                        <Button
                            label="Join"
                            variant=ButtonVariant::Primary
                            disabled={busy}
                            @test_id={"app.join.confirm"}
                            on_click={|| send(UiCommand::JoinInvited)}
                        />
                    </List>
                </Card>
            </Show>
            <Show condition={pasting}>
                <List spacing=Space::Md>
                    <Text style=TextStyle::CAPTION>"Paste the invite link someone sent you."</Text>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <TextInput
                            @sizing=ItemSize::Percent(100.0)
                            value={link}
                            label="Invite link"
                            placeholder="https://…/#invite/…"
                            @test_id={"app.join.link"}
                            on_submit={move |_value: String| open()}
                        />
                        <Button
                            label="Open"
                            variant=ButtonVariant::Secondary
                            disabled={cannot}
                            @test_id={"app.join.open"}
                            on_click={open}
                        />
                    </List>
                </List>
            </Show>
        </List>
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
    let title = create_memo(move || status.get().organization);
    let every_type = create_memo(move || status.get().every_session_type);
    let session_type = create_memo(move || status.get().session_type);
    let new_label =
        create_memo(move || format!("New {} session", session_type_name(session_type.get())));
    view! {
        <Column>
            <Text style=TextStyle::TITLE>{title}</Text>
            <Show condition={loading}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Progress width=INLINE_PROGRESS_WIDTH />
                    <Text style=TextStyle::CAPTION>"Loading sessions and profiles…"</Text>
                </List>
            </Show>
            <Text style=TextStyle::HEADING>"Settings profile"</Text>
            <Text style=TextStyle::CAPTION>
                "The settings this device uses. Each device starts with a profile of its own."
            </Text>
            <ForEach keys={profile_keys}>|id: Uuid| {
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
                <Text style=TextStyle::HEADING @sizing=ItemSize::Percent(100.0)>"Session"</Text>
                <Text style=TextStyle::CAPTION>"Show every type"</Text>
                <Switch
                    on={Binding::new(every_type, |every: bool| send(UiCommand::EverySessionType(every)))}
                    label="Show sessions of every type"
                />
            </List>
            <Text style=TextStyle::CAPTION>
                "The blocks you had open. Start a new session to begin again with the same settings."
            </Text>
            <ForEach keys={session_keys}>|id: Uuid| {
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
                label={new_label}
                glyph=ICON_PLUS
                variant=ButtonVariant::Secondary
                menu_label="New session of another type"
                disabled={loading}
                items={view! {
                    <ForEach keys={(0..SESSION_TYPES.len()).collect::<Vec<_>>()}>|index: usize|
                        <MenuItem label={format!("New {} session", SESSION_TYPES[index].1)} />
                    </ForEach>
                }}
                on_click={move || send(UiCommand::OpenNewSession(session_type.get_untracked()))}
                on_select={|path: Vec<usize>| {
                    if let Some((editor, _)) = path.first().and_then(|index| SESSION_TYPES.get(*index)) {
                        send(UiCommand::OpenNewSession(*editor));
                    }
                }}
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
    let loaded = create_memo(move || state.get() == OrganizationsState::Loaded);
    let failed = create_memo(move || state.get() == OrganizationsState::Failed);
    let loading = create_memo(move || state.get() == OrganizationsState::Loading);
    view! {
        <List spacing=Space::None>
            <Show condition={loaded}>
                <Text style=TextStyle::CAPTION>
                    "You do not have any organizations yet. Create one below."
                </Text>
            </Show>
            <Show condition={failed}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Text style=TextStyle::CAPTION>"Could not load organizations."</Text>
                    <Button
                        label="Retry"
                        variant=ButtonVariant::Secondary
                        on_click={|| send(UiCommand::ReloadOrganizations)}
                    />
                </List>
            </Show>
            <Show condition={loading}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Progress width=INLINE_PROGRESS_WIDTH />
                    <Text style=TextStyle::CAPTION>"Loading organizations…"</Text>
                </List>
            </Show>
        </List>
    }
}

#[component]
fn CreateOrganization(busy: Memo<bool>, created: Memo<u64>) -> NodeId {
    let name = create_signal(String::new());
    create_effect(move || {
        created.get();
        untrack(|| name.set(String::new()));
    });
    let cannot = create_memo(move || busy.get() || name.get().trim().is_empty());
    let create = move || {
        if !cannot.get_untracked() {
            send(UiCommand::CreateOrganization(name.get_untracked()));
        }
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
            <TextInput
                @sizing=ItemSize::Percent(100.0)
                value={name}
                placeholder="Organization name"
                label="Organization name"
                on_submit={move |_value: String| create()}
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
pub(super) fn ReauthDialog(view: AppViewStore) -> NodeId {
    let reauth = view.reauth;
    let open = create_memo(move || reauth.get().is_some() && !view.add_account.get().open);
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
                <Show condition={create_memo(move || error.get().is_some())}>
                    <ErrorText text={error} />
                </Show>
            </List>
        </Dialog>
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

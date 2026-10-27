use be_protocol::OrganizationRole;
use beui::NodeId;
use beui::Overflow;
use beui::icons::{ICON_TRASH, ICON_USER_MINUS};
use beui::reactive::*;
use beui::styled::*;
use uuid::Uuid;

use block_accounts_ui::ErrorText;

use super::{AppViewStore, UiCommand, send};
use beui::{Align, Direction, ItemSize};

#[component]
pub(super) fn Dialogs(view: AppViewStore) -> NodeId {
    view! {
        <List spacing=Space::None>
            <DiscardDialog view={view} />
            <LeaveDialog view={view} />
            <InviteDialog view={view} />
            <AgentsDialog view={view} />
            <ContactsDialog view={view} />
            <MembersDialog view={view} />
            <AboutDialog view={view} />
            <UnsentDialog view={view} />
        </List>
    }
}

#[component]
fn InviteDialog(view: AppViewStore) -> NodeId {
    let invite = view.invite;
    let open = create_memo(move || invite.get().is_some());
    let busy = create_memo(move || invite.get().is_some_and(|invite| invite.busy));
    let sent = create_memo(move || invite.get().map(|invite| invite.sent));
    let email = create_signal(String::new());
    let administrator = create_signal(false);
    create_effect(move || {
        sent.get();
        untrack(|| email.set(String::new()));
    });
    let role = create_memo(move || match administrator.get() {
        true => OrganizationRole::Administrator,
        false => OrganizationRole::Editor,
    });
    let cannot = create_memo(move || email.get().trim().is_empty());
    view! {
        <Dialog
            open={open}
            title="Invite member"
            width={Size::Custom(380.0)}
            id="app.invite"
            actions={vec![
                DialogAction::primary("Create invite link", move || {
                    send(UiCommand::SendInvite(email.get_untracked(), role.get_untracked()))
                })
                .disabled(cannot)
                .busy(busy)
                .named("send"),
            ]}
            on_dismiss={|| send(UiCommand::CloseInvite)}
        >
            <InvitePanel view email administrator role />
        </Dialog>
    }
}

#[component]
fn AgentsDialog(view: AppViewStore) -> NodeId {
    let agents = view.agents;
    let open = create_memo(move || agents.get().is_some());
    let title = create_memo(
        move || match agents.get().and_then(|agents| agents.created) {
            Some(_) => "Agent credentials".to_owned(),
            None => "Agents".to_owned(),
        },
    );
    let made = create_memo(move || agents.get().and_then(|agents| agents.created).is_some());
    let creating = create_memo(move || !made.get());
    let busy = create_memo(move || agents.get().is_some_and(|agents| agents.busy));
    let name = create_signal(String::new());
    create_effect(move || {
        if made.get() {
            untrack(|| name.set(String::new()));
        }
    });
    let cannot = create_memo(move || name.get().trim().is_empty());
    view! {
        <Dialog
            open={open}
            title={title}
            width={Size::Custom(460.0)}
            id="app.agents"
            actions={vec![
                DialogAction::primary("Add agent", move || send(UiCommand::CreateAgent(name.get_untracked())))
                    .disabled(cannot)
                    .busy(busy)
                    .shown(creating)
                    .named("add"),
                DialogAction::cancel("Done").shown(made),
            ]}
            on_dismiss={|| send(UiCommand::CloseAgents)}
        >
            <AgentsPanel view name />
        </Dialog>
    }
}

#[component]
fn ContactsDialog(view: AppViewStore) -> NodeId {
    let contacts = view.contacts;
    let open = create_memo(move || contacts.get().is_some());
    let busy = create_memo(move || contacts.get().is_some_and(|contacts| contacts.busy));
    view! {
        <Dialog
            open={open}
            title="Contacts"
            width={Size::Custom(460.0)}
            id="app.contacts"
            actions={vec![
                DialogAction::primary("New contact link", || send(UiCommand::NewContactLink))
                    .busy(busy)
                    .named("new-link"),
            ]}
            on_dismiss={|| send(UiCommand::CloseContacts)}
        >
            <ContactsPanel view />
        </Dialog>
    }
}

#[component]
fn MembersDialog(view: AppViewStore) -> NodeId {
    let members = view.members;
    let open = create_memo(move || members.get().is_some());
    view! {
        <Dialog
            open={open}
            title="Members"
            width={Size::Custom(480.0)}
            id="app.members"
            actions={vec![
                DialogAction::primary("Invite member", || {
                    send(UiCommand::CloseMembers);
                    send(UiCommand::InviteMember);
                })
                .named("invite"),
            ]}
            on_dismiss={|| send(UiCommand::CloseMembers)}
        >
            <MembersPanel view />
        </Dialog>
    }
}

#[component]
fn MembersPanel(view: AppViewStore) -> NodeId {
    let members = view.members;
    let shown = create_memo(move || members.get().unwrap_or_default());
    let busy = create_memo(move || shown.get().busy);
    let error = create_memo(move || shown.get().error);
    let keys = create_memo(move || {
        shown
            .get()
            .members
            .iter()
            .map(|member| member.account)
            .collect::<Vec<Uuid>>()
    });
    let removing = create_memo(move || shown.get().removing.unwrap_or_default());
    let asks_removal = create_memo(move || !removing.get().is_empty());
    view! {
        <Frame>
            <List spacing=Space::Md>
                <Text style=TextStyle::CAPTION>
                    "Administrators can open every block in this organization. Removing someone takes away what was shared with them here, and gives new keys to everything they could open."
                </Text>
                <ForEach keys={keys}>|account: Uuid| {
                    let row = create_memo(move || {
                        shown
                            .get()
                            .members
                            .into_iter()
                            .find(|member| member.account == account)
                            .unwrap_or_default()
                    });
                    let name = create_memo(move || row.get().name);
                    let detail = create_memo(move || row.get().detail);
                    let fixed = create_memo(move || {
                        let row = row.get();
                        row.me || row.agent || busy.get()
                    });
                    let role_label = create_memo(move || match row.get().administrator {
                        true => "Make editor".to_owned(),
                        false => "Make administrator".to_owned(),
                    });
                    let next_role = move || match row.get_untracked().administrator {
                        true => OrganizationRole::Editor,
                        false => OrganizationRole::Administrator,
                    };
                    let unremovable = create_memo(move || row.get().me || busy.get());
                    view! {
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <List @sizing=ItemSize::Percent(100.0) spacing=Space::Xs2>
                                <Text>{name}</Text>
                                <Text style=TextStyle::CAPTION>{detail}</Text>
                            </List>
                            <Button
                                label={role_label}
                                variant=ButtonVariant::Secondary
                                disabled={fixed}
                                @test_id={format!("app.members.role.{account}")}
                                on_click={move || send(UiCommand::SetMemberRole(account, next_role()))}
                            />
                            <Button
                                icon_only=true
                                glyph=ICON_USER_MINUS
                                label="Remove member"
                                disabled={unremovable}
                                @test_id={format!("app.members.remove.{account}")}
                                on_click={move || send(UiCommand::RemoveMember(account))}
                            />
                        </List>
                    }
                }</ForEach>
                <Show condition={busy}>
                    <Progress width=INLINE_PROGRESS_WIDTH />
                </Show>
                <ErrorText text={error} />
                <Dialog
                    open={asks_removal}
                    kind=DialogKind::Alert
                    title="Remove member?"
                    id="app.members.remove"
                    actions={vec![
                        DialogAction::destructive("Remove", || send(UiCommand::ConfirmRemoveMember)),
                        DialogAction::cancel("Keep"),
                    ]}
                    on_dismiss={|| send(UiCommand::CancelRemoveMember)}
                >
                    <Text style=TextStyle::PARAGRAPH>{removing}</Text>
                </Dialog>
            </List>
        </Frame>
    }
}

#[component]
fn ContactsPanel(view: AppViewStore) -> NodeId {
    let contacts = view.contacts;
    let shown = create_memo(move || contacts.get().unwrap_or_default());
    let busy = create_memo(move || shown.get().busy);
    let error = create_memo(move || shown.get().error);
    let keys = create_memo(move || {
        shown
            .get()
            .contacts
            .iter()
            .map(|contact| contact.account)
            .collect::<Vec<Uuid>>()
    });
    let none = create_memo(move || keys.with(Vec::is_empty));
    let link = create_memo(move || shown.get().link.unwrap_or_default());
    let has_link = create_memo(move || !link.get().is_empty());
    let offer = create_memo(move || shown.get().offer.unwrap_or_default());
    let offered = create_memo(move || !offer.get().is_empty());
    let connected = create_memo(move || shown.get().connected.unwrap_or_default());
    let has_connected = create_memo(move || !connected.get().is_empty());
    let removing = create_memo(move || shown.get().removing.unwrap_or_default());
    let asks_removal = create_memo(move || !removing.get().is_empty());
    let pasted = create_signal(String::new());
    create_effect(move || {
        if offered.get() {
            untrack(|| pasted.set(String::new()));
        }
    });
    let cannot_open = create_memo(move || busy.get() || pasted.get().trim().is_empty());
    view! {
        <Frame>
            <List spacing=Space::Md>
                <Text style=TextStyle::CAPTION>
                    "You can share only with your contacts. Someone becomes one when you send them a contact link yourself, through an app you trust, and they open it, or when they send you theirs."
                </Text>
                <Show condition={none}>
                    <Text style=TextStyle::CAPTION>"No contacts yet."</Text>
                </Show>
                <ForEach keys={keys}>|account: Uuid| {
                    let row = create_memo(move || {
                        shown
                            .get()
                            .contacts
                            .into_iter()
                            .find(|contact| contact.account == account)
                            .unwrap_or_default()
                    });
                    let name = create_memo(move || row.get().name);
                    let detail = create_memo(move || {
                        let row = row.get();
                        match row.verified {
                            true => row.email,
                            false if row.email.is_empty() => "Not confirmed yet".to_owned(),
                            false => format!("{}, not confirmed yet", row.email),
                        }
                    });
                    view! {
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <List @sizing=ItemSize::Percent(100.0) spacing=Space::Xs2>
                                <Text>{name}</Text>
                                <Text style=TextStyle::CAPTION>{detail}</Text>
                            </List>
                            <Button
                                icon_only=true
                                glyph=ICON_TRASH
                                label="Remove contact"
                                @test_id={format!("app.contacts.remove.{account}")}
                                on_click={move || send(UiCommand::RemoveContact(account))}
                            />
                        </List>
                    }
                }</ForEach>
                <Dialog
                    open={asks_removal}
                    kind=DialogKind::Alert
                    title="Remove contact?"
                    id="app.contacts.remove"
                    actions={vec![
                        DialogAction::destructive("Remove", || send(UiCommand::ConfirmRemoveContact)),
                        DialogAction::cancel("Keep"),
                    ]}
                    on_dismiss={|| send(UiCommand::CancelRemoveContact)}
                >
                    <Text style=TextStyle::PARAGRAPH>{removing}</Text>
                </Dialog>
                <Show condition={has_link}>
                    <List spacing=Space::Sm>
                        <Text style=TextStyle::PARAGRAPH @test_id={"app.contacts.link-help"}>
                            "Send this link yourself, through an app you trust, to the one person you want to connect with. It works once, and it is shown only once."
                        </Text>
                        <TextInput
                            value={Binding::new(link, Callback::empty())}
                            label="Contact link"
                            copyable=true
                            @test_id={"app.contacts.link"}
                        />
                    </List>
                </Show>
                <Text style=TextStyle::CAPTION>"Open a contact link someone sent you"</Text>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <TextInput
                        @sizing=ItemSize::Percent(100.0)
                        value={pasted}
                        label="Contact link"
                        @test_id={"app.contacts.paste"}
                        on_submit={move |value: String| send(UiCommand::OpenContactLink(value))}
                    />
                    <Button
                        label="Open"
                        variant=ButtonVariant::Secondary
                        disabled={cannot_open}
                        @test_id={"app.contacts.open"}
                        on_click={move || send(UiCommand::OpenContactLink(pasted.get_untracked()))}
                    />
                </List>
                <Dialog
                    open={offered}
                    kind=DialogKind::Alert
                    title="Connect?"
                    id="app.contacts.offered"
                    actions={vec![
                        DialogAction::primary("Connect", || send(UiCommand::AcceptContact)).named("accept"),
                        DialogAction::cancel("Cancel"),
                    ]}
                    on_dismiss={|| send(UiCommand::DeclineContact)}
                >
                    <Text style=TextStyle::PARAGRAPH @test_id={"app.contacts.offer"}>{offer}</Text>
                </Dialog>
                <Show condition={has_connected}>
                    <Text style=TextStyle::PARAGRAPH>{connected}</Text>
                </Show>
                <ErrorText text={error} />
            </List>
        </Frame>
    }
}

#[component]
fn AgentsPanel(view: AppViewStore, name: Signal<String>) -> NodeId {
    let agents = view.agents;
    let shown = create_memo(move || agents.get().unwrap_or_default());
    let organization = create_memo(move || format!("Organization: {}", shown.get().organization));
    let error = create_memo(move || shown.get().error);
    let created = create_memo(move || shown.get().created);
    let made = create_memo(move || created.get().is_some());
    let creating = create_memo(move || !made.get());
    let keys = create_memo(move || {
        shown
            .get()
            .agents
            .iter()
            .map(|agent| agent.account)
            .collect::<Vec<Uuid>>()
    });
    let none = create_memo(move || keys.with(Vec::is_empty));
    view! {
        <Frame>
            <List spacing=Space::Md>
                <Show condition={creating}>
                    <List spacing=Space::Md>
                        <Text style=TextStyle::CAPTION>{organization}</Text>
                        <Text style=TextStyle::CAPTION>
                            "An agent is an account for a program, such as a coding agent. It sees only the blocks you share with it. Share a workspace or blocks with it from this device after setup. This list shows the agents you created."
                        </Text>
                        <Show condition={none}>
                            <Text style=TextStyle::CAPTION>"No agents yet."</Text>
                        </Show>
                        <ForEach keys={keys}>|account: Uuid| {
                            let label = create_memo(move || {
                                shown
                                    .get()
                                    .agents
                                    .iter()
                                    .find(|agent| agent.account == account)
                                    .map(|agent| agent.name.clone())
                                    .unwrap_or_default()
                            });
                            view! {
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Md
                                >
                                    <Text
                                        overflow=Overflow::Ellipsis
                                        @sizing=ItemSize::Percent(100.0)
                                    >
                                        {label}
                                    </Text>
                                    <Button
                                        icon_only=true
                                        glyph=ICON_TRASH
                                        label="Remove agent"
                                        @test_id={format!("app.agents.remove.{account}")}
                                        on_click={move || send(UiCommand::RemoveAgent(account))}
                                    />
                                </List>
                            }
                        }</ForEach>
                        <Text style=TextStyle::CAPTION>"Name"</Text>
                        <TextInput value={name} label="Agent name" @test_id={"app.agents.name"} />
                        <ErrorText text={error} />
                    </List>
                </Show>
                <Show condition={made}>
                    <AgentCredentials created={created} />
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn AgentCredentials(created: Memo<Option<super::CreatedAgent>>) -> NodeId {
    let field = |read: fn(&super::CreatedAgent) -> String| {
        create_memo(move || created.get().as_ref().map(read).unwrap_or_default())
    };
    let heading = field(|created| {
        format!(
            "Copy these secret credentials for {}. They are shown only once. Set BE3_AGENT_CREDENTIALS to this value, then share a workspace or blocks with the agent.",
            created.name
        )
    });
    let proxy = create_signal(false);
    let credentials = create_memo(move || {
        let Some(created) = created.get() else {
            return String::new();
        };
        let mut bundle = serde_json::json!({
            "server": created.server,
            "organization": created.organization,
            "recovery_phrase": created.phrase,
        });
        if proxy.get() {
            bundle["authentication"] = "proxy".into();
        } else {
            bundle["token"] = created.token.into();
        }
        bundle.to_string()
    });
    let header = field(|created| format!("Bearer {}", created.token));
    view! {
        <List spacing=Space::Sm>
            <Text style=TextStyle::PARAGRAPH @test_id={"app.agents.credentials-help"}>
                {heading}
            </Text>
            <Checkbox
                label="Account token supplied by proxy"
                checked={proxy}
                @test_id={"app.agents.proxy"}
            />
            <Show condition={proxy}>
                <List spacing=Space::Sm>
                    <Text style=TextStyle::PARAGRAPH>
                        "Add a proxy rule for the server in these credentials (api.be3.pfg.pw for the hosted service). Set its Authorization header to the value below and keep it outside the agent container. If a gateway secret is required, add Be3-Gateway-Authorization: Bearer <gateway secret> as a separate rule."
                    </Text>
                    <TextInput
                        value={Binding::new(header, Callback::empty())}
                        label="Proxy Authorization header"
                        copyable=true
                        @test_id={"app.agents.header"}
                    />
                </List>
            </Show>
            <TextInput
                value={Binding::new(credentials, Callback::empty())}
                label="Agent credentials"
                copyable=true
                @test_id={"app.agents.credentials"}
            />
        </List>
    }
}

#[component]
fn UnsentDialog(view: AppViewStore) -> NodeId {
    let unsent = view.unsent;
    let open = create_memo(move || unsent.get().is_some());
    view! {
        <Dialog
            open={open}
            title="Unsent edits"
            width={Size::Custom(460.0)}
            id="app.unsent"
            on_dismiss={|| send(UiCommand::Unsent(false))}
        >
            <UnsentPanel view />
        </Dialog>
    }
}

#[component]
fn UnsentPanel(view: AppViewStore) -> NodeId {
    let unsent = view.unsent;
    let shown = create_memo(move || unsent.get().unwrap_or_default());
    let keys = create_memo(move || {
        shown
            .get()
            .rows
            .iter()
            .map(|row| row.block)
            .collect::<Vec<Uuid>>()
    });
    let none = create_memo(move || keys.with(Vec::is_empty));
    let discarding = create_memo(move || {
        let shown = shown.get();
        let block = shown.discarding?;
        let name = shown
            .rows
            .iter()
            .find(|row| row.block == block)
            .map(|row| row.name.clone())
            .unwrap_or_default();
        Some(format!(
            "Discard the edits to {name}? They are kept only on this device, and cannot be brought back."
        ))
    });
    let asks_discard = create_memo(move || discarding.with(Option::is_some));
    let discarding = create_memo(move || discarding.get().unwrap_or_default());
    view! {
        <Frame>
            <List spacing=Space::Md>
                <Text style=TextStyle::CAPTION>
                    "These edits were made on this device but could not be sent, because you can no longer edit these blocks. They stay here until you restore or discard them. Restore sends them again once you can edit the block."
                </Text>
                <Show condition={none}>
                    <Text style=TextStyle::CAPTION>"No unsent edits."</Text>
                </Show>
                <ForEach keys={keys}>|block: Uuid| {
                    let row = create_memo(move || {
                        shown
                            .get()
                            .rows
                            .into_iter()
                            .find(|row| row.block == block)
                            .unwrap_or_default()
                    });
                    let name = create_memo(move || row.get().name);
                    let detail = create_memo(move || match row.get().offerable {
                        true => "You can edit it again".to_owned(),
                        false => "You cannot edit it now".to_owned(),
                    });
                    let blocked = create_memo(move || !row.get().offerable);
                    view! {
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <List @sizing=ItemSize::Percent(100.0) spacing=Space::Xs2>
                                <Text>{name}</Text>
                                <Text style=TextStyle::CAPTION>{detail}</Text>
                            </List>
                            <Button
                                label="Restore"
                                variant=ButtonVariant::Primary
                                disabled={blocked}
                                @test_id={format!("app.unsent.restore.{block}")}
                                on_click={move || send(UiCommand::RestoreUnsent(block))}
                            />
                            <Button
                                icon_only=true
                                glyph=ICON_TRASH
                                label="Discard"
                                @test_id={format!("app.unsent.discard.{block}")}
                                on_click={move || send(UiCommand::DiscardUnsent(block))}
                            />
                        </List>
                    }
                }</ForEach>
                <Dialog
                    open={asks_discard}
                    kind=DialogKind::Alert
                    title="Discard edits?"
                    id="app.unsent.discard"
                    actions={vec![
                        DialogAction::destructive("Discard", || send(UiCommand::ConfirmDiscardUnsent)),
                        DialogAction::cancel("Keep"),
                    ]}
                    on_dismiss={|| send(UiCommand::CancelDiscardUnsent)}
                >
                    <Text style=TextStyle::PARAGRAPH>{discarding}</Text>
                </Dialog>
            </List>
        </Frame>
    }
}

#[component]
fn AboutDialog(view: AppViewStore) -> NodeId {
    let open = view.about;
    view! {
        <Dialog
            open={open}
            title="About"
            width={Size::Custom(420.0)}
            on_dismiss={|| send(UiCommand::About(false))}
        >
            <AboutPanel />
        </Dialog>
    }
}

#[component]
fn InvitePanel(
    view: AppViewStore,
    email: Signal<String>,
    administrator: Signal<bool>,
    role: Memo<OrganizationRole>,
) -> NodeId {
    let invite = view.invite;
    let organization = create_memo(move || {
        format!(
            "Organization: {}",
            invite
                .get()
                .map(|invite| invite.organization)
                .unwrap_or_default()
        )
    });
    let error = create_memo(move || invite.get().and_then(|invite| invite.error));
    let link = create_memo(move || {
        invite
            .get()
            .and_then(|invite| invite.link)
            .unwrap_or_default()
    });
    let has_link = create_memo(move || !link.get().is_empty());
    let role_index = create_memo(move || usize::from(administrator.get()));
    let description = create_memo(move || match role.get() {
        OrganizationRole::Administrator => "Can open every block in the organization.".to_owned(),
        OrganizationRole::Editor => {
            "Can only open blocks they create or are given access to.".to_owned()
        }
    });
    view! {
        <Frame>
            <List spacing=Space::Md>
                <Text style=TextStyle::CAPTION>{organization}</Text>
                <Text style=TextStyle::CAPTION>"Email address"</Text>
                <TextInput value={email} label="Email address" @test_id={"app.invite.email"} />
                <Text style=TextStyle::CAPTION>"Role"</Text>
                <SegmentedControl
                    label="Role"
                    selected={Binding::new(role_index, move |index: usize| administrator.set(index == 1))}
                    options={view! {
                        <ChoiceOption label={OrganizationRole::Editor.label()} />
                        <ChoiceOption label={OrganizationRole::Administrator.label()} />
                    }}
                />
                <Text style=TextStyle::CAPTION>{description}</Text>
                <ErrorText text={error} />
                <Show condition={has_link}>
                    <List spacing=Space::Sm>
                        <Text style=TextStyle::PARAGRAPH @test_id={"app.invite.help"}>
                            "Send them this link yourself, through an app you trust. Only their account can use it, and only once. It is shown only once."
                        </Text>
                        <TextInput
                            value={Binding::new(link, Callback::empty())}
                            label="Invite link"
                            @test_id={"app.invite.link"}
                        />
                    </List>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn AboutPanel() -> NodeId {
    view! {
        <Frame>
            <List spacing=Space::Md>
                <Text style=TextStyle::PARAGRAPH>"Block"</Text>
                <Text style=TextStyle::CAPTION>"Version"</Text>
                <Text style=TextStyle::CODE>{env!("CARGO_PKG_VERSION").to_owned()}</Text>
                <Text style=TextStyle::CAPTION>"Build"</Text>
                <Text style=TextStyle::CODE>{crate::build().to_owned()}</Text>
            </List>
        </Frame>
    }
}

#[component]
fn LeaveDialog(view: AppViewStore) -> NodeId {
    let leaving = view.leaving;
    let open = create_memo(move || leaving.get());
    view! {
        <Dialog
            open={open}
            kind=DialogKind::Alert
            title="Leave?"
            id="app.leave"
            actions={vec![
                DialogAction::destructive("Leave", || send(UiCommand::ConfirmLeave)),
                DialogAction::cancel("Stay"),
            ]}
            on_dismiss={|| send(UiCommand::CancelLeave)}
        >
            <Text style=TextStyle::PARAGRAPH>
                "You will no longer see this or anything inside it, unless someone shares it with you again. Whoever can edit it gives it new keys."
            </Text>
        </Dialog>
    }
}

#[component]
fn DiscardDialog(view: AppViewStore) -> NodeId {
    let discard = view.discard;
    let open = create_memo(move || discard.get().is_some());
    let title = create_memo(move || {
        discard
            .get()
            .map(|discard| discard.title)
            .unwrap_or_default()
    });
    let message = create_memo(move || {
        discard
            .get()
            .map(|discard| discard.message)
            .unwrap_or_default()
    });
    let button = create_memo(move || {
        discard
            .get()
            .map(|discard| discard.button)
            .unwrap_or_default()
    });
    view! {
        <Dialog
            open={open}
            kind=DialogKind::Alert
            title={title}
            actions={vec![
                DialogAction::destructive(button, || send(UiCommand::Discard)),
                DialogAction::cancel("Cancel"),
            ]}
            on_dismiss={|| send(UiCommand::CancelDiscard)}
        >
            <Text style=TextStyle::PARAGRAPH>{message}</Text>
        </Dialog>
    }
}

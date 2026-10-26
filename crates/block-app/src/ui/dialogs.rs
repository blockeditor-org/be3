use be_protocol::OrganizationRole;
use beui::NodeId;
use beui::Wrap;
use beui::icons::{ICON_TRASH, ICON_USER_MINUS};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::ChoiceOption;
use uuid::Uuid;

use super::onboarding::ErrorText;

use super::{AppViewStore, UiCommand, send};

#[component]
pub(super) fn Dialogs(view: AppViewStore) -> NodeId {
    view! {
        <List spacing=0.0>
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
    view! {
        <Dialog
            open={open}
            title="Invite member"
            width=380.0
            on_dismiss={|| send(UiCommand::CloseInvite)}
        >
            <InvitePanel view />
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
    let done = create_memo(
        move || match agents.get().and_then(|agents| agents.created) {
            Some(_) => "Done".to_owned(),
            None => String::new(),
        },
    );
    view! {
        <Dialog
            open={open}
            title={title}
            width=460.0
            id="app.agents"
            cancel={done}
            on_dismiss={|| send(UiCommand::CloseAgents)}
        >
            <AgentsPanel view />
        </Dialog>
    }
}

#[component]
fn ContactsDialog(view: AppViewStore) -> NodeId {
    let contacts = view.contacts;
    let open = create_memo(move || contacts.get().is_some());
    view! {
        <Dialog
            open={open}
            title="Contacts"
            width=460.0
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
            width=480.0
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
            <List spacing=8.0>
                <Text style=TextStyle::CAPTION wrap=Wrap::Words>
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
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
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
                            <IconButton
                                glyph=ICON_USER_MINUS
                                label="Remove member"
                                disabled={unremovable}
                                @test_id={format!("app.members.remove.{account}")}
                                on_click={move || send(UiCommand::RemoveMember(account))}
                            />
                        </List>
                    }
                }</ForEach>
                <Show condition={asks_removal}>
                    <List spacing=6.0>
                        <Text style=TextStyle::PARAGRAPH wrap=Wrap::Words>{removing}</Text>
                        <List direction=Direction::Horizontal spacing=8.0>
                            <Button
                                label="Remove"
                                variant=ButtonVariant::Primary
                                @test_id={"app.members.remove.confirm"}
                                on_click={|| send(UiCommand::ConfirmRemoveMember)}
                            />
                            <Button
                                label="Keep"
                                variant=ButtonVariant::Secondary
                                on_click={|| send(UiCommand::CancelRemoveMember)}
                            />
                        </List>
                    </List>
                </Show>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Button
                        label="Invite member"
                        variant=ButtonVariant::Primary
                        @test_id={"app.members.invite"}
                        on_click={|| {
                            send(UiCommand::CloseMembers);
                            send(UiCommand::InviteMember);
                        }}
                    />
                    <Show condition={busy}>
                        <Spinner />
                    </Show>
                </List>
                <ErrorText text={error} />
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
            <List spacing=8.0>
                <Text style=TextStyle::CAPTION wrap=Wrap::Words>
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
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                                <Text>{name}</Text>
                                <Text style=TextStyle::CAPTION>{detail}</Text>
                            </List>
                            <IconButton
                                glyph=ICON_TRASH
                                label="Remove contact"
                                @test_id={format!("app.contacts.remove.{account}")}
                                on_click={move || send(UiCommand::RemoveContact(account))}
                            />
                        </List>
                    }
                }</ForEach>
                <Show condition={asks_removal}>
                    <List spacing=6.0>
                        <Text style=TextStyle::PARAGRAPH wrap=Wrap::Words>{removing}</Text>
                        <List direction=Direction::Horizontal spacing=8.0>
                            <Button
                                label="Remove"
                                variant=ButtonVariant::Primary
                                @test_id={"app.contacts.remove.confirm"}
                                on_click={|| send(UiCommand::ConfirmRemoveContact)}
                            />
                            <Button
                                label="Keep"
                                variant=ButtonVariant::Secondary
                                on_click={|| send(UiCommand::CancelRemoveContact)}
                            />
                        </List>
                    </List>
                </Show>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Button
                        label="New contact link"
                        variant=ButtonVariant::Primary
                        disabled={busy}
                        @test_id={"app.contacts.new-link"}
                        on_click={|| send(UiCommand::NewContactLink)}
                    />
                    <Show condition={busy}>
                        <Spinner />
                    </Show>
                </List>
                <Show condition={has_link}>
                    <List spacing=6.0>
                        <Text
                            style=TextStyle::PARAGRAPH
                            @test_id={"app.contacts.link-help"}
                            wrap=Wrap::Words
                        >
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
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
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
                <Show condition={offered}>
                    <List spacing=6.0>
                        <Text
                            style=TextStyle::PARAGRAPH
                            @test_id={"app.contacts.offer"}
                            wrap=Wrap::Words
                        >
                            {offer}
                        </Text>
                        <List direction=Direction::Horizontal spacing=8.0>
                            <Button
                                label="Connect"
                                variant=ButtonVariant::Primary
                                @test_id={"app.contacts.accept"}
                                on_click={|| send(UiCommand::AcceptContact)}
                            />
                            <Button
                                label="Cancel"
                                variant=ButtonVariant::Secondary
                                on_click={|| send(UiCommand::DeclineContact)}
                            />
                        </List>
                    </List>
                </Show>
                <Show condition={has_connected}>
                    <Text style=TextStyle::PARAGRAPH wrap=Wrap::Words>{connected}</Text>
                </Show>
                <ErrorText text={error} />
            </List>
        </Frame>
    }
}

#[component]
fn AgentsPanel(view: AppViewStore) -> NodeId {
    let agents = view.agents;
    let shown = create_memo(move || agents.get().unwrap_or_default());
    let organization = create_memo(move || format!("Organization: {}", shown.get().organization));
    let busy = create_memo(move || shown.get().busy);
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
    let name = create_signal(String::new());
    create_effect(move || {
        if made.get() {
            untrack(|| name.set(String::new()));
        }
    });
    let cannot = create_memo(move || busy.get() || name.get().trim().is_empty());
    view! {
        <Frame>
            <List spacing=8.0>
                <Show condition={creating}>
                    <List spacing=8.0>
                        <Text style=TextStyle::CAPTION>{organization}</Text>
                        <Text style=TextStyle::CAPTION wrap=Wrap::Words>
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
                                    spacing=8.0
                                >
                                    <Text @sizing=ItemSize::Percent(100.0)>{label}</Text>
                                    <IconButton
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
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Button
                                label="Add agent"
                                variant=ButtonVariant::Primary
                                disabled={cannot}
                                @test_id={"app.agents.add"}
                                on_click={move || send(UiCommand::CreateAgent(name.get_untracked()))}
                            />
                            <Show condition={busy}>
                                <Spinner />
                            </Show>
                            <Spacer @sizing=ItemSize::Percent(100.0) />
                        </List>
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
        <List spacing=6.0>
            <Text
                style=TextStyle::PARAGRAPH
                wrap=Wrap::Words
                @test_id={"app.agents.credentials-help"}
            >
                {heading}
            </Text>
            <Checkbox
                label="Account token supplied by proxy"
                checked={proxy}
                @test_id={"app.agents.proxy"}
            />
            <Show condition={proxy}>
                <List spacing=6.0>
                    <Text style=TextStyle::PARAGRAPH wrap=Wrap::Words>
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
            width=460.0
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
            <List spacing=8.0>
                <Text style=TextStyle::CAPTION wrap=Wrap::Words>
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
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
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
                            <IconButton
                                glyph=ICON_TRASH
                                label="Discard"
                                @test_id={format!("app.unsent.discard.{block}")}
                                on_click={move || send(UiCommand::DiscardUnsent(block))}
                            />
                        </List>
                    }
                }</ForEach>
                <Show condition={asks_discard}>
                    <List spacing=6.0>
                        <Text style=TextStyle::PARAGRAPH wrap=Wrap::Words>{discarding}</Text>
                        <List direction=Direction::Horizontal spacing=8.0>
                            <Button
                                label="Discard"
                                variant=ButtonVariant::Danger
                                @test_id={"app.unsent.discard.confirm"}
                                on_click={|| send(UiCommand::ConfirmDiscardUnsent)}
                            />
                            <Button
                                label="Keep"
                                variant=ButtonVariant::Secondary
                                on_click={|| send(UiCommand::CancelDiscardUnsent)}
                            />
                        </List>
                    </List>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn AboutDialog(view: AppViewStore) -> NodeId {
    let open = view.about;
    view! {
        <Dialog open={open} title="About" width=420.0 on_dismiss={|| send(UiCommand::About(false))}>
            <AboutPanel />
        </Dialog>
    }
}

#[component]
fn InvitePanel(view: AppViewStore) -> NodeId {
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
    let busy = create_memo(move || invite.get().is_some_and(|invite| invite.busy));
    let error = create_memo(move || invite.get().and_then(|invite| invite.error));
    let sent = create_memo(move || invite.get().map(|invite| invite.sent));
    let link = create_memo(move || {
        invite
            .get()
            .and_then(|invite| invite.link)
            .unwrap_or_default()
    });
    let has_link = create_memo(move || !link.get().is_empty());
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
    let role_index = create_memo(move || usize::from(administrator.get()));
    let description = create_memo(move || match role.get() {
        OrganizationRole::Administrator => "Can open every block in the organization.".to_owned(),
        OrganizationRole::Editor => {
            "Can only open blocks they create or are given access to.".to_owned()
        }
    });
    let cannot = create_memo(move || busy.get() || email.get().trim().is_empty());
    view! {
        <Frame>
            <List spacing=8.0>
                <Text style=TextStyle::CAPTION>{organization}</Text>
                <Text style=TextStyle::CAPTION>"Email address"</Text>
                <TextInput value={email} label="Email address" @test_id={"app.invite.email"} />
                <Text style=TextStyle::CAPTION>"Role"</Text>
                <Tabs
                    selected={Binding::new(role_index, move |index: usize| administrator.set(index == 1))}
                    options={view! {
                        <ChoiceOption label={OrganizationRole::Editor.label()} />
                        <ChoiceOption label={OrganizationRole::Administrator.label()} />
                    }}
                />
                <Text style=TextStyle::CAPTION>{description}</Text>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Button
                        label="Create invite link"
                        variant=ButtonVariant::Primary
                        disabled={cannot}
                        @test_id={"app.invite.send"}
                        on_click={move || {
                            send(UiCommand::SendInvite(email.get_untracked(), role.get_untracked()))
                        }}
                    />
                    <Show condition={busy}>
                        <Spinner />
                    </Show>
                </List>
                <ErrorText text={error} />
                <Show condition={has_link}>
                    <List spacing=6.0>
                        <Text
                            style=TextStyle::PARAGRAPH
                            @test_id={"app.invite.help"}
                            wrap=Wrap::Words
                        >
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
            <List spacing=8.0>
                <Text style=TextStyle::PARAGRAPH wrap=Wrap::Words>"Block"</Text>
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
            title="Leave?"
            id="app.leave"
            confirm="Leave"
            cancel="Stay"
            destructive=true
            on_confirm={|| send(UiCommand::ConfirmLeave)}
            on_dismiss={|| send(UiCommand::CancelLeave)}
        >
            <Text style=TextStyle::PARAGRAPH wrap=Wrap::Words>
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
            title={title}
            confirm={button}
            cancel="Cancel"
            destructive=true
            on_confirm={|| send(UiCommand::Discard)}
            on_dismiss={|| send(UiCommand::CancelDiscard)}
        >
            <Text style=TextStyle::PARAGRAPH wrap=Wrap::Words>{message}</Text>
        </Dialog>
    }
}

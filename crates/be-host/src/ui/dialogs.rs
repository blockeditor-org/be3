use be_block_protocol::OrganizationRole;
use beui::NodeId;
use beui::Overflow;
use beui::icons::{ICON_COPY, ICON_TRASH, ICON_USER_MINUS};
use beui::reactive::*;
use beui::styled::*;
use beui::text::Phrase;

use be_host_accounts_ui::ErrorText;

use super::{
    AgentRow, ContactRow, HostUiStore, InvitationRow, MemberRow, UiCommand, UnsentRow, send,
};
use beui::{Align, Direction};

#[component]
pub(super) fn Dialogs(view: HostUiStore) -> NodeId {
    view! {
        <List spacing=Space::None>
            <DiscardDialog view={view} />
            <LeaveDialog view={view} />
            <InviteDialog view={view} />
            <InvitationsDialog view={view} />
            <AgentsDialog view={view} />
            <ContactsDialog view={view} />
            <MembersDialog view={view} />
            <AboutDialog view={view} />
            <UnsentDialog view={view} />
        </List>
    }
}

#[component]
fn InviteDialog(view: HostUiStore) -> NodeId {
    let invite = view.invite;
    let open = create_memo(move || invite.with(Option::is_some));
    let link = create_memo(move || invite.get().and_then(|invite| invite.link));
    let made = create_memo(move || link.with(Option::is_some));
    view! {
        <Dialog
            open={open}
            title="Invite member"
            width={Size::Custom(400.0)}
            id="host.invite"
            actions={vec![
                DialogAction::primary("Copy link", move || {
                    if let Some(link) = link.get_untracked() {
                        copy(Clip::text(link));
                    }
                })
                .disabled(move || !made.get())
                .named("copy"),
                DialogAction::cancel("Done"),
            ]}
            on_dismiss={|| send(UiCommand::CloseInvite)}
        >
            <InvitePanel view />
        </Dialog>
    }
}

/// What the Invite member dialog shows: the link it made, as text and as a QR code, and that
/// nothing was sent.
#[component]
fn InvitePanel(view: HostUiStore) -> NodeId {
    let invite = view.invite;
    let organization = create_memo(move || {
        format!(
            "Anyone who opens this link with an account on this server can join {} as an editor, once, within seven days.",
            invite
                .get()
                .map(|invite| invite.organization)
                .unwrap_or_default()
        )
    });
    let busy = create_memo(move || invite.get().is_some_and(|invite| invite.busy));
    let error = create_memo(move || invite.get().and_then(|invite| invite.error));
    view! {
        <Frame>
            <List spacing=Space::Md>
                <Text style=TextStyle::PARAGRAPH>{organization}</Text>
                <Show value={busy}>
                    <Progress width=INLINE_PROGRESS_WIDTH />
                </Show>
                <ErrorText text={error} />
                <Show
                    value={move || invite.get().and_then(|invite| invite.link)}
                >|link: ReadSignal<String>| {
                    let code = create_memo(move || link.with(|link| crate::qr::image(link)));
                    view! {
                        <List spacing=Space::Md>
                            <Text style=TextStyle::CAPTION @test_id={"host.invite.help"}>
                                "Nothing was sent: send this link yourself, through an app you trust, or show them the code to scan. You can copy it again from Invitations."
                            </Text>
                            <TextInput
                                value={Binding::new(link, Callback::empty())}
                                label="Invite link"
                                @test_id={"host.invite.link"}
                            />
                            <List align=Align::Center spacing=Space::None>
                                <Frame
                                    width={Size::Custom(192.0)}
                                    height={Size::Custom(192.0)}
                                    @test_id={"host.invite.qr"}
                                >
                                    <Picture image={code} smooth=false />
                                </Frame>
                            </List>
                        </List>
                    }
                }</Show>
            </List>
        </Frame>
    }
}

#[component]
fn InvitationsDialog(view: HostUiStore) -> NodeId {
    let invitations = view.invitations;
    let open = create_memo(move || invitations.with(Option::is_some));
    view! {
        <Dialog
            open={open}
            title="Invitations"
            width={Size::Custom(520.0)}
            size=DialogSize::Large
            id="host.invitations"
            actions={vec![
                DialogAction::primary("New invite link", || send(UiCommand::InviteMember))
                    .named("new"),
            ]}
            on_dismiss={|| send(UiCommand::CloseInvitations)}
        >
            <InvitationsPanel view />
        </Dialog>
    }
}

/// The organization's invite list: each invitation's state, whose it is and when, its link again
/// for the person's own, revoking one still open, and a fresh link for one expired.
#[component]
fn InvitationsPanel(view: HostUiStore) -> NodeId {
    let invitations = view.invitations;
    let shown = create_memo(move || invitations.get().unwrap_or_default());
    let busy = create_memo(move || shown.get().busy);
    let error = create_memo(move || shown.get().error);
    let rows = create_memo(move || shown.get().rows);
    let empty = create_memo(move || rows.with(Vec::is_empty) && !busy.get());
    view! {
        <Frame>
            <List spacing=Space::Md>
                <Text style=TextStyle::CAPTION>
                    "An invite link joins whoever opens it as an editor, once, within seven days. Revoke one you no longer want used; remove someone who joined from Members."
                </Text>
                <Show value={empty}>
                    <Text style=TextStyle::PARAGRAPH>"No invitations yet."</Text>
                </Show>
                <ForEach
                    value={rows}
                    key={|row: &InvitationRow| row.id}
                >|row: ReadSignal<InvitationRow>| {
                    let id = row.with_untracked(|row| row.id);
                    let title = create_memo(move || row.get().title);
                    let detail = create_memo(move || row.get().detail);
                    let link = create_memo(move || row.get().link);
                    view! {
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <ListItem grow=1.0>
                                <List spacing=Space::Xs2>
                                    <Text>{title}</Text>
                                    <Text style=TextStyle::CAPTION>{detail}</Text>
                                </List>
                            </ListItem>
                            <Show value={move || link.with(Option::is_some)}>
                                <Button
                                    icon_only=true
                                    glyph=ICON_COPY
                                    label="Copy link"
                                    @test_id={format!("host.invitations.copy.{id}")}
                                    on_click={move || {
                                        if let Some(link) = link.get_untracked() {
                                            copy(Clip::text(link));
                                        }
                                    }}
                                />
                            </Show>
                            <Show value={move || row.get().revocable}>
                                <Button
                                    label="Revoke"
                                    variant=ButtonVariant::Secondary
                                    disabled={busy}
                                    @test_id={format!("host.invitations.revoke.{id}")}
                                    on_click={move || send(UiCommand::RevokeInvitation(id))}
                                />
                            </Show>
                            <Show value={move || row.get().renewable}>
                                <Button
                                    label="New link"
                                    variant=ButtonVariant::Secondary
                                    @test_id={format!("host.invitations.renew.{id}")}
                                    on_click={|| send(UiCommand::InviteMember)}
                                />
                            </Show>
                        </List>
                    }
                }</ForEach>
                <Show value={busy}>
                    <Progress width=INLINE_PROGRESS_WIDTH />
                </Show>
                <ErrorText text={error} />
            </List>
        </Frame>
    }
}

#[component]
fn AgentsDialog(view: HostUiStore) -> NodeId {
    let agents = view.agents;
    let open = create_memo(move || agents.with(Option::is_some));
    let title = create_memo(
        move || match agents.get().and_then(|agents| agents.created) {
            Some(_) => "Agent credentials".to_owned(),
            None => "Agents".to_owned(),
        },
    );
    let made = create_memo(move || agents.get().and_then(|agents| agents.created).is_some());
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
            size=DialogSize::Large
            id="host.agents"
            actions={vec![
                DialogAction::primary("Add agent", move || send(UiCommand::CreateAgent(name.get_untracked())))
                    .disabled(cannot)
                    .busy(busy)
                    .shown(move || !made.get())
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
fn ContactsDialog(view: HostUiStore) -> NodeId {
    let contacts = view.contacts;
    let open = create_memo(move || contacts.with(Option::is_some));
    let busy = create_memo(move || contacts.get().is_some_and(|contacts| contacts.busy));
    view! {
        <Dialog
            open={open}
            title="Contacts"
            width={Size::Custom(460.0)}
            size=DialogSize::Large
            id="host.contacts"
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
fn MembersDialog(view: HostUiStore) -> NodeId {
    let members = view.members;
    let open = create_memo(move || members.with(Option::is_some));
    view! {
        <Dialog
            open={open}
            title="Members"
            width={Size::Custom(480.0)}
            size=DialogSize::Large
            id="host.members"
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
fn MembersPanel(view: HostUiStore) -> NodeId {
    let members = view.members;
    let shown = create_memo(move || members.get().unwrap_or_default());
    let busy = create_memo(move || shown.get().busy);
    let error = create_memo(move || shown.get().error);
    let rows = create_memo(move || shown.get().members);
    let removing = create_memo(move || shown.get().removing.unwrap_or_default());
    view! {
        <Frame>
            <List spacing=Space::Md>
                <Text style=TextStyle::CAPTION>
                    "Administrators can open every block in this organization. Removing someone takes away what was shared with them here, and gives new keys to everything they could open."
                </Text>
                <ForEach
                    value={rows}
                    key={|member: &MemberRow| member.account}
                >|row: ReadSignal<MemberRow>| {
                    let account = row.with_untracked(|row| row.account);
                    let name = create_memo(move || row.get().name);
                    let detail = create_memo(move || row.get().detail);
                    let unsigned = create_memo(move || row.get().unsigned);
                    let next_role = move || match row.get_untracked().administrator {
                        true => OrganizationRole::Editor,
                        false => OrganizationRole::Administrator,
                    };
                    view! {
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <ListItem grow=1.0>
                                <List spacing=Space::Xs2>
                                    <Text>{name}</Text>
                                    <Text style=TextStyle::CAPTION>{detail}</Text>
                                    <Show value={unsigned}>
                                        <Text style=TextStyle::CAPTION>
                                            "No invitation signed them in: the server added them. Remove them unless you know why they are here."
                                        </Text>
                                    </Show>
                                </List>
                            </ListItem>
                            <Button
                                label={move || match row.get().administrator {
                                    true => "Make editor".to_owned(),
                                    false => "Make administrator".to_owned(),
                                }}
                                variant=ButtonVariant::Secondary
                                disabled={move || {
                                    let row = row.get();
                                    row.me || row.agent || busy.get()
                                }}
                                @test_id={format!("host.members.role.{account}")}
                                on_click={move || send(UiCommand::SetMemberRole(account, next_role()))}
                            />
                            <Button
                                icon_only=true
                                glyph=ICON_USER_MINUS
                                label="Remove member"
                                disabled={move || row.get().me || busy.get()}
                                @test_id={format!("host.members.remove.{account}")}
                                on_click={move || send(UiCommand::RemoveMember(account))}
                            />
                        </List>
                    }
                }</ForEach>
                <Show value={busy}>
                    <Progress width=INLINE_PROGRESS_WIDTH />
                </Show>
                <ErrorText text={error} />
                <Dialog
                    open={move || !removing.with(String::is_empty)}
                    kind=DialogKind::Alert
                    title="Remove member?"
                    id="host.members.remove"
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
fn ContactsPanel(view: HostUiStore) -> NodeId {
    let contacts = view.contacts;
    let shown = create_memo(move || contacts.get().unwrap_or_default());
    let busy = create_memo(move || shown.get().busy);
    let error = create_memo(move || shown.get().error);
    let rows = create_memo(move || shown.get().contacts);
    let offer = create_memo(move || shown.get().offer.unwrap_or_default());
    let offered = create_memo(move || !offer.with(String::is_empty));
    let connected = create_memo(move || shown.get().connected.unwrap_or_default());
    let removing = create_memo(move || shown.get().removing.unwrap_or_default());
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
                <ForEach
                    value={rows}
                    key={|contact: &ContactRow| contact.account}
                    fallback={|| view! {
                        <Text style=TextStyle::CAPTION>"No contacts yet."</Text>
                    }}
                >|row: ReadSignal<ContactRow>| {
                    let account = row.with_untracked(|row| row.account);
                    let name = create_memo(move || row.get().name);
                    let detail = create_memo(move || {
                        let row = row.get();
                        use be_block_client::contacts::Mismatch;
                        match (row.verified, row.mismatch) {
                            (true, Some(Mismatch::Older)) => format!(
                                "{}. The server sent an older key for them than this device has; it keeps the newer one.",
                                row.email
                            ),
                            (true, Some(Mismatch::Unreadable)) => format!(
                                "{}. The server's copy of this contact can't be read here, so this device keeps the one it had. If you took a new recovery phrase on another device, this one needs it too.",
                                row.email
                            ),
                            (true, None) => row.email,
                            (false, _) if row.email.is_empty() => "Not confirmed yet".to_owned(),
                            (false, _) => format!("{}, not confirmed yet", row.email),
                        }
                    });
                    view! {
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <ListItem grow=1.0>
                                <List spacing=Space::Xs2>
                                    <Text>{name}</Text>
                                    <Text style=TextStyle::CAPTION>{detail}</Text>
                                </List>
                            </ListItem>
                            <Button
                                icon_only=true
                                glyph=ICON_TRASH
                                label="Remove contact"
                                @test_id={format!("host.contacts.remove.{account}")}
                                on_click={move || send(UiCommand::RemoveContact(account))}
                            />
                        </List>
                    }
                }</ForEach>
                <Dialog
                    open={move || !removing.with(String::is_empty)}
                    kind=DialogKind::Alert
                    title="Remove contact?"
                    id="host.contacts.remove"
                    actions={vec![
                        DialogAction::destructive("Remove", || send(UiCommand::ConfirmRemoveContact)),
                        DialogAction::cancel("Keep"),
                    ]}
                    on_dismiss={|| send(UiCommand::CancelRemoveContact)}
                >
                    <Text style=TextStyle::PARAGRAPH>{removing}</Text>
                </Dialog>
                <Show value={move || shown.get().link}>|link: ReadSignal<String>|
                    <List spacing=Space::Sm>
                        <Text style=TextStyle::PARAGRAPH @test_id={"host.contacts.link-help"}>
                            "Send this link yourself, through an app you trust, to the one person you want to connect with. It works once, and it is shown only once."
                        </Text>
                        <TextInput
                            value={Binding::new(link, Callback::empty())}
                            label="Contact link"
                            copyable=true
                            @test_id={"host.contacts.link"}
                        />
                    </List>
                </Show>
                <Text style=TextStyle::CAPTION>"Open a contact link someone sent you"</Text>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <ListItem grow=1.0>
                        <TextInput
                            value={pasted}
                            label="Contact link"
                            @test_id={"host.contacts.paste"}
                            on_submit={move |value: String| send(UiCommand::OpenContactLink(value))}
                        />
                    </ListItem>
                    <Button
                        label="Open"
                        variant=ButtonVariant::Secondary
                        disabled={cannot_open}
                        @test_id={"host.contacts.open"}
                        on_click={move || send(UiCommand::OpenContactLink(pasted.get_untracked()))}
                    />
                </List>
                <Dialog
                    open={offered}
                    kind=DialogKind::Alert
                    title="Connect?"
                    id="host.contacts.offered"
                    actions={vec![
                        DialogAction::primary("Connect", || send(UiCommand::AcceptContact)).named("accept"),
                        DialogAction::cancel("Cancel"),
                    ]}
                    on_dismiss={|| send(UiCommand::DeclineContact)}
                >
                    <Text style=TextStyle::PARAGRAPH @test_id={"host.contacts.offer"}>{offer}</Text>
                </Dialog>
                <Show value={move || !connected.with(String::is_empty)}>
                    <Text style=TextStyle::PARAGRAPH>{connected}</Text>
                </Show>
                <ErrorText text={error} />
            </List>
        </Frame>
    }
}

#[component]
fn AgentsPanel(view: HostUiStore, name: Signal<String>) -> NodeId {
    let agents = view.agents;
    let shown = create_memo(move || agents.get().unwrap_or_default());
    let organization = create_memo(move || format!("Organization: {}", shown.get().organization));
    let error = create_memo(move || shown.get().error);
    let created = create_memo(move || shown.get().created);
    let rows = create_memo(move || shown.get().agents);
    view! {
        <Frame>
            <List spacing=Space::Md>
                <Show
                    value={created}
                    fallback={move || view! {
                        <List spacing=Space::Md>
                            <Text style=TextStyle::CAPTION>{organization}</Text>
                            <Text style=TextStyle::CAPTION>
                                "An agent is an account for a program, such as a coding agent. It sees only the blocks you share with it. Share a workspace or blocks with it from this device after setup. This list shows the agents you created."
                            </Text>
                            <ForEach
                                value={rows}
                                key={|agent: &AgentRow| agent.account}
                                fallback={|| view! {
                                    <Text style=TextStyle::CAPTION>"No agents yet."</Text>
                                }}
                            >|agent: ReadSignal<AgentRow>| {
                                let account = agent.with_untracked(|agent| agent.account);
                                let label = create_memo(move || agent.with(|agent| agent.name.clone()));
                                view! {
                                    <List
                                        direction=Direction::Horizontal
                                        align=Align::Center
                                        spacing=Space::Md
                                    >
                                        <ListItem grow=1.0>
                                            <Text overflow=Overflow::Ellipsis>{label}</Text>
                                        </ListItem>
                                        <Button
                                            icon_only=true
                                            glyph=ICON_TRASH
                                            label="Remove agent"
                                            @test_id={format!("host.agents.remove.{account}")}
                                            on_click={move || send(UiCommand::RemoveAgent(account))}
                                        />
                                    </List>
                                }
                            }</ForEach>
                            <Text style=TextStyle::CAPTION>"Name"</Text>
                            <TextInput
                                value={name}
                                label="Agent name"
                                @test_id={"host.agents.name"}
                            />
                            <ErrorText text={error} />
                        </List>
                    }}
                >|created: ReadSignal<super::CreatedAgent>|
                    <AgentCredentials created={created} />
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn AgentCredentials(created: ReadSignal<super::CreatedAgent>) -> NodeId {
    let field = |read: fn(&super::CreatedAgent) -> String| create_memo(move || created.with(read));
    let heading = field(|created| {
        format!(
            "Copy these secret credentials for {}. They are shown only once. Set BE3_AGENT_CREDENTIALS to this value, then share a workspace or blocks with the agent.",
            created.name
        )
    });
    let proxy = create_signal(false);
    let credentials = create_memo(move || created.with(|created| created.bundle(proxy.get())));
    let header = field(|created| format!("Bearer {}", created.token));
    view! {
        <List spacing=Space::Sm>
            <Text style=TextStyle::PARAGRAPH @test_id={"host.agents.credentials-help"}>
                {heading}
            </Text>
            <Checkbox
                label="Account token supplied by proxy"
                checked={proxy}
                @test_id={"host.agents.proxy"}
            />
            <Show value={proxy}>
                <List spacing=Space::Sm>
                    <Text style=TextStyle::PARAGRAPH>
                        "Add a proxy rule for the server in these credentials (api.be3.pfg.pw for the hosted service). Set its Authorization header to the value below and keep it outside the agent container. If a gateway secret is required, add Be3-Gateway-Authorization: Bearer <gateway secret> as a separate rule."
                    </Text>
                    <TextInput
                        value={Binding::new(header, Callback::empty())}
                        label="Proxy Authorization header"
                        copyable=true
                        @test_id={"host.agents.header"}
                    />
                </List>
            </Show>
            <TextInput
                value={Binding::new(credentials, Callback::empty())}
                label="Agent credentials"
                copyable=true
                @test_id={"host.agents.credentials"}
            />
        </List>
    }
}

#[component]
fn UnsentDialog(view: HostUiStore) -> NodeId {
    let unsent = view.unsent;
    let open = create_memo(move || unsent.with(Option::is_some));
    view! {
        <Dialog
            open={open}
            title="Unsent edits"
            width={Size::Custom(460.0)}
            size=DialogSize::Large
            id="host.unsent"
            on_dismiss={|| send(UiCommand::Unsent(false))}
        >
            <UnsentPanel view />
        </Dialog>
    }
}

#[component]
fn UnsentPanel(view: HostUiStore) -> NodeId {
    let unsent = view.unsent;
    let shown = create_memo(move || unsent.get().unwrap_or_default());
    let rows = create_memo(move || shown.get().rows);
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
                    "These edits were made on this device but could not be sent: you can no longer edit these blocks, or the server refused them. They stay here until you restore or discard them. Restore sends them again."
                </Text>
                <ForEach
                    value={rows}
                    key={|row: &UnsentRow| row.block}
                    fallback={|| view! {
                        <Text style=TextStyle::CAPTION>"No unsent edits."</Text>
                    }}
                >|row: ReadSignal<UnsentRow>| {
                    let block = row.with_untracked(|row| row.block);
                    let name = create_memo(move || row.get().name);
                    let detail = create_memo(move || {
                        let row = row.get();
                        let why = match (row.refused, row.offerable) {
                            (Some(why), _) => format!("The server refused it: {why}"),
                            (None, true) => "You can edit it again".to_owned(),
                            (None, false) => "You cannot edit it now".to_owned(),
                        };
                        match row.kept_edits {
                            0 => why,
                            1 => format!("{why}. 1 unsaved edit"),
                            kept => format!("{why}. {kept} unsaved edits"),
                        }
                    });
                    view! {
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <ListItem grow=1.0>
                                <List spacing=Space::Xs2>
                                    <Text>{name}</Text>
                                    <Text style=TextStyle::CAPTION>{detail}</Text>
                                </List>
                            </ListItem>
                            <Button
                                label="Restore"
                                variant=ButtonVariant::Primary
                                disabled={move || !row.get().offerable}
                                @test_id={format!("host.unsent.restore.{block}")}
                                on_click={move || send(UiCommand::RestoreUnsent(block))}
                            />
                            <Button
                                icon_only=true
                                glyph=ICON_TRASH
                                label="Discard"
                                @test_id={format!("host.unsent.discard.{block}")}
                                on_click={move || send(UiCommand::DiscardUnsent(block))}
                            />
                        </List>
                    }
                }</ForEach>
                <Dialog
                    open={asks_discard}
                    kind=DialogKind::Alert
                    title="Discard edits?"
                    id="host.unsent.discard"
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
fn AboutDialog(view: HostUiStore) -> NodeId {
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
fn AboutPanel() -> NodeId {
    view! {
        <Frame>
            <List spacing=Space::Md>
                <Text style=TextStyle::PARAGRAPH>"be-host"</Text>
                <Text style=TextStyle::CAPTION>"Version"</Text>
                <Text style=TextStyle::CODE>{env!("CARGO_PKG_VERSION").to_owned()}</Text>
                <Text style=TextStyle::CAPTION>"Build"</Text>
                <Text style=TextStyle::CODE>{crate::build().to_owned()}</Text>
            </List>
        </Frame>
    }
}

#[component]
fn LeaveDialog(view: HostUiStore) -> NodeId {
    let leaving = view.leaving;
    let open = create_memo(move || leaving.with(Option::is_some));
    let title = create_memo(move || {
        leaving.with(|leaving| match leaving {
            Some(leaving) => Phrase::new()
                .text("Leave ")
                .quoted(be_lib_quoted_name::quotable_name(&leaving.name))
                .text("?"),
            None => Phrase::new(),
        })
    });
    let message = create_memo(move || {
        let within =
            leaving.with(|leaving| leaving.as_ref().and_then(|leaving| leaving.within.clone()));
        let from = match within {
            Some(within) => Phrase::new()
                .text("You will no longer see it, in ")
                .quoted(be_lib_quoted_name::quotable_name(&within))
                .text(", or anything inside it"),
            None => Phrase::new().text("You will no longer see it or anything inside it"),
        };
        from.text(
            ", unless someone shares it with you again. Whoever can edit it gives it new keys.",
        )
    });
    let busy = view.leave_busy;
    let busy = create_memo(move || busy.get());
    let refused = view.leave_refused;
    let refused = create_memo(move || refused.get());
    view! {
        <Dialog
            open={open}
            kind=DialogKind::Alert
            title={title}
            id="host.leave"
            actions={vec![
                DialogAction::destructive("Leave", move || {
                    if let Some(leaving) = leaving.get_untracked() {
                        send(UiCommand::ConfirmLeave(leaving.block));
                    }
                })
                .busy(busy),
                DialogAction::cancel("Stay"),
            ]}
            on_dismiss={|| send(UiCommand::CancelLeave)}
        >
            <Text style=TextStyle::PARAGRAPH>{message}</Text>
            <Show value={move || !refused.with(String::is_empty)}>
                <Text style=TextStyle::CAPTION @test_id="host.leave.refused">{refused}</Text>
            </Show>
        </Dialog>
    }
}

#[component]
fn DiscardDialog(view: HostUiStore) -> NodeId {
    let discard = view.discard;
    let open = create_memo(move || discard.with(Option::is_some));
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

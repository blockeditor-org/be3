use be_protocol::OrganizationRole;
use beui::NodeId;
use beui::icons::{ICON_CONTENT_COPY, ICON_DELETE};
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
            <InviteDialog view={view} />
            <AgentsDialog view={view} />
            <ContactsDialog view={view} />
            <AboutDialog view={view} />
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
    view! {
        <Dialog open={open} title="Agents" width=460.0 on_dismiss={|| send(UiCommand::CloseAgents)}>
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
    let (pasted, set_pasted) = create_signal(String::new());
    create_effect(move || {
        if offered.get() {
            untrack(|| set_pasted.set(String::new()));
        }
    });
    let cannot_open = create_memo(move || busy.get() || pasted.get().trim().is_empty());
    view! {
        <Frame>
            <List spacing=8.0>
                <Text
                    style=TextStyle::CAPTION
                    string="You can share only with your contacts. Someone becomes one when you send them a contact link yourself, through an app you trust, and they open it, or when they send you theirs."
                    wrap=true
                />
                <Show condition={none}>
                    <Text style=TextStyle::CAPTION string="No contacts yet." />
                </Show>
                <ForEach keys={keys}>
                    {move |account: Uuid| {
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
                                false => format!("{}, not confirmed yet", row.email),
                            }
                        });
                        view! {
                            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                                <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                                    <Text string={name} />
                                    <Text style=TextStyle::CAPTION string={detail} />
                                </List>
                                <IconButton
                                    glyph={ICON_DELETE.to_owned()}
                                    label="Remove contact"
                                    @test_id={format!("app.contacts.remove.{account}")}
                                    on_click={move || send(UiCommand::RemoveContact(account))}
                                />
                            </List>
                        }
                    }}
                </ForEach>
                <Show condition={asks_removal}>
                    {move || view! {
                        <List spacing=6.0>
                            <Text style=TextStyle::PARAGRAPH string={removing} wrap=true />
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
                    }}
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
                    {move || view! {
                        <List spacing=6.0>
                            <Text
                                style=TextStyle::PARAGRAPH
                                @test_id={"app.contacts.link-help"}
                                string="Send this link yourself, through an app you trust, to the one person you want to connect with. It works once, and it is shown only once."
                                wrap=true
                            />
                            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                                <TextInput
                                    @sizing=ItemSize::Percent(100.0)
                                    value={link}
                                    label="Contact link"
                                    @test_id={"app.contacts.link"}
                                />
                                <IconButton
                                    glyph={ICON_CONTENT_COPY.to_owned()}
                                    label="Copy link"
                                    @test_id={"app.contacts.copy"}
                                    on_click={move || copy_text(link.get_untracked())}
                                />
                            </List>
                        </List>
                    }}
                </Show>
                <Text style=TextStyle::CAPTION string="Open a contact link someone sent you" />
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <TextInput
                        @sizing=ItemSize::Percent(100.0)
                        value={pasted}
                        label="Contact link"
                        @test_id={"app.contacts.paste"}
                        on_change={move |value: String| set_pasted.set(value)}
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
                    {move || view! {
                        <List spacing=6.0>
                            <Text
                                style=TextStyle::PARAGRAPH
                                @test_id={"app.contacts.offer"}
                                string={offer}
                                wrap=true
                            />
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
                    }}
                </Show>
                <Show condition={has_connected}>
                    <Text style=TextStyle::PARAGRAPH string={connected} wrap=true />
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
    let keys = create_memo(move || {
        shown
            .get()
            .agents
            .iter()
            .map(|agent| agent.account)
            .collect::<Vec<Uuid>>()
    });
    let none = create_memo(move || keys.with(Vec::is_empty));
    let names = shown;
    let (name, set_name) = create_signal(String::new());
    create_effect(move || {
        if made.get() {
            untrack(|| set_name.set(String::new()));
        }
    });
    let cannot = create_memo(move || busy.get() || name.get().trim().is_empty());
    view! {
        <Frame>
            <List spacing=8.0>
                <Text style=TextStyle::CAPTION string={organization} />
                <Text
                    style=TextStyle::CAPTION
                    string="An agent is an account for a program, such as a coding agent, that reads and edits this organization through the be command line tool. It sees only the blocks you share with it, and only this device knows its key, so share them from here."
                    wrap=true
                />
                <Show condition={none}>
                    <Text style=TextStyle::CAPTION string="No agents yet." />
                </Show>
                <ForEach keys={keys}>
                    {move |account: Uuid| {
                        let label = create_memo(move || {
                            names
                                .get()
                                .agents
                                .iter()
                                .find(|agent| agent.account == account)
                                .map(|agent| agent.name.clone())
                                .unwrap_or_default()
                        });
                        view! {
                            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                                <Text @sizing=ItemSize::Percent(100.0) string={label} />
                                <IconButton
                                    glyph={ICON_DELETE.to_owned()}
                                    label="Remove agent"
                                    @test_id={format!("app.agents.remove.{account}")}
                                    on_click={move || send(UiCommand::RemoveAgent(account))}
                                />
                            </List>
                        }
                    }}
                </ForEach>
                <Text style=TextStyle::CAPTION string="Name" />
                <TextInput
                    value={name}
                    label="Agent name"
                    @test_id={"app.agents.name"}
                    on_change={move |value: String| set_name.set(value)}
                />
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
                <Show condition={made}>
                    {move || view! {
                        <AgentCredentials created={created} />
                    }}
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
            "Give {} these as environment variables. They are shown only once: the token and the recovery phrase are not kept anywhere you can read them again.",
            created.name
        )
    });
    let server = field(|created| created.server.clone());
    let organization = field(|created| created.organization.clone());
    let token = field(|created| created.token.clone());
    let phrase = field(|created| created.phrase.clone());
    view! {
        <List spacing=6.0>
            <Text style=TextStyle::PARAGRAPH string={heading} wrap=true />
            <Text style=TextStyle::CODE string="BE3_SERVER" />
            <TextInput value={server} label="BE3_SERVER" @test_id={"app.agents.server"} />
            <Text style=TextStyle::CODE string="BE3_ORGANIZATION" />
            <TextInput
                value={organization}
                label="BE3_ORGANIZATION"
                @test_id={"app.agents.organization"}
            />
            <Text style=TextStyle::CODE string="BE3_TOKEN" />
            <TextInput value={token} label="BE3_TOKEN" @test_id={"app.agents.token"} />
            <Text style=TextStyle::CODE string="BE3_RECOVERY_PHRASE" />
            <TextInput value={phrase} label="BE3_RECOVERY_PHRASE" @test_id={"app.agents.phrase"} />
        </List>
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
    let (email, set_email) = create_signal(String::new());
    let (administrator, set_administrator) = create_signal(false);
    create_effect(move || {
        sent.get();
        untrack(|| set_email.set(String::new()));
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
                <Text style=TextStyle::CAPTION string={organization} />
                <Text style=TextStyle::CAPTION string="Email address" />
                <TextInput
                    value={email}
                    label="Email address"
                    @test_id={"app.invite.email"}
                    on_change={move |value: String| set_email.set(value)}
                />
                <Text style=TextStyle::CAPTION string="Role" />
                <Tabs
                    selected={role_index}
                    on_change={move |index: usize| set_administrator.set(index == 1)}
                    options={view! {
                        <ChoiceOption label={OrganizationRole::Editor.label()} />
                        <ChoiceOption label={OrganizationRole::Administrator.label()} />
                    }}
                />
                <Text style=TextStyle::CAPTION string={description} />
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
                    {move || view! {
                        <List spacing=6.0>
                            <Text
                                style=TextStyle::PARAGRAPH
                                @test_id={"app.invite.help"}
                                string="Send them this link yourself, through an app you trust. Only their account can use it, and only once. It is shown only once."
                                wrap=true
                            />
                            <TextInput
                                value={link}
                                label="Invite link"
                                @test_id={"app.invite.link"}
                            />
                        </List>
                    }}
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
                <Text style=TextStyle::PARAGRAPH string="Block" wrap=true />
                <Text style=TextStyle::CAPTION string="Version" />
                <Text style=TextStyle::CODE string={env!("CARGO_PKG_VERSION").to_owned()} />
                <Text style=TextStyle::CAPTION string="Build" />
                <Text style=TextStyle::CODE string={crate::build().to_owned()} />
            </List>
        </Frame>
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
        <Dialog open={open} title={title} on_dismiss={|| send(UiCommand::CancelDiscard)}>
            <List spacing=12.0>
                <Text style=TextStyle::PARAGRAPH string={message} wrap=true />
                <List direction=Direction::Horizontal spacing=8.0>
                    <Button
                        label={button}
                        variant=ButtonVariant::Primary
                        on_click={|| send(UiCommand::Discard)}
                    />
                    <Button
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        on_click={|| send(UiCommand::CancelDiscard)}
                    />
                </List>
            </List>
        </Dialog>
    }
}

use be_protocol::WorkspaceRole;
use beui::NodeId;
use beui::icons::ICON_DELETE;
use beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, Spacer, component, create_effect,
    create_memo, create_signal, untrack, view,
};
use beui::styled::{Button, ButtonVariant, Dialog, IconButton, Spinner, Tabs, TextInput};
use beui::unstyled::ChoiceOption;
use uuid::Uuid;

use super::onboarding::ErrorText;

use super::{AppViewStore, UiCommand, send};
use beui::styled::{Text, TextStyle};

#[component]
pub(super) fn Dialogs(view: AppViewStore) -> NodeId {
    view! {
        <List spacing=0.0>
            <DiscardDialog view={view} />
            <InviteDialog view={view} />
            <AgentsDialog view={view} />
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
fn AgentsPanel(view: AppViewStore) -> NodeId {
    let agents = view.agents;
    let shown = create_memo(move || agents.get().unwrap_or_default());
    let workspace = create_memo(move || format!("Workspace: {}", shown.get().workspace));
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
                <Text style=TextStyle::CAPTION string={workspace} />
                <Text
                    style=TextStyle::CAPTION
                    string="An agent is an account for a program, such as a coding agent, that reads and edits this workspace through the be command line tool. It sees only the blocks you share with it."
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
    let workspace = field(|created| created.workspace.clone());
    let token = field(|created| created.token.clone());
    let phrase = field(|created| created.phrase.clone());
    view! {
        <List spacing=6.0>
            <Text style=TextStyle::PARAGRAPH string={heading} wrap=true />
            <Text style=TextStyle::CODE string="BE3_SERVER" />
            <TextInput value={server} label="BE3_SERVER" @test_id={"app.agents.server"} />
            <Text style=TextStyle::CODE string="BE3_WORKSPACE" />
            <TextInput value={workspace} label="BE3_WORKSPACE" @test_id={"app.agents.workspace"} />
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
    let workspace = create_memo(move || {
        format!(
            "Workspace: {}",
            invite
                .get()
                .map(|invite| invite.workspace)
                .unwrap_or_default()
        )
    });
    let busy = create_memo(move || invite.get().is_some_and(|invite| invite.busy));
    let error = create_memo(move || invite.get().and_then(|invite| invite.error));
    let sent = create_memo(move || invite.get().map(|invite| invite.sent));
    let (email, set_email) = create_signal(String::new());
    let (administrator, set_administrator) = create_signal(false);
    create_effect(move || {
        sent.get();
        untrack(|| set_email.set(String::new()));
    });
    let role = create_memo(move || match administrator.get() {
        true => WorkspaceRole::Administrator,
        false => WorkspaceRole::Editor,
    });
    let role_index = create_memo(move || usize::from(administrator.get()));
    let description = create_memo(move || match role.get() {
        WorkspaceRole::Administrator => "Can open every block in the workspace.".to_owned(),
        WorkspaceRole::Editor => {
            "Can only open blocks they create or are given access to.".to_owned()
        }
    });
    let cannot = create_memo(move || busy.get() || email.get().trim().is_empty());
    view! {
        <Frame>
            <List spacing=8.0>
                <Text style=TextStyle::CAPTION string={workspace} />
                <Text style=TextStyle::CAPTION string="Email address" />
                <TextInput
                    value={email}
                    label="Email address"
                    on_change={move |value: String| set_email.set(value)}
                />
                <Text style=TextStyle::CAPTION string="Role" />
                <Tabs
                    selected={role_index}
                    on_change={move |index: usize| set_administrator.set(index == 1)}
                    options={view! {
                        <ChoiceOption label={WorkspaceRole::Editor.label()} />
                        <ChoiceOption label={WorkspaceRole::Administrator.label()} />
                    }}
                />
                <Text style=TextStyle::CAPTION string={description} />
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Button
                        label="Send invitation"
                        variant=ButtonVariant::Primary
                        disabled={cannot}
                        on_click={move || {
                            send(UiCommand::SendInvite(email.get_untracked(), role.get_untracked()))
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

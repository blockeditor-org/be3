use be_protocol::WorkspaceRole;
use beui::NodeId;
use beui::icons::ICON_DELETE;
use beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, Spacer, clone, component,
    create_effect, create_memo, create_signal, untrack, view,
};
use beui::styled::{
    Body, Button, ButtonVariant, Caption, Code, Dialog, IconButton, Paragraph, Spinner, Tabs,
    TextInput,
};
use beui::unstyled::ChoiceOption;
use uuid::Uuid;

use super::onboarding::ErrorText;

use super::{AppViewStore, UiCommand, send};

#[component]
pub(super) fn Dialogs(view: AppViewStore) -> NodeId {
    view! {
        <List spacing=0.0>
            <DiscardDialog view={view.clone()} />
            <InviteDialog view={view.clone()} />
            <AgentsDialog view={view.clone()} />
            <AboutDialog view={view.clone()} />
        </List>
    }
}

#[component]
fn InviteDialog(view: AppViewStore) -> NodeId {
    let invite = view.invite.clone();
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
    let agents = view.agents.clone();
    let open = create_memo(move || agents.get().is_some());
    view! {
        <Dialog open={open} title="Agents" width=460.0 on_dismiss={|| send(UiCommand::CloseAgents)}>
            <AgentsPanel view />
        </Dialog>
    }
}

#[component]
fn AgentsPanel(view: AppViewStore) -> NodeId {
    let agents = view.agents.clone();
    let shown = create_memo(clone!(agents -> move || agents.get().unwrap_or_default()));
    let workspace =
        create_memo(clone!(shown -> move || format!("Workspace: {}", shown.get().workspace)));
    let busy = create_memo(clone!(shown -> move || shown.get().busy));
    let error = create_memo(clone!(shown -> move || shown.get().error));
    let created = create_memo(clone!(shown -> move || shown.get().created));
    let made = create_memo(clone!(created -> move || created.get().is_some()));
    let keys = create_memo(clone!(shown -> move || {
        shown.get().agents.iter().map(|agent| agent.account).collect::<Vec<Uuid>>()
    }));
    let none = create_memo(clone!(keys -> move || keys.with(Vec::is_empty)));
    let names = shown.clone();
    let (name, set_name) = create_signal(String::new());
    create_effect(clone!(set_name made -> move || {
        if made.get() {
            untrack(|| set_name.set(String::new()));
        }
    }));
    let cannot =
        create_memo(clone!(name busy -> move || busy.get() || name.get().trim().is_empty()));
    view! {
        <Frame>
            <List spacing=8.0>
                <Caption content={workspace} />
                <Caption
                    content="An agent is an account for a program, such as a coding agent, that reads and edits this workspace through the be command line tool. It sees only the blocks you share with it."
                    wrap=true
                />
                <Show condition={none}>
                    <Caption content="No agents yet." />
                </Show>
                <ForEach keys={keys}>
                    {move |account: Uuid| {
                        let label = create_memo(clone!(names -> move || {
                            names
                                .get()
                                .agents
                                .iter()
                                .find(|agent| agent.account == account)
                                .map(|agent| agent.name.clone())
                                .unwrap_or_default()
                        }));
                        view! {
                            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                                <Body @sizing=ItemSize::Percent(100.0) content={label} />
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
                <Caption content="Name" />
                <TextInput
                    value={name.clone()}
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
                    {move || clone!(created -> view! {
                        <AgentCredentials created={created} />
                    })}
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn AgentCredentials(created: Memo<Option<super::CreatedAgent>>) -> NodeId {
    let field = |read: fn(&super::CreatedAgent) -> String| {
        create_memo(clone!(created -> move || created.get().as_ref().map(read).unwrap_or_default()))
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
            <Paragraph content={heading} />
            <Code content="BE3_SERVER" />
            <TextInput value={server} label="BE3_SERVER" @test_id={"app.agents.server"} />
            <Code content="BE3_WORKSPACE" />
            <TextInput value={workspace} label="BE3_WORKSPACE" @test_id={"app.agents.workspace"} />
            <Code content="BE3_TOKEN" />
            <TextInput value={token} label="BE3_TOKEN" @test_id={"app.agents.token"} />
            <Code content="BE3_RECOVERY_PHRASE" />
            <TextInput value={phrase} label="BE3_RECOVERY_PHRASE" @test_id={"app.agents.phrase"} />
        </List>
    }
}

#[component]
fn AboutDialog(view: AppViewStore) -> NodeId {
    let open = view.about.clone();
    view! {
        <Dialog open={open} title="About" width=420.0 on_dismiss={|| send(UiCommand::About(false))}>
            <AboutPanel />
        </Dialog>
    }
}

#[component]
fn InvitePanel(view: AppViewStore) -> NodeId {
    let invite = view.invite.clone();
    let workspace = create_memo(clone!(invite -> move || {
        format!(
            "Workspace: {}",
            invite.get().map(|invite| invite.workspace).unwrap_or_default()
        )
    }));
    let busy =
        create_memo(clone!(invite -> move || invite.get().is_some_and(|invite| invite.busy)));
    let error = create_memo(clone!(invite -> move || invite.get().and_then(|invite| invite.error)));
    let sent = create_memo(move || invite.get().map(|invite| invite.sent));
    let (email, set_email) = create_signal(String::new());
    let (administrator, set_administrator) = create_signal(false);
    create_effect(clone!(set_email -> move || {
        sent.get();
        untrack(|| set_email.set(String::new()));
    }));
    let role = create_memo(clone!(administrator -> move || match administrator.get() {
        true => WorkspaceRole::Administrator,
        false => WorkspaceRole::Editor,
    }));
    let role_index = create_memo(clone!(administrator -> move || usize::from(administrator.get())));
    let description = create_memo(clone!(role -> move || match role.get() {
        WorkspaceRole::Administrator => "Can open every block in the workspace.".to_owned(),
        WorkspaceRole::Editor => "Can only open blocks they create or are given access to.".to_owned(),
    }));
    let cannot =
        create_memo(clone!(email busy -> move || busy.get() || email.get().trim().is_empty()));
    view! {
        <Frame>
            <List spacing=8.0>
                <Caption content={workspace} />
                <Caption content="Email address" />
                <TextInput
                    value={email.clone()}
                    label="Email address"
                    on_change={move |value: String| set_email.set(value)}
                />
                <Caption content="Role" />
                <Tabs
                    selected={role_index}
                    on_change={move |index: usize| set_administrator.set(index == 1)}
                    options={view! {
                        <ChoiceOption label={WorkspaceRole::Editor.label()} />
                        <ChoiceOption label={WorkspaceRole::Administrator.label()} />
                    }}
                />
                <Caption content={description} />
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
                <Paragraph content="Block" />
                <Caption content="Version" />
                <Code content={env!("CARGO_PKG_VERSION").to_owned()} />
                <Caption content="Commit" />
                <Code content={crate::COMMIT.to_owned()} />
            </List>
        </Frame>
    }
}

#[component]
fn DiscardDialog(view: AppViewStore) -> NodeId {
    let discard = view.discard.clone();
    let open = create_memo(clone!(discard -> move || discard.get().is_some()));
    let title = create_memo(
        clone!(discard -> move || discard.get().map(|discard| discard.title).unwrap_or_default()),
    );
    let message = create_memo(
        clone!(discard -> move || discard.get().map(|discard| discard.message).unwrap_or_default()),
    );
    let button = create_memo(move || {
        discard
            .get()
            .map(|discard| discard.button)
            .unwrap_or_default()
    });
    view! {
        <Dialog open={open} title={title} on_dismiss={|| send(UiCommand::CancelDiscard)}>
            <List spacing=12.0>
                <Paragraph content={message} />
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

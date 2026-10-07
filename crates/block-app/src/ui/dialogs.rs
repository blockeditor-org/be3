use be_protocol::WorkspaceRole;
use beui::NodeId;
use beui::reactive::{
    Align, Direction, Frame, List, Show, clone, component, create_effect, create_memo,
    create_signal, untrack, view,
};
use beui::styled::{
    Button, ButtonVariant, Caption, Code, Dialog, Paragraph, Spinner, Tabs, TextInput,
};
use beui::unstyled::ChoiceOption;

use super::onboarding::ErrorText;

use super::{AppViewStore, UiCommand, send};

#[component]
pub(super) fn Dialogs(view: AppViewStore) -> NodeId {
    view! {
        <List spacing=0.0>
            <DiscardDialog view={view.clone()} />
            <InviteDialog view={view.clone()} />
            <AboutDialog view={view.clone()} />
            <RunProgramDialog view />
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
fn AboutDialog(view: AppViewStore) -> NodeId {
    let open = view.about.clone();
    view! {
        <Dialog open={open} title="About" width=420.0 on_dismiss={|| send(UiCommand::About(false))}>
            <AboutPanel />
        </Dialog>
    }
}

#[component]
fn RunProgramDialog(view: AppViewStore) -> NodeId {
    let open = view.run_program.clone();
    let (command, set_command) = create_signal(String::new());
    create_effect(clone!(open set_command -> move || {
        if open.get() {
            untrack(|| set_command.set(String::new()));
        }
    }));
    let blank = create_memo(clone!(command -> move || command.get().trim().is_empty()));
    let run = clone!(command -> move || {
        let line = command.get_untracked();
        if !line.trim().is_empty() {
            send(UiCommand::Launch(line));
        }
    });
    let submit = run.clone();
    view! {
        <Dialog
            open={open}
            title="Run a program"
            width=420.0
            on_dismiss={|| send(UiCommand::RunProgram(false))}
        >
            <Frame>
                <List spacing=8.0>
                    <Caption content="Its windows open in the workspace." />
                    <TextInput
                        @test_id={"app.run-program.command"}
                        value={command}
                        label="Command"
                        placeholder="A command, like foot or gtk4-demo"
                        on_change={move |line: String| set_command.set(line)}
                        on_submit={move |_: String| submit()}
                    />
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Button
                            @test_id={"app.run-program.run"}
                            label="Run"
                            variant=ButtonVariant::Primary
                            disabled={blank}
                            on_click={run}
                        />
                    </List>
                </List>
            </Frame>
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

use beui::reactive::{
    Align, Direction, Frame, ItemSize, List, Show, Spacer, clone, component, create_effect,
    create_memo, create_signal, untrack, view,
};
use beui::styled::{
    Button, ButtonVariant, Caption, Code, Dialog, Paragraph, Spinner, Tabs, TextInput, Window,
};
use beui::unstyled::ChoiceOption;
use beui::{NodeId, pos2, vec2};
use block::WorkspaceRole;
use block_client::properties::MAX_NAME_BYTES;

use super::onboarding::ErrorText;
use super::{AppViewStore, UiCommand, send};
use crate::surfaces::{self, HostSurface, SurfaceId};

#[component]
pub(super) fn Dialogs(view: AppViewStore) -> NodeId {
    view! {
        <List spacing=0.0>
            <InviteWindow view={view.clone()} />
            <AboutWindow view={view.clone()} />
            <DiscardDialog view={view.clone()} />
            <RenameDialog view={view.clone()} />
            <ArtifactSettingsDialog view={view.clone()} />
            <UnlinkDialog view />
        </List>
    }
}

#[component]
fn InviteWindow(view: AppViewStore) -> NodeId {
    let invite = view.invite.clone();
    let open = create_memo(clone!(invite -> move || invite.get().is_some()));
    let workspace = create_memo(clone!(invite -> move || {
        format!(
            "Workspace: {}",
            invite.get().map(|invite| invite.workspace).unwrap_or_default()
        )
    }));
    let busy = create_memo(clone!(invite -> move || invite.get().is_some_and(|invite| invite.busy)));
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
    let cannot = create_memo(clone!(email busy -> move || busy.get() || email.get().trim().is_empty()));
    view! {
        <Window
            open={open}
            title="Invite member"
            position={pos2(120.0, 96.0)}
            size={vec2(360.0, 330.0)}
            on_close={|| send(UiCommand::CloseInvite)}
        >
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
        </Window>
    }
}

#[component]
fn AboutWindow(view: AppViewStore) -> NodeId {
    let open = view.about.clone();
    view! {
        <Window
            open={open}
            title="About"
            position={pos2(160.0, 120.0)}
            size={vec2(420.0, 190.0)}
            on_close={|| send(UiCommand::About(false))}
        >
            <List spacing=8.0>
                <Paragraph content="Block" />
                <Caption content="Version" />
                <Code content={env!("CARGO_PKG_VERSION").to_owned()} />
                <Caption content="Commit" />
                <Code content={crate::COMMIT.to_owned()} />
            </List>
        </Window>
    }
}

#[component]
fn DiscardDialog(view: AppViewStore) -> NodeId {
    let discard = view.discard.clone();
    let open = create_memo(clone!(discard -> move || discard.get().is_some()));
    let title = create_memo(clone!(discard -> move || discard.get().map(|discard| discard.title).unwrap_or_default()));
    let message = create_memo(clone!(discard -> move || discard.get().map(|discard| discard.message).unwrap_or_default()));
    let button = create_memo(move || discard.get().map(|discard| discard.button).unwrap_or_default());
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

#[component]
fn RenameDialog(view: AppViewStore) -> NodeId {
    let rename = view.rename.clone();
    let open = create_memo(clone!(rename -> move || rename.get().is_some()));
    let (name, set_name) = create_signal(String::new());
    create_effect(clone!(set_name -> move || {
        let initial = rename.get().map(|rename| rename.name).unwrap_or_default();
        untrack(|| set_name.set(initial));
    }));
    let invalid = create_memo(clone!(name -> move || name.get().len() > MAX_NAME_BYTES));
    let error = create_memo(clone!(invalid -> move || {
        invalid
            .get()
            .then(|| format!("Name must be at most {MAX_NAME_BYTES} UTF-8 bytes."))
    }));
    let submit = clone!(name invalid -> move || {
        if !invalid.get_untracked() {
            send(UiCommand::SubmitRename(name.get_untracked()));
        }
    });
    let submit_on_enter = submit.clone();
    view! {
        <Dialog open={open} title="Rename block" on_dismiss={|| send(UiCommand::CancelRename)}>
            <List spacing=10.0>
                <TextInput
                    value={name}
                    label="Name"
                    focused=true
                    on_change={move |value: String| set_name.set(value)}
                    on_submit={move |_value: String| submit_on_enter()}
                />
                <ErrorText text={error} />
                <List direction=Direction::Horizontal spacing=8.0>
                    <Button
                        label="Rename"
                        variant=ButtonVariant::Primary
                        disabled={invalid}
                        on_click={submit}
                    />
                    <Button
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        on_click={|| send(UiCommand::CancelRename)}
                    />
                </List>
            </List>
        </Dialog>
    }
}

#[component]
fn ArtifactSettingsDialog(view: AppViewStore) -> NodeId {
    let settings = view.artifact_settings.clone();
    let open = create_memo(clone!(settings -> move || settings.get().is_some()));
    let unchanged = create_memo(clone!(settings -> move || !settings.get().is_some_and(|settings| settings.changed)));
    let summary = create_memo(move || settings.get().and_then(|settings| settings.summary));
    let has_summary = create_memo(clone!(summary -> move || summary.get().is_some()));
    let summary_text = create_memo(move || summary.get().unwrap_or_default());
    let height = surfaces::handle(SurfaceId::ArtifactSettings).height();
    view! {
        <Dialog
            open={open}
            title="Dynamic artifact settings"
            width=360.0
            on_dismiss={|| send(UiCommand::CancelArtifactSettings)}
        >
            <List spacing=12.0>
                <Frame height={height}>
                    <HostSurface id=SurfaceId::ArtifactSettings />
                </Frame>
                <Show condition={has_summary}>
                    <Caption content={summary_text} />
                </Show>
                <List direction=Direction::Horizontal spacing=8.0>
                    <Button
                        label="Apply"
                        variant=ButtonVariant::Primary
                        disabled={unchanged}
                        on_click={|| send(UiCommand::ApplyArtifactSettings)}
                    />
                    <Button
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        on_click={|| send(UiCommand::CancelArtifactSettings)}
                    />
                </List>
            </List>
        </Dialog>
    }
}

#[component]
fn UnlinkDialog(view: AppViewStore) -> NodeId {
    let open = view.unlink.clone();
    view! {
        <Dialog
            open={open}
            title="Unlink from the source block?"
            width=360.0
            on_dismiss={|| send(UiCommand::CancelUnlink)}
        >
            <List spacing=12.0>
                <Paragraph content="This block keeps what was generated for it, but stops being rebuilt from its source and becomes editable." />
                <Paragraph content="The link and its settings cannot be restored." />
                <List direction=Direction::Horizontal spacing=8.0>
                    <Button
                        label="Unlink"
                        variant=ButtonVariant::Primary
                        on_click={|| send(UiCommand::Unlink)}
                    />
                    <Button
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        on_click={|| send(UiCommand::CancelUnlink)}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
            </List>
        </Dialog>
    }
}

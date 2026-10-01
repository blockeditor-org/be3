use beui::NodeId;
use beui::reactive::{
    Align, Direction, ItemSize, List, Show, Spacer, clone, component, create_effect, create_memo,
    create_signal, untrack, view,
};
use beui::styled::{
    Button, ButtonVariant, Caption, Card, Dialog, Heading, Paragraph, Spinner, TextInput, Title,
};

use super::onboarding::{Column, ErrorText};
use super::{AppViewStore, UiCommand, send};

#[component]
pub(super) fn RecoveryScreen(view: AppViewStore) -> NodeId {
    let recovery = view.recovery.clone();
    let phrase = create_memo(clone!(recovery -> move || {
        recovery
            .get()
            .words
            .chunks(4)
            .enumerate()
            .map(|(row, words)| {
                words
                    .iter()
                    .enumerate()
                    .map(|(column, word)| format!("{}. {word}", row * 4 + column + 1))
                    .collect::<Vec<_>>()
                    .join("    ")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }));
    let busy = create_memo(clone!(recovery -> move || recovery.get().busy));
    let replacing = create_memo(clone!(recovery -> move || recovery.get().replacing));
    let first_time = create_memo(clone!(replacing -> move || !replacing.get()));
    let title = create_memo(clone!(replacing -> move || match replacing.get() {
        true => "New recovery phrase".to_owned(),
        false => "Save your recovery phrase".to_owned(),
    }));
    let error = create_memo(clone!(recovery -> move || recovery.get().error));
    let label = |index: usize| {
        create_memo(clone!(recovery -> move || {
            recovery
                .get()
                .checked
                .get(index)
                .map(|position| format!("Word {}", position + 1))
                .unwrap_or_default()
        }))
    };
    let (first_label, second_label, third_label) = (label(0), label(1), label(2));
    let (first, set_first) = create_signal(String::new());
    let (second, set_second) = create_signal(String::new());
    let (third, set_third) = create_signal(String::new());
    let cannot = create_memo(clone!(busy first second third -> move || {
        busy.get()
            || first.get().trim().is_empty()
            || second.get().trim().is_empty()
            || third.get().trim().is_empty()
    }));
    let confirm = clone!(cannot first second third -> move || {
        if !cannot.get_untracked() {
            send(UiCommand::ConfirmRecovery(vec![
                first.get_untracked(),
                second.get_untracked(),
                third.get_untracked(),
            ]));
        }
    });
    let submit = confirm.clone();
    view! {
        <Column>
            <Title content={title} />
            <Show condition={replacing.clone()}>
                <Paragraph
                    content="Once you save this phrase, your old one stops working. Every workspace key sealed to the old phrase is sealed to this one instead."
                />
            </Show>
            <Paragraph
                content="Your notes are encrypted with a key the server never sees. These twelve words are the only way to open them on a new device when no other device of yours is at hand. Write them down and keep them somewhere safe: nobody can recover them for you."
            />
            <Card>
                <Paragraph content={phrase} />
            </Card>
            <Heading content="Check that you saved them" />
            <Caption content={first_label.clone()} />
            <TextInput
                value={first}
                label={first_label}
                on_change={move |value: String| set_first.set(value)}
            />
            <Caption content={second_label.clone()} />
            <TextInput
                value={second}
                label={second_label}
                on_change={move |value: String| set_second.set(value)}
            />
            <Caption content={third_label.clone()} />
            <TextInput
                value={third}
                label={third_label}
                on_change={move |value: String| set_third.set(value)}
                on_submit={move |_value: String| submit()}
            />
            <ErrorText text={error} />
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Show condition={busy}>
                    <Spinner />
                </Show>
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Show condition={first_time}>
                    <Button
                        label="Switch account"
                        variant=ButtonVariant::Secondary
                        on_click={|| send(UiCommand::SwitchAccount)}
                    />
                </Show>
                <Show condition={replacing}>
                    <Button
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        on_click={|| send(UiCommand::CancelRecovery)}
                    />
                </Show>
                <Button
                    label="I saved them"
                    variant=ButtonVariant::Primary
                    disabled={cannot}
                    on_click={confirm}
                />
            </List>
        </Column>
    }
}

#[component]
pub(super) fn UnlockScreen(view: AppViewStore) -> NodeId {
    let unlock = view.unlock.clone();
    let title = create_memo(clone!(unlock -> move || format!("Unlock {}", unlock.get().workspace)));
    let sealed = create_memo(clone!(unlock -> move || unlock.get().sealed));
    let unsealed = create_memo(clone!(sealed -> move || !sealed.get()));
    let code = create_memo(clone!(unlock -> move || unlock.get().code.unwrap_or_default()));
    let waiting = create_memo(clone!(unlock -> move || unlock.get().code.is_some()));
    let idle = create_memo(clone!(waiting -> move || !waiting.get()));
    let error = create_memo(clone!(unlock -> move || unlock.get().error));
    let (phrase, set_phrase) = create_signal(String::new());
    let cannot = create_memo(clone!(phrase -> move || phrase.get().trim().is_empty()));
    let open = clone!(phrase cannot -> move || {
        if !cannot.get_untracked() {
            send(UiCommand::UnlockWithPhrase(phrase.get_untracked()));
        }
    });
    let submit = open.clone();
    view! {
        <Column>
            <Title content={title} />
            <Paragraph content="This device does not have the key to this workspace yet." />
            <Show condition={sealed}>
                {move || clone!(cannot open phrase set_phrase submit -> view! {
                    <List spacing=8.0>
                        <Heading content="With your recovery phrase" />
                        <TextInput
                            value={phrase}
                            label="Recovery phrase"
                            placeholder="twelve words"
                            on_change={move |value: String| set_phrase.set(value)}
                            on_submit={move |_value: String| submit()}
                        />
                        <List direction=Direction::Horizontal spacing=8.0>
                            <Spacer @sizing=ItemSize::Percent(100.0) />
                            <Button
                                label="Unlock"
                                variant=ButtonVariant::Primary
                                disabled={cannot}
                                on_click={open}
                            />
                        </List>
                    </List>
                })}
            </Show>
            <Show condition={unsealed}>
                <Paragraph
                    content="No key for this workspace is sealed to your recovery phrase yet. It will be once a member opens the workspace on a device that has the key."
                />
            </Show>
            <Heading content="With another device" />
            <Show condition={idle}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Paragraph
                        @sizing=ItemSize::Percent(100.0)
                        content="Open any workspace on a device that already has this one, and it will ask for a code."
                    />
                    <Button
                        label="Show a code"
                        variant=ButtonVariant::Secondary
                        on_click={|| send(UiCommand::StartPairing)}
                    />
                </List>
            </Show>
            <Show condition={waiting}>
                {move || clone!(code -> view! {
                    <List spacing=8.0>
                        <Paragraph content="Type this code on the other device:" />
                        <Title content={code} />
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Spinner />
                            <Caption content="Waiting for the other device…" />
                            <Spacer @sizing=ItemSize::Percent(100.0) />
                            <Button
                                label="Cancel"
                                variant=ButtonVariant::Secondary
                                on_click={|| send(UiCommand::CancelPairing)}
                            />
                        </List>
                    </List>
                })}
            </Show>
            <ErrorText text={error} />
            <List direction=Direction::Horizontal spacing=8.0>
                <Button
                    label="Back to workspaces"
                    variant=ButtonVariant::Ghost
                    on_click={|| send(UiCommand::SwitchWorkspace)}
                />
            </List>
        </Column>
    }
}

#[component]
pub(super) fn PairingDialog(view: AppViewStore) -> NodeId {
    let pairing = view.pairing.clone();
    let open = create_memo(clone!(pairing -> move || !pairing.get().is_empty()));
    let request = create_memo(clone!(pairing -> move || pairing.get().first().cloned()));
    let message = create_memo(clone!(request -> move || {
        let (device, workspace) = request
            .get()
            .map(|request| (request.device, request.workspace))
            .unwrap_or_default();
        format!(
            "A {device} device wants to open {workspace}. If it is yours, type the code it shows."
        )
    }));
    let (code, set_code) = create_signal(String::new());
    create_effect(clone!(request set_code -> move || {
        request.get();
        untrack(|| set_code.set(String::new()));
    }));
    let cannot = create_memo(clone!(code -> move || code.get().trim().is_empty()));
    let approve = clone!(request code cannot -> move || {
        if let Some(request) = request.get_untracked()
            && !cannot.get_untracked()
        {
            send(UiCommand::ApprovePairing(request.from, code.get_untracked()));
        }
    });
    let submit = approve.clone();
    let dismiss = clone!(request -> move || {
        if let Some(request) = request.get_untracked() {
            send(UiCommand::DismissPairing(request.from));
        }
    });
    let ignore = dismiss.clone();
    view! {
        <Dialog open={open} title="Add a device" width=340.0 on_dismiss={dismiss}>
            <List spacing=10.0>
                <Paragraph content={message} />
                <TextInput
                    value={code}
                    label="Code"
                    placeholder="XXXX-XXXX"
                    on_change={move |value: String| set_code.set(value)}
                    on_submit={move |_value: String| submit()}
                />
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Button label="Ignore" variant=ButtonVariant::Secondary on_click={ignore} />
                    <Button
                        label="Approve"
                        variant=ButtonVariant::Primary
                        disabled={cannot}
                        on_click={approve}
                    />
                </List>
            </List>
        </Dialog>
    }
}

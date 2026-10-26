use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

use super::onboarding::{Column, ErrorText};
use super::{AppViewStore, UiCommand, send};

#[component]
pub(super) fn RecoveryScreen(view: AppViewStore) -> NodeId {
    let recovery = view.recovery;
    let phrase = create_memo(move || {
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
    });
    let busy = create_memo(move || recovery.get().busy);
    let replacing = create_memo(move || recovery.get().replacing);
    let first_time = create_memo(move || !replacing.get());
    let title = create_memo(move || match replacing.get() {
        true => "New recovery phrase".to_owned(),
        false => "Save your recovery phrase".to_owned(),
    });
    let error = create_memo(move || recovery.get().error);
    let label = |index: usize| {
        create_memo(move || {
            recovery
                .get()
                .checked
                .get(index)
                .map(|position| format!("Word {}", position + 1))
                .unwrap_or_default()
        })
    };
    let (first_label, second_label, third_label) = (label(0), label(1), label(2));
    let first = create_signal(String::new());
    let second = create_signal(String::new());
    let third = create_signal(String::new());
    let cannot = create_memo(move || {
        busy.get()
            || first.get().trim().is_empty()
            || second.get().trim().is_empty()
            || third.get().trim().is_empty()
    });
    let confirm = move || {
        if !cannot.get_untracked() {
            send(UiCommand::ConfirmRecovery(vec![
                first.get_untracked(),
                second.get_untracked(),
                third.get_untracked(),
            ]));
        }
    };
    view! {
        <Column>
            <Text style=TextStyle::TITLE>{title}</Text>
            <Show condition={replacing}>
                <Text style=TextStyle::PARAGRAPH>
                    "Once you save this phrase, your old one stops working. Every key sealed to the old phrase is sealed to this one instead, and your other devices will ask to be unlocked again."
                </Text>
            </Show>
            <Text style=TextStyle::PARAGRAPH>
                "Your notes are encrypted with a key the server never sees. These twelve words are the only way to open them on a new device when no other device of yours is at hand. Write them down and keep them somewhere safe: nobody can recover them for you."
            </Text>
            <Card>
                <Text style=TextStyle::PARAGRAPH>{phrase}</Text>
            </Card>
            <Text style=TextStyle::HEADING>"Check that you saved them"</Text>
            <Text style=TextStyle::CAPTION>{first_label}</Text>
            <TextInput value={first} label={first_label} />
            <Text style=TextStyle::CAPTION>{second_label}</Text>
            <TextInput value={second} label={second_label} />
            <Text style=TextStyle::CAPTION>{third_label}</Text>
            <TextInput
                value={third}
                label={third_label}
                on_submit={move |_value: String| confirm()}
            />
            <ErrorText text={error} />
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Show condition={busy}>
                    <Progress width=INLINE_PROGRESS_WIDTH />
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
    let unlock = view.unlock;
    let title = create_memo(move || format!("Unlock {}", unlock.get().account));
    let code = create_memo(move || unlock.get().code.unwrap_or_default());
    let waiting = create_memo(move || unlock.get().code.is_some());
    let idle = create_memo(move || !waiting.get());
    let error = create_memo(move || unlock.get().error);
    let phrase = create_signal(String::new());
    let cannot = create_memo(move || phrase.get().trim().is_empty());
    let open = move || {
        if !cannot.get_untracked() {
            send(UiCommand::UnlockWithPhrase(phrase.get_untracked()));
        }
    };
    view! {
        <Column>
            <Text style=TextStyle::TITLE>{title}</Text>
            <Text style=TextStyle::PARAGRAPH>
                "This device does not have this account's key yet, so it cannot open anything the account keeps."
            </Text>
            <List spacing=8.0>
                <Text style=TextStyle::HEADING>"With your recovery phrase"</Text>
                <TextInput
                    value={phrase}
                    label="Recovery phrase"
                    placeholder="twelve words"
                    on_submit={move |_value: String| open()}
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
            <Text style=TextStyle::HEADING>"With another device"</Text>
            <Show condition={idle}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Text style=TextStyle::PARAGRAPH @sizing=ItemSize::Percent(100.0)>
                        "Open any organization of this account on a device that is unlocked, and it will ask for a code."
                    </Text>
                    <Button
                        label="Show a code"
                        variant=ButtonVariant::Secondary
                        on_click={|| send(UiCommand::StartPairing)}
                    />
                </List>
            </Show>
            <Show condition={waiting}>
                <List spacing=8.0>
                    <Text style=TextStyle::PARAGRAPH>"Type this code on the other device:"</Text>
                    <Text style=TextStyle::TITLE>{code}</Text>
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Progress width=INLINE_PROGRESS_WIDTH />
                        <Text style=TextStyle::CAPTION>"Waiting for the other device…"</Text>
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                        <Button
                            label="Cancel"
                            variant=ButtonVariant::Secondary
                            on_click={|| send(UiCommand::CancelPairing)}
                        />
                    </List>
                </List>
            </Show>
            <ErrorText text={error} />
            <List direction=Direction::Horizontal spacing=8.0>
                <Button
                    label="Switch account"
                    variant=ButtonVariant::Ghost
                    on_click={|| send(UiCommand::SwitchAccount)}
                />
            </List>
        </Column>
    }
}

#[component]
pub(super) fn PairingDialog(view: AppViewStore) -> NodeId {
    let pairing = view.pairing;
    let open = create_memo(move || !pairing.get().is_empty());
    let request = create_memo(move || pairing.get().first().cloned());
    let message = create_memo(move || {
        let device = request
            .get()
            .map(|request| request.device)
            .unwrap_or_default();
        format!(
            "A {device} device wants this account's key. If it is yours, type the code it shows."
        )
    });
    let code = create_signal(String::new());
    create_effect(move || {
        request.get();
        untrack(|| code.set(String::new()));
    });
    let cannot = create_memo(move || code.get().trim().is_empty());
    let approve = move || {
        if let Some(request) = request.get_untracked()
            && !cannot.get_untracked()
        {
            send(UiCommand::ApprovePairing(
                request.from,
                code.get_untracked(),
            ));
        }
    };
    let dismiss = move || {
        if let Some(request) = request.get_untracked() {
            send(UiCommand::DismissPairing(request.from));
        }
    };
    view! {
        <Dialog
            open={open}
            title="Add a device"
            width=340.0
            confirm="Approve"
            cancel="Ignore"
            confirm_disabled={cannot}
            on_confirm={approve}
            on_dismiss={dismiss}
        >
            <Text style=TextStyle::PARAGRAPH>{message}</Text>
            <TextInput value={code} label="Code" placeholder="XXXX-XXXX" />
        </Dialog>
    }
}

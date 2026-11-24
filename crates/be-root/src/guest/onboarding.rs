//! What the block stack waits for before an organization's blocks open: the accounts and adding or removing one, the organizations, the sessions, the recovery phrase, unlocking, and a failure.

use super::*;
use be_wasm_root_api::{Invitation, OrganizationsArea, SessionKind, SessionsArea};
use beui::icons::{
    ICON_BOXES, ICON_CHECK, ICON_CHEVRON_DOWN, ICON_CLOUD, ICON_COPY, ICON_MONITOR, ICON_PLUS, ICON_ELLIPSIS, ICON_REFRESH_CW, ICON_USER_PLUS, ICON_USERS_ROUND,
};

/// What the block stack waits for the person to do: the accounts, the organizations or the
/// sessions, each splitting its own memo off the one view.
#[component]
pub(super) fn Onboarding(
    core: Signal<CoreView>,
    want: Callback<Asking>,
    secret: Signal<Option<Held>>,
    pending: Signal<bool>,
    ask: Callback<SecretCommand>,
) -> NodeId {
    let waits = create_memo(move || core.with(|view| view.waits_for.clone()));
    let recovery = create_memo(move || waits.get() == Some(CoreWait::Recovery));
    let unlock = create_memo(move || waits.get() == Some(CoreWait::Unlock));
    let failure = create_memo(move || waits.get() == Some(CoreWait::Failure));
    let accounts = create_memo(move || waits.get() == Some(CoreWait::Account));
    let organizations = create_memo(move || waits.get() == Some(CoreWait::Organization));
    let sessions = create_memo(move || waits.get() == Some(CoreWait::Session));
    view! {
        <Frame width=Length::FILL height=Length::FILL fill=ColorRole::Background>
            <List spacing=Space::None>
                <Show value={accounts}>
                    <ListItem grow=1.0>
                        <AccountsCard core={core} want={want} />
                    </ListItem>
                </Show>
                <Show value={organizations}>
                    <ListItem grow=1.0>
                        <OrganizationsCard core={core} want={want} />
                    </ListItem>
                </Show>
                <Show value={sessions}>
                    <ListItem grow=1.0>
                        <SessionsCard core={core} />
                    </ListItem>
                </Show>
                <Show value={recovery}>
                    <ListItem grow=1.0>
                        <RecoveryCard core={core} secret={secret} pending={pending} ask={ask} />
                    </ListItem>
                </Show>
                <Show value={unlock}>
                    <ListItem grow=1.0>
                        <UnlockCard core={core} secret={secret} pending={pending} ask={ask} />
                    </ListItem>
                </Show>
                <Show value={failure}>
                    <ListItem grow=1.0>
                        <FailureCard core={core} />
                    </ListItem>
                </Show>
            </List>
        </Frame>
    }
}

/// The recovery phrase, while the person writes it down, as be-host's own asks it: its words,
/// asked for as it shows and held only while it does, numbered in rows of four; a field for
/// each word at the checked positions, typed back as one secret, the words in order and
/// separated by spaces, and cleared; I saved them; and Cancel while replacing an old phrase, or
/// Switch account.
#[component]
pub(super) fn RecoveryCard(
    core: Signal<CoreView>,
    secret: Signal<Option<Held>>,
    pending: Signal<bool>,
    ask: Callback<SecretCommand>,
) -> NodeId {
    let area = create_memo(move || core.with(|view| view.recovery.clone()));
    let busy = create_memo(move || pending.get() || area.with(|area| area.busy));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let replacing = create_memo(move || area.with(|area| area.replacing));
    let title = create_memo(move || match replacing.get() {
        true => "New recovery phrase".to_owned(),
        false => RECOVERY_TITLE.to_owned(),
    });
    ask.call(SecretCommand::RecoveryPhrase);
    // The one copy of the words it shows beside the phrase itself.
    let words = create_memo(move || {
        secret.with(|secret| {
            let words: Vec<&str> = secret
                .as_ref()
                .and_then(Held::phrase)
                .map(|phrase| phrase.split_whitespace().collect())
                .unwrap_or_default();
            words
                .chunks(4)
                .enumerate()
                .map(|(row, chunk)| {
                    chunk
                        .iter()
                        .enumerate()
                        .map(|(column, word)| format!("{}. {word}", row * 4 + column + 1))
                        .collect::<Vec<_>>()
                        .join("    ")
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
    });
    let label = move |index: usize| {
        create_memo(move || {
            area.with(|area| {
                area.checked
                    .get(index)
                    .map(|position| format!("Word {position}"))
                    .unwrap_or_default()
            })
        })
    };
    let (first_label, second_label, third_label) = (label(0), label(1), label(2));
    let (first, second, third) = (
        create_signal(String::new()),
        create_signal(String::new()),
        create_signal(String::new()),
    );
    let cannot = create_memo(move || {
        busy.get()
            || [first, second, third]
                .iter()
                .any(|word| word.with(|word| word.trim().is_empty()))
    });
    let confirm = move || {
        if cannot.get_untracked() {
            return;
        }
        let words = [first, second, third]
            .iter()
            .map(|word| word.get_untracked().trim().to_owned())
            .collect::<Vec<_>>()
            .join(" ");
        for word in [first, second, third] {
            word.set(String::new());
        }
        ask_core(CoreCommand::ConfirmRecovery(Password::from(words)));
    };
    view! {
        <be_root_accounts_ui::Column>
            <Text style=TextStyle::TITLE>{title}</Text>
            <Show value={replacing}>
                <Text style=TextStyle::PARAGRAPH>
                    "Once you save this phrase, your old one stops working. Every key sealed to the old phrase is sealed to this one instead, and your other devices will ask to be unlocked again."
                </Text>
            </Show>
            <Text style=TextStyle::PARAGRAPH>
                "Your notes are encrypted with a key the server never sees. These twelve words are the only way to open them on a new device when no other device of yours is at hand. Write them down and keep them somewhere safe: nobody can recover them for you."
            </Text>
            <Card>
                <Text style=TextStyle::PARAGRAPH>{words}</Text>
            </Card>
            <Text style=TextStyle::HEADING>"Check that you saved them"</Text>
            <Text style=TextStyle::CAPTION>{first_label}</Text>
            <TextInput value={first} label={first_label} kind={InputKind::Password} />
            <Text style=TextStyle::CAPTION>{second_label}</Text>
            <TextInput value={second} label={second_label} kind={InputKind::Password} />
            <Text style=TextStyle::CAPTION>{third_label}</Text>
            <TextInput
                value={third}
                label={third_label}
                kind={InputKind::Password}
                on_submit={move |_: String| confirm()}
            />
            <be_root_accounts_ui::ErrorText text={error} />
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Show value={busy}>
                    <Progress width=INLINE_PROGRESS_WIDTH />
                </Show>
                <Spacer />
                <Show
                    value={replacing}
                    fallback={|| view! {
                        <Button
                            label="Switch account"
                            variant=ButtonVariant::Secondary
                            on_click={|| ask_core(CoreCommand::SwitchAccount)}
                        />
                    }}
                >
                    <Button
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        on_click={|| ask_core(CoreCommand::CancelRecovery)}
                    />
                </Show>
                <Button
                    label="I saved them"
                    variant=ButtonVariant::Primary
                    disabled={cannot}
                    on_click={confirm}
                />
            </List>
        </be_root_accounts_ui::Column>
    }
}

/// Unlocking the account's keys on this device, as be-host's own asks it: with its recovery
/// phrase, handed over as a secret and cleared; or with another device, whose code, asked for,
/// shows until it is withdrawn; or Switch account.
#[component]
pub(super) fn UnlockCard(
    core: Signal<CoreView>,
    secret: Signal<Option<Held>>,
    pending: Signal<bool>,
    ask: Callback<SecretCommand>,
) -> NodeId {
    let area = create_memo(move || core.with(|view| view.unlock.clone()));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let title = create_memo(move || area.with(|area| format!("Unlock {}", area.account)));
    let code = create_memo(move || {
        secret.with(|secret| secret.as_ref().and_then(Held::code).map(str::to_owned))
    });
    let phrase = create_signal(String::new());
    let cannot = create_memo(move || phrase.with(|phrase| phrase.trim().is_empty()));
    let unlock = move || {
        if cannot.get_untracked() {
            return;
        }
        let typed = phrase.get_untracked();
        phrase.set(String::new());
        ask_core(CoreCommand::UnlockWithPhrase(Password::from(typed)));
    };
    view! {
        <be_root_accounts_ui::Column>
            <Text style=TextStyle::TITLE>{title}</Text>
            <Text style=TextStyle::PARAGRAPH>
                "This device does not have this account's key yet, so it cannot open anything the account keeps."
            </Text>
            <List spacing=Space::Md>
                <Text style=TextStyle::HEADING>"With your recovery phrase"</Text>
                <TextInput
                    value={phrase}
                    label="Recovery phrase"
                    placeholder="twelve words"
                    on_submit={move |_: String| unlock()}
                />
                <List direction=Direction::Horizontal spacing=Space::Md>
                    <Spacer />
                    <Button
                        label="Unlock"
                        variant=ButtonVariant::Primary
                        disabled={cannot}
                        on_click={unlock}
                    />
                </List>
            </List>
            <Text style=TextStyle::HEADING>"With another device"</Text>
            <Show
                value={code}
                fallback={move || view! {
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <ListItem grow=1.0>
                            <Text style=TextStyle::PARAGRAPH>
                                "Open any organization of this account on a device that is unlocked, and it will ask for a code."
                            </Text>
                        </ListItem>
                        <Button
                            label="Show a code"
                            variant=ButtonVariant::Secondary
                            disabled={pending}
                            on_click={move || ask.call(SecretCommand::PairingCode)}
                        />
                    </List>
                }}
            >|code: ReadSignal<String>|
                <List spacing=Space::Md>
                    <Text style=TextStyle::PARAGRAPH>"Type this code on the other device:"</Text>
                    <Text size=TextSize::Xl>{code}</Text>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <Progress width=INLINE_PROGRESS_WIDTH />
                        <Text style=TextStyle::CAPTION>"Waiting for the other device…"</Text>
                        <Spacer />
                        <Button
                            label="Cancel"
                            variant=ButtonVariant::Secondary
                            on_click={move || {
                                secret.set(None);
                                ask_core(CoreCommand::CancelPairing);
                            }}
                        />
                    </List>
                </List>
            </Show>
            <be_root_accounts_ui::ErrorText text={error} />
            <List direction=Direction::Horizontal spacing=Space::Md>
                <Button
                    label="Switch account"
                    variant=ButtonVariant::Ghost
                    on_click={|| ask_core(CoreCommand::SwitchAccount)}
                />
            </List>
        </be_root_accounts_ui::Column>
    }
}

/// What the block stack's failure leaves the person to do: start it again, delete this
/// device's copy of the blocks (asked first, saying what is lost), or quit.
#[component]
pub(super) fn FailureCard(core: Signal<CoreView>) -> NodeId {
    let failure = create_memo(move || core.with(|view| view.failure.clone()));
    let message = create_memo(move || {
        failure.with(|it| it.as_ref().map(|it| it.message.clone()).unwrap_or_default())
    });
    let asking = create_signal(false);
    let confirmation = create_memo(move || {
        let accounts = "This removes every saved account and its key on this device. You will need to sign in again, and unlock each account with its recovery phrase or another device.";
        match failure.with(|it| it.as_ref().map_or(0, |it| it.unsaved)) {
            0 => accounts.to_owned(),
            1 => format!(
                "{accounts} One block has changes that have not reached the server yet, and they will be lost."
            ),
            count => format!(
                "{accounts} {count} blocks have changes that have not reached the server yet, and they will be lost."
            ),
        }
    });
    let copy = move || {
        let text = message.get_untracked();
        be_wasm_ops::guest::spawn(async move {
            let item = be_lib_ui_types::ClipItem::text(text);
            // A host that cannot hear it has stopped, which the root hears from its frames.
            let _ = host::copy(vec![be_wasm_ops::encode(&item)]).await;
        });
    };
    view! {
        <be_root_accounts_ui::Column>
            <Text style=TextStyle::TITLE align=TextAlign::Center>{FAILURE_TITLE}</Text>
            <Frame height={Size::Custom(240.0)}>
                <Scroll>
                    <Text style=TextStyle::PARAGRAPH>{message}</Text>
                </Scroll>
            </Frame>
            <List direction=Direction::Horizontal spacing=Space::Md wrap=true>
                <Button
                    label="Restart"
                    variant=ButtonVariant::Primary
                    on_click={|| ask_core(CoreCommand::Restart)}
                />
                <Button label="Copy" variant=ButtonVariant::Secondary on_click={copy} />
                <Button
                    label="Delete client database..."
                    variant=ButtonVariant::Secondary
                    on_click={move || asking.set(true)}
                />
                <Button
                    label="Exit"
                    variant=ButtonVariant::Ghost
                    on_click={|| ask_core(CoreCommand::Exit)}
                />
            </List>
            <Dialog
                open={asking}
                kind=DialogKind::Alert
                title="Delete client database?"
                actions={vec![
                    DialogAction::destructive("Delete", move || {
                        asking.set(false);
                        ask_core(CoreCommand::DeleteClientDatabase);
                    }),
                    DialogAction::cancel("Cancel"),
                ]}
                on_dismiss={move || asking.set(false)}
            >
                <Text style=TextStyle::PARAGRAPH>{confirmation}</Text>
            </Dialog>
        </be_root_accounts_ui::Column>
    }
}

/// The account's organizations: each to open, in its last session or choosing one; an
/// invitation opened, to join or put aside; creating one; opening an invitation's link, which
/// the field hands over as a secret and clears; and switching account.
#[component]
pub(super) fn OrganizationsCard(core: Signal<CoreView>, want: Callback<Asking>) -> NodeId {
    let area = create_memo(move || core.with(|view| view.organizations.clone()));
    let listed = create_memo(move || area.with(|area| area.listed.clone()));
    let rows = create_memo(move || match listed.get() {
        OrganizationsListed::Loaded(rows) => rows,
        _ => Vec::new(),
    });
    let busy = create_memo(move || area.with(|area| area.busy));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let account = create_memo(move || {
        core.with(|view| {
            view.accounts
                .iter()
                .find(|account| account.current)
                .map(account_row)
                .unwrap_or_default()
        })
    });
    let name = create_memo(move || account.with(|account| account.name.clone()));
    let leave = move || {
        let (key, name, local) =
            account.with_untracked(|it| (it.key.clone(), it.name.clone(), it.local));
        match local {
            // One kept on this device alone asks first, as removing it does from the accounts.
            true => want.call(Asking::ForgetAccount { key, name }),
            false => ask_core(CoreCommand::ForgetAccount(key)),
        }
    };
    view! {
        <be_root_accounts_ui::Column>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <ListItem grow=1.0>
                    <Text style=TextStyle::TITLE>{ORGANIZATIONS_TITLE}</Text>
                </ListItem>
                <MenuButton
                    label="Account options"
                    glyph=ICON_ELLIPSIS
                    icon_only=true
                    arrow=false
                    items={view! {
                        <MenuItem
                            label={move || be_root_accounts_ui::leave_label(account.get().local).to_owned()}
                            on_click={leave}
                        />
                    }}
                />
                <Button
                    icon_only=true
                    glyph=ICON_REFRESH_CW
                    label="Reload organizations"
                    disabled={busy}
                    on_click={|| ask_core(CoreCommand::ReloadOrganizations)}
                />
            </List>
            <Card>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <ListItem grow=1.0>
                        <List spacing=Space::Xs>
                            <Text size=TextSize::Lg>{name}</Text>
                            <be_root_accounts_ui::AccountDetails account />
                        </List>
                    </ListItem>
                    <Button
                        label="Switch account"
                        glyph=ICON_USERS_ROUND
                        variant=ButtonVariant::Secondary
                        on_click={|| ask_core(CoreCommand::SwitchAccount)}
                    />
                </List>
            </Card>
            <Text style=TextStyle::HEADING>"Open an organization"</Text>
            <ForEach
                value={rows}
                key={|row: &OrganizationRow| row.id}
                fallback={move || view! {
                    <Card>
                        <OrganizationsPlaceholder listed />
                    </Card>
                }}
            >|row: ReadSignal<OrganizationRow>| {
                let id = row.with_untracked(|row| row.id);
                let label = create_memo(move || row.with(|row| row.name.clone()));
                view! {
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                        <ListItem grow=1.0>
                            <Button
                                label={label}
                                glyph=ICON_BOXES
                                variant=ButtonVariant::Secondary
                                on_click={move || ask_core(CoreCommand::OpenOrganization(id))}
                            />
                        </ListItem>
                        <Button
                            icon_only=true
                            glyph=ICON_CHEVRON_DOWN
                            label="Choose a session or profile"
                            on_click={move || ask_core(CoreCommand::ChooseSessionIn(id))}
                        />
                    </List>
                }
            }</ForEach>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                <Text align=TextAlign::Center>
                    <Icon glyph=ICON_USER_PLUS />
                </Text>
                <Text style=TextStyle::HEADING>"Join an organization"</Text>
            </List>
            <JoinOrganization area={area} busy={busy} />
            <Text style=TextStyle::HEADING>"Create an organization"</Text>
            <CreateOrganization busy={busy} />
            <be_root_accounts_ui::ErrorText text={error} />
        </be_root_accounts_ui::Column>
    }
}

/// What shows in place of the organizations while there are none: that there are none yet,
/// that they could not be listed, with Retry, or that they are loading.
#[component]
fn OrganizationsPlaceholder(listed: Memo<OrganizationsListed>) -> NodeId {
    let loading = create_memo(move || listed.get() == OrganizationsListed::Loading);
    let failed = create_memo(move || listed.get() == OrganizationsListed::Failed);
    let none = create_memo(move || matches!(listed.get(), OrganizationsListed::Loaded(_)));
    view! {
        <List spacing=Space::None>
            <Show value={none}>
                <Text style=TextStyle::CAPTION>
                    "You do not have any organizations yet. Create one below."
                </Text>
            </Show>
            <Show value={failed}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Text style=TextStyle::CAPTION>"Could not load organizations."</Text>
                    <Button
                        label="Retry"
                        variant=ButtonVariant::Secondary
                        on_click={|| ask_core(CoreCommand::ReloadOrganizations)}
                    />
                </List>
            </Show>
            <Show value={loading}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Progress width=INLINE_PROGRESS_WIDTH />
                    <Text style=TextStyle::CAPTION>"Loading organizations…"</Text>
                </List>
            </Show>
        </List>
    }
}

/// Joining an organization by an invite link: the link, pasted and handed over as a secret,
/// shown as typed and cleared as it goes; then the invitation it opened, to join or put aside,
/// and why it cannot be joined, with Use another account.
#[component]
fn JoinOrganization(area: Memo<OrganizationsArea>, busy: Memo<bool>) -> NodeId {
    let joining = create_memo(move || area.with(|area| area.joining.clone()));
    let link = create_signal(String::new());
    let cannot = create_memo(move || busy.get() || link.with(|link| link.trim().is_empty()));
    let open = move || {
        if cannot.get_untracked() {
            return;
        }
        // The link is a bearer secret: shown as typed, so the person sees what they pasted,
        // it leaves the field as it is handed over.
        let typed = link.get_untracked();
        link.set(String::new());
        ask_core(CoreCommand::OpenInvitation(Password::from(typed)));
    };
    view! {
        <List spacing=Space::Md>
            <Show
                value={joining}
                fallback={move || view! {
                    <List spacing=Space::Md>
                        <Text style=TextStyle::CAPTION>
                            "Paste the invite link someone sent you."
                        </Text>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <ListItem grow=1.0>
                                <TextInput
                                    value={link}
                                    label="Invite link"
                                    placeholder="https://…/#invite/…"
                                    on_submit={move |_: String| open()}
                                />
                            </ListItem>
                            <Button
                                label="Open"
                                variant=ButtonVariant::Secondary
                                disabled={cannot}
                                on_click={open}
                            />
                        </List>
                    </List>
                }}
            >|invitation: ReadSignal<Invitation>| {
                let title = create_memo(move || {
                    invitation.with(|it| format!("Join {}?", it.organization))
                });
                let invited = create_memo(move || {
                    invitation.with(|it| format!("Invited by {}. You join as an editor.", it.inviter))
                });
                let problem = create_memo(move || invitation.with(|it| it.problem.clone()));
                let can_join = create_memo(move || invitation.with(|it| it.can_join));
                let refused = create_memo(move || problem.with(Option::is_some));
                let problem_text = create_memo(move || problem.get().unwrap_or_default());
                view! {
                    <Card>
                        <List spacing=Space::Md>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Md
                            >
                                <ListItem grow=1.0>
                                    <List spacing=Space::Xs2>
                                        <Text size=TextSize::Lg>{title}</Text>
                                        <Text style=TextStyle::CAPTION>{invited}</Text>
                                    </List>
                                </ListItem>
                                <Button
                                    label="Cancel"
                                    variant=ButtonVariant::Secondary
                                    disabled={busy}
                                    on_click={|| ask_core(CoreCommand::CancelJoin)}
                                />
                                <Show value={can_join}>
                                    <Button
                                        label="Join"
                                        variant=ButtonVariant::Primary
                                        disabled={busy}
                                        on_click={|| ask_core(CoreCommand::JoinInvited)}
                                    />
                                </Show>
                            </List>
                            <Show value={refused}>
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Md
                                >
                                    <ListItem grow=1.0>
                                        <Text style=TextStyle::CAPTION>{problem_text}</Text>
                                    </ListItem>
                                    <Button
                                        label="Use another account"
                                        variant=ButtonVariant::Secondary
                                        on_click={|| ask_core(CoreCommand::SwitchAccount)}
                                    />
                                </List>
                            </Show>
                        </List>
                    </Card>
                }
            }</Show>
        </List>
    }
}

/// Creating an organization by its name, which clears once asked.
#[component]
fn CreateOrganization(busy: Memo<bool>) -> NodeId {
    let name = create_signal(String::new());
    let cannot = create_memo(move || busy.get() || name.with(|name| name.trim().is_empty()));
    let create = move || {
        if cannot.get_untracked() {
            return;
        }
        let named = name.get_untracked().trim().to_owned();
        name.set(String::new());
        ask_core(CoreCommand::CreateOrganization(named));
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
            <ListItem grow=1.0>
                <TextInput
                    value={name}
                    placeholder="Organization name"
                    label="Organization name"
                    on_submit={move |_: String| create()}
                />
            </ListItem>
            <Button
                label="Create"
                variant=ButtonVariant::Primary
                disabled={cannot}
                on_click={create}
            />
        </List>
    }
}

/// The organization's sessions: those of the type this device opens, or of every type, each to
/// open; a new one; this device's profiles, each to use, and a new one; and Close, back in the
/// session open.
#[component]
pub(super) fn SessionsCard(core: Signal<CoreView>) -> NodeId {
    let area = create_memo(move || core.with(|view| view.sessions.clone()));
    let loading = create_memo(move || !area.with(|area| area.loaded));
    let title = create_memo(move || core.with(|view| view.organization.clone().unwrap_or_default()));
    // Which types it lists is the root's own choice, not the host's.
    let every = create_signal(false);
    let kind_name = move |area: &SessionsArea, kind: [u8; 16]| {
        area.session_kinds
            .iter()
            .find(|it| it.id == kind)
            .map(|it| it.name.clone())
    };
    let shown = create_memo(move || {
        let every = every.get();
        area.with(|area| {
            area.sessions
                .iter()
                .filter(|session| every || session.session_type == area.session_type)
                .map(|session| {
                    let kind = every.then(|| kind_name(area, session.session_type)).flatten();
                    let label = match (kind, session.current) {
                        (Some(kind), true) => format!("{} ({kind}, last used)", session.name),
                        (Some(kind), false) => format!("{} ({kind})", session.name),
                        (None, true) => format!("{} (last used)", session.name),
                        (None, false) => session.name.clone(),
                    };
                    (session.id, label)
                })
                .collect::<Vec<_>>()
        })
    });
    let profiles = create_memo(move || area.with(|area| area.profiles.clone()));
    let kinds = create_memo(move || area.with(|area| area.session_kinds.clone()));
    let new_label = create_memo(move || {
        area.with(|area| {
            format!(
                "New {} session",
                kind_name(area, area.session_type).unwrap_or_default()
            )
        })
    });
    view! {
        <be_root_accounts_ui::Column>
            <Text style=TextStyle::TITLE>{title}</Text>
            <Show value={loading}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Progress width=INLINE_PROGRESS_WIDTH />
                    <Text style=TextStyle::CAPTION>"Loading sessions and profiles…"</Text>
                </List>
            </Show>
            <Text style=TextStyle::HEADING>"Settings profile"</Text>
            <Text style=TextStyle::CAPTION>
                "The settings this device uses. Each device starts with a profile of its own."
            </Text>
            <ForEach
                value={profiles}
                key={|profile: &ProfileRow| profile.id}
            >|profile: ReadSignal<ProfileRow>| {
                let id = profile.with_untracked(|profile| profile.id);
                let label = create_memo(move || {
                    profile.with(|profile| match profile.current {
                        true => format!("{} (this device)", profile.name),
                        false => profile.name.clone(),
                    })
                });
                let glyph = create_memo(move || profile.with(|profile| profile.current.then_some(ICON_CHECK)));
                view! {
                    <Button
                        label={label}
                        glyph={glyph}
                        variant=ButtonVariant::Secondary
                        on_click={move || ask_core(CoreCommand::UseProfile(id))}
                    />
                }
            }</ForEach>
            <Button
                label="New settings profile"
                glyph=ICON_PLUS
                variant=ButtonVariant::Ghost
                disabled={loading}
                on_click={|| ask_core(CoreCommand::NewProfile)}
            />
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <ListItem grow=1.0>
                    <Text style=TextStyle::HEADING>"Session"</Text>
                </ListItem>
                <Text style=TextStyle::CAPTION>"Show every type"</Text>
                <Switch on={every} label="Show sessions of every type" />
            </List>
            <Text style=TextStyle::CAPTION>
                "The blocks you had open. Start a new session to begin again with the same settings."
            </Text>
            <ForEach
                value={shown}
                key={|session: &([u8; 16], String)| session.0}
            >|session: ReadSignal<([u8; 16], String)>| {
                let id = session.with_untracked(|session| session.0);
                let label = create_memo(move || session.with(|session| session.1.clone()));
                view! {
                    <Button
                        label={label}
                        variant=ButtonVariant::Secondary
                        on_click={move || ask_core(CoreCommand::OpenSession(id))}
                    />
                }
            }</ForEach>
            <SplitButton
                label={new_label}
                glyph=ICON_PLUS
                variant=ButtonVariant::Secondary
                menu_label="New session of another type"
                disabled={loading}
                items={view! {
                    <ForEach value={kinds} key={|kind: &SessionKind| kind.id}>|kind: ReadSignal<SessionKind>| {
                        let id = kind.with_untracked(|kind| kind.id);
                        let label = create_memo(move || kind.with(|kind| format!("New {} session", kind.name)));
                        view! {
                            <MenuItem
                                label={label}
                                on_click={move || ask_core(CoreCommand::NewSession(id))}
                            />
                        }
                    }</ForEach>
                }}
                on_click={move || ask_core(CoreCommand::NewSession(area.with_untracked(|area| area.session_type)))}
            />
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button
                    label="Keep the current session"
                    variant=ButtonVariant::Ghost
                    on_click={|| ask_core(CoreCommand::CloseSessions)}
                />
                <Button
                    label="Back to organizations"
                    variant=ButtonVariant::Ghost
                    on_click={|| ask_core(CoreCommand::SwitchOrganization)}
                />
            </List>
        </be_root_accounts_ui::Column>
    }
}

/// The accounts this device keeps, as be-root-accounts-ui shows them: opening one, choosing its
/// organization, removing it (asking first for one kept on this device alone, whose blocks go
/// with it), or adding one, which `want` opens a dialog for.
#[component]
pub(super) fn AccountsCard(core: Signal<CoreView>, want: Callback<Asking>) -> NodeId {
    // A memo per area of the core's view: these change only when the accounts do.
    let shown = create_memo(move || core.with(|view| view.accounts.clone()));
    let accounts = create_memo(move || {
        shown.with(|accounts| accounts.iter().map(account_row).collect::<Vec<_>>())
    });
    let error = create_memo(move || core.with(|view| view.account_error.clone()));
    let forget = move |key: String| {
        let local = shown.with_untracked(|accounts| {
            accounts
                .iter()
                .find(|account| account.key == key)
                .filter(|account| account.server.is_none())
                .map(|account| account.name.clone())
        });
        match local {
            Some(name) => want.call(Asking::ForgetAccount { key, name }),
            None => ask_core(CoreCommand::ForgetAccount(key)),
        }
    };
    view! {
        <Frame width=Length::FILL height=Length::FILL fill=ColorRole::Background>
            <be_root_accounts_ui::AccountsScreen
                accounts
                error
                on_add={move || want.call(Asking::AddAccount { resumed: false })}
                on_open={|key: String| ask_core(CoreCommand::OpenAccount(key))}
                on_choose_organization={|key: String| ask_core(CoreCommand::ChooseOrganization(key))}
                on_log_out={forget}
            />
        </Frame>
    }
}

/// An account as be-root-accounts-ui shows it.
pub(super) fn account_row(account: &AccountRow) -> be_root_accounts_ui::AccountRow {
    be_root_accounts_ui::AccountRow {
        key: account.key.clone(),
        name: account.name.clone(),
        email: account.email.clone(),
        server: account
            .server
            .clone()
            .unwrap_or_else(|| "On this device".to_owned()),
        local: account.server.is_none(),
        has_last_organization: account.has_last_organization,
        current: account.current,
        companies: account.organizations.clone(),
    }
}

/// Where the person chose to keep the account they add.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Keeping {
    /// The build's default server.
    Default,
    /// Another server, by its address.
    Server,
    /// This device alone.
    Device,
}

/// Adding an account: where to keep it, then a server's address and the gateway token it may
/// need, or a name for one on this device; then, while the host signs in, the code the
/// browser's page asks for, with Open browser and Check now. It closes once an account it did
/// not list when it opened (`known`) is added. Escape steps back to choosing where, or closes.
#[component]
pub(super) fn AddAccountDialog(
    core: Signal<CoreView>,
    known: Vec<String>,
    done: Callback<Closed>,
) -> NodeId {
    let signing = create_memo(move || core.with(|view| view.signing_in.clone()));
    let error = create_memo(move || core.with(|view| view.account_error.clone()));
    let added = create_memo(move || {
        core.with(|view| {
            view.accounts
                .iter()
                .any(|account| !known.contains(&account.key))
        })
    });
    create_effect(move || {
        if added.get() {
            done.call(Closed::Added);
        }
    });
    // A sign-in it showed that ends with no error signed in: an account signed in to again
    // (reauthenticating) is no account added, but closes it all the same.
    let showed = StoredCell::new(false);
    create_effect(move || {
        let signing = core.with(|view| view.signing_in.is_some());
        let failed = core.with(|view| view.account_error.is_some());
        match signing {
            true => showed.set(true),
            false if showed.get() && !failed => done.call(Closed::Added),
            false => {}
        }
    });
    // Where to keep it: the default server, this device, or another server, as be-host's own
    // dialog offers them.
    let default = create_memo(move || core.with(|view| view.default_server.clone()));
    let has_default = create_memo(move || default.with(Option::is_some));
    let default_name = create_memo(move || {
        default.with(|it| it.as_ref().map(|it| it.name.clone()).unwrap_or_default())
    });
    let default_url = create_memo(move || {
        default.with(|it| it.as_ref().map(|it| it.url.clone()).unwrap_or_default())
    });
    let selection = create_signal(None::<Keeping>);
    let url = create_signal(String::new());
    let gateway = create_signal(String::new());
    let name = create_signal(String::new());
    let pending = create_memo(move || signing.with(Option::is_some));
    let browser = create_memo(move || {
        signing.with(|signing| signing.as_ref().and_then(|signing| signing.browser.clone()))
    });
    let waiting = create_memo(move || browser.with(Option::is_some));
    let connecting = create_memo(move || pending.get() && !waiting.get());
    let editing = create_memo(move || selection.with(Option::is_some) && !pending.get());
    let choosing = create_memo(move || selection.with(Option::is_none) && !pending.get());
    let local = create_memo(move || selection.get() == Some(Keeping::Device));
    let remote = create_memo(move || selection.with(Option::is_some) && !local.get());
    let gateway_required = create_memo(move || {
        selection.get() == Some(Keeping::Default)
            && default.with(|it| it.as_ref().is_some_and(|it| it.gateway_required))
    });
    let gateway_label = create_memo(move || match gateway_required.get() {
        true => "Gateway token".to_owned(),
        false => "Gateway token (optional)".to_owned(),
    });
    let ready = create_memo(move || {
        !pending.get()
            && match selection.get() {
                Some(Keeping::Device) => name.with(|name| !name.trim().is_empty()),
                Some(Keeping::Default) => {
                    !gateway_required.get() || gateway.with(|token| !token.trim().is_empty())
                }
                Some(Keeping::Server) => url.with(|url| !url.trim().is_empty()),
                None => false,
            }
    });
    let submit = move || {
        if !ready.get_untracked() {
            return;
        }
        match selection.get_untracked() {
            Some(Keeping::Device) => {
                let name = name.get_untracked().trim().to_owned();
                ask_core(CoreCommand::AddAccount(NewAccount::Local(name)));
            }
            Some(keeping) => {
                let address = match keeping {
                    Keeping::Default => default_url.get_untracked(),
                    _ => url.get_untracked().trim().to_owned(),
                };
                // The token leaves the field as it is handed over, as the lock screen's
                // password does.
                let typed = gateway.get_untracked();
                gateway.set(String::new());
                let gateway = (!typed.trim().is_empty()).then(|| Password::from(typed));
                ask_core(CoreCommand::AddAccount(NewAccount::Server(ServerAccount {
                    url: address,
                    gateway,
                })));
            }
            None => {}
        }
    };
    let title = create_memo(move || {
        match (pending.get(), selection.get()) {
            (true, _) => SIGN_IN_TITLE,
            (false, Some(Keeping::Device)) => "Create local account",
            (false, Some(_)) => "Connect to server",
            (false, None) => ADD_ACCOUNT_TITLE,
        }
        .to_owned()
    });
    let footer_primary = create_memo(move || match (waiting.get(), editing.get()) {
        (true, _) => "Open browser".to_owned(),
        (false, true) if local.get() => "Create account".to_owned(),
        (false, true) => "Continue".to_owned(),
        (false, false) => String::new(),
    });
    let code = create_memo(move || {
        browser.with(|it| it.as_ref().map(|it| it.code.clone()).unwrap_or_default())
    });
    let link = create_memo(move || {
        browser.with(|it| it.as_ref().map(|it| it.url.clone()).unwrap_or_default())
    });
    let copy = |text: String| {
        be_wasm_ops::guest::spawn(async move {
            let item = be_lib_ui_types::ClipItem::text(text);
            // A host that cannot hear it has stopped, which the root hears from its frames.
            let _ = host::copy(vec![be_wasm_ops::encode(&item)]).await;
        });
    };
    // Escape steps back to choosing where, or closes.
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Bubble,
        KeyReveal::Focus,
        move |event| {
            let back = event
                .press()
                .is_some_and(|press| press.pressed && !press.repeat && press.key == Key::Escape)
                && editing.get_untracked();
            if back {
                selection.set(None);
            }
            back
        },
    );
    view! {
        <Frame width=Length::FILL>
            <DialogBody
                title={title}
                actions={vec![
                    DialogAction::secondary("Back", move || selection.set(None)).shown(editing),
                    DialogAction::cancel("Cancel"),
                    DialogAction::secondary("Check now", || ask_core(CoreCommand::CheckSignIn))
                        .shown(waiting),
                    DialogAction::primary(footer_primary, move || match waiting.get_untracked() {
                        true => ask_core(CoreCommand::OpenSignInPage),
                        false => submit(),
                    })
                    .disabled(move || !waiting.get() && !ready.get())
                    .busy(connecting)
                    .shown(create_memo(move || !footer_primary.with(String::is_empty))),
                ]}
                on_dismiss={move || done.call(Closed::Dismissed)}
            >
                <List spacing=Space::Xl2>
                    <Show value={choosing}>
                        <List spacing=Space::Lg>
                            <Text style=TextStyle::PARAGRAPH>"Choose where to keep your account."</Text>
                            <Frame
                                fill=ColorRole::Surface
                                border={Border::DEFAULT}
                                radius=Radius::Md
                                padding=Space::Sm
                            >
                                <List spacing=Space::Xs>
                                    <Show value={has_default}>
                                        <ActionRow
                                            label={default_name}
                                            detail={default_url}
                                            glyph=ICON_CLOUD
                                            on_click={move || selection.set(Some(Keeping::Default))}
                                        />
                                    </Show>
                                    <ActionRow
                                        label="On this device"
                                        detail="A local account, stored on this device"
                                        glyph=ICON_MONITOR
                                        on_click={move || selection.set(Some(Keeping::Device))}
                                    />
                                    <ActionRow
                                        label="Another server"
                                        detail="Connect using a server address"
                                        glyph=ICON_PLUS
                                        on_click={move || selection.set(Some(Keeping::Server))}
                                    />
                                </List>
                            </Frame>
                        </List>
                    </Show>
                    <Show value={editing}>
                        <List spacing=Space::Xl>
                            <Show value={local}>
                                <List spacing=Space::Xl>
                                    <Text style=TextStyle::PARAGRAPH>
                                        "Your data stays on this device. You only need a display name."
                                    </Text>
                                    <List spacing=Space::Sm>
                                        <Text style=TextStyle::CAPTION>"Display name"</Text>
                                        <TextInput
                                            value={name}
                                            label="Display name"
                                            placeholder="Your name"
                                            focused=true
                                            on_submit={move |_: String| submit()}
                                        />
                                    </List>
                                </List>
                            </Show>
                            <Show value={move || selection.get() == Some(Keeping::Default)}>
                                <List spacing=Space::Xs>
                                    <Text size=TextSize::Lg>{default_name}</Text>
                                    <Text style=TextStyle::CAPTION>{default_url}</Text>
                                </List>
                            </Show>
                            <Show value={move || selection.get() == Some(Keeping::Server)}>
                                <List spacing=Space::Sm>
                                    <Text style=TextStyle::CAPTION>"Server address"</Text>
                                    <TextInput
                                        value={url}
                                        label="Server address"
                                        placeholder="https://example.com"
                                        focused=true
                                        on_submit={move |_: String| submit()}
                                    />
                                </List>
                            </Show>
                            <Show value={remote}>
                                <List spacing=Space::Sm>
                                    <Text style=TextStyle::CAPTION>{gateway_label}</Text>
                                    <TextInput
                                        value={gateway}
                                        label={gateway_label}
                                        kind={InputKind::Password}
                                        on_submit={move |_: String| submit()}
                                    />
                                </List>
                                <Text style=TextStyle::CAPTION>
                                    "Next, you’ll sign in or create an account in your browser."
                                </Text>
                            </Show>
                        </List>
                    </Show>
                    <Show value={waiting}>
                        <List spacing=Space::Xl>
                            <Text style=TextStyle::PARAGRAPH>
                                "Sign in and approve this code in your browser. You can also use a browser on another device. This is checked every few seconds; once you have approved it, choose Check now."
                            </Text>
                            <Card>
                                <List spacing=Space::Xl>
                                    <List spacing=Space::Sm>
                                        <Text style=TextStyle::CAPTION>"Device code"</Text>
                                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Lg>
                                            <ListItem grow=1.0>
                                                <Text size=TextSize::Xl>{code}</Text>
                                            </ListItem>
                                            <Button
                                                icon_only=true
                                                label="Copy code"
                                                glyph=ICON_COPY
                                                variant=ButtonVariant::Ghost
                                                on_click={move || copy(code.get_untracked())}
                                            />
                                        </List>
                                    </List>
                                    <Separator />
                                    <List spacing=Space::Sm>
                                        <Text style=TextStyle::CAPTION>"Sign-in link"</Text>
                                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Lg>
                                            <ListItem grow=1.0>
                                                <Text style=TextStyle::CAPTION>{link}</Text>
                                            </ListItem>
                                            <Button
                                                icon_only=true
                                                label="Copy link"
                                                glyph=ICON_COPY
                                                variant=ButtonVariant::Ghost
                                                on_click={move || copy(link.get_untracked())}
                                            />
                                        </List>
                                    </List>
                                </List>
                            </Card>
                            <Text style=TextStyle::CAPTION>
                                "Waiting for approval. This window will continue automatically."
                            </Text>
                        </List>
                    </Show>
                    <Show value={connecting}>
                        <Text style=TextStyle::PARAGRAPH>"Connecting to the server…"</Text>
                    </Show>
                    <be_root_accounts_ui::ErrorText text={error} />
                </List>
            </DialogBody>
        </Frame>
    }
}

/// The question before removing an account kept on this device alone, `name`, whose blocks go
/// with it: Remove, which asks the block stack to forget it, or Keep. Escape keeps it.
#[component]
pub(super) fn ForgetAccountDialog(key: String, name: String, done: Callback<Closed>) -> NodeId {
    let remove = move || {
        ask_core(CoreCommand::ForgetAccount(key.clone()));
        done.call(Closed::Dismissed);
    };
    view! {
        <Frame width=Length::FILL>
            <DialogBody
                kind=DialogKind::Alert
                title={format!("Remove {name}?")}
                actions={vec![
                    DialogAction::destructive("Remove", remove),
                    DialogAction::cancel("Keep"),
                ]}
                on_dismiss={move || done.call(Closed::Dismissed)}
            >
                <Text style=TextStyle::PARAGRAPH>
                    "This account lives only on this device and has no password, so once it is removed nobody can open it again. Its blocks and settings are lost, except what it shared with other accounts."
                </Text>
            </DialogBody>
        </Frame>
    }
}

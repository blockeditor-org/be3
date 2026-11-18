//! What the block stack waits for before an organization's blocks open: the accounts and adding or removing one, the organizations, the sessions, the recovery phrase, unlocking, and a failure.

use super::*;

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
                        <OrganizationsCard core={core} />
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

/// The recovery phrase, while the person writes it down: its words, asked for as it shows and
/// held only while it does, numbered; a field for the words at the checked positions, typed
/// back as a secret and cleared; Confirm, New phrase, and Keep the old one while replacing.
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
    ask.call(SecretCommand::RecoveryPhrase);
    // The one copy of the words it shows beside the phrase itself.
    let words = create_memo(move || {
        secret.with(|secret| {
            secret
                .as_ref()
                .and_then(Held::phrase)
                .map(|phrase| {
                    phrase
                        .split_whitespace()
                        .enumerate()
                        .map(|(at, word)| format!("{} {word}", at + 1))
                        .collect::<Vec<_>>()
                        .join("   ")
                })
                .unwrap_or_default()
        })
    });
    let asked = create_memo(move || {
        area.with(|area| {
            let positions: Vec<String> = area.checked.iter().map(u32::to_string).collect();
            format!(
                "Type words {} to show you wrote it down.",
                positions.join(", ")
            )
        })
    });
    let typed = create_signal(String::new());
    let confirm = move || {
        let words = typed.get_untracked();
        typed.set(String::new());
        if !words.trim().is_empty() {
            ask_core(CoreCommand::ConfirmRecovery(Password::from(words)));
        }
    };
    let renew = move || {
        // The host makes the new phrase with the command, before the request after it.
        ask_core(CoreCommand::NewRecoveryPhrase);
        ask.call(SecretCommand::RecoveryPhrase);
    };
    view! {
        <be_root_accounts_ui::Column>
            <Text style=TextStyle::TITLE>{RECOVERY_TITLE}</Text>
            <Text style=TextStyle::CAPTION>
                "Write these words down and keep them safe: they are the only way back to your blocks if you lose this device."
            </Text>
            <Text>{words}</Text>
            <Text style=TextStyle::CAPTION>{asked}</Text>
            <be_root_accounts_ui::ErrorText text={error} />
            <TextInput
                value={typed}
                label="Words"
                placeholder="Words"
                kind={InputKind::Password}
                on_submit={move |_: String| confirm()}
            />
            <List direction=Direction::Horizontal spacing=Space::Md>
                <Button
                    label="Confirm"
                    variant=ButtonVariant::Primary
                    disabled={busy}
                    on_click={confirm}
                />
                <Button
                    label="New phrase"
                    variant=ButtonVariant::Secondary
                    disabled={busy}
                    on_click={renew}
                />
                <Show value={replacing}>
                    <Button
                        label="Keep the old one"
                        variant=ButtonVariant::Secondary
                        on_click={|| ask_core(CoreCommand::CancelRecovery)}
                    />
                </Show>
            </List>
        </be_root_accounts_ui::Column>
    }
}

/// Unlocking the account's keys on this device: its recovery phrase, typed as a secret and
/// cleared; or pairing with a device that holds them, whose code, asked for, shows until it is
/// withdrawn.
#[component]
pub(super) fn UnlockCard(
    core: Signal<CoreView>,
    secret: Signal<Option<Held>>,
    pending: Signal<bool>,
    ask: Callback<SecretCommand>,
) -> NodeId {
    let area = create_memo(move || core.with(|view| view.unlock.clone()));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let lead = create_memo(move || {
        area.with(|area| format!("The keys of {} are not on this device yet.", area.account))
    });
    let code = create_memo(move || {
        secret.with(|secret| secret.as_ref().and_then(Held::code).map(str::to_owned))
    });
    let pairing = create_memo(move || code.with(Option::is_some));
    let unpaired = create_memo(move || !pairing.get());
    let code_line = create_memo(move || {
        let account = area.with(|area| area.account.clone());
        code.get()
            .map(|code| format!("Enter {code} on a device signed in as {account}."))
            .unwrap_or_default()
    });
    let phrase = create_signal(String::new());
    let unlock = move || {
        let typed = phrase.get_untracked();
        phrase.set(String::new());
        if !typed.trim().is_empty() {
            ask_core(CoreCommand::UnlockWithPhrase(Password::from(typed)));
        }
    };
    view! {
        <be_root_accounts_ui::Column>
            <Text style=TextStyle::TITLE>{UNLOCK_TITLE}</Text>
            <Text style=TextStyle::CAPTION>{lead}</Text>
            <be_root_accounts_ui::ErrorText text={error} />
            <List direction=Direction::Horizontal align=Align::End spacing=Space::Md>
                <ListItem grow=1.0>
                    <TextInput
                        value={phrase}
                        label="Recovery phrase"
                        placeholder="Recovery phrase"
                        kind={InputKind::Password}
                        on_submit={move |_: String| unlock()}
                    />
                </ListItem>
                <Button label="Unlock" variant=ButtonVariant::Primary on_click={unlock} />
            </List>
            <Show value={unpaired}>
                <Button
                    label="Pair with another device"
                    variant=ButtonVariant::Secondary
                    disabled={pending}
                    on_click={move || ask.call(SecretCommand::PairingCode)}
                />
            </Show>
            <Show value={pairing}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <ListItem grow=1.0>
                        <Text>{code_line}</Text>
                    </ListItem>
                    <Button
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        on_click={move || {
                            secret.set(None);
                            ask_core(CoreCommand::CancelPairing);
                        }}
                    />
                </List>
            </Show>
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
    let calm = create_memo(move || !asking.get());
    let question = create_memo(move || {
        let unsaved = failure.with(|it| it.as_ref().map_or(0, |it| it.unsaved));
        match unsaved {
            0 => "Delete this device's copy of your blocks? They download again from the server."
                .to_owned(),
            1 => "Delete this device's copy of your blocks? 1 change not saved is lost.".to_owned(),
            unsaved => format!(
                "Delete this device's copy of your blocks? {unsaved} changes not saved are lost."
            ),
        }
    });
    view! {
        <be_root_accounts_ui::Column>
            <Text style=TextStyle::TITLE>{FAILURE_TITLE}</Text>
            <Text style=TextStyle::PARAGRAPH>{message}</Text>
            <Show value={calm}>
                <List direction=Direction::Horizontal spacing=Space::Md>
                    <Button
                        label="Restart"
                        variant=ButtonVariant::Primary
                        on_click={|| ask_core(CoreCommand::Restart)}
                    />
                    <Button
                        label="Delete local copy"
                        variant=ButtonVariant::Secondary
                        on_click={move || asking.set(true)}
                    />
                    <Button
                        label="Quit"
                        variant=ButtonVariant::Secondary
                        on_click={|| ask_core(CoreCommand::Exit)}
                    />
                </List>
            </Show>
            <Show value={asking}>
                <List spacing=Space::Md>
                    <Text style=TextStyle::PARAGRAPH>{question}</Text>
                    <List direction=Direction::Horizontal spacing=Space::Md>
                        <Button
                            label="Keep"
                            variant=ButtonVariant::Secondary
                            on_click={move || asking.set(false)}
                        />
                        <Button
                            label="Delete"
                            variant=ButtonVariant::Danger
                            on_click={move || {
                                asking.set(false);
                                ask_core(CoreCommand::DeleteClientDatabase);
                            }}
                        />
                    </List>
                </List>
            </Show>
        </be_root_accounts_ui::Column>
    }
}

/// The account's organizations: each to open, in its last session or choosing one; an
/// invitation opened, to join or put aside; creating one; opening an invitation's link, which
/// the field hands over as a secret and clears; and switching account.
#[component]
pub(super) fn OrganizationsCard(core: Signal<CoreView>) -> NodeId {
    let area = create_memo(move || core.with(|view| view.organizations.clone()));
    let listed = create_memo(move || area.with(|area| area.listed.clone()));
    let loading = create_memo(move || listed.get() == OrganizationsListed::Loading);
    let failed = create_memo(move || listed.get() == OrganizationsListed::Failed);
    let rows = create_memo(move || match listed.get() {
        OrganizationsListed::Loaded(rows) => rows,
        _ => Vec::new(),
    });
    let busy = create_memo(move || area.with(|area| area.busy));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let joining = create_memo(move || area.with(|area| area.joining.is_some()));
    let invited = create_memo(move || {
        area.with(|area| {
            area.joining
                .as_ref()
                .map(|invitation| {
                    let lead = format!(
                        "{} invited you to join {}",
                        invitation.inviter, invitation.organization
                    );
                    match &invitation.problem {
                        Some(problem) => format!("{lead}. {problem}"),
                        None => lead,
                    }
                })
                .unwrap_or_default()
        })
    });
    let cannot_join = create_memo(move || {
        busy.get() || !area.with(|area| area.joining.as_ref().is_some_and(|it| it.can_join))
    });
    let name = create_signal(String::new());
    let link = create_signal(String::new());
    let create = move || {
        let named = name.get_untracked().trim().to_owned();
        if !named.is_empty() {
            name.set(String::new());
            ask_core(CoreCommand::CreateOrganization(named));
        }
    };
    let open_link = move || {
        // The link is a bearer secret: shown as typed, so the person sees what they pasted,
        // it leaves the field as it is handed over.
        let typed = link.get_untracked();
        link.set(String::new());
        if !typed.trim().is_empty() {
            ask_core(CoreCommand::OpenInvitation(Password::from(typed)));
        }
    };
    view! {
        <be_root_accounts_ui::Column>
            <Text style=TextStyle::TITLE>{ORGANIZATIONS_TITLE}</Text>
            <Show value={loading}>
                <Text style=TextStyle::CAPTION>"Loading organizations…"</Text>
            </Show>
            <Show value={failed}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <ListItem grow=1.0>
                        <Text style=TextStyle::CAPTION>
                            "The organizations could not be listed."
                        </Text>
                    </ListItem>
                    <Button
                        label="Try again"
                        variant=ButtonVariant::Secondary
                        on_click={|| ask_core(CoreCommand::ReloadOrganizations)}
                    />
                </List>
            </Show>
            <ForEach
                value={rows}
                key={|row: &OrganizationRow| row.id}
            >|row: ReadSignal<OrganizationRow>| {
                let id = row.with_untracked(|row| row.id);
                let name = create_memo(move || row.with(|row| row.name.clone()));
                view! {
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <ListItem grow=1.0>
                            <Text>{name}</Text>
                        </ListItem>
                        <Button
                            label="Sessions"
                            variant=ButtonVariant::Secondary
                            on_click={move || ask_core(CoreCommand::ChooseSessionIn(id))}
                        />
                        <Button
                            label="Open"
                            variant=ButtonVariant::Primary
                            on_click={move || ask_core(CoreCommand::OpenOrganization(id))}
                        />
                    </List>
                }
            }</ForEach>
            <Show value={joining}>
                <Card>
                    <List spacing=Space::Md>
                        <Text>{invited}</Text>
                        <List
                            direction=Direction::Horizontal
                            justify=Justify::End
                            spacing=Space::Md
                        >
                            <Button
                                label="Not now"
                                variant=ButtonVariant::Secondary
                                on_click={|| ask_core(CoreCommand::CancelJoin)}
                            />
                            <Button
                                label="Join"
                                variant=ButtonVariant::Primary
                                disabled={cannot_join}
                                on_click={|| ask_core(CoreCommand::JoinInvited)}
                            />
                        </List>
                    </List>
                </Card>
            </Show>
            <be_root_accounts_ui::ErrorText text={error} />
            <List direction=Direction::Horizontal align=Align::End spacing=Space::Md>
                <ListItem grow=1.0>
                    <TextInput
                        value={name}
                        label="New organization"
                        placeholder="New organization"
                        on_submit={move |_: String| create()}
                    />
                </ListItem>
                <Button label="Create" disabled={busy} on_click={create} />
            </List>
            <List direction=Direction::Horizontal align=Align::End spacing=Space::Md>
                <ListItem grow=1.0>
                    <TextInput
                        value={link}
                        label="Invitation link"
                        placeholder="Invitation link"
                        on_submit={move |_: String| open_link()}
                    />
                </ListItem>
                <Button label="Open invitation" disabled={busy} on_click={open_link} />
            </List>
            <Button
                label="Switch account"
                variant=ButtonVariant::Secondary
                on_click={|| ask_core(CoreCommand::SwitchAccount)}
            />
        </be_root_accounts_ui::Column>
    }
}

/// The organization's sessions: those of the type this device opens, or of every type, each to
/// open; a new one; this device's profiles, each to use, and a new one; and Close, back in the
/// session open.
#[component]
pub(super) fn SessionsCard(core: Signal<CoreView>) -> NodeId {
    let area = create_memo(move || core.with(|view| view.sessions.clone()));
    let loading = create_memo(move || !area.with(|area| area.loaded));
    // Which types it lists is the root's own choice, not the host's.
    let every = create_signal(false);
    let shown = create_memo(move || {
        let every = every.get();
        area.with(|area| {
            area.sessions
                .iter()
                .filter(|session| every || session.session_type == area.session_type)
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let profiles = create_memo(move || area.with(|area| area.profiles.clone()));
    let new_label = create_memo(move || {
        area.with(|area| {
            let name = area
                .session_kinds
                .iter()
                .find(|kind| kind.id == area.session_type)
                .map(|kind| kind.name.clone())
                .unwrap_or_default();
            format!("New {name} session")
        })
    });
    let every_label = create_memo(move || match every.get() {
        true => "Show this type only".to_owned(),
        false => "Show every type".to_owned(),
    });
    view! {
        <be_root_accounts_ui::Column>
            <Text style=TextStyle::TITLE>{SESSIONS_TITLE}</Text>
            <Show value={loading}>
                <Text style=TextStyle::CAPTION>"Loading sessions and profiles…"</Text>
            </Show>
            <ForEach
                value={shown}
                key={|session: &SessionRow| session.id}
            >|session: ReadSignal<SessionRow>| {
                let id = session.with_untracked(|session| session.id);
                let label = create_memo(move || {
                    session.with(|session| match session.current {
                        true => format!("{} (last used)", session.name),
                        false => session.name.clone(),
                    })
                });
                view! {
                    <Button
                        label={label}
                        variant=ButtonVariant::Secondary
                        on_click={move || ask_core(CoreCommand::OpenSession(id))}
                    />
                }
            }</ForEach>
            <List direction=Direction::Horizontal spacing=Space::Md>
                <Button
                    label={new_label}
                    variant=ButtonVariant::Primary
                    on_click={move || ask_core(CoreCommand::NewSession(area.with_untracked(|area| area.session_type)))}
                />
                <Button
                    label={every_label}
                    variant=ButtonVariant::Secondary
                    on_click={move || every.set(!every.get_untracked())}
                />
            </List>
            <Text weight=Weight::Bold>"Profiles"</Text>
            <ForEach
                value={profiles}
                key={|profile: &ProfileRow| profile.id}
            >|profile: ReadSignal<ProfileRow>| {
                let id = profile.with_untracked(|profile| profile.id);
                let label = create_memo(move || {
                    profile.with(|profile| match profile.current {
                        true => format!("{} (in use)", profile.name),
                        false => profile.name.clone(),
                    })
                });
                view! {
                    <Button
                        label={label}
                        variant=ButtonVariant::Secondary
                        on_click={move || ask_core(CoreCommand::UseProfile(id))}
                    />
                }
            }</ForEach>
            <List direction=Direction::Horizontal spacing=Space::Md>
                <Button
                    label="New profile"
                    variant=ButtonVariant::Secondary
                    on_click={|| ask_core(CoreCommand::NewProfile)}
                />
                <Button
                    label="Close"
                    variant=ButtonVariant::Secondary
                    on_click={|| ask_core(CoreCommand::CloseSessions)}
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
    Server,
    Device,
}

/// Adding an account: where to keep it, then a server's address and the gateway token it may
/// need, or a name for one on this device; then, while the host signs in, the code the
/// browser's page asks for, with Open browser and Check now. It closes once an account it did
/// not list when it opened (`known`) is added. Escape steps back to choosing where, or closes.
#[component]
pub(super) fn AddAccountDialog(core: Signal<CoreView>, known: Vec<String>, done: Callback<Closed>) -> NodeId {
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
    let keeping = create_signal(None::<Keeping>);
    let url = create_signal(String::new());
    let gateway = create_signal(String::new());
    let name = create_signal(String::new());
    let waiting = create_memo(move || signing.with(Option::is_some));
    let browser = create_memo(move || {
        signing.with(|signing| signing.as_ref().and_then(|signing| signing.browser.clone()))
    });
    let has_browser = create_memo(move || browser.with(Option::is_some));
    let connecting = create_memo(move || waiting.get() && !has_browser.get());
    let choosing = create_memo(move || !waiting.get() && keeping.get().is_none());
    let to_server = create_memo(move || !waiting.get() && keeping.get() == Some(Keeping::Server));
    let on_device = create_memo(move || !waiting.get() && keeping.get() == Some(Keeping::Device));
    let code = create_memo(move || {
        browser.with(|browser| {
            browser
                .as_ref()
                .map(|browser| format!("Enter {} on the page that opens.", browser.code))
                .unwrap_or_default()
        })
    });
    let connect = move || {
        let address = url.get_untracked().trim().to_owned();
        if address.is_empty() {
            return;
        }
        // The token leaves the field as it is handed over, as the lock screen's password does.
        let typed = gateway.get_untracked();
        gateway.set(String::new());
        let gateway = (!typed.trim().is_empty()).then(|| Password::from(typed));
        ask_core(CoreCommand::AddAccount(NewAccount::Server(ServerAccount {
            url: address,
            gateway,
        })));
    };
    let create = move || {
        let name = name.get_untracked().trim().to_owned();
        if !name.is_empty() {
            ask_core(CoreCommand::AddAccount(NewAccount::Local(name)));
        }
    };
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Bubble,
        KeyReveal::Focus,
        move |event| {
            let closes = event
                .press()
                .is_some_and(|press| press.pressed && !press.repeat && press.key == Key::Escape);
            if closes {
                match keeping.get_untracked().is_some() && !waiting.get_untracked() {
                    true => keeping.set(None),
                    false => done.call(Closed::Dismissed),
                }
            }
            closes
        },
    );
    let dismiss = move || done.call(Closed::Dismissed);
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <List spacing=Space::None>
                <Show value={choosing}>
                    <DialogBody
                        title=ADD_ACCOUNT_TITLE
                        actions={vec![DialogAction::cancel("Cancel")]}
                        on_dismiss={dismiss}
                    >
                        <be_root_accounts_ui::ErrorText text={error} />
                        <List direction=Direction::Horizontal spacing=Space::Md>
                            <Button
                                label="Connect to a server"
                                variant=ButtonVariant::Secondary
                                on_click={move || keeping.set(Some(Keeping::Server))}
                            />
                            <Button
                                label="On this device"
                                variant=ButtonVariant::Secondary
                                on_click={move || keeping.set(Some(Keeping::Device))}
                            />
                        </List>
                    </DialogBody>
                </Show>
                <Show value={to_server}>
                    <DialogBody
                        title="Connect to server"
                        actions={vec![
                            DialogAction::secondary("Back", move || keeping.set(None)),
                            DialogAction::cancel("Cancel"),
                            DialogAction::primary("Continue", connect)
                                .disabled(move || url.with(|url| url.trim().is_empty())),
                        ]}
                        on_dismiss={dismiss}
                    >
                        <be_root_accounts_ui::ErrorText text={error} />
                        <TextInput
                            value={url}
                            label="Server address"
                            focused=true
                            on_submit={move |_: String| connect()}
                        />
                        <TextInput
                            value={gateway}
                            label="Gateway token (optional)"
                            kind={InputKind::Password}
                            on_submit={move |_: String| connect()}
                        />
                    </DialogBody>
                </Show>
                <Show value={on_device}>
                    <DialogBody
                        title="Create local account"
                        actions={vec![
                            DialogAction::secondary("Back", move || keeping.set(None)),
                            DialogAction::cancel("Cancel"),
                            DialogAction::primary("Create account", create)
                                .disabled(move || name.with(|name| name.trim().is_empty())),
                        ]}
                        on_dismiss={dismiss}
                    >
                        <be_root_accounts_ui::ErrorText text={error} />
                        <TextInput
                            value={name}
                            label="Your name"
                            focused=true
                            on_submit={move |_: String| create()}
                        />
                    </DialogBody>
                </Show>
                <Show value={has_browser}>
                    <DialogBody
                        title=SIGN_IN_TITLE
                        actions={vec![
                            DialogAction::cancel("Cancel"),
                            DialogAction::secondary("Check now", || ask_core(CoreCommand::CheckSignIn)),
                            DialogAction::primary("Open browser", || ask_core(CoreCommand::OpenSignInPage)),
                        ]}
                        on_dismiss={dismiss}
                    >
                        <Text style=TextStyle::PARAGRAPH>{code}</Text>
                    </DialogBody>
                </Show>
                <Show value={connecting}>
                    <DialogBody
                        title=SIGN_IN_TITLE
                        actions={vec![DialogAction::cancel("Cancel")]}
                        on_dismiss={dismiss}
                    >
                        <Text style=TextStyle::PARAGRAPH>"Connecting…"</Text>
                    </DialogBody>
                </Show>
            </List>
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
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Bubble,
        KeyReveal::Focus,
        move |event| {
            let closes = event
                .press()
                .is_some_and(|press| press.pressed && !press.repeat && press.key == Key::Escape);
            if closes {
                done.call(Closed::Dismissed);
            }
            closes
        },
    );
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
        >
            <DialogBody
                kind=DialogKind::Alert
                title=FORGET_ACCOUNT_TITLE
                scrolls=false
                actions={vec![
                    DialogAction::destructive("Remove", remove),
                    DialogAction::cancel("Keep"),
                ]}
                on_dismiss={move || done.call(Closed::Dismissed)}
            >
                <Text style=TextStyle::PARAGRAPH>
                    {format!("Remove {name}? Its blocks are kept only on this device, and go with it.")}
                </Text>
            </DialogBody>
        </Frame>
    }
}

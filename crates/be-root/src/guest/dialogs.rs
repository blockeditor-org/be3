//! The block stack's dialogs and the host's questions in the dialog's place: keeping display settings, unsent edits, pairing a device, signing in again, discarding unsaved changes, invitations, members, agents, leaving a block, contacts and share links.

use super::*;

/// The question of keeping display settings that just changed: Keep, or Revert. It counts down
/// on its clock, which the root pins to its host's frame clock, from `shown_at`, the host's
/// time it opened at, to `revert-at`, when the host reverts them itself; reaching it, it closes
/// answering not kept, a courtesy the host does not wait for. Escape and a press on the scrim
/// revert at once, as be-host's prompt does (`Presented`).
#[component]
pub(super) fn KeepDisplayDialog(
    asked: DisplayQuestion,
    shown_at: Duration,
    done: Callback<Closed>,
) -> NodeId {
    let round = asked.round;
    let within = Duration::from_nanos(asked.revert_at).saturating_sub(shown_at);
    let deadline = now() + within;
    let left = create_signal(within);
    let ticking = create_timer(move || {
        let remaining = deadline.saturating_duration_since(now());
        left.set(remaining);
        if remaining.is_zero() {
            done.call(Closed::Reverted(round));
            return None;
        }
        Some(remaining.min(Duration::from_secs(1)))
    });
    ticking.start(within.min(Duration::from_secs(1)));
    let message = create_memo(move || match left.get().as_secs_f64().ceil() as u64 {
        1 => "Reverting in 1 second.".to_owned(),
        seconds => format!("Reverting in {seconds} seconds."),
    });
    view! {
        <Frame width=Length::FILL>
            <DialogBody
                kind=DialogKind::Alert
                title=KEEP_DISPLAY_TITLE
                actions={vec![
                    DialogAction::primary("Keep", move || done.call(Closed::Kept(round))),
                    DialogAction::cancel("Revert"),
                ]}
                on_dismiss={move || done.call(Closed::Reverted(round))}
            >
                <Text style=TextStyle::PARAGRAPH>{message}</Text>
            </DialogBody>
        </Frame>
    }
}

/// The list of the block stack's unsent edits, as `core` shows them: each block's name and why
/// its edits were set aside, with Restore, which sends them again, and Discard, which asks
/// first, in the list's place. A block that leaves the list takes its question with it. Escape
/// cancels the question, or else closes the list.
#[component]
pub(super) fn UnsentDialog(core: Signal<CoreView>, done: Callback<Closed>) -> NodeId {
    // A memo per area of the core's view: this one changes only when the unsent edits do.
    let edits = create_memo(move || core.with(|view| view.unsent.clone()));
    let discarding = create_signal(None::<[u8; 16]>);
    let asked = create_memo(move || {
        let block = discarding.get()?;
        edits.with(|edits| edits.iter().find(|edit| edit.block == block).cloned())
    });
    let asks = create_memo(move || asked.with(Option::is_some));
    let question = create_memo(move || {
        let name = asked.with(|edit| edit.as_ref().map(edit_name).unwrap_or_default());
        format!(
            "Discard the edits to {name}? They are kept only on this device, and cannot be brought back."
        )
    });
    let discard = move || {
        if let Some(edit) = asked.get() {
            ask_core(CoreCommand::DiscardUnsent(edit.block));
        }
        discarding.set(None);
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
                match asks.get_untracked() {
                    true => discarding.set(None),
                    false => done.call(Closed::Dismissed),
                }
            }
            closes
        },
    );
    view! {
        <Frame width=Length::FILL>
            <Show
                value={asks}
                fallback={move || view! {
                    <DialogBody
                        title=UNSENT_TITLE
                        actions={vec![DialogAction::cancel("Close")]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <List spacing=Space::Md>
                            <Text style=TextStyle::CAPTION>
                                "These edits were made on this device but could not be sent: you can no longer edit these blocks, or the server refused them. They stay here until you restore or discard them. Restore sends them again."
                            </Text>
                            <ForEach
                                value={edits}
                                key={|edit: &UnsentEdit| edit.block}
                                fallback={|| view! {
                                    <Text style=TextStyle::CAPTION>"No unsent edits."</Text>
                                }}
                            >|edit: ReadSignal<UnsentEdit>| {
                                let block = edit.with_untracked(|edit| edit.block);
                                let name = create_memo(move || edit.with(edit_name));
                                let detail = create_memo(move || edit.with(edit_detail));
                                view! {
                                    <List
                                        direction=Direction::Horizontal
                                        align=Align::Center
                                        spacing=Space::Md
                                    >
                                        <ListItem grow=1.0>
                                            <List spacing=Space::Xs2>
                                                <Text>{name}</Text>
                                                <Text style=TextStyle::CAPTION>{detail}</Text>
                                            </List>
                                        </ListItem>
                                        <Button
                                            label="Restore"
                                            variant=ButtonVariant::Primary
                                            disabled={move || !edit.get().offerable}
                                            on_click={move || ask_core(CoreCommand::RestoreUnsent(block))}
                                        />
                                        <Button
                                            icon_only=true
                                            glyph=ICON_TRASH
                                            label="Discard"
                                            on_click={move || discarding.set(Some(block))}
                                        />
                                    </List>
                                }
                            }</ForEach>
                        </List>
                    </DialogBody>
                }}
            >
                <DialogBody
                    kind=DialogKind::Alert
                    title=DISCARD_UNSENT_TITLE
                    actions={vec![
                        DialogAction::destructive("Discard", discard),
                        DialogAction::cancel("Keep"),
                    ]}
                    on_dismiss={move || discarding.set(None)}
                >
                    <Text style=TextStyle::PARAGRAPH>{question}</Text>
                </DialogBody>
            </Show>
        </Frame>
    }
}

/// Another device's request to pair, the host's question: the code that device shows, typed as
/// a secret and cleared, to approve it, which stays until the host no longer lists the request;
/// or Ignore, after which the root asks about it no more, as be-host's Add a device does.
#[component]
pub(super) fn PairingDialog(core: Signal<CoreView>, id: u64, done: Callback<Closed>) -> NodeId {
    let request = create_memo(move || {
        core.with(|view| {
            view.pairing
                .iter()
                .find(|request| request.id == id)
                .cloned()
        })
    });
    let lead = create_memo(move || {
        request.with(|request| {
            let device = request
                .as_ref()
                .map(|it| it.device.clone())
                .unwrap_or_default();
            format!(
                "A {device} device wants this account's key. If it is yours, type the code it shows."
            )
        })
    });
    let refused = create_memo(move || request.with(|request| request.as_ref()?.refused.clone()));
    let code = create_signal(String::new());
    let cannot = create_memo(move || code.with(|code| code.trim().is_empty()));
    let approve = move || {
        let typed = code.get_untracked();
        code.set(String::new());
        if !typed.trim().is_empty() {
            ask_core(CoreCommand::ApprovePairing(PairingApproval {
                id,
                code: Password::from(typed),
            }));
        }
    };
    view! {
        <Frame width=Length::FILL>
            <DialogBody
                kind=DialogKind::Alert
                title=PAIRING_TITLE
                actions={vec![
                    DialogAction::primary("Approve", approve).disabled(cannot),
                    DialogAction::cancel("Ignore"),
                ]}
                on_dismiss={move || done.call(Closed::Dismissed)}
            >
                <Text style=TextStyle::PARAGRAPH>{lead}</Text>
                <TextInput
                    value={code}
                    label="Code"
                    placeholder="XXXX-XXXX"
                    on_submit={move |_: String| approve()}
                />
                <be_root_accounts_ui::ErrorText text={refused} />
            </DialogBody>
        </Frame>
    }
}

/// The host's question of signing in again to an account whose sign-in ended: Continue in
/// browser, which starts the sign-in the dialog adding an account then shows; Log out, which
/// forgets the account; or closing it, which leaves the account for the accounts, as be-host's
/// own does.
#[component]
pub(super) fn ReauthDialog(core: Signal<CoreView>, done: Callback<Closed>) -> NodeId {
    let asked = create_memo(move || core.with(|view| view.reauthenticating.clone()));
    let lead = create_memo(move || {
        asked.with(|asked| {
            let name = asked
                .as_ref()
                .map(|asked| asked.name.as_str())
                .unwrap_or_default();
            format!(
                "Your session for {name} is no longer valid. Continue in your browser to sign in again."
            )
        })
    });
    let error = create_memo(move || asked.with(|asked| asked.as_ref()?.error.clone()));
    let busy = create_memo(move || asked.with(|asked| asked.as_ref().is_some_and(|it| it.busy)));
    let log_out = move || {
        if let Some(key) = asked.with_untracked(|asked| asked.as_ref().map(|it| it.key.clone())) {
            ask_core(CoreCommand::ForgetAccount(key));
        }
    };
    view! {
        <Frame width=Length::FILL>
            <DialogBody
                kind=DialogKind::Alert
                title=REAUTH_TITLE
                actions={vec![
                    DialogAction::secondary("Log out", log_out),
                    DialogAction::cancel("Not now"),
                    DialogAction::primary("Continue in browser", || ask_core(CoreCommand::Reauthenticate))
                        .busy(busy),
                ]}
                on_dismiss={move || done.call(Closed::Dismissed)}
            >
                <Text style=TextStyle::PARAGRAPH>{lead}</Text>
                <be_root_accounts_ui::ErrorText text={error} />
            </DialogBody>
        </Frame>
    }
}

/// The host's question before discarding the changes not saved to this device yet, for
/// `reason`: discarding them, or keeping them (Escape, or closing it).
#[component]
pub(super) fn DiscardDialog(reason: DiscardReason, done: Callback<Closed>) -> NodeId {
    let (message, button) = match reason {
        DiscardReason::SwitchOrganization => (
            "Switching organizations will discard changes that are not saved on this device yet. Changes waiting to sync are kept.",
            "Discard and switch",
        ),
        DiscardReason::Close => (
            "Closing be-host will discard changes that are not saved on this device yet. Changes waiting to sync are kept.",
            "Discard and close",
        ),
    };
    view! {
        <Frame width=Length::FILL>
            <DialogBody
                kind=DialogKind::Alert
                title=DISCARD_TITLE
                actions={vec![
                    DialogAction::destructive(button, move || done.call(Closed::Confirmed)),
                    DialogAction::cancel("Cancel"),
                ]}
                on_dismiss={move || done.call(Closed::Dismissed)}
            >
                <Text style=TextStyle::PARAGRAPH>{message}</Text>
            </DialogBody>
        </Frame>
    }
}

/// The organization's invitations: each with how it stands, its link again for one of the
/// person's own still open, and Revoke while open; and New link, which makes one. A link comes
/// back once, as the answer to the request that made it (`ask`, `make-secret`), and `link`
/// holds it only while it shows: Done, or closing the dialog, drops it, zeroed. While a request
/// is `pending` it asks for no other, so the answer it shows is the latest request's.
#[component]
pub(super) fn InvitationsDialog(
    core: Signal<CoreView>,
    link: Signal<Option<Held>>,
    pending: Signal<bool>,
    ask: Callback<SecretCommand>,
    done: Callback<Closed>,
) -> NodeId {
    let area = create_memo(move || core.with(|view| view.invitations.clone()));
    let rows = create_memo(move || area.with(|area| area.rows.clone()));
    let busy = create_memo(move || pending.get() || area.with(|area| area.busy || !area.listed));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let showing = create_memo(move || link.with(Option::is_some));
    let listing = create_memo(move || !showing.get());
    let joins = create_memo(move || {
        core.with(|view| {
            format!(
                "Anyone who opens this link with an account on this server can join {} as an editor, once, within seven days.",
                view.organization
            )
        })
    });
    // What it shows of the link, the one copy it keeps beside the link itself.
    let shown = create_memo(move || {
        link.with(|link| {
            link.as_ref()
                .and_then(Held::link)
                .unwrap_or_default()
                .to_owned()
        })
    });
    let copy = move || {
        if let Some(text) =
            link.with_untracked(|link| link.as_ref().and_then(Held::link).map(str::to_owned))
        {
            be_wasm_ops::guest::spawn(async move {
                let item = be_lib_ui_types::ClipItem::text(text);
                let _ = host::copy(vec![be_wasm_ops::encode(&item)]).await;
            });
        }
    };
    view! {
        <Frame width=Length::FILL>
            <List spacing=Space::None>
                <Show value={listing}>
                    <DialogBody
                        title=INVITATIONS_TITLE
                        actions={vec![
                            DialogAction::primary("New invite link", move || ask.call(SecretCommand::InviteMember))
                                .busy(busy)
                                .disabled(move || pending.get()),
                        ]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <Text style=TextStyle::CAPTION>
                            "An invite link joins whoever opens it as an editor, once, within seven days. Revoke one you no longer want used; remove someone who joined from Members."
                        </Text>
                        <ForEach
                            value={rows}
                            key={|row: &InvitationRow| row.id}
                            fallback={|| view! {
                                <Text style=TextStyle::PARAGRAPH>"No invitations yet."</Text>
                            }}
                        >|row: ReadSignal<InvitationRow>| {
                            let id = row.with_untracked(|row| row.id);
                            let title = create_memo(move || row.with(invitation_title));
                            let open = create_memo(move || {
                                row.with(|row| matches!(row.state, InvitationState::Waiting | InvitationState::Opened))
                            });
                            let has_link = create_memo(move || row.with(|row| row.has_link));
                            view! {
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Sm
                                >
                                    <ListItem grow=1.0>
                                        <Text style=TextStyle::CAPTION>{title}</Text>
                                    </ListItem>
                                    <Show value={has_link}>
                                        <Button
                                            label="Show link"
                                            variant=ButtonVariant::Secondary
                                            disabled={pending}
                                            on_click={move || ask.call(SecretCommand::InvitationLink(id))}
                                        />
                                    </Show>
                                    <Show value={open}>
                                        <Button
                                            label="Revoke"
                                            variant=ButtonVariant::Secondary
                                            on_click={move || ask_core(CoreCommand::RevokeInvitation(id))}
                                        />
                                    </Show>
                                </List>
                            }
                        }</ForEach>
                        <be_root_accounts_ui::ErrorText text={error} />
                    </DialogBody>
                </Show>
                <Show value={showing}>
                    <DialogBody
                        title="Invite member"
                        actions={vec![
                            DialogAction::primary("Copy link", copy),
                            DialogAction::secondary("Done", move || link.set(None)),
                        ]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <Text style=TextStyle::PARAGRAPH>{joins}</Text>
                        <Text style=TextStyle::CAPTION>
                            "Nothing was sent: send this link yourself, through an app you trust. You can copy it again from Invitations."
                        </Text>
                        <TextInput
                            value={Binding::new(shown, Callback::empty())}
                            label="Invite link"
                        />
                    </DialogBody>
                </Show>
            </List>
        </Frame>
    }
}

/// The organization's members: each with their email and role, and, but for the person
/// themselves and agents, a change of role (administrator or editor) and Remove, which asks
/// first in the list's place. A member who leaves the list takes the question with them.
#[component]
pub(super) fn MembersDialog(core: Signal<CoreView>, done: Callback<Closed>) -> NodeId {
    let area = create_memo(move || core.with(|view| view.members.clone()));
    let members = create_memo(move || area.with(|area| area.members.clone()));
    let busy = create_memo(move || area.with(|area| area.busy || !area.listed));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let removing = create_signal(None::<[u8; 16]>);
    let asked = create_memo(move || {
        let account = removing.get()?;
        members.with(|members| {
            members
                .iter()
                .find(|member| member.account == account)
                .cloned()
        })
    });
    let asks = create_memo(move || asked.with(Option::is_some));
    let listing = create_memo(move || !asks.get());
    let question = create_memo(move || {
        let name = asked.with(|member| {
            member
                .as_ref()
                .map(|member| member.name.clone())
                .unwrap_or_default()
        });
        format!(
            "Remove {name} from this organization? They lose everything shared with them here, and what they could open is given new keys."
        )
    });
    let remove = move || {
        if let Some(member) = asked.get_untracked() {
            ask_core(CoreCommand::RemoveMember(member.account));
        }
        removing.set(None);
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
                match asks.get_untracked() {
                    true => removing.set(None),
                    false => done.call(Closed::Dismissed),
                }
            }
            closes
        },
    );
    view! {
        <Frame width=Length::FILL>
            <List spacing=Space::None>
                <Show value={listing}>
                    <DialogBody
                        title=MEMBERS_TITLE
                        actions={vec![DialogAction::cancel("Close")]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <Text style=TextStyle::CAPTION>
                            "Administrators can open every block in this organization. Removing someone takes away what was shared with them here, and gives new keys to everything they could open."
                        </Text>
                        <be_root_accounts_ui::ErrorText text={error} />
                        <ForEach
                            value={members}
                            key={|member: &MemberRow| member.account}
                        >|member: ReadSignal<MemberRow>| {
                            let account = member.with_untracked(|member| member.account);
                            let name = create_memo(move || member.with(|member| member.name.clone()));
                            let detail = create_memo(move || member.with(member_detail));
                            let fixed = create_memo(move || member.with(|member| member.me || member.agent) || busy.get());
                            let unsigned = create_memo(move || member.with(|member| member.unsigned));
                            let promote = create_memo(move || member.with(|member| member.role != MemberRole::Administrator));
                            let role_label = create_memo(move || match promote.get() {
                                true => "Make administrator".to_owned(),
                                false => "Make editor".to_owned(),
                            });
                            let change = move || {
                                let role = match promote.get_untracked() {
                                    true => MemberRole::Administrator,
                                    false => MemberRole::Editor,
                                };
                                ask_core(CoreCommand::SetMemberRole(RoleChange { account, role }));
                            };
                            view! {
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Sm
                                >
                                    <ListItem grow=1.0>
                                        <List spacing=Space::Xs2>
                                            <Text>{name}</Text>
                                            <Text style=TextStyle::CAPTION>{detail}</Text>
                                            <Show value={unsigned}>
                                                <Text style={TextStyle::CAPTION.color(TextColor::Warning)}>
                                                    "No invitation signed them in: the server added them. Remove them unless you know why they are here."
                                                </Text>
                                            </Show>
                                        </List>
                                    </ListItem>
                                    <Button
                                        label={role_label}
                                        variant=ButtonVariant::Secondary
                                        disabled={fixed}
                                        on_click={change}
                                    />
                                    <Button
                                        icon_only=true
                                        glyph=ICON_USER_MINUS
                                        label="Remove member"
                                        disabled={fixed}
                                        on_click={move || removing.set(Some(account))}
                                    />
                                </List>
                            }
                        }</ForEach>
                    </DialogBody>
                </Show>
                <Show value={asks}>
                    <DialogBody
                        kind=DialogKind::Alert
                        title=REMOVE_MEMBER_TITLE
                        actions={vec![
                            DialogAction::destructive("Remove", remove),
                            DialogAction::cancel("Keep"),
                        ]}
                        on_dismiss={move || removing.set(None)}
                    >
                        <Text style=TextStyle::PARAGRAPH>{question}</Text>
                    </DialogBody>
                </Show>
            </List>
        </Frame>
    }
}

/// An agent's credentials as be-host's own dialog copies them: the JSON a program reads from
/// BE3_AGENT_CREDENTIALS.
pub(super) fn agent_bundle(agent: &be_wasm_root_api::AgentCredentials) -> String {
    let quoted = |text: &str| {
        let mut quoted = String::from('"');
        for character in text.chars() {
            match character {
                '"' => quoted.push_str("\\\""),
                '\\' => quoted.push_str("\\\\"),
                character if character < ' ' => {
                    quoted.push_str(&format!("\\u{:04x}", u32::from(character)))
                }
                character => quoted.push(character),
            }
        }
        quoted.push('"');
        quoted
    };
    let id = agent.organization;
    let hex: String = id.iter().map(|byte| format!("{byte:02x}")).collect();
    let organization = format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    );
    format!(
        "{{\"organization\":{},\"recovery_phrase\":{},\"server\":{},\"token\":{}}}",
        quoted(&organization),
        quoted(agent.phrase.as_str().unwrap_or_default()),
        quoted(&agent.server),
        quoted(agent.token.as_str().unwrap_or_default())
    )
}

/// The host's question before leaving a block an editor asked to leave: Leave, which asks the
/// host to and stays, busy, until the host no longer asks (it left, or why it could not shows),
/// or Stay (Escape, or closing it).
#[component]
pub(super) fn LeaveDialog(core: Signal<CoreView>, done: Callback<Closed>) -> NodeId {
    let asked = create_memo(move || core.with(|view| view.leaving.clone()));
    // be-host's wording: the block's name and the folder it is in quoted, as untrusted text.
    let title = create_memo(move || {
        asked.with(|asked| match asked {
            Some(leaving) => Phrase::new()
                .text("Leave ")
                .quoted(be_lib_quoted_name::quotable_name(&leaving.name))
                .text("?"),
            None => Phrase::new(),
        })
    });
    let question = create_memo(move || {
        let within = asked.with(|asked| asked.as_ref().and_then(|it| it.within.clone()));
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
    let busy = create_memo(move || asked.with(|asked| asked.as_ref().is_some_and(|it| it.busy)));
    let refused = create_memo(move || asked.with(|asked| asked.as_ref()?.refused.clone()));
    let leave = move || {
        if let Some(block) = asked.with_untracked(|asked| asked.as_ref().map(|it| it.block)) {
            ask_core(CoreCommand::ConfirmLeave(block));
        }
    };
    view! {
        <Frame width=Length::FILL>
            <DialogBody
                kind=DialogKind::Alert
                title={title}
                actions={vec![
                    DialogAction::destructive("Leave", leave).busy(busy),
                    DialogAction::cancel("Stay"),
                ]}
                on_dismiss={move || done.call(Closed::Dismissed)}
            >
                <Text style=TextStyle::PARAGRAPH>{question}</Text>
                <be_root_accounts_ui::ErrorText text={refused} />
            </DialogBody>
        </Frame>
    }
}

/// A line saying how something went, while there is one: unlike an error, in the text's own
/// colour.
#[component]
pub(super) fn Note(text: Memo<Option<String>>) -> NodeId {
    view! {
        <List spacing=Space::None>
            <Show value={text}>|text: ReadSignal<String>|
                <Text style=TextStyle::PARAGRAPH>{text}</Text>
            </Show>
        </List>
    }
}

/// Who a card names, as be-host words it: their name and email, or what they gave.
pub(super) fn card_named(card: &ContactCard) -> String {
    match (card.name.trim().is_empty(), card.email.is_empty()) {
        (true, _) => card.email.clone(),
        (false, true) => card.name.clone(),
        (false, false) => format!("{} ({})", card.name, card.email),
    }
}

/// The person's contacts: each with Remove, which asks first in the list's place; a contact
/// link to make, which comes back once as `make-secret`'s answer and shows with Copy and Done;
/// and a pasted contact link to open, handed over as a secret and cleared, then who it is from,
/// to connect with or not. While a contact link is being made it stays open, as the agents do.
#[component]
pub(super) fn ContactsDialog(
    core: Signal<CoreView>,
    link: Signal<Option<Held>>,
    pending: Signal<bool>,
    ask: Callback<SecretCommand>,
    done: Callback<Closed>,
) -> NodeId {
    let area = create_memo(move || core.with(|view| view.contacts.clone()));
    let contacts = create_memo(move || area.with(|area| area.contacts.clone()));
    let busy = create_memo(move || pending.get() || area.with(|area| area.busy));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let offer = create_memo(move || {
        area.with(|area| {
            area.offered
                .as_ref()
                .map(|card| format!("Connect with {}?", card_named(card)))
        })
    });
    let offered = create_memo(move || offer.with(Option::is_some));
    let offer = create_memo(move || offer.get().unwrap_or_default());
    let connected = create_memo(move || {
        area.with(|area| {
            area.connected
                .as_ref()
                .map(|card| format!("You and {} are now contacts.", card_named(card)))
        })
    });
    let showing =
        create_memo(move || link.with(|link| link.as_ref().and_then(Held::link).is_some()));
    let removing = create_signal(None::<[u8; 16]>);
    let asked = create_memo(move || {
        let account = removing.get()?;
        contacts.with(|contacts| contacts.iter().find(|it| it.account == account).cloned())
    });
    let asks = create_memo(move || asked.with(Option::is_some));
    let listing = create_memo(move || !showing.get() && !asks.get());
    let question = create_memo(move || {
        asked.with(|asked| {
            let name = asked
                .as_ref()
                .map(|contact| {
                    card_named(&ContactCard {
                        name: contact.name.clone(),
                        email: contact.email.clone(),
                    })
                })
                .unwrap_or_default();
            format!("Remove {name}? Whatever you and they have shared with each other is unshared.")
        })
    });
    let shown = create_memo(move || {
        link.with(|link| {
            link.as_ref()
                .and_then(Held::link)
                .unwrap_or_default()
                .to_owned()
        })
    });
    let copy = move || {
        let text = shown.get_untracked();
        if !text.is_empty() {
            be_wasm_ops::guest::spawn(async move {
                let item = be_lib_ui_types::ClipItem::text(text);
                let _ = host::copy(vec![be_wasm_ops::encode(&item)]).await;
            });
        }
    };
    let pasted = create_signal(String::new());
    let open_link = move || {
        // The link is a bearer secret: shown as typed, it leaves the field as it is handed over.
        let typed = pasted.get_untracked();
        pasted.set(String::new());
        if !typed.trim().is_empty() {
            ask_core(CoreCommand::OpenContactLink(Password::from(typed)));
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
            // While a contact link is being made it stays open, so the link reaches the
            // person rather than live and unheld.
            if closes && !pending.get_untracked() {
                match asks.get_untracked() {
                    true => removing.set(None),
                    false => done.call(Closed::Dismissed),
                }
            }
            closes
        },
    );
    view! {
        <Frame width=Length::FILL>
            <List spacing=Space::None>
                <Show value={listing}>
                    <DialogBody
                        title=CONTACTS_TITLE
                        actions={vec![
                            DialogAction::cancel("Close").disabled(move || pending.get()),
                            DialogAction::primary("New contact link", move || ask.call(SecretCommand::ContactLink))
                                .busy(busy)
                                .disabled(move || pending.get()),
                        ]}
                        on_dismiss={move || if !pending.get_untracked() { done.call(Closed::Dismissed) }}
                    >
                        <Text style=TextStyle::CAPTION>
                            "You can share only with your contacts. Someone becomes one when you send them a contact link yourself, through an app you trust, and they open it, or when they send you theirs."
                        </Text>
                        <be_root_accounts_ui::ErrorText text={error} />
                        <Note text={connected} />
                        <Show value={offered}>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Sm
                            >
                                <ListItem grow=1.0>
                                    <Text>{offer}</Text>
                                </ListItem>
                                <Button
                                    label="Cancel"
                                    variant=ButtonVariant::Secondary
                                    on_click={|| ask_core(CoreCommand::ForgetContactLink)}
                                />
                                <Button
                                    label="Connect"
                                    variant=ButtonVariant::Primary
                                    disabled={busy}
                                    on_click={|| ask_core(CoreCommand::AcceptContact)}
                                />
                            </List>
                        </Show>
                        <ForEach
                            value={contacts}
                            key={|contact: &ContactRow| contact.account}
                            fallback={|| view! {
                                <Text style=TextStyle::CAPTION>"No contacts yet."</Text>
                            }}
                        >|contact: ReadSignal<ContactRow>| {
                            let account = contact.with_untracked(|contact| contact.account);
                            let name = create_memo(move || contact.with(|it| card_named(&ContactCard { name: it.name.clone(), email: it.email.clone() })));
                            let detail = create_memo(move || contact.with(contact_detail));
                            view! {
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Sm
                                >
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
                                        on_click={move || removing.set(Some(account))}
                                    />
                                </List>
                            }
                        }</ForEach>
                        <Text style=TextStyle::CAPTION>"Open a contact link someone sent you"</Text>
                        <List direction=Direction::Horizontal align=Align::End spacing=Space::Md>
                            <ListItem grow=1.0>
                                <TextInput
                                    value={pasted}
                                    label="Contact link"
                                    on_submit={move |_: String| open_link()}
                                />
                            </ListItem>
                            <Button
                                label="Open"
                                variant=ButtonVariant::Secondary
                                disabled={busy}
                                on_click={open_link}
                            />
                        </List>
                    </DialogBody>
                </Show>
                <Show value={showing}>
                    <DialogBody
                        title="Contact link"
                        actions={vec![
                            DialogAction::primary("Copy link", copy),
                            DialogAction::secondary("Done", move || link.set(None)),
                        ]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <Text style=TextStyle::PARAGRAPH>
                            "Send this link yourself, through an app you trust, to the one person you want to connect with. It works once, and it is shown only once."
                        </Text>
                        <TextInput
                            value={Binding::new(shown, Callback::empty())}
                            label="Contact link"
                        />
                    </DialogBody>
                </Show>
                <Show value={asks}>
                    <DialogBody
                        kind=DialogKind::Alert
                        title=REMOVE_CONTACT_TITLE
                        actions={vec![
                            DialogAction::destructive("Remove", move || {
                                if let Some(contact) = asked.get_untracked() {
                                    ask_core(CoreCommand::RemoveContact(contact.account));
                                }
                                removing.set(None);
                            }),
                            DialogAction::cancel("Keep"),
                        ]}
                        on_dismiss={move || removing.set(None)}
                    >
                        <Text style=TextStyle::PARAGRAPH>{question}</Text>
                    </DialogBody>
                </Show>
            </List>
        </Frame>
    }
}

/// Whether a contact's key is held, and how the server's record of it differed, in words.
pub(super) fn contact_detail(contact: &ContactRow) -> String {
    match (&contact.mismatch, contact.verified) {
        (Some(ContactMismatch::Older), _) => {
            "The server served an older record of this contact: this device keeps its own."
                .to_owned()
        }
        (Some(ContactMismatch::Unreadable), _) => {
            "The server's record of this contact can't be read here: this device keeps its own."
                .to_owned()
        }
        (None, true) => "Verified".to_owned(),
        (None, false) => "Not verified yet".to_owned(),
    }
}

/// What a share link opens, as be-host's own dialog asks it: the block, where, what it lets
/// the person do, and whether they join its organization as a guest.
pub(super) fn share_offered(preview: &SharePreview) -> String {
    let can = match preview.access {
        ShareAccess::Edit => "view and edit it",
        ShareAccess::Join => "join it while someone hosts it",
        ShareAccess::View => "view it",
    };
    let named = share_named(preview);
    let organization = share_organization(preview);
    let guest = match preview.guest {
        true => format!(
            " You will join {organization} as a guest: you see only what links share with you there."
        ),
        false => String::new(),
    };
    format!("{named}, in {organization}, is shared with you: you can {can}.{guest}")
}

/// A shared block's name as one line, cut short, in quotes; its maker's words.
pub(super) fn share_named(preview: &SharePreview) -> String {
    let name: String = preview
        .name
        .split(|character: char| character.is_whitespace() || character.is_control())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    match name.chars().count() {
        0 => "The block".to_owned(),
        count if count > 60 => format!("“{}…”", name.chars().take(60).collect::<String>()),
        _ => format!("“{name}”"),
    }
}

pub(super) fn share_organization(preview: &SharePreview) -> String {
    match preview.organization.trim().is_empty() {
        true => "its organization".to_owned(),
        false => preview.organization.clone(),
    }
}

/// Opening a share link someone sent: the pasted link, handed over as a secret and cleared;
/// then what it shares, to open or not; then what it opened.
#[component]
pub(super) fn ShareLinkDialog(core: Signal<CoreView>, done: Callback<Closed>) -> NodeId {
    let area = create_memo(move || core.with(|view| view.share_links.clone()));
    let busy = create_memo(move || area.with(|area| area.busy));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let offer = create_memo(move || area.with(|area| area.offered.as_ref().map(share_offered)));
    let offered = create_memo(move || offer.with(Option::is_some));
    let opened = create_memo(move || {
        area.with(|area| {
            area.opened.as_ref().map(|preview| {
                format!(
                    "{} is open to you: you find it under Shared with me in {}.",
                    share_named(preview),
                    share_organization(preview)
                )
            })
        })
    });
    let pasting = create_memo(move || !offered.get());
    let offer = create_memo(move || offer.get().unwrap_or_default());
    let pasted = create_signal(String::new());
    let open_link = move || {
        let typed = pasted.get_untracked();
        pasted.set(String::new());
        if !typed.trim().is_empty() {
            ask_core(CoreCommand::OpenShareLink(Password::from(typed)));
        }
    };
    view! {
        <Frame width=Length::FILL>
            <List spacing=Space::None>
                <Show value={pasting}>
                    <DialogBody
                        title=SHARE_LINK_TITLE
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <Text style=TextStyle::CAPTION>
                            "Someone shared a block with you by a link. Opening it gives your account what the link grants, and makes no one your contact."
                        </Text>
                        <List direction=Direction::Horizontal align=Align::End spacing=Space::Md>
                            <ListItem grow=1.0>
                                <TextInput
                                    value={pasted}
                                    label="Share link"
                                    focused=true
                                    on_submit={move |_: String| open_link()}
                                />
                            </ListItem>
                            <Button
                                label="Open"
                                variant=ButtonVariant::Secondary
                                disabled={busy}
                                on_click={open_link}
                            />
                        </List>
                        <Note text={opened} />
                        <be_root_accounts_ui::ErrorText text={error} />
                    </DialogBody>
                </Show>
                <Show value={offered}>
                    <DialogBody
                        kind=DialogKind::Alert
                        title="Open this block?"
                        actions={vec![
                            DialogAction::primary("Open", || ask_core(CoreCommand::AcceptShareLink))
                                .busy(busy),
                            DialogAction::secondary("Cancel", || ask_core(CoreCommand::ForgetShareLink)),
                        ]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <Text style=TextStyle::PARAGRAPH>{offer}</Text>
                    </DialogBody>
                </Show>
            </List>
        </Frame>
    }
}

/// A member's email and role, in words.
pub(super) fn member_detail(member: &MemberRow) -> String {
    let role = match &member.role {
        MemberRole::Administrator => "Administrator",
        MemberRole::Editor => "Editor",
        MemberRole::Guest => "Guest",
    };
    match (member.agent, member.email.is_empty()) {
        (true, _) => format!("{}, agent", member.email),
        (false, true) => role.to_owned(),
        (false, false) => format!("{}, {role}", member.email),
    }
}

/// The organization's agents: each with Remove, and a name to make one by, whose credentials
/// come back once, as the answer to the request that made it (`ask`, `make-secret`): shown in
/// the list's place with Copy and Done, held only while they show.
#[component]
pub(super) fn AgentsDialog(
    core: Signal<CoreView>,
    made: Signal<Option<Held>>,
    pending: Signal<bool>,
    ask: Callback<SecretCommand>,
    done: Callback<Closed>,
) -> NodeId {
    let area = create_memo(move || core.with(|view| view.agents.clone()));
    let agents = create_memo(move || area.with(|area| area.agents.clone()));
    let busy = create_memo(move || pending.get() || area.with(|area| area.busy || !area.listed));
    let error = create_memo(move || area.with(|area| area.error.clone()));
    let showing =
        create_memo(move || made.with(|made| made.as_ref().and_then(Held::agent).is_some()));
    let listing = create_memo(move || !showing.get());
    let name = create_signal(String::new());
    let create = move || {
        let named = name.get_untracked().trim().to_owned();
        if !named.is_empty() && !pending.get_untracked() {
            name.set(String::new());
            ask.call(SecretCommand::CreateAgent(named));
        }
    };
    // What it shows and copies of the credentials, as be-host's own dialog does: the bundle a
    // program reads from BE3_AGENT_CREDENTIALS, the one copy it keeps beside them.
    let heading = create_memo(move || {
        made.with(|made| {
            let name = made.as_ref().and_then(Held::agent).map(|agent| agent.name.clone());
            format!(
                "Copy these secret credentials for {}. They are shown only once. Set BE3_AGENT_CREDENTIALS to this value, then share a workspace or blocks with the agent.",
                name.unwrap_or_default()
            )
        })
    });
    let credentials = create_memo(move || {
        made.with(|made| {
            made.as_ref()
                .and_then(Held::agent)
                .map(agent_bundle)
                .unwrap_or_default()
        })
    });
    let copy = move || {
        let text = credentials.get_untracked();
        if !text.is_empty() {
            be_wasm_ops::guest::spawn(async move {
                let item = be_lib_ui_types::ClipItem::text(text);
                let _ = host::copy(vec![be_wasm_ops::encode(&item)]).await;
            });
        }
    };
    // While an agent is being made it stays open, so its credentials, which cannot be had
    // again, reach the person: Close and its close button wait for the answer, and so does the
    // root for Escape and the scrim (`Dialog::holds_while_pending`).
    let organization = create_memo(move || core.with(|view| format!("Organization: {}", view.organization)));
    let dismiss = move || {
        if !pending.get_untracked() {
            done.call(Closed::Dismissed);
        }
    };
    view! {
        <Frame width=Length::FILL>
            <List spacing=Space::None>
                <Show value={listing}>
                    <DialogBody
                        title=AGENTS_TITLE
                        actions={vec![
                            DialogAction::cancel("Close").disabled(move || pending.get()),
                            DialogAction::primary("Add agent", create)
                                .busy(busy)
                                .disabled(move || pending.get() || name.with(|name| name.trim().is_empty())),
                        ]}
                        on_dismiss={dismiss}
                    >
                        <Text style=TextStyle::CAPTION>{organization}</Text>
                        <Text style=TextStyle::CAPTION>
                            "An agent is an account for a program, such as a coding agent. It sees only the blocks you share with it. Share a workspace or blocks with it from this device after setup. This list shows the agents you created."
                        </Text>
                        <ForEach
                            value={agents}
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
                                    spacing=Space::Sm
                                >
                                    <ListItem grow=1.0>
                                        <Text overflow=Overflow::Ellipsis>{label}</Text>
                                    </ListItem>
                                    <Button
                                        icon_only=true
                                        glyph=ICON_TRASH
                                        label="Remove agent"
                                        disabled={busy}
                                        on_click={move || ask_core(CoreCommand::RemoveAgent(account))}
                                    />
                                </List>
                            }
                        }</ForEach>
                        <Text style=TextStyle::CAPTION>"Name"</Text>
                        <TextInput
                            value={name}
                            label="Agent name"
                            focused=true
                            on_submit={move |_: String| create()}
                        />
                        <be_root_accounts_ui::ErrorText text={error} />
                    </DialogBody>
                </Show>
                <Show value={showing}>
                    <DialogBody
                        title="Agent credentials"
                        actions={vec![
                            DialogAction::primary("Copy", copy),
                            DialogAction::secondary("Done", move || made.set(None)),
                        ]}
                        on_dismiss={move || done.call(Closed::Dismissed)}
                    >
                        <Text style=TextStyle::PARAGRAPH>{heading}</Text>
                        <TextInput
                            value={Binding::new(credentials, Callback::empty())}
                            label="Agent credentials"
                        />
                    </DialogBody>
                </Show>
            </List>
        </Frame>
    }
}

/// How an invitation stands, who made it and when, in words.
pub(super) fn invitation_title(row: &InvitationRow) -> String {
    let state = match row.state {
        InvitationState::Waiting => "Waiting",
        InvitationState::Opened => "Opened",
        InvitationState::Accepted => "Accepted",
        InvitationState::Expired => "Expired",
        InvitationState::Revoked => "Revoked",
    };
    let made = format!("{state} · made by {} on {}", row.inviter, date(row.created));
    match (&row.state, &row.accepter, row.ended) {
        (InvitationState::Accepted, Some(by), Some(on)) => {
            format!("{made} · by {by} on {}", date(on))
        }
        (InvitationState::Revoked, _, Some(on)) => format!("{made} · revoked {}", date(on)),
        _ => format!("{made} · expires {}", date(row.expires)),
    }
}

/// A day in UTC, from milliseconds since the epoch, as `2026-11-15`.
pub(super) fn date(milliseconds: i64) -> String {
    let days = milliseconds.div_euclid(86_400_000);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted + 2) / 5 + 1;
    let month = if shifted < 10 {
        shifted + 3
    } else {
        shifted - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year}-{month:02}-{day:02}")
}

/// An unsent block's name, or what the root calls one with none.
pub(super) fn edit_name(edit: &UnsentEdit) -> String {
    edit.name.clone().unwrap_or_else(|| "Untitled".to_owned())
}

/// Why a block's edits went unsent, and how many never became a commit, in words.
pub(super) fn edit_detail(edit: &UnsentEdit) -> String {
    let why = match (&edit.aside, edit.offerable) {
        (Some(SetAside::Refused(refusal)), _) => {
            let why = match refusal {
                Refusal::OverQuota => "it would go over a limit",
                Refusal::Invalid => "it could not take it as it is",
                Refusal::Policy => "your organization does not allow it",
                Refusal::NotPermitted => "you may not make it there",
                Refusal::Unexplained => "it would not say why",
            };
            format!("The server refused it: {why}")
        }
        (Some(SetAside::KeyOutdated), _) => {
            "It was locked offline with a key someone who lost access may hold: discard it to go on editing".to_owned()
        }
        (Some(SetAside::Misplaced), _) => {
            "It was locked offline under a folder it is not really in: discard it to go on editing".to_owned()
        }
        (None, true) => "You can edit it again".to_owned(),
        (None, false) => "You cannot edit it now".to_owned(),
    };
    match edit.kept_edits {
        0 => why,
        1 => format!("{why}. 1 unsaved edit"),
        kept => format!("{why}. {kept} unsaved edits"),
    }
}

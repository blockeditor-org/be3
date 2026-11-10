use std::collections::HashMap;

use be_sdk_beui::be_block::ObjectId;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::accesskit::{Node, Role};
use be_sdk_beui::beui::datetime::{DateTime, HourCycle};
use be_sdk_beui::beui::icons::{
    Glyph, ICON_CHECK, ICON_EYE, ICON_HEART, ICON_PARTY_POPPER, ICON_PENCIL, ICON_REPLY, ICON_SEND,
    ICON_STICKER, ICON_THUMBS_UP, ICON_TRASH, ICON_X,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{ContentProjection, Editor, Signer, utc_offset};
use be_type_chat::{ChatContent, Message, MessageEdit, Shown, Signature};
use beui::{Align, Direction};
use uuid::Uuid;

type Chat = ContentProjection<ChatContent>;

/// The reactions offered, each a Lucide icon by name (what a reaction stores) and what it
/// says.
pub const REACTIONS: [(&str, Glyph, &str); 5] = [
    ("thumbs-up", ICON_THUMBS_UP, "Thumbs up"),
    ("heart", ICON_HEART, "Heart"),
    ("party-popper", ICON_PARTY_POPPER, "Celebrate"),
    ("eye", ICON_EYE, "Seen"),
    ("check", ICON_CHECK, "Done"),
];

/// What a message row shows, read from the content in one pass per change, so a row
/// updates only when what it shows changed.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    author: String,
    mine: bool,
    time: String,
    body: String,
    edited: bool,
    reply: Option<String>,
    reactions: Vec<Reacted>,
    /// What its author signed: the message, and its edit if it has one.
    proofs: Vec<Proof>,
}

/// One record a member signed (plans/authorship.md), to ask the host who signed it.
#[derive(Clone, Debug, PartialEq)]
struct Proof {
    author: Uuid,
    bytes: Vec<u8>,
    signature: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
struct Reacted {
    reaction: String,
    count: usize,
    mine: bool,
}

/// What the person is doing in the editor: none of it is in the block until it is sent.
#[derive(Clone, Copy)]
struct Composer {
    draft: Signal<String>,
    replying: Signal<Option<ObjectId>>,
    editing: Signal<Option<ObjectId>>,
    picking: Signal<Option<ObjectId>>,
}

#[component]
pub fn Chat(editor: Editor) -> NodeId {
    let chat: Chat = editor.block_content::<ChatContent>();
    let me = editor.blocks().account_id();
    let block = editor.block_id();
    let rows = chat.project_keyed(move |content: &ChatContent, rows| {
        let shown = content.shown();
        let by_id: HashMap<ObjectId, &Shown> =
            shown.iter().map(|message| (message.id, message)).collect();
        let today = local_date(now());
        let rows_now: Vec<(ObjectId, Row)> = shown
            .iter()
            .map(|message| {
                let row = Row {
                    proofs: proofs(content, block, message),
                    ..row(&by_id, message, me, today)
                };
                (message.id, row)
            })
            .collect();
        rows.reconcile_owned(rows_now);
    });
    let ids = rows.keys();
    let empty = create_memo(move || ids.with(Vec::is_empty));
    let composer = Composer {
        draft: create_signal(String::new()),
        replying: create_signal(None),
        editing: create_signal(None),
        picking: create_signal(None),
    };
    // A message replied to or being edited can be deleted, here or elsewhere.
    create_effect(move || {
        ids.with(|ids| {
            if composer
                .replying
                .get_untracked()
                .is_some_and(|id| !ids.contains(&id))
            {
                composer.replying.set(None);
            }
            if composer
                .editing
                .get_untracked()
                .is_some_and(|id| !ids.contains(&id))
            {
                composer.editing.set(None);
                composer.draft.set(String::new());
            }
            if composer
                .picking
                .get_untracked()
                .is_some_and(|id| !ids.contains(&id))
            {
                composer.picking.set(None);
            }
        });
    });
    let send = move || submit(editor, &chat, block, me, composer);
    let replying = create_memo(move || {
        let id = composer.replying.get()?;
        let quoted = rows.try_get(&id)?;
        Some(quoted.with(|row| format!("Replying to {}: {}", row.author, excerpt(&row.body))))
    });
    let reply_label = create_memo(move || replying.get().unwrap_or_default());
    let has_reply = create_memo(move || replying.get().is_some());
    let editing = create_memo(move || composer.editing.get().is_some());
    let send_label = create_memo(move || match editing.get() {
        true => "Save".to_owned(),
        false => "Send".to_owned(),
    });

    let message_rows = move |id: ObjectId| {
        let shown = rows.get(&id);
        view! {
            <MessageRow editor chat={chat} me id row={shown} composer />
        }
    };

    view! {
        <List spacing=Space::None>
            <ListItem grow=1.0>
                <Scroll follow_end=true>
                    <Frame padding={Padding::all(Space::Xl)}>
                        <List spacing=Space::Md>
                            <Show condition={empty}>
                                <Text style=TextStyle::CAPTION>
                                    "No messages yet. Say hello below."
                                </Text>
                            </Show>
                            <ForEach keys={ids} view={message_rows} />
                        </List>
                    </Frame>
                </Scroll>
            </ListItem>
            <Separator />
            <Frame padding={Padding::all(Space::Lg)}>
                <List spacing=Space::Sm>
                    <Show condition={has_reply}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                            <ListItem grow=1.0>
                                <Text style=TextStyle::CAPTION>{reply_label}</Text>
                            </ListItem>
                            <Button
                                label="Cancel the reply"
                                glyph=ICON_X
                                icon_only=true
                                variant=ButtonVariant::Ghost
                                size=ButtonSize::Small
                                @test_id={"chat.reply.cancel"}
                                on_click={move || composer.replying.set(None)}
                            />
                        </List>
                    </Show>
                    <Show condition={editing}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                            <ListItem grow=1.0>
                                <Text style=TextStyle::CAPTION>"Editing a message"</Text>
                            </ListItem>
                            <Button
                                label="Cancel the edit"
                                glyph=ICON_X
                                icon_only=true
                                variant=ButtonVariant::Ghost
                                size=ButtonSize::Small
                                @test_id={"chat.edit.cancel"}
                                on_click={move || {
                                    composer.editing.set(None);
                                    composer.draft.set(String::new());
                                }}
                            />
                        </List>
                    </Show>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <ListItem grow=1.0>
                            <TextInput
                                value={composer.draft}
                                placeholder="Message"
                                label="Message"
                                @test_id={"chat.draft"}
                                on_submit={move |_| send()}
                            />
                        </ListItem>
                        <Button
                            label={send_label}
                            glyph=ICON_SEND
                            variant=ButtonVariant::Primary
                            @test_id={"chat.send"}
                            on_click={move || send()}
                        />
                    </List>
                </List>
            </Frame>
        </List>
    }
}

fn row(shown: &HashMap<ObjectId, &Shown>, message: &Shown, me: Uuid, today: i64) -> Row {
    let reply = message
        .message
        .reply_to
        .map(|quoted| match shown.get(&quoted) {
            Some(quoted) => format!(
                "{}: {}",
                author(quoted.message.author, me),
                excerpt(&quoted.body)
            ),
            None => "A deleted message".to_owned(),
        });
    let reactions = message
        .reactions
        .iter()
        .map(|(reaction, authors)| Reacted {
            reaction: reaction.clone(),
            count: authors.len(),
            mine: authors.contains(&me),
        })
        .collect();
    Row {
        author: author(message.message.author, me),
        mine: message.message.author == me,
        proofs: Vec::new(),
        time: time(message.message.time, today),
        body: message.body.clone(),
        edited: message.edited,
        reply,
        reactions,
    }
}

/// No host service names members yet, so others show by their account's first digits.
fn author(account: Uuid, me: Uuid) -> String {
    match account == me {
        true => "You".to_owned(),
        false => format!("Member {}", &account.simple().to_string()[..4]),
    }
}

const SECONDS_PER_DAY: i64 = 86_400;

/// Seconds since the epoch in the device's time zone.
fn local_seconds(millis: i64) -> i64 {
    millis.div_euclid(1000) + i64::from(utc_offset())
}

/// The local day `millis` falls on, counted from the epoch.
fn local_date(millis: i64) -> i64 {
    local_seconds(millis).div_euclid(SECONDS_PER_DAY)
}

/// When a message was written in local time, with its date unless that was today.
fn time(millis: i64, today: i64) -> String {
    let at = DateTime::from_unix(local_seconds(millis));
    let clock = at.time.format(HourCycle::H24);
    match local_date(millis) == today {
        true => clock,
        false => format!(
            "{} {} {} {clock}",
            at.date.day,
            &at.date.month_name()[..3],
            at.date.year
        ),
    }
}

fn excerpt(body: &str) -> String {
    const LONGEST: usize = 48;
    let line = body.lines().next().unwrap_or_default();
    match line.char_indices().nth(LONGEST) {
        Some((end, _)) => format!("{}…", &line[..end]),
        None => line.to_owned(),
    }
}

fn now() -> i64 {
    i64::try_from(wall_clock().as_millis()).unwrap_or(i64::MAX)
}

/// The message and its edit as their author signed them, for each row to verify.
fn proofs(content: &ChatContent, chat: Uuid, message: &Shown) -> Vec<Proof> {
    let sent = &message.message;
    let mut proofs = vec![Proof {
        author: sent.author,
        bytes: sent.signed_bytes(chat, message.id),
        signature: sent.signature.0.clone(),
    }];
    if let Some(edit) = content.edit_of(message.id) {
        proofs.push(Proof {
            author: edit.author,
            bytes: edit.signed_bytes(chat, message.id),
            signature: edit.signature.0.clone(),
        });
    }
    proofs
}

/// Sends the draft, or saves the edit, once the host signed it as the person; unsigned if the
/// host could not, which others see as forged rather than lose it.
fn submit(editor: Editor, chat: &Chat, block: Uuid, me: Uuid, composer: Composer) {
    let body = composer.draft.get_untracked().trim().to_owned();
    if body.is_empty() {
        return;
    }
    let chat = *chat;
    match composer.editing.get_untracked() {
        Some(message) => {
            let edit = MessageEdit {
                author: me,
                time: now(),
                body,
                signature: Signature::default(),
            };
            let signing = editor.host().sign(edit.signed_bytes(block, message));
            editor.then(signing, move |signed| {
                let edit = MessageEdit {
                    signature: Signature(signed.unwrap_or_default()),
                    ..edit
                };
                if let Ok(change) = chat.read(|content| content.put_edit(message, &edit)) {
                    chat.operate(change);
                }
            });
            composer.editing.set(None);
        }
        None => {
            let message =
                Message::new(me, now(), body).replying_to(composer.replying.get_untracked());
            if let Ok((id, message)) = chat.read(|content| content.draft(block, message)) {
                let signing = editor.host().sign(message.signed_bytes(block, id));
                editor.then(signing, move |signed| {
                    let message = Message {
                        signature: Signature(signed.unwrap_or_default()),
                        ..message
                    };
                    chat.operate(ChatContent::send_as(id, &message));
                });
            }
            composer.replying.set(None);
        }
    }
    composer.draft.set(String::new());
}

/// The least a row's records are trusted: one forged record makes the message forged.
fn least(signers: impl IntoIterator<Item = Signer>) -> Signer {
    let rank = |signer: &Signer| match signer {
        Signer::Verified => 0,
        Signer::Unverified => 1,
        Signer::Changed => 2,
        Signer::Bad => 3,
    };
    signers
        .into_iter()
        .max_by_key(rank)
        .unwrap_or(Signer::Verified)
}

fn toggle_reaction(chat: &Chat, me: Uuid, message: ObjectId, reaction: &str, mine: bool) {
    let change = chat.read(|content| match mine {
        true => content.unreact(message, me, reaction, now()),
        false => content.react(message, me, reaction, now()),
    });
    if let Ok(change) = change {
        chat.operate(change);
    }
}

#[component]
fn MessageRow(
    editor: Editor,
    chat: Chat,
    me: Uuid,
    id: ObjectId,
    row: ReadSignal<Row>,
    composer: Composer,
) -> NodeId {
    let proofs = create_memo(move || row.with(|row| row.proofs.clone()));
    // Who signed it, once the host says: nothing is marked until then.
    let signer = create_signal(None::<Signer>);
    create_effect(move || {
        let proofs = proofs.get();
        signer.set(None);
        let host = editor.host();
        let asked = async move {
            let mut found = Vec::new();
            for proof in proofs {
                let verified = host.verify(proof.author, proof.bytes, proof.signature);
                found.push(verified.await?.ok()?);
            }
            Some(least(found))
        };
        let asking = editor.then(asked, move |found| signer.set(Some(found)));
        on_cleanup(move || asking.cancel());
    });
    let unverified = create_memo(move || signer.get() == Some(Signer::Unverified));
    let changed = create_memo(move || signer.get() == Some(Signer::Changed));
    let forged = create_memo(move || signer.get() == Some(Signer::Bad));
    let author = create_memo(move || row.with(|row| row.author.clone()));
    let time = create_memo(move || row.with(|row| row.time.clone()));
    let body = create_memo(move || row.with(|row| row.body.clone()));
    let edited = create_memo(move || row.with(|row| row.edited));
    let mine = create_memo(move || row.with(|row| row.mine));
    let quote = create_memo(move || row.with(|row| row.reply.clone().unwrap_or_default()));
    let replies = create_memo(move || row.with(|row| row.reply.is_some()));
    let reactions = create_memo(move || {
        row.with(|row| {
            row.reactions
                .iter()
                .map(|reacted| reacted.reaction.clone())
                .collect::<Vec<String>>()
        })
    });
    let reacted = create_memo(move || reactions.with(|reactions| !reactions.is_empty()));
    let picking = create_memo(move || composer.picking.get() == Some(id));
    let fill = create_memo(move || match mine.get() {
        true => ColorRole::AccentSoft,
        false => ColorRole::Control,
    });

    let reaction_chips = move |reaction: String| {
        let name = reaction.clone();
        let name_for_id = reaction.clone();
        let held = create_memo(move || {
            row.with(|row| {
                row.reactions
                    .iter()
                    .find(|reacted| reacted.reaction == name)
                    .map(|reacted| (reacted.count, reacted.mine))
                    .unwrap_or_default()
            })
        });
        let count = create_memo(move || held.get().0.to_string());
        let pressed = create_memo(move || held.get().1);
        let glyph = be_sdk_beui::beui::icons::named(&reaction);
        let said = REACTIONS
            .iter()
            .find(|(offered, _, _)| *offered == reaction)
            .map_or_else(|| reaction.clone(), |(_, _, said)| (*said).to_owned());
        let accessible = create_memo(move || {
            let mut node = Node::new(Role::Button);
            node.set_label(format!("{said}, {}", count.get()));
            node
        });
        view! {
            <ToggleButton
                label={count}
                glyph={glyph}
                pressed={Binding::new(pressed, move |_| {
                    toggle_reaction(&chat, me, id, &reaction, pressed.get_untracked());
                })}
                accessibility={accessible}
                @test_id={format!("chat.message.{id}.reaction.{name_for_id}")}
            />
        }
    };
    let picker = move |index: usize| {
        let (reaction, glyph, label) = REACTIONS[index];
        view! {
            <Button
                label
                glyph
                icon_only=true
                variant=ButtonVariant::Ghost
                size=ButtonSize::Small
                @test_id={format!("chat.message.{id}.react.{reaction}")}
                on_click={move || {
                    let mine = row.with_untracked(|row| {
                        row.reactions
                            .iter()
                            .any(|reacted| reacted.reaction == reaction && reacted.mine)
                    });
                    toggle_reaction(&chat, me, id, reaction, mine);
                    composer.picking.set(None);
                }}
            />
        }
    };
    let picks = create_memo(|| (0..REACTIONS.len()).collect::<Vec<usize>>());

    view! {
        <Frame
            fill={fill}
            radius=Radius::Md
            padding={Padding::symmetric(Space::Lg, Space::Md)}
            @test_id={format!("chat.message.{id}")}
        >
            <List spacing=Space::Sm>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                    <ListItem grow=1.0>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                            <Text weight=Weight::Bold>{author}</Text>
                            <Text style=TextStyle::CAPTION>{time}</Text>
                            <Show condition={edited}>
                                <Text style=TextStyle::CAPTION>"edited"</Text>
                            </Show>
                            <Show condition={unverified}>
                                <Text style=TextStyle::CAPTION>"not verified"</Text>
                            </Show>
                            <Show condition={changed}>
                                <Text style=TextStyle::CAPTION color=TextColor::Warning>
                                    "key changed"
                                </Text>
                            </Show>
                            <Show condition={forged}>
                                <Text style=TextStyle::CAPTION color=TextColor::Danger>
                                    "forged"
                                </Text>
                            </Show>
                        </List>
                    </ListItem>
                    <Button
                        label="Reply"
                        glyph=ICON_REPLY
                        icon_only=true
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Small
                        @test_id={format!("chat.message.{id}.reply")}
                        on_click={move || {
                            if composer.editing.get_untracked().is_some() {
                                composer.editing.set(None);
                                composer.draft.set(String::new());
                            }
                            composer.replying.set(Some(id));
                        }}
                    />
                    <Button
                        label="React"
                        glyph=ICON_STICKER
                        icon_only=true
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Small
                        @test_id={format!("chat.message.{id}.react")}
                        on_click={move || {
                            let open = composer.picking.get_untracked() == Some(id);
                            composer.picking.set((!open).then_some(id));
                        }}
                    />
                    <Show condition={mine}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                            <Button
                                label="Edit"
                                glyph=ICON_PENCIL
                                icon_only=true
                                variant=ButtonVariant::Ghost
                                size=ButtonSize::Small
                                @test_id={format!("chat.message.{id}.edit")}
                                on_click={move || {
                                    composer.replying.set(None);
                                    composer.editing.set(Some(id));
                                    composer.draft.set(body.get_untracked());
                                }}
                            />
                            <Button
                                label="Delete"
                                glyph=ICON_TRASH
                                icon_only=true
                                variant=ButtonVariant::Ghost
                                size=ButtonSize::Small
                                @test_id={format!("chat.message.{id}.delete")}
                                on_click={move || {
                                    if let Ok(delete) = chat.read(|content| content.delete(id, me, now())) {
                                        chat.operate(delete);
                                    }
                                    if composer.editing.get_untracked() == Some(id) {
                                        composer.editing.set(None);
                                        composer.draft.set(String::new());
                                    }
                                }}
                            />
                        </List>
                    </Show>
                </List>
                <Show condition={replies}>
                    <Frame
                        fill=ColorRole::Shade
                        radius=Radius::Sm
                        padding={Padding::symmetric(Space::Md, Space::Xs)}
                    >
                        <Text style=TextStyle::CAPTION>{quote}</Text>
                    </Frame>
                </Show>
                <Text>{body}</Text>
                <Show condition={reacted}>
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=Space::Sm
                        wrap=true
                    >
                        <ForEach keys={reactions} view={reaction_chips} />
                    </List>
                </Show>
                <Show condition={picking}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                        <ForEach keys={picks} view={picker} />
                    </List>
                </Show>
            </List>
        </Frame>
    }
}

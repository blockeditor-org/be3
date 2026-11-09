use be_sdk_beui::be_block::ObjectId;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::datetime::{DateTime, HourCycle};
use be_sdk_beui::beui::icons::{
    Glyph, ICON_CHECK, ICON_EYE, ICON_HEART, ICON_PARTY_POPPER, ICON_PENCIL, ICON_REPLY,
    ICON_SEND, ICON_STICKER, ICON_THUMBS_UP, ICON_TRASH, ICON_X,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{ContentProjection, Editor};
use be_type_chat::{ChatContent, Message};
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
    let rows = chat.project_keyed(move |content: &ChatContent, rows| {
        rows.reconcile_owned(
            content
                .messages()
                .into_iter()
                .filter_map(|id| Some((id, row(content, me, id)?))),
        );
    });
    let ids = rows.keys();
    let empty = create_memo(move || ids.with(Vec::is_empty));
    let composer = Composer {
        draft: create_signal(String::new()),
        replying: create_signal(None),
        editing: create_signal(None),
        picking: create_signal(None),
    };
    let block = editor.block_id();
    let send = move || submit(&chat, block, me, composer);
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
            <MessageRow chat={chat} me id row={shown} composer />
        }
    };

    view! {
        <List spacing=Space::None>
            <ListItem grow=1.0>
                <Scroll follow_end=true>
                    <Frame padding={Padding::all(Space::Xl)}>
                        <List spacing=Space::Md>
                            <Show condition={empty}>
                                <Text style=TextStyle::CAPTION>"No messages yet. Say hello below."</Text>
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

fn row(content: &ChatContent, me: Uuid, id: ObjectId) -> Option<Row> {
    let message = content.message(id)?;
    let reply = message.reply_to.map(|quoted| {
        match (content.message(quoted), content.body(quoted)) {
            (Some(quoted), Some(body)) => {
                format!("{}: {}", author(quoted.author, me), excerpt(&body))
            }
            _ => "A deleted message".to_owned(),
        }
    });
    let reactions = content
        .reactions(id)
        .into_iter()
        .map(|(reaction, authors)| Reacted {
            reaction,
            count: authors.len(),
            mine: authors.contains(&me),
        })
        .collect();
    Some(Row {
        author: author(message.author, me),
        mine: message.author == me,
        time: time(message.time),
        body: content.body(id).unwrap_or_default(),
        edited: content.edit_of(id).is_some(),
        reply,
        reactions,
    })
}

/// No host service names members yet, so others show by their account's first digits.
fn author(account: Uuid, me: Uuid) -> String {
    match account == me {
        true => "You".to_owned(),
        false => format!("Member {}", &account.simple().to_string()[..4]),
    }
}

fn time(millis: i64) -> String {
    DateTime::from_unix(millis.div_euclid(1000))
        .time
        .format(HourCycle::H24)
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

fn submit(chat: &Chat, block: Uuid, me: Uuid, composer: Composer) {
    let body = composer.draft.get_untracked().trim().to_owned();
    if body.is_empty() {
        return;
    }
    match composer.editing.get_untracked() {
        Some(message) => {
            chat.operate(ChatContent::edit(message, me, now(), body));
            composer.editing.set(None);
        }
        None => {
            let message = Message::new(me, now(), body).replying_to(composer.replying.get_untracked());
            if let Ok((_, send)) = chat.read(|content| content.send(block, message)) {
                chat.operate(send);
            }
            composer.replying.set(None);
        }
    }
    composer.draft.set(String::new());
}

fn toggle_reaction(chat: &Chat, me: Uuid, message: ObjectId, reaction: &str, mine: bool) {
    match mine {
        true => chat.operate(ChatContent::unreact(message, me, reaction)),
        false => chat.operate(ChatContent::react(message, me, reaction, now())),
    }
}

#[component]
fn MessageRow(
    chat: Chat,
    me: Uuid,
    id: ObjectId,
    row: ReadSignal<Row>,
    composer: Composer,
) -> NodeId {
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
        view! {
            <ToggleButton
                label={count}
                glyph={glyph}
                pressed={Binding::new(pressed, move |_| {
                    toggle_reaction(&chat, me, id, &reaction, pressed.get_untracked());
                })}
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
                            composer.editing.set(None);
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
                    <Frame fill=ColorRole::Shade radius=Radius::Sm padding={Padding::symmetric(Space::Md, Space::Xs)}>
                        <Text style=TextStyle::CAPTION>{quote}</Text>
                    </Frame>
                </Show>
                <Text>{body}</Text>
                <Show condition={reacted}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm wrap=true>
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

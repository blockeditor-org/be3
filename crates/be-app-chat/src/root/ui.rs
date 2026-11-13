use std::collections::HashMap;
use std::rc::Rc;

use be_sdk_beui::be_block_content::be_block_model::BlockRef;
use be_sdk_beui::be_block_content::{ObjectId, SettingsLayer};
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::accesskit::{Node, Role};
use be_sdk_beui::beui::datetime::{DateTime, HourCycle};
use be_sdk_beui::beui::icons::{
    Glyph, ICON_ARROW_DOWN, ICON_ARROW_UP, ICON_CHECK, ICON_EYE, ICON_HEART, ICON_LOCK,
    ICON_PAPERCLIP, ICON_PARTY_POPPER, ICON_PENCIL, ICON_REPLY, ICON_SEND, ICON_STICKER,
    ICON_THUMBS_UP, ICON_TRASH, ICON_X,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{BlockFilter, BlockLink, BlockQuery, ChildTarget, ContentProjection, Editor, KeptEdit, LayeredSettings, PickedFile, Signer, utc_offset, watch_block_label};
use be_type_chat::{
    ChatContent, ChatReads, Deletion, Message, MessageEdit, Reaction, ReactionKey, ReadPlace,
    Segment, Shown, Signature,
};
use beui::{Align, Direction, ScrollPosition};
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
    /// The blocks sent with it.
    attachments: Vec<Uuid>,
    /// What its authors signed: the message, its edit if it has one, and every reaction record
    /// to it; for a deleted message's row, the deletion.
    proofs: Vec<Proof>,
    /// The row of a message whose deletion is forged, in the deleted message's place: it shows
    /// who claims to have deleted it, and takes no reply, reaction or edit.
    deleted: bool,
}

/// One record a member signed (plans/authorship.md), to ask the host who signed it.
#[derive(Clone, Debug, PartialEq)]
struct Proof {
    author: Uuid,
    record: Record,
    signature: Vec<u8>,
}

/// What a signature covers, before the chat it is in is named: the block signatures bind to is
/// known only once the host says (in a revision preview it is the block the revision is of).
#[derive(Clone, Debug, PartialEq)]
enum Record {
    Message(ObjectId, Message),
    /// An edit, and whether it was replaced (`ChatContent::replaced`): shown again, it was put
    /// back.
    Edit(ObjectId, MessageEdit, Message, bool),
    Deletion(ObjectId, Deletion),
    /// A reaction record, and whether it was replaced.
    Reaction(ReactionKey, Reaction, bool),
    /// A record removed outright, which no signature can cover.
    Removed,
}

impl Record {
    /// The bytes its author signed, or none for a record that is forged whatever its
    /// signature: a removal, or an edit put back after it was replaced.
    fn signed_bytes(&self, chat: Uuid) -> Option<Vec<u8>> {
        match self {
            Self::Message(id, message) => Some(message.signed_bytes(chat, *id)),
            Self::Edit(id, edit, sent, replaced) => {
                (!replaced).then(|| edit.signed_bytes(chat, *id, sent.hash(chat, *id)))
            }
            Self::Deletion(id, deletion) => Some(deletion.signed_bytes(chat, *id)),
            Self::Reaction(key, reaction, replaced) => {
                (!replaced).then(|| key.signed_bytes(chat, reaction))
            }
            Self::Removed => None,
        }
    }
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
    /// The blocks to send with the next message.
    attaching: Signal<Vec<Pending>>,
}

/// A block the draft carries (decisions/chat_attachments_are_references.md): an existing block
/// by reference, where it is, or one made for the draft (in the picker, or a pasted image) as a
/// child of the chat.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pending {
    id: Uuid,
    block_type: Uuid,
    made: bool,
}

/// The type of each block the chat's messages may carry, by id, with whether it is the chat's
/// child: its children and what it references. One missing is one this account cannot read.
type Attached = Memo<HashMap<Uuid, (Uuid, bool)>>;

fn attached(editor: Editor, block: Uuid) -> Attached {
    let children = editor.blocks().watch(BlockQuery::Children(block));
    let references = editor.blocks().watch(BlockQuery::References(block));
    let found = create_signal(HashMap::new());
    create_binding(move || {
        let mut known = HashMap::new();
        if let Ok(blocks) = references.read() {
            known.extend(
                blocks
                    .into_iter()
                    .map(|info| (info.id, (info.block_type, false))),
            );
        }
        if let Ok(blocks) = children.read() {
            known.extend(
                blocks
                    .into_iter()
                    .map(|info| (info.id, (info.block_type, true))),
            );
        }
        found.set(known);
    });
    create_memo(move || found.get())
}

/// Picks a block to send with the next message: an existing one is referenced where it is, and
/// one made in the picker is made in the chat.
fn attach(editor: Editor, chat: Chat, me: Uuid, composer: Composer, anchor: NodeRef) {
    editor.pick_block(
        BlockFilter {
            name: "Attach".to_owned(),
            block_types: Vec::new(),
            excluded: Vec::new(),
            templates: true,
            place: None,
        },
        &anchor,
        move |picked| {
            let Ok(picked) = picked else {
                return;
            };
            let block_type = Uuid::from_bytes(picked.block_type);
            match picked.linked {
                true => pend(
                    composer,
                    Pending {
                        id: picked.id(),
                        block_type,
                        made: false,
                    },
                ),
                false => draft_made(chat, me, composer, picked.id(), block_type),
            }
        },
    );
}

fn pend(composer: Composer, pending: Pending) {
    composer.attaching.update(|attaching| {
        if !attaching.iter().any(|held| held.id == pending.id) {
            attaching.push(pending);
        }
    });
}

/// Lists a block made in the chat as a draft of `me`'s next message (`ChatContent::make_draft`),
/// which keeps it the chat's child until a message carries it, and attaches it.
fn draft_made(chat: Chat, me: Uuid, composer: Composer, id: Uuid, block_type: Uuid) {
    if let Ok(draft) = chat.read(|content| content.make_draft(id, me, now())) {
        chat.operate(draft.into());
    }
    pend(
        composer,
        Pending {
            id,
            block_type,
            made: true,
        },
    );
}

/// An image pasted into the composer becomes a child of the chat, attached to the draft.
fn paste_image(editor: Editor, chat: Chat, me: Uuid, composer: Composer, file: PickedFile) {
    let image = be_type_image::ImageFile::from_file(file.name, file.data);
    let id = editor.create_child(&image);
    draft_made(
        chat,
        me,
        composer,
        id,
        <be_type_image::ImageContent as be_sdk_beui::be_block_content::BlockContent>::CONTENT_TYPE,
    );
}

/// Takes a block made for the draft out of the chat, which deletes it: only while it is still
/// one of the chat's children and still a draft, so a block moved elsewhere since, or one a
/// message carries, is left alone. A referenced block is never deleted.
fn drop_made(chat: Chat, attached: Attached, pending: Pending) {
    let child = attached
        .with_untracked(|attached| attached.get(&pending.id).is_some_and(|(_, child)| *child));
    if !pending.made || !child {
        return;
    }
    if let Ok(Some(dropped)) = chat.read(|content| content.drop_draft(pending.id)) {
        chat.operate(dropped.into());
    }
}

/// A block sent with a message: a link to it by its name, or a lock and "No access" for a
/// block this account cannot read.
#[component]
fn AttachmentChip(editor: Editor, attached: Attached, id: Uuid, link_id: String) -> NodeId {
    let target = create_memo(move || {
        attached.with(|attached| {
            attached
                .get(&id)
                .map(|(block_type, _)| ChildTarget::new(id, *block_type))
        })
    });
    let readable = create_memo(move || target.get().is_some());
    let hidden = create_memo(move || !readable.get());
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing=Space::Xs
            @test_id={link_id}
        >
            <Show condition={readable}>
                <BlockLink editor block={target} />
            </Show>
            <Show condition={hidden}>
                <NoAccess />
            </Show>
        </List>
    }
}

/// An attachment this account cannot read: not a link, since it would open nothing.
#[component]
fn NoAccess() -> NodeId {
    component_accessibility(create_memo(|| {
        let mut node = Node::new(Role::Label);
        node.set_label("An attached block you have no access to");
        node
    }));
    view! {
        <Text size=TextSize::Sm color=TextColor::Muted>
            <Icon glyph=ICON_LOCK color=TextColor::Muted />
            " No access"
        </Text>
    }
}

/// A block the draft carries: its type's icon and its name.
#[component]
fn PendingChip(editor: Editor, pending: Pending, link_id: String) -> NodeId {
    let target = create_memo(move || Some(ChildTarget::new(pending.id, pending.block_type)));
    let shown = watch_block_label(editor, target);
    // A block made for the draft is not listed by the chat until it is sent: its type's icon,
    // not a deleted block's.
    let glyph = create_memo(move || {
        be_sdk_beui::be_sdk_ui::BlockLabel::new(
            editor.block_types().as_ref(),
            pending.block_type,
            None,
            false,
        )
        .icon
    });
    let name = create_memo(move || shown.with(|shown| shown.name.clone()));
    let made = pending.made;
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing=Space::Xs
            @test_id={link_id}
        >
            <Text size=TextSize::Sm>
                <Icon glyph={glyph} color=TextColor::Muted />
                " "
                {name}
            </Text>
            <Show condition={made}>
                <Text size=TextSize::Xs color=TextColor::Muted>"New"</Text>
            </Show>
        </List>
    }
}

#[component]
pub fn Chat(editor: Editor) -> NodeId {
    let chat: Chat = editor.block_content::<ChatContent>();
    let me = editor.blocks().account_id();
    let block = editor.block_id();
    // The block signatures bind to: this one, or in a revision preview the one it is of.
    let signing = create_signal(block);
    let asking = editor.then(editor.host().signing_block(), move |found| {
        if let Ok(found) = found {
            signing.set(found);
        }
    });
    on_cleanup(move || asking.cancel());
    let forged = forged_deletions(editor, chat, signing);
    let rows: KeyedStore<ObjectId, Row> = KeyedStore::new();
    create_effect(move || {
        let _ = chat.revision();
        let forged = forged.get();
        if let Ok(rows_now) =
            chat.read(|content| row_list(content, &content.shown(), me, &forged, None))
        {
            rows.reconcile_owned(rows_now);
        }
    });
    let ids = rows.keys();
    let older = Older::new(chat, me, forged);
    let older_ids = older.rows.keys();
    let empty = create_memo(move || ids.with(Vec::is_empty) && older_ids.with(Vec::is_empty));
    let composer = Composer {
        draft: create_signal(String::new()),
        replying: create_signal(None),
        editing: create_signal(None),
        picking: create_signal(None),
        attaching: create_signal(Vec::new()),
    };
    let attached = attached(editor, block);
    let attach_button = NodeRef::new();
    let attaching_ids = create_memo(move || {
        composer.attaching.with(|attaching| {
            attaching
                .iter()
                .map(|pending| pending.id)
                .collect::<Vec<_>>()
        })
    });
    let has_attaching = create_memo(move || composer.attaching.with(|ids| !ids.is_empty()));
    let makes_children = create_memo(move || {
        composer
            .attaching
            .with(|attaching| attaching.iter().any(|pending| pending.made))
    });
    // The blocks this account made for a draft are the chat's drafts (`drafts_of`): they come
    // back when the editor opens again, and leave the composer once something else takes them
    // out of the chat (a move, a delete in the files pane, a send from another device).
    let restored = std::cell::Cell::new(false);
    create_effect(move || {
        let _ = chat.revision();
        let Ok(drafted) = chat.read(|content| content.drafts_of(me)) else {
            return;
        };
        let types = attached.get();
        let restoring = !restored.replace(true);
        let mut attaching = composer.attaching.get_untracked();
        let before = attaching.clone();
        attaching.retain(|held| !held.made || drafted.contains(&held.id));
        if restoring {
            for id in drafted {
                if let Some((block_type, _)) = types.get(&id)
                    && !attaching.iter().any(|held| held.id == id)
                {
                    attaching.push(Pending {
                        id,
                        block_type: *block_type,
                        made: true,
                    });
                }
            }
        }
        if attaching != before {
            composer.attaching.set(attaching);
        }
    });
    let attaching_chip = move |id: Uuid| {
        let pending = composer
            .attaching
            .with_untracked(|attaching| attaching.iter().find(|held| held.id == id).copied());
        let Some(pending) = pending else {
            return view! {
                <Frame />
            };
        };
        view! {
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                <PendingChip editor pending link_id={format!("chat.attaching.{id}")} />
                <Button
                    label="Remove the attachment"
                    glyph=ICON_X
                    icon_only=true
                    variant=ButtonVariant::Ghost
                    size=ButtonSize::Small
                    @test_id={format!("chat.attaching.{id}.remove")}
                    on_click={move || {
                        let mut removed = Vec::new();
                        composer.attaching.update(|attaching| {
                            removed = attaching.iter().filter(|held| held.id == id).copied().collect();
                            attaching.retain(|held| held.id != id);
                        });
                        for pending in removed {
                            drop_made(chat, attached, pending);
                        }
                    }}
                />
            </List>
        }
    };
    // A message replied to or being edited can be deleted, here or elsewhere.
    create_effect(move || {
        let older_ids = older_ids.get();
        ids.with(|ids| {
            let ids: Vec<ObjectId> = older_ids.iter().chain(ids.iter()).copied().collect();
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
    let send = move || {
        submit(
            editor,
            &chat,
            signing.get_untracked(),
            me,
            composer,
            attached,
        )
    };
    let scroll = ComponentRef::<ScrollApi>::new();
    let at_end = create_signal(true);
    let unread = unread_count(editor, chat, block, me, at_end);
    let has_unread = create_memo(move || unread.get().is_some_and(|count| count > 0));
    let unread_label = create_memo(move || match unread.get().unwrap_or_default() {
        1 => "1 new message".to_owned(),
        count => format!("{count} new messages"),
    });
    let replying = create_memo(move || {
        let id = composer.replying.get()?;
        let quoted = rows.try_get(&id).or_else(|| older.rows.try_get(&id))?;
        Some(quoted.with(|row| format!("Replying to {}: {}", row.author, excerpt(&row.body))))
    });
    let reply_label = create_memo(move || replying.get().unwrap_or_default());
    let has_reply = create_memo(move || replying.get().is_some());
    let editing = create_memo(move || composer.editing.get().is_some());
    let send_label = create_memo(move || match editing.get() {
        true => "Save".to_owned(),
        false => "Send".to_owned(),
    });

    // The messages sent from this device that the server lacks: sending, or not sent once
    // the server refused them. The replica keeps them, so they show whichever editor sent them.
    let lacking = chat.lacking();
    let unsent = create_memo(move || {
        lacking.with(|lacking| {
            lacking
                .iter()
                .flat_map(|edit| {
                    let unsent = Unsent {
                        lacking: edit.id,
                        failed: edit.failed,
                    };
                    edit.operation
                        .iter()
                        .flat_map(ChatContent::sent_in)
                        .map(move |id| (id, unsent))
                        .collect::<Vec<_>>()
                })
                .collect::<HashMap<ObjectId, Unsent>>()
        })
    });

    // The oldest recent messages close into a segment once the recent ones are long enough.
    // Any editor may propose it, and the segment closes once: the messages it names are the
    // oldest ones in the order every replica has, and a second proposal finds them gone.
    let visible = editor.presence_visible();
    let lacking_any = create_memo(move || lacking.with(|lacking| !lacking.is_empty()));
    create_effect(move || {
        let _ = chat.revision();
        let visible = visible.get();
        let unconfirmed = lacking_any.get();
        // Only while every edit of ours (a message, a close, an edit, a reaction) is confirmed: a
        // device holds at most one unconfirmed close, so two devices rarely close apart.
        if !visible || unconfirmed {
            return;
        }
        if let Ok(Some(closing)) = chat.read(|content| content.closing()) {
            untrack(|| chat.operate(closing));
        }
    });

    let message_rows = move |id: ObjectId| {
        let shown = rows.get(&id);
        view! {
            <MessageRow
                editor
                chat={chat}
                me
                id
                row={shown}
                composer
                unsent
                block
                signing
                attached
            />
        }
    };
    let older_message_rows = move |id: ObjectId| {
        let shown = older.rows.get(&id);
        view! {
            <MessageRow
                editor
                chat={chat}
                me
                id
                row={shown}
                composer
                unsent
                block
                signing
                attached
            />
        }
    };
    let has_older = older.remaining;
    let older_failed = older.failed;

    view! {
        <List spacing=Space::None>
            <ListItem grow=1.0>
                <Scroll
                    api={scroll}
                    follow_end=true
                    on_change={move |position: ScrollPosition| {
                        at_end.set(position.offset >= position.max_offset() - 1.0);
                        older.at_start.set(position.offset <= 1.0);
                    }}
                >
                    <Frame padding={Padding::all(Space::Xl)}>
                        <List spacing=Space::Md>
                            <Show condition={empty}>
                                <Text style=TextStyle::CAPTION>
                                    "No messages yet. Say hello below."
                                </Text>
                            </Show>
                            <Show condition={has_older}>
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Sm
                                >
                                    <Button
                                        label="Earlier messages"
                                        glyph=ICON_ARROW_UP
                                        variant=ButtonVariant::Secondary
                                        size=ButtonSize::Small
                                        @test_id={"chat.older"}
                                        on_click={move || older.load()}
                                    />
                                    <Show condition={older_failed}>
                                        <Text style=TextStyle::CAPTION>
                                            "Could not read the earlier messages"
                                        </Text>
                                    </Show>
                                </List>
                            </Show>
                            <ForEach keys={older_ids} view={older_message_rows} />
                            <ForEach keys={ids} view={message_rows} />
                        </List>
                    </Frame>
                </Scroll>
            </ListItem>
            <Separator />
            <Frame padding={Padding::all(Space::Lg)}>
                <List spacing=Space::Sm>
                    <Show condition={has_unread}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                            <Button
                                label={unread_label}
                                glyph=ICON_ARROW_DOWN
                                variant=ButtonVariant::Secondary
                                size=ButtonSize::Small
                                @test_id={"chat.unread"}
                                on_click={move || scroll.get().scroll_to_end()}
                            />
                        </List>
                    </Show>
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
                    <Show condition={has_attaching}>
                        <List spacing=Space::Xs>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Sm
                            >
                                <ForEach keys={attaching_ids} view={attaching_chip} />
                            </List>
                            <Show condition={makes_children}>
                                <Text style=TextStyle::CAPTION>
                                    "What is made here is part of the chat: everyone in it can open it."
                                </Text>
                            </Show>
                        </List>
                    </Show>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <Button
                            label="Attach a block"
                            glyph=ICON_PAPERCLIP
                            icon_only=true
                            variant=ButtonVariant::Ghost
                            disabled={editing}
                            @node_ref={&attach_button}
                            @test_id={"chat.attach"}
                            on_click={move || attach(editor, chat, me, composer, attach_button)}
                        />
                        <ListItem grow=1.0>
                            <TextInput
                                value={composer.draft}
                                placeholder="Message"
                                label="Message"
                                @test_id={"chat.draft"}
                                on_submit={move |_| send()}
                                on_image_paste={move |file: PickedFile| paste_image(editor, chat, me, composer, file)}
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

/// The messages of closed segments, read back by range as the person scrolls to the start of
/// the chat (guides/chat.md, "The editor"): the newest segment first, one at a time, each checked
/// against the index before it shows.
#[derive(Clone, Copy)]
struct Older {
    chat: Chat,
    /// The segments read so far, each as its index entry names it (two that overlap can open at
    /// the same message), with the messages its part holds.
    read: Signal<Vec<(Segment, Vec<(ObjectId, Message)>)>>,
    segments: ReadSignal<Vec<Segment>>,
    rows: KeyedStore<ObjectId, Row>,
    reading: Signal<bool>,
    /// The last read failed, or returned bytes the index does not describe.
    failed: Signal<bool>,
    /// The chat is scrolled to its first message.
    at_start: Signal<bool>,
    /// A closed segment is left to read.
    remaining: Memo<bool>,
}

impl Older {
    fn new(chat: Chat, me: Uuid, forged: Memo<Vec<(ObjectId, Deletion)>>) -> Self {
        let segments = chat.project(|content: &ChatContent| content.segments());
        let read: Signal<Vec<(Segment, Vec<(ObjectId, Message)>)>> = create_signal(Vec::new());
        let rows = KeyedStore::new();
        let older = Self {
            chat,
            read,
            segments,
            rows,
            reading: create_signal(false),
            failed: create_signal(false),
            at_start: create_signal(false),
            remaining: create_memo(move || {
                segments.with(|segments| read.with(|read| unread_segment(segments, read).is_some()))
            }),
        };
        // Reaching the start of the chat reads the next older segment.
        create_effect(move || {
            if older.at_start.get() {
                untrack(|| older.load());
            }
        });
        // What was read follows the edits, deletions and reactions made to it since.
        create_effect(move || {
            let _ = chat.revision();
            let (segments, read, forged) = (segments.get(), read.get(), forged.get());
            let rows_now = chat
                .read(|content| {
                    // Segments closed apart can overlap: a message shows once, in its place.
                    let read_in_order: Vec<&[(ObjectId, Message)]> = segments
                        .iter()
                        .filter_map(|segment| read.iter().find(|(read, _)| read == segment))
                        .map(|(_, messages)| messages.as_slice())
                        .collect();
                    let messages = ChatContent::weave(&read_in_order);
                    let order: Vec<ObjectId> = messages.iter().map(|(id, _)| *id).collect();
                    row_list(
                        content,
                        &content.shown_of(messages),
                        me,
                        &forged,
                        Some(&order),
                    )
                })
                .unwrap_or_default();
            rows.reconcile_owned(rows_now);
        });
        older
    }

    /// Reads the newest segment not read yet, if there is one and none is being read.
    fn load(&self) {
        if self.reading.get_untracked() {
            return;
        }
        let read = self.read.get_untracked();
        let segments = self.segments.get_untracked();
        let Some(index) = unread_segment(&segments, &read) else {
            return;
        };
        self.read_segment_at(&segments[index], true);
    }

    /// Reads `segment`, by the range the header held now gives it. The
    /// replica's header can differ from this one's while the chat is busy, which moves the range
    /// and fails the check against the index; the read is tried once more then.
    fn read_segment_at(&self, segment: &Segment, retry: bool) {
        let live = self.chat.live();
        let Ok(Some((index, range))) = self.chat.read(|content| {
            let index = content.segments().iter().position(|held| held == segment)?;
            Some((index, content.segment_ranges().get(index).cloned()?))
        }) else {
            return;
        };
        self.reading.set(true);
        self.failed.set(false);
        let this = *self;
        let segment = segment.clone();
        live.read_range(
            range.start,
            range.end.saturating_sub(range.start),
            move |bytes| {
                let messages = bytes.ok().and_then(|bytes| {
                    this.chat
                        .read(|content| content.read_segment(index, &bytes))
                        .ok()
                        .flatten()
                });
                match messages {
                    Some(messages) => {
                        this.reading.set(false);
                        let mut read = this.read.get_untracked();
                        read.push((segment, messages));
                        this.read.set(read);
                    }
                    None if retry => this.read_segment_at(&segment, false),
                    None => {
                        this.reading.set(false);
                        this.failed.set(true);
                    }
                }
            },
        );
    }
}

/// The newest segment of `segments` not in `read`: segments are told apart by their whole index
/// entry, since two closed apart can open at the same message.
pub(crate) fn unread_segment<T>(segments: &[Segment], read: &[(Segment, T)]) -> Option<usize> {
    segments
        .iter()
        .rposition(|segment| !read.iter().any(|(read, _)| read == segment))
}

/// The rows of `shown` messages, each with its replied-to message and who signed it, and a
/// row for each message of `forged` (forged deletions) in its place: by `order`, the messages
/// read back from closed segments with the deleted ones, when given, else after the message
/// the deletion says it followed.
fn row_list(
    content: &ChatContent,
    shown: &[Shown],
    me: Uuid,
    forged: &[(ObjectId, Deletion)],
    order: Option<&[ObjectId]>,
) -> Vec<(ObjectId, Row)> {
    let by_id: HashMap<ObjectId, &Shown> =
        shown.iter().map(|message| (message.id, message)).collect();
    let today = local_date(now());
    let mut rows: Vec<(ObjectId, Row)> = shown
        .iter()
        .map(|message| {
            let row = Row {
                proofs: proofs(content, message),
                ..row(&by_id, message, me, today)
            };
            (message.id, row)
        })
        .collect();
    for (message, deletion) in forged {
        let after = match order {
            // Read back from a segment: after the nearest message before it that has a row.
            Some(order) => {
                let Some(at) = order.iter().position(|id| id == message) else {
                    continue;
                };
                order[..at]
                    .iter()
                    .rev()
                    .find(|id| rows.iter().any(|(row, _)| row == *id))
                    .copied()
            }
            // A recent message's deletion names the message it followed; one that followed
            // none may be of a closed message, shown with the segment instead.
            None if deletion.after.is_none() && content.may_be_closed(*message) => continue,
            None => deletion.after,
        };
        let at = match after {
            None => Some(0),
            Some(after) => rows
                .iter()
                .position(|(id, _)| *id == after)
                .map(|at| at + 1),
        };
        if let Some(at) = at {
            rows.insert(at, (*message, deleted_row(*message, deletion, me, today)));
        }
    }
    rows
}

/// How many messages others sent since this account last read the chat, none until its
/// owner-only `ChatReads` loaded. Each device keeps its own place there and the furthest
/// counts, so every device the account uses agrees. The chat is read while it is scrolled to its
/// newest message, shown, and its own view has the keyboard: a chat left open behind others,
/// or beside the one being typed in, is not.
fn unread_count(
    editor: Editor,
    chat: Chat,
    block: Uuid,
    me: Uuid,
    at_end: Signal<bool>,
) -> Memo<Option<usize>> {
    let reads = LayeredSettings::<ChatReads>::this_device(editor);
    let (read, ready) = (reads.effective(), reads.ready());
    let device = editor.host().client_id();
    let unread = create_memo(move || {
        if !ready.get() {
            return None;
        }
        let places = read.with(|reads| reads.places(block));
        chat.read(|content| content.unread(places.into_iter().map(|place| place.message), me))
            .ok()
    });
    let shown = editor.presence_visible();
    let keyboard = editor.has_keyboard();
    create_effect(move || {
        let reading = at_end.get() && shown.get() && keyboard.get();
        if !reading || unread.get().unwrap_or_default() == 0 {
            return;
        }
        // Tracked, so a write the settings were not loaded for is made once they are.
        let places = read.with(|reads| reads.places(block));
        let moved = untrack(|| {
            chat.read(|content| {
                let newest = content.newest()?;
                let here = places.iter().filter(|place| place.device == device);
                if here
                    .clone()
                    .any(|place| !content.reads_further(newest, Some(place.message)))
                {
                    return None;
                }
                let seen: Vec<ReadPlace> = places
                    .iter()
                    .filter(|place| place.device == device || content.behind(place.message, newest))
                    .cloned()
                    .collect();
                Some((newest, seen))
            })
        });
        let Ok(Some((newest, seen))) = moved else {
            return;
        };
        let edit: KeptEdit<ChatReads> =
            Rc::new(move |_| ChatReads::set_read(block, device, newest, &seen));
        // Not yet loaded, it is written on the next change instead.
        let _ = reads.write_all(SettingsLayer::EveryDevice, &[edit]);
    });
    unread
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
        attachments: message
            .message
            .attachments
            .iter()
            .map(|attachment| attachment.0)
            .collect(),
        deleted: false,
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

/// A message the server lacks: the edit that sent it, and whether the server refused it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Unsent {
    lacking: u64,
    failed: bool,
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
fn proofs(content: &ChatContent, message: &Shown) -> Vec<Proof> {
    let sent = &message.message;
    let mut proofs = vec![Proof {
        author: sent.author,
        record: Record::Message(message.id, sent.clone()),
        signature: sent.signature.0.clone(),
    }];
    // An edit or deletion record removed outright is no record its author signed.
    if content.edit_removed(message.id, sent.author) || content.deletion_removed(message.id) {
        proofs.push(Proof {
            author: sent.author,
            signature: Vec::new(),
            record: Record::Removed,
        });
    }
    if let Some(edit) = content.edit_by(message.id, sent.author) {
        let replaced = content.replaced(edit.key(message.id));
        proofs.push(Proof {
            author: edit.author,
            signature: edit.signature.0.clone(),
            record: Record::Edit(message.id, edit, sent.clone(), replaced),
        });
    }
    for (key, reaction) in content.reaction_records(message.id) {
        proofs.push(match reaction {
            Some(reaction) => Proof {
                author: key.author,
                signature: reaction.signature.0.clone(),
                record: Record::Reaction(
                    key.clone(),
                    reaction.clone(),
                    content.replaced(reaction.key(&key)),
                ),
            },
            None => Proof {
                author: key.author,
                signature: Vec::new(),
                record: Record::Removed,
            },
        });
    }
    proofs
}

/// The row of a message whose deletion is forged.
fn deleted_row(message: ObjectId, deletion: &Deletion, me: Uuid, today: i64) -> Row {
    Row {
        author: author(deletion.author, me),
        mine: false,
        time: time(deletion.time, today),
        body: "Message deleted".to_owned(),
        edited: false,
        reply: None,
        reactions: Vec::new(),
        attachments: Vec::new(),
        proofs: vec![Proof {
            author: deletion.author,
            signature: deletion.signature.0.clone(),
            record: Record::Deletion(message, deletion.clone()),
        }],
        deleted: true,
    }
}

/// The deletions the host says are forged, each with the message it deleted: each deletion is
/// asked about once, when it first shows in the content, and only a forged one gets a row.
fn forged_deletions(
    editor: Editor,
    chat: Chat,
    signing: Signal<Uuid>,
) -> Memo<Vec<(ObjectId, Deletion)>> {
    type Verdicts = HashMap<ObjectId, (Deletion, Option<Signer>)>;
    let verdicts: Signal<Verdicts> = create_signal(HashMap::new());
    create_effect(move || {
        let _ = chat.revision();
        let block = signing.get();
        let Ok(made) = chat.read(ChatContent::deletions_made) else {
            return;
        };
        for (message, deletion) in made {
            let asked = verdicts.with_untracked(|verdicts| {
                verdicts
                    .get(&message)
                    .is_some_and(|(held, _)| *held == deletion)
            });
            if asked {
                continue;
            }
            verdicts.update(|verdicts| {
                verdicts.insert(message, (deletion.clone(), None));
            });
            let asking = editor.host().verify(
                deletion.author,
                deletion.signed_bytes(block, message),
                deletion.signature.0.clone(),
            );
            editor.then(asking, move |found| {
                let Ok(signer) = found else {
                    return;
                };
                verdicts.update(|verdicts| {
                    if let Some((held, verdict)) = verdicts.get_mut(&message)
                        && *held == deletion
                    {
                        *verdict = Some(signer);
                    }
                });
            });
        }
    });
    create_memo(move || {
        verdicts.with(|verdicts| {
            let mut forged: Vec<(ObjectId, Deletion)> = verdicts
                .iter()
                .filter(|(_, (_, verdict))| *verdict == Some(Signer::Bad))
                .map(|(message, (deletion, _))| (*message, deletion.clone()))
                .collect();
            forged.sort_by_key(|(message, _)| *message);
            forged
        })
    })
}

/// Deletes `message` as `me`, signed once the host signed it; unsigned if it could not, which
/// others see as forged rather than lose it.
fn delete(editor: Editor, chat: Chat, block: Uuid, me: Uuid, message: ObjectId) {
    let Ok(deletion) = chat.read(|content| content.deletion(message, me, now())) else {
        return;
    };
    let signing = editor.host().sign(deletion.signed_bytes(block, message));
    editor.then(signing, move |signed| {
        let deletion = Deletion {
            signature: Signature(signed.unwrap_or_default()),
            ..deletion
        };
        if let Ok(change) = chat.read(|content| content.put_deletion(message, &deletion)) {
            chat.operate(change.into());
        }
    });
}

/// The longest body sent: well under what `sign` accepts, with the rest of the record.
const MAX_BODY_BYTES: usize = 32 * 1024;

/// Sends the draft, or saves the edit, once the host signed it as the person; unsigned if the
/// host could not, which others see as forged rather than lose it.
fn submit(
    editor: Editor,
    chat: &Chat,
    block: Uuid,
    me: Uuid,
    composer: Composer,
    attached: Attached,
) {
    let body = composer.draft.get_untracked().trim().to_owned();
    let editing = composer.editing.get_untracked();
    let pending = match editing {
        Some(_) => Vec::new(),
        None => composer.attaching.get_untracked(),
    };
    // A block made for it is carried only once it is one of the chat's children.
    let unready = pending.iter().any(|pending| {
        pending.made
            && !attached.with_untracked(|attached| {
                attached.get(&pending.id).is_some_and(|(_, child)| *child)
            })
    });
    if unready {
        return;
    }
    let attachments: Vec<Uuid> = pending.iter().map(|pending| pending.id).collect();
    // A body the host would refuse to sign stays in the composer rather than go out as forged.
    if (body.is_empty() && attachments.is_empty()) || body.len() > MAX_BODY_BYTES {
        return;
    }
    let chat = *chat;
    match editing {
        Some(message) => {
            let prepared = chat
                .read(|content| {
                    let of = content.hash(block, message)?;
                    Some((content.next_edit(message, me, now(), body), of))
                })
                .ok()
                .flatten();
            let Some((edit, of)) = prepared else {
                return;
            };
            let signing = editor.host().sign(edit.signed_bytes(block, message, of));
            editor.then(signing, move |signed| {
                let edit = MessageEdit {
                    signature: Signature(signed.unwrap_or_default()),
                    ..edit
                };
                if let Ok(change) = chat.read(|content| content.put_edit(message, &edit)) {
                    chat.operate(change.into());
                }
            });
            composer.editing.set(None);
        }
        None => {
            let message = Message {
                attachments: attachments.into_iter().map(BlockRef).collect(),
                ..Message::new(me, now(), body).replying_to(composer.replying.get_untracked())
            };
            if let Ok((id, message)) = chat.read(|content| content.draft(block, message)) {
                let signing = editor.host().sign(message.signed_bytes(block, id));
                editor.then(signing, move |signed| {
                    let message = Message {
                        signature: Signature(signed.unwrap_or_default()),
                        ..message
                    };
                    if let Ok(send) = chat.read(|content| content.send_drafted(id, &message)) {
                        chat.operate(send.into());
                    }
                });
            }
            composer.replying.set(None);
            composer.attaching.set(Vec::new());
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

/// Reacts to `message`, or takes `me`'s reaction back, as a record signed once the host signed
/// it; unsigned if it could not.
fn toggle_reaction(
    editor: Editor,
    chat: Chat,
    block: Uuid,
    me: Uuid,
    message: ObjectId,
    reaction: &str,
    mine: bool,
) {
    let Ok((key, reaction)) =
        chat.read(|content| content.reaction(message, me, reaction, now(), !mine))
    else {
        return;
    };
    let signing = editor.host().sign(key.signed_bytes(block, &reaction));
    editor.then(signing, move |signed| {
        let reaction = Reaction {
            signature: Signature(signed.unwrap_or_default()),
            ..reaction
        };
        if let Ok(change) = chat.read(|content| content.put_reaction(&key, &reaction)) {
            chat.operate(change.into());
        }
    });
}

#[component]
fn MessageRow(
    editor: Editor,
    chat: Chat,
    me: Uuid,
    id: ObjectId,
    row: ReadSignal<Row>,
    composer: Composer,
    unsent: Memo<HashMap<ObjectId, Unsent>>,
    block: Uuid,
    signing: Signal<Uuid>,
    attached: Attached,
) -> NodeId {
    let attachment_ids = create_memo(move || row.with(|row| row.attachments.clone()));
    let has_attachments = create_memo(move || attachment_ids.with(|ids| !ids.is_empty()));
    // A message of attachments alone has no line of text.
    let has_body = create_memo(move || row.with(|row| !row.body.is_empty()));
    let attachment_chip = move |attachment: Uuid| {
        view! {
            <AttachmentChip
                editor
                id={attachment}
                attached
                link_id={format!("chat.message.{id}.attachment.{attachment}")}
            />
        }
    };
    let proofs = create_memo(move || row.with(|row| row.proofs.clone()));
    // Who signed it, once the host says: nothing is marked until then.
    let signer = create_signal(None::<Signer>);
    create_effect(move || {
        let proofs = proofs.get();
        let chat = signing.get();
        signer.set(None);
        let host = editor.host();
        let asked = async move {
            let mut found = Vec::new();
            for proof in proofs {
                let Some(bytes) = proof.record.signed_bytes(chat) else {
                    found.push(Signer::Bad);
                    continue;
                };
                let verified = host.verify(proof.author, bytes, proof.signature);
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
    let actionable = create_memo(move || row.with(|row| !row.deleted));
    let deleted = create_memo(move || !actionable.get());
    let body_color = create_memo(move || match deleted.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
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
    let unsent = create_memo(move || unsent.with(|unsent| unsent.get(&id).copied()));
    let sending = create_memo(move || unsent.get().is_some());
    let failed = create_memo(move || unsent.get().is_some_and(|unsent| unsent.failed));
    let sending_now = create_memo(move || sending.get() && !failed.get());
    // A message the server refused is sent again as a new one, or given up; either way the
    // one that failed goes.
    let let_go = move |again: bool| {
        let Some(unsent) = unsent.get_untracked() else {
            return;
        };
        if again {
            let sent = chat.read(|content| {
                let message = content.message(id)?;
                let body = content.body(id)?;
                let again = Message {
                    attachments: message.attachments,
                    ..Message::new(me, now(), body).replying_to(message.reply_to)
                };
                Some(content.send(block, again).1)
            });
            if let Ok(Some(send)) = sent {
                chat.operate(send.into());
            }
        }
        // Given up, or sent as a new message: deleting it leaves the chat listing only what
        // another message carries, so a block made for it alone goes with it (to the trash, where
        // Restore finds it) and a referenced one stays where it is.
        delete(editor, chat, signing.get_untracked(), me, id);
        chat.dismiss_lacking(unsent.lacking);
    };
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
                    toggle_reaction(
                        editor,
                        chat,
                        signing.get_untracked(),
                        me,
                        id,
                        &reaction,
                        pressed.get_untracked(),
                    );
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
                    toggle_reaction(
                        editor,
                        chat,
                        signing.get_untracked(),
                        me,
                        id,
                        reaction,
                        mine,
                    );
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
                            <Show condition={sending_now}>
                                <Text
                                    style=TextStyle::CAPTION
                                    @test_id={format!("chat.message.{id}.sending")}
                                >
                                    "Sending"
                                </Text>
                            </Show>
                            <Show condition={failed}>
                                <Text
                                    style=TextStyle::CAPTION
                                    color=TextColor::Danger
                                    @test_id={format!("chat.message.{id}.not_sent")}
                                >
                                    "Not sent"
                                </Text>
                            </Show>
                            <Show condition={failed}>
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Xs
                                >
                                    <Button
                                        label="Send again"
                                        variant=ButtonVariant::Ghost
                                        size=ButtonSize::Small
                                        @test_id={format!("chat.message.{id}.retry")}
                                        on_click={move || let_go(true)}
                                    />
                                    <Button
                                        label="Discard"
                                        variant=ButtonVariant::Ghost
                                        size=ButtonSize::Small
                                        @test_id={format!("chat.message.{id}.discard")}
                                        on_click={move || let_go(false)}
                                    />
                                </List>
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
                    <Show condition={actionable}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
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
                        </List>
                    </Show>
                    <Show condition={mine}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                            <Show condition={has_body}>
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
                            </Show>
                            <Button
                                label="Delete"
                                glyph=ICON_TRASH
                                icon_only=true
                                variant=ButtonVariant::Ghost
                                size=ButtonSize::Small
                                @test_id={format!("chat.message.{id}.delete")}
                                on_click={move || {
                                    delete(editor, chat, signing.get_untracked(), me, id);
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
                <Show condition={has_body}>
                    <Text italic={deleted} color={body_color}>{body}</Text>
                </Show>
                <Show condition={has_attachments}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                        <ForEach keys={attachment_ids} view={attachment_chip} />
                    </List>
                </Show>
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

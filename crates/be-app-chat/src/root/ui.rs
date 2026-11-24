use std::collections::HashMap;
use std::rc::Rc;

use be_sdk_beui::be_block_content::be_block_model::BlockRef;
use be_sdk_beui::be_block_content::{Along, SettingsLayer, Signer};
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
use be_sdk_beui::{
    BlockFilter, BlockLink, BlockQuery, ChildTarget, ContentProjection, Editor, KeptEdit,
    LayeredSettings, PickedFile, utc_offset, watch_block_label,
};
use be_type_chat::{ChatContent, ChatEntry, ChatLog, ChatReads, Shown};
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

/// How many entries of the chat's log a read asks for: the newest as the editor opens, and
/// each earlier page as the person asks for it.
const PAGE: u64 = 100;

/// A row of the chat: a message in the log, by its place; one this device sent that waits for
/// the server, by its entry's id; or one the server refused, by its place among those.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum RowKey {
    Logged(u64),
    /// A message of the history an import brought, by its place in the block it was written in.
    History(u64),
    Kept(Uuid),
    Refused(u64),
}

impl std::fmt::Display for RowKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Logged(seq) => write!(formatter, "{seq}"),
            Self::History(seq) => write!(formatter, "history.{seq}"),
            Self::Kept(id) => write!(formatter, "kept.{id}"),
            Self::Refused(token) => write!(formatter, "refused.{token}"),
        }
    }
}

/// Where a row's message is.
#[derive(Clone, Debug, PartialEq)]
enum State {
    /// In the log, with who signed what it shows (`Shown::signer`).
    Logged(Signer),
    /// Deleted by its author, with a deletion whose key is worth saying: an old one, or one
    /// that changed. Deleted with a trusted key, a message shows no row.
    Deleted(Signer),
    /// Redacted by someone other than its author (a manager, the server).
    Removed,
    /// Waits on this device for the server.
    Sending,
    /// The server refused it, saying why: the token of the append that sent it.
    NotSent(String, u64),
}

/// What a message row shows, read from the log in one pass per change, so a row updates only
/// when what it shows changed.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    author: Uuid,
    mine: bool,
    time: String,
    /// When the server took it, for a message that arrived well after its claimed time: it was
    /// written offline.
    arrived: Option<String>,
    body: String,
    edited: bool,
    reply: Option<Quote>,
    reactions: Vec<Reacted>,
    /// The blocks sent with it.
    attachments: Vec<Uuid>,
    state: State,
    /// What this account did to it that the server refused (deletions, edits, reactions): what
    /// to say of them all, and the tokens of their appends, to dismiss together.
    undone: Option<(String, Vec<u64>)>,
}

#[derive(Clone, Debug, PartialEq)]
struct Reacted {
    reaction: String,
    count: usize,
    mine: bool,
}

/// How long after its claimed time a message must arrive to show as sent offline: past the
/// clock drift between a device and the server, and a slow connection's delay.
const OFFLINE_AFTER: i64 = 5 * 60 * 1000;

/// What a reply quotes: its author and the start of what it said, or that it was deleted.
#[derive(Clone, Debug, PartialEq)]
enum Quote {
    Message(Uuid, String),
    Deleted,
}

/// What the chat calls each account: "You", or a member by the name the host gives
/// (`Editor::members`): its contact card's, or the one the server gave, which the row marks with
/// an "unverified name" caption and a quote or "Replying to" with "(unverified name)" after it. An account the host has not named (before it answers, offline, or one that may no
/// longer view the chat) shows as "Member" and its account's first digits.
#[derive(Clone, Copy)]
struct Names {
    me: Uuid,
    members: Memo<HashMap<Uuid, (String, bool)>>,
}

impl Names {
    fn new(editor: Editor, me: Uuid) -> Self {
        let members = editor.members();
        Self {
            me,
            members: create_memo(move || {
                members
                    .get()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|member| !member.name.is_empty())
                    .map(|member| (member.account, (member.name, member.verified)))
                    .collect()
            }),
        }
    }

    fn name(&self, account: Uuid) -> String {
        if account == self.me {
            return "You".to_owned();
        }
        self.members.with(|members| match members.get(&account) {
            Some((name, _)) => name.clone(),
            None => format!("Member {}", &account.simple().to_string()[..4]),
        })
    }

    /// Whether `account` is named by what the server says rather than a contact card.
    fn unverified(&self, account: Uuid) -> bool {
        account != self.me
            && self
                .members
                .with(|members| members.get(&account).is_some_and(|(_, verified)| !verified))
    }

    /// The name a line of text calls `account` by, as a reply's quote and the composer's
    /// "Replying to" do: a name the server gave says so in it, as no caption follows it there.
    fn inline(&self, account: Uuid) -> String {
        let name = self.name(account);
        match self.unverified(account) {
            true => format!("{name} (unverified name)"),
            false => name,
        }
    }

    fn quote(&self, quote: &Quote) -> String {
        match quote {
            Quote::Message(author, excerpt) => format!("{}: {excerpt}", self.inline(*author)),
            Quote::Deleted => "A deleted message".to_owned(),
        }
    }
}

/// What the person is doing in the editor: none of it is in the block until it is sent.
#[derive(Clone, Copy)]
struct Composer {
    draft: Signal<String>,
    /// The message replied to, by its place.
    replying: Signal<Option<u64>>,
    editing: Signal<Option<u64>>,
    picking: Signal<Option<u64>>,
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
        chat.operate(draft);
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
        chat.operate(dropped);
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
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing=Space::Xs
            @test_id={link_id}
        >
            <Show
                value={move || target.get().is_some()}
                fallback={|| view! {
                    <NoAccess />
                }}
            >
                <BlockLink editor block={target} />
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
            {made.then(|| view! {
                <Text size=TextSize::Xs color=TextColor::Muted>"New"</Text>
            })}
        </List>
    }
}

#[component]
pub fn Chat(editor: Editor) -> NodeId {
    let chat: Chat = editor.block_content::<ChatContent>();
    let me = editor.blocks().account_id();
    let names = Names::new(editor, me);
    let block = editor.block_id();
    // The log as this editor holds it, folded; and what this device sent that the server has
    // not taken yet, or refused.
    let log = create_memo(move || chat.with_log(|items| ChatLog::of(items)));
    let kept = create_memo(move || {
        chat.with_kept(|kept| {
            kept.map(|(id, bytes)| (id, bytes.to_vec()))
                .collect::<Vec<_>>()
        })
    });
    let refused = create_memo(move || {
        chat.with_refused(|refused| {
            refused
                .map(|(token, bytes, why)| (token, bytes.to_vec(), why.to_owned()))
                .collect::<Vec<_>>()
        })
    });
    let first = create_memo(move || chat.with_log(|items| items.next().map(|item| item.seq)));
    // The newest page as the chat opens: a read from past the log's end names its length.
    let asked = std::cell::Cell::new((false, false));
    create_effect(move || {
        if chat.ready().is_err() {
            return;
        }
        let size = chat.log_size();
        let held = first.get().is_some();
        let (opened, tail) = asked.get();
        if !opened {
            asked.set((true, tail));
            untrack(|| chat.read_log(u64::MAX, 1));
        } else if size > 0 && !held && !tail {
            asked.set((true, true));
            untrack(|| chat.read_log(size.saturating_sub(PAGE), PAGE as u32));
        }
    });
    let has_older = create_memo(move || first.get().is_some_and(|first| first > 0));
    let load_older = move || {
        if let Some(first) = first.get_untracked() {
            let from = first.saturating_sub(PAGE);
            chat.read_log(from, (first - from) as u32);
        }
    };

    let rows: KeyedStore<RowKey, Row> = KeyedStore::new();
    create_effect(move || {
        let today = local_date(now());
        let mut list = Vec::new();
        // What the server refused of what this account did to a message shows on its row.
        // Several on one message say so in one caption, in the order they were refused.
        let mut undone: HashMap<u64, (String, Vec<u64>)> = HashMap::new();
        refused.with(|refused| {
            for (token, bytes, why) in refused {
                let (message, done) = match ChatEntry::decode(bytes) {
                    Some(ChatEntry::Delete { message, .. }) => (message, "Not deleted"),
                    Some(ChatEntry::Edit { message, .. }) => (message, "Edit not saved"),
                    Some(ChatEntry::React { message, .. }) => (message, "Reaction not saved"),
                    Some(ChatEntry::Message { .. }) | None => continue,
                };
                let said = format!("{done}: {why}");
                let (text, tokens) = undone.entry(message).or_default();
                if !text.is_empty() {
                    text.push_str("; ");
                }
                text.push_str(&said);
                tokens.push(*token);
            }
        });
        log.with(|log| {
            list.extend(log.messages().filter_map(|shown| {
                let mut row = row(log, shown, me, today)?;
                row.undone = undone.get(&shown.seq).cloned();
                Some((RowKey::Logged(shown.seq), row))
            }));
            kept.with(|kept| {
                list.extend(kept.iter().filter_map(|(id, bytes)| {
                    let row = unsent_row(log, bytes, me, State::Sending, today)?;
                    Some((RowKey::Kept(*id), row))
                }));
            });
            refused.with(|refused| {
                list.extend(
                    refused
                        .iter()
                        .filter_map(|(token, bytes, why)| {
                            unsent_row(log, bytes, me, State::NotSent(why.clone(), *token), today)
                        })
                        .enumerate()
                        .map(|(index, row)| (RowKey::Refused(index as u64), row)),
                );
            });
        });
        rows.reconcile_owned(list);
    });
    let ids = rows.keys();
    let empty = create_memo(move || ids.with(Vec::is_empty));
    // The history an import brought: read-only, as the file says it was written, with no
    // verdict on when (its times are the file's word).
    let history_rows: KeyedStore<RowKey, Row> = KeyedStore::new();
    create_effect(move || {
        let today = local_date(now());
        let list: Vec<(RowKey, Row)> = log.with(|log| {
            let Some(history) = log.history() else {
                return Vec::new();
            };
            history
                .log
                .messages()
                .filter_map(|shown| {
                    let mut row = row(&history.log, shown, me, today)?;
                    row.arrived = None;
                    Some((RowKey::History(shown.seq), row))
                })
                .collect()
        });
        history_rows.reconcile_owned(list);
    });
    let history_ids = history_rows.keys();
    let importer =
        create_memo(move || log.with(|log| log.history().map(|history| history.importer)));
    let has_history = create_memo(move || importer.get().is_some());
    let history_heading = create_memo(move || match importer.get() {
        Some(importer) => format!("Imported by {} from an export", names.name(importer)),
        None => String::new(),
    });
    let nothing = create_memo(move || empty.get() && !has_history.get());

    // Where each append went shows in the log and in what is sending or not sent, so its
    // answers are let go.
    create_effect(move || {
        chat.track();
        untrack(|| chat.take_appended());
    });
    // What a message of this account's deleted carries is no longer listed once the deletion is
    // in the log, by whichever of its editors is open: a deletion the server refused unlists
    // nothing.
    create_effect(move || {
        let deleted: Vec<(Uuid, i64)> = log.with(|log| {
            log.messages()
                .filter(|shown| shown.author == me)
                .filter_map(|shown| Some((shown.id, shown.deleted?.0)))
                .collect()
        });
        for (id, time) in deleted {
            if let Ok(Some(unlisted)) = chat.read(|content| content.delete(id, me, time)) {
                untrack(|| chat.operate(unlisted));
            }
        }
    });
    // What a message of this account's carries is listed as the message's once its entry has an
    // id (`ChatContent::send`): in the log, or waiting on the device. One sent by an editor that
    // closed before its append was answered is listed by the next one open.
    create_effect(move || {
        let Ok(drafts) = chat.read(|content| content.drafts_of(me)) else {
            return;
        };
        if drafts.is_empty() {
            return;
        }
        let drafted = |attachments: &[BlockRef]| {
            attachments
                .iter()
                .any(|attachment| drafts.contains(&attachment.0))
        };
        let mut sent: Vec<(Uuid, i64, Vec<BlockRef>)> = log.with(|log| {
            log.messages()
                .filter(|shown| shown.author == me && drafted(&shown.attachments))
                .map(|shown| (shown.id, shown.time, shown.attachments.clone()))
                .collect()
        });
        chat.with_kept(|kept| {
            for (id, bytes) in kept {
                if let Some(ChatEntry::Message {
                    time, attachments, ..
                }) = ChatEntry::decode(bytes)
                    && drafted(&attachments)
                {
                    sent.push((id, time, attachments));
                }
            }
        });
        for (id, time, attachments) in sent {
            if let Ok(Some(edit)) = chat.read(|content| content.send(id, me, time, &attachments)) {
                untrack(|| chat.operate(edit));
            }
        }
    });

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
        chat.track();
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
    // A message replied to, being edited or reacted to can be deleted or removed, here or
    // elsewhere.
    create_effect(move || {
        log.with(|log| {
            let gone = |seq: Option<u64>| {
                seq.is_some_and(|seq| {
                    log.message(seq)
                        .is_none_or(|shown| shown.deleted.is_some() || shown.removed)
                })
            };
            if gone(composer.replying.get_untracked()) {
                composer.replying.set(None);
            }
            if gone(composer.editing.get_untracked()) {
                composer.editing.set(None);
                composer.draft.set(String::new());
            }
            if gone(composer.picking.get_untracked()) {
                composer.picking.set(None);
            }
        });
    });
    let send = move || submit(chat, log, me, composer, attached);
    let scroll = ComponentRef::<ScrollApi>::new();
    let at_end = create_signal(true);
    let unread = unread_count(editor, chat, log, block, me, at_end);
    let replying = create_memo(move || {
        let seq = composer.replying.get()?;
        let (author, body) = log.with(|log| {
            log.message(seq).map(|shown| {
                (
                    shown.author,
                    excerpt(shown.body.as_deref().unwrap_or_default()),
                )
            })
        })?;
        Some(format!("Replying to {}: {body}", names.inline(author)))
    });
    let editing = create_memo(move || composer.editing.get().is_some());

    let history_message_rows = {
        move |key: RowKey| {
            let shown = history_rows.get(&key);
            view! {
                <MessageRow chat={chat} editor me names key row={shown} composer log attached />
            }
        }
    };
    let message_rows = {
        move |key: RowKey| {
            let shown = rows.get(&key);
            view! {
                <MessageRow chat={chat} editor me names key row={shown} composer log attached />
            }
        }
    };

    view! {
        <List spacing=Space::None>
            <ListItem grow=1.0>
                <Scroll
                    api={scroll}
                    follow_end=true
                    on_change={move |position: ScrollPosition| {
                        at_end.set(position.offset >= position.max_offset() - 1.0);
                    }}
                >
                    <Frame padding={Padding::all(Space::Xl)}>
                        <List spacing=Space::Md>
                            <Show value={nothing}>
                                <Text style=TextStyle::CAPTION>
                                    "No messages yet. Say hello below."
                                </Text>
                            </Show>
                            <Show value={has_older}>
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
                                        on_click={load_older}
                                    />
                                </List>
                            </Show>
                            <Show value={has_history}>
                                <Text style=TextStyle::CAPTION @test_id={"chat.history"}>
                                    {history_heading}
                                </Text>
                                <ForEach value={history_ids} view={history_message_rows} />
                                <Separator />
                            </Show>
                            <ForEach value={ids} view={message_rows} />
                        </List>
                    </Frame>
                </Scroll>
            </ListItem>
            <Separator />
            <Frame padding={Padding::all(Space::Lg)}>
                <List spacing=Space::Sm>
                    <Show
                        value={move || unread.get().filter(|count| *count > 0)}
                    >|count: ReadSignal<usize>|
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                            <Button
                                label={move || match count.get() {
                                    1 => "1 new message".to_owned(),
                                    count => format!("{count} new messages"),
                                }}
                                glyph=ICON_ARROW_DOWN
                                variant=ButtonVariant::Secondary
                                size=ButtonSize::Small
                                @test_id={"chat.unread"}
                                on_click={move || scroll.get().scroll_to_end()}
                            />
                        </List>
                    </Show>
                    <Show value={replying}>|reply_label: ReadSignal<String>|
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                            <ListItem grow=1.0>
                                <Text style=TextStyle::CAPTION @test_id={"chat.replying"}>
                                    {reply_label}
                                </Text>
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
                    <Show value={editing}>
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
                    <Show value={has_attaching}>
                        <List spacing=Space::Xs>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Sm
                            >
                                <ForEach value={attaching_ids} view={attaching_chip} />
                            </List>
                            <Show value={makes_children}>
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
                            label={move || match editing.get() {
                                true => "Save".to_owned(),
                                false => "Send".to_owned(),
                            }}
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

/// How many messages others sent since this account last read the chat, among the entries
/// this editor holds, none until its owner-only `ChatReads` loaded. Each device keeps its own
/// place there and the furthest counts, so every device the account uses agrees. The chat is
/// read while it is scrolled to its newest message, shown, and its own view has the keyboard: a
/// chat left open behind others, or beside the one being typed in, is not.
fn unread_count(
    editor: Editor,
    chat: Chat,
    log: Memo<ChatLog>,
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
        let place = read.with(|reads| reads.place(block));
        Some(log.with(|log| log.unread(place, me)))
    });
    let shown = editor.presence_visible();
    let keyboard = editor.has_keyboard();
    create_effect(move || {
        let reading = at_end.get() && shown.get() && keyboard.get();
        if !reading || unread.get().unwrap_or_default() == 0 {
            return;
        }
        // Tracked, so a write the settings were not loaded for is made once they are.
        let place = read.with(|reads| reads.place(block));
        let Some(last) = untrack(|| chat.with_log(|items| items.last().map(|item| item.seq)))
        else {
            return;
        };
        if place.is_some_and(|place| place >= last) {
            return;
        }
        let edit: KeptEdit<ChatReads> = Rc::new(move |_| ChatReads::set_read(block, device, last));
        // Not yet loaded, it is written on the next change instead.
        let _ = reads.write_all(SettingsLayer::EveryDevice, &[edit]);
    });
    unread
}

/// The row of a message in the log, or none for one that shows no row: deleted by its author
/// with a key worth nothing to say about, or one no key this device holds opens. An entry
/// someone else redacted shows in its place, saying so.
fn row(log: &ChatLog, shown: &Shown, me: Uuid, today: i64) -> Option<Row> {
    let mine = shown.author == me;
    if shown.removed {
        return Some(Row {
            author: shown.author,
            mine,
            time: time(shown.arrival, today),
            arrived: None,
            body: "Removed, not by its author".to_owned(),
            edited: false,
            reply: None,
            reactions: Vec::new(),
            attachments: Vec::new(),
            state: State::Removed,
            undone: None,
        });
    }
    if let Some((_, signer)) = shown.deleted {
        return matches!(signer, Signer::Changed | Signer::Replaced | Signer::Bad).then(|| Row {
            author: shown.author,
            mine,
            time: time(shown.time, today),
            arrived: None,
            body: "Message deleted".to_owned(),
            edited: false,
            reply: None,
            reactions: Vec::new(),
            attachments: Vec::new(),
            state: State::Deleted(signer),
            undone: None,
        });
    }
    let body = shown.body.clone()?;
    let reactions = shown
        .reactions
        .iter()
        .map(|(reaction, authors)| Reacted {
            reaction: reaction.clone(),
            count: authors.len(),
            mine: authors.contains(&me),
        })
        .collect();
    Some(Row {
        author: shown.author,
        mine,
        time: time(shown.time, today),
        arrived: (shown.arrival - shown.time > OFFLINE_AFTER)
            .then(|| format!("sent offline, arrived {}", time(shown.arrival, today))),
        body,
        edited: shown.edited,
        reply: shown.reply_to.map(|quoted| quote(log, quoted)),
        reactions,
        attachments: shown
            .attachments
            .iter()
            .map(|attachment| attachment.0)
            .collect(),
        state: State::Logged(shown.signer),
        undone: None,
    })
}

/// The row of a message this device sent that is not in the log: waiting for the server, or
/// refused. None for an entry that is no message (an edit, a reaction), which shows no row.
fn unsent_row(log: &ChatLog, bytes: &[u8], me: Uuid, state: State, today: i64) -> Option<Row> {
    let ChatEntry::Message {
        time: written,
        body,
        reply_to,
        attachments,
    } = ChatEntry::decode(bytes)?
    else {
        return None;
    };
    Some(Row {
        author: me,
        mine: true,
        time: time(written, today),
        arrived: None,
        body,
        edited: false,
        reply: reply_to.map(|quoted| quote(log, quoted)),
        reactions: Vec::new(),
        attachments: attachments.iter().map(|attachment| attachment.0).collect(),
        state,
        undone: None,
    })
}

fn quote(log: &ChatLog, quoted: u64) -> Quote {
    match log
        .message(quoted)
        .filter(|shown| shown.deleted.is_none())
        .and_then(|shown| Some((shown.author, shown.body.as_deref()?)))
    {
        Some((author, body)) => Quote::Message(author, excerpt(body)),
        None => Quote::Deleted,
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

/// The longest body sent, well within what an entry carries.
const MAX_BODY_BYTES: usize = 32 * 1024;

/// A key for what a sent message announces, so a send made again tells once: the account and
/// a count of the sends this editor made, with the time.
fn announcement_key(me: Uuid, time: i64) -> String {
    thread_local! {
        static SENT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }
    let count = SENT.with(|sent| {
        sent.set(sent.get() + 1);
        sent.get()
    });
    format!("{}.{time}.{count}", me.simple())
}

/// Appends a message, which the host signs as the person's, telling the chat's members who
/// are away, and mentioning the author of the message it replies to. What it carries is listed
/// as the message's once its entry has an id.
fn send_message(chat: Chat, log: Memo<ChatLog>, me: Uuid, entry: ChatEntry) {
    let ChatEntry::Message { time, reply_to, .. } = &entry else {
        return;
    };
    let mentions: Vec<Uuid> = reply_to
        .and_then(|quoted| log.with_untracked(|log| log.message(quoted).map(|shown| shown.author)))
        .filter(|author| *author != me)
        .into_iter()
        .collect();
    let along = Along {
        announcement: entry.announcement(announcement_key(me, *time), mentions),
        redacts: None,
    };
    chat.append(entry.encode(), along);
}

/// Sends the draft, or saves the edit.
fn submit(chat: Chat, log: Memo<ChatLog>, me: Uuid, composer: Composer, attached: Attached) {
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
    let attachments: Vec<BlockRef> = pending.iter().map(|pending| BlockRef(pending.id)).collect();
    if (body.is_empty() && attachments.is_empty()) || body.len() > MAX_BODY_BYTES {
        return;
    }
    match editing {
        Some(message) => {
            let edit = ChatEntry::Edit {
                message,
                time: now(),
                body,
            };
            chat.append(edit.encode(), Along::default());
            composer.editing.set(None);
        }
        None => {
            let message = ChatEntry::Message {
                time: now(),
                body,
                reply_to: composer.replying.get_untracked(),
                attachments,
            };
            send_message(chat, log, me, message);
            composer.replying.set(None);
            composer.attaching.set(Vec::new());
        }
    }
    composer.draft.set(String::new());
}

/// Deletes the message at `seq`: a deletion entry, which redacts the message once the server
/// takes it. What it carries is no longer listed once the deletion is in the log.
fn delete(chat: Chat, seq: u64) {
    let deletion = ChatEntry::Delete {
        message: seq,
        time: now(),
    };
    chat.append(
        deletion.encode(),
        Along {
            announcement: None,
            redacts: Some(seq),
        },
    );
}

/// Reacts to the message at `seq`, or takes `me`'s reaction back, which redacts the record that
/// kept it.
fn toggle_reaction(chat: Chat, log: Memo<ChatLog>, me: Uuid, seq: u64, reaction: &str) {
    let held = log.with_untracked(|log| log.reaction(seq, me, reaction));
    let kept = !held.is_some_and(|(_, kept)| kept);
    let redacts = held.filter(|(_, kept)| *kept).map(|(place, _)| place);
    let entry = ChatEntry::React {
        message: seq,
        reaction: reaction.to_owned(),
        kept,
        takes_back: redacts,
        time: now(),
    };
    chat.append(
        entry.encode(),
        Along {
            announcement: None,
            redacts,
        },
    );
}

#[component]
fn MessageRow(
    chat: Chat,
    editor: Editor,
    me: Uuid,
    names: Names,
    key: RowKey,
    row: ReadSignal<Row>,
    composer: Composer,
    log: Memo<ChatLog>,
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
                link_id={format!("chat.message.{key}.attachment.{attachment}")}
            />
        }
    };
    let state = create_memo(move || row.with(|row| row.state.clone()));
    let signer = create_memo(move || match state.get() {
        State::Logged(signer) | State::Deleted(signer) => Some(signer),
        State::Removed | State::Sending | State::NotSent(..) => None,
    });
    let unverified = create_memo(move || signer.get() == Some(Signer::Unverified));
    let changed = create_memo(move || signer.get() == Some(Signer::Changed));
    let replaced = create_memo(move || signer.get() == Some(Signer::Replaced));
    let forged = create_memo(move || signer.get() == Some(Signer::Bad));
    let old_key = create_memo(move || match matches!(state.get(), State::Deleted(_)) {
        true => "deleted with an old key".to_owned(),
        false => "sent with an old key".to_owned(),
    });
    let account = create_memo(move || row.with(|row| row.author));
    let author = create_memo(move || names.name(account.get()));
    let name_unverified = create_memo(move || names.unverified(account.get()));
    // A name the server gave says so already; one caption is enough for a key not verified too.
    let key_unverified = create_memo(move || unverified.get() && !name_unverified.get());
    let time = create_memo(move || row.with(|row| row.time.clone()));
    let arrived = create_memo(move || row.with(|row| row.arrived.clone().unwrap_or_default()));
    let late = create_memo(move || row.with(|row| row.arrived.is_some()));
    let body = create_memo(move || row.with(|row| row.body.clone()));
    let edited = create_memo(move || row.with(|row| row.edited));
    let mine = create_memo(move || row.with(|row| row.mine));
    let deleted = create_memo(move || matches!(state.get(), State::Deleted(_) | State::Removed));
    // Only a message in the log takes a reply, a reaction, an edit or a deletion.
    let logged = match key {
        RowKey::Logged(seq) => Some(seq),
        RowKey::History(_) | RowKey::Kept(_) | RowKey::Refused(_) => None,
    };
    let actionable = create_memo(move || logged.is_some() && !deleted.get());
    let own_actions = create_memo(move || actionable.get() && mine.get());
    let quote = create_memo(move || {
        row.with(|row| {
            row.reply
                .as_ref()
                .map(|quote| names.quote(quote))
                .unwrap_or_default()
        })
    });
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
    let picking = create_memo(move || logged.is_some() && composer.picking.get() == logged);
    let sending = create_memo(move || matches!(state.get(), State::Sending));
    let failed = create_memo(move || matches!(state.get(), State::NotSent(..)));
    let undone = create_memo(move || row.with(|row| row.undone.clone()));
    let has_undone = create_memo(move || undone.with(Option::is_some));
    let undone_text =
        create_memo(move || undone.with(|undone| undone.clone().unwrap_or_default().0));
    let dismiss_undone = move || {
        if let Some((_, tokens)) = undone.get_untracked() {
            for token in tokens {
                chat.dismiss_refused(token);
            }
        }
    };
    // A message the server refused is sent again as a new one, or given up; either way the
    // one that failed goes.
    let let_go = move |again: bool| {
        let State::NotSent(_, token) = state.get_untracked() else {
            return;
        };
        if again {
            let entry = chat.with_refused(|refused| {
                for (refused, bytes, _) in refused {
                    if refused == token {
                        return ChatEntry::decode(bytes);
                    }
                }
                None
            });
            if let Some(entry) = entry {
                send_message(chat, log, me, entry);
            }
        }
        chat.dismiss_refused(token);
    };

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
                    if let Some(seq) = logged {
                        toggle_reaction(chat, log, me, seq, &reaction);
                    }
                })}
                accessibility={accessible}
                @test_id={format!("chat.message.{key}.reaction.{name_for_id}")}
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
                @test_id={format!("chat.message.{key}.react.{reaction}")}
                on_click={move || {
                    if let Some(seq) = logged {
                        toggle_reaction(chat, log, me, seq, reaction);
                    }
                    composer.picking.set(None);
                }}
            />
        }
    };
    let picks = create_memo(|| (0..REACTIONS.len()).collect::<Vec<usize>>());

    view! {
        <Frame
            fill={move || match mine.get() {
                true => ColorRole::AccentSoft,
                false => ColorRole::Control,
            }}
            radius=Radius::Md
            padding={Padding::symmetric(Space::Lg, Space::Md)}
            @test_id={format!("chat.message.{key}")}
        >
            <List spacing=Space::Sm>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                    <ListItem grow=1.0>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                            <Text weight=Weight::Bold>{author}</Text>
                            <Show value={name_unverified}>
                                <Text
                                    style=TextStyle::CAPTION
                                    @test_id={format!("chat.message.{key}.name_unverified")}
                                >
                                    "unverified name"
                                </Text>
                            </Show>
                            <Text style=TextStyle::CAPTION>{time}</Text>
                            <Show value={late}>
                                <Text
                                    style=TextStyle::CAPTION
                                    @test_id={format!("chat.message.{key}.arrived")}
                                >
                                    {arrived}
                                </Text>
                            </Show>
                            <Show value={edited}>
                                <Text style=TextStyle::CAPTION>"edited"</Text>
                            </Show>
                            <Show value={sending}>
                                <Text
                                    style=TextStyle::CAPTION
                                    @test_id={format!("chat.message.{key}.sending")}
                                >
                                    "Sending"
                                </Text>
                            </Show>
                            <Show value={failed}>
                                <Text
                                    style=TextStyle::CAPTION
                                    color=TextColor::Danger
                                    @test_id={format!("chat.message.{key}.not_sent")}
                                >
                                    "Not sent"
                                </Text>
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Xs
                                >
                                    <Button
                                        label="Send again"
                                        variant=ButtonVariant::Ghost
                                        size=ButtonSize::Small
                                        @test_id={format!("chat.message.{key}.retry")}
                                        on_click={move || let_go(true)}
                                    />
                                    <Button
                                        label="Discard"
                                        variant=ButtonVariant::Ghost
                                        size=ButtonSize::Small
                                        @test_id={format!("chat.message.{key}.discard")}
                                        on_click={move || let_go(false)}
                                    />
                                </List>
                            </Show>
                            <Show value={key_unverified}>
                                <Text style=TextStyle::CAPTION>"not verified"</Text>
                            </Show>
                            <Show value={changed}>
                                <Text style=TextStyle::CAPTION color=TextColor::Warning>
                                    "key changed"
                                </Text>
                            </Show>
                            <Show value={replaced}>
                                <Text style=TextStyle::CAPTION color=TextColor::Warning>
                                    {old_key}
                                </Text>
                            </Show>
                            <Show value={forged}>
                                <Text style=TextStyle::CAPTION color=TextColor::Danger>
                                    "forged"
                                </Text>
                            </Show>
                        </List>
                    </ListItem>
                    <Show value={actionable}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                            <Button
                                label="Reply"
                                glyph=ICON_REPLY
                                icon_only=true
                                variant=ButtonVariant::Ghost
                                size=ButtonSize::Small
                                @test_id={format!("chat.message.{key}.reply")}
                                on_click={move || {
                                    if composer.editing.get_untracked().is_some() {
                                        composer.editing.set(None);
                                        composer.draft.set(String::new());
                                    }
                                    composer.replying.set(logged);
                                }}
                            />
                            <Button
                                label="React"
                                glyph=ICON_STICKER
                                icon_only=true
                                variant=ButtonVariant::Ghost
                                size=ButtonSize::Small
                                @test_id={format!("chat.message.{key}.react")}
                                on_click={move || {
                                    let open = composer.picking.get_untracked() == logged;
                                    composer.picking.set(if open { None } else { logged });
                                }}
                            />
                        </List>
                    </Show>
                    <Show value={own_actions}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                            <Show value={has_body}>
                                <Button
                                    label="Edit"
                                    glyph=ICON_PENCIL
                                    icon_only=true
                                    variant=ButtonVariant::Ghost
                                    size=ButtonSize::Small
                                    @test_id={format!("chat.message.{key}.edit")}
                                    on_click={move || {
                                        composer.replying.set(None);
                                        composer.editing.set(logged);
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
                                @test_id={format!("chat.message.{key}.delete")}
                                on_click={move || {
                                    if let Some(seq) = logged {
                                        delete(chat, seq);
                                        if composer.editing.get_untracked() == Some(seq) {
                                            composer.editing.set(None);
                                            composer.draft.set(String::new());
                                        }
                                    }
                                }}
                            />
                        </List>
                    </Show>
                </List>
                <Show value={replies}>
                    <Frame
                        fill=ColorRole::Shade
                        radius=Radius::Sm
                        padding={Padding::symmetric(Space::Md, Space::Xs)}
                    >
                        <Text style=TextStyle::CAPTION>{quote}</Text>
                    </Frame>
                </Show>
                <Show value={has_body}>
                    <Text
                        italic={deleted}
                        color={move || match deleted.get() {
                            true => TextColor::Muted,
                            false => TextColor::Default,
                        }}
                    >
                        {body}
                    </Text>
                </Show>
                <Show value={has_undone}>
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=Space::Xs
                        wrap=true
                    >
                        <Text
                            style=TextStyle::CAPTION
                            color=TextColor::Danger
                            @test_id={format!("chat.message.{key}.undone")}
                        >
                            {undone_text}
                        </Text>
                        <Button
                            label="Dismiss"
                            variant=ButtonVariant::Ghost
                            size=ButtonSize::Small
                            @test_id={format!("chat.message.{key}.dismiss")}
                            on_click={dismiss_undone}
                        />
                    </List>
                </Show>
                <Show value={has_attachments}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                        <ForEach value={attachment_ids} view={attachment_chip} />
                    </List>
                </Show>
                <Show value={reacted}>
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=Space::Sm
                        wrap=true
                    >
                        <ForEach value={reactions} view={reaction_chips} />
                    </List>
                </Show>
                <Show value={picking}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                        <ForEach value={picks} view={picker} />
                    </List>
                </Show>
            </List>
        </Frame>
    }
}

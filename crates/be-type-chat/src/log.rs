use std::collections::{BTreeMap, BTreeSet};

use be_block_content::{Announcement, LogItem, LogSigner};
use be_block_model::BlockRef;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An entry of a chat's signed log, each its author's: who wrote it, when the server took it
/// and who signed it are the log's (`LogItem`), so an entry says only what its author wrote.
/// A message is named by its place in the log.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChatEntry {
    Message {
        /// When its author says it was written, in milliseconds since the Unix epoch.
        time: i64,
        body: String,
        reply_to: Option<u64>,
        /// Child blocks of the chat (`ChatContent::send` lists them).
        attachments: Vec<BlockRef>,
    },
    /// What the message says now: only its author's count.
    Edit {
        message: u64,
        time: i64,
        body: String,
    },
    /// A reaction kept or taken back, `reaction` naming a Lucide icon: each person's latest
    /// record of each reaction counts. Taking one back names the record that kept it
    /// (`takes_back`), which is redacted, so the log says who took it out.
    React {
        message: u64,
        reaction: String,
        kept: bool,
        takes_back: Option<u64>,
        time: i64,
    },
    /// Its author's deletion of the message, whose payload is then redacted: only the
    /// message's author's counts.
    Delete { message: u64, time: i64 },
}

impl ChatEntry {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("a chat entry always encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        postcard::from_bytes(bytes).ok()
    }

    /// What a sent message tells the chat's members who are away: its body, under `key`
    /// (one the editor makes, so a send made again tells once), mentioning `mentions` (the
    /// author of the message it replies to), whom a member who wants only mentions is told
    /// for. None for an entry that is no message.
    pub fn announcement(&self, key: String, mentions: Vec<Uuid>) -> Option<Announcement> {
        let ChatEntry::Message { body, .. } = self else {
            return None;
        };
        Some(Announcement {
            key,
            text: body.clone(),
            several: "{count} new messages".to_owned(),
            mentions,
        })
    }
}

/// A message as the chat shows it: what its author sent, with their latest edit, the
/// reactions kept, and their deletion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shown {
    pub seq: u64,
    pub id: Uuid,
    pub author: Uuid,
    /// When the server took it.
    pub arrival: i64,
    /// When its author says it was written.
    pub time: i64,
    /// What it says now; none once redacted, or while no key this device holds opens it.
    pub body: Option<String>,
    pub edited: bool,
    pub reply_to: Option<u64>,
    pub attachments: Vec<BlockRef>,
    /// Each reaction kept, by name, with who keeps it.
    pub reactions: Vec<(String, Vec<Uuid>)>,
    /// Who signed what it shows, the least trusted of the message and the edit it shows.
    pub signer: LogSigner,
    /// Its author's deletion: when, and who signed it.
    pub deleted: Option<(i64, LogSigner)>,
    /// Redacted by someone else (a manager, the server), not taken back by its author: what it
    /// was is gone, so it may not have been a message at all.
    pub removed: bool,
}

/// What the entries of a chat's log an editor holds show: each message by its place, as its
/// author's own entries make it. Others' edits and deletions count for nothing, and an entry
/// signed as nobody's (`Bad`) changes nothing, though a message so signed still shows, marked.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChatLog {
    messages: BTreeMap<u64, Shown>,
    /// Each reaction's latest record, by message, author and reaction: the place it was made
    /// at, and whether it is kept.
    reactions: BTreeMap<(u64, Uuid, String), (u64, bool)>,
    /// Entries with no payload to read, by place.
    unopened: BTreeMap<u64, LogItem>,
    /// The places of redacted entries a later entry of their author's names, deleting or taking
    /// them back: any other redacted entry was removed by someone else, and shows so.
    taken_back: BTreeSet<u64>,
}

impl ChatLog {
    /// The fold of `items`, in the log's order.
    pub fn of<'a>(items: impl IntoIterator<Item = &'a LogItem>) -> Self {
        let mut log = Self::default();
        for item in items {
            log.take(item);
        }
        log.show_removed();
        log.group_reactions();
        log
    }

    fn take(&mut self, item: &LogItem) {
        let entry = item.payload.as_deref().map(ChatEntry::decode);
        match entry {
            Some(Some(ChatEntry::Message {
                time,
                body,
                reply_to,
                attachments,
            })) => {
                self.messages.insert(
                    item.seq,
                    Shown {
                        seq: item.seq,
                        id: item.id,
                        author: item.author,
                        arrival: item.arrival,
                        time,
                        body: Some(body),
                        edited: false,
                        reply_to,
                        attachments,
                        reactions: Vec::new(),
                        signer: item.signer,
                        deleted: None,
                        removed: false,
                    },
                );
            }
            // Redacted (a deleted message, a reaction taken back, or removed by someone else),
            // or not opened here: kept by place, and shown as the message its author's deletion
            // names, or as removed (`show_removed`).
            None | Some(None) => {
                self.unopened.insert(item.seq, item.clone());
            }
            _ if item.signer == LogSigner::Bad => {}
            Some(Some(ChatEntry::Edit { message, body, .. })) => {
                if let Some(shown) = self.authored(message, item.author) {
                    shown.body = Some(body);
                    shown.edited = true;
                    shown.signer = least_trusted(shown.signer, item.signer);
                }
            }
            Some(Some(ChatEntry::React {
                message,
                reaction,
                kept,
                takes_back,
                ..
            })) => {
                if let Some(taken) = takes_back {
                    self.take_back(taken, item);
                }
                if self.messages.contains_key(&message) {
                    self.reactions
                        .insert((message, item.author, reaction), (item.seq, kept));
                }
            }
            Some(Some(ChatEntry::Delete { message, time })) => {
                // One a take-back named already (a reaction taken back) was no message.
                let taken = self.taken_back.contains(&message);
                self.take_back(message, item);
                if let Some(gone) = self
                    .unopened
                    .get(&message)
                    .filter(|gone| !taken && gone.author == item.author)
                {
                    self.messages.insert(
                        message,
                        Shown {
                            seq: gone.seq,
                            id: gone.id,
                            author: gone.author,
                            arrival: gone.arrival,
                            time: gone.arrival,
                            body: None,
                            edited: false,
                            reply_to: None,
                            attachments: Vec::new(),
                            reactions: Vec::new(),
                            signer: gone.signer,
                            deleted: None,
                            removed: false,
                        },
                    );
                }
                if let Some(shown) = self.authored(message, item.author) {
                    shown.deleted = Some((time, item.signer));
                    shown.attachments.clear();
                }
            }
        }
    }

    /// Notes that `item` names the redacted entry at `seq` as its author's to take back; one
    /// naming another author's entry counts for nothing.
    fn take_back(&mut self, seq: u64, item: &LogItem) {
        if seq < item.seq
            && self
                .unopened
                .get(&seq)
                .is_some_and(|taken| taken.author == item.author)
        {
            self.taken_back.insert(seq);
        }
    }

    /// Shows each entry redacted by someone other than its author in its place: no later entry
    /// of its author's takes it back.
    fn show_removed(&mut self) {
        for (seq, gone) in &self.unopened {
            if gone.signer != LogSigner::Redacted
                || self.taken_back.contains(seq)
                || self.messages.contains_key(seq)
            {
                continue;
            }
            self.messages.insert(
                *seq,
                Shown {
                    seq: *seq,
                    id: gone.id,
                    author: gone.author,
                    arrival: gone.arrival,
                    time: gone.arrival,
                    body: None,
                    edited: false,
                    reply_to: None,
                    attachments: Vec::new(),
                    reactions: Vec::new(),
                    signer: gone.signer,
                    deleted: None,
                    removed: true,
                },
            );
        }
    }

    /// The message at `seq`, if `author` wrote it.
    fn authored(&mut self, seq: u64, author: Uuid) -> Option<&mut Shown> {
        self.messages
            .get_mut(&seq)
            .filter(|shown| shown.author == author)
    }

    fn group_reactions(&mut self) {
        for ((message, author, reaction), (_, kept)) in &self.reactions {
            if !kept {
                continue;
            }
            let Some(shown) = self.messages.get_mut(message) else {
                continue;
            };
            match shown
                .reactions
                .iter_mut()
                .find(|(name, _)| name == reaction)
            {
                Some((_, by)) => by.push(*author),
                None => shown.reactions.push((reaction.clone(), vec![*author])),
            }
        }
    }

    /// The messages, in the log's order, deleted ones too.
    pub fn messages(&self) -> impl Iterator<Item = &Shown> {
        self.messages.values()
    }

    pub fn message(&self, seq: u64) -> Option<&Shown> {
        self.messages.get(&seq)
    }

    /// The place of the newest message.
    pub fn newest(&self) -> Option<u64> {
        self.messages.keys().next_back().copied()
    }

    /// The place of `author`'s latest record of `reaction` on the message at `seq`, and whether
    /// it keeps the reaction: taking one back redacts the record that kept it.
    pub fn reaction(&self, seq: u64, author: Uuid, reaction: &str) -> Option<(u64, bool)> {
        self.reactions
            .get(&(seq, author, reaction.to_owned()))
            .copied()
    }

    /// How many messages others sent that `me` has not read: those after the furthest place
    /// read (`place`, the last place read) and after `me`'s own last message, since answering
    /// is reading. Deleted and removed ones count for nothing, nor do forged ones (`Bad`), which
    /// answer nothing either.
    pub fn unread(&self, place: Option<u64>, me: Uuid) -> usize {
        let own = self
            .messages
            .values()
            .filter(|shown| shown.author == me && !shown.removed && shown.signer != LogSigner::Bad)
            .map(|shown| shown.seq)
            .next_back();
        let from = match (place, own) {
            (Some(place), Some(own)) => Some(place.max(own)),
            (place, own) => place.or(own),
        };
        self.messages
            .range(from.map_or(0, |from| from + 1)..)
            .filter(|(_, shown)| {
                shown.author != me
                    && shown.deleted.is_none()
                    && shown.body.is_some()
                    && shown.signer != LogSigner::Bad
            })
            .count()
    }
}

/// The less trusted of two verdicts on what one row shows.
pub fn least_trusted(one: LogSigner, other: LogSigner) -> LogSigner {
    fn rank(signer: LogSigner) -> u8 {
        match signer {
            LogSigner::Verified => 0,
            LogSigner::Unverified => 1,
            LogSigner::Unknown => 2,
            LogSigner::Changed => 3,
            LogSigner::Redacted => 4,
            LogSigner::Replaced => 5,
            LogSigner::Bad => 6,
        }
    }
    match rank(other) > rank(one) {
        true => other,
        false => one,
    }
}

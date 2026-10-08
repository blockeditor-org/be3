use std::collections::VecDeque;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{Duration, Instant};

use text_editor_core::{
    ChangeLog, CursorPosition, Document, DocumentEdit, DocumentRead, Pos, Position, SeqOp,
    Sequence, TextChange, TextIndentation, TextLanguage, anchor_in, anchor_index_in, changed,
    deleted_anchor_index_in,
};

mod page;

pub(crate) use page::CollaborationPage;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Side {
    Left,
    Right,
}

impl Side {
    const fn index(self) -> usize {
        match self {
            Side::Left => 0,
            Side::Right => 1,
        }
    }

    const fn client(self) -> u64 {
        match self {
            Side::Left => 1,
            Side::Right => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Caret {
    pub anchor: Position,
    pub focus: Position,
}

#[derive(Clone, Debug)]
enum Message {
    Edit(Side, SeqOp<u8>),
    Caret(Caret),
}

struct Group {
    undo: Vec<SeqOp<u8>>,
    redo: Vec<SeqOp<u8>>,
    cursors: Vec<CursorPosition>,
}

struct Peer {
    confirmed: Sequence<u8>,
    visible: Sequence<u8>,
    pending: VecDeque<SeqOp<u8>>,
    language: TextLanguage,
    indentation: TextIndentation,
    revision: u64,
    changes: ChangeLog,
    external: bool,
    undo: Vec<Group>,
    redo: Vec<Group>,
    group_open: bool,
    seen: Option<Caret>,
}

impl Peer {
    fn new(text: &str) -> Self {
        let confirmed = Sequence::from_items(text.as_bytes().to_vec());
        let mut visible = confirmed.clone();
        visible.mirror();
        Self {
            confirmed,
            visible,
            pending: VecDeque::new(),
            language: TextLanguage::Markdown,
            indentation: TextIndentation::default(),
            revision: 0,
            changes: ChangeLog::default(),
            external: false,
            undo: Vec::new(),
            redo: Vec::new(),
            group_open: false,
            seen: None,
        }
    }

    fn bump(&mut self, change: Option<TextChange>) {
        self.revision += 1;
        self.changes.record(self.revision, change);
    }

    fn rebuild(&mut self) -> TextChange {
        let before = self.visible.items();
        let mut visible = self.confirmed.clone();
        visible.mirror();
        for op in &self.pending {
            visible.apply(op);
        }
        self.visible = visible;
        difference(&before, self.visible.mirrored().unwrap_or_default())
    }
}

type Queue = VecDeque<(Message, Option<Instant>)>;

pub(crate) struct Simulation {
    peers: [Peer; 2],
    down: Queue,
    up: Queue,
    held: bool,
}

impl Simulation {
    pub fn new(text: &str) -> Self {
        Self {
            peers: [Peer::new(text), Peer::new(text)],
            down: VecDeque::new(),
            up: VecDeque::new(),
            held: false,
        }
    }

    pub fn text(&self, side: Side) -> String {
        String::from_utf8_lossy(&self.peers[side.index()].visible.items()).into_owned()
    }

    pub fn in_flight(&self, from: Side) -> usize {
        self.queue(from)
            .iter()
            .filter(|(message, _)| matches!(message, Message::Edit(..)))
            .count()
    }

    pub fn seen(&self, side: Side) -> Option<Caret> {
        self.peers[side.index()].seen
    }

    pub fn take_external(&mut self, side: Side) -> bool {
        std::mem::take(&mut self.peers[side.index()].external)
    }

    pub fn hold(&mut self, held: bool, now: Instant) {
        self.held = held;
        if held {
            return;
        }
        for (_, sent) in self.down.iter_mut().chain(self.up.iter_mut()) {
            sent.get_or_insert(now);
        }
    }

    pub fn publish(&mut self, from: Side, caret: Caret, now: Instant) {
        let sent = self.sent(now);
        let queue = self.queue_mut(from);
        if sent.is_none() {
            queue.retain(|(message, sent)| sent.is_some() || matches!(message, Message::Edit(..)));
        }
        queue.push_back((Message::Caret(caret), sent));
    }

    pub fn next_due(&self, latency: Duration) -> Option<Instant> {
        [&self.down, &self.up]
            .into_iter()
            .filter_map(|queue| queue.front().and_then(|(_, sent)| *sent))
            .map(|sent| sent + latency)
            .min()
    }

    fn sent(&self, now: Instant) -> Option<Instant> {
        (!self.held).then_some(now)
    }

    fn enqueue(&mut self, side: Side, op: SeqOp<u8>, now: Instant) -> bool {
        let sent = self.sent(now);
        let queue = match side {
            Side::Left => &mut self.down,
            Side::Right => &mut self.up,
        };
        let last = queue
            .iter()
            .rposition(|(message, sent)| sent.is_some() || matches!(message, Message::Edit(..)))
            .filter(|_| sent.is_none());
        let leftover = match last.and_then(|index| queue.get_mut(index)) {
            Some((Message::Edit(author, last), None)) if *author == side => last.absorb(op),
            _ => Some(op),
        };
        let absorbed = leftover.is_none();
        queue.extend(leftover.map(|op| (Message::Edit(side, op), sent)));
        absorbed
    }

    pub fn deliver(&mut self, from: Side, now: Instant, latency: Option<Duration>) {
        loop {
            let due = self
                .queue(from)
                .front()
                .is_some_and(|(_, sent)| match latency {
                    None => true,
                    Some(latency) => sent.is_some_and(|sent| sent + latency <= now),
                });
            if !due {
                break;
            }
            let Some((message, _)) = self.queue_mut(from).pop_front() else {
                break;
            };
            match from {
                Side::Right => self.reach_owner(message, now),
                Side::Left => self.reach_follower(message),
            }
        }
    }

    pub fn step(&mut self, from: Side, now: Instant) {
        let mut edited = false;
        while let Some((message, _)) = self.queue(from).front() {
            let edit = matches!(message, Message::Edit(..));
            if edit && edited {
                break;
            }
            edited |= edit;
            let Some((message, _)) = self.queue_mut(from).pop_front() else {
                break;
            };
            match from {
                Side::Right => self.reach_owner(message, now),
                Side::Left => self.reach_follower(message),
            }
        }
    }

    fn queue(&self, from: Side) -> &Queue {
        match from {
            Side::Left => &self.down,
            Side::Right => &self.up,
        }
    }

    fn queue_mut(&mut self, from: Side) -> &mut Queue {
        match from {
            Side::Left => &mut self.down,
            Side::Right => &mut self.up,
        }
    }

    fn reach_owner(&mut self, message: Message, now: Instant) {
        let owner = &mut self.peers[Side::Left.index()];
        match message {
            Message::Caret(caret) => owner.seen = Some(caret),
            Message::Edit(author, op) => {
                owner.confirmed.apply(&op);
                let change = owner
                    .visible
                    .apply(&op)
                    .map_or(TextChange::NONE, |splices| {
                        changed(TextChange::NONE, &splices)
                    });
                owner.external = true;
                owner.bump(Some(change));
                let sent = self.sent(now);
                self.down.push_back((Message::Edit(author, op), sent));
            }
        }
    }

    fn reach_follower(&mut self, message: Message) {
        let follower = &mut self.peers[Side::Right.index()];
        match message {
            Message::Caret(caret) => follower.seen = Some(caret),
            Message::Edit(author, op) => {
                follower.confirmed.apply(&op);
                let change = if author == Side::Right {
                    follower.pending.pop_front();
                    None
                } else if follower.pending.is_empty() {
                    Some(
                        follower
                            .visible
                            .apply(&op)
                            .map_or(TextChange::NONE, |splices| {
                                changed(TextChange::NONE, &splices)
                            }),
                    )
                } else {
                    Some(follower.rebuild())
                };
                if let Some(change) = change {
                    follower.external = true;
                    follower.bump(Some(change));
                }
            }
        }
    }

    fn submit(&mut self, side: Side, op: SeqOp<u8>, now: Instant) -> Option<TextChange> {
        let peer = &mut self.peers[side.index()];
        let splices = peer.visible.apply(&op)?;
        if side == Side::Left {
            peer.confirmed.apply(&op);
        }
        let pending = (side == Side::Right).then(|| op.clone());
        let absorbed = self.enqueue(side, op, now);
        if let Some(op) = pending {
            let pending = &mut self.peers[side.index()].pending;
            match pending.back_mut().filter(|_| absorbed) {
                Some(last) => {
                    last.absorb(op);
                }
                None => pending.push_back(op),
            }
        }
        Some(changed(TextChange::NONE, &splices))
    }

    fn replay(&mut self, side: Side, ops: &[SeqOp<u8>]) {
        let now = beui::reactive::now();
        let mut change = TextChange::NONE;
        for op in ops {
            if let Some(made) = self.submit(side, op.clone(), now) {
                change = change.then(made);
            }
        }
        self.peers[side.index()].bump(Some(change));
    }
}

fn difference(old: &[u8], new: &[u8]) -> TextChange {
    let prefix = old
        .iter()
        .zip(new)
        .take_while(|(before, after)| before == after)
        .count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(before, after)| before == after)
        .count();
    TextChange {
        start: prefix,
        old_end: old.len() - suffix,
        new_end: new.len() - suffix,
    }
}

pub(crate) struct SideDocument {
    simulation: Arc<RwLock<Simulation>>,
    side: Side,
}

impl SideDocument {
    pub fn new(simulation: &Arc<RwLock<Simulation>>, side: Side) -> Self {
        Self {
            simulation: Arc::clone(simulation),
            side,
        }
    }

    fn read_simulation(&self) -> RwLockReadGuard<'_, Simulation> {
        self.simulation
            .read()
            .expect("the collaboration simulation was poisoned")
    }

    fn write_simulation(&self) -> RwLockWriteGuard<'_, Simulation> {
        self.simulation
            .write()
            .expect("the collaboration simulation was poisoned")
    }
}

impl Document for SideDocument {
    fn read(&self) -> Option<Box<dyn DocumentRead + '_>> {
        Some(Box::new(SideRead {
            simulation: self.read_simulation(),
            side: self.side,
        }))
    }

    fn revision(&self) -> u64 {
        self.read_simulation().peers[self.side.index()].revision
    }

    fn changes_since(&self, revision: u64) -> Option<TextChange> {
        let simulation = self.read_simulation();
        let peer = &simulation.peers[self.side.index()];
        peer.changes.since(revision, peer.revision)
    }

    fn set_language(&self, language: TextLanguage) {
        let mut simulation = self.write_simulation();
        let peer = &mut simulation.peers[self.side.index()];
        peer.language = language;
        peer.bump(None);
    }

    fn set_indentation(&self, indentation: TextIndentation) {
        let mut simulation = self.write_simulation();
        let peer = &mut simulation.peers[self.side.index()];
        peer.indentation = indentation;
        peer.bump(None);
    }

    fn edit(&self, cursors: Vec<CursorPosition>, edit: &mut dyn FnMut(&mut dyn DocumentEdit)) {
        let mut simulation = self.write_simulation();
        let mut transaction = Transaction {
            simulation: &mut simulation,
            side: self.side,
            now: beui::reactive::now(),
            change: TextChange::NONE,
            group: Group {
                undo: Vec::new(),
                redo: Vec::new(),
                cursors,
            },
        };
        edit(&mut transaction);
        let (change, group) = (transaction.change, transaction.group);
        if group.redo.is_empty() {
            return;
        }
        let peer = &mut simulation.peers[self.side.index()];
        peer.bump(Some(change));
        peer.redo.clear();
        let grouping = peer.group_open;
        match peer.undo.last_mut() {
            Some(open) if grouping => {
                open.undo.extend(group.undo);
                open.redo.extend(group.redo);
            }
            _ => peer.undo.push(group),
        }
        peer.group_open = true;
    }

    fn finish_history_group(&self) {
        self.write_simulation().peers[self.side.index()].group_open = false;
    }

    fn undo(&self) -> Option<Vec<CursorPosition>> {
        let mut simulation = self.write_simulation();
        let peer = &mut simulation.peers[self.side.index()];
        peer.group_open = false;
        let group = peer.undo.pop()?;
        let undo: Vec<SeqOp<u8>> = group.undo.iter().rev().cloned().collect();
        simulation.replay(self.side, &undo);
        let cursors = group.cursors.clone();
        simulation.peers[self.side.index()].redo.push(group);
        Some(cursors)
    }

    fn redo(&self) -> Option<Vec<CursorPosition>> {
        let mut simulation = self.write_simulation();
        let peer = &mut simulation.peers[self.side.index()];
        peer.group_open = false;
        let group = peer.redo.pop()?;
        simulation.replay(self.side, &group.redo);
        let cursors = group.cursors.clone();
        simulation.peers[self.side.index()].undo.push(group);
        Some(cursors)
    }
}

struct SideRead<'a> {
    simulation: RwLockReadGuard<'a, Simulation>,
    side: Side,
}

impl SideRead<'_> {
    fn peer(&self) -> &Peer {
        &self.simulation.peers[self.side.index()]
    }
}

impl DocumentRead for SideRead<'_> {
    fn len(&self) -> usize {
        self.peer().visible.len()
    }

    fn chunk(&self, index: usize) -> &[u8] {
        self.peer().visible.chunk(index)
    }

    fn anchor(&self, index: usize) -> Option<Pos> {
        anchor_in(&self.peer().visible, index)
    }

    fn anchor_index(&self, anchor: Pos) -> Option<usize> {
        anchor_index_in(&self.peer().visible, anchor)
    }

    fn deleted_anchor_index(&self, anchor: Pos) -> Option<usize> {
        deleted_anchor_index_in(&self.peer().visible, anchor)
    }

    fn language(&self) -> TextLanguage {
        self.peer().language
    }

    fn indentation(&self) -> TextIndentation {
        self.peer().indentation
    }
}

struct Transaction<'a> {
    simulation: &'a mut Simulation,
    side: Side,
    now: Instant,
    change: TextChange,
    group: Group,
}

impl Transaction<'_> {
    fn visible(&self) -> &Sequence<u8> {
        &self.simulation.peers[self.side.index()].visible
    }

    fn run(&mut self, op: Option<SeqOp<u8>>) {
        let Some(op) = op else {
            return;
        };
        let Some((undo, redo)) = self.visible().inverse(&op) else {
            return;
        };
        let Some(made) = self.simulation.submit(self.side, op, self.now) else {
            return;
        };
        self.change = self.change.then(made);
        self.group.undo.push(undo);
        self.group.redo.push(redo);
    }
}

impl DocumentRead for Transaction<'_> {
    fn len(&self) -> usize {
        self.visible().len()
    }

    fn chunk(&self, index: usize) -> &[u8] {
        self.visible().chunk(index)
    }

    fn anchor(&self, index: usize) -> Option<Pos> {
        anchor_in(self.visible(), index)
    }

    fn anchor_index(&self, anchor: Pos) -> Option<usize> {
        anchor_index_in(self.visible(), anchor)
    }

    fn deleted_anchor_index(&self, anchor: Pos) -> Option<usize> {
        deleted_anchor_index_in(self.visible(), anchor)
    }

    fn language(&self) -> TextLanguage {
        self.simulation.peers[self.side.index()].language
    }

    fn indentation(&self) -> TextIndentation {
        self.simulation.peers[self.side.index()].indentation
    }
}

impl DocumentEdit for Transaction<'_> {
    fn document(&self) -> &dyn DocumentRead {
        self
    }

    fn replace(&mut self, index: usize, delete: usize, insert: &[u8]) {
        let len = self.visible().len();
        let index = index.min(len);
        let delete = delete.min(len - index);
        if delete > 0 {
            let op = self.visible().delete(index..index + delete);
            self.run(op);
        }
        if !insert.is_empty() {
            let op = self
                .visible()
                .insert(self.side.client(), index, insert.to_vec());
            self.run(op);
        }
    }

    fn replace_atomically(&mut self, index: usize, delete: usize, insert: &[u8]) {
        let len = self.visible().len();
        let index = index.min(len);
        let delete = delete.min(len - index);
        let op = self
            .visible()
            .replace(self.side.client(), index..index + delete, insert.to_vec());
        self.run(op);
    }
}

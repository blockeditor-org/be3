use std::collections::{BTreeMap, HashMap, VecDeque};

use super::{CHUNK, LOADED, Pos, SeqOp, Sequence, Span, Splice};

const CLIENTS: u64 = 3;
const LONGEST_INSERT: usize = 6;

pub fn sequence(data: &[u8]) {
    let mut input = Input(data);
    let start = input.items(48);
    let mut authority = Sequence::from_items(start.clone());
    let mut reference = Reference::new(&start);
    let mut log: Vec<(u64, SeqOp<u8>)> = Vec::new();
    let mut clients: Vec<Client> = (1..=CLIENTS)
        .map(|id| Client::new(id, &authority))
        .collect();
    while !input.is_empty() {
        let client = input.below(clients.len());
        match input.byte() % 10 {
            0..=4 => clients[client].edit(&mut input),
            5 | 6 => {
                if let Some((op, parts)) = clients[client].send() {
                    merged(
                        &mut authority,
                        &mut reference,
                        &mut log,
                        clients[client].id,
                        op,
                        &parts,
                    );
                }
            }
            7 => clients[client].catch_up(&log),
            8 => {
                let op = garbage(&mut input, &authority);
                sequenced(&mut authority, &mut reference, &mut log, 0, op);
            }
            _ => {
                let adopted = Sequence::from_state(authority.state(), &authority.items())
                    .expect("a sequence's own state is well formed");
                assert_eq!(order(&adopted), order(&authority));
                check(&adopted);
                authority = adopted;
            }
        }
    }
    for client in &mut clients {
        while let Some((op, parts)) = client.send() {
            merged(
                &mut authority,
                &mut reference,
                &mut log,
                client.id,
                op,
                &parts,
            );
        }
    }
    for client in &mut clients {
        client.catch_up(&log);
        assert!(client.pending.is_empty());
        assert_eq!(order(&client.confirmed), order(&authority));
        assert_eq!(order(&client.visible), order(&authority));
    }
}

fn merged(
    authority: &mut Sequence<u8>,
    reference: &mut Reference,
    log: &mut Vec<(u64, SeqOp<u8>)>,
    author: u64,
    op: SeqOp<u8>,
    parts: &[SeqOp<u8>],
) {
    let mut separate = authority.clone();
    for part in parts {
        separate.apply(part);
    }
    sequenced(authority, reference, log, author, op);
    assert_eq!(order(&separate), order(authority), "{parts:?}");
    for client in 0..=CLIENTS {
        assert_eq!(
            separate.next_offset(client),
            authority.next_offset(client),
            "{parts:?}"
        );
    }
}

fn sequenced(
    authority: &mut Sequence<u8>,
    reference: &mut Reference,
    log: &mut Vec<(u64, SeqOp<u8>)>,
    author: u64,
    op: SeqOp<u8>,
) {
    if let SeqOp::Insert { client, start, .. } | SeqOp::Replace { client, start, .. } = &op
        && author != 0
    {
        assert_eq!(*start, authority.next_offset(*client), "{op:?}");
    }
    let before = authority.items();
    let splices = authority.apply(&op);
    assert_eq!(splices.is_some(), reference.apply(&op), "{op:?}");
    assert_eq!(order(authority), reference.view(), "{op:?}");
    for (client, next) in &reference.next {
        assert_eq!(authority.next_offset(*client), *next, "{op:?}");
    }
    check(authority);
    if let Some(splices) = splices {
        replayed(&before, &splices, &authority.items());
    }
    log.push((author, op));
}

fn replayed(before: &[u8], splices: &[Splice], after: &[u8]) {
    let removed: usize = splices.iter().map(|splice| splice.removed).sum();
    let inserted: usize = splices.iter().map(|splice| splice.inserted).sum();
    assert_eq!(before.len() - removed + inserted, after.len());
    if let [splice] = splices {
        let mut replayed = before.to_vec();
        replayed.splice(
            splice.at..splice.at + splice.removed,
            after[splice.at..splice.at + splice.inserted]
                .iter()
                .copied(),
        );
        assert_eq!(replayed, after);
    }
}

struct Client {
    id: u64,
    confirmed: Sequence<u8>,
    visible: Sequence<u8>,
    pending: VecDeque<SeqOp<u8>>,
    parts: VecDeque<Vec<SeqOp<u8>>>,
    sent: usize,
    seen: usize,
    undo: Vec<SeqOp<u8>>,
    caret: usize,
}

impl Client {
    fn new(id: u64, authority: &Sequence<u8>) -> Self {
        let mut visible = authority.clone();
        visible.mirror();
        Self {
            id,
            confirmed: authority.clone(),
            visible,
            pending: VecDeque::new(),
            parts: VecDeque::new(),
            sent: 0,
            seen: 0,
            undo: Vec::new(),
            caret: 0,
        }
    }

    fn edit(&mut self, input: &mut Input) {
        let len = self.visible.len();
        let from = input.below(len + 1);
        let to = (from + input.below(8)).min(len);
        let caret = self.caret.min(len);
        let kind = input.byte() % 9;
        let (op, caret) = match kind {
            0 | 1 => {
                let items = input.items(LONGEST_INSERT);
                let after = from + items.len();
                (self.visible.insert(self.id, from, items), after)
            }
            2 => (self.visible.delete(from..to), from),
            7 => {
                let items = input.items(LONGEST_INSERT);
                let after = caret + items.len();
                (self.visible.insert(self.id, caret, items), after)
            }
            8 => {
                let back = caret.saturating_sub(1);
                (self.visible.delete(back..caret), back)
            }
            _ => (self.unpositioned(input, kind, from..to, len), caret),
        };
        self.caret = caret;
        let Some(op) = op else {
            return;
        };
        if let Some((back, _)) = self.visible.inverse(&op) {
            self.undo.push(back);
        }
        let before = self.visible.items();
        if let Some(splices) = self.visible.apply(&op) {
            replayed(&before, &splices, &self.visible.items());
        }
        check(&self.visible);
        let part = op.clone();
        let unsent = self.pending.len() > self.sent && input.byte().is_multiple_of(2);
        let leftover = match self.pending.back_mut().filter(|_| unsent) {
            Some(last) => last.absorb(op.clone()),
            None => Some(op),
        };
        match leftover {
            Some(op) => {
                self.pending.push_back(op);
                self.parts.push_back(vec![part]);
            }
            None => {
                if let Some(parts) = self.parts.back_mut() {
                    parts.push(part);
                }
            }
        }
    }

    fn unpositioned(
        &mut self,
        input: &mut Input,
        kind: u8,
        range: std::ops::Range<usize>,
        len: usize,
    ) -> Option<SeqOp<u8>> {
        match kind {
            3 => self
                .visible
                .replace(self.id, range, input.items(LONGEST_INSERT)),
            4 => self.visible.move_range(range, input.below(len + 1)),
            5 => self.undo.pop(),
            _ => {
                let everything = order(&self.visible);
                let after =
                    (!everything.is_empty()).then(|| everything[input.below(everything.len())].0);
                let items = input.items(LONGEST_INSERT);
                (!items.is_empty()).then(|| SeqOp::Insert {
                    after,
                    client: self.id,
                    start: self.visible.next_offset(self.id),
                    items,
                })
            }
        }
    }

    fn send(&mut self) -> Option<(SeqOp<u8>, Vec<SeqOp<u8>>)> {
        let op = self.pending.get(self.sent)?.clone();
        let parts = self.parts.get(self.sent)?.clone();
        self.sent += 1;
        Some((op, parts))
    }

    fn catch_up(&mut self, log: &[(u64, SeqOp<u8>)]) {
        let mut rebuild = false;
        for (author, op) in &log[self.seen..] {
            self.confirmed.apply(op);
            if *author == self.id {
                let mine = self.pending.pop_front();
                self.parts.pop_front();
                assert_eq!(mine.as_ref(), Some(op));
                self.sent -= 1;
            } else if self.pending.is_empty() && !rebuild {
                self.visible.apply(op);
            } else {
                rebuild = true;
            }
        }
        self.seen = log.len();
        if rebuild {
            self.visible = self.confirmed.clone();
            self.visible.mirror();
            for op in &self.pending {
                self.visible.apply(op);
            }
        }
        check(&self.confirmed);
        check(&self.visible);
        if self.pending.is_empty() {
            assert_eq!(order(&self.visible), order(&self.confirmed));
        }
    }
}

fn garbage(input: &mut Input, authority: &Sequence<u8>) -> SeqOp<u8> {
    let everything = order(authority);
    let pos = |input: &mut Input| match input.byte() % 4 {
        0 => Pos {
            client: input.below(CLIENTS as usize + 1) as u64,
            offset: input.number(),
        },
        _ if everything.is_empty() => Pos {
            client: LOADED,
            offset: 0,
        },
        _ => everything[input.below(everything.len())].0,
    };
    let spans = |input: &mut Input| -> Vec<Span> {
        (0..input.below(3))
            .map(|_| {
                let first = pos(input);
                Span {
                    client: first.client,
                    start: first.offset,
                    len: match input.byte() % 4 {
                        0 => input.number(),
                        _ => input.below(6) as u64,
                    },
                }
            })
            .collect()
    };
    let client = match input.byte() % 8 {
        0 => input.below(CLIENTS as usize + 1) as u64,
        _ => CLIENTS + 1,
    };
    let start = match input.byte() % 4 {
        _ if (1..=CLIENTS).contains(&client) => input.number() | 1 << 63,
        0 => input.number(),
        _ => authority.next_offset(client),
    };
    match input.byte() % 6 {
        0 => SeqOp::Insert {
            after: (!input.byte().is_multiple_of(4)).then(|| pos(input)),
            client,
            start,
            items: input.items(LONGEST_INSERT),
        },
        1 => SeqOp::Delete {
            spans: spans(input),
        },
        2 => {
            let spans = spans(input);
            SeqOp::Undelete {
                items: carried(input, &spans),
                spans,
            }
        }
        3 => SeqOp::Replace {
            spans: spans(input),
            client,
            start,
            items: input.items(LONGEST_INSERT),
        },
        4 => {
            let hide = spans(input);
            let show = spans(input);
            SeqOp::Swap {
                items: carried(input, &show),
                hide,
                show,
            }
        }
        _ => SeqOp::Move {
            first: pos(input),
            last: pos(input),
            after: (!input.byte().is_multiple_of(4)).then(|| pos(input)),
        },
    }
}

fn carried(input: &mut Input, spans: &[Span]) -> Vec<u8> {
    let total = spans
        .iter()
        .try_fold(0u64, |total, span| total.checked_add(span.len))
        .filter(|total| *total <= 64);
    let len = match (total, input.byte() % 8) {
        (Some(total), 0..=6) => total as usize,
        _ => input.below(8),
    };
    (0..len).map(|_| input.byte()).collect()
}

struct Input<'a>(&'a [u8]);

impl Input<'_> {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn byte(&mut self) -> u8 {
        let Some((first, rest)) = self.0.split_first() else {
            return 0;
        };
        self.0 = rest;
        *first
    }

    fn below(&mut self, bound: usize) -> usize {
        match bound {
            0 => 0,
            _ => usize::from(u16::from_le_bytes([self.byte(), self.byte()])) % bound,
        }
    }

    fn number(&mut self) -> u64 {
        (0..8).fold(0, |number, _| number << 8 | u64::from(self.byte()))
    }

    fn items(&mut self, longest: usize) -> Vec<u8> {
        let len = self.below(longest + 1);
        (0..len).map(|_| self.byte()).collect()
    }
}

pub(crate) fn check(sequence: &Sequence<u8>) {
    let mut seen = BTreeMap::new();
    for (at, chunk) in sequence.chunks.iter().enumerate() {
        assert_eq!(sequence.order.get(&chunk.key), Some(&at));
        assert_eq!(
            chunk.visible,
            chunk
                .fragments
                .iter()
                .map(|fragment| fragment.visible_len())
                .sum::<usize>()
        );
        assert!(chunk.fragments.len() <= 2 * CHUNK);
        for fragment in &chunk.fragments {
            assert!(fragment.len > 0);
            if fragment.visible {
                assert_eq!(sequence.slice(*fragment).len() as u64, fragment.len);
            }
            assert_eq!(sequence.index.get(&fragment.first()), Some(&chunk.key));
            assert!(seen.insert(fragment.first(), fragment.len).is_none());
        }
    }
    assert_eq!(sequence.index.len(), seen.len());
    for ((first, run), (next, _)) in sequence.runs.iter().zip(sequence.runs.iter().skip(1)) {
        assert!(
            first.client != next.client || first.offset + run.len() as u64 != next.offset,
            "contiguous runs were not joined"
        );
    }
    let items = sequence.items();
    if let Some(flat) = sequence.mirrored() {
        assert_eq!(flat, items);
    }
    let mut visible = 0;
    for (at, (pos, item)) in order(sequence).into_iter().enumerate() {
        let shown = item.is_some();
        if at % 5 == 0 {
            assert_eq!(sequence.place_of(pos), Some((visible, shown)));
        }
        if shown && at % 5 == 0 {
            let chunk = sequence.chunk(visible);
            assert!(!chunk.is_empty());
            assert_eq!(chunk, &items[visible..visible + chunk.len()]);
        }
        visible += usize::from(shown);
    }
    assert!(sequence.chunk(items.len()).is_empty());
    assert_eq!(
        sequence.len(),
        sequence
            .chunks
            .iter()
            .map(|chunk| chunk.visible)
            .sum::<usize>()
    );
}

pub(crate) fn order(sequence: &Sequence<u8>) -> Vec<(Pos, Option<u8>)> {
    sequence
        .fragments()
        .flat_map(|fragment| {
            sequence
                .slice(*fragment)
                .iter()
                .copied()
                .map(Some)
                .chain(std::iter::repeat(None))
                .take(fragment.len as usize)
                .enumerate()
                .map(|(at, item)| {
                    (
                        Pos {
                            client: fragment.client,
                            offset: fragment.start + at as u64,
                        },
                        item.filter(|_| fragment.visible),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

struct Reference {
    order: Vec<(Pos, u8, bool)>,
    next: BTreeMap<u64, u64>,
    indices: HashMap<Pos, usize>,
}

impl Reference {
    fn new(items: &[u8]) -> Self {
        let mut reference = Self {
            order: items
                .iter()
                .enumerate()
                .map(|(at, item)| {
                    (
                        Pos {
                            client: LOADED,
                            offset: at as u64,
                        },
                        *item,
                        true,
                    )
                })
                .collect(),
            next: BTreeMap::from([(LOADED, items.len() as u64)]),
            indices: HashMap::new(),
        };
        reference.reindex();
        reference
    }

    fn at(&self, pos: Pos) -> Option<usize> {
        self.indices.get(&pos).copied()
    }

    fn view(&self) -> Vec<(Pos, Option<u8>)> {
        self.order
            .iter()
            .map(|(pos, item, visible)| (*pos, visible.then_some(*item)))
            .collect()
    }

    fn carries(spans: &[Span], items: &[u8]) -> bool {
        spans
            .iter()
            .try_fold(0u64, |total, span| total.checked_add(span.len))
            .is_some_and(|total| total == items.len() as u64)
    }

    fn show(&mut self, spans: &[Span], items: &[u8]) -> bool {
        let mut changed = false;
        let mut rest = items;
        for span in spans {
            let (carried, after) = rest.split_at((span.len as usize).min(rest.len()));
            rest = after;
            for (index, item) in self
                .positions(*span)
                .unwrap_or_default()
                .into_iter()
                .zip(carried)
            {
                if !self.order[index].2 {
                    self.order[index] = (self.order[index].0, *item, true);
                    changed = true;
                }
            }
        }
        changed
    }

    fn reindex(&mut self) {
        self.indices = self
            .order
            .iter()
            .enumerate()
            .map(|(at, (pos, _, _))| (*pos, at))
            .collect();
    }

    fn positions(&self, span: Span) -> Option<Vec<usize>> {
        let end = span.start.checked_add(span.len)?;
        if span.len == 0 || end > self.next.get(&span.client).copied().unwrap_or(0) {
            return None;
        }
        (span.start..end)
            .map(|offset| {
                self.at(Pos {
                    client: span.client,
                    offset,
                })
            })
            .collect()
    }

    fn all(&self, spans: &[Span], visible: bool) -> bool {
        spans.iter().all(|span| {
            self.positions(*span)
                .is_some_and(|at| at.iter().all(|index| self.order[*index].2 == visible))
        })
    }

    fn set(&mut self, spans: &[Span], visible: bool) -> bool {
        let mut changed = false;
        for span in spans {
            for index in self.positions(*span).unwrap_or_default() {
                changed |= self.order[index].2 != visible;
                self.order[index].2 = visible;
            }
        }
        changed
    }

    fn reserve(&mut self, client: u64, start: u64, items: &[u8]) {
        let next = self.next.get(&client).copied().unwrap_or(0);
        if start == next {
            self.next.insert(client, next + items.len() as u64);
        }
    }

    fn add(&mut self, after: Option<Pos>, client: u64, start: u64, items: &[u8]) -> bool {
        let next = self.next.get(&client).copied().unwrap_or(0);
        if items.is_empty() || start != next {
            return false;
        }
        let index = match after {
            None => 0,
            Some(anchor) => match self.at(anchor) {
                Some(index) => index + 1,
                None => {
                    self.reserve(client, start, items);
                    return false;
                }
            },
        };
        let added = items.iter().enumerate().map(|(at, item)| {
            (
                Pos {
                    client,
                    offset: start + at as u64,
                },
                *item,
                true,
            )
        });
        self.order.splice(index..index, added);
        self.next.insert(client, next + items.len() as u64);
        self.reindex();
        true
    }

    fn apply(&mut self, op: &SeqOp<u8>) -> bool {
        match op {
            SeqOp::Insert {
                after,
                client,
                start,
                items,
            } => self.add(*after, *client, *start, items),
            SeqOp::Delete { spans } => self.set(spans, false),
            SeqOp::Undelete { spans, items } => {
                Self::carries(spans, items) && self.show(spans, items)
            }
            SeqOp::Replace {
                spans,
                client,
                start,
                items,
            } => {
                let next = self.next.get(client).copied().unwrap_or(0);
                if *start != next {
                    return false;
                }
                let Some(last) = spans.last().filter(|_| self.all(spans, true)) else {
                    self.reserve(*client, *start, items);
                    return false;
                };
                self.set(spans, false);
                if !items.is_empty() {
                    let after = Pos {
                        client: last.client,
                        offset: last.start + last.len - 1,
                    };
                    self.add(Some(after), *client, *start, items);
                }
                true
            }
            SeqOp::Swap { hide, show, items } => {
                if (hide.is_empty() && show.is_empty())
                    || !Self::carries(show, items)
                    || !self.all(hide, true)
                    || !self.all(show, false)
                {
                    return false;
                }
                self.set(hide, false);
                self.show(show, items);
                true
            }
            SeqOp::Move { first, last, after } => {
                let (Some(start), Some(end)) = (self.at(*first), self.at(*last)) else {
                    return false;
                };
                if start > end {
                    return false;
                }
                let target = match after {
                    None => None,
                    Some(anchor) => match self.at(*anchor) {
                        Some(index) => Some(index),
                        None => return false,
                    },
                };
                if target.is_some_and(|index| start <= index && index <= end) {
                    return false;
                }
                let previous = start.checked_sub(1).map(|index| self.order[index].0);
                if previous == *after {
                    return false;
                }
                let moved: Vec<_> = self.order.drain(start..=end).collect();
                self.reindex();
                let index = match after {
                    None => 0,
                    Some(anchor) => self.at(*anchor).map_or(0, |index| index + 1),
                };
                self.order.splice(index..index, moved);
                self.reindex();
                true
            }
        }
    }
}

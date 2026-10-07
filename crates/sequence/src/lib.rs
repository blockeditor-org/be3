use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    ops::Range,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeSeq};

pub const LOADED: u64 = 0;

#[cfg(not(any(test, feature = "fuzzing")))]
const CHUNK: usize = 64;

#[cfg(any(test, feature = "fuzzing"))]
const CHUNK: usize = 2;

#[derive(Debug)]
pub struct Malformed;

impl fmt::Display for Malformed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a sequence's session state is malformed")
    }
}

impl std::error::Error for Malformed {}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Pos {
    pub client: u64,
    pub offset: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Span {
    pub client: u64,
    pub start: u64,
    pub len: u64,
}

impl Span {
    fn end(self) -> u64 {
        self.start + self.len
    }

    fn last(self) -> Pos {
        Pos {
            client: self.client,
            offset: self.end() - 1,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SeqOp<T> {
    Insert {
        after: Option<Pos>,
        client: u64,
        start: u64,
        items: Vec<T>,
    },
    Delete {
        spans: Vec<Span>,
    },
    Undelete {
        spans: Vec<Span>,
    },
    Replace {
        spans: Vec<Span>,
        client: u64,
        start: u64,
        items: Vec<T>,
    },
    Swap {
        hide: Vec<Span>,
        show: Vec<Span>,
    },
    Move {
        first: Pos,
        last: Pos,
        after: Option<Pos>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Splice {
    pub at: usize,
    pub removed: usize,
    pub inserted: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Fragment {
    client: u64,
    start: u64,
    len: u64,
    visible: bool,
}

impl Fragment {
    fn first(self) -> Pos {
        Pos {
            client: self.client,
            offset: self.start,
        }
    }

    fn end(self) -> u64 {
        self.start + self.len
    }

    fn visible_len(self) -> usize {
        if self.visible { self.len as usize } else { 0 }
    }
}

#[derive(Clone, Debug)]
struct Chunk {
    key: u32,
    fragments: Vec<Fragment>,
    visible: usize,
}

#[derive(Deserialize, Serialize)]
pub struct State<T> {
    buffers: BTreeMap<u64, Vec<T>>,
    fragments: Vec<Fragment>,
}

#[derive(Clone)]
pub struct Sequence<T> {
    buffers: BTreeMap<u64, Vec<T>>,
    chunks: Vec<Chunk>,
    index: BTreeMap<Pos, u32>,
    order: HashMap<u32, usize>,
    next_key: u32,
    visible: usize,
}

type Location = (usize, usize);

impl<T> Default for Sequence<T> {
    fn default() -> Self {
        Self::build(BTreeMap::new(), Vec::new())
    }
}

impl<T: Clone> Sequence<T> {
    pub fn from_items(items: Vec<T>) -> Self {
        let fragments = match items.len() {
            0 => Vec::new(),
            len => vec![Fragment {
                client: LOADED,
                start: 0,
                len: len as u64,
                visible: true,
            }],
        };
        Self::build(BTreeMap::from([(LOADED, items)]), fragments)
    }

    pub fn items(&self) -> Vec<T> {
        self.iter().cloned().collect()
    }

    pub fn refreshed(&self) -> Self {
        Self::from_items(self.items())
    }

    pub fn state(&self) -> State<T> {
        State {
            buffers: self.buffers.clone(),
            fragments: self.fragments().copied().collect(),
        }
    }

    pub fn insert(&self, client: u64, index: usize, items: Vec<T>) -> Option<SeqOp<T>> {
        if items.is_empty() || index > self.len() {
            return None;
        }
        Some(SeqOp::Insert {
            after: index.checked_sub(1).and_then(|previous| self.pos(previous)),
            client,
            start: self.next_offset(client),
            items,
        })
    }

    pub fn replace(&self, client: u64, range: Range<usize>, items: Vec<T>) -> Option<SeqOp<T>> {
        if range.is_empty() {
            return self.insert(client, range.start, items);
        }
        let spans = self.spans(range);
        (!spans.is_empty()).then(|| SeqOp::Replace {
            spans,
            client,
            start: self.next_offset(client),
            items,
        })
    }
}

impl<T> Sequence<T> {
    fn build(buffers: BTreeMap<u64, Vec<T>>, fragments: Vec<Fragment>) -> Self {
        let mut sequence = Self {
            buffers,
            chunks: Vec::new(),
            index: BTreeMap::new(),
            order: HashMap::new(),
            next_key: 0,
            visible: 0,
        };
        let pieces: Vec<Vec<Fragment>> = match fragments.is_empty() {
            true => vec![Vec::new()],
            false => fragments.chunks(CHUNK).map(<[_]>::to_vec).collect(),
        };
        for piece in pieces {
            let key = sequence.new_key();
            for fragment in &piece {
                sequence.index.insert(fragment.first(), key);
            }
            sequence.chunks.push(Chunk {
                key,
                visible: piece.iter().map(|fragment| fragment.visible_len()).sum(),
                fragments: piece,
            });
        }
        sequence.visible = sequence.chunks.iter().map(|chunk| chunk.visible).sum();
        sequence.reorder();
        sequence
    }

    pub fn from_state(state: State<T>) -> Result<Self, Malformed> {
        let mut seen: BTreeMap<u64, Vec<(u64, u64)>> = BTreeMap::new();
        for fragment in &state.fragments {
            let held = state.buffers.get(&fragment.client).map_or(0, Vec::len) as u64;
            let end = fragment.start.checked_add(fragment.len).ok_or(Malformed)?;
            if fragment.len == 0 || end > held {
                return Err(Malformed);
            }
            seen.entry(fragment.client)
                .or_default()
                .push((fragment.start, fragment.end()));
        }
        for ranges in seen.values_mut() {
            ranges.sort_unstable();
            if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
                return Err(Malformed);
            }
        }
        Ok(Self::build(state.buffers, state.fragments))
    }

    pub fn len(&self) -> usize {
        self.visible
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn fragment_count(&self) -> usize {
        self.chunks.iter().map(|chunk| chunk.fragments.len()).sum()
    }

    pub fn is_fresh(&self) -> bool {
        let mut fragments = self.fragments();
        let loaded = self.buffers.get(&LOADED).map_or(0, Vec::len);
        let whole = match fragments.next() {
            None => loaded == 0,
            Some(fragment) => {
                fragment.client == LOADED
                    && fragment.start == 0
                    && fragment.len as usize == loaded
                    && fragment.visible
            }
        };
        whole && fragments.next().is_none() && self.buffers.keys().all(|client| *client == LOADED)
    }

    pub fn next_offset(&self, client: u64) -> u64 {
        self.buffers.get(&client).map_or(0, Vec::len) as u64
    }

    pub fn slices(&self) -> impl Iterator<Item = &[T]> {
        self.fragments()
            .filter(|fragment| fragment.visible)
            .map(|fragment| self.slice(*fragment))
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.slices().flatten()
    }

    pub fn pos(&self, index: usize) -> Option<Pos> {
        let (ci, fi, inner) = self.find_visible(index)?;
        let fragment = self.chunks[ci].fragments[fi];
        Some(Pos {
            client: fragment.client,
            offset: fragment.start + inner as u64,
        })
    }

    pub fn index(&self, pos: Pos) -> Option<usize> {
        let (ci, fi) = self.locate(pos)?;
        let fragment = self.chunks[ci].fragments[fi];
        fragment
            .visible
            .then(|| self.visible_before(ci, fi) + (pos.offset - fragment.start) as usize)
    }

    pub fn spans(&self, range: Range<usize>) -> Vec<Span> {
        let mut spans: Vec<Span> = Vec::new();
        let Some((ci, fi, inner)) = self.find_visible(range.start) else {
            return spans;
        };
        let mut remaining = range.end.saturating_sub(range.start);
        let mut skip = inner as u64;
        for fragment in self.fragments_from(ci, fi) {
            if remaining == 0 {
                break;
            }
            if !fragment.visible {
                continue;
            }
            let start = fragment.start + skip;
            let len = (fragment.len - skip).min(remaining as u64);
            skip = 0;
            remaining -= len as usize;
            push_span(
                &mut spans,
                Span {
                    client: fragment.client,
                    start,
                    len,
                },
            );
        }
        spans
    }

    pub fn delete(&self, range: Range<usize>) -> Option<SeqOp<T>> {
        let spans = self.spans(range);
        (!spans.is_empty()).then_some(SeqOp::Delete { spans })
    }

    pub fn move_range(&self, range: Range<usize>, to: usize) -> Option<SeqOp<T>> {
        if range.is_empty() || range.end > self.len() || to > self.len() {
            return None;
        }
        if (range.start..=range.end).contains(&to) {
            return None;
        }
        Some(SeqOp::Move {
            first: self.pos(range.start)?,
            last: self.pos(range.end - 1)?,
            after: to.checked_sub(1).and_then(|previous| self.pos(previous)),
        })
    }

    pub fn applies(&self, operation: &SeqOp<T>) -> bool {
        match operation {
            SeqOp::Insert {
                after,
                client,
                start,
                items,
            } => !items.is_empty() && self.can_insert(*after, *client, *start),
            SeqOp::Delete { spans } => !self.parts(spans, true).is_empty(),
            SeqOp::Undelete { spans } => !self.parts(spans, false).is_empty(),
            SeqOp::Replace {
                spans,
                client,
                start,
                ..
            } => {
                spans.last().is_some()
                    && self.all(spans, true)
                    && *start == self.next_offset(*client)
            }
            SeqOp::Swap { hide, show } => {
                !(hide.is_empty() && show.is_empty())
                    && self.all(hide, true)
                    && self.all(show, false)
            }
            SeqOp::Move { first, last, after } => self.moves(*first, *last, *after),
        }
    }

    pub fn inverse(&self, operation: &SeqOp<T>) -> Option<(SeqOp<T>, SeqOp<T>)>
    where
        T: Clone,
    {
        if !self.applies(operation) {
            return None;
        }
        Some(match operation {
            SeqOp::Insert {
                client,
                start,
                items,
                ..
            } => {
                let spans = vec![Span {
                    client: *client,
                    start: *start,
                    len: items.len() as u64,
                }];
                (
                    SeqOp::Delete {
                        spans: spans.clone(),
                    },
                    SeqOp::Undelete { spans },
                )
            }
            SeqOp::Delete { spans } => {
                let spans = self.parts(spans, true);
                (
                    SeqOp::Undelete {
                        spans: spans.clone(),
                    },
                    SeqOp::Delete { spans },
                )
            }
            SeqOp::Undelete { spans } => {
                let spans = self.parts(spans, false);
                (
                    SeqOp::Delete {
                        spans: spans.clone(),
                    },
                    SeqOp::Undelete { spans },
                )
            }
            SeqOp::Replace {
                spans,
                client,
                start,
                items,
            } => {
                let inserted: Vec<Span> = match items.len() {
                    0 => Vec::new(),
                    len => vec![Span {
                        client: *client,
                        start: *start,
                        len: len as u64,
                    }],
                };
                (
                    SeqOp::Swap {
                        hide: inserted.clone(),
                        show: spans.clone(),
                    },
                    SeqOp::Swap {
                        hide: spans.clone(),
                        show: inserted,
                    },
                )
            }
            SeqOp::Swap { hide, show } => (
                SeqOp::Swap {
                    hide: show.clone(),
                    show: hide.clone(),
                },
                operation.clone(),
            ),
            SeqOp::Move { first, last, .. } => (
                SeqOp::Move {
                    first: *first,
                    last: *last,
                    after: self.predecessor(*first),
                },
                operation.clone(),
            ),
        })
    }

    pub fn apply(&mut self, operation: &SeqOp<T>) -> Option<Vec<Splice>>
    where
        T: Clone,
    {
        if !self.applies(operation) {
            self.reserve(operation);
            return None;
        }
        let mut splices = Vec::new();
        match operation {
            SeqOp::Insert {
                after,
                client,
                start,
                items,
            } => self.add(*after, *client, *start, items, &mut splices)?,
            SeqOp::Delete { spans } => self.set_visible(spans, false, &mut splices),
            SeqOp::Undelete { spans } => self.set_visible(spans, true, &mut splices),
            SeqOp::Replace {
                spans,
                client,
                start,
                items,
            } => {
                self.set_visible(spans, false, &mut splices);
                if !items.is_empty() {
                    let after = spans.last().map(|span| span.last());
                    self.add(after, *client, *start, items, &mut splices)?;
                }
            }
            SeqOp::Swap { hide, show } => {
                self.set_visible(hide, false, &mut splices);
                self.set_visible(show, true, &mut splices);
            }
            SeqOp::Move { first, last, after } => {
                self.relocate(*first, *last, *after, &mut splices)?;
            }
        }
        Some(splices)
    }

    fn reserve(&mut self, operation: &SeqOp<T>)
    where
        T: Clone,
    {
        let (SeqOp::Insert {
            client,
            start,
            items,
            ..
        }
        | SeqOp::Replace {
            client,
            start,
            items,
            ..
        }) = operation
        else {
            return;
        };
        if *start == self.next_offset(*client) {
            self.buffers
                .entry(*client)
                .or_default()
                .extend_from_slice(items);
        }
    }

    fn can_insert(&self, after: Option<Pos>, client: u64, start: u64) -> bool {
        start == self.next_offset(client)
            && after.is_none_or(|anchor| self.locate(anchor).is_some())
    }

    fn add(
        &mut self,
        after: Option<Pos>,
        client: u64,
        start: u64,
        items: &[T],
        splices: &mut Vec<Splice>,
    ) -> Option<()>
    where
        T: Clone,
    {
        if !self.can_insert(after, client, start) {
            return None;
        }
        self.buffers
            .entry(client)
            .or_default()
            .extend_from_slice(items);
        let fragment = Fragment {
            client,
            start,
            len: items.len() as u64,
            visible: true,
        };
        let at = self.place(after, vec![fragment])?;
        splices.push(Splice {
            at,
            removed: 0,
            inserted: items.len(),
        });
        Some(())
    }

    fn set_visible(&mut self, spans: &[Span], visible: bool, splices: &mut Vec<Splice>) {
        for span in spans {
            let Some(starts) = self.isolate(*span) else {
                continue;
            };
            for start in starts {
                let Some((ci, fi)) = self.locate(start) else {
                    continue;
                };
                let fragment = self.chunks[ci].fragments[fi];
                if fragment.visible == visible {
                    continue;
                }
                let at = self.visible_before(ci, fi);
                let len = fragment.len as usize;
                splices.push(match visible {
                    true => Splice {
                        at,
                        removed: 0,
                        inserted: len,
                    },
                    false => Splice {
                        at,
                        removed: len,
                        inserted: 0,
                    },
                });
                self.chunks[ci].fragments[fi].visible = visible;
                self.recount(ci);
            }
        }
    }

    fn relocate(
        &mut self,
        first: Pos,
        last: Pos,
        after: Option<Pos>,
        splices: &mut Vec<Splice>,
    ) -> Option<()> {
        self.cut(first)?;
        self.cut_after(last)?;
        let (ci, fi) = self.locate(first)?;
        let at = self.visible_before(ci, fi);
        let mut moved = Vec::new();
        let (mut chunk, mut at_fragment) = (ci, fi);
        while chunk < self.chunks.len() {
            if at_fragment >= self.chunks[chunk].fragments.len() {
                chunk += 1;
                at_fragment = 0;
                continue;
            }
            let fragment = self.chunks[chunk].fragments.remove(at_fragment);
            self.index.remove(&fragment.first());
            moved.push(fragment);
            if fragment.client == last.client
                && (fragment.start..fragment.end()).contains(&last.offset)
            {
                break;
            }
        }
        for touched in ci..=chunk.min(self.chunks.len() - 1) {
            self.recount(touched);
        }
        self.prune();
        let count: usize = moved.iter().map(|fragment| fragment.visible_len()).sum();
        let to = self.place(after, moved)?;
        if count > 0 {
            splices.push(Splice {
                at,
                removed: count,
                inserted: 0,
            });
            splices.push(Splice {
                at: to,
                removed: 0,
                inserted: count,
            });
        }
        Some(())
    }

    fn moves(&self, first: Pos, last: Pos, after: Option<Pos>) -> bool {
        let (Some(start), Some(end)) = (self.global(first), self.global(last)) else {
            return false;
        };
        if start > end {
            return false;
        }
        if let Some(anchor) = after {
            let Some(target) = self.global(anchor) else {
                return false;
            };
            if start <= target && target <= end {
                return false;
            }
        }
        self.predecessor(first) != after
    }

    fn global(&self, pos: Pos) -> Option<(usize, usize, u64)> {
        let (ci, fi) = self.locate(pos)?;
        Some((ci, fi, pos.offset - self.chunks[ci].fragments[fi].start))
    }

    fn predecessor(&self, pos: Pos) -> Option<Pos> {
        let (ci, fi) = self.locate(pos)?;
        let fragment = self.chunks[ci].fragments[fi];
        if pos.offset > fragment.start {
            return Some(Pos {
                client: pos.client,
                offset: pos.offset - 1,
            });
        }
        let previous = match fi {
            0 => self.chunks[..ci]
                .iter()
                .rev()
                .find_map(|chunk| chunk.fragments.last())?,
            _ => &self.chunks[ci].fragments[fi - 1],
        };
        Some(Pos {
            client: previous.client,
            offset: previous.end() - 1,
        })
    }

    fn all(&self, spans: &[Span], visible: bool) -> bool {
        spans.iter().all(|span| {
            self.walk(*span).is_some_and(|fragments| {
                fragments
                    .iter()
                    .all(|(fragment, _)| fragment.visible == visible)
            })
        })
    }

    fn parts(&self, spans: &[Span], visible: bool) -> Vec<Span> {
        let mut parts = Vec::new();
        for span in spans {
            let Some(fragments) = self.walk(*span) else {
                continue;
            };
            for (fragment, part) in fragments {
                if fragment.visible == visible {
                    push_span(&mut parts, part);
                }
            }
        }
        parts
    }

    fn walk(&self, span: Span) -> Option<Vec<(Fragment, Span)>> {
        let end = span.start.checked_add(span.len)?;
        if span.len == 0 || end > self.next_offset(span.client) {
            return None;
        }
        let mut pieces = Vec::new();
        let mut offset = span.start;
        while offset < span.end() {
            let (ci, fi) = self.locate(Pos {
                client: span.client,
                offset,
            })?;
            let fragment = self.chunks[ci].fragments[fi];
            let end = fragment.end().min(span.end());
            pieces.push((
                fragment,
                Span {
                    client: span.client,
                    start: offset,
                    len: end - offset,
                },
            ));
            offset = end;
        }
        Some(pieces)
    }

    fn isolate(&mut self, span: Span) -> Option<Vec<Pos>> {
        let pieces = self.walk(span)?;
        self.cut(Pos {
            client: span.client,
            offset: span.start,
        })?;
        self.cut_after(span.last())?;
        Some(
            pieces
                .into_iter()
                .map(|(_, part)| Pos {
                    client: part.client,
                    offset: part.start,
                })
                .collect(),
        )
    }

    fn place(&mut self, after: Option<Pos>, fragments: Vec<Fragment>) -> Option<usize> {
        let (ci, fi) = match after {
            None => (0, 0),
            Some(anchor) => {
                self.cut_after(anchor)?;
                let (ci, fi) = self.locate(anchor)?;
                (ci, fi + 1)
            }
        };
        let at = self.visible_before(ci, fi);
        if let [single] = fragments.as_slice()
            && fi > 0
        {
            let previous = &mut self.chunks[ci].fragments[fi - 1];
            if previous.client == single.client
                && previous.visible == single.visible
                && previous.end() == single.start
            {
                previous.len += single.len;
                self.recount(ci);
                return Some(at);
            }
        }
        let key = self.chunks[ci].key;
        for fragment in &fragments {
            self.index.insert(fragment.first(), key);
        }
        self.chunks[ci].fragments.splice(fi..fi, fragments);
        self.rebalance(ci);
        Some(at)
    }

    fn cut(&mut self, pos: Pos) -> Option<()> {
        let (ci, fi) = self.locate(pos)?;
        let fragment = self.chunks[ci].fragments[fi];
        if fragment.start == pos.offset {
            return Some(());
        }
        let left = pos.offset - fragment.start;
        self.chunks[ci].fragments[fi].len = left;
        self.chunks[ci].fragments.insert(
            fi + 1,
            Fragment {
                start: pos.offset,
                len: fragment.len - left,
                ..fragment
            },
        );
        self.index.insert(pos, self.chunks[ci].key);
        self.rebalance(ci);
        Some(())
    }

    fn cut_after(&mut self, pos: Pos) -> Option<()> {
        let (ci, fi) = self.locate(pos)?;
        let fragment = self.chunks[ci].fragments[fi];
        match pos.offset + 1 < fragment.end() {
            true => self.cut(Pos {
                client: pos.client,
                offset: pos.offset + 1,
            }),
            false => Some(()),
        }
    }

    fn locate(&self, pos: Pos) -> Option<Location> {
        let (start, key) = self.index.range(..=pos).next_back()?;
        if start.client != pos.client {
            return None;
        }
        let ci = *self.order.get(key)?;
        let fi = self.chunks[ci]
            .fragments
            .iter()
            .position(|fragment| fragment.first() == *start)?;
        (pos.offset < self.chunks[ci].fragments[fi].end()).then_some((ci, fi))
    }

    fn find_visible(&self, index: usize) -> Option<(usize, usize, usize)> {
        let mut remaining = index;
        for (ci, chunk) in self.chunks.iter().enumerate() {
            if remaining >= chunk.visible {
                remaining -= chunk.visible;
                continue;
            }
            for (fi, fragment) in chunk.fragments.iter().enumerate() {
                let len = fragment.visible_len();
                if remaining < len {
                    return Some((ci, fi, remaining));
                }
                remaining -= len;
            }
        }
        None
    }

    fn visible_before(&self, ci: usize, fi: usize) -> usize {
        let chunks: usize = self.chunks[..ci].iter().map(|chunk| chunk.visible).sum();
        let fragments: usize = self.chunks[ci].fragments[..fi]
            .iter()
            .map(|fragment| fragment.visible_len())
            .sum();
        chunks + fragments
    }

    fn fragments(&self) -> impl Iterator<Item = &Fragment> {
        self.chunks.iter().flat_map(|chunk| chunk.fragments.iter())
    }

    fn fragments_from(&self, ci: usize, fi: usize) -> impl Iterator<Item = Fragment> + '_ {
        self.chunks[ci].fragments[fi..]
            .iter()
            .chain(
                self.chunks[ci + 1..]
                    .iter()
                    .flat_map(|chunk| chunk.fragments.iter()),
            )
            .copied()
    }

    fn slice(&self, fragment: Fragment) -> &[T] {
        self.buffers
            .get(&fragment.client)
            .and_then(|buffer| buffer.get(fragment.start as usize..fragment.end() as usize))
            .unwrap_or_default()
    }

    fn recount(&mut self, ci: usize) {
        let chunk = &mut self.chunks[ci];
        let counted: usize = chunk
            .fragments
            .iter()
            .map(|fragment| fragment.visible_len())
            .sum();
        self.visible = self.visible - chunk.visible + counted;
        chunk.visible = counted;
    }

    fn rebalance(&mut self, ci: usize) {
        if self.chunks[ci].fragments.len() <= 2 * CHUNK {
            self.recount(ci);
            return;
        }
        let fragments = std::mem::take(&mut self.chunks[ci].fragments);
        let mut pieces = fragments.chunks(CHUNK).map(<[_]>::to_vec);
        self.chunks[ci].fragments = pieces.next().unwrap_or_default();
        self.recount(ci);
        let mut at = ci;
        for piece in pieces {
            at += 1;
            let key = self.new_key();
            for fragment in &piece {
                self.index.insert(fragment.first(), key);
            }
            self.chunks.insert(
                at,
                Chunk {
                    key,
                    fragments: piece,
                    visible: 0,
                },
            );
            self.recount(at);
        }
        self.reorder();
    }

    fn prune(&mut self) {
        if self.chunks.iter().all(|chunk| !chunk.fragments.is_empty()) {
            return;
        }
        self.chunks.retain(|chunk| !chunk.fragments.is_empty());
        if self.chunks.is_empty() {
            let key = self.new_key();
            self.chunks.push(Chunk {
                key,
                fragments: Vec::new(),
                visible: 0,
            });
        }
        self.reorder();
    }

    fn reorder(&mut self) {
        self.order = self
            .chunks
            .iter()
            .enumerate()
            .map(|(at, chunk)| (chunk.key, at))
            .collect();
    }

    fn new_key(&mut self) -> u32 {
        self.next_key += 1;
        self.next_key
    }
}

fn push_span(spans: &mut Vec<Span>, span: Span) {
    if let Some(previous) = spans.last_mut()
        && previous.client == span.client
        && previous.end() == span.start
    {
        previous.len += span.len;
        return;
    }
    spans.push(span);
}

impl<T: PartialEq> PartialEq for Sequence<T> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl<T: Eq> Eq for Sequence<T> {}

impl<T: fmt::Debug> fmt::Debug for Sequence<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}

impl<T: Serialize> Serialize for Sequence<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut items = serializer.serialize_seq(Some(self.len()))?;
        for item in self.iter() {
            items.serialize_element(item)?;
        }
        items.end()
    }
}

impl<'de, T: Deserialize<'de> + Clone> Deserialize<'de> for Sequence<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<T>::deserialize(deserializer).map(Self::from_items)
    }
}

#[cfg(any(test, feature = "fuzzing"))]
pub mod fuzz;

#[cfg(test)]
mod tests;

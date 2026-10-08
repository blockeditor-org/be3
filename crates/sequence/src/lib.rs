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
        items: Vec<T>,
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
        items: Vec<T>,
    },
    Move {
        first: Pos,
        last: Pos,
        after: Option<Pos>,
    },
}

impl<T> SeqOp<T> {
    pub fn absorb(&mut self, next: SeqOp<T>) -> Option<SeqOp<T>> {
        match (self, next) {
            (
                SeqOp::Insert {
                    client,
                    start,
                    items,
                    ..
                },
                SeqOp::Insert {
                    after: Some(after),
                    client: next_client,
                    start: next_start,
                    items: next_items,
                },
            ) if next_client == *client
                && next_start == *start + items.len() as u64
                && after
                    == (Pos {
                        client: *client,
                        offset: next_start - 1,
                    }) =>
            {
                items.extend(next_items);
                None
            }
            (SeqOp::Delete { spans }, SeqOp::Delete { spans: next }) => {
                spans.extend(next);
                None
            }
            (_, next) => Some(next),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Splice {
    pub at: usize,
    pub removed: usize,
    pub inserted: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Run<'a, T> {
    pub first: Pos,
    pub len: u64,
    pub visible: bool,
    pub items: &'a [T],
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
pub struct State {
    next: BTreeMap<u64, u64>,
    fragments: Vec<Fragment>,
}

#[derive(Clone)]
pub struct Sequence<T> {
    runs: BTreeMap<Pos, Vec<T>>,
    next: BTreeMap<u64, u64>,
    chunks: Vec<Chunk>,
    index: BTreeMap<Pos, u32>,
    order: HashMap<u32, usize>,
    next_key: u32,
    visible: usize,
    flat: Option<Vec<T>>,
}

type Location = (usize, usize);

impl<T> Default for Sequence<T> {
    fn default() -> Self {
        Self::build(BTreeMap::new(), BTreeMap::new(), Vec::new())
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
        let next = BTreeMap::from([(LOADED, items.len() as u64)]);
        let runs = BTreeMap::from([(
            Pos {
                client: LOADED,
                offset: 0,
            },
            items,
        )]);
        Self::build(runs, next, fragments)
    }

    pub fn items(&self) -> Vec<T> {
        self.iter().cloned().collect()
    }

    pub fn mirror(&mut self) {
        self.flat = Some(self.items());
    }

    pub fn refreshed(&self) -> Self {
        Self::from_items(self.items())
    }

    pub fn from_state(state: State, visible: &[T]) -> Result<Self, Malformed> {
        let mut seen: BTreeMap<u64, Vec<(u64, u64)>> = BTreeMap::new();
        let mut shown = 0usize;
        for fragment in &state.fragments {
            let next = state.next.get(&fragment.client).copied().unwrap_or(0);
            let end = fragment.start.checked_add(fragment.len).ok_or(Malformed)?;
            if fragment.len == 0 || end > next {
                return Err(Malformed);
            }
            if fragment.visible {
                shown = usize::try_from(fragment.len)
                    .ok()
                    .and_then(|len| shown.checked_add(len))
                    .ok_or(Malformed)?;
            }
            seen.entry(fragment.client)
                .or_default()
                .push((fragment.start, end));
        }
        if shown != visible.len() {
            return Err(Malformed);
        }
        for ranges in seen.values_mut() {
            ranges.sort_unstable();
            if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
                return Err(Malformed);
            }
        }
        let shown: Vec<Fragment> = state
            .fragments
            .iter()
            .filter(|fragment| fragment.visible)
            .copied()
            .collect();
        let mut sequence = Self::build(BTreeMap::new(), state.next, state.fragments);
        let mut rest = visible;
        for fragment in shown {
            let (taken, after) = rest.split_at(fragment.len as usize);
            sequence.store(fragment.first(), taken);
            rest = after;
        }
        Ok(sequence)
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
    pub fn state(&self) -> State {
        State {
            next: self.next.clone(),
            fragments: self.fragments().copied().collect(),
        }
    }

    fn build(
        runs: BTreeMap<Pos, Vec<T>>,
        next: BTreeMap<u64, u64>,
        fragments: Vec<Fragment>,
    ) -> Self {
        let mut sequence = Self {
            runs,
            next,
            chunks: Vec::new(),
            index: BTreeMap::new(),
            order: HashMap::new(),
            next_key: 0,
            visible: 0,
            flat: None,
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
        let loaded = self.next_offset(LOADED) as usize;
        let whole = match fragments.next() {
            None => loaded == 0,
            Some(fragment) => {
                fragment.client == LOADED
                    && fragment.start == 0
                    && fragment.len as usize == loaded
                    && fragment.visible
            }
        };
        whole && fragments.next().is_none() && self.next.keys().all(|client| *client == LOADED)
    }

    pub fn next_offset(&self, client: u64) -> u64 {
        self.next.get(&client).copied().unwrap_or(0)
    }

    pub fn mirrored(&self) -> Option<&[T]> {
        self.flat.as_deref()
    }

    pub fn chunk(&self, index: usize) -> &[T] {
        if let Some(flat) = &self.flat {
            return flat.get(index..).unwrap_or_default();
        }
        let Some((ci, fi, inner)) = self.find_visible(index) else {
            return &[];
        };
        &self.slice(self.chunks[ci].fragments[fi])[inner..]
    }

    pub fn previous(&self, pos: Pos) -> Option<Option<Pos>> {
        let (ci, fi) = self.locate(pos)?;
        let fragment = self.chunks[ci].fragments[fi];
        if pos.offset > fragment.start {
            return Some(Some(Pos {
                client: pos.client,
                offset: pos.offset - 1,
            }));
        }
        let before = self.chunks[..ci]
            .iter()
            .flat_map(|chunk| chunk.fragments.iter())
            .chain(&self.chunks[ci].fragments[..fi])
            .next_back()
            .copied();
        Some(before.map(|fragment| Pos {
            client: fragment.client,
            offset: fragment.end() - 1,
        }))
    }

    pub fn place_of(&self, pos: Pos) -> Option<(usize, bool)> {
        let (ci, fi) = self.locate(pos)?;
        let fragment = self.chunks[ci].fragments[fi];
        let before = self.visible_before(ci, fi);
        Some(match fragment.visible {
            true => (before + (pos.offset - fragment.start) as usize, true),
            false => (before, false),
        })
    }

    pub fn slices(&self) -> impl Iterator<Item = &[T]> {
        self.fragments()
            .filter(|fragment| fragment.visible)
            .map(|fragment| self.slice(*fragment))
    }

    pub fn runs(&self) -> impl Iterator<Item = Run<'_, T>> {
        self.fragments().map(|fragment| Run {
            first: fragment.first(),
            len: fragment.len,
            visible: fragment.visible,
            items: self.slice(*fragment),
        })
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
            SeqOp::Undelete { spans, items } => {
                Self::carries(spans, items) && !self.parts(spans, false).is_empty()
            }
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
            SeqOp::Swap { hide, show, items } => {
                !(hide.is_empty() && show.is_empty())
                    && Self::carries(show, items)
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
                    SeqOp::Undelete {
                        spans,
                        items: items.clone(),
                    },
                )
            }
            SeqOp::Delete { spans } => {
                let spans = self.parts(spans, true);
                let items = self.read(&spans)?;
                (
                    SeqOp::Undelete {
                        spans: spans.clone(),
                        items,
                    },
                    SeqOp::Delete { spans },
                )
            }
            SeqOp::Undelete { spans, .. } => (
                SeqOp::Delete {
                    spans: self.parts(spans, false),
                },
                operation.clone(),
            ),
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
                        items: self.read(spans)?,
                    },
                    SeqOp::Swap {
                        hide: spans.clone(),
                        show: inserted,
                        items: items.clone(),
                    },
                )
            }
            SeqOp::Swap { hide, show, .. } => (
                SeqOp::Swap {
                    hide: show.clone(),
                    show: hide.clone(),
                    items: self.read(hide)?,
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
            SeqOp::Undelete { spans, items } => self.show(spans, items, &mut splices),
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
            SeqOp::Swap { hide, show, items } => {
                self.set_visible(hide, false, &mut splices);
                self.show(show, items, &mut splices);
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
            self.next
                .insert(*client, start.saturating_add(items.len() as u64));
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
        self.store(
            Pos {
                client,
                offset: start,
            },
            items,
        );
        self.next
            .insert(client, start.saturating_add(items.len() as u64));
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

    fn set_visible(&mut self, spans: &[Span], visible: bool, splices: &mut Vec<Splice>)
    where
        T: Clone,
    {
        for span in spans {
            let Some(starts) = self.isolate(*span) else {
                continue;
            };
            for &start in &starts {
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
                if self.flat.is_some() {
                    let items = self.slice(fragment).to_vec();
                    if let Some(flat) = &mut self.flat {
                        match visible {
                            true => drop(flat.splice(at..at, items)),
                            false => drop(flat.drain(at..at + len)),
                        }
                    }
                }
                self.chunks[ci].fragments[fi].visible = visible;
                self.recount(ci);
            }
            for start in starts {
                self.join(start);
            }
            if let Some(next) = self.next_first(span.last()) {
                self.join(next);
            }
        }
    }

    fn next_first(&self, pos: Pos) -> Option<Pos> {
        let (ci, fi) = self.locate(pos)?;
        match self.chunks[ci].fragments.get(fi + 1) {
            Some(next) => Some(next.first()),
            None => self
                .chunks
                .get(ci + 1)?
                .fragments
                .first()
                .map(|next| next.first()),
        }
    }

    fn join(&mut self, pos: Pos) {
        let Some((ci, fi)) = self.locate(pos) else {
            return;
        };
        let (pci, pfi) = match fi {
            0 if ci == 0 => return,
            0 => (ci - 1, self.chunks[ci - 1].fragments.len().wrapping_sub(1)),
            _ => (ci, fi - 1),
        };
        let Some(&previous) = self.chunks[pci].fragments.get(pfi) else {
            return;
        };
        let current = self.chunks[ci].fragments[fi];
        if previous.client != current.client
            || previous.visible != current.visible
            || previous.end() != current.start
        {
            return;
        }
        let joined = Fragment {
            len: previous.len + current.len,
            ..previous
        };
        let held = |fragment| !self.slice(fragment).is_empty();
        if held(previous) != held(current) || held(previous) != held(joined) {
            return;
        }
        self.chunks[pci].fragments[pfi] = joined;
        self.chunks[ci].fragments.remove(fi);
        self.index.remove(&current.first());
        self.recount(pci);
        self.recount(ci);
        self.prune();
    }

    fn relocate(
        &mut self,
        first: Pos,
        last: Pos,
        after: Option<Pos>,
        splices: &mut Vec<Splice>,
    ) -> Option<()>
    where
        T: Clone,
    {
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
        if let Some(flat) = &mut self.flat {
            flat.drain(at..at + count);
        }
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

    fn place(&mut self, after: Option<Pos>, fragments: Vec<Fragment>) -> Option<usize>
    where
        T: Clone,
    {
        let (ci, fi) = match after {
            None => (0, 0),
            Some(anchor) => {
                self.cut_after(anchor)?;
                let (ci, fi) = self.locate(anchor)?;
                (ci, fi + 1)
            }
        };
        let at = self.visible_before(ci, fi);
        if self.flat.is_some() {
            let items: Vec<T> = fragments
                .iter()
                .filter(|fragment| fragment.visible)
                .flat_map(|fragment| self.slice(*fragment).iter().cloned())
                .collect();
            if let Some(flat) = &mut self.flat {
                flat.splice(at..at, items);
            }
        }
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
        let Some((first, run)) = self.runs.range(..=fragment.first()).next_back() else {
            return &[];
        };
        if first.client != fragment.client {
            return &[];
        }
        let offset = (fragment.start - first.offset) as usize;
        run.get(offset..offset + fragment.len as usize)
            .unwrap_or_default()
    }

    fn store(&mut self, at: Pos, items: &[T])
    where
        T: Clone,
    {
        let start = match self.runs.range_mut(..=at).next_back() {
            Some((first, run))
                if first.client == at.client && at.offset - first.offset <= run.len() as u64 =>
            {
                let offset = (at.offset - first.offset) as usize;
                let overlap = (run.len() - offset).min(items.len());
                run[offset..offset + overlap].clone_from_slice(&items[..overlap]);
                run.extend_from_slice(&items[overlap..]);
                *first
            }
            _ => {
                self.runs.insert(at, items.to_vec());
                at
            }
        };
        let Some(run) = self.runs.get(&start) else {
            return;
        };
        let after = Pos {
            client: start.client,
            offset: start.offset + run.len() as u64,
        };
        if let Some(next) = self.runs.remove(&after)
            && let Some(run) = self.runs.get_mut(&start)
        {
            run.extend(next);
        }
    }

    fn read(&self, spans: &[Span]) -> Option<Vec<T>>
    where
        T: Clone,
    {
        let mut items = Vec::new();
        for span in spans {
            for (fragment, part) in self.walk(*span)? {
                let content = self.slice(fragment);
                let offset = (part.start - fragment.start) as usize;
                items.extend_from_slice(content.get(offset..offset + part.len as usize)?);
            }
        }
        Some(items)
    }

    fn show(&mut self, spans: &[Span], items: &[T], splices: &mut Vec<Splice>)
    where
        T: Clone,
    {
        let mut rest = items;
        for span in spans {
            let (carried, after) = rest.split_at((span.len as usize).min(rest.len()));
            rest = after;
            let Some(pieces) = self.walk(*span) else {
                continue;
            };
            let mut offset = 0;
            for (fragment, part) in pieces {
                let len = part.len as usize;
                if !fragment.visible {
                    self.store(
                        Pos {
                            client: part.client,
                            offset: part.start,
                        },
                        &carried[offset..offset + len],
                    );
                }
                offset += len;
            }
            self.set_visible(std::slice::from_ref(span), true, splices);
        }
    }

    fn carries(spans: &[Span], items: &[T]) -> bool {
        spans
            .iter()
            .try_fold(0u64, |total, span| total.checked_add(span.len))
            .is_some_and(|total| total == items.len() as u64)
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

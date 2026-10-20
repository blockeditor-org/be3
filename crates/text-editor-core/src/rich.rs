use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::document::{anchor_in, place_in};
use crate::{Pos, SeqOp, Sequence};

pub const VOID: &str = "\u{fffc}";

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Name(Arc<str>);

impl Name {
    pub fn new(name: &str) -> Self {
        Self(Arc::from(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Name {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", &*self.0)
    }
}

impl fmt::Display for Name {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for Name {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Name {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|name| Self(Arc::from(name)))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Text(Name),
}

impl Value {
    pub fn text(text: &str) -> Self {
        Self::Text(Name::new(text))
    }

    pub const fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value.as_str()),
            _ => None,
        }
    }

    pub const fn is_true(&self) -> bool {
        matches!(self, Self::Bool(true))
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct MarkSet(Arc<[(Name, Value)]>);

impl MarkSet {
    pub fn new(pairs: impl IntoIterator<Item = (Name, Value)>) -> Self {
        let mut pairs: Vec<(Name, Value)> = pairs.into_iter().collect();
        pairs.sort_by(|left, right| left.0.cmp(&right.0));
        pairs.dedup_by(|later, earlier| later.0 == earlier.0);
        Self(pairs.into())
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0
            .binary_search_by(|(name, _)| name.as_str().cmp(key))
            .ok()
            .map(|at| &self.0[at].1)
    }

    pub fn has(&self, key: &str, value: &Value) -> bool {
        self.get(key) == Some(value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Name, &Value)> {
        self.0.iter().map(|(name, value)| (name, value))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn with(&self, key: &Name, value: Option<Value>) -> Self {
        Self::new(
            self.iter()
                .filter(|(name, _)| *name != key)
                .map(|(name, value)| (name.clone(), value.clone()))
                .chain(value.map(|value| (key.clone(), value))),
        )
    }

    pub fn shared(&self, other: &Self) -> Self {
        Self::new(
            self.iter()
                .filter(|(name, value)| other.has(name.as_str(), value))
                .map(|(name, value)| (name.clone(), value.clone())),
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Expand {
    After,
    Never,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Scope {
    Inline,
    Block,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FormatOp {
    pub start: Option<Pos>,
    pub end: Pos,
    pub scope: Scope,
    pub expand: Expand,
    pub key: Name,
    pub value: Option<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Change {
    pub scope: Scope,
    pub expand: Expand,
    pub key: Name,
    pub value: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RichOp {
    Text(SeqOp<u8>),
    Format(FormatOp),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MarkId {
    pub clock: u64,
    pub client: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Mark {
    pub id: MarkId,
    pub op: FormatOp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Seed {
    pub range: Range<usize>,
    pub scope: Scope,
    pub expand: Expand,
    pub key: Name,
    pub value: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormatRun {
    pub len: usize,
    pub marks: MarkSet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub start: usize,
    pub end: usize,
    pub id: Pos,
    pub props: MarkSet,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Formats {
    pub runs: Vec<FormatRun>,
    pub blocks: Vec<Block>,
    starts: Vec<usize>,
}

impl Formats {
    pub fn new(runs: Vec<FormatRun>, blocks: Vec<Block>) -> Self {
        let mut at = 0;
        let starts = runs
            .iter()
            .map(|run| {
                let start = at;
                at += run.len;
                start
            })
            .collect();
        Self {
            runs,
            blocks,
            starts,
        }
    }

    pub fn marks_at(&self, index: usize) -> Option<&MarkSet> {
        let run = self
            .starts
            .partition_point(|start| *start <= index)
            .checked_sub(1)?;
        let run = &self.runs[run];
        (index < self.starts[self.starts.len() - 1] + self.runs[self.runs.len() - 1].len)
            .then_some(&run.marks)
    }

    pub fn block_at(&self, index: usize) -> Option<&Block> {
        let at = self.blocks.partition_point(|block| block.end < index);
        self.blocks.get(at)
    }

    pub fn marks_in(&self, range: Range<usize>) -> impl Iterator<Item = (Range<usize>, &MarkSet)> {
        let first = self
            .starts
            .partition_point(|start| *start <= range.start)
            .saturating_sub(1);
        self.starts[first..]
            .iter()
            .zip(&self.runs[first..])
            .map(move |(start, run)| {
                (
                    (*start).max(range.start)..(start + run.len).min(range.end),
                    &run.marks,
                )
            })
            .take_while(move |(within, _)| within.start < range.end)
            .filter(|(within, _)| within.start < within.end)
    }
}

#[derive(Default)]
struct Painted {
    spans: Vec<(Range<usize>, Value)>,
}

impl Painted {
    fn paint(&mut self, range: Range<usize>, value: Option<&Value>) {
        if range.start >= range.end {
            return;
        }
        let first = self
            .spans
            .partition_point(|(span, _)| span.end <= range.start);
        let last = self
            .spans
            .partition_point(|(span, _)| span.start < range.end);
        let mut replaced: Vec<(Range<usize>, Value)> = Vec::new();
        if let Some((span, held)) = self.spans.get(first).filter(|_| first < last)
            && span.start < range.start
        {
            replaced.push((span.start..range.start, held.clone()));
        }
        if let Some(value) = value {
            replaced.push((range.clone(), value.clone()));
        }
        if let Some((span, held)) = last
            .checked_sub(1)
            .and_then(|at| self.spans.get(at))
            .filter(|_| first < last)
            && span.end > range.end
        {
            replaced.push((range.end..span.end, held.clone()));
        }
        self.spans.splice(first..last, replaced);
    }

    fn at(&self, index: usize) -> Option<&Value> {
        let at = self.spans.partition_point(|(span, _)| span.end <= index);
        self.spans
            .get(at)
            .filter(|(span, _)| span.start <= index)
            .map(|(_, value)| value)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Marks {
    marks: Vec<Mark>,
    clock: u64,
}

impl Marks {
    pub fn loaded(text: &Sequence<u8>, seeds: &[Seed]) -> Self {
        let bytes = text.items();
        let mut marks = Self::default();
        for seed in seeds {
            if let Some(op) = Self::op(
                text,
                &bytes,
                seed.range.clone(),
                seed.scope,
                seed.expand,
                &seed.key,
                Some(seed.value.clone()),
            ) {
                marks.mint(sequence::LOADED, op);
            }
        }
        marks
    }

    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    pub fn mint(&mut self, client: u64, op: FormatOp) -> Mark {
        let mark = Mark {
            id: MarkId {
                clock: self.clock + 1,
                client,
            },
            op,
        };
        self.apply(mark.clone());
        mark
    }

    pub fn apply(&mut self, mark: Mark) -> bool {
        self.clock = self.clock.max(mark.id.clock);
        match self.marks.binary_search_by_key(&mark.id, |held| held.id) {
            Ok(_) => false,
            Err(at) => {
                self.marks.insert(at, mark);
                true
            }
        }
    }

    pub fn op(
        text: &Sequence<u8>,
        bytes: &[u8],
        range: Range<usize>,
        scope: Scope,
        expand: Expand,
        key: &Name,
        value: Option<Value>,
    ) -> Option<FormatOp> {
        let end = range.end.min(bytes.len());
        let start = range.start.min(end);
        let (start, end) = match (scope, expand) {
            (Scope::Block, _) => {
                let first = line_start(bytes, start);
                let start = match first {
                    0 => None,
                    first => Some(anchor_in(text, first - 1)?),
                };
                return Some(FormatOp {
                    start,
                    end: anchor_in(text, line_end(bytes, end))?,
                    scope,
                    expand,
                    key: key.clone(),
                    value,
                });
            }
            (Scope::Inline, _) if start == end => return None,
            (Scope::Inline, Expand::After) => (start, end),
            (Scope::Inline, Expand::Never) => (start, end - 1),
        };
        Some(FormatOp {
            start: Some(anchor_in(text, start)?),
            end: anchor_in(text, end)?,
            scope,
            expand,
            key: key.clone(),
            value,
        })
    }

    pub fn covered(text: &Sequence<u8>, bytes: &[u8], op: &FormatOp) -> Option<Range<usize>> {
        let (start, end) = span_of(text, op)?;
        match op.scope {
            Scope::Block => (start <= end)
                .then(|| line_start(bytes, start.min(bytes.len()))..(end + 1).min(bytes.len())),
            Scope::Inline => (start < end).then_some(start..end),
        }
    }

    pub fn inverse(&self, text: &Sequence<u8>, bytes: &[u8], op: &FormatOp) -> Vec<FormatOp> {
        let Some((start, end)) = span_of(text, op) else {
            return Vec::new();
        };
        let formats = self.formats(text, bytes);
        let mut values: Vec<(Range<usize>, Option<Value>)> = Vec::new();
        let mut extend =
            |range: Range<usize>, value: Option<Value>, adjacent: usize| match values.last_mut() {
                Some((last, held)) if *held == value && last.end + adjacent >= range.start => {
                    last.end = range.end;
                }
                _ => values.push((range, value)),
            };
        match op.scope {
            Scope::Block => {
                for block in formats
                    .blocks
                    .iter()
                    .filter(|block| start <= block.end && block.end <= end)
                {
                    extend(
                        block.start..block.end,
                        block.props.get(op.key.as_str()).cloned(),
                        1,
                    );
                }
            }
            Scope::Inline => {
                for (range, marks) in formats.marks_in(start..end) {
                    extend(range, marks.get(op.key.as_str()).cloned(), 0);
                }
            }
        }
        values
            .into_iter()
            .filter_map(|(range, value)| {
                Self::op(text, bytes, range, op.scope, op.expand, &op.key, value)
            })
            .collect()
    }

    pub fn formats(&self, text: &Sequence<u8>, bytes: &[u8]) -> Formats {
        let len = bytes.len();
        let terminators: Vec<usize> = memchr::memchr_iter(b'\n', bytes)
            .chain(std::iter::once(len))
            .collect();
        let mut inline: Vec<(Name, Painted)> = Vec::new();
        let mut block: Vec<(Name, Painted)> = Vec::new();
        for mark in &self.marks {
            let Some((start, end)) = span_of(text, &mark.op) else {
                continue;
            };
            let (painted, range) = match mark.op.scope {
                Scope::Inline => (&mut inline, start..end.min(len)),
                Scope::Block => (
                    &mut block,
                    terminators.partition_point(|terminator| *terminator < start)
                        ..terminators.partition_point(|terminator| *terminator <= end),
                ),
            };
            let at = match painted.iter().position(|(name, _)| *name == mark.op.key) {
                Some(at) => at,
                None => {
                    painted.push((mark.op.key.clone(), Painted::default()));
                    painted.len() - 1
                }
            };
            painted[at].1.paint(range, mark.op.value.as_ref());
        }
        let resolve = |painted: &[(Name, Painted)], index: usize| {
            MarkSet::new(
                painted
                    .iter()
                    .filter_map(|(name, painted)| Some((name.clone(), painted.at(index)?.clone()))),
            )
        };
        let mut bounds: Vec<usize> = inline
            .iter()
            .flat_map(|(_, painted)| painted.spans.iter())
            .flat_map(|(span, _)| [span.start, span.end])
            .chain([0, len + 1])
            .filter(|at| *at <= len + 1)
            .collect();
        bounds.sort_unstable();
        bounds.dedup();
        let mut runs: Vec<FormatRun> = Vec::new();
        for window in bounds.windows(2) {
            let marks = resolve(&inline, window[0]);
            let len = window[1] - window[0];
            match runs.last_mut() {
                Some(last) if last.marks == marks => last.len += len,
                _ => runs.push(FormatRun { len, marks }),
            }
        }
        let mut start = 0;
        let blocks = terminators
            .iter()
            .enumerate()
            .map(|(line, terminator)| {
                let held = Block {
                    start,
                    end: *terminator,
                    id: anchor_in(text, *terminator).unwrap_or(Pos {
                        client: sequence::LOADED,
                        offset: 0,
                    }),
                    props: resolve(&block, line),
                };
                start = terminator + 1;
                held
            })
            .collect();
        Formats::new(runs, blocks)
    }
}

fn after(text: &Sequence<u8>, anchor: Pos) -> Option<usize> {
    match text.place_of(anchor) {
        Some((index, true)) => Some(index + 1),
        Some((index, false)) => Some(index),
        None => place_in(text, anchor),
    }
}

fn span_of(text: &Sequence<u8>, op: &FormatOp) -> Option<(usize, usize)> {
    let start = match (op.scope, op.start) {
        (_, None) => 0,
        (Scope::Block, Some(start)) => after(text, start)?,
        (Scope::Inline, Some(start)) => place_in(text, start)?,
    };
    let end = match (op.scope, op.expand) {
        (Scope::Inline, Expand::Never) => after(text, op.end)?,
        _ => place_in(text, op.end)?,
    };
    Some((start, end))
}

pub fn line_start(bytes: &[u8], index: usize) -> usize {
    memchr::memrchr(b'\n', &bytes[..index.min(bytes.len())]).map_or(0, |newline| newline + 1)
}

pub fn line_end(bytes: &[u8], index: usize) -> usize {
    let index = index.min(bytes.len());
    memchr::memchr(b'\n', &bytes[index..]).map_or(bytes.len(), |newline| index + newline)
}

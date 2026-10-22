use std::collections::{BTreeSet, VecDeque};

use uuid::Uuid;

use crate::marks::{Expand, Name, Scope, Seed, Value as MarkValue};
use crate::{Anchor, Change, Document, Edit, List, Model, ObjectId, Place, RichText, Step, Value};

const CLIENTS: usize = 3;

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Board {
    columns: List<Column>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Column {
    name: String,
    cards: List<Card>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Card {
    text: String,
    under: List<Card>,
    note: RichText,
}

const FORMATS: [(&str, Scope); 3] = [
    ("bold", Scope::Inline),
    ("italic", Scope::Inline),
    ("heading", Scope::Block),
];

pub fn lists(data: &[u8]) {
    let mut input = Input(data);
    let mut authority = Document::new(&Board::default());
    for column in 0..1 + input.below(3) {
        let id = fresh(&mut input);
        let made = Column {
            name: format!("column {column}"),
            cards: List::default(),
        };
        let mut edit = vec![Board::COLUMNS.insert_as(id, ObjectId::ROOT, Anchor::End, &made)];
        for card in 0..input.below(4) {
            let note = format!("note {column}.{card}\nsecond line");
            let seeds = match input.byte() % 2 {
                0 => Vec::new(),
                _ => vec![Seed {
                    range: 0..4,
                    scope: Scope::Inline,
                    expand: Expand::After,
                    key: Name::new("bold"),
                    value: MarkValue::Bool(true),
                }],
            };
            let made = Card {
                text: format!("card {column}.{card}"),
                under: List::default(),
                note: RichText::new(note, seeds),
            };
            edit.push(Column::CARDS.insert_as(fresh(&mut input), id, Anchor::End, &made));
        }
        authority.apply(&Edit(edit));
    }
    let mut log: Vec<(usize, Edit)> = Vec::new();
    let mut clients: Vec<Client> = (0..CLIENTS).map(|id| Client::new(id, &authority)).collect();
    while !input.is_empty() {
        let client = input.below(CLIENTS);
        match input.byte() % 8 {
            0..=3 => clients[client].edit(&mut input),
            4 | 5 => {
                if let Some(edit) = clients[client].send() {
                    authority.apply(&edit);
                    check(&authority);
                    log.push((client, edit));
                }
            }
            6 => clients[client].catch_up(&log),
            _ => {
                let mut reloaded = Document::<Board>::from_bytes(&authority.to_bytes())
                    .expect("a document's own bytes decode");
                reloaded
                    .adopt_session_state(&authority.session_state())
                    .expect("a document's own session state is well formed");
                assert_eq!(reloaded, authority);
                assert_eq!(reloaded.session_state(), authority.session_state());
                check(&reloaded);
                authority = reloaded;
            }
        }
    }
    for client in &mut clients {
        while let Some(edit) = client.send() {
            authority.apply(&edit);
            check(&authority);
            log.push((client.id, edit));
        }
    }
    for client in &mut clients {
        client.catch_up(&log);
        assert!(client.pending.is_empty());
        for replica in [&client.confirmed, &client.visible] {
            assert_eq!(*replica, authority);
            assert_eq!(replica.session_state(), authority.session_state());
        }
    }
}

struct Client {
    id: usize,
    confirmed: Document<Board>,
    visible: Document<Board>,
    pending: VecDeque<Edit>,
    sent: usize,
    seen: usize,
    undo: Vec<Step>,
    ever: BTreeSet<ObjectId>,
}

impl Client {
    fn new(id: usize, authority: &Document<Board>) -> Self {
        Self {
            id,
            confirmed: authority.clone(),
            visible: authority.clone(),
            pending: VecDeque::new(),
            sent: 0,
            seen: 0,
            undo: Vec::new(),
            ever: objects(authority).into_iter().collect(),
        }
    }

    fn edit(&mut self, input: &mut Input) {
        crate::set_local_client(self.id as u64 + 1);
        let edit = match input.byte() % 10 {
            5 => match self.undo.pop() {
                Some(step) => step.undo(),
                None => return,
            },
            kind @ 6..=9 => {
                let Some(change) = self.write(kind, input) else {
                    return;
                };
                change.into()
            }
            kind => {
                let Some(change) = self.change(kind, input) else {
                    return;
                };
                change.into()
            }
        };
        if let Some(step) = self.visible.step(&edit) {
            self.undo.push(step);
        }
        self.visible.apply(&edit);
        check(&self.visible);
        self.ever.extend(objects(&self.visible));
        self.pending.push_back(edit);
    }

    fn change(&self, kind: u8, input: &mut Input) -> Option<Change> {
        let lists = places(&self.visible);
        let objects = objects(&self.visible);
        let place = *pick(input, &lists)?;
        let anchor = match input.byte() % 4 {
            0 => Anchor::Start,
            1 => Anchor::End,
            _ => match pick(input, &self.ever.iter().copied().collect::<Vec<_>>()) {
                Some(id) => Anchor::After(*id),
                None => Anchor::End,
            },
        };
        match kind {
            0 | 1 => {
                let id = fresh(input);
                let card = Card {
                    text: format!("{:02x}", input.byte()),
                    under: List::default(),
                    note: RichText::default(),
                };
                (place.field == Column::CARDS.index())
                    .then(|| Column::CARDS.insert_as(id, place.object, anchor, &card))
            }
            2 => pick(input, &objects).map(|id| Change::remove(*id)),
            _ => pick(input, &objects).map(|id| Change::Move {
                object: *id,
                place,
                anchor,
                client: crate::local_client(),
            }),
        }
    }

    fn write(&self, kind: u8, input: &mut Input) -> Option<Change> {
        let cards: Vec<ObjectId> = objects(&self.visible)
            .into_iter()
            .filter(|id| self.visible.rich(*id, Card::NOTE).is_some())
            .collect();
        let card = *pick(input, &cards)?;
        let rich = self.visible.rich(card, Card::NOTE)?;
        let len = rich.text.len();
        let client = crate::local_client();
        let start = input.below(len + 1);
        let end = start + input.below(len - start + 1);
        match kind {
            6 => {
                let typed = match input.byte() % 4 {
                    0 => b"\n".to_vec(),
                    _ => format!("{:02x}", input.byte()).into_bytes(),
                };
                let op = rich.text.insert(client, start, typed)?;
                Some(Card::NOTE.edit(card, op))
            }
            7 => Some(Card::NOTE.edit(card, rich.text.delete(start..end)?)),
            8 => {
                let (key, scope) = FORMATS[input.below(FORMATS.len())];
                let value = (!input.byte().is_multiple_of(3)).then_some(MarkValue::Bool(true));
                let expand = match input.byte() % 2 {
                    0 => Expand::After,
                    _ => Expand::Never,
                };
                let mark =
                    rich.format(client, start..end, scope, expand, &Name::new(key), value)?;
                Some(Card::NOTE.mark(card, mark))
            }
            _ => {
                let marks = rich.marks.marks();
                let mark = pick(input, marks)?;
                Some(Card::NOTE.unmark(card, mark.id))
            }
        }
    }

    fn send(&mut self) -> Option<Edit> {
        let edit = self.pending.get(self.sent)?;
        self.sent += 1;
        Some(postcard::from_bytes(&wire(edit)).expect("an edit decodes"))
    }

    fn catch_up(&mut self, log: &[(usize, Edit)]) {
        for (author, edit) in &log[self.seen..] {
            if *author == self.id {
                self.confirmed.apply(edit);
                let mine = self.pending.pop_front().map(|mine| wire(&mine));
                assert_eq!(mine, Some(wire(edit)));
                self.sent -= 1;
            } else if self.pending.is_empty() {
                self.confirmed.apply(edit);
                self.visible.apply(edit);
            } else {
                let local: Vec<Edit> = self.pending.iter().cloned().collect();
                self.visible
                    .rebase(&mut self.confirmed, edit, &local, &mut Vec::new());
                let mut rebuilt = self.confirmed.clone();
                for edit in &local {
                    rebuilt.apply(edit);
                }
                assert_eq!(self.visible, rebuilt);
                assert_eq!(self.visible.session_state(), rebuilt.session_state());
            }
        }
        self.seen = log.len();
        check(&self.confirmed);
        check(&self.visible);
        if self.pending.is_empty() {
            assert_eq!(self.visible, self.confirmed);
            assert_eq!(self.visible.session_state(), self.confirmed.session_state());
        }
    }
}

fn wire(edit: &Edit) -> Vec<u8> {
    postcard::to_stdvec(edit).expect("an edit encodes")
}

fn places(document: &Document<Board>) -> Vec<Place> {
    let mut places = Vec::new();
    for (id, object) in document.tree().objects() {
        for (index, value) in object.fields().iter().enumerate() {
            if let (Value::List(_), Ok(field)) = (value, u16::try_from(index)) {
                places.push(Place { object: *id, field });
            }
        }
    }
    places
}

fn objects(document: &Document<Board>) -> Vec<ObjectId> {
    document
        .tree()
        .objects()
        .keys()
        .copied()
        .filter(|id| *id != ObjectId::ROOT)
        .collect()
}

fn check(document: &Document<Board>) {
    let tree = document.tree();
    let mut listed = BTreeSet::new();
    for place in places(document) {
        let Some(Value::List(items)) = tree
            .object(place.object)
            .and_then(|object| object.fields().get(usize::from(place.field)))
        else {
            unreachable!("a place came from a list");
        };
        for (index, id) in items.iter().enumerate() {
            assert!(listed.insert(*id), "{id} is listed twice");
            assert_eq!(items.index_of(*id), Some(index));
            let object = tree.object(*id).expect("a listed object exists");
            assert_eq!(
                object.parent(),
                Some(place),
                "{id} is listed outside its parent"
            );
        }
    }
    for (id, object) in tree.objects() {
        if *id == ObjectId::ROOT {
            assert_eq!(object.parent(), None);
            continue;
        }
        assert!(listed.contains(id), "{id} is in no list");
        let mut seen = BTreeSet::from([*id]);
        let mut cursor = object.parent().map(|place| place.object);
        while let Some(up) = cursor {
            assert!(seen.insert(up), "{id} is inside itself");
            cursor = tree
                .object(up)
                .and_then(|held| held.parent())
                .map(|place| place.object);
        }
    }
}

fn fresh(input: &mut Input) -> ObjectId {
    ObjectId::from_uuid(Uuid::from_u64_pair(input.number(), input.number()))
}

fn pick<'a, T>(input: &mut Input, from: &'a [T]) -> Option<&'a T> {
    match from.len() {
        0 => None,
        len => from.get(input.below(len)),
    }
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
}

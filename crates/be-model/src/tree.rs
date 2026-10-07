use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    Anchor, Change, Malformed, Model, Object, ObjectId, Objects, Place, Sequence, Touched, Value,
    sequence::State,
};

pub(crate) type Gone = BTreeMap<ObjectId, (Place, Anchor)>;

#[derive(Default, Deserialize, Serialize)]
struct Session {
    gone: Gone,
    texts: BTreeMap<Place, State<u8>>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Tree {
    objects: Objects,
    #[serde(skip)]
    gone: Gone,
}

impl PartialEq for Tree {
    fn eq(&self, other: &Self) -> bool {
        self.objects == other.objects
    }
}

impl Eq for Tree {}

impl Tree {
    pub(crate) fn from_objects(objects: impl IntoIterator<Item = (ObjectId, Object)>) -> Self {
        Self {
            objects: objects.into_iter().collect(),
            gone: Gone::new(),
        }
    }

    pub(crate) fn objects(&self) -> &Objects {
        &self.objects
    }

    pub(crate) fn session_state(&self) -> Vec<u8> {
        let texts: BTreeMap<Place, State<u8>> = self
            .texts()
            .filter(|(_, sequence)| !sequence.is_fresh())
            .map(|(place, sequence)| (place, sequence.state()))
            .collect();
        if self.gone.is_empty() && texts.is_empty() {
            return Vec::new();
        }
        let session = Session {
            gone: self.gone.clone(),
            texts,
        };
        postcard::to_stdvec(&session).unwrap_or_default()
    }

    pub(crate) fn adopt_session_state(&mut self, bytes: &[u8]) -> Result<(), Malformed> {
        let Session { gone, texts } = match bytes.is_empty() {
            true => Session::default(),
            false => postcard::from_bytes(bytes).map_err(|_| Malformed)?,
        };
        let mut adopted = Vec::new();
        for (place, state) in texts {
            let sequence = Sequence::from_state(state)?;
            match self.value(place.object, place.field) {
                Some(Value::Text(held)) if *held == sequence => adopted.push((place, sequence)),
                _ => return Err(Malformed),
            }
        }
        self.gone = gone;
        self.refresh_texts();
        for (place, sequence) in adopted {
            if let Some(Value::Text(held)) = self.value_mut(place.object, place.field) {
                *held = sequence;
            }
        }
        Ok(())
    }

    fn texts(&self) -> impl Iterator<Item = (Place, &Sequence<u8>)> {
        self.objects.iter().flat_map(|(id, object)| {
            object
                .fields
                .iter()
                .enumerate()
                .filter_map(move |(index, value)| match (value, u16::try_from(index)) {
                    (Value::Text(sequence), Ok(field)) => {
                        Some((Place { object: *id, field }, sequence))
                    }
                    _ => None,
                })
        })
    }

    pub(crate) fn refresh_texts(&mut self) {
        for object in self.objects.values_mut() {
            refresh(&mut object.fields);
        }
    }

    pub fn upgrade(&mut self, id: ObjectId, blank: Vec<Value>) {
        let Some(object) = self.objects.get_mut(&id) else {
            return;
        };
        for (index, value) in blank.into_iter().enumerate() {
            match object.fields.get_mut(index) {
                Some(held) if std::mem::discriminant(&*held) != std::mem::discriminant(&value) => {
                    *held = value;
                }
                Some(_) => {}
                None => object.fields.push(value),
            }
        }
    }

    pub(crate) fn list_ids(&self, place: Place) -> Vec<ObjectId> {
        self.list(place).cloned().unwrap_or_default()
    }

    pub fn contains(&self, id: ObjectId) -> bool {
        self.objects.contains_key(&id)
    }

    pub fn object(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(&id)
    }

    pub fn fields<M: Model>(&self, id: ObjectId) -> Vec<Value> {
        let mut fields = M::blank();
        if let Some(object) = self.objects.get(&id) {
            for (slot, stored) in fields.iter_mut().zip(&object.fields) {
                if std::mem::discriminant(slot) == std::mem::discriminant(stored) {
                    slot.clone_from(stored);
                }
            }
        }
        fields
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).unwrap_or_default()
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, Malformed> {
        let tree: Self = postcard::from_bytes(bytes).map_err(|_| Malformed)?;
        if !tree.objects.contains_key(&ObjectId::ROOT) {
            return Err(Malformed);
        }
        Ok(tree)
    }

    pub(crate) fn apply(&mut self, change: &Change) -> bool {
        match change {
            Change::Set {
                object,
                field,
                value,
            } => match self.value_mut(*object, *field) {
                Some(Value::Register(held)) if held != value => {
                    held.clone_from(value);
                    true
                }
                _ => false,
            },
            Change::SetIf {
                object,
                field,
                expected,
                value,
            } => match self.value_mut(*object, *field) {
                Some(Value::Register(held)) if held == expected && held != value => {
                    held.clone_from(value);
                    true
                }
                _ => false,
            },
            Change::Add { object, field, by } => match self.value_mut(*object, *field) {
                Some(Value::Count(held)) if *by != 0 => {
                    *held = held.saturating_add(*by);
                    true
                }
                _ => false,
            },
            Change::Put {
                object,
                field,
                key,
                value,
            } => match self.value_mut(*object, *field) {
                Some(Value::Map(entries)) => put(entries, key, value.as_ref()),
                _ => false,
            },
            Change::PutIf {
                object,
                field,
                key,
                expected,
                value,
            } => match self.value_mut(*object, *field) {
                Some(Value::Map(entries)) if entries.get(key) == expected.as_ref() => {
                    put(entries, key, value.as_ref())
                }
                _ => false,
            },
            Change::Paint {
                object,
                field,
                cells,
            } => match self.value_mut(*object, *field) {
                Some(Value::Grid(held)) => held.paint(cells),
                _ => false,
            },
            Change::Reshape {
                object,
                field,
                expected,
                bounds,
                cells,
            } => match self.value_mut(*object, *field) {
                Some(Value::Grid(held))
                    if expected.is_none_or(|expected| expected == held.bounds()) =>
                {
                    let reshaped = held.reshape(*bounds);
                    held.paint(cells) || reshaped
                }
                _ => false,
            },
            Change::Insert {
                place,
                anchor,
                objects,
            } => self.insert(*place, *anchor, objects),
            Change::Stamp {
                object,
                field,
                key,
                stamped,
            } => match self.value_mut(*object, *field) {
                Some(Value::Latest(entries)) => crate::latest::stamp(entries, key, stamped),
                _ => false,
            },
            Change::Text { object, field, op } => match self.value_mut(*object, *field) {
                Some(Value::Text(sequence)) => sequence.apply(op).is_some(),
                _ => false,
            },
            Change::Remove { object } => self.remove(*object),
            Change::RemoveIf { object, expected } => {
                self.holds(*object, expected) && self.remove(*object)
            }
            Change::Move {
                object,
                place,
                anchor,
            } => self.relocate(*object, *place, *anchor),
            Change::MoveIf {
                object,
                expected,
                place,
                anchor,
            } => {
                self.objects.get(object).and_then(|held| held.parent) == Some(*expected)
                    && self.relocate(*object, *place, *anchor)
            }
        }
    }

    pub(crate) fn inverse(&self, change: &Change) -> Option<(Change, Change)> {
        match change {
            Change::Set {
                object,
                field,
                value,
            } => {
                let Some(Value::Register(held)) = self.value(*object, *field) else {
                    return None;
                };
                (held != value).then(|| conditional(*object, *field, held, value))
            }
            Change::SetIf {
                object,
                field,
                expected,
                value,
            } => {
                let Some(Value::Register(held)) = self.value(*object, *field) else {
                    return None;
                };
                (held == expected && held != value)
                    .then(|| conditional(*object, *field, held, value))
            }
            Change::Add { object, field, by } => {
                let Some(Value::Count(_)) = self.value(*object, *field) else {
                    return None;
                };
                (*by != 0).then(|| {
                    (
                        Change::Add {
                            object: *object,
                            field: *field,
                            by: by.saturating_neg(),
                        },
                        change.clone(),
                    )
                })
            }
            Change::Put {
                object,
                field,
                key,
                value,
            } => {
                let Some(Value::Map(entries)) = self.value(*object, *field) else {
                    return None;
                };
                let held = entries.get(key);
                (held != value.as_ref())
                    .then(|| conditional_entry(*object, *field, key, held, value.as_ref()))
            }
            Change::PutIf {
                object,
                field,
                key,
                expected,
                value,
            } => {
                let Some(Value::Map(entries)) = self.value(*object, *field) else {
                    return None;
                };
                let held = entries.get(key);
                (held == expected.as_ref() && held != value.as_ref())
                    .then(|| conditional_entry(*object, *field, key, held, value.as_ref()))
            }
            Change::Paint {
                object,
                field,
                cells,
            } => {
                let Some(Value::Grid(held)) = self.value(*object, *field) else {
                    return None;
                };
                let (back, forward) = held.inverse_paint(cells);
                (!forward.is_empty()).then_some({
                    (
                        Change::Paint {
                            object: *object,
                            field: *field,
                            cells: back,
                        },
                        Change::Paint {
                            object: *object,
                            field: *field,
                            cells: forward,
                        },
                    )
                })
            }
            Change::Reshape {
                object,
                field,
                expected,
                bounds,
                cells,
            } => {
                let Some(Value::Grid(held)) = self.value(*object, *field) else {
                    return None;
                };
                let before = held.bounds();
                if expected.is_some_and(|expected| expected != before) {
                    return None;
                }
                let mut reshaped = held.clone();
                reshaped.reshape(*bounds);
                let (overwritten, painted) = reshaped.inverse_paint(cells);
                if before == *bounds && painted.is_empty() {
                    return None;
                }
                let mut restored = held.outside(*bounds);
                restored.extend(overwritten);
                Some((
                    Change::Reshape {
                        object: *object,
                        field: *field,
                        expected: Some(*bounds),
                        bounds: before,
                        cells: restored,
                    },
                    Change::Reshape {
                        object: *object,
                        field: *field,
                        expected: Some(before),
                        bounds: *bounds,
                        cells: painted,
                    },
                ))
            }
            Change::Stamp { .. } => None,
            Change::Text { object, field, op } => {
                let Some(Value::Text(sequence)) = self.value(*object, *field) else {
                    return None;
                };
                let (back, forward) = sequence.inverse(op)?;
                Some((
                    Change::Text {
                        object: *object,
                        field: *field,
                        op: back,
                    },
                    Change::Text {
                        object: *object,
                        field: *field,
                        op: forward,
                    },
                ))
            }
            Change::Insert { place, objects, .. } => {
                let (top, _) = objects.first()?;
                (!self.contains(*top) && self.list(*place).is_some())
                    .then(|| (Change::Remove { object: *top }, change.clone()))
            }
            Change::Remove { object } | Change::RemoveIf { object, .. } => {
                if let Change::RemoveIf { expected, .. } = change
                    && !self.holds(*object, expected)
                {
                    return None;
                }
                let place = self.objects.get(object)?.parent?;
                Some((
                    Change::Insert {
                        place,
                        anchor: self.anchor_of(*object, place),
                        objects: self.subtree(*object),
                    },
                    change.clone(),
                ))
            }
            Change::Move {
                object,
                place,
                anchor,
            } => self.inverse_move(*object, *place, *anchor),
            Change::MoveIf {
                object,
                expected,
                place,
                anchor,
            } => {
                if self.objects.get(object)?.parent? != *expected {
                    return None;
                }
                self.inverse_move(*object, *place, *anchor)
            }
        }
    }

    fn inverse_move(
        &self,
        object: ObjectId,
        place: Place,
        anchor: Anchor,
    ) -> Option<(Change, Change)> {
        let from = self.objects.get(&object)?.parent?;
        if !self.can_move(object, place) || anchor == Anchor::After(object) {
            return None;
        }
        if from == place {
            let items = self.list(place)?;
            let mut moved: Vec<ObjectId> = items
                .iter()
                .copied()
                .filter(|held| *held != object)
                .collect();
            let index = self.index_in(&moved, place, anchor);
            moved.insert(index, object);
            if moved == *items {
                return None;
            }
        }
        Some((
            Change::MoveIf {
                object,
                expected: place,
                place: from,
                anchor: self.anchor_of(object, from),
            },
            Change::MoveIf {
                object,
                expected: from,
                place,
                anchor,
            },
        ))
    }

    pub(crate) fn touched(&self, change: &Change, out: &mut Vec<Touched>) {
        match change {
            Change::Set { object, field, .. }
            | Change::SetIf { object, field, .. }
            | Change::Add { object, field, .. }
            | Change::Put { object, field, .. }
            | Change::PutIf { object, field, .. }
            | Change::Paint { object, field, .. }
            | Change::Reshape { object, field, .. }
            | Change::Stamp { object, field, .. }
            | Change::Text { object, field, .. } => {
                out.push(Touched::Field(*object, *field));
                self.touch_up(*object, out);
            }
            Change::Insert { place, objects, .. } => {
                self.touch_place(*place, out);
                out.extend(objects.iter().map(|(id, _)| Touched::Subtree(*id)));
            }
            Change::Remove { object } | Change::RemoveIf { object, .. } => {
                if let Some(place) = self.objects.get(object).and_then(|held| held.parent) {
                    self.touch_place(place, out);
                }
                out.extend(
                    self.subtree(*object)
                        .into_iter()
                        .map(|(id, _)| Touched::Subtree(id)),
                );
            }
            Change::Move { object, place, .. } | Change::MoveIf { object, place, .. } => {
                if let Some(from) = self.objects.get(object).and_then(|held| held.parent) {
                    self.touch_place(from, out);
                }
                self.touch_place(*place, out);
            }
        }
    }

    fn touch_place(&self, place: Place, out: &mut Vec<Touched>) {
        out.push(Touched::Field(place.object, place.field));
        self.touch_up(place.object, out);
    }

    fn touch_up(&self, object: ObjectId, out: &mut Vec<Touched>) {
        let mut cursor = Some(object);
        while let Some(id) = cursor {
            out.push(Touched::Subtree(id));
            cursor = self
                .objects
                .get(&id)
                .and_then(|held| held.parent)
                .map(|parent| parent.object);
        }
    }

    pub(crate) fn subtree(&self, top: ObjectId) -> Vec<(ObjectId, Object)> {
        let mut out = Vec::new();
        let mut pending = vec![top];
        while let Some(id) = pending.pop() {
            let Some(object) = self.objects.get(&id) else {
                continue;
            };
            for value in object.fields.iter().rev() {
                if let Value::List(children) = value {
                    pending.extend(children.iter().rev());
                }
            }
            out.push((id, object.clone()));
        }
        out
    }

    fn holds(&self, object: ObjectId, expected: &[Value]) -> bool {
        self.objects
            .get(&object)
            .is_some_and(|held| held.fields == expected)
    }

    fn value(&self, object: ObjectId, field: u16) -> Option<&Value> {
        self.objects.get(&object)?.fields.get(usize::from(field))
    }

    fn value_mut(&mut self, object: ObjectId, field: u16) -> Option<&mut Value> {
        self.objects
            .get_mut(&object)?
            .fields
            .get_mut(usize::from(field))
    }

    fn list(&self, place: Place) -> Option<&Vec<ObjectId>> {
        match self.value(place.object, place.field)? {
            Value::List(items) => Some(items),
            _ => None,
        }
    }

    fn list_mut(&mut self, place: Place) -> Option<&mut Vec<ObjectId>> {
        match self.value_mut(place.object, place.field)? {
            Value::List(items) => Some(items),
            _ => None,
        }
    }

    fn anchor_of(&self, object: ObjectId, place: Place) -> Anchor {
        let Some(items) = self.list(place) else {
            return Anchor::End;
        };
        match items.iter().position(|held| *held == object) {
            Some(0) | None => Anchor::Start,
            Some(index) => Anchor::After(items[index - 1]),
        }
    }

    fn index_in(&self, items: &[ObjectId], place: Place, anchor: Anchor) -> usize {
        let mut anchor = anchor;
        let mut seen = BTreeSet::new();
        loop {
            match anchor {
                Anchor::Start => return 0,
                Anchor::End => return items.len(),
                Anchor::After(sibling) => {
                    if let Some(index) = items.iter().position(|held| *held == sibling) {
                        return index + 1;
                    }
                    match self.gone.get(&sibling) {
                        Some((left, before)) if *left == place && seen.insert(sibling) => {
                            anchor = *before;
                        }
                        _ => return items.len(),
                    }
                }
            }
        }
    }

    fn place_in(&mut self, id: ObjectId, place: Place, anchor: Anchor) -> bool {
        let Some(items) = self.list(place) else {
            return false;
        };
        let index = self.index_in(items, place, anchor);
        if let Some(items) = self.list_mut(place) {
            items.insert(index, id);
        }
        true
    }

    fn detach(&mut self, id: ObjectId) {
        let Some(place) = self.objects.get(&id).and_then(|object| object.parent) else {
            return;
        };
        let anchor = self.anchor_of(id, place);
        self.gone.insert(id, (place, anchor));
        if let Some(items) = self.list_mut(place) {
            items.retain(|held| *held != id);
        }
    }

    fn insert(&mut self, place: Place, anchor: Anchor, objects: &[(ObjectId, Object)]) -> bool {
        let Some((top, _)) = objects.first() else {
            return false;
        };
        if self.list(place).is_none() || objects.iter().any(|(id, _)| self.contains(*id)) {
            return false;
        }
        for (index, (id, object)) in objects.iter().enumerate() {
            let mut object = object.clone();
            if index == 0 {
                object.parent = Some(place);
            }
            refresh(&mut object.fields);
            self.objects.insert(*id, object);
        }
        self.place_in(*top, place, anchor)
    }

    fn remove(&mut self, object: ObjectId) -> bool {
        if object == ObjectId::ROOT || !self.contains(object) {
            return false;
        }
        self.detach(object);
        for (id, _) in self.subtree(object) {
            self.objects.remove(&id);
        }
        true
    }

    fn can_move(&self, object: ObjectId, place: Place) -> bool {
        if object == ObjectId::ROOT || !self.contains(object) || self.list(place).is_none() {
            return false;
        }
        let mut cursor = Some(place.object);
        while let Some(id) = cursor {
            if id == object {
                return false;
            }
            cursor = self
                .objects
                .get(&id)
                .and_then(|held| held.parent)
                .map(|parent| parent.object);
        }
        true
    }

    fn relocate(&mut self, object: ObjectId, place: Place, anchor: Anchor) -> bool {
        if !self.can_move(object, place) || anchor == Anchor::After(object) {
            return false;
        }
        self.detach(object);
        if let Some(held) = self.objects.get_mut(&object) {
            held.parent = Some(place);
        }
        self.place_in(object, place, anchor)
    }

    pub(crate) fn children_by_place(&self) -> BTreeMap<Place, Vec<ObjectId>> {
        let mut children: BTreeMap<Place, Vec<ObjectId>> = BTreeMap::new();
        for (id, object) in &self.objects {
            if let Some(parent) = object.parent {
                children.entry(parent).or_default().push(*id);
            }
        }
        children
    }

    pub(crate) fn set_list(&mut self, place: Place, items: Vec<ObjectId>) {
        if let Some(held) = self.list_mut(place) {
            *held = items;
        }
    }
}

fn conditional(object: ObjectId, field: u16, before: &[u8], after: &[u8]) -> (Change, Change) {
    (
        Change::SetIf {
            object,
            field,
            expected: after.to_vec(),
            value: before.to_vec(),
        },
        Change::SetIf {
            object,
            field,
            expected: before.to_vec(),
            value: after.to_vec(),
        },
    )
}

fn put(entries: &mut BTreeMap<Vec<u8>, Vec<u8>>, key: &[u8], value: Option<&Vec<u8>>) -> bool {
    if entries.get(key) == value {
        return false;
    }
    match value {
        Some(value) => {
            entries.insert(key.to_vec(), value.clone());
        }
        None => {
            entries.remove(key);
        }
    }
    true
}

fn conditional_entry(
    object: ObjectId,
    field: u16,
    key: &[u8],
    before: Option<&Vec<u8>>,
    after: Option<&Vec<u8>>,
) -> (Change, Change) {
    (
        Change::PutIf {
            object,
            field,
            key: key.to_vec(),
            expected: after.cloned(),
            value: before.cloned(),
        },
        Change::PutIf {
            object,
            field,
            key: key.to_vec(),
            expected: before.cloned(),
            value: after.cloned(),
        },
    )
}

fn refresh(fields: &mut [Value]) {
    for value in fields {
        if let Value::Text(sequence) = value
            && !sequence.is_fresh()
        {
            *sequence = sequence.refreshed();
        }
    }
}

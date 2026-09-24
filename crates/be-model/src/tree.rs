use std::collections::BTreeMap;

use crate::{Anchor, Change, Malformed, Model, Object, ObjectId, Objects, Place, Touched, Value};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Tree {
    objects: Objects,
}

impl Tree {
    pub(crate) fn from_objects(objects: impl IntoIterator<Item = (ObjectId, Object)>) -> Self {
        Self {
            objects: objects.into_iter().collect(),
        }
    }

    pub(crate) fn from_map(objects: Objects) -> Self {
        Self { objects }
    }

    pub(crate) fn objects(&self) -> &Objects {
        &self.objects
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
        postcard::to_stdvec(&self.objects).unwrap_or_default()
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, Malformed> {
        let objects: Objects = postcard::from_bytes(bytes).map_err(|_| Malformed)?;
        if !objects.contains_key(&ObjectId::ROOT) {
            return Err(Malformed);
        }
        Ok(Self { objects })
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
            Change::Remove { object } => self.remove(*object),
            Change::Move {
                object,
                place,
                anchor,
            } => self.relocate(*object, *place, *anchor),
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
                (!forward.is_empty()).then(|| {
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
            Change::Insert { place, objects, .. } => {
                let (top, _) = objects.first()?;
                (!self.contains(*top) && self.list(*place).is_some())
                    .then(|| (Change::Remove { object: *top }, change.clone()))
            }
            Change::Remove { object } => {
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
            } => {
                let from = self.objects.get(object)?.parent?;
                self.can_move(*object, *place).then(|| {
                    (
                        Change::Move {
                            object: *object,
                            place: from,
                            anchor: self.anchor_of(*object, from),
                        },
                        Change::Move {
                            object: *object,
                            place: *place,
                            anchor: *anchor,
                        },
                    )
                })
            }
        }
    }

    pub(crate) fn touched(&self, change: &Change, out: &mut Vec<Touched>) {
        match change {
            Change::Set { object, field, .. }
            | Change::SetIf { object, field, .. }
            | Change::Add { object, field, .. }
            | Change::Put { object, field, .. }
            | Change::PutIf { object, field, .. }
            | Change::Paint { object, field, .. }
            | Change::Reshape { object, field, .. } => {
                out.push(Touched::Field(*object, *field));
                self.touch_up(*object, out);
            }
            Change::Insert { place, objects, .. } => {
                self.touch_place(*place, out);
                out.extend(objects.iter().map(|(id, _)| Touched::Subtree(*id)));
            }
            Change::Remove { object } => {
                if let Some(place) = self.objects.get(object).and_then(|held| held.parent) {
                    self.touch_place(place, out);
                }
                out.extend(
                    self.subtree(*object)
                        .into_iter()
                        .map(|(id, _)| Touched::Subtree(id)),
                );
            }
            Change::Move { object, place, .. } => {
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

    fn place_in(&mut self, id: ObjectId, place: Place, anchor: Anchor) -> bool {
        let Some(items) = self.list_mut(place) else {
            return false;
        };
        let index = match anchor {
            Anchor::Start => 0,
            Anchor::End => items.len(),
            Anchor::After(sibling) => items
                .iter()
                .position(|held| *held == sibling)
                .map_or(items.len(), |index| index + 1),
        };
        items.insert(index, id);
        true
    }

    fn detach(&mut self, id: ObjectId) {
        let Some(place) = self.objects.get(&id).and_then(|object| object.parent) else {
            return;
        };
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

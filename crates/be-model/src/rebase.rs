use std::collections::BTreeSet;

use crate::{Change, Edit, ObjectId, Place, Tree};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Spot {
    Object(ObjectId),
    Field(ObjectId, u16),
}

impl Spot {
    pub(crate) fn object(self) -> ObjectId {
        match self {
            Self::Object(id) | Self::Field(id, _) => id,
        }
    }
}

pub(crate) fn exact(tree: &Tree, change: &Change, spots: &mut BTreeSet<Spot>) {
    spotted(&[tree], change, spots);
}

pub(crate) fn around(trees: &[&Tree], edit: &Edit, spots: &mut BTreeSet<Spot>) {
    for change in &edit.0 {
        spotted(trees, change, spots);
    }
}

fn spotted(trees: &[&Tree], change: &Change, spots: &mut BTreeSet<Spot>) {
    let mut place = |place: Place| spots.insert(Spot::Field(place.object, place.field));
    match change {
        Change::Set { object, field, .. }
        | Change::SetIf { object, field, .. }
        | Change::Add { object, field, .. }
        | Change::Put { object, field, .. }
        | Change::PutIf { object, field, .. }
        | Change::Paint { object, field, .. }
        | Change::Reshape { object, field, .. }
        | Change::Stamp { object, field, .. }
        | Change::Text { object, field, .. }
        | Change::Mark { object, field, .. }
        | Change::Unmark { object, field, .. }
        | Change::Series { object, field, .. } => {
            spots.insert(Spot::Field(*object, *field));
        }
        Change::Insert {
            place: into,
            objects,
            ..
        } => {
            place(*into);
            spots.extend(objects.iter().map(|(id, _)| Spot::Object(*id)));
        }
        Change::Remove { object } | Change::RemoveIf { object, .. } => {
            for tree in trees {
                if let Some(parent) = tree.object(*object).and_then(|held| held.parent) {
                    spots.insert(Spot::Field(parent.object, parent.field));
                }
                spots.extend(tree.subtree_ids(*object).into_iter().map(Spot::Object));
            }
            spots.insert(Spot::Object(*object));
        }
        Change::Move {
            object, place: to, ..
        } => {
            place(*to);
            for tree in trees {
                if let Some(parent) = tree.object(*object).and_then(|held| held.parent) {
                    spots.insert(Spot::Field(parent.object, parent.field));
                }
            }
            spots.insert(Spot::Object(*object));
        }
    }
}

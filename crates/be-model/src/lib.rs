extern crate self as be_model;

use std::{collections::BTreeMap, fmt, marker::PhantomData};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod field;
mod history;
mod merge;
mod tree;

pub use be_model_derive::Model;
pub use field::{Count, Field, FieldRef, Item, List, Register};
pub use history::Step;
pub use tree::Tree;

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct ObjectId(Uuid);

impl ObjectId {
    pub const ROOT: Self = Self(Uuid::nil());

    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub const fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Place {
    pub object: ObjectId,
    pub field: u16,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Anchor {
    Start,
    After(ObjectId),
    End,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Touched {
    Everything,
    Field(ObjectId, u16),
    Subtree(ObjectId),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Value {
    Register(Vec<u8>),
    Count(i64),
    List(Vec<ObjectId>),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Object {
    parent: Option<Place>,
    fields: Vec<Value>,
}

impl Object {
    pub fn new(parent: Option<Place>, fields: Vec<Value>) -> Self {
        Self { parent, fields }
    }

    pub fn parent(&self) -> Option<Place> {
        self.parent
    }

    pub fn fields(&self) -> &[Value] {
        &self.fields
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Change {
    Set {
        object: ObjectId,
        field: u16,
        value: Vec<u8>,
    },
    SetIf {
        object: ObjectId,
        field: u16,
        expected: Vec<u8>,
        value: Vec<u8>,
    },
    Add {
        object: ObjectId,
        field: u16,
        by: i64,
    },
    Insert {
        place: Place,
        anchor: Anchor,
        objects: Vec<(ObjectId, Object)>,
    },
    Remove {
        object: ObjectId,
    },
    Move {
        object: ObjectId,
        place: Place,
        anchor: Anchor,
    },
}

impl Change {
    pub fn remove(object: ObjectId) -> Self {
        Self::Remove { object }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Edit(pub Vec<Change>);

impl From<Change> for Edit {
    fn from(change: Change) -> Self {
        Self(vec![change])
    }
}

impl FromIterator<Change> for Edit {
    fn from_iter<I: IntoIterator<Item = Change>>(changes: I) -> Self {
        Self(changes.into_iter().collect())
    }
}

pub trait Model: Sized {
    fn blank() -> Vec<Value>;

    fn read(tree: &Tree, id: ObjectId) -> Self;

    fn write(&self, id: ObjectId, parent: Option<Place>, out: &mut Vec<(ObjectId, Object)>);
}

#[derive(Debug)]
pub struct Malformed;

impl fmt::Display for Malformed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a model document is malformed")
    }
}

impl std::error::Error for Malformed {}

pub struct Document<R> {
    tree: Tree,
    root: PhantomData<fn() -> R>,
}

impl<R: Model> Document<R> {
    pub fn new(root: &R) -> Self {
        let mut objects = Vec::new();
        root.write(ObjectId::ROOT, None, &mut objects);
        Self::from_tree(Tree::from_objects(objects))
    }

    fn from_tree(tree: Tree) -> Self {
        Self {
            tree,
            root: PhantomData,
        }
    }

    pub fn root(&self) -> R {
        R::read(&self.tree, ObjectId::ROOT)
    }

    pub fn read<T: Model>(&self, id: ObjectId) -> Option<T> {
        self.tree.contains(id).then(|| T::read(&self.tree, id))
    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub fn apply(&mut self, edit: &Edit) {
        for change in &edit.0 {
            self.tree.apply(change);
        }
    }

    pub fn apply_touching(&mut self, edit: &Edit, touched: &mut Vec<Touched>) {
        for change in &edit.0 {
            self.tree.touched(change, touched);
            self.tree.apply(change);
        }
    }

    pub fn field<M, F: Field>(&self, object: ObjectId, field: FieldRef<M, F>) -> F {
        let value = self
            .tree
            .object(object)
            .and_then(|held| held.fields.get(usize::from(field.index())))
            .filter(|value| std::mem::discriminant(*value) == std::mem::discriminant(&F::blank()))
            .cloned()
            .unwrap_or_else(F::blank);
        F::read(&self.tree, &value)
    }

    pub fn ids<M, T>(&self, owner: ObjectId, field: FieldRef<M, List<T>>) -> Vec<ObjectId> {
        match self
            .tree
            .object(owner)
            .and_then(|held| held.fields.get(usize::from(field.index())))
        {
            Some(Value::List(ids)) => ids.clone(),
            _ => Vec::new(),
        }
    }

    pub fn step(&self, edit: &Edit) -> Option<Step> {
        history::step(&self.tree, edit)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.tree.encode()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Malformed> {
        Tree::decode(bytes).map(Self::from_tree)
    }

    pub fn merge(base: &Self, ours: &Self, theirs: &Self) -> (Self, usize) {
        let (tree, conflicts) = merge::merge3(&base.tree, &ours.tree, &theirs.tree);
        (Self::from_tree(tree), conflicts)
    }
}

impl<R: Model> Default for Document<R> {
    fn default() -> Self {
        Self::from_tree(Tree::from_objects(vec![(
            ObjectId::ROOT,
            Object::new(None, R::blank()),
        )]))
    }
}

impl<R> Clone for Document<R> {
    fn clone(&self) -> Self {
        Self {
            tree: self.tree.clone(),
            root: PhantomData,
        }
    }
}

impl<R> PartialEq for Document<R> {
    fn eq(&self, other: &Self) -> bool {
        self.tree == other.tree
    }
}

impl<R> Eq for Document<R> {}

impl<R> fmt::Debug for Document<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.tree.fmt(formatter)
    }
}

pub(crate) type Objects = BTreeMap<ObjectId, Object>;

#[cfg(test)]
mod tests;

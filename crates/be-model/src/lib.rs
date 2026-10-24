extern crate self as be_model;

use std::{
    cell,
    collections::{BTreeMap, BTreeSet},
    fmt,
    marker::PhantomData,
    sync::OnceLock,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod byte_string;
mod field;
mod grid;
mod history;
mod items;
mod latest;
mod merge;
mod rebase;
pub mod references;
pub mod schema;
mod series;
mod stored;
mod text;
mod tree;

pub use be_model_derive::Model;
pub use byte_string::{Bytes, bytes};
pub use field::{Count, Field, FieldRef, Item, List, Map, Register};
pub use grid::{Bounds, Cell, Cells, Grid, Paint};
pub use history::Step;
pub use items::Items;
pub use latest::{Latest, LatestMap, Stamp, Stamped};
pub use references::BlockRef;
pub use schema::{Kind, Property, Shape};
pub use sequence::marks;
pub use sequence::{LOADED, Pos, SeqOp, Sequence, Span, Splice};
pub use series::Series;
pub use stored::{BLOB_REF, BLOCK_REF, CRITICAL, Stored, decode as from_cbor, encode as to_cbor};
pub use text::{RichText, Text};
pub use tree::Tree;

thread_local! {
    static CLIENT: cell::Cell<u64> = cell::Cell::new(Uuid::new_v4().as_u64_pair().0 | 1 << 63);
}

pub fn local_client() -> u64 {
    CLIENT.with(cell::Cell::get)
}

#[cfg(any(test, feature = "fuzzing"))]
pub fn set_local_client(client: u64) {
    CLIENT.with(|held| held.set(client));
}

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
    Behind(Pos),
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
    List(Items),
    Map(BTreeMap<Vec<u8>, Vec<u8>>),
    Grid(Cells),
    Latest(BTreeMap<Vec<u8>, Stamped>),
    Text(Sequence<u8>),
    Rich(marks::Rich),
    Series(Sequence<Vec<u8>>),
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Kept {
    kind: Option<String>,
    properties: BTreeMap<String, Vec<u8>>,
}

impl Kept {
    pub fn kind(&self) -> Option<&str> {
        self.kind.as_deref()
    }

    pub fn properties(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.properties
    }

    pub fn is_empty(&self) -> bool {
        self.kind.is_none() && self.properties.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Object {
    parent: Option<Place>,
    fields: Vec<Value>,
    kept: Kept,
}

impl Object {
    pub fn new(parent: Option<Place>, fields: Vec<Value>) -> Self {
        Self {
            parent,
            fields,
            kept: Kept::default(),
        }
    }

    pub fn parent(&self) -> Option<Place> {
        self.parent
    }

    pub fn fields(&self) -> &[Value] {
        &self.fields
    }

    pub fn kept(&self) -> &Kept {
        &self.kept
    }

    pub fn is_locked(&self) -> bool {
        self.kept.kind.is_some()
            || self
                .kept
                .properties
                .values()
                .any(|bytes| stored::is_critical(bytes))
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
        client: u64,
        objects: Vec<(ObjectId, Object)>,
    },
    Put {
        object: ObjectId,
        field: u16,
        key: Vec<u8>,
        value: Option<Vec<u8>>,
    },
    PutIf {
        object: ObjectId,
        field: u16,
        key: Vec<u8>,
        expected: Option<Vec<u8>>,
        value: Option<Vec<u8>>,
    },
    Paint {
        object: ObjectId,
        field: u16,
        cells: Vec<Paint>,
    },
    Reshape {
        object: ObjectId,
        field: u16,
        expected: Option<Bounds>,
        bounds: Bounds,
        cells: Vec<Paint>,
    },
    Remove {
        object: ObjectId,
    },
    RemoveIf {
        object: ObjectId,
        expected: Vec<Value>,
    },
    Move {
        object: ObjectId,
        place: Place,
        anchor: Anchor,
        client: u64,
    },
    Stamp {
        object: ObjectId,
        field: u16,
        key: Vec<u8>,
        stamped: Stamped,
    },
    Text {
        object: ObjectId,
        field: u16,
        op: SeqOp<u8>,
    },
    Mark {
        object: ObjectId,
        field: u16,
        mark: marks::Mark,
    },
    Unmark {
        object: ObjectId,
        field: u16,
        id: marks::MarkId,
    },
    Series {
        object: ObjectId,
        field: u16,
        op: SeqOp<Vec<u8>>,
    },
}

impl Change {
    pub fn remove(object: ObjectId) -> Self {
        Self::Remove { object }
    }

    pub fn remove_if_blank<M: Model>(object: ObjectId) -> Self {
        Self::RemoveIf {
            object,
            expected: M::blank(),
        }
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

    fn reread(self, tree: &Tree, id: ObjectId) -> Self;

    fn write(&self, id: ObjectId, parent: Option<Place>, out: &mut Vec<(ObjectId, Object)>);

    fn kind() -> Kind;
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
    references: OnceLock<references::Index>,
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
            references: OnceLock::new(),
            root: PhantomData,
        }
    }

    pub fn root(&self) -> R {
        R::read(&self.tree, ObjectId::ROOT)
    }

    pub fn read<T: Model>(&self, id: ObjectId) -> Option<T> {
        self.tree.contains(id).then(|| T::read(&self.tree, id))
    }

    pub fn reread<T: Model>(&self, id: ObjectId, held: T) -> Option<T> {
        self.tree.contains(id).then(|| held.reread(&self.tree, id))
    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub fn apply(&mut self, edit: &Edit) {
        for change in &edit.0 {
            self.change(change);
        }
    }

    pub fn apply_touching(&mut self, edit: &Edit, touched: &mut Vec<Touched>) {
        for change in &edit.0 {
            self.tree.touched(change, touched);
            self.change(change);
        }
    }

    pub fn rebase(
        &mut self,
        confirmed: &mut Self,
        incoming: &Edit,
        local: &[Edit],
        touched: &mut Vec<Touched>,
    ) {
        let mut spots = BTreeSet::new();
        for edit in local {
            rebase::around(&[&confirmed.tree, &self.tree], edit, &mut spots);
        }
        for change in &incoming.0 {
            rebase::exact(&confirmed.tree, change, &mut spots);
            confirmed.change(change);
        }
        for spot in &spots {
            self.tree.touch_spot(*spot, touched);
        }
        for spot in &spots {
            self.tree.restore(*spot, &confirmed.tree);
        }
        for spot in &spots {
            self.tree.touch_spot(*spot, touched);
        }
        if let Some(index) = self.references.get_mut() {
            let objects: BTreeSet<ObjectId> = spots.iter().map(|spot| spot.object()).collect();
            for object in objects {
                index.refresh(&self.tree, object);
            }
        }
        for edit in local {
            self.apply_touching(edit, touched);
        }
    }

    fn change(&mut self, change: &Change) {
        let Some(index) = self.references.get_mut() else {
            self.tree.apply(R::kind, change);
            return;
        };
        let removed = match change {
            Change::Remove { object } | Change::RemoveIf { object, .. } => {
                self.tree.subtree_ids(*object)
            }
            _ => Vec::new(),
        };
        if !self.tree.apply(R::kind, change) {
            return;
        }
        match change {
            Change::Set { object, field, .. }
            | Change::SetIf { object, field, .. }
            | Change::Put { object, field, .. }
            | Change::PutIf { object, field, .. }
            | Change::Stamp { object, field, .. } => {
                index.refresh_field(&self.tree, *object, *field)
            }
            Change::Series {
                object,
                field,
                op: SeqOp::Insert { items, .. },
            } => index.add(&self.tree, *object, *field, items),
            Change::Series { object, field, .. } => {
                index.refresh_field(&self.tree, *object, *field)
            }
            Change::Insert { objects, .. } => {
                for (id, _) in objects {
                    index.refresh(&self.tree, *id);
                }
            }
            Change::Remove { .. } | Change::RemoveIf { .. } => {
                for id in removed {
                    index.refresh(&self.tree, id);
                }
            }
            Change::Add { .. }
            | Change::Paint { .. }
            | Change::Reshape { .. }
            | Change::Move { .. }
            | Change::Text { .. }
            | Change::Mark { .. }
            | Change::Unmark { .. } => {}
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
            Some(Value::List(ids)) => ids.ids(),
            _ => Vec::new(),
        }
    }

    pub fn index_of<M, T>(
        &self,
        owner: ObjectId,
        field: FieldRef<M, List<T>>,
        id: ObjectId,
    ) -> Option<usize> {
        match self
            .tree
            .object(owner)?
            .fields
            .get(usize::from(field.index()))?
        {
            Value::List(ids) => ids.index_of(id),
            _ => None,
        }
    }

    pub fn step(&self, edit: &Edit) -> Option<Step> {
        history::step(&self.tree, R::kind, edit)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        stored::save(&self.tree, &R::kind())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Malformed> {
        stored::load(bytes, &R::kind()).map(Self::from_tree)
    }

    pub fn to_stored(&self) -> Stored {
        stored::document(&self.tree, &R::kind())
    }

    pub fn is_locked(&self, id: ObjectId) -> bool {
        self.tree.object(id).is_some_and(Object::is_locked)
    }

    pub fn block_refs(&self) -> Vec<Uuid> {
        self.references
            .get_or_init(|| references::Index::build(&self.tree))
            .ids()
    }

    pub fn text<M>(&self, object: ObjectId, field: FieldRef<M, Text>) -> Option<&Sequence<u8>> {
        match self
            .tree
            .object(object)?
            .fields
            .get(usize::from(field.index()))?
        {
            Value::Text(sequence) => Some(sequence),
            _ => None,
        }
    }

    pub fn series<M, T>(
        &self,
        object: ObjectId,
        field: FieldRef<M, Series<T>>,
    ) -> Option<&Sequence<Vec<u8>>> {
        match self
            .tree
            .object(object)?
            .fields
            .get(usize::from(field.index()))?
        {
            Value::Series(sequence) => Some(sequence),
            _ => None,
        }
    }

    pub fn rich<M>(&self, object: ObjectId, field: FieldRef<M, RichText>) -> Option<&marks::Rich> {
        match self
            .tree
            .object(object)?
            .fields
            .get(usize::from(field.index()))?
        {
            Value::Rich(rich) => Some(rich),
            _ => None,
        }
    }

    pub fn session_state(&self) -> Vec<u8> {
        self.tree.session_state()
    }

    pub fn adopt_session_state(&mut self, bytes: &[u8]) -> Result<(), Malformed> {
        self.tree.adopt_session_state(bytes)
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
            references: self.references.clone(),
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

pub type Objects = BTreeMap<ObjectId, Object>;

#[cfg(any(test, feature = "fuzzing"))]
pub mod fuzz;

#[cfg(test)]
mod tests;

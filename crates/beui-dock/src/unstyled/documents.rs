use std::cell::Cell;
use std::collections::HashMap;

use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::*;

use super::{DockEntry, DockKey, DockingLayout};
use beui_view::types::ItemSize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DockSlot(u64);

impl DockSlot {
    pub fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockOpened<T> {
    Shown(DockSlot),
    Replaced(DockSlot, T),
    Added(DockSlot),
}

impl<T> DockOpened<T> {
    pub fn slot(&self) -> DockSlot {
        match self {
            DockOpened::Shown(slot) | DockOpened::Replaced(slot, _) | DockOpened::Added(slot) => {
                *slot
            }
        }
    }
}

struct DocumentsInner<K, T, I> {
    key: Box<dyn Fn(DockSlot) -> K>,
    identity: Box<dyn Fn(&T) -> I>,
    next: Cell<u64>,
}

pub struct DockDocuments<K: 'static, T: 'static, I: 'static> {
    layout: DockingLayout<K>,
    contents: KeyedStore<DockSlot, T>,
    inner: Stored<DocumentsInner<K, T, I>>,
}

impl<K, T, I> Clone for DockDocuments<K, T, I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, T, I> Copy for DockDocuments<K, T, I> {}

impl<K, T, I> DockDocuments<K, T, I>
where
    K: DockKey,
    T: Clone + PartialEq + 'static,
    I: PartialEq + 'static,
{
    pub fn new(
        layout: DockingLayout<K>,
        key: impl Fn(DockSlot) -> K + 'static,
        identity: impl Fn(&T) -> I + 'static,
    ) -> Self {
        Self {
            layout,
            contents: KeyedStore::new(),
            inner: store(DocumentsInner {
                key: Box::new(key),
                identity: Box::new(identity),
                next: Cell::new(1),
            }),
        }
    }

    pub fn slots(&self) -> ReadSignal<Vec<DockSlot>> {
        self.contents.keys()
    }

    pub fn key(&self, slot: DockSlot) -> K {
        self.inner.with(|inner| (inner.key)(slot))
    }

    pub fn identity(&self, content: &T) -> I {
        self.inner.with(|inner| (inner.identity)(content))
    }

    pub fn content(&self, slot: DockSlot) -> Option<ReadSignal<T>> {
        self.contents.try_get(&slot)
    }

    pub fn find(&self, identity: &I) -> Option<DockSlot> {
        self.slots().get().into_iter().find(|slot| {
            self.content(*slot)
                .is_some_and(|content| content.with(|content| self.identity(content) == *identity))
        })
    }

    pub fn shown(&self) -> Option<DockSlot> {
        let slots: HashMap<K, DockSlot> = self
            .slots()
            .get()
            .into_iter()
            .map(|slot| (self.key(slot), slot))
            .collect();
        self.layout.state().with(|state| {
            state
                .recent_tabs()
                .into_iter()
                .find_map(|tab| slots.get(&self.layout.key_of(tab)?).copied())
        })
    }

    pub fn open(&self, content: T, beside: bool) -> DockOpened<T> {
        let identity = self.identity(&content);
        let opened = match untrack(|| self.find(&identity)) {
            Some(slot) => {
                self.contents.set(slot, content);
                DockOpened::Shown(slot)
            }
            None => match untrack(|| self.shown()).filter(|_| !beside) {
                Some(slot) => {
                    let previous = self.contents.get(&slot).get_untracked();
                    self.contents.set(slot, content);
                    DockOpened::Replaced(slot, previous)
                }
                None => {
                    let slot = self.mint();
                    self.contents.set(slot, content);
                    return DockOpened::Added(slot);
                }
            },
        };
        self.layout.show(&self.key(opened.slot()));
        opened
    }

    pub fn refresh(&self, content: T) {
        let identity = self.identity(&content);
        if let Some(slot) = untrack(|| self.find(&identity)) {
            self.contents.set(slot, content);
        }
    }

    pub fn close(&self, slot: DockSlot) {
        self.contents.remove(&slot);
    }

    pub fn clear(&self) {
        self.contents.reconcile_owned([]);
    }

    fn mint(&self) -> DockSlot {
        self.inner.with(|inner| {
            let slot = inner.next.get();
            inner.next.set(slot + 1);
            DockSlot(slot)
        })
    }
}

pub struct DockDocument<K: 'static, T: 'static, I: 'static> {
    documents: DockDocuments<K, T, I>,
    slot: DockSlot,
    content: ReadSignal<T>,
}

impl<K, T, I> Clone for DockDocument<K, T, I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, T, I> Copy for DockDocument<K, T, I> {}

impl<K, T, I> DockDocument<K, T, I>
where
    K: DockKey,
    T: Clone + PartialEq + 'static,
    I: PartialEq + 'static,
{
    pub fn key(&self) -> K {
        self.documents.key(self.slot)
    }

    pub fn slot(&self) -> DockSlot {
        self.slot
    }

    pub fn content(&self) -> ReadSignal<T> {
        self.content
    }

    pub fn close(&self) {
        self.documents.close(self.slot);
    }
}

#[component]
pub fn DockDocumentTabs<K, T, I>(
    documents: DockDocuments<K, T, I>,
    #[prop(children)] tab: RenderFn<DockDocument<K, T, I>, DockEntry<K>>,
) -> DynamicSegment<DockEntry<K>>
where
    K: DockKey,
    T: Clone + PartialEq + 'static,
    I: PartialEq + 'static,
{
    view! {
        <ForEach keys={documents.slots()}>|slot: DockSlot| {
            let content = documents.contents.get(&slot);
            tab.call(DockDocument {
                documents,
                slot,
                content,
            })
        }</ForEach>
    }
}

#[component]
pub fn DockDocumentBody<K, T, I>(
    document: DockDocument<K, T, I>,
    #[prop(children)] body: RenderFn<ReadSignal<T>>,
) -> NodeId
where
    K: DockKey,
    T: Clone + PartialEq + 'static,
    I: PartialEq + 'static,
{
    let documents = document.documents;
    let identity = Func::new(move |content: T| documents.identity(&content));
    view! {
        <List spacing=0.0>
            <Keyed value={document.content} key={identity}>|content: ReadSignal<T>| {
                sizing(body.call(content), ItemSize::Percent(100.0))
            }</Keyed>
        </List>
    }
}

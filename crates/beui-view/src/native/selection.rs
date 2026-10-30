use stable_hash::HashMap;
use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use beui_core::base::list::Direction;
use beui_core::base::offset::OffsetNode;
use beui_core::base::text::TextNode;
use beui_core::geometry::{Pos2, Rect};
use beui_core::node::NodeId;

use crate::core::nodes::tree_local;
use beui_core::current::with_document;

trait AnyKey {
    fn as_any(&self) -> &dyn Any;
    fn same(&self, other: &dyn Any) -> bool;
}

impl<K: PartialEq + 'static> AnyKey for K {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn same(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<K>().is_some_and(|other| other == self)
    }
}

#[derive(Clone)]
pub(crate) struct RowKey(Rc<dyn AnyKey>);

impl RowKey {
    pub fn new<K: PartialEq + 'static>(key: K) -> Self {
        Self(Rc::new(key))
    }

    pub fn get<K: 'static>(&self) -> Option<&K> {
        self.0.as_any().downcast_ref::<K>()
    }
}

impl PartialEq for RowKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.same(other.0.as_any())
    }
}

impl fmt::Debug for RowKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RowKey")
    }
}

pub(crate) trait SelectableRows {
    fn count(&self) -> usize;
    fn key_at(&self, index: usize) -> Option<RowKey>;
    fn index_of(&self, key: &RowKey) -> Option<usize>;
    fn shown(&self) -> Vec<(RowKey, NodeId)>;
    fn text(&self, key: &RowKey) -> Option<String>;
    fn previous_index(&self, key: &RowKey) -> Option<usize>;
    fn replacement(&self, key: &RowKey, forward: bool) -> Option<(RowKey, bool)>;
    fn watch(&self, watcher: &Rc<dyn RowWatcher>);
}

pub(crate) trait RowWatcher {
    fn row_shown(&self, list: NodeId, row: NodeId, key: RowKey);
    fn keys_changed(&self, list: NodeId);
}

type Lists = Rc<RefCell<HashMap<NodeId, Rc<dyn SelectableRows>>>>;

#[derive(Clone, Default)]
struct Registry(Lists);

fn registry() -> Lists {
    tree_local(Registry::default).0
}

pub(crate) fn register_rows(list: NodeId, rows: Rc<dyn SelectableRows>) {
    registry().borrow_mut().insert(list, rows);
}

pub(crate) fn unregister_rows(list: NodeId) {
    if let Some(lists) = beui_core::current::try_with_document(|document| {
        document.extension::<Registry>().map(|registry| registry.0)
    })
    .flatten()
    {
        lists.borrow_mut().remove(&list);
    }
}

pub(crate) fn rows_of(list: NodeId) -> Option<Rc<dyn SelectableRows>> {
    registry().borrow().get(&list).cloned()
}

#[derive(Clone)]
pub(crate) enum SelectionItem {
    Text(NodeId),
    Rows(NodeId, Rc<dyn SelectableRows>),
}

pub(crate) fn selection_items(root: NodeId) -> Vec<SelectionItem> {
    let lists = registry();
    let lists = lists.borrow();
    with_document(|document| {
        let mut found = Vec::new();
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            if !document.arena.contains(id)
                || document.arena.taken(id)
                || document.node_rect(id).is_none()
            {
                continue;
            }
            if let Some(rows) = lists.get(&id) {
                found.push(SelectionItem::Rows(id, Rc::clone(rows)));
                continue;
            }
            if let Some(text) = document.arena.kind_of::<TextNode>(id) {
                if !document.text(text).is_empty() {
                    found.push(SelectionItem::Text(id));
                }
                continue;
            }
            pending.extend(document.arena.get(id).children().into_iter().rev());
        }
        found
    })
}

pub(crate) fn texts_under(root: NodeId) -> Vec<NodeId> {
    with_document(|document| {
        let mut found = Vec::new();
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            if !document.arena.contains(id) || document.arena.taken(id) {
                continue;
            }
            if document.arena.kind_of::<TextNode>(id).is_some() {
                found.push(id);
                continue;
            }
            pending.extend(document.arena.get(id).children().into_iter().rev());
        }
        found
    })
}

pub(crate) struct Beyond {
    pub inside: Pos2,
    pub scrolled: bool,
}

pub(crate) fn scroll_toward(node: NodeId, pos: Pos2, rate: f32) -> Beyond {
    with_document(|document| {
        let mut current = document.arena.parent(node);
        let offset = loop {
            let Some(id) = current else {
                return Beyond {
                    inside: pos,
                    scrolled: false,
                };
            };
            if let Some(offset) = document.arena.kind_of::<OffsetNode>(id)
                && document.arena.get_as::<OffsetNode>(offset).direction == Direction::Vertical
            {
                break offset;
            }
            current = document.arena.parent(id);
        };
        let (Some(rect), Some(position)) =
            (document.node_rect(offset), document.offset_position(offset))
        else {
            return Beyond {
                inside: pos,
                scrolled: false,
            };
        };
        let (top, bottom) = edges(rect);
        let past = match pos.y {
            y if y < top => y - top,
            y if y > bottom => y - bottom,
            _ => 0.0,
        };
        let inside = Pos2::new(pos.x, pos.y.clamp(top, bottom));
        if past == 0.0 {
            return Beyond {
                inside,
                scrolled: false,
            };
        }
        let current = document.offset_value(offset);
        let wanted = (current + past * rate).clamp(0.0, position.max_offset());
        if wanted != current {
            document.set_offset_value(offset, wanted);
        }
        Beyond {
            inside,
            scrolled: wanted != current,
        }
    })
}

fn edges(rect: Rect) -> (f32, f32) {
    (rect.top(), (rect.bottom() - 1.0).max(rect.top()))
}

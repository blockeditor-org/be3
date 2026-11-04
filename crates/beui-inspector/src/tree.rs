use accesskit::{Node as AccessNode, NodeId as AccessNodeId};

use beui_core::accessibility::AccessibilityView;
use beui_core::component_tree::ComponentTreeKey;
use beui_core::document::Document;
use beui_types::NodeId;

use super::State;

const AUTO_EXPAND_DEPTH: usize = 3;
const DETAIL_LIMIT: usize = 24;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Node(NodeId),
    Component(NodeId, usize),
    Internals(NodeId, usize),
    Text(NodeId),
    AccessKit(NodeId),
}

impl Key {
    pub fn node(self) -> NodeId {
        match self {
            Key::Node(id)
            | Key::Component(id, _)
            | Key::Internals(id, _)
            | Key::Text(id)
            | Key::AccessKit(id) => id,
        }
    }

    pub fn test_id(self) -> String {
        match self {
            Key::Node(id) => format!("inspector.row.node.{}", id.index()),
            Key::Component(id, level) => {
                format!("inspector.row.component.{}.{level}", id.index())
            }
            Key::Internals(id, level) => format!("inspector.row.internals.{}.{level}", id.index()),
            Key::Text(id) => format!("inspector.row.text.{}", id.index()),
            Key::AccessKit(id) => format!("inspector.row.accesskit.{}", id.index()),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Entry {
    pub key: Key,
    pub depth: usize,
    pub kind: String,
    pub expandable: bool,
    pub expanded: bool,
    pub detail: String,
    pub size: String,
}

pub fn collect(target: &Document, state: &State) -> Vec<Entry> {
    let mut entries = Vec::new();
    if let Some(root) = target.root() {
        visit(target, state, Key::Node(root), 0, &mut entries);
    }
    entries
}

fn component_key(key: ComponentTreeKey) -> Key {
    match key {
        ComponentTreeKey::Component(node, level) => Key::Component(node, level),
        ComponentTreeKey::Internals(node, level) => Key::Internals(node, level),
        ComponentTreeKey::Text(node) => Key::Text(node),
    }
}

pub fn collect_components(target: &Document, state: &State) -> Vec<Entry> {
    fn visit(
        target: &Document,
        state: &State,
        tree: &beui_core::component_tree::ComponentTree,
        index: usize,
        depth: usize,
        entries: &mut Vec<Entry>,
    ) {
        let row = &tree.rows[index];
        let key = component_key(row.key);
        let expandable = !row.children.is_empty();
        let expanded = expandable && state.expanded(key, row.expanded);
        entries.push(Entry {
            key,
            depth,
            kind: row.label.clone(),
            expandable,
            expanded,
            detail: String::new(),
            size: size(target, key.node()),
        });
        if expanded {
            for child in &row.children {
                visit(target, state, tree, *child, depth + 1, entries);
            }
        }
    }
    let tree = target.component_tree();
    let mut entries = Vec::new();
    for root in &tree.roots {
        visit(target, state, &tree, *root, 0, &mut entries);
    }
    entries
}

pub fn component_count(target: &Document) -> usize {
    target
        .component_tree()
        .rows
        .iter()
        .filter(|row| matches!(row.key, ComponentTreeKey::Component(..)))
        .count()
}

pub fn component_path(target: &Document, id: NodeId, chosen: Option<Key>) -> Vec<Key> {
    fn descend(
        tree: &beui_core::component_tree::ComponentTree,
        index: usize,
        id: NodeId,
        chosen: Option<Key>,
        path: &mut Vec<Key>,
    ) -> bool {
        let row = &tree.rows[index];
        path.push(component_key(row.key));
        if chosen.map_or(row.key.node() == id, |key| component_key(row.key) == key)
            || row
                .children
                .iter()
                .any(|child| descend(tree, *child, id, chosen, path))
        {
            return true;
        }
        path.pop();
        false
    }
    let tree = target.component_tree();
    let mut path = Vec::new();
    for root in &tree.roots {
        if descend(&tree, *root, id, chosen, &mut path) {
            break;
        }
    }
    path
}

pub fn collect_accesskit(target: &Document, state: &State) -> Vec<Entry> {
    let Some(nodes) = target.accessibility_view() else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    visit_accesskit(target, state, &nodes, nodes.root(), 0, &mut entries);
    entries
}

pub fn accesskit_count(target: &Document) -> usize {
    target.accessibility_view().map_or(0, |nodes| nodes.len())
}

pub fn accesskit_path(target: &Document, id: NodeId) -> Vec<Key> {
    let Some(nodes) = target.accessibility_view() else {
        return Vec::new();
    };
    let mut path = Vec::new();
    descend_accesskit(target, &nodes, nodes.root(), id, &mut path);
    path
}

fn descend_accesskit(
    target: &Document,
    nodes: &AccessibilityView<'_>,
    access_id: AccessNodeId,
    id: NodeId,
    path: &mut Vec<Key>,
) -> bool {
    let Some(node_id) = target.local_node_id(access_id) else {
        return false;
    };
    path.push(Key::AccessKit(node_id));
    if node_id == id {
        return true;
    }
    if let Some(node) = nodes.get(&access_id) {
        for child in node.children() {
            if descend_accesskit(target, nodes, *child, id, path) {
                return true;
            }
        }
    }
    path.pop();
    false
}

fn visit_accesskit(
    target: &Document,
    state: &State,
    nodes: &AccessibilityView<'_>,
    access_id: AccessNodeId,
    depth: usize,
    entries: &mut Vec<Entry>,
) {
    let Some(node) = nodes.get(&access_id) else {
        return;
    };
    let Some(id) = target.local_node_id(access_id) else {
        return;
    };
    let key = Key::AccessKit(id);
    let expandable = !node.children().is_empty();
    let expanded = expandable && state.expanded(key, depth < AUTO_EXPAND_DEPTH);
    entries.push(Entry {
        key,
        depth,
        kind: format!("{:?}", node.role()),
        expandable,
        expanded,
        detail: accesskit_detail(node),
        size: accesskit_size(node),
    });
    if expanded {
        for child in node.children() {
            visit_accesskit(target, state, nodes, *child, depth + 1, entries);
        }
    }
}

fn visit(target: &Document, state: &State, key: Key, depth: usize, entries: &mut Vec<Entry>) {
    let children = children(target, key);
    let expandable = !children.is_empty();
    let expanded = expandable && state.expanded(key, auto_expand(key, depth));
    entries.push(Entry {
        key,
        depth,
        kind: kind(target, key).to_owned(),
        expandable,
        expanded,
        detail: detail(target, key),
        size: size(target, key.node()),
    });
    if expanded {
        for child in children {
            visit(target, state, child, depth + 1, entries);
        }
    }
}

pub fn path(target: &Document, id: NodeId) -> Vec<Key> {
    let mut path = Vec::new();
    if let Some(root) = target.root() {
        descend(target, Key::Node(root), id, &mut path);
    }
    path
}

fn descend(target: &Document, key: Key, id: NodeId, path: &mut Vec<Key>) -> bool {
    path.push(key);
    if key == Key::Node(id) {
        return true;
    }
    for child in children(target, key) {
        if descend(target, child, id, path) {
            return true;
        }
    }
    path.pop();
    false
}

pub fn count(target: &Document, id: NodeId) -> usize {
    1 + target
        .children(id)
        .into_iter()
        .map(|child| count(target, child))
        .sum::<usize>()
}

pub fn properties(target: &Document, id: NodeId) -> Vec<(String, String)> {
    let mut rows = vec![(
        "node".to_owned(),
        format!("{} #{}", target.node_kind(id), id.index()),
    )];
    let names = target.component_names(id);
    if !names.is_empty() {
        rows.push(("components".to_owned(), names.join(" > ")));
    }
    let test_ids = target.node_test_ids(id);
    if !test_ids.is_empty() {
        rows.push(("test id".to_owned(), test_ids.join(", ")));
    }
    if let Some(rect) = target.node_rect(id) {
        rows.push((
            "position".to_owned(),
            format!("{}, {}", rect.left().round(), rect.top().round()),
        ));
        rows.push((
            "size".to_owned(),
            format!("{} x {}", rect.width().round(), rect.height().round()),
        ));
    }
    if let Some(parent) = target.node_parent(id) {
        rows.push((
            "parent".to_owned(),
            format!("{} #{}", target.node_kind(parent), parent.index()),
        ));
    }
    rows.push(("children".to_owned(), target.children(id).len().to_string()));
    rows.push((
        "descendants".to_owned(),
        (count(target, id) - 1).to_string(),
    ));
    if target.focused_node() == Some(id) {
        rows.push(("focused".to_owned(), "true".to_owned()));
    }
    rows.extend(
        target
            .node_properties(id)
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value)),
    );
    rows
}

pub fn label(target: &Document, id: NodeId) -> String {
    match target.node_detail(id) {
        Some(detail) => format!("{} {}", target.node_kind(id), trim(&detail)),
        None => target.node_kind(id).to_owned(),
    }
}

fn children(target: &Document, key: Key) -> Vec<Key> {
    match key {
        Key::AccessKit(_) => Vec::new(),
        Key::Node(id) => target.children(id).into_iter().map(Key::Node).collect(),
        Key::Component(..) | Key::Internals(..) | Key::Text(..) => unreachable!(),
    }
}

fn kind(target: &Document, key: Key) -> &'static str {
    match key {
        Key::Node(id) => target.node_kind(id),
        Key::Component(..) | Key::Internals(..) | Key::Text(..) => unreachable!(),
        Key::AccessKit(_) => unreachable!(),
    }
}

fn auto_expand(key: Key, depth: usize) -> bool {
    match key {
        Key::Component(..) | Key::Internals(..) | Key::Text(..) => unreachable!(),
        Key::Node(_) | Key::AccessKit(_) => depth < AUTO_EXPAND_DEPTH,
    }
}

fn detail(target: &Document, key: Key) -> String {
    let detail = match key {
        Key::Node(id) => target.node_detail(id),
        Key::Component(..) | Key::Internals(..) | Key::Text(..) => unreachable!(),
        Key::AccessKit(_) => unreachable!(),
    };
    detail.map(|detail| trim(&detail)).unwrap_or_default()
}

fn accesskit_detail(node: &AccessNode) -> String {
    node.label()
        .or_else(|| node.value())
        .or_else(|| node.description())
        .map(trim)
        .unwrap_or_default()
}

fn accesskit_size(node: &AccessNode) -> String {
    match node.bounds() {
        Some(bounds) => format!("{} x {}", bounds.width().round(), bounds.height().round()),
        None => String::new(),
    }
}

fn size(target: &Document, id: NodeId) -> String {
    match target.node_rect(id) {
        Some(rect) => format!("{} x {}", rect.width().round(), rect.height().round()),
        None => String::new(),
    }
}

fn trim(detail: &str) -> String {
    let detail = detail.replace(['\n', '\t'], " ");
    if detail.chars().count() <= DETAIL_LIMIT {
        return detail;
    }
    let kept: String = detail.chars().take(DETAIL_LIMIT).collect();
    format!("{kept}...")
}

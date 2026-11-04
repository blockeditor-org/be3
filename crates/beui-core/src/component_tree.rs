use crate::document::Document;
use beui_types::NodeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComponentTreeKey {
    Component(NodeId, usize),
    Internals(NodeId, usize),
    Text(NodeId),
}

impl ComponentTreeKey {
    pub fn node(self) -> NodeId {
        match self {
            Self::Component(node, _) | Self::Internals(node, _) | Self::Text(node) => node,
        }
    }
}

pub struct ComponentTreeRow {
    pub key: ComponentTreeKey,
    pub label: String,
    pub children: Vec<usize>,
    pub expanded: bool,
}

#[derive(Default)]
pub struct ComponentTree {
    pub rows: Vec<ComponentTreeRow>,
    pub roots: Vec<usize>,
}

impl ComponentTree {
    fn push(&mut self, parent: Option<usize>, row: ComponentTreeRow) -> usize {
        let index = self.rows.len();
        self.rows.push(row);
        match parent {
            Some(parent) => self.rows[parent].children.push(index),
            None => self.roots.push(index),
        }
        index
    }

    pub fn text(&self, document: &Document, include_internals: bool) -> String {
        fn visit(
            tree: &ComponentTree,
            document: &Document,
            index: usize,
            depth: usize,
            internals: bool,
            out: &mut String,
        ) {
            let row = &tree.rows[index];
            if matches!(row.key, ComponentTreeKey::Internals(..)) && !internals {
                return;
            }
            out.push_str(&"  ".repeat(depth));
            out.push_str(&row.label);
            if matches!(row.key, ComponentTreeKey::Component(..)) {
                for id in document.node_test_ids(row.key.node()) {
                    out.push_str(" #");
                    out.push_str(id);
                }
            }
            out.push('\n');
            for child in &row.children {
                visit(tree, document, *child, depth + 1, internals, out);
            }
        }
        let mut out = String::new();
        for root in &self.roots {
            visit(self, document, *root, 0, include_internals, &mut out);
        }
        out
    }
}

impl Document {
    pub fn component_tree(&self) -> ComponentTree {
        fn visit(
            document: &Document,
            node: NodeId,
            stack: &mut Vec<(u64, usize, usize, Option<u64>)>,
            tree: &mut ComponentTree,
        ) {
            let start = stack.len();
            let mut outermost = None;
            for (level, record) in document.components(node).iter().enumerate().rev() {
                let owner = record
                    .owner
                    .and_then(|owner| stack.iter().rposition(|entry| entry.0 == owner));
                let passed_to = stack
                    .iter()
                    .skip(owner.map_or(0, |owner| owner + 1))
                    .rev()
                    .find(|entry| entry.3 == record.owner);
                let parent = passed_to
                    .map(|entry| entry.1)
                    .or_else(|| owner.map(|owner| stack[owner].2))
                    .or_else(|| stack.first().map(|entry| entry.1));
                let index = tree.push(
                    parent,
                    ComponentTreeRow {
                        key: ComponentTreeKey::Component(node, level),
                        label: record.info.label(),
                        children: Vec::new(),
                        expanded: true,
                    },
                );
                outermost.get_or_insert(index);
                let internals = tree.push(
                    Some(index),
                    ComponentTreeRow {
                        key: ComponentTreeKey::Internals(node, level),
                        label: "internals".to_owned(),
                        children: Vec::new(),
                        expanded: !record.info.internal_by_default(),
                    },
                );
                stack.push((record.id, index, internals, record.owner));
            }
            if document.node_kind(node) == "text"
                && let Some(text) = document.node_detail(node)
            {
                tree.push(
                    outermost.or_else(|| stack.last().map(|entry| entry.1)),
                    ComponentTreeRow {
                        key: ComponentTreeKey::Text(node),
                        label: text,
                        children: Vec::new(),
                        expanded: true,
                    },
                );
            }
            for child in document.children(node) {
                visit(document, child, stack, tree);
            }
            stack.truncate(start);
        }
        let mut tree = ComponentTree::default();
        if let Some(root) = self.root() {
            visit(self, root, &mut Vec::new(), &mut tree);
        }
        for index in 0..tree.rows.len() {
            let children = tree.rows[index]
                .children
                .iter()
                .copied()
                .filter(|child| {
                    !matches!(tree.rows[*child].key, ComponentTreeKey::Internals(..))
                        || !tree.rows[*child].children.is_empty()
                })
                .collect();
            tree.rows[index].children = children;
        }
        tree
    }
}

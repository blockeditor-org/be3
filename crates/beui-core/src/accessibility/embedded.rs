use accesskit::{Affine, Node, NodeId as AccessNodeId, Rect as AccessRect, Role, TreeUpdate};
use foldhash::{HashMap, HashMapExt, HashSet, HashSetExt};

use super::{DOCUMENT_SHIFT, Fragment};
use crate::geometry::{Rect, Vec2};

const PANE: u64 = 0;

pub struct Embedded {
    document: u32,
    nodes: HashMap<AccessNodeId, Node>,
    root: Option<AccessNodeId>,
    focus: Option<AccessNodeId>,
    local: HashMap<AccessNodeId, u64>,
    remote: HashMap<u64, AccessNodeId>,
    parents: HashMap<AccessNodeId, AccessNodeId>,
    next: u64,
    changed: HashSet<AccessNodeId>,
    restructured: bool,
    placed: Option<(String, Rect, Vec2, f32)>,
}

impl Embedded {
    pub fn new(document: u32) -> Self {
        Self {
            document,
            nodes: HashMap::new(),
            root: None,
            focus: None,
            local: HashMap::new(),
            remote: HashMap::new(),
            parents: HashMap::new(),
            next: PANE + 1,
            changed: HashSet::new(),
            restructured: false,
            placed: None,
        }
    }

    pub fn document(&self) -> u32 {
        self.document
    }

    pub fn update(&mut self, update: TreeUpdate) {
        if let Some(tree) = update.tree
            && self.root != Some(tree.root)
        {
            self.root = Some(tree.root);
            self.restructured = true;
        }
        for (id, node) in update.nodes {
            match self.nodes.get(&id) {
                Some(old) if *old == node => continue,
                Some(old) => self.restructured |= old.children() != node.children(),
                None => self.restructured = true,
            }
            self.changed.insert(id);
            self.nodes.insert(id, node);
        }
        self.focus = Some(update.focus);
    }

    pub fn fragment(
        &mut self,
        title: &str,
        area: Rect,
        origin: Vec2,
        scale: f32,
        focused: bool,
        full: bool,
    ) -> Fragment {
        let placed = (title.to_owned(), area, origin, scale);
        let moved = self.placed.as_ref() != Some(&placed);
        self.placed = Some(placed);
        let restructured = std::mem::take(&mut self.restructured);
        if restructured {
            self.restructure();
        }
        let resend = full || restructured;
        let root = self.root.filter(|root| self.nodes.contains_key(root));
        let mut nodes = Vec::new();
        if resend || moved {
            let mut pane = Node::new(Role::Pane);
            pane.set_label(title);
            pane.set_bounds(bounds(area));
            if let Some(root) = root {
                pane.set_children(vec![self.host(root)]);
            }
            nodes.push((self.host_id(PANE), pane));
        }
        let changed = std::mem::take(&mut self.changed);
        let mut sent: Vec<AccessNodeId> = match resend {
            true => self.nodes.keys().copied().collect(),
            false => changed
                .into_iter()
                .filter(|id| self.nodes.contains_key(id))
                .collect(),
        };
        if moved
            && !resend
            && let Some(root) = root
            && !sent.contains(&root)
        {
            sent.push(root);
        }
        sent.sort_unstable_by_key(|id| id.0);
        for id in sent {
            let node = self.translate(id, origin, scale);
            nodes.push((self.host(id), node));
        }
        let focus = self
            .focus
            .filter(|focus| focused && Some(*focus) != root && self.nodes.contains_key(focus))
            .map(|focus| self.host(focus));
        Fragment {
            nodes,
            root: self.host_id(PANE),
            focus,
            windows: Vec::new(),
            window_roots: Vec::new(),
        }
    }

    pub fn plugin_node(&self, id: AccessNodeId) -> Option<AccessNodeId> {
        if super::document_of(id) != u64::from(self.document) {
            return None;
        }
        match id.0 & ((1 << DOCUMENT_SHIFT) - 1) {
            PANE => self.root,
            local => self.remote.get(&local).copied(),
        }
    }

    fn restructure(&mut self) {
        self.parents.clear();
        let mut reached = HashSet::with_capacity(self.nodes.len());
        let mut pending = Vec::new();
        if let Some(root) = self.root.filter(|root| self.nodes.contains_key(root)) {
            reached.insert(root);
            pending.push(root);
        }
        while let Some(id) = pending.pop() {
            let Some(node) = self.nodes.get(&id) else {
                continue;
            };
            for child in node.children().iter().rev() {
                if self.nodes.contains_key(child) && reached.insert(*child) {
                    self.parents.insert(*child, id);
                    pending.push(*child);
                }
            }
        }
        self.nodes.retain(|id, _| reached.contains(id));
        self.changed.retain(|id| reached.contains(id));
        let remote = &mut self.remote;
        self.local.retain(|id, local| {
            let kept = reached.contains(id);
            if !kept {
                remote.remove(local);
            }
            kept
        });
    }

    fn translate(&mut self, id: AccessNodeId, origin: Vec2, scale: f32) -> Node {
        let mut node = self.nodes[&id].clone();
        let mut seen = HashSet::new();
        let children: Vec<AccessNodeId> = node
            .children()
            .iter()
            .filter(|child| self.parents.get(child) == Some(&id) && seen.insert(**child))
            .copied()
            .collect();
        node.set_children(
            children
                .into_iter()
                .map(|child| self.host(child))
                .collect::<Vec<_>>(),
        );
        macro_rules! relations {
            ($($getter:ident $setter:ident $clearer:ident),*) => {$(
                let related: Vec<AccessNodeId> = node
                    .$getter()
                    .iter()
                    .filter(|related| self.nodes.contains_key(related))
                    .copied()
                    .collect();
                match related.is_empty() {
                    true => node.$clearer(),
                    false => node.$setter(
                        related.into_iter().map(|related| self.host(related)).collect::<Vec<_>>(),
                    ),
                }
            )*};
        }
        relations!(
            controls set_controls clear_controls,
            details set_details clear_details,
            described_by set_described_by clear_described_by,
            flow_to set_flow_to clear_flow_to,
            labelled_by set_labelled_by clear_labelled_by,
            owns set_owns clear_owns,
            radio_group set_radio_group clear_radio_group
        );
        macro_rules! relation {
            ($($getter:ident $setter:ident $clearer:ident),*) => {$(
                match node.$getter().filter(|related| self.nodes.contains_key(related)) {
                    Some(related) => {
                        let related = self.host(related);
                        node.$setter(related);
                    }
                    None => node.$clearer(),
                }
            )*};
        }
        relation!(
            active_descendant set_active_descendant clear_active_descendant,
            error_message set_error_message clear_error_message,
            in_page_link_target set_in_page_link_target clear_in_page_link_target,
            member_of set_member_of clear_member_of,
            next_on_line set_next_on_line clear_next_on_line,
            previous_on_line set_previous_on_line clear_previous_on_line,
            popup_for set_popup_for clear_popup_for
        );
        if let Some(mut selection) = node.text_selection().copied() {
            match (
                self.nodes.contains_key(&selection.anchor.node),
                self.nodes.contains_key(&selection.focus.node),
            ) {
                (true, true) => {
                    selection.anchor.node = self.host(selection.anchor.node);
                    selection.focus.node = self.host(selection.focus.node);
                    node.set_text_selection(selection);
                }
                _ => node.clear_text_selection(),
            }
        }
        node.clear_tree_id();
        if Some(id) == self.root {
            let local = node.transform().copied().unwrap_or(Affine::IDENTITY);
            node.set_transform(
                Affine::translate((origin.x.into(), origin.y.into()))
                    * Affine::scale(scale.into())
                    * local,
            );
            node.set_role(Role::GenericContainer);
            node.clear_label();
        }
        node
    }

    fn host(&mut self, id: AccessNodeId) -> AccessNodeId {
        let local = match self.local.get(&id) {
            Some(local) => *local,
            None => {
                let local = self.next;
                self.next += 1;
                self.local.insert(id, local);
                self.remote.insert(local, id);
                local
            }
        };
        self.host_id(local)
    }

    fn host_id(&self, local: u64) -> AccessNodeId {
        AccessNodeId((u64::from(self.document) << DOCUMENT_SHIFT) | local)
    }
}

fn bounds(rect: Rect) -> AccessRect {
    AccessRect::new(
        rect.min.x.into(),
        rect.min.y.into(),
        rect.max.x.into(),
        rect.max.y.into(),
    )
}

//! Grafting an app's accessibility tree into the host's: the app describes each screen as an
//! AccessKit tree in its own ids and coordinates (`FrameReport::description`); the host
//! shows it as a pane under its own tree, in the host's ids and placed where the screen is
//! drawn. `Graft` keeps one screen's tree and sends only what changed (`Graft::grafted`), and
//! takes the host's ids back to the app's for the actions the host is asked for
//! (`Graft::app_node`). It knows no GUI framework: the host puts the pane under its own tree.

use be_lib_geometry::{Rect, Vec2};
use be_lib_stable_hash::{HashMap, HashSet};
use be_sdk_api::accesskit::{Affine, Node, NodeId, Rect as AccessRect, Role, TreeUpdate};

/// A host id is `document << DOCUMENT_SHIFT | local`: each grafted screen numbers its nodes in a
/// document of its own, which the host gives it (`Graft::new`) from the same space as its own
/// documents.
pub const DOCUMENT_SHIFT: u32 = 40;

const PANE: u64 = 0;

/// What a graft sends the host's tree this frame (`Graft::grafted`), in the host's ids.
pub struct Grafted {
    /// The nodes that changed, the pane among them when it did.
    pub nodes: Vec<(NodeId, Node)>,
    /// The pane the app's tree hangs under, for the host to make a child of its own node.
    pub root: NodeId,
    /// The app's focused node, while its screen has the keyboard.
    pub focus: Option<NodeId>,
}

/// Where a screen's tree is drawn this frame.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Placing<'a> {
    /// The pane's label: the app's name, or none for a pane that is not a window of its own
    /// (an anchored view, a dock bar).
    pub title: Option<&'a str>,
    /// The pane's bounds, where the screen shows (its rect clipped), in host coordinates.
    pub area: Rect,
    /// Where the screen's origin is drawn, in host coordinates.
    pub origin: Vec2,
    /// How much larger the screen is drawn than its app lays it out (its zoom).
    pub scale: f32,
}

/// One screen's tree, grafted into the host's: what the app has described of it, and the host
/// ids its nodes were given.
pub struct Graft {
    document: u32,
    nodes: HashMap<NodeId, Node>,
    root: Option<NodeId>,
    focus: Option<NodeId>,
    local: HashMap<NodeId, u64>,
    remote: HashMap<u64, NodeId>,
    parents: HashMap<NodeId, NodeId>,
    next: u64,
    changed: HashSet<NodeId>,
    restructured: bool,
    placed: Option<(Option<String>, Rect, Vec2, f32)>,
}

impl Graft {
    /// A graft that names its nodes in `document`, which the host's tree must reserve for it:
    /// no other graft and none of the host's own nodes may use it.
    pub fn new(document: u32) -> Self {
        Self {
            document,
            nodes: HashMap::default(),
            root: None,
            focus: None,
            local: HashMap::default(),
            remote: HashMap::default(),
            parents: HashMap::default(),
            next: PANE + 1,
            changed: HashSet::default(),
            restructured: false,
            placed: None,
        }
    }

    /// The document its host ids are in, by which the host routes an action back to it.
    pub fn document(&self) -> u32 {
        self.document
    }

    /// Forgets the app's tree, as a restart does (`Run::restart`): the pane stays, empty, until
    /// the new run describes the screen. Host ids are not reused, so one the old run's node had
    /// never names a node of the new run's.
    pub(crate) fn restarted(&mut self) {
        self.nodes.clear();
        self.local.clear();
        self.remote.clear();
        self.parents.clear();
        self.root = None;
        self.focus = None;
        self.changed.clear();
        self.restructured = true;
    }

    /// Takes the app's update to its tree, sent with its frame report.
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

    /// What to send the host's tree for the screen placed at `placing`: everything when `full`
    /// (the host's tree does not have it) or when the app's tree changed shape, else what
    /// changed and, if the screen moved, the pane and the app's root. Its focus counts only while
    /// the screen has the keyboard (`focused`).
    pub fn grafted(&mut self, placing: Placing, focused: bool, full: bool) -> Grafted {
        let Placing {
            title,
            area,
            origin,
            scale,
        } = placing;
        let placed = (title.map(str::to_owned), area, origin, scale);
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
            let mut pane = match title {
                Some(title) => {
                    let mut pane = Node::new(Role::Pane);
                    pane.set_label(title);
                    pane
                }
                None => Node::new(Role::GenericContainer),
            };
            pane.set_bounds(bounds(area));
            if let Some(root) = root {
                pane.set_children(vec![self.host(root)]);
            }
            nodes.push((self.host_id(PANE), pane));
        }
        let changed = std::mem::take(&mut self.changed);
        let mut sent: Vec<NodeId> = match resend {
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
        Grafted {
            nodes,
            root: self.host_id(PANE),
            focus,
        }
    }

    /// The app's id for a node of the host's, if it is one of this graft's: the pane stands
    /// for the app's root.
    pub fn app_node(&self, id: NodeId) -> Option<NodeId> {
        if id.0 >> DOCUMENT_SHIFT != u64::from(self.document) {
            return None;
        }
        match id.0 & ((1 << DOCUMENT_SHIFT) - 1) {
            PANE => self.root,
            local => self.remote.get(&local).copied(),
        }
    }

    fn restructure(&mut self) {
        self.parents.clear();
        let mut reached = HashSet::with_capacity_and_hasher(self.nodes.len(), Default::default());
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

    fn translate(&mut self, id: NodeId, origin: Vec2, scale: f32) -> Node {
        let mut node = self.nodes[&id].clone();
        let mut seen = HashSet::default();
        let children: Vec<NodeId> = node
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
                let related: Vec<NodeId> = node
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

    fn host(&mut self, id: NodeId) -> NodeId {
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

    fn host_id(&self, local: u64) -> NodeId {
        NodeId((u64::from(self.document) << DOCUMENT_SHIFT) | local)
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

#[cfg(test)]
mod tests;

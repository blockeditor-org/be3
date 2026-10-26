use std::cell::Ref;

use foldhash::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use accesskit::{
    Action, ActionData, ActionRequest, Affine, Node, NodeId as AccessNodeId, Rect as AccessRect,
    Role, Tree, TreeId, TreeUpdate,
};

use crate::base::focus::focus_of;
use crate::base::list::Direction;
use crate::base::offset::OffsetNode;
use crate::base::overlay::OverlayNode;

pub const LAYER_CLASS: &str = "beui-layer";
pub const TEST_ID_SEPARATOR: char = '\u{1f}';
use crate::base::text::TextNode;
use crate::document::Document;
use crate::geometry::{Pos2, Vec2};
use crate::input::{Key, KeyPress, Modifiers};
use crate::node::{Arena, NodeId, NodeMap, Placed, Rects, SPACE_SLOTS, SpaceId};
use crate::window::WindowId;

mod embedded;

pub use embedded::Embedded;

pub const WINDOW_NODE: AccessNodeId = AccessNodeId(0);
pub(crate) const DOCUMENT_SHIFT: u32 = 40;
const GENERATION_SHIFT: u32 = 32;
const SLOT_SHIFT: u32 = 30;
const SLOT_MASK: u32 = 0b11;
const INDEX_MASK: u32 = (1 << SLOT_SHIFT) - 1;
const FALLBACK_NUMERIC_STEP: f64 = 0.05;
static NEXT_DOCUMENT_ID: AtomicU32 = AtomicU32::new(1);

#[derive(Clone)]
pub struct Fragment {
    pub nodes: Vec<(AccessNodeId, Node)>,
    pub root: AccessNodeId,
    pub focus: Option<AccessNodeId>,
    pub windows: Vec<(AccessNodeId, WindowId)>,
    pub window_roots: Vec<(WindowId, AccessNodeId)>,
    pub focus_window: Option<WindowId>,
}

impl Fragment {
    pub fn scale(&mut self, scale: f32) {
        let root = self.root;
        for (id, node) in &mut self.nodes {
            if *id == root {
                let local = node.transform().copied().unwrap_or(Affine::IDENTITY);
                node.set_transform(Affine::scale(scale.into()) * local);
            }
        }
    }
}

pub fn next_document_id() -> u32 {
    NEXT_DOCUMENT_ID.fetch_add(1, Ordering::Relaxed)
}

pub fn document_of(id: AccessNodeId) -> u64 {
    id.0 >> DOCUMENT_SHIFT
}

pub fn tree_update(
    title: &str,
    viewport: Vec2,
    scale: f32,
    fragments: Vec<Fragment>,
) -> TreeUpdate {
    let mut root = Node::new(Role::Window);
    root.set_label(title);
    root.set_bounds(AccessRect::new(
        0.0,
        0.0,
        viewport.x.into(),
        viewport.y.into(),
    ));
    root.set_transform(Affine::scale(scale.into()));
    root.set_children(
        fragments
            .iter()
            .map(|fragment| fragment.root)
            .collect::<Vec<_>>(),
    );
    let focus = fragments
        .iter()
        .find_map(|fragment| fragment.focus)
        .unwrap_or(WINDOW_NODE);
    let mut nodes = vec![(WINDOW_NODE, root)];
    nodes.extend(fragments.into_iter().flat_map(|fragment| fragment.nodes));
    TreeUpdate {
        nodes,
        tree: Some(Tree {
            root: WINDOW_NODE,
            toolkit_name: Some("beui".to_owned()),
            toolkit_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        }),
        tree_id: TreeId::ROOT,
        focus,
    }
}

pub fn window_tree_update(
    window: WindowId,
    title: &str,
    viewport: Vec2,
    scale: f32,
    fragments: &[Fragment],
) -> TreeUpdate {
    let roots: Vec<AccessNodeId> = fragments
        .iter()
        .flat_map(|fragment| &fragment.window_roots)
        .filter(|(held, _)| *held == window)
        .map(|(_, root)| *root)
        .collect();
    let in_window = |fragment: &Fragment, id: AccessNodeId| {
        fragment
            .windows
            .iter()
            .any(|(node, held)| *node == id && *held == window)
    };
    let mut root = Node::new(Role::Window);
    root.set_label(title);
    root.set_bounds(AccessRect::new(
        0.0,
        0.0,
        viewport.x.into(),
        viewport.y.into(),
    ));
    root.set_transform(Affine::scale(scale.into()));
    root.set_children(roots);
    let focus = fragments
        .iter()
        .filter(|fragment| fragment.focus_window == Some(window))
        .find_map(|fragment| fragment.focus)
        .unwrap_or(WINDOW_NODE);
    let mut nodes = vec![(WINDOW_NODE, root)];
    for fragment in fragments {
        nodes.extend(
            fragment
                .nodes
                .iter()
                .filter(|(id, _)| in_window(fragment, *id))
                .cloned(),
        );
    }
    TreeUpdate {
        nodes,
        tree: Some(Tree {
            root: WINDOW_NODE,
            toolkit_name: Some("beui".to_owned()),
            toolkit_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        }),
        tree_id: TreeId::ROOT,
        focus,
    }
}

#[derive(Default)]
pub struct AccessibilityTree {
    root: Option<NodeId>,
    entries: NodeMap<Entry>,
    spaces: HashMap<SpaceId, (Entry, u64)>,
    parents: NodeMap<NodeId>,
    marks: NodeMap<u64>,
    reached: NodeMap<u64>,
    gathered: NodeMap<Gathering>,
    detached: NodeMap<Spot>,
    moved: Vec<SpaceId>,
    pass: u64,
    focus: Option<AccessNodeId>,
    changed: Vec<SpaceId>,
    inspected: bool,
    scale: Option<f32>,
}

#[derive(Clone, Copy, PartialEq)]
struct Spot {
    frame: Option<SpaceId>,
    at: Pos2,
    scale: f32,
}

#[derive(Clone, Copy)]
struct Frame {
    space: Option<SpaceId>,
    shift: Vec2,
    scale: f32,
    walk: Option<SpaceId>,
}

impl Frame {
    const SCREEN: Self = Self {
        space: None,
        shift: Vec2::ZERO,
        scale: 1.0,
        walk: None,
    };

    fn within(space: SpaceId) -> Self {
        Self {
            space: Some(space),
            shift: Vec2::ZERO,
            scale: 1.0,
            walk: Some(space),
        }
    }
}

struct Entry {
    node: Node,
    focus: Option<AccessNodeId>,
    key: Spot,
}

#[derive(Default)]
struct Gathered {
    children: Vec<AccessNodeId>,
    focus: Option<AccessNodeId>,
}

struct Gathering {
    children: Vec<AccessNodeId>,
    focus: Option<AccessNodeId>,
    key: Spot,
}

impl AccessibilityTree {
    pub fn mark(&mut self, id: NodeId, arena: &Arena) {
        if self.root.is_none() {
            return;
        }
        let mut current = Some(id);
        while let Some(node) = current {
            if self.marks.get(&node) == Some(&self.pass) {
                return;
            }
            self.marks.insert(node, self.pass);
            current = self
                .parents
                .get(&node)
                .copied()
                .or_else(|| arena.parent(node));
        }
    }

    pub fn mark_placed(&mut self, id: NodeId, parent: Option<NodeId>, arena: &Arena) {
        if let Some(parent) = parent
            && !self.parents.contains_key(&id)
        {
            self.parents.insert(id, parent);
        }
        self.mark(id, arena);
    }

    pub fn move_space(&mut self, space: SpaceId) {
        if self.root.is_some() {
            self.moved.push(space);
        }
    }

    pub fn forget(&mut self, id: NodeId, arena: &Arena) {
        self.mark(id, arena);
        self.entries.remove(&id);
        self.parents.remove(&id);
        self.marks.remove(&id);
        self.reached.remove(&id);
        self.gathered.remove(&id);
        self.detached.remove(&id);
        if !self.spaces.is_empty() {
            for slot in 1..SPACE_SLOTS {
                self.spaces.remove(&SpaceId::inside(id, slot as u8));
            }
        }
    }

    pub fn reset(&mut self) {
        if self.root.is_some() || !self.changed.is_empty() || !self.moved.is_empty() {
            let pass = self.pass;
            *self = Self {
                pass,
                ..Self::default()
            };
        }
    }

    pub fn take_inspected(&mut self) -> bool {
        std::mem::take(&mut self.inspected)
    }

    pub fn discard_changes(&mut self) {
        self.changed.clear();
    }

    fn stale(&self, id: NodeId) -> bool {
        self.marks.get(&id) == Some(&self.pass)
    }

    fn entry(&self, key: SpaceId) -> Option<&Entry> {
        match key.slot() {
            0 => self.entries.get(&key.node()),
            _ => self.spaces.get(&key).map(|(entry, _)| entry),
        }
    }

    fn reached(&self, key: SpaceId) -> bool {
        match key.slot() {
            0 => self.reached.get(&key.node()) == Some(&self.pass),
            _ => self
                .spaces
                .get(&key)
                .is_some_and(|(_, reached)| *reached == self.pass),
        }
    }

    fn remove_entry(&mut self, key: SpaceId) -> Option<Entry> {
        match key.slot() {
            0 => self.entries.remove(&key.node()),
            _ => self.spaces.remove(&key).map(|(entry, _)| entry),
        }
    }

    fn purge(&mut self, document: &Document, dropped: Vec<SpaceId>) {
        let mut pending = dropped;
        while let Some(key) = pending.pop() {
            if self.reached(key) {
                continue;
            }
            if let Some(entry) = self.remove_entry(key) {
                pending.extend(
                    entry
                        .node
                        .children()
                        .iter()
                        .filter_map(|child| document.local_key(*child)),
                );
            }
        }
    }

    fn follow_moved_spaces(&mut self, document: &Document) {
        for space in std::mem::take(&mut self.moved) {
            let Some((entry, _)) = self.spaces.get_mut(&space) else {
                continue;
            };
            let mut node = entry.node.clone();
            if !document.place_space(&mut node, space, entry.key) {
                continue;
            }
            if node != entry.node {
                entry.node = node;
                self.changed.push(space);
            }
        }
    }
}

pub struct AccessibilityView<'a> {
    document: &'a Document,
    tree: Ref<'a, AccessibilityTree>,
    root: AccessNodeId,
}

impl AccessibilityView<'_> {
    pub fn root(&self) -> AccessNodeId {
        self.root
    }

    pub fn get(&self, id: &AccessNodeId) -> Option<&Node> {
        let local = self.document.local_key(*id)?;
        self.tree.entry(local).map(|entry| &entry.node)
    }

    pub fn focus(&self) -> Option<AccessNodeId> {
        self.tree.focus
    }

    pub fn len(&self) -> usize {
        self.tree.entries.iter().count() + self.tree.spaces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.tree
            .entries
            .iter()
            .map(|(_, entry)| &entry.node)
            .chain(self.tree.spaces.values().map(|(entry, _)| &entry.node))
    }
}

struct Pass<'a> {
    document: &'a Document,
    tree: &'a mut AccessibilityTree,
    dropped: Vec<SpaceId>,
    described: usize,
    walked: usize,
}

impl Pass<'_> {
    fn locate(&mut self, id: NodeId, placed: Placed, frame: Frame) -> Option<(Spot, Vec2)> {
        let rects = &self.document.rects;
        if placed.space == frame.walk {
            self.tree.detached.remove(&id);
            let key = Spot {
                frame: frame.space,
                at: Pos2::ZERO + frame.shift + placed.rect.min.to_vec2() * frame.scale,
                scale: frame.scale,
            };
            return Some((key, placed.rect.size()));
        }
        let key = detached_spot(rects, id, frame.space)?;
        self.tree.detached.insert(id, key);
        Some((key, placed.rect.size()))
    }

    fn visit(
        &mut self,
        id: NodeId,
        parent: Option<NodeId>,
        frame: Frame,
        force: bool,
        out: &mut Gathered,
    ) {
        let document = self.document;
        if !document.arena.contains(id) {
            return;
        }
        self.walked += 1;
        if let Some(parent) = parent {
            self.tree.parents.insert(id, parent);
        }
        let Some(placed) = document.rects.placed(&id) else {
            return;
        };
        let Some((key, size)) = self.locate(id, placed, frame) else {
            return;
        };
        let element = document.arena.get(id);
        let overlay = element.as_any().downcast_ref::<OverlayNode>();
        if overlay.is_some_and(|node| !node.is_open()) {
            return;
        }
        let layered = overlay.is_some();

        let explicit = document.accessibility.get(&id);
        let text = element
            .as_any()
            .downcast_ref::<TextNode>()
            .and_then(TextNode::accessible_text);
        let named = !document.node_test_ids(id).is_empty();
        if explicit.is_none() && text.is_none() && !named && !force && !layered {
            if !self.tree.stale(id)
                && let Some(held) = self.tree.gathered.get(&id)
                && held.key == key
            {
                let pass = self.tree.pass;
                for child in &held.children {
                    match document.local_key(*child) {
                        Some(local) if local.slot() == 0 => {
                            self.tree.reached.insert(local.node(), pass);
                        }
                        Some(local) => {
                            if let Some((_, reached)) = self.tree.spaces.get_mut(&local) {
                                *reached = pass;
                            }
                        }
                        None => {}
                    }
                }
                out.children.extend_from_slice(&held.children);
                if held.focus.is_some() {
                    out.focus = held.focus;
                }
                return;
            }
            let mut inner = Gathered::default();
            let within = Frame {
                space: key.frame,
                shift: key.at.to_vec2(),
                scale: key.scale,
                walk: Some(SpaceId::of(id)),
            };
            self.visit_children(id, within, &mut inner);
            out.children.extend_from_slice(&inner.children);
            if inner.focus.is_some() {
                out.focus = inner.focus;
            }
            self.tree.gathered.insert(
                id,
                Gathering {
                    children: inner.children,
                    focus: inner.focus,
                    key,
                },
            );
            return;
        }

        let access = document.access_node_id(id);
        self.tree.reached.insert(id, self.tree.pass);
        out.children.push(access);
        if !self.tree.stale(id)
            && let Some(entry) = self.tree.entries.get(&id)
            && entry.key == key
        {
            if entry.focus.is_some() {
                out.focus = entry.focus;
            }
            return;
        }

        let mut inner = Gathered::default();
        self.visit_children(id, Frame::within(SpaceId::of(id)), &mut inner);
        self.described += 1;
        let (mut node, focus) = document.describe(id, key, size, explicit, text, inner);
        if layered {
            node.set_class_name(LAYER_CLASS);
        }
        if focus.is_some() {
            out.focus = focus;
        }
        self.settle(SpaceId::of(id), node, focus, key);
    }

    fn visit_children(&mut self, id: NodeId, frame: Frame, out: &mut Gathered) {
        let document = self.document;
        let mut children = Vec::new();
        document
            .arena
            .get(id)
            .for_each_shown_child(&mut |child| children.push(child));
        let inside = |child: NodeId| {
            document
                .rects
                .placed(&child)
                .and_then(|placed| placed.space)
                .filter(|space| space.node() == id && space.slot() > 0)
        };
        let mut entered = [false; SPACE_SLOTS];
        for (index, child) in children.iter().enumerate() {
            let Some(space) = inside(*child) else {
                self.visit(*child, Some(id), frame, false, out);
                continue;
            };
            if std::mem::replace(&mut entered[space.slot() as usize], true) {
                continue;
            }
            let members: Vec<NodeId> = children[index..]
                .iter()
                .copied()
                .filter(|child| inside(*child) == Some(space))
                .collect();
            self.visit_space(space, &members, frame, out);
        }
    }

    fn visit_space(
        &mut self,
        space: SpaceId,
        members: &[NodeId],
        frame: Frame,
        out: &mut Gathered,
    ) {
        let document = self.document;
        let key = Spot {
            frame: frame.space,
            at: Pos2::ZERO + frame.shift,
            scale: frame.scale,
        };
        let mut inner = Gathered::default();
        for member in members {
            self.visit(
                *member,
                Some(space.node()),
                Frame::within(space),
                false,
                &mut inner,
            );
        }
        let mut node = Node::new(Role::GenericContainer);
        if !document.place_space(&mut node, space, key) {
            out.children.extend(inner.children);
            return;
        }
        node.set_children(inner.children);
        out.children.push(document.access_space_id(space));
        if inner.focus.is_some() {
            out.focus = inner.focus;
        }
        self.settle(space, node, inner.focus, key);
    }

    fn settle(&mut self, local: SpaceId, node: Node, focus: Option<AccessNodeId>, key: Spot) {
        let document = self.document;
        match self.tree.entry(local) {
            Some(old) if old.node == node => {}
            Some(old) => {
                if old.node.children() != node.children() {
                    self.dropped.extend(
                        old.node
                            .children()
                            .iter()
                            .filter_map(|child| document.local_key(*child)),
                    );
                }
                self.tree.changed.push(local);
            }
            None => self.tree.changed.push(local),
        }
        let entry = Entry { node, focus, key };
        match local.slot() {
            0 => {
                self.tree.entries.insert(local.node(), entry);
            }
            _ => {
                self.tree.spaces.insert(local, (entry, self.tree.pass));
            }
        }
    }
}

fn detached_spot(rects: &Rects, id: NodeId, frame: Option<SpaceId>) -> Option<Spot> {
    let shown = rects.get(&id)?;
    let around = rects.transform(frame);
    Some(Spot {
        frame,
        at: around.local_pos(shown.min),
        scale: rects.scale_of(&id) / around.scale,
    })
}

impl Document {
    fn refresh_accessibility(&self) -> Option<AccessNodeId> {
        let mut tree = self.accessibility_tree.borrow_mut();
        let Some(root) = self.root else {
            tree.reset();
            return None;
        };
        if tree.root != Some(root) {
            tree.reset();
            tree.root = Some(root);
        }
        for id in self.arena.changed() {
            tree.mark(*id, &self.arena);
        }
        let moved: Vec<NodeId> = tree
            .detached
            .iter()
            .filter(|(id, spot)| detached_spot(&self.rects, *id, spot.frame) != Some(**spot))
            .map(|(id, _)| id)
            .collect();
        for id in moved {
            tree.mark(id, &self.arena);
        }
        let scale = self.pixels_per_point();
        let rescaled = tree.scale.replace(scale) != Some(scale);

        let mut pass = Pass {
            document: self,
            tree: &mut tree,
            dropped: Vec::new(),
            described: 0,
            walked: 0,
        };
        let mut top = Gathered::default();
        pass.visit(root, None, Frame::SCREEN, true, &mut top);
        let (dropped, described, walked) = (pass.dropped, pass.described, pass.walked);
        self.work.note_described(described, walked);
        tree.purge(self, dropped);
        tree.follow_moved_spaces(self);
        tree.pass = tree.pass.wrapping_add(1);
        tree.focus = top.focus;
        let Some(access) = top.children.first().copied() else {
            tree.reset();
            return None;
        };
        if rescaled {
            tree.changed.push(SpaceId::of(root));
        }
        Some(access)
    }

    pub fn accessibility_update(&self, full: bool) -> Option<Fragment> {
        let root = self.refresh_accessibility()?;
        let mut tree = self.accessibility_tree.borrow_mut();
        let mut changed = std::mem::take(&mut tree.changed);
        let nodes: Vec<(AccessNodeId, Node)> =
            if full {
                tree.entries
                    .iter()
                    .map(|(id, entry)| (self.access_node_id(id), entry.node.clone()))
                    .chain(tree.spaces.iter().map(|(space, (entry, _))| {
                        (self.access_space_id(*space), entry.node.clone())
                    }))
                    .collect()
            } else {
                changed.sort_unstable_by_key(|key| {
                    (key.node().index(), key.node().generation(), key.slot())
                });
                changed.dedup();
                changed
                    .into_iter()
                    .filter_map(|key| {
                        let entry = tree.entry(key)?;
                        Some((self.access_space_id(key), entry.node.clone()))
                    })
                    .collect()
            };
        drop(tree);
        let (windows, window_roots) = match self.opens_windows() {
            true => (
                nodes
                    .iter()
                    .filter_map(|(id, _)| {
                        let window = self.window_of(self.local_key(*id)?.node());
                        (!window.is_main()).then_some((*id, window))
                    })
                    .collect(),
                self.open_windows()
                    .into_iter()
                    .filter_map(|window| {
                        let node = self.window_node(window)?;
                        Some((window, self.access_node_id(node.into())))
                    })
                    .collect(),
            ),
            false => (Vec::new(), Vec::new()),
        };
        let focus = self.accessibility_tree.borrow().focus;
        let focus_window = focus
            .and_then(|focus| self.local_key(focus))
            .map(|focus| self.window_of(focus.node()));
        Some(Fragment {
            nodes,
            root,
            focus,
            windows,
            window_roots,
            focus_window,
        })
    }

    pub fn accessibility_view(&self) -> Option<AccessibilityView<'_>> {
        let root = self.refresh_accessibility()?;
        self.accessibility_tree.borrow_mut().inspected = true;
        Some(AccessibilityView {
            document: self,
            tree: self.accessibility_tree.borrow(),
            root,
        })
    }

    fn place_space(&self, node: &mut Node, space: SpaceId, key: Spot) -> bool {
        let (Some((translation, scale)), Some(host)) =
            (self.rects.space_of(space), self.rects.placed(&space.node()))
        else {
            return false;
        };
        let offset = key.at.to_vec2() + translation * key.scale;
        node.set_transform(
            Affine::translate((offset.x as f64, offset.y as f64))
                * Affine::scale((scale * key.scale) as f64),
        );
        let size = host.rect.size();
        let seen = |at: f32, shift: f32| ((at - shift) / scale) as f64;
        node.set_bounds(AccessRect::new(
            seen(0.0, translation.x),
            seen(0.0, translation.y),
            seen(size.x, translation.x),
            seen(size.y, translation.y),
        ));
        true
    }

    fn describe(
        &self,
        id: NodeId,
        key: Spot,
        size: Vec2,
        explicit: Option<&Node>,
        text: Option<&str>,
        inner: Gathered,
    ) -> (Node, Option<AccessNodeId>) {
        let mut focus = inner.focus;
        let mut node = if let Some(node) = explicit {
            node.clone()
        } else if let Some(text) = text {
            let mut node = Node::new(Role::Label);
            node.set_value(text);
            node
        } else {
            Node::new(Role::GenericContainer)
        };
        let offset = key.at.to_vec2();
        if key.scale != 1.0 {
            node.set_transform(
                Affine::translate((offset.x as f64, offset.y as f64))
                    * Affine::scale(key.scale as f64),
            );
        } else if offset != Vec2::ZERO {
            node.set_transform(Affine::translate((offset.x as f64, offset.y as f64)));
        }
        node.set_bounds(AccessRect::new(0.0, 0.0, size.x.into(), size.y.into()));
        node.set_children(inner.children);
        let test_ids = self.node_test_ids(id);
        if !test_ids.is_empty() {
            node.set_author_id(test_ids.join(&TEST_ID_SEPARATOR.to_string()));
        }

        if explicit.is_some() {
            if node.is_disabled() {
                node.clear_actions();
            } else {
                if matches!(node.role(), Role::Slider | Role::SpinButton) {
                    node.add_action(Action::Increment);
                    node.add_action(Action::Decrement);
                    node.add_action(Action::SetValue);
                }
                if is_text_input(node.role()) {
                    node.add_action(Action::ReplaceSelectedText);
                    node.add_action(Action::SetValue);
                }
                let bridges_focus = node.supports_action(Action::Focus)
                    || node.supports_action(Action::Click)
                    || is_focusable_control(node.role());
                if bridges_focus && let Some(focusable) = self.first_focusable_within(id) {
                    node.add_action(Action::Focus);
                    if self.focusable_can_activate(focusable) {
                        node.add_action(Action::Click);
                    }
                    if self.focused == Some(focusable) {
                        focus = Some(self.access_node_id(id));
                    }
                }
            }
        } else if self.focused == Some(id) {
            node.add_action(Action::Focus);
            focus = Some(self.access_node_id(id));
        }
        (node, focus)
    }

    pub fn handle_accessibility_action(&mut self, request: ActionRequest) -> bool {
        let Some(target) = self.local_node_id(request.target_node) else {
            return false;
        };
        if !self.contains(target) {
            return true;
        }
        if self
            .accessibility
            .get(&target)
            .is_some_and(Node::is_disabled)
        {
            return true;
        }
        let focusable = self.first_focusable_within(target);
        self.set_focus_visible(true);
        match request.action {
            Action::Focus => {
                if let Some(focusable) = focusable {
                    self.update_focus(Some(focusable));
                }
            }
            Action::Blur => {
                if focusable == self.focused {
                    self.update_focus(None);
                }
            }
            Action::Click => {
                if let Some(focusable) = focusable {
                    self.update_focus(Some(focusable));
                    self.activate_focusable(focusable);
                }
            }
            Action::Increment => {
                if let Some(focusable) = focusable {
                    self.step_focusable(focusable, 1.0);
                }
            }
            Action::Decrement => {
                if let Some(focusable) = focusable {
                    self.step_focusable(focusable, -1.0);
                }
            }
            Action::ReplaceSelectedText => {
                if let (Some(focusable), Some(ActionData::Value(value))) = (focusable, request.data)
                {
                    self.update_focus(Some(focusable));
                    self.text_focused(&value);
                }
            }
            Action::SetValue => match (focusable, request.data) {
                (Some(focusable), Some(ActionData::Value(value))) => {
                    self.update_focus(Some(focusable));
                    self.key_focused(KeyPress {
                        key: Key::A,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::CTRL,
                    });
                    self.text_focused(&value);
                }
                (Some(focusable), Some(ActionData::NumericValue(value))) => {
                    let node = self.accessibility.get(&target);
                    let current = node.and_then(Node::numeric_value).unwrap_or(value);
                    let step = node
                        .and_then(Node::numeric_value_step)
                        .filter(|step| step.is_finite() && *step > 0.0)
                        .unwrap_or(FALLBACK_NUMERIC_STEP);
                    self.step_focusable(focusable, ((value - current) / step) as f32);
                }
                _ => {}
            },
            Action::ScrollUp
            | Action::ScrollDown
            | Action::ScrollLeft
            | Action::ScrollRight
            | Action::SetScrollOffset => {
                if let Some(scroll) = self.first_offset_within(target)
                    && let Some(position) = self.offset_position(scroll)
                {
                    let direction = self.arena.get_as::<OffsetNode>(scroll).direction;
                    let offset = match (request.action, request.data, direction) {
                        (Action::ScrollUp, _, Direction::Vertical)
                        | (Action::ScrollLeft, _, Direction::Horizontal) => {
                            position.offset - position.viewport
                        }
                        (Action::ScrollDown, _, Direction::Vertical)
                        | (Action::ScrollRight, _, Direction::Horizontal) => {
                            position.offset + position.viewport
                        }
                        (
                            Action::SetScrollOffset,
                            Some(ActionData::SetScrollOffset(point)),
                            Direction::Vertical,
                        ) => point.y as f32,
                        (
                            Action::SetScrollOffset,
                            Some(ActionData::SetScrollOffset(point)),
                            Direction::Horizontal,
                        ) => point.x as f32,
                        _ => position.offset,
                    };
                    self.set_offset_value(scroll, offset.clamp(0.0, position.max_offset()));
                }
            }
            _ => {}
        }
        true
    }

    fn access_node_id(&self, id: NodeId) -> AccessNodeId {
        self.access_space_id(SpaceId::of(id))
    }

    fn access_space_id(&self, key: SpaceId) -> AccessNodeId {
        let id = key.node();
        AccessNodeId(
            ((self.accessibility_id as u64) << DOCUMENT_SHIFT)
                | (u64::from(id.generation() as u8) << GENERATION_SHIFT)
                | (u64::from(key.slot()) << SLOT_SHIFT)
                | u64::from(id.index()),
        )
    }

    fn local_key(&self, id: AccessNodeId) -> Option<SpaceId> {
        if document_of(id) != self.accessibility_id as u64 {
            return None;
        }
        let local = self
            .arena
            .id_at(id.0 as u32 & INDEX_MASK)
            .filter(|local| (id.0 >> GENERATION_SHIFT) as u8 == local.generation() as u8)?;
        Some(match (id.0 >> SLOT_SHIFT) as u32 & SLOT_MASK {
            0 => SpaceId::of(local),
            slot => SpaceId::inside(local, slot as u8),
        })
    }

    pub fn local_node_id(&self, id: AccessNodeId) -> Option<NodeId> {
        self.local_key(id).map(SpaceId::node)
    }

    fn first_focusable_within(&self, id: NodeId) -> Option<NodeId> {
        let element = self.arena.get(id);
        if focus_of(element).is_some() {
            return Some(id);
        }
        element
            .children()
            .into_iter()
            .find_map(|child| self.first_focusable_within(child))
    }

    fn focusable_can_activate(&self, id: NodeId) -> bool {
        focus_of(self.arena.get(id)).is_some_and(|focus| !focus.on_activate.is_empty())
    }
}

fn is_text_input(role: Role) -> bool {
    matches!(
        role,
        Role::TextInput
            | Role::MultilineTextInput
            | Role::SearchInput
            | Role::EmailInput
            | Role::NumberInput
            | Role::PasswordInput
            | Role::PhoneNumberInput
            | Role::UrlInput
    )
}

fn is_focusable_control(role: Role) -> bool {
    matches!(
        role,
        Role::TreeItem
            | Role::ListBoxOption
            | Role::MenuItem
            | Role::CheckBox
            | Role::RadioButton
            | Role::Button
            | Role::DefaultButton
            | Role::Switch
            | Role::ComboBox
            | Role::EditableComboBox
            | Role::DisclosureTriangle
            | Role::MenuItemCheckBox
            | Role::MenuItemRadio
            | Role::Slider
            | Role::SpinButton
            | Role::Tab
            | Role::Link
            | Role::TextInput
            | Role::MultilineTextInput
            | Role::SearchInput
            | Role::EmailInput
            | Role::NumberInput
            | Role::PasswordInput
            | Role::PhoneNumberInput
            | Role::UrlInput
    )
}

use std::cell::Ref;
use std::sync::atomic::{AtomicU32, Ordering};

use accesskit::{
    Action, ActionData, ActionRequest, Affine, Node, NodeId as AccessNodeId, Rect as AccessRect,
    Role, Tree, TreeId, TreeUpdate,
};

use crate::Document;
use crate::base::focusable::FocusableNode;
use crate::base::frame::FrameNode;
use crate::base::overlay::OverlayNode;
use crate::base::text::TextNode;
use crate::geometry::{Rect, Vec2};
use crate::input::{Key, KeyPress, Modifiers};
use crate::node::{Arena, NodeId, NodeMap};

pub(crate) const WINDOW_NODE: AccessNodeId = AccessNodeId(0);
const DOCUMENT_SHIFT: u32 = 32;
const FALLBACK_NUMERIC_STEP: f64 = 0.05;
static NEXT_DOCUMENT_ID: AtomicU32 = AtomicU32::new(1);

#[derive(Clone)]
pub(crate) struct Fragment {
    pub(crate) nodes: Vec<(AccessNodeId, Node)>,
    pub(crate) root: AccessNodeId,
    pub(crate) focus: Option<AccessNodeId>,
}

impl Fragment {
    pub(crate) fn scale(&mut self, scale: f32) {
        let root = self.root;
        for (id, node) in &mut self.nodes {
            if *id == root {
                node.set_transform(Affine::scale(scale.into()));
            }
        }
    }
}

pub(crate) fn next_document_id() -> u32 {
    NEXT_DOCUMENT_ID.fetch_add(1, Ordering::Relaxed)
}

pub(crate) fn tree_update(
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

#[derive(Default)]
pub(crate) struct AccessibilityTree {
    root: Option<NodeId>,
    entries: NodeMap<Entry>,
    parents: NodeMap<NodeId>,
    marks: NodeMap<u64>,
    reached: NodeMap<u64>,
    pass: u64,
    focus: Option<AccessNodeId>,
    changed: Vec<NodeId>,
    inspected: bool,
}

struct Entry {
    node: Node,
    focus: Option<AccessNodeId>,
}

#[derive(Default)]
struct Gathered {
    children: Vec<AccessNodeId>,
    focus: Option<AccessNodeId>,
}

impl AccessibilityTree {
    pub(crate) fn mark(&mut self, id: NodeId, arena: &Arena) {
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

    pub(crate) fn forget(&mut self, id: NodeId, arena: &Arena) {
        self.mark(id, arena);
        self.entries.remove(&id);
        self.parents.remove(&id);
        self.marks.remove(&id);
        self.reached.remove(&id);
    }

    pub(crate) fn reset(&mut self) {
        if self.root.is_some() || !self.changed.is_empty() {
            let pass = self.pass;
            *self = Self {
                pass,
                ..Self::default()
            };
        }
    }

    pub(crate) fn take_inspected(&mut self) -> bool {
        std::mem::take(&mut self.inspected)
    }

    pub(crate) fn discard_changes(&mut self) {
        self.changed.clear();
    }

    fn stale(&self, id: NodeId) -> bool {
        self.marks.get(&id) == Some(&self.pass)
    }

    fn purge(&mut self, document: &Document, dropped: Vec<NodeId>) {
        let mut pending = dropped;
        while let Some(id) = pending.pop() {
            if self.reached.get(&id) == Some(&self.pass) {
                continue;
            }
            if let Some(entry) = self.entries.remove(&id) {
                pending.extend(
                    entry
                        .node
                        .children()
                        .iter()
                        .filter_map(|child| document.local_node_id(*child)),
                );
            }
        }
    }
}

pub(crate) struct AccessibilityView<'a> {
    document: &'a Document,
    tree: Ref<'a, AccessibilityTree>,
    root: AccessNodeId,
}

impl AccessibilityView<'_> {
    pub(crate) fn root(&self) -> AccessNodeId {
        self.root
    }

    pub(crate) fn get(&self, id: &AccessNodeId) -> Option<&Node> {
        let local = self.document.local_node_id(*id)?;
        self.tree.entries.get(&local).map(|entry| &entry.node)
    }

    pub(crate) fn len(&self) -> usize {
        self.tree.entries.iter().count()
    }
}

struct Pass<'a> {
    document: &'a Document,
    tree: &'a mut AccessibilityTree,
    dropped: Vec<NodeId>,
    described: usize,
}

impl Pass<'_> {
    fn visit(&mut self, id: NodeId, parent: Option<NodeId>, force: bool, out: &mut Gathered) {
        let document = self.document;
        if !document.arena.contains(id) {
            return;
        }
        if let Some(parent) = parent {
            self.tree.parents.insert(id, parent);
        }
        let Some(rect) = document.rects.get(&id).copied() else {
            return;
        };
        let element = document.arena.get(id);
        if element
            .as_any()
            .downcast_ref::<FrameNode>()
            .is_some_and(|node| !node.visible)
            || element
                .as_any()
                .downcast_ref::<OverlayNode>()
                .is_some_and(|node| !node.is_open())
        {
            return;
        }

        let explicit = document.accessibility.get(&id);
        let text = element
            .as_any()
            .downcast_ref::<TextNode>()
            .and_then(TextNode::accessible_text);
        if explicit.is_none() && text.is_none() && !force {
            for child in element.children() {
                self.visit(child, Some(id), false, out);
            }
            return;
        }

        let access = document.access_node_id(id);
        self.tree.reached.insert(id, self.tree.pass);
        out.children.push(access);
        if !self.tree.stale(id)
            && let Some(entry) = self.tree.entries.get(&id)
        {
            if entry.focus.is_some() {
                out.focus = entry.focus;
            }
            return;
        }

        let mut inner = Gathered::default();
        for child in element.children() {
            self.visit(child, Some(id), false, &mut inner);
        }
        self.described += 1;
        let (node, focus) = document.describe(id, rect, explicit, text, inner);
        if focus.is_some() {
            out.focus = focus;
        }
        match self.tree.entries.get(&id) {
            Some(old) if old.node == node => {}
            Some(old) => {
                if old.node.children() != node.children() {
                    self.dropped.extend(
                        old.node
                            .children()
                            .iter()
                            .filter_map(|child| document.local_node_id(*child)),
                    );
                }
                self.tree.changed.push(id);
            }
            None => self.tree.changed.push(id),
        }
        self.tree.entries.insert(id, Entry { node, focus });
    }
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
        for id in self.arena.changed_since(0) {
            tree.mark(*id, &self.arena);
        }

        let mut pass = Pass {
            document: self,
            tree: &mut tree,
            dropped: Vec::new(),
            described: 0,
        };
        let mut top = Gathered::default();
        pass.visit(root, None, true, &mut top);
        let (dropped, described) = (pass.dropped, pass.described);
        self.work.note_described(described);
        tree.purge(self, dropped);
        tree.pass = tree.pass.wrapping_add(1);
        tree.focus = top.focus;
        let Some(access) = top.children.first().copied() else {
            tree.reset();
            return None;
        };
        Some(access)
    }

    pub(crate) fn accessibility_update(&self, full: bool) -> Option<Fragment> {
        let root = self.refresh_accessibility()?;
        let mut tree = self.accessibility_tree.borrow_mut();
        let mut changed = std::mem::take(&mut tree.changed);
        let nodes = if full {
            tree.entries
                .iter()
                .map(|(id, entry)| (self.access_node_id(id), entry.node.clone()))
                .collect()
        } else {
            changed.sort_unstable_by_key(|id| id.index());
            changed.dedup();
            changed
                .into_iter()
                .filter_map(|id| {
                    let entry = tree.entries.get(&id)?;
                    Some((self.access_node_id(id), entry.node.clone()))
                })
                .collect()
        };
        Some(Fragment {
            nodes,
            root,
            focus: tree.focus,
        })
    }

    pub(crate) fn accessibility_view(&self) -> Option<AccessibilityView<'_>> {
        let root = self.refresh_accessibility()?;
        self.accessibility_tree.borrow_mut().inspected = true;
        Some(AccessibilityView {
            document: self,
            tree: self.accessibility_tree.borrow(),
            root,
        })
    }

    fn describe(
        &self,
        id: NodeId,
        rect: Rect,
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
        node.set_bounds(access_rect(rect));
        node.set_children(inner.children);

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

    pub(crate) fn handle_accessibility_action(&mut self, request: ActionRequest) -> bool {
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
            Action::ScrollUp | Action::ScrollDown | Action::SetScrollOffset => {
                if let Some(scroll) = self.first_offset_within(target)
                    && let Some(position) = self.offset_position(scroll)
                {
                    let offset = match (request.action, request.data) {
                        (Action::ScrollUp, _) => position.offset - position.viewport,
                        (Action::ScrollDown, _) => position.offset + position.viewport,
                        (Action::SetScrollOffset, Some(ActionData::SetScrollOffset(point))) => {
                            point.y as f32
                        }
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
        AccessNodeId(((self.accessibility_id as u64) << DOCUMENT_SHIFT) | id.index() as u64)
    }

    pub(crate) fn local_node_id(&self, id: AccessNodeId) -> Option<NodeId> {
        ((id.0 >> DOCUMENT_SHIFT) == self.accessibility_id as u64)
            .then(|| NodeId::from_index(id.0 as u32))
    }

    fn first_focusable_within(&self, id: NodeId) -> Option<NodeId> {
        let element = self.arena.get(id);
        if element.as_any().is::<FocusableNode>() {
            return Some(id);
        }
        element
            .children()
            .into_iter()
            .find_map(|child| self.first_focusable_within(child))
    }

    fn focusable_can_activate(&self, id: NodeId) -> bool {
        self.arena
            .get(id)
            .as_any()
            .downcast_ref::<FocusableNode>()
            .is_some_and(|node| !node.on_activate.is_empty())
    }
}

fn access_rect(rect: Rect) -> AccessRect {
    AccessRect::new(
        rect.left().into(),
        rect.top().into(),
        rect.right().into(),
        rect.bottom().into(),
    )
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

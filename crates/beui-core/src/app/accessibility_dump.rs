use stable_hash::{HashMap, HashSet};
use std::cell::{OnceCell, RefCell};
use std::fmt::Write as _;
use std::rc::Rc;

use accesskit::{Affine, Node, NodeId, Rect, Role, TreeUpdate};

pub const ACTIONS: [accesskit::Action; 16] = [
    accesskit::Action::Click,
    accesskit::Action::Focus,
    accesskit::Action::Blur,
    accesskit::Action::Expand,
    accesskit::Action::Collapse,
    accesskit::Action::Increment,
    accesskit::Action::Decrement,
    accesskit::Action::SetValue,
    accesskit::Action::ReplaceSelectedText,
    accesskit::Action::ScrollUp,
    accesskit::Action::ScrollDown,
    accesskit::Action::ScrollLeft,
    accesskit::Action::ScrollRight,
    accesskit::Action::ScrollIntoView,
    accesskit::Action::ShowContextMenu,
    accesskit::Action::ShowTooltip,
];

pub fn action_named(name: &str) -> Option<accesskit::Action> {
    ACTIONS
        .into_iter()
        .find(|action| format!("{action:?}") == name)
}

#[derive(Default, Clone)]
pub struct AccessibilityDump {
    held: Rc<Held>,
}

#[derive(Default)]
struct Held {
    tree: RefCell<Nodes>,
    rendered: RefCell<Option<Rc<Rendered>>>,
}

#[derive(Default)]
struct Nodes {
    nodes: HashMap<NodeId, Node>,
    root: Option<NodeId>,
    focus: Option<NodeId>,
    kept: usize,
    described: HashMap<NodeId, Described>,
}

struct Described {
    placed: Placed,
    line: Option<Rc<Line>>,
}

#[derive(Clone, Copy, PartialEq)]
struct Placed {
    transform: Affine,
    clip: Rect,
    depth: usize,
    focused: bool,
}

#[derive(Default)]
pub struct Rendered {
    lines: Vec<Rc<Line>>,
    scale: f32,
    written: OnceCell<String>,
    test_ids: OnceCell<Rc<TestIds>>,
}

impl Rendered {
    pub fn text(&self) -> &str {
        self.written.get_or_init(|| {
            let mut text = String::new();
            for line in &self.lines {
                for _ in 0..line.depth {
                    text.push_str("  ");
                }
                text.push_str(&line.text);
                text.push('\n');
            }
            text
        })
    }

    pub fn lines(&self) -> &[Rc<Line>] {
        &self.lines
    }

    pub fn test_ids(&self) -> &Rc<TestIds> {
        self.test_ids.get_or_init(|| {
            let mut named: HashMap<String, Vec<TestIdAt>> = HashMap::default();
            for line in &self.lines {
                let Some(bounds) = line.bounds else {
                    continue;
                };
                for test_id in &line.test_ids {
                    named.entry(test_id.clone()).or_default().push(TestIdAt {
                        rect: bounds.scaled(self.scale.recip()),
                        visible: line.visible,
                    });
                }
            }
            Rc::new(TestIds { named })
        })
    }
}

#[derive(Default)]
pub struct TestIds {
    named: HashMap<String, Vec<TestIdAt>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TestIdAt {
    pub rect: crate::geometry::Rect,
    pub visible: bool,
}

impl TestIds {
    pub fn rect(&self, test_id: &str) -> Option<crate::geometry::Rect> {
        match self.named.get(test_id).map(Vec::as_slice) {
            None | Some([]) => None,
            Some([shown]) => Some(shown.rect),
            Some(shown) => panic!(
                "test id {test_id:?} names {} nodes on screen; give each its own id",
                shown.len()
            ),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &[TestIdAt])> {
        self.named
            .iter()
            .map(|(test_id, shown)| (test_id.as_str(), shown.as_slice()))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub id: NodeId,
    pub actions: Vec<accesskit::Action>,
    pub depth: usize,
    pub role: Role,
    pub label: String,
    pub value: String,
    pub toggled: Option<accesskit::Toggled>,
    pub disabled: bool,
    pub focused: bool,
    pub text: String,
    pub bounds: Option<crate::geometry::Rect>,
    pub visible: bool,
    pub shown: Option<crate::geometry::Rect>,
    pub test_ids: Vec<String>,
}

impl beui_types::target::TreeLine for Line {
    fn depth(&self) -> usize {
        self.depth
    }
    fn text(&self) -> &str {
        &self.text
    }
    fn test_ids(&self) -> &[String] {
        &self.test_ids
    }
    fn bounds(&self) -> Option<crate::geometry::Rect> {
        self.bounds
    }
    fn visible(&self) -> bool {
        self.visible
    }
    fn shown(&self) -> Option<crate::geometry::Rect> {
        self.shown
    }
}

impl AccessibilityDump {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, update: TreeUpdate) {
        let mut tree = self.held.tree.borrow_mut();
        tree.check_attached(&update);
        let root = update.tree.map(|tree| tree.root).or(tree.root);
        let mut changed = root != tree.root || tree.focus != Some(update.focus);
        tree.root = root;
        tree.focus = Some(update.focus);
        for (id, node) in update.nodes {
            if tree.nodes.get(&id) == Some(&node) {
                continue;
            }
            changed = true;
            tree.described.remove(&id);
            tree.nodes.insert(id, node);
        }
        if !changed {
            return;
        }
        *self.held.rendered.borrow_mut() = None;
        if tree.nodes.len() > 2 * tree.kept + 1024 {
            tree.prune();
        }
    }

    pub fn rendered(&self) -> Rc<Rendered> {
        if let Some(rendered) = &*self.held.rendered.borrow() {
            return Rc::clone(rendered);
        }
        let rendered = Rc::new(self.held.tree.borrow_mut().render());
        *self.held.rendered.borrow_mut() = Some(Rc::clone(&rendered));
        rendered
    }
}

impl Nodes {
    fn check_attached(&self, update: &TreeUpdate) {
        let claimed: HashSet<NodeId> = update
            .nodes
            .iter()
            .flat_map(|(_, node)| node.children().iter().copied())
            .collect();
        let root = update.tree.as_ref().map(|tree| tree.root);
        let unclaimed: Vec<NodeId> = update
            .nodes
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| Some(*id) != root && !claimed.contains(id))
            .collect();
        if unclaimed.is_empty() {
            return;
        }
        let mut reached = HashSet::default();
        if let Some(root) = self.root {
            self.reach(root, &mut reached);
        }
        let detached: Vec<NodeId> = unclaimed
            .into_iter()
            .filter(|id| !reached.contains(id))
            .collect();
        assert!(
            detached.is_empty(),
            "the update includes {} nodes which are neither in the current tree nor a child of another node from the update, which accesskit rejects: {detached:?}",
            detached.len()
        );
    }

    fn reach(&self, id: NodeId, reached: &mut HashSet<NodeId>) {
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            if let Some(node) = self.nodes.get(&id)
                && reached.insert(id)
            {
                pending.extend(node.children().iter().copied());
            }
        }
    }

    fn prune(&mut self) {
        let mut reached = HashSet::default();
        if let Some(root) = self.root {
            self.reach(root, &mut reached);
        }
        self.nodes.retain(|id, _| reached.contains(id));
        self.described.retain(|id, _| reached.contains(id));
        self.kept = self.nodes.len();
    }

    fn render(&mut self) -> Rendered {
        let Some(root) = self.root else {
            return Rendered::default();
        };
        let mut walk = Walk {
            nodes: &self.nodes,
            focus: self.focus,
            described: &mut self.described,
            reached: HashSet::default(),
            lines: Vec::new(),
        };
        let everywhere = Rect::new(f64::MIN, f64::MIN, f64::MAX, f64::MAX);
        walk.node(root, Affine::IDENTITY, everywhere, 0);
        let Walk { reached, lines, .. } = walk;
        self.nodes.retain(|id, _| reached.contains(id));
        self.described.retain(|id, _| reached.contains(id));
        self.kept = self.nodes.len();
        let scale = self
            .nodes
            .get(&root)
            .and_then(Node::transform)
            .map_or(1.0, |transform| transform.as_coeffs()[0] as f32);
        Rendered {
            lines,
            scale,
            ..Rendered::default()
        }
    }
}

struct Walk<'a> {
    nodes: &'a HashMap<NodeId, Node>,
    focus: Option<NodeId>,
    described: &'a mut HashMap<NodeId, Described>,
    reached: HashSet<NodeId>,
    lines: Vec<Rc<Line>>,
}

impl Walk<'_> {
    fn node(&mut self, id: NodeId, parent: Affine, clip: Rect, depth: usize) {
        if !self.reached.insert(id) {
            return;
        }
        let Some(node) = self.nodes.get(&id) else {
            return;
        };
        let transform = parent * node.transform().copied().unwrap_or(Affine::IDENTITY);
        let rect = node
            .bounds()
            .map(|bounds| transform.transform_rect_bbox(bounds));
        let placed = Placed {
            transform,
            clip,
            depth,
            focused: self.focus == Some(id),
        };
        let line = match self.described.get(&id) {
            Some(described) if described.placed == placed => described.line.clone(),
            _ => {
                let line = describe(id, node, rect, placed).map(Rc::new);
                self.described.insert(
                    id,
                    Described {
                        placed,
                        line: line.clone(),
                    },
                );
                line
            }
        };
        let shown = line.is_some();
        if let Some(line) = line {
            self.lines.push(line);
        }
        let depth = depth + usize::from(shown);
        let clip = match rect {
            _ if node.class_name() == Some(crate::accessibility::LAYER_CLASS) => {
                Rect::new(f64::MIN, f64::MIN, f64::MAX, f64::MAX)
            }
            Some(rect) if matches!(node.role(), Role::Window | Role::ScrollView | Role::Pane) => {
                clip.intersect(rect)
            }
            _ => clip,
        };
        for child in node.children() {
            self.node(*child, transform, clip, depth);
        }
    }
}

fn geometry_rect(rect: Rect) -> crate::geometry::Rect {
    crate::geometry::Rect::from_min_max(
        crate::geometry::pos2(rect.x0 as f32, rect.y0 as f32),
        crate::geometry::pos2(rect.x1 as f32, rect.y1 as f32),
    )
}

fn describe(id: NodeId, node: &Node, rect: Option<Rect>, placed: Placed) -> Option<Line> {
    let label = node.label().filter(|label| !label.is_empty());
    let numeric = node.numeric_value().map(|value| {
        let value = (value * 1000.0).round() / 1000.0;
        value.to_string()
    });
    let value = node
        .value()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or(numeric);
    let value = value.as_deref();
    let test_ids: Vec<String> = node
        .author_id()
        .map(|ids| {
            ids.split(crate::accessibility::TEST_ID_SEPARATOR)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let shown = node.role() != Role::GenericContainer
        || label.is_some()
        || value.is_some()
        || !test_ids.is_empty();
    if !shown {
        return None;
    }
    let mut text = String::new();
    let mut visible_part = true;
    let mut shown = rect;
    let _ = write!(text, "{:?}", node.role());
    if let Some(label) = label {
        let _ = write!(text, " {label:?}");
    }
    if let Some(value) = value {
        let _ = write!(text, " value={value:?}");
    }
    if let Some(toggled) = node.toggled() {
        let _ = write!(text, " toggled={toggled:?}");
    }
    if node.is_disabled() {
        text.push_str(" disabled");
    }
    if node.is_selected() == Some(true) {
        text.push_str(" selected");
    }
    if placed.focused {
        text.push_str(" focused");
    }
    for test_id in &test_ids {
        let _ = write!(text, " #{test_id}");
    }
    if let Some(rect) = rect {
        let _ = write!(
            text,
            " at {},{} size {}x{}",
            rect.x0.round(),
            rect.y0.round(),
            rect.width().round(),
            rect.height().round()
        );
        let part = placed.clip.intersect(rect);
        let visible = part.area();
        if visible <= 0.0 && rect.area() > 0.0 {
            text.push_str(" offscreen");
            visible_part = false;
            shown = None;
        } else if visible < rect.area() {
            text.push_str(" partly offscreen");
            shown = Some(part);
        }
    }
    Some(Line {
        id,
        actions: ACTIONS
            .into_iter()
            .filter(|action| node.supports_action(*action))
            .collect(),
        depth: placed.depth,
        role: node.role(),
        label: label.unwrap_or_default().to_owned(),
        value: value.unwrap_or_default().to_owned(),
        toggled: node.toggled(),
        disabled: node.is_disabled(),
        focused: placed.focused,
        text,
        bounds: rect.map(geometry_rect),
        visible: visible_part,
        shown: shown.map(geometry_rect),
        test_ids,
    })
}

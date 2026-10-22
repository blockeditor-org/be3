use foldhash::{HashMap, HashSet};
use std::cell::RefCell;
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
}

#[derive(Default)]
pub struct Rendered {
    written: String,
    lines: Vec<Line>,
    test_ids: Rc<TestIds>,
}

impl Rendered {
    pub fn text(&self) -> &str {
        &self.written
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub fn test_ids(&self) -> &Rc<TestIds> {
        &self.test_ids
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
    pub test_ids: Vec<String>,
}

impl AccessibilityDump {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, update: TreeUpdate) {
        let mut tree = self.held.tree.borrow_mut();
        let root = update.tree.map(|tree| tree.root).or(tree.root);
        let changed = root != tree.root
            || tree.focus != Some(update.focus)
            || update
                .nodes
                .iter()
                .any(|(id, node)| tree.nodes.get(id) != Some(node));
        tree.root = root;
        tree.focus = Some(update.focus);
        tree.nodes.extend(update.nodes);
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
        self.kept = self.nodes.len();
    }

    fn render(&mut self) -> Rendered {
        let Some(root) = self.root else {
            return Rendered::default();
        };
        let mut reached = HashSet::default();
        let mut text = String::new();
        let mut lines = Vec::new();
        let everywhere = Rect::new(f64::MIN, f64::MIN, f64::MAX, f64::MAX);
        self.render_node(
            root,
            Affine::IDENTITY,
            everywhere,
            0,
            &mut reached,
            &mut text,
            &mut lines,
        );
        self.nodes.retain(|id, _| reached.contains(id));
        self.kept = self.nodes.len();
        let scale = self
            .nodes
            .get(&root)
            .and_then(Node::transform)
            .map_or(1.0, |transform| transform.as_coeffs()[0] as f32);
        let mut named: HashMap<String, Vec<TestIdAt>> = HashMap::default();
        for line in &lines {
            let Some(bounds) = line.bounds else {
                continue;
            };
            for test_id in &line.test_ids {
                named.entry(test_id.clone()).or_default().push(TestIdAt {
                    rect: bounds.scaled(scale.recip()),
                    visible: line.visible,
                });
            }
        }
        Rendered {
            written: text,
            lines,
            test_ids: Rc::new(TestIds { named }),
        }
    }

    fn render_node(
        &self,
        id: NodeId,
        parent: Affine,
        clip: Rect,
        depth: usize,
        reached: &mut HashSet<NodeId>,
        text: &mut String,
        lines: &mut Vec<Line>,
    ) {
        if !reached.insert(id) {
            return;
        }
        let Some(node) = self.nodes.get(&id) else {
            return;
        };
        let transform = parent * node.transform().copied().unwrap_or(Affine::IDENTITY);
        let rect = node
            .bounds()
            .map(|bounds| transform.transform_rect_bbox(bounds));
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
            .map(|ids| ids.split(' ').map(str::to_owned).collect())
            .unwrap_or_default();
        let shown = node.role() != Role::GenericContainer
            || label.is_some()
            || value.is_some()
            || !test_ids.is_empty();
        if shown {
            let start = text.len();
            let mut visible_part = true;
            let _ = write!(text, "{}{:?}", "  ".repeat(depth), node.role());
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
            if self.focus == Some(id) {
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
                let visible = clip.intersect(rect).area();
                if visible <= 0.0 && rect.area() > 0.0 {
                    text.push_str(" offscreen");
                    visible_part = false;
                } else if visible < rect.area() {
                    text.push_str(" partly offscreen");
                }
            }
            lines.push(Line {
                id,
                actions: ACTIONS
                    .into_iter()
                    .filter(|action| node.supports_action(*action))
                    .collect(),
                depth,
                role: node.role(),
                label: label.unwrap_or_default().to_owned(),
                value: value.unwrap_or_default().to_owned(),
                toggled: node.toggled(),
                disabled: node.is_disabled(),
                focused: self.focus == Some(id),
                text: text[start..].trim_start().to_owned(),
                bounds: rect.map(|rect| {
                    crate::geometry::Rect::from_min_max(
                        crate::geometry::pos2(rect.x0 as f32, rect.y0 as f32),
                        crate::geometry::pos2(rect.x1 as f32, rect.y1 as f32),
                    )
                }),
                visible: visible_part,
                test_ids,
            });
            text.push('\n');
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
            self.render_node(*child, transform, clip, depth, reached, text, lines);
        }
    }
}

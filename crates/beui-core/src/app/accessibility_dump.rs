use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

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

#[derive(Default)]
pub struct AccessibilityDump {
    nodes: HashMap<NodeId, Node>,
    root: Option<NodeId>,
    focus: Option<NodeId>,
    written: String,
    lines: Vec<Line>,
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
}

impl AccessibilityDump {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, update: TreeUpdate) {
        if let Some(tree) = update.tree {
            self.root = Some(tree.root);
        }
        self.focus = Some(update.focus);
        self.nodes.extend(update.nodes);
        let Some(root) = self.root else {
            return;
        };
        let mut reached = HashSet::new();
        let mut text = String::new();
        let mut lines = Vec::new();
        let everywhere = Rect::new(f64::MIN, f64::MIN, f64::MAX, f64::MAX);
        self.render(
            root,
            Affine::IDENTITY,
            everywhere,
            0,
            &mut reached,
            &mut text,
            &mut lines,
        );
        self.nodes.retain(|id, _| reached.contains(id));
        self.lines = lines;
        self.written = text;
    }

    pub fn text(&self) -> &str {
        &self.written
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    fn render(
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
        let shown = node.role() != Role::GenericContainer || label.is_some() || value.is_some();
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
            if self.focus == Some(id) {
                text.push_str(" focused");
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
            });
            text.push('\n');
        }
        let depth = depth + usize::from(shown);
        let clip = match rect {
            Some(rect) if matches!(node.role(), Role::Window | Role::ScrollView | Role::Pane) => {
                clip.intersect(rect)
            }
            _ => clip,
        };
        for child in node.children() {
            self.render(*child, transform, clip, depth, reached, text, lines);
        }
    }
}

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use accesskit::{Affine, Node, NodeId, Rect, Role, TreeUpdate};

pub(crate) struct AccessibilityDump {
    path: PathBuf,
    nodes: HashMap<NodeId, Node>,
    root: Option<NodeId>,
    focus: Option<NodeId>,
    written: String,
}

impl AccessibilityDump {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            nodes: HashMap::new(),
            root: None,
            focus: None,
            written: String::new(),
        }
    }

    pub(crate) fn update(&mut self, update: TreeUpdate) {
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
        let everywhere = Rect::new(f64::MIN, f64::MIN, f64::MAX, f64::MAX);
        self.render(
            root,
            Affine::IDENTITY,
            everywhere,
            0,
            &mut reached,
            &mut text,
        );
        self.nodes.retain(|id, _| reached.contains(id));
        if text == self.written {
            return;
        }
        let staged = self.path.with_extension("tmp");
        let written =
            std::fs::write(&staged, &text).and_then(|()| std::fs::rename(&staged, &self.path));
        if let Err(error) = written {
            eprintln!(
                "could not write the accessibility tree to {}: {error}",
                self.path.display()
            );
        }
        self.written = text;
    }

    fn render(
        &self,
        id: NodeId,
        parent: Affine,
        clip: Rect,
        depth: usize,
        reached: &mut HashSet<NodeId>,
        text: &mut String,
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
        let value = node.value().filter(|value| !value.is_empty());
        let shown = node.role() != Role::GenericContainer || label.is_some() || value.is_some();
        if shown {
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
                } else if visible < rect.area() {
                    text.push_str(" partly offscreen");
                }
            }
            text.push('\n');
        }
        let depth = depth + usize::from(shown);
        let clip = match rect {
            Some(rect) if matches!(node.role(), Role::Window | Role::ScrollView) => {
                clip.intersect(rect)
            }
            _ => clip,
        };
        for child in node.children() {
            self.render(*child, transform, clip, depth, reached, text);
        }
    }
}

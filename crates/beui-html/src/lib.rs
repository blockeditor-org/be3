pub use beui_tree::reactive;
pub use reactive::*;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

#[derive(Clone)]
pub struct Node(Rc<NodeData>);

struct NodeData {
    kind: Kind,
    attrs: RefCell<Vec<(String, Option<String>)>>,
    children: RefCell<ChildList<Node>>,
}

enum Kind {
    Element(&'static str),
    Text(String),
    Raw(String),
    Fragment,
}

impl Node {
    fn new(kind: Kind) -> Self {
        Self(Rc::new(NodeData {
            kind,
            attrs: RefCell::default(),
            children: RefCell::default(),
        }))
    }

    fn is_void(&self) -> bool {
        let Kind::Element(tag) = &self.0.kind else {
            return false;
        };
        [
            "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
            "source", "track", "wbr",
        ]
        .iter()
        .any(|void| tag.eq_ignore_ascii_case(void))
    }

    pub fn to_html(&self) -> String {
        let mut output = String::new();
        self.write(&mut output);
        output
    }

    fn write(&self, output: &mut String) {
        match &self.0.kind {
            Kind::Text(value) => escape_into(value, output),
            Kind::Raw(value) => output.push_str(value),
            Kind::Element(tag) => {
                output.push('<');
                output.push_str(tag);
                for (name, value) in self.0.attrs.borrow().iter() {
                    if let Some(value) = value {
                        output.push(' ');
                        output.push_str(name);
                        output.push_str("=\"");
                        escape_into(value, output);
                        output.push('"');
                    }
                }
                output.push('>');
                if !self.is_void() {
                    for child in self.0.children.borrow().iter() {
                        child.write(output);
                    }
                    output.push_str("</");
                    output.push_str(tag);
                    output.push('>');
                }
            }
            Kind::Fragment => {
                for child in self.0.children.borrow().iter() {
                    child.write(output);
                }
            }
        }
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b':'))
}

fn set_attr(node: &Node, name: &str, value: Option<String>) {
    assert!(
        matches!(node.0.kind, Kind::Element(_)),
        "attributes require an element"
    );
    assert!(valid_name(name), "invalid HTML attribute name");
    let mut attrs = node.0.attrs.borrow_mut();
    if let Some(attr) = attrs.iter_mut().find(|attr| attr.0 == name) {
        attr.1 = value;
    } else {
        attrs.push((name.into(), value));
    }
}

fn escape_into(value: &str, output: &mut String) {
    for c in value.chars() {
        output.push_str(match c {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '"' => "&quot;",
            '\'' => "&#39;",
            _ => {
                output.push(c);
                continue;
            }
        });
    }
}

fn text(value: impl ToString) -> Node {
    Node::new(Kind::Text(value.to_string()))
}

pub fn render(build: impl FnOnce() -> Node) -> String {
    format!("<!doctype html>{}", render_fragment(build))
}

pub fn render_fragment(build: impl FnOnce() -> Node) -> String {
    let _root = RootScope::new();
    let scope = Scope::new();
    let node = scope.run(build);
    node.to_html()
}

impl ChildValue for Node {}
beui_tree::child_type!(Node);
impl SlotChild for Node {
    type Stored = Node;
    fn store(self) -> Node {
        self
    }
}
impl IntoChild<Node> for String {
    fn into_child(self) -> Node {
        text(self)
    }
}
impl IntoChild<Node> for &str {
    fn into_child(self) -> Node {
        text(self)
    }
}
impl IntoSegment<Node> for String {
    fn into_segment(self) -> ChildSegment<Node> {
        ChildSegment::One(text(self))
    }
}
impl IntoSegment<Node> for &str {
    fn into_segment(self) -> ChildSegment<Node> {
        ChildSegment::One(text(self))
    }
}

struct Host(Weak<NodeData>);
impl SlotHost<Node> for Host {
    fn append(&self, node: Node) {
        if let Some(data) = self.0.upgrade() {
            assert!(
                !Node(data.clone()).is_void(),
                "void HTML elements cannot have children"
            );
            data.children.borrow_mut().push(node);
        }
    }
    fn open(&self) -> SlotId {
        let data = self.0.upgrade().expect("element alive during mount");
        assert!(
            !Node(data.clone()).is_void(),
            "void HTML elements cannot have children"
        );
        data.children.borrow_mut().open()
    }
    fn fill(&self, slot: SlotId, items: Vec<Node>) {
        if let Some(data) = self.0.upgrade() {
            data.children.borrow_mut().fill(slot, items);
        }
    }
}
impl IntoSlotHost<Node> for &Node {
    fn into_slot_host(self) -> Rc<dyn SlotHost<Node>> {
        Rc::new(Host(Rc::downgrade(&self.0)))
    }
}

#[component]
pub fn Element(tag: &'static str, children: Children<Node>) -> Node {
    assert!(valid_name(tag), "invalid HTML element name");
    let node = Node::new(Kind::Element(tag));
    children.mount(&node);
    node
}

#[component]
pub fn Text(value: Prop<String>) -> Node {
    let node = Node::new(Kind::Fragment);
    let weak = Rc::downgrade(&node.0);
    create_effect(move || {
        let value = value.get();
        if let Some(data) = weak.upgrade() {
            let mut children = data.children.borrow_mut();
            *children = ChildList::default();
            children.push(text(value));
        }
    });
    node
}

pub fn attr(node: Node, (name, value): (&'static str, impl IntoProp<String>)) -> Node {
    let value = value.into_prop();
    let weak = Rc::downgrade(&node.0);
    create_effect(move || {
        let value = value.get();
        if let Some(data) = weak.upgrade() {
            set_attr(&Node(data), name, Some(value));
        }
    });
    node
}

#[cfg(test)]
mod tests;

#[component]
pub fn Fragment(#[prop(default = Vec::new())] nodes: Vec<Node>, children: Children<Node>) -> Node {
    let node = Node::new(Kind::Fragment);
    for child in nodes {
        node.0.children.borrow_mut().push(child);
    }
    children.mount(&node);
    node
}

#[component]
pub fn Stylesheet(css: String) -> Node {
    assert!(
        !css.to_ascii_lowercase().contains("</style"),
        "CSS contains a closing style tag"
    );
    view! {
        <Element tag="style">{Node::new(Kind::Raw(css))}</Element>
    }
}

#[component]
pub fn Script(kind: &'static str, source: String) -> Node {
    assert!(
        !source.to_ascii_lowercase().contains("</script"),
        "script contains a closing script tag"
    );
    view! {
        <Element tag="script" @attr={("type", kind.to_string())}>
            {Node::new(Kind::Raw(source))}
        </Element>
    }
}

pub fn flag(node: Node, (name, value): (&'static str, impl IntoProp<bool>)) -> Node {
    let value = value.into_prop();
    let weak = Rc::downgrade(&node.0);
    create_effect(move || {
        let value = value.get();
        if let Some(data) = weak.upgrade() {
            set_attr(&Node(data), name, value.then(String::new));
        }
    });
    node
}

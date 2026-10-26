#[cfg(target_arch = "wasm32")]
pub mod dom;
pub mod elements;
mod event;
pub mod tw;
pub use beui_tree::reactive;
pub use event::Event;
pub use reactive::*;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

#[derive(Clone)]
pub struct Node(Rc<NodeData>);

struct NodeData {
    kind: Kind,
    attrs: RefCell<Vec<(String, Option<String>)>>,
    children: RefCell<ChildList<Node>>,
    css: RefCell<ChildList<tw::Style>>,
    #[cfg(target_arch = "wasm32")]
    listeners: RefCell<Vec<(&'static str, Callback<Event>)>>,
    #[cfg(target_arch = "wasm32")]
    live: RefCell<Option<dom::Live>>,
}

enum Kind {
    Element(&'static str),
    Text(RefCell<String>),
    Raw(String),
    Fragment,
    Styles,
}

impl Node {
    fn new(kind: Kind) -> Self {
        Self(Rc::new(NodeData {
            kind,
            attrs: RefCell::default(),
            children: RefCell::default(),
            css: RefCell::default(),
            #[cfg(target_arch = "wasm32")]
            listeners: RefCell::default(),
            #[cfg(target_arch = "wasm32")]
            live: RefCell::default(),
        }))
    }

    fn tag(&self) -> Option<&'static str> {
        match self.0.kind {
            Kind::Element(tag) => Some(tag),
            _ => None,
        }
    }

    fn holds_raw_text(&self) -> bool {
        self.tag().is_some_and(|tag| {
            RAW_TEXT_ELEMENTS
                .iter()
                .any(|raw| tag.eq_ignore_ascii_case(raw))
        })
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
        reactive::flush();
        let mut output = String::new();
        let mut sheet = tw::Sheet::default();
        self.collect_css(&mut sheet);
        assert!(
            sheet.is_empty() || self.has_styles(),
            "typed CSS requires a <tw::Styles /> outlet"
        );
        self.write(&mut output, &sheet);
        output
    }

    fn has_styles(&self) -> bool {
        matches!(self.0.kind, Kind::Styles) || self.0.children.borrow().iter().any(Node::has_styles)
    }

    fn collect_css(&self, sheet: &mut tw::Sheet) {
        sheet.add(&self.0.css.borrow());
        for child in self.0.children.borrow().iter() {
            child.collect_css(sheet);
        }
    }

    fn write(&self, output: &mut String, sheet: &tw::Sheet) {
        match &self.0.kind {
            Kind::Text(value) => escape_into(&value.borrow(), output),
            Kind::Raw(value) => output.push_str(value),
            Kind::Styles => {
                output.push_str("<style>");
                sheet.write(output);
                output.push_str("</style>");
            }
            Kind::Element(tag) => {
                output.push('<');
                output.push_str(tag);
                let css_class = tw::class_name(&self.0.css.borrow());
                let attrs = self.0.attrs.borrow();
                for (name, value) in attrs.iter() {
                    if let Some(value) = value {
                        output.push(' ');
                        output.push_str(name);
                        output.push_str("=\"");
                        escape_into(value, output);
                        if name == "class"
                            && let Some(class) = &css_class
                        {
                            output.push(' ');
                            output.push_str(class);
                        }
                        output.push('"');
                    }
                }
                if let Some(class) = css_class
                    && !attrs
                        .iter()
                        .any(|(name, value)| name == "class" && value.is_some())
                {
                    output.push_str(" class=\"");
                    output.push_str(&class);
                    output.push('"');
                }
                output.push('>');
                if !self.is_void() {
                    self.write_children(output, sheet);
                    output.push_str("</");
                    output.push_str(tag);
                    output.push('>');
                }
            }
            Kind::Fragment => self.write_children(output, sheet),
        }
    }

    fn write_children(&self, output: &mut String, sheet: &tw::Sheet) {
        let markers = !self.holds_raw_text();
        for (dynamic, items) in self.0.children.borrow().slots() {
            let marked = markers && dynamic.is_some();
            if marked {
                output.push_str(START_MARKER_HTML);
            }
            for child in items {
                child.write(output, sheet);
            }
            if marked {
                output.push_str(END_MARKER_HTML);
            }
        }
    }
}

const START_MARKER_HTML: &str = "<!--[-->";
const END_MARKER_HTML: &str = "<!--]-->";

const RAW_TEXT_ELEMENTS: [&str; 5] = ["script", "style", "textarea", "title", "noscript"];

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
    {
        let mut attrs = node.0.attrs.borrow_mut();
        if let Some(attr) = attrs.iter_mut().find(|attr| attr.0 == name) {
            if attr.1 == value {
                return;
            }
            attr.1 = value.clone();
        } else {
            attrs.push((name.into(), value.clone()));
        }
    }
    #[cfg(target_arch = "wasm32")]
    dom::set_attribute(node, name, value.as_deref());
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
    Node::new(Kind::Text(RefCell::new(value.to_string())))
}

fn set_text(node: &Node, value: String) {
    let Kind::Text(held) = &node.0.kind else {
        unreachable!("text binding on a text node");
    };
    if *held.borrow() == value {
        return;
    }
    *held.borrow_mut() = value;
    #[cfg(target_arch = "wasm32")]
    dom::set_text(node, &held.borrow());
}

pub fn render<C: IntoSegment<Node>>(build: impl FnOnce() -> C) -> String {
    format!("<!doctype html>{}", render_fragment(build))
}

pub fn render_fragment<C: IntoSegment<Node>>(build: impl FnOnce() -> C) -> String {
    let _root = RootScope::new();
    let scope = Scope::new();
    let node = scope.run(|| {
        let node = Node::new(Kind::Fragment);
        Children::from(into_segment(build())).mount(&node);
        node
    });
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
            #[cfg(target_arch = "wasm32")]
            dom::sync_children(&Node(data));
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
            #[cfg(target_arch = "wasm32")]
            dom::sync_slot(&Node(data), slot);
        }
    }
}
impl IntoSlotHost<Node> for &Node {
    fn into_slot_host(self) -> Rc<dyn SlotHost<Node>> {
        Rc::new(Host(Rc::downgrade(&self.0)))
    }
}

#[component]
pub fn Element(
    tag: &'static str,
    #[prop(default = Children::default())] css: Children<tw::Style>,
    children: Children<Node>,
) -> Node {
    assert!(valid_name(tag), "invalid HTML element name");
    let node = Node::new(Kind::Element(tag));
    css.mount(tw::host(&node));
    children.mount(&node);
    node
}

#[component]
pub fn Text(value: Prop<String>) -> Node {
    match value {
        Prop::Static(value) => text(value),
        Prop::Dynamic(read) => {
            let node = text("");
            let weak = Rc::downgrade(&node.0);
            create_effect(move || {
                let value = read.get();
                if let Some(data) = weak.upgrade() {
                    set_text(&Node(data), value);
                }
            });
            node
        }
    }
}

fn bind_attribute<T: Clone + ToString + 'static>(
    node: &Node,
    name: impl AsRef<str>,
    value: Prop<Option<T>>,
) {
    let name = name.as_ref();
    assert!(
        matches!(node.0.kind, Kind::Element(_)),
        "attributes require an element"
    );
    assert!(valid_name(name), "invalid HTML attribute name");
    match value {
        Prop::Static(value) => {
            if let Some(value) = value {
                set_attr(node, name, Some(value.to_string()));
            }
        }
        Prop::Dynamic(read) => {
            let name = name.to_owned();
            let weak = Rc::downgrade(&node.0);
            create_effect(move || {
                let value = read.get().map(|value| value.to_string());
                if let Some(data) = weak.upgrade() {
                    set_attr(&Node(data), &name, value);
                }
            });
        }
    }
}

fn bind_flag(node: &Node, name: &'static str, value: Prop<bool>) {
    assert!(
        matches!(node.0.kind, Kind::Element(_)),
        "attributes require an element"
    );
    assert!(valid_name(name), "invalid HTML attribute name");
    match value {
        Prop::Static(value) => {
            if value {
                set_attr(node, name, Some(String::new()));
            }
        }
        Prop::Dynamic(read) => {
            let weak = Rc::downgrade(&node.0);
            create_effect(move || {
                let value = read.get();
                if let Some(data) = weak.upgrade() {
                    set_attr(&Node(data), name, value.then(String::new));
                }
            });
        }
    }
}

pub fn attr(node: Node, (name, value): (&'static str, impl IntoProp<Option<String>>)) -> Node {
    bind_attribute(&node, name, value.into_prop());
    node
}

pub fn data(node: Node, (name, value): (&'static str, impl IntoProp<Option<String>>)) -> Node {
    assert!(valid_name(name), "invalid data attribute name");
    bind_attribute(&node, format!("data-{name}"), value.into_prop());
    node
}

pub fn aria(node: Node, (name, value): (&'static str, impl IntoProp<Option<String>>)) -> Node {
    assert!(valid_name(name), "invalid ARIA attribute name");
    bind_attribute(&node, format!("aria-{name}"), value.into_prop());
    node
}

#[cfg(test)]
mod tests;

#[component]
pub fn Stylesheet(css: String) -> Node {
    assert!(
        !css.to_ascii_lowercase().contains("</style"),
        "CSS contains a closing style tag"
    );
    view! {
        <elements::Style>{Node::new(Kind::Raw(css))}</elements::Style>
    }
}

#[component]
pub fn Script(kind: &'static str, source: String) -> Node {
    assert!(
        !source.to_ascii_lowercase().contains("</script"),
        "script contains a closing script tag"
    );
    view! {
        <elements::Script r#type={kind.to_string()}>
            {Node::new(Kind::Raw(source))}
        </elements::Script>
    }
}

pub fn flag(node: Node, (name, value): (&'static str, impl IntoProp<bool>)) -> Node {
    bind_flag(&node, name, value.into_prop());
    node
}

pub fn on(node: Node, (name, handler): (&'static str, impl IntoCallback<Event, ()>)) -> Node {
    assert!(valid_name(name), "invalid event name");
    listen(&node, name, handler.into_callback());
    node
}

fn listen(node: &Node, name: &'static str, handler: Callback<Event>) {
    assert!(
        matches!(node.0.kind, Kind::Element(_)),
        "event listeners require an element"
    );
    #[cfg(target_arch = "wasm32")]
    if !handler.is_empty() {
        node.0.listeners.borrow_mut().push((name, handler));
        dom::listen(node, name, handler);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (name, handler);
}

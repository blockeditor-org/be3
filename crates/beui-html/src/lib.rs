#[cfg(target_arch = "wasm32")]
pub mod dom;
pub mod elements;
mod event;
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
    markup: RefCell<Option<String>>,
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
}

impl Node {
    fn new(kind: Kind) -> Self {
        Self(Rc::new(NodeData {
            kind,
            attrs: RefCell::default(),
            children: RefCell::default(),
            markup: RefCell::default(),
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

    pub fn element(tag: &'static str) -> Self {
        assert!(valid_name(tag), "invalid HTML element name");
        Self::new(Kind::Element(tag))
    }

    pub fn text(value: impl ToString) -> Self {
        text(value)
    }

    pub fn set_attribute(&self, name: &str, value: Option<String>) {
        set_attr(self, name, value);
    }

    pub fn set_text(&self, value: String) {
        set_text(self, value);
    }

    pub fn set_markup(&self, markup: Option<String>) {
        assert!(
            matches!(self.0.kind, Kind::Element(_)) && !self.is_void() && !self.holds_raw_text(),
            "markup goes inside an element that holds children"
        );
        assert!(
            self.0.children.borrow().iter().next().is_none(),
            "an element holds markup or children, not both"
        );
        if *self.0.markup.borrow() == markup {
            return;
        }
        *self.0.markup.borrow_mut() = markup;
        #[cfg(target_arch = "wasm32")]
        dom::set_markup(self);
    }

    #[cfg(target_arch = "wasm32")]
    fn markup(&self) -> Option<String> {
        self.0.markup.borrow().clone()
    }

    pub fn listen(&self, name: &'static str, handler: Callback<Event>) {
        assert!(valid_name(name), "invalid event name");
        listen(self, name, handler);
    }

    #[cfg(target_arch = "wasm32")]
    pub fn dom(&self) -> Option<web_sys::Node> {
        dom::live_node(self)
    }

    pub fn is(&self, other: &Node) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub fn find(&self, found: &impl Fn(&Node) -> bool) -> Option<Node> {
        if found(self) {
            return Some(self.clone());
        }
        let children = self.0.children.borrow();
        children.iter().find_map(|child| child.find(found))
    }

    pub fn to_html(&self) -> String {
        reactive::flush();
        let mut output = String::new();
        self.write(&mut output, None);
        output
    }

    fn write(&self, output: &mut String, raw_text_of: Option<&str>) {
        match &self.0.kind {
            Kind::Text(value) => match raw_text_of {
                Some(tag) => raw_text_into(&value.borrow(), tag, output),
                None => escape_into(&value.borrow(), output),
            },
            Kind::Raw(value) => output.push_str(value),
            Kind::Element(tag) => {
                output.push('<');
                output.push_str(tag);
                let attrs = self.0.attrs.borrow();
                for (name, value) in attrs.iter() {
                    if let Some(value) = value {
                        output.push(' ');
                        output.push_str(name);
                        output.push_str("=\"");
                        escape_into(value, output);
                        output.push('"');
                    }
                }
                output.push('>');
                if let Some(markup) = self.0.markup.borrow().as_deref() {
                    output.push_str(markup);
                    output.push_str("</");
                    output.push_str(tag);
                    output.push('>');
                } else if !self.is_void() {
                    let raw_text_of = UNESCAPED_ELEMENTS
                        .iter()
                        .find(|raw| tag.eq_ignore_ascii_case(raw))
                        .copied();
                    self.write_children(output, raw_text_of);
                    output.push_str("</");
                    output.push_str(tag);
                    output.push('>');
                }
            }
            Kind::Fragment => self.write_children(output, raw_text_of),
        }
    }

    fn write_children(&self, output: &mut String, raw_text_of: Option<&str>) {
        let markers = !self.holds_raw_text();
        for (dynamic, items) in self.0.children.borrow().slots() {
            let marked = markers && dynamic.is_some();
            if marked {
                output.push_str(START_MARKER_HTML);
            }
            for child in items {
                child.write(output, raw_text_of);
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

const UNESCAPED_ELEMENTS: [&str; 2] = ["script", "style"];

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

fn raw_text_into(value: &str, tag: &str, output: &mut String) {
    let closing = format!("</{}", tag.to_ascii_lowercase());
    assert!(
        !value.to_ascii_lowercase().contains(&closing),
        "raw text contains a closing {tag} tag"
    );
    output.push_str(value);
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
            assert!(
                data.markup.borrow().is_none(),
                "an element holds markup or children, not both"
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
pub fn Element(tag: &'static str, children: Children<Node>) -> Node {
    assert!(valid_name(tag), "invalid HTML element name");
    let node = Node::new(Kind::Element(tag));
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

use crate::{
    Callback, Children, Event, IntoSegment, Kind, Node, Scope, SlotId, flush, into_segment,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

const START_MARKER: &str = "[";
const END_MARKER: &str = "]";

type Listener = Closure<dyn FnMut(web_sys::Event)>;

thread_local! {
    static DISPLACED: std::cell::RefCell<Option<web_sys::Element>> = const { std::cell::RefCell::new(None) };
}

pub(crate) struct Live {
    node: web_sys::Node,
    anchors: Vec<(SlotId, web_sys::Comment, web_sys::Comment)>,
    listeners: Vec<Listener>,
}

pub struct Mounted {
    _root: Node,
    _scope: Scope,
}

pub fn mount<C: IntoSegment<Node>>(
    container: &web_sys::Node,
    build: impl FnOnce() -> C,
) -> Mounted {
    let (root, scope) = build_root(build);
    attach(&root, container.clone());
    sync_children(&root);
    Mounted {
        _root: root,
        _scope: scope,
    }
}

pub fn hydrate<C: IntoSegment<Node>>(
    container: &web_sys::Node,
    build: impl FnOnce() -> C,
) -> Mounted {
    let (root, scope) = build_root(build);
    attach(&root, container.clone());
    claim_children(&root, container);
    Mounted {
        _root: root,
        _scope: scope,
    }
}

fn build_root<C: IntoSegment<Node>>(build: impl FnOnce() -> C) -> (Node, Scope) {
    let scope = Scope::detached();
    let root = scope.run(|| {
        let node = Node::new(Kind::Fragment);
        Children::from(into_segment(build())).mount(&node);
        node
    });
    flush();
    (root, scope)
}

fn document() -> web_sys::Document {
    web_sys::window()
        .and_then(|window| window.document())
        .expect("the live HTML backend runs in a page with a document")
}

pub(crate) fn live_node(node: &Node) -> Option<web_sys::Node> {
    node.0.live.borrow().as_ref().map(|live| live.node.clone())
}

fn attach(node: &Node, dom: web_sys::Node) {
    *node.0.live.borrow_mut() = Some(Live {
        node: dom,
        anchors: Vec::new(),
        listeners: Vec::new(),
    });
    let listeners = node.0.listeners.borrow().clone();
    for (name, handler) in listeners {
        add_listener(node, name, handler);
    }
}

fn add_listener(node: &Node, name: &'static str, handler: Callback<Event>) {
    let mut live = node.0.live.borrow_mut();
    let Some(live) = live.as_mut() else {
        return;
    };
    let listener = Listener::new(move |event: web_sys::Event| {
        handler.call(Event(event));
        flush();
    });
    let _ = live
        .node
        .add_event_listener_with_callback(name, listener.as_ref().unchecked_ref());
    live.listeners.push(listener);
}

pub(crate) fn listen(node: &Node, name: &'static str, handler: Callback<Event>) {
    add_listener(node, name, handler);
}

pub(crate) fn set_attribute(node: &Node, name: &str, value: Option<&str>) {
    let Some(dom) = live_node(node) else {
        return;
    };
    let Some(element) = dom.dyn_ref::<web_sys::Element>() else {
        return;
    };
    let _ = match value {
        Some(value) => element.set_attribute(name, value),
        None => element.remove_attribute(name),
    };
    let tag = element.local_name();
    let property: Option<wasm_bindgen::JsValue> = match (name, tag.as_str()) {
        ("value", "input" | "select" | "textarea") => Some(value.unwrap_or_default().into()),
        ("checked", "input") | ("selected", "option") => Some(value.is_some().into()),
        _ => None,
    };
    if let Some(property) = property {
        let _ = js_sys::Reflect::set(element, &name.into(), &property);
    }
}

pub(crate) fn set_markup(node: &Node) {
    if let Some(element) = live_node(node).and_then(|dom| dom.dyn_into::<web_sys::Element>().ok()) {
        element.set_inner_html(node.markup().as_deref().unwrap_or_default());
    }
}

pub(crate) fn set_text(node: &Node, value: &str) {
    if let Some(dom) = live_node(node) {
        dom.set_text_content(Some(value));
    }
}

fn ensure_dom(node: &Node) -> web_sys::Node {
    live_node(node).unwrap_or_else(|| create(node))
}

fn create(node: &Node) -> web_sys::Node {
    let document = document();
    let dom: web_sys::Node = match &node.0.kind {
        Kind::Element(tag) => {
            let element = document
                .create_element(tag)
                .expect("element names are checked when they are built");
            for (name, value) in node.0.attrs.borrow().iter() {
                if let Some(value) = value {
                    let _ = element.set_attribute(name, value);
                }
            }
            element.into()
        }
        Kind::Text(value) => document.create_text_node(&value.borrow()).into(),
        Kind::Raw(value) => document.create_text_node(value).into(),
        Kind::Fragment => unreachable!("a fragment node is only ever a root"),
    };
    attach(node, dom.clone());
    if matches!(node.0.kind, Kind::Element(_)) {
        sync_children(node);
    }
    dom
}

fn anchors(node: &Node, slot: SlotId) -> (web_sys::Comment, web_sys::Comment) {
    let mut live = node.0.live.borrow_mut();
    let live = live.as_mut().expect("anchors belong to a live node");
    if let Some((_, start, end)) = live.anchors.iter().find(|anchor| anchor.0 == slot) {
        return (start.clone(), end.clone());
    }
    let document = document();
    let start = document.create_comment(START_MARKER);
    let end = document.create_comment(END_MARKER);
    live.anchors.push((slot, start.clone(), end.clone()));
    (start, end)
}

fn children_of(node: &Node) -> Vec<(Option<SlotId>, Vec<Node>)> {
    node.0
        .children
        .borrow()
        .slots()
        .map(|(slot, items)| (slot, items.to_vec()))
        .collect()
}

pub(crate) fn sync_children(node: &Node) {
    let Some(parent) = live_node(node) else {
        return;
    };
    if node.markup().is_some() {
        set_markup(node);
        return;
    }
    let mut wanted = Vec::new();
    for (slot, items) in children_of(node) {
        match slot {
            None => wanted.extend(items.iter().map(ensure_dom)),
            Some(slot) => {
                let (start, end) = anchors(node, slot);
                wanted.push(start.into());
                wanted.extend(items.iter().map(ensure_dom));
                wanted.push(end.into());
            }
        }
    }
    place(&parent, parent.first_child(), wanted, None);
}

pub(crate) fn sync_slot(node: &Node, slot: SlotId) {
    let Some(parent) = live_node(node) else {
        return;
    };
    let (start, end) = anchors(node, slot);
    let items = node
        .0
        .children
        .borrow()
        .slot(slot)
        .map(<[Node]>::to_vec)
        .unwrap_or_default();
    let wanted = items.iter().map(ensure_dom).collect();
    place(&parent, start.next_sibling(), wanted, Some(end.into()));
}

fn place(
    parent: &web_sys::Node,
    mut cursor: Option<web_sys::Node>,
    wanted: Vec<web_sys::Node>,
    end: Option<web_sys::Node>,
) {
    for dom in wanted {
        if cursor.as_ref() == Some(&dom) {
            cursor = dom.next_sibling();
        } else {
            insert(parent, &dom, cursor.as_ref());
        }
    }
    while let Some(stale) = cursor {
        if Some(&stale) == end.as_ref() {
            break;
        }
        cursor = stale.next_sibling();
        displace(&stale);
        let _ = parent.remove_child(&stale);
    }
}

fn insert(parent: &web_sys::Node, dom: &web_sys::Node, before: Option<&web_sys::Node>) {
    let moves = dom.is_connected() && parent.is_connected();
    if moves
        && let Ok(move_before) = js_sys::Reflect::get(parent, &"moveBefore".into())
        && let Ok(move_before) = move_before.dyn_into::<js_sys::Function>()
    {
        let before = before.map_or(wasm_bindgen::JsValue::NULL, |before| before.into());
        if move_before.call2(parent, dom, &before).is_ok() {
            return;
        }
    }
    displace(dom);
    let _ = parent.insert_before(dom, before);
    restore_focus();
}

fn displace(dom: &web_sys::Node) {
    if !dom.is_connected() {
        return;
    }
    let Some(active) = document().active_element() else {
        return;
    };
    if dom.contains(Some(active.as_ref())) {
        DISPLACED.with(|displaced| displaced.replace(Some(active)));
    }
}

fn restore_focus() {
    let Some(element) = DISPLACED.with(|displaced| {
        let mut displaced = displaced.borrow_mut();
        displaced
            .as_ref()
            .is_some_and(|element| element.is_connected())
            .then(|| displaced.take())
            .flatten()
    }) else {
        return;
    };
    let lost = document()
        .active_element()
        .is_none_or(|active| active.tag_name() == "BODY");
    if !lost {
        return;
    }
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &"preventScroll".into(), &true.into());
    if let Ok(focus) = js_sys::Reflect::get(&element, &"focus".into())
        && let Ok(focus) = focus.dyn_into::<js_sys::Function>()
    {
        let _ = focus.call1(&element, &options);
    }
}

fn mismatch(what: &str) {
    if cfg!(debug_assertions) {
        web_sys::console::warn_1(
            &format!("beui-html hydration mismatch: {what}; rebuilding that part").into(),
        );
    }
}

fn skip_doctype(cursor: &mut Option<web_sys::Node>) {
    while let Some(dom) = cursor.as_ref()
        && dom.node_type() == web_sys::Node::DOCUMENT_TYPE_NODE
    {
        *cursor = dom.next_sibling();
    }
}

fn comment(dom: Option<&web_sys::Node>, data: &str) -> Option<web_sys::Comment> {
    dom?.dyn_ref::<web_sys::Comment>()
        .filter(|comment| comment.data() == data)
        .cloned()
}

fn claim_children(node: &Node, parent: &web_sys::Node) {
    if let Some(markup) = node.markup() {
        if let Some(element) = parent.dyn_ref::<web_sys::Element>()
            && element.inner_html() != markup
        {
            element.set_inner_html(&markup);
        }
        return;
    }
    let children = children_of(node);
    if node.holds_raw_text() && children.iter().any(|(slot, _)| slot.is_some()) {
        sync_children(node);
        return;
    }
    let mut cursor = parent.first_child();
    for (slot, items) in children {
        let Some(slot) = slot else {
            for item in &items {
                claim(item, parent, &mut cursor);
            }
            continue;
        };
        let start = claim_start(parent, &mut cursor);
        for item in &items {
            claim(item, parent, &mut cursor);
        }
        let end = claim_end(parent, &mut cursor);
        if let Some(live) = node.0.live.borrow_mut().as_mut() {
            live.anchors.push((slot, start, end));
        }
    }
    let mut extra = false;
    while let Some(stale) = cursor {
        cursor = stale.next_sibling();
        if stale.node_type() != web_sys::Node::DOCUMENT_TYPE_NODE {
            extra = true;
            let _ = parent.remove_child(&stale);
        }
    }
    if extra {
        mismatch("the page has nodes the view does not build");
    }
}

fn claim_start(parent: &web_sys::Node, cursor: &mut Option<web_sys::Node>) -> web_sys::Comment {
    skip_doctype(cursor);
    if let Some(start) = comment(cursor.as_ref(), START_MARKER) {
        *cursor = start.next_sibling();
        return start;
    }
    mismatch("a dynamic range has no start marker");
    let start = document().create_comment(START_MARKER);
    let _ = parent.insert_before(&start, cursor.as_ref());
    start
}

fn claim_end(parent: &web_sys::Node, cursor: &mut Option<web_sys::Node>) -> web_sys::Comment {
    let mut scan = cursor.clone();
    while let Some(dom) = scan {
        if let Some(end) = comment(Some(&dom), END_MARKER) {
            if cursor.as_ref() != Some(&dom) {
                mismatch("a dynamic range has nodes the view does not build");
            }
            while let Some(stale) = cursor.take() {
                if stale == dom {
                    break;
                }
                *cursor = stale.next_sibling();
                let _ = parent.remove_child(&stale);
            }
            *cursor = end.next_sibling();
            return end;
        }
        if comment(Some(&dom), START_MARKER).is_some() {
            break;
        }
        scan = dom.next_sibling();
    }
    mismatch("a dynamic range has no end marker");
    let end = document().create_comment(END_MARKER);
    let _ = parent.insert_before(&end, cursor.as_ref());
    end
}

fn claim(item: &Node, parent: &web_sys::Node, cursor: &mut Option<web_sys::Node>) {
    skip_doctype(cursor);
    let claimed = match &item.0.kind {
        Kind::Element(tag) => claim_element(cursor.as_ref(), tag),
        Kind::Text(value) => claim_text(cursor.as_ref(), &value.borrow()),
        Kind::Raw(value) => claim_text(cursor.as_ref(), value),
        Kind::Fragment => None,
    };
    match claimed {
        Some(dom) => {
            *cursor = dom.next_sibling();
            attach(item, dom.clone());
            if matches!(item.0.kind, Kind::Element(_)) {
                claim_children(item, &dom);
            }
        }
        None => {
            let dom = ensure_dom(item);
            let _ = parent.insert_before(&dom, cursor.as_ref());
        }
    }
}

fn claim_element(dom: Option<&web_sys::Node>, tag: &str) -> Option<web_sys::Node> {
    let found = dom?
        .dyn_ref::<web_sys::Element>()
        .filter(|element| element.local_name().eq_ignore_ascii_case(tag))
        .map(|element| element.clone().into());
    if found.is_none() {
        mismatch(&format!("expected a <{tag}>"));
    }
    found
}

fn claim_text(dom: Option<&web_sys::Node>, expected: &str) -> Option<web_sys::Node> {
    if expected.is_empty() {
        return None;
    }
    let Some(text) = dom.and_then(|dom| dom.dyn_ref::<web_sys::Text>()) else {
        mismatch("expected text");
        return None;
    };
    let data = text.data();
    if data != expected {
        if data.starts_with(expected) {
            let _ = text.split_text(expected.encode_utf16().count() as u32);
        } else {
            mismatch("the text differs");
            text.set_data(expected);
        }
    }
    Some(text.clone().into())
}

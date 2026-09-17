use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

pub use crate::base::ItemSize;

use crate::base::{Align, Direction};
use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{ClickHandler, Handler, NodeId};
use crate::unstyled;

pub use beui_macros::{component, view};
pub use reactive::{
    Effect, KeyedStore, Memo, ReadSignal, Scope, ScopeContext, Selector, Store, WriteSignal, batch,
    clone, create_effect, create_memo, create_selector, create_signal, on_cleanup, owner_scope,
    provide_context, settle, untrack, use_context,
};

thread_local! {
    static CURRENT_DOCUMENT: RefCell<Option<Document>> = const { RefCell::new(None) };
    static ACTIVE_DOCUMENT: Cell<*mut Document> = const { Cell::new(std::ptr::null_mut()) };
    static CURRENT_COMPONENT: RefCell<Option<Rc<ComponentContext>>> = const { RefCell::new(None) };
}

#[derive(Default)]
struct ComponentContext {
    target: Cell<Option<NodeId>>,
    states: RefCell<Vec<Box<dyn Any>>>,
    accessibility: RefCell<Option<accesskit::Node>>,
    size: RefCell<Option<(ReadSignal<Vec2>, WriteSignal<Vec2>)>>,
    placement: RefCell<Option<(ReadSignal<Rect>, WriteSignal<Rect>)>>,
}

struct ActiveDocumentGuard;

impl Drop for ActiveDocumentGuard {
    fn drop(&mut self) {
        ACTIVE_DOCUMENT.with(|active| active.set(std::ptr::null_mut()));
    }
}

pub(crate) enum DocumentGuard<'a> {
    Installed(&'a mut Document),
    Reentrant,
}

impl Drop for DocumentGuard<'_> {
    fn drop(&mut self) {
        if let DocumentGuard::Installed(document) = self {
            let restored = CURRENT_DOCUMENT.with(|cell| cell.borrow_mut().take());
            **document = restored
                .expect("beui::reactive document guard dropped without an installed document");
        }
    }
}

pub(crate) fn install(document: &mut Document) -> DocumentGuard<'_> {
    let already_installed = CURRENT_DOCUMENT
        .with(|cell| cell.try_borrow().map(|slot| slot.is_some()))
        .unwrap_or(true);
    if already_installed {
        return DocumentGuard::Reentrant;
    }
    let taken = std::mem::take(document);
    CURRENT_DOCUMENT.with(|cell| {
        let previous = cell.borrow_mut().replace(taken);
        assert!(
            previous.is_none(),
            "beui::reactive: a document is already installed on this thread"
        );
    });
    DocumentGuard::Installed(document)
}

pub(crate) fn enter<R>(document: &mut Document, f: impl FnOnce() -> R) -> R {
    let context = document.reactive_scope().context();
    let _guard = install(document);
    context.run(f)
}

pub fn with_reactive_scope<R>(document: &mut Document, f: impl FnOnce() -> R) -> R {
    enter(document, f)
}

pub fn with_document<R>(f: impl FnOnce(&mut Document) -> R) -> R {
    CURRENT_DOCUMENT.with(|cell| match cell.try_borrow_mut() {
        Ok(mut slot) => {
            let document = slot.as_mut().expect(
                "beui::reactive binding used without an active document; \
                 call it from inside build(), or from inside event dispatch",
            );
            let ptr: *mut Document = document;
            ACTIVE_DOCUMENT.with(|active| active.set(ptr));
            let _guard = ActiveDocumentGuard;
            f(document)
        }
        Err(_) => {
            let ptr = ACTIVE_DOCUMENT.with(Cell::get);
            assert!(
                !ptr.is_null(),
                "beui::reactive binding used without an active document; \
                 call it from inside build(), or from inside event dispatch"
            );
            f(unsafe { &mut *ptr })
        }
    })
}

pub fn build(f: impl FnOnce() -> NodeId) -> Document {
    let mut document = Document::new();
    let root = enter(&mut document, f);
    document.set_root(root);
    document
}

pub fn node_size(node: NodeId) -> ReadSignal<Vec2> {
    with_document(|document| document.watch_size(node))
}

pub fn node_rect(node: NodeId) -> ReadSignal<Rect> {
    with_document(|document| document.watch_placement(node))
}

pub fn copy_text(text: impl Into<String>) {
    let text = text.into();
    with_document(|document| document.copy_text(text));
}

pub(crate) fn node_scope(document: &Document, owner: Option<ScopeContext>) -> Scope {
    owner
        .or_else(owner_scope)
        .and_then(|owner| owner.child())
        .unwrap_or_else(|| document.reactive_scope().context().run(Scope::new))
}

pub fn in_new_scope(f: impl FnOnce() -> NodeId) -> NodeId {
    let scope = with_document(|document| node_scope(document, None));
    let node = scope.context().run(f);
    with_document(|document| document.register_node_scope(node, scope));
    node
}

pub fn component<T: ChildValue>(f: impl FnOnce() -> T) -> T {
    let scope = Scope::new();
    let context = Rc::new(ComponentContext::default());
    let value = in_component(Some(context.clone()), || scope.run(f));
    let root = value.anchor();
    context.target.set(Some(root));
    let states = context.states.take();
    let accessibility = context.accessibility.take();
    let size = context.size.take();
    let placement = context.placement.take();
    with_document(|document| {
        for state in states {
            document.set_component_state_dyn(root, state);
        }
        if let Some(node) = accessibility {
            document.set_accessibility(root, node);
        }
        if let Some((read, write)) = size {
            document.register_size_watcher(root, read, write);
        }
        if let Some((read, write)) = placement {
            document.register_placement_watcher(root, read, write);
        }
        document.register_node_scope(root, scope);
    });
    value
}

fn current_component() -> Rc<ComponentContext> {
    CURRENT_COMPONENT
        .with(|cell| cell.borrow().clone())
        .expect("a component binding was used outside of a #[component] body")
}

pub fn component_size() -> ReadSignal<Vec2> {
    let component = current_component();
    if let Some(target) = component.target.get() {
        return node_size(target);
    }
    if let Some((read, _)) = component.size.borrow().as_ref() {
        return read.clone();
    }
    let (read, write) = create_signal(Vec2::ZERO);
    component.size.replace(Some((read.clone(), write)));
    read
}

pub fn component_rect() -> ReadSignal<Rect> {
    let component = current_component();
    if let Some(target) = component.target.get() {
        return node_rect(target);
    }
    if let Some((read, _)) = component.placement.borrow().as_ref() {
        return read.clone();
    }
    let (read, write) = create_signal(Rect::ZERO);
    component.placement.replace(Some((read.clone(), write)));
    read
}

pub fn set_component_state<T: 'static>(state: T) {
    let component = current_component();
    match component.target.get() {
        Some(target) => {
            with_document(|document| document.set_component_state_dyn(target, Box::new(state)))
        }
        None => component.states.borrow_mut().push(Box::new(state)),
    }
}

fn set_accessibility(component: &ComponentContext, node: accesskit::Node) {
    match component.target.get() {
        Some(target) => with_document(|document| document.set_accessibility(target, node)),
        None => {
            component.accessibility.replace(Some(node));
        }
    }
}

pub fn component_accessibility(node: impl IntoProp<accesskit::Node>) {
    let component = current_component();
    let node = node.into_prop();
    create_effect(move || set_accessibility(&component, node.get()));
}

pub struct MissingProp;

#[derive(Clone, Default)]
pub struct NodeRef(Rc<Cell<Option<NodeId>>>);

impl PartialEq for NodeRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl NodeRef {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fill(&self, node: NodeId) {
        self.0.set(Some(node));
    }

    pub fn get(&self) -> NodeId {
        self.0
            .get()
            .expect("node_ref read before the node it points at was built")
    }

    pub fn try_get(&self) -> Option<NodeId> {
        self.0.get()
    }
}

pub struct Callback<V, R = ()>(Rc<RefCell<Option<Handler<V, R>>>>);

impl<V, R> Callback<V, R> {
    pub fn new(handler: impl FnMut(V) -> R + 'static) -> Self {
        Self(Rc::new(RefCell::new(Some(Box::new(handler)))))
    }

    pub fn empty() -> Self {
        Self(Rc::new(RefCell::new(None)))
    }

    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_none()
    }

    pub fn set(&self, handler: impl FnMut(V) -> R + 'static) {
        *self.0.borrow_mut() = Some(Box::new(handler));
    }

    pub fn call(&self, value: V) -> R
    where
        R: Default,
    {
        let Some(mut handler) = self.0.borrow_mut().take() else {
            return R::default();
        };
        let result = handler(value);
        let mut slot = self.0.borrow_mut();
        if slot.is_none() {
            *slot = Some(handler);
        }
        result
    }
}

impl<V, R> Clone for Callback<V, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<V, R> Default for Callback<V, R> {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Default)]
pub struct ClickCallback(Rc<RefCell<Option<ClickHandler>>>);

impl ClickCallback {
    pub fn new(handler: impl FnMut() + 'static) -> Self {
        Self(Rc::new(RefCell::new(Some(Box::new(handler)))))
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_none()
    }

    pub fn set(&self, handler: impl FnMut() + 'static) {
        *self.0.borrow_mut() = Some(Box::new(handler));
    }

    pub fn call(&self) {
        let Some(mut handler) = self.0.borrow_mut().take() else {
            return;
        };
        handler();
        let mut slot = self.0.borrow_mut();
        if slot.is_none() {
            *slot = Some(handler);
        }
    }
}

struct ComponentGuard(Option<Rc<ComponentContext>>);

impl Drop for ComponentGuard {
    fn drop(&mut self) {
        CURRENT_COMPONENT.with(|cell| *cell.borrow_mut() = self.0.take());
    }
}

fn in_component<R>(owner: Option<Rc<ComponentContext>>, f: impl FnOnce() -> R) -> R {
    let previous = CURRENT_COMPONENT.with(|cell| std::mem::replace(&mut *cell.borrow_mut(), owner));
    let _guard = ComponentGuard(previous);
    f()
}

pub struct Render<H = ()> {
    owner: Option<Rc<ComponentContext>>,
    render: Box<dyn FnOnce(H) -> NodeId>,
}

impl<H> Render<H> {
    pub fn new(render: impl FnOnce(H) -> NodeId + 'static) -> Self {
        Self {
            owner: CURRENT_COMPONENT.with(|cell| cell.borrow().clone()),
            render: Box::new(render),
        }
    }

    pub fn call(self, handle: H) -> NodeId {
        in_component(self.owner, || (self.render)(handle))
    }
}

pub struct RenderFn<H> {
    owner: Option<Rc<ComponentContext>>,
    render: Rc<dyn Fn(H) -> NodeId>,
}

impl<H> RenderFn<H> {
    pub fn new(render: impl Fn(H) -> NodeId + 'static) -> Self {
        Self {
            owner: CURRENT_COMPONENT.with(|cell| cell.borrow().clone()),
            render: Rc::new(render),
        }
    }

    pub fn call(&self, handle: H) -> NodeId {
        in_component(self.owner.clone(), || (self.render)(handle))
    }
}

impl<H> Clone for RenderFn<H> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            render: self.render.clone(),
        }
    }
}

pub struct Func<V, R>(Rc<dyn Fn(V) -> R>);

impl<V, R> Func<V, R> {
    pub fn new(function: impl Fn(V) -> R + 'static) -> Self {
        Self(Rc::new(function))
    }

    pub fn call(&self, value: V) -> R {
        (self.0)(value)
    }
}

impl<V, R> Clone for Func<V, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub trait IntoFunc<V, R> {
    fn into_func(self) -> Func<V, R>;
}

impl<V, R, F: Fn(V) -> R + 'static> IntoFunc<V, R> for F {
    fn into_func(self) -> Func<V, R> {
        Func::new(self)
    }
}

impl<V, R> IntoFunc<V, R> for Func<V, R> {
    fn into_func(self) -> Func<V, R> {
        self
    }
}

pub trait IntoRender<H> {
    fn into_render(self) -> Render<H>;
}

impl<H, F: FnOnce(H) -> NodeId + 'static> IntoRender<H> for F {
    fn into_render(self) -> Render<H> {
        Render::new(self)
    }
}

impl<H> IntoRender<H> for Render<H> {
    fn into_render(self) -> Render<H> {
        self
    }
}

pub trait IntoRenderFn<H> {
    fn into_render_fn(self) -> RenderFn<H>;
}

impl<H, F: Fn(H) -> NodeId + 'static> IntoRenderFn<H> for F {
    fn into_render_fn(self) -> RenderFn<H> {
        RenderFn::new(self)
    }
}

impl<H> IntoRenderFn<H> for RenderFn<H> {
    fn into_render_fn(self) -> RenderFn<H> {
        self
    }
}

pub enum Prop<T> {
    Static(T),
    Dynamic(Box<dyn Fn() -> T>),
}

impl<T: 'static> Prop<T> {
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        match self {
            Prop::Static(value) => value.clone(),
            Prop::Dynamic(read) => read(),
        }
    }

    pub fn peek(&self) -> T
    where
        T: Clone,
    {
        untrack(|| self.get())
    }

    pub fn map<U: 'static>(self, f: impl Fn(T) -> U + 'static) -> Prop<U> {
        match self {
            Prop::Static(value) => Prop::Static(f(value)),
            Prop::Dynamic(read) => Prop::Dynamic(Box::new(move || f(read()))),
        }
    }
}

pub trait IntoProp<T> {
    fn into_prop(self) -> Prop<T>;
}

impl<T: 'static> IntoProp<T> for T {
    fn into_prop(self) -> Prop<T> {
        Prop::Static(self)
    }
}

impl IntoProp<String> for &'static str {
    fn into_prop(self) -> Prop<String> {
        Prop::Static(self.to_string())
    }
}

impl<T: 'static> IntoProp<T> for Prop<T> {
    fn into_prop(self) -> Prop<T> {
        self
    }
}

impl<T: Clone + 'static> IntoProp<T> for ReadSignal<T> {
    fn into_prop(self) -> Prop<T> {
        Prop::Dynamic(Box::new(move || self.get()))
    }
}

impl<T: Clone + PartialEq + 'static> IntoProp<T> for Memo<T> {
    fn into_prop(self) -> Prop<T> {
        Prop::Dynamic(Box::new(move || self.get()))
    }
}

pub fn intrinsic(node: NodeId) -> ListChild {
    ListChild::new(node, ItemSize::Intrinsic)
}

pub fn fixed(node: NodeId, size: impl IntoProp<f32>) -> ListChild {
    ListChild {
        node,
        size: size.into_prop().map(ItemSize::Fixed),
    }
}

pub fn percent(node: NodeId, weight: impl IntoProp<f32>) -> ListChild {
    ListChild {
        node,
        size: weight.into_prop().map(ItemSize::Percent),
    }
}

pub fn size(node: NodeId, size: impl IntoProp<ItemSize>) -> ListChild {
    ListChild::new(node, size)
}

pub type Child = NodeId;

#[diagnostic::on_unimplemented(
    message = "a `#[component]` function returns a node or a value built around one",
    label = "give this component a return type that names the node it builds"
)]
pub trait ChildValue {
    fn anchor(&self) -> NodeId;
}

impl ChildValue for NodeId {
    fn anchor(&self) -> NodeId {
        *self
    }
}

impl ChildValue for ListChild {
    fn anchor(&self) -> NodeId {
        self.node
    }
}

pub struct ListChild {
    pub node: NodeId,
    pub size: Prop<ItemSize>,
}

impl ListChild {
    pub fn new(node: NodeId, size: impl IntoProp<ItemSize>) -> Self {
        Self {
            node,
            size: size.into_prop(),
        }
    }
}

#[diagnostic::on_unimplemented(
    message = "a `{Self}` cannot be a child of this component",
    label = "this component takes `{T}` children"
)]
pub trait IntoChild<T> {
    fn into_child(self) -> T;
}

pub fn into_child<T>(child: impl IntoChild<T>) -> T {
    child.into_child()
}

impl IntoChild<NodeId> for NodeId {
    fn into_child(self) -> NodeId {
        self
    }
}

impl IntoChild<ListChild> for NodeId {
    fn into_child(self) -> ListChild {
        ListChild::new(self, ItemSize::Intrinsic)
    }
}

#[diagnostic::on_unimplemented(
    message = "`@sizing` only applies to a child of a list",
    label = "remove `@sizing`, or put this child in a `Row`, `Column`, `List` or `Stack`",
    note = "`intrinsic`, `fixed`, `percent` and `size` give a child a size too, so a hand-built \
            `Vec<ListChild>` only fits a list"
)]
pub trait AcceptsSizing {
    fn from_list_child(child: ListChild) -> Self;
}

impl AcceptsSizing for ListChild {
    fn from_list_child(child: ListChild) -> Self {
        child
    }
}

impl<T: AcceptsSizing> IntoChild<T> for ListChild {
    fn into_child(self) -> T {
        T::from_list_child(self)
    }
}

pub struct Children<T>(Vec<T>);

impl<T> Default for Children<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T, const N: usize> From<[T; N]> for Children<T> {
    fn from(children: [T; N]) -> Self {
        Self(Vec::from(children))
    }
}

impl<T, U: IntoChild<T>> From<Vec<U>> for Children<T> {
    fn from(children: Vec<U>) -> Self {
        Self(children.into_iter().map(IntoChild::into_child).collect())
    }
}

impl<T> Children<T> {
    pub fn into_items(self) -> Vec<T> {
        self.0
    }
}

impl Children<ListChild> {
    fn mount(self, parent: NodeId) {
        let initial_sizes: Vec<ItemSize> = untrack(|| {
            self.0
                .iter()
                .map(|child| match &child.size {
                    Prop::Static(size) => *size,
                    Prop::Dynamic(read) => read(),
                })
                .collect()
        });
        with_document(|document| {
            for (child, size) in self.0.iter().zip(&initial_sizes) {
                document.append_child(parent, child.node, *size);
            }
        });
        for child in self.0 {
            let ListChild { node, size } = child;
            if let Prop::Dynamic(read) = size {
                create_effect(move || {
                    let size = read();
                    with_document(|document| document.set_child_size(parent, node, size));
                });
            }
        }
    }
}

impl Children<NodeId> {
    pub(crate) fn mount_scroll_items(self, scroll: NodeId) {
        with_document(|document| {
            for child in &self.0 {
                document.append_scroll_item(scroll, *child);
            }
        });
    }
}

#[diagnostic::on_unimplemented(
    message = "this component builds exactly one child",
    label = "write one child between these tags"
)]
pub trait OneChild<T> {
    fn one_child(self) -> T;
}

impl<T> OneChild<T> for [T; 1] {
    fn one_child(self) -> T {
        let [child] = self;
        child
    }
}

#[diagnostic::on_unimplemented(
    message = "this component builds at most one child",
    label = "write one child between these tags, or none at all"
)]
pub trait AtMostOneChild<T> {
    fn at_most_one_child(self) -> Option<T>;
}

impl<T> AtMostOneChild<T> for [T; 0] {
    fn at_most_one_child(self) -> Option<T> {
        None
    }
}

impl<T> AtMostOneChild<T> for [T; 1] {
    fn at_most_one_child(self) -> Option<T> {
        let [child] = self;
        Some(child)
    }
}

#[diagnostic::on_unimplemented(
    message = "this component hands a value to the children it builds",
    label = "write these children as a single closure taking that value"
)]
pub trait UnitHandle<H> {}

impl<F> UnitHandle<()> for F {}

pub use crate::base::canvas::{Canvas, CanvasItem, CanvasView};
pub use crate::base::click_catcher::ClickCatcher;
pub use crate::base::embed::{Embed, EmbedPlacement, EmbedSlot};
pub use crate::base::focusable::Focusable;
pub use crate::base::frame::Frame;
pub use crate::base::scroll::{Scroll, VirtualList};
pub use crate::base::text::Text;

#[component]
pub fn List(
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = Align::Stretch)] align: Prop<Align>,
    spacing: Prop<f32>,
    children: Children<ListChild>,
) -> NodeId {
    let list = with_document(|document| document.create_list(direction.peek(), 0.0));
    create_effect(move || {
        with_document(|document| document.set_list_direction(list, direction.get()))
    });
    create_effect(move || with_document(|document| document.set_list_align(list, align.get())));
    create_effect(move || with_document(|document| document.set_list_spacing(list, spacing.get())));
    children.mount(list);
    list
}

#[component]
pub fn Row(spacing: Prop<f32>, children: Children<ListChild>) -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing children />
    }
}

#[component]
pub fn Column(spacing: Prop<f32>, children: Children<ListChild>) -> NodeId {
    view! {
        <List direction=Direction::Vertical spacing children />
    }
}

#[component]
pub fn CenteredRow(spacing: Prop<f32>, children: Children<ListChild>) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing children />
    }
}

#[component]
pub fn Spacer() -> NodeId {
    with_document(Document::create_frame)
}

#[component]
pub fn Show(condition: Prop<bool>, #[prop(children)] then: Render) -> NodeId {
    let mut then = Some(then);
    let visibility = with_document(Document::create_frame);
    create_effect(move || {
        let visible = condition.get();
        if visible && let Some(build) = then.take() {
            let child = in_new_scope(|| build.call(()));
            with_document(|document| document.set_frame_child(visibility, child));
        }
        with_document(|document| document.set_visible(visibility, visible));
    });
    visibility
}

#[component]
pub fn Dynamic<T>(
    value: Prop<T>,
    #[prop(default = ItemSize::Intrinsic)] item_size: ItemSize,
    #[prop(children)] view: RenderFn<T>,
) -> NodeId
where
    T: Clone + 'static,
{
    let parent = view! {
        <Column spacing=0.0 />
    };
    let built: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
    create_effect(move || {
        let value = value.get();
        let child = in_new_scope(|| view.call(value));
        let previous = built.replace(Some(child));
        with_document(|document| {
            if let Some(previous) = previous {
                document.remove_child(parent, previous);
                document.remove_node(previous);
            }
            document.append_child(parent, child, item_size);
        });
    });
    parent
}

#[component]
pub fn ForEach<K>(
    spacing: f32,
    keys: Prop<Vec<K>>,
    #[prop(children)] view: RenderFn<K>,
    #[prop(default = ItemSize::Intrinsic)] item_size: ItemSize,
) -> NodeId
where
    K: Clone + Hash + Eq + 'static,
{
    let parent = view! {
        <Column spacing />
    };
    let existing: Rc<RefCell<HashMap<K, NodeId>>> = Rc::new(RefCell::new(HashMap::new()));
    create_effect(move || {
        let keys = keys.get();
        let mut existing = existing.borrow_mut();
        let mut next = HashMap::with_capacity(keys.len());
        let mut children = Vec::with_capacity(keys.len());
        for key in keys {
            let node = match existing.remove(&key) {
                Some(node) => node,
                None => {
                    let build = key.clone();
                    let view = view.clone();
                    in_new_scope(move || view.call(build))
                }
            };
            children.push((node, item_size));
            assert!(
                next.insert(key, node).is_none(),
                "for_each was given the same key twice"
            );
        }
        with_document(|document| {
            document.set_children(parent, &children);
            for removed in existing.values() {
                document.remove_node(*removed);
            }
        });
        *existing = next;
    });
    parent
}

#[component]
pub fn Keyed<T, K>(
    value: Prop<T>,
    key: Func<T, K>,
    #[prop(default = ItemSize::Intrinsic)] item_size: ItemSize,
    #[prop(children)] view: RenderFn<ReadSignal<T>>,
) -> NodeId
where
    T: Clone + PartialEq + 'static,
    K: PartialEq + 'static,
{
    let parent = view! {
        <Column spacing=0.0 />
    };
    let (current, set_current) = create_signal(value.peek());
    let built: Rc<RefCell<Option<(K, NodeId)>>> = Rc::new(RefCell::new(None));
    create_effect(move || {
        let value = value.get();
        let next = key.call(value.clone());
        set_current.set(value);
        let mut built = built.borrow_mut();
        if built.as_ref().is_some_and(|(key, _)| *key == next) {
            return;
        }
        let current = current.clone();
        let view = view.clone();
        let child = in_new_scope(move || view.call(current));
        let previous = built.replace((next, child));
        with_document(|document| {
            document.set_children(parent, &[(child, item_size)]);
            if let Some((_, previous)) = previous {
                document.remove_node(previous);
            }
        });
    });
    parent
}

#[component]
pub fn Button(
    children: Child,
    #[prop(default = false)] disabled: Prop<bool>,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::Button disabled on_click={move || on_click.call()} children />
    }
}

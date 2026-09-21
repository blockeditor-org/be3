use std::any::Any;
use std::cell::{Cell, RefCell};
use std::hash::Hash;
use std::marker::PhantomData;
use std::rc::Rc;

pub use crate::base::{Align, Direction, ItemSize};

use crate::base::child_list::{ChildHost, ChildList, SlotId};
use crate::base::list::{ListItem, ListNode};
use crate::base::offset::OffsetNode;
use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{ClickHandler, Handler, NodeId};
use crate::unstyled;

pub use beui_macros::{component, view};
pub use reactive::{
    Effect, KeyedItems, KeyedStore, Memo, ReadSignal, Scope, ScopeContext, Selector, Store,
    WriteSignal, batch, clone, create_effect, create_memo, create_selector, create_signal,
    on_cleanup, owner_scope, provide_context, settle, untrack, use_context,
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
    try_with_document(f).expect(
        "beui::reactive binding used without an active document; \
         call it from inside build(), or from inside event dispatch",
    )
}

pub fn try_with_document<R>(f: impl FnOnce(&mut Document) -> R) -> Option<R> {
    CURRENT_DOCUMENT.with(|cell| match cell.try_borrow_mut() {
        Ok(mut slot) => {
            let document = slot.as_mut()?;
            let ptr: *mut Document = document;
            ACTIVE_DOCUMENT.with(|active| active.set(ptr));
            let _guard = ActiveDocumentGuard;
            Some(f(document))
        }
        Err(_) => {
            let ptr = ACTIVE_DOCUMENT.with(Cell::get);
            (!ptr.is_null()).then(|| f(unsafe { &mut *ptr }))
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

pub fn each_frame(work: impl Fn() + 'static) {
    let work: Rc<dyn Fn()> = Rc::new(work);
    with_document(|document| document.register_frame_hook(Rc::downgrade(&work)));
    on_cleanup(move || drop(work));
}

pub fn on_shortcut(shortcut: impl Fn(crate::input::KeyPress) -> bool + 'static) {
    let shortcut: Rc<crate::document::Shortcut> = Rc::new(shortcut);
    with_document(|document| document.register_shortcut(Rc::downgrade(&shortcut)));
    on_cleanup(move || drop(shortcut));
}

pub fn bind_test_id(node: NodeId, test_id: Prop<String>) {
    let reading = match test_id {
        Prop::Static(value) => {
            with_document(|document| document.set_test_id(node, value));
            return;
        }
        Prop::Dynamic(reading) => reading,
    };
    let scope = with_document(|document| node_scope(document, None));
    scope.context().run(|| {
        let published: Cell<Option<String>> = Cell::new(None);
        create_effect(move || {
            let next = reading();
            let previous = published.replace(Some(next.clone()));
            with_document(|document| {
                if let Some(previous) = previous
                    && previous != next
                {
                    document.clear_test_id(node, &previous);
                }
                document.set_test_id(node, next);
            });
        });
    });
    with_document(|document| document.register_node_scope(node, scope));
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
    let mut value = in_component(Some(context.clone()), || scope.run(f));
    let anchor = value.anchor();
    context.target.set(anchor);
    let states = context.states.take();
    let accessibility = context.accessibility.take();
    let size = context.size.take();
    let placement = context.placement.take();
    match anchor {
        Some(root) => with_document(|document| {
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
        }),
        None => assert!(
            states.is_empty() && accessibility.is_none() && size.is_none() && placement.is_none(),
            "a component that builds no node has nothing for `component_state`, \
             `component_accessibility`, `component_size` or `component_rect` to watch"
        ),
    }
    value.adopt_scope(scope);
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

pub struct Render<H = (), C = NodeId> {
    owner: Option<Rc<ComponentContext>>,
    render: Box<dyn FnOnce(H) -> C>,
}

impl<H, C> Render<H, C> {
    pub fn new(render: impl FnOnce(H) -> C + 'static) -> Self {
        Self {
            owner: CURRENT_COMPONENT.with(|cell| cell.borrow().clone()),
            render: Box::new(render),
        }
    }

    pub fn call(self, handle: H) -> C {
        in_component(self.owner, || (self.render)(handle))
    }
}

pub struct RenderFn<H, C = NodeId> {
    owner: Option<Rc<ComponentContext>>,
    render: Rc<dyn Fn(H) -> C>,
}

impl<H, C> RenderFn<H, C> {
    pub fn new(render: impl Fn(H) -> C + 'static) -> Self {
        Self {
            owner: CURRENT_COMPONENT.with(|cell| cell.borrow().clone()),
            render: Rc::new(render),
        }
    }

    pub fn call(&self, handle: H) -> C {
        in_component(self.owner.clone(), || (self.render)(handle))
    }
}

impl<H, C> Clone for RenderFn<H, C> {
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

pub trait IntoRender<H, C> {
    fn into_render(self) -> Render<H, C>;
}

impl<H, C, R: IntoChild<C>, F: FnOnce(H) -> R + 'static> IntoRender<H, C> for F {
    fn into_render(self) -> Render<H, C> {
        Render::new(move |handle| self(handle).into_child())
    }
}

impl<H, C> IntoRender<H, C> for Render<H, C> {
    fn into_render(self) -> Render<H, C> {
        self
    }
}

pub trait IntoRenderFn<H, C> {
    fn into_render_fn(self) -> RenderFn<H, C>;
}

impl<H, C, R: IntoChild<C>, F: Fn(H) -> R + 'static> IntoRenderFn<H, C> for F {
    fn into_render_fn(self) -> RenderFn<H, C> {
        RenderFn::new(move |handle| self(handle).into_child())
    }
}

impl<H, C> IntoRenderFn<H, C> for RenderFn<H, C> {
    fn into_render_fn(self) -> RenderFn<H, C> {
        self
    }
}

pub enum Prop<T> {
    Static(T),
    Dynamic(Rc<dyn Fn() -> T>),
}

impl<T: Clone> Clone for Prop<T> {
    fn clone(&self) -> Self {
        match self {
            Prop::Static(value) => Prop::Static(value.clone()),
            Prop::Dynamic(read) => Prop::Dynamic(Rc::clone(read)),
        }
    }
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
            Prop::Dynamic(read) => Prop::Dynamic(Rc::new(move || f(read()))),
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
        Prop::Dynamic(Rc::new(move || self.get()))
    }
}

impl<T: Clone + PartialEq + 'static> IntoProp<T> for Memo<T> {
    fn into_prop(self) -> Prop<T> {
        Prop::Dynamic(Rc::new(move || self.get()))
    }
}

pub type Child = NodeId;

#[diagnostic::on_unimplemented(
    message = "a `#[component]` function returns a node, a value built around one, or a value that keeps a `ChildScope`",
    label = "implement `ChildValue` for this component's return type"
)]
pub trait ChildValue {
    fn anchor(&self) -> Option<NodeId>;
    fn adopt_scope(&mut self, scope: Scope);
}

impl ChildValue for NodeId {
    fn anchor(&self) -> Option<NodeId> {
        Some(*self)
    }

    fn adopt_scope(&mut self, scope: Scope) {
        let node = *self;
        with_document(|document| document.register_node_scope(node, scope));
    }
}

impl ChildValue for ListChild {
    fn anchor(&self) -> Option<NodeId> {
        Some(self.node)
    }

    fn adopt_scope(&mut self, scope: Scope) {
        self.node.adopt_scope(scope);
    }
}

#[derive(Clone, Default)]
pub struct ChildScope(Option<Rc<Scope>>);

impl ChildScope {
    pub fn adopt(&mut self, scope: Scope) {
        self.0 = Some(Rc::new(scope));
    }

    pub fn is_alive(&self) -> bool {
        self.0.as_ref().is_some_and(|scope| !scope.is_disposed())
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

    fn watch(self, parent: Option<NodeId>) -> (NodeId, ItemSize) {
        let initial = self.size.peek();
        let ListChild { node, size } = self;
        if let (Some(parent), Prop::Dynamic(read)) = (parent, size) {
            create_effect(move || {
                let size = read();
                with_document(|document| document.set_child_size(parent, node, size));
            });
        }
        (node, initial)
    }
}

fn build_in_slot<C: SlotChild>(
    slot: &ChildSlot<C>,
    build: impl FnOnce() -> C,
) -> (C::Stored, Scope) {
    let scope = Scope::detached();
    let stored = scope.context().run(|| slot.store(build()));
    slot.fill(vec![stored.clone()]);
    (stored, scope)
}

fn discard_previous<C: SlotChild>(previous: Option<(C::Stored, Scope)>) {
    let Some((stored, scope)) = previous else {
        return;
    };
    ChildSlot::<C>::discard(&stored);
    drop(scope);
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
    label = "remove `@sizing`, or put this child in a `Row`, `Column`, `List` or `Stack`"
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

pub enum ChildSegment<T: SlotChild> {
    One(T),
    Many(Vec<T>),
    Nested(Vec<ChildSegment<T>>),
    Dynamic(DynamicSegment<T>),
}

impl<T: SlotChild> ChildSegment<T> {
    fn items(self) -> Vec<T> {
        match self {
            ChildSegment::One(item) => vec![item],
            ChildSegment::Many(items) => items,
            ChildSegment::Nested(segments) => {
                segments.into_iter().flat_map(ChildSegment::items).collect()
            }
            ChildSegment::Dynamic(_) => panic!(
                "a show, a dynamic or a keyed run builds however many children its value asks \
                 for, and this slot takes a fixed child; put it in a `Column` or hand it to a \
                 slot that takes a run"
            ),
        }
    }

    fn map(self, change: &Rc<dyn Fn(T) -> T>) -> Self {
        match self {
            ChildSegment::One(item) => ChildSegment::One(change(item)),
            ChildSegment::Many(items) => {
                ChildSegment::Many(items.into_iter().map(|item| change(item)).collect())
            }
            ChildSegment::Nested(segments) => ChildSegment::Nested(
                segments
                    .into_iter()
                    .map(|segment| segment.map(change))
                    .collect(),
            ),
            ChildSegment::Dynamic(segment) => ChildSegment::Dynamic(segment.map(change.clone())),
        }
    }
}

pub struct DynamicSegment<T: SlotChild> {
    scope: Option<Scope>,
    install: Box<dyn FnOnce(ChildSlot<T>)>,
    child: PhantomData<T>,
}

impl<T: SlotChild> DynamicSegment<T> {
    pub(crate) fn new(install: impl FnOnce(ChildSlot<T>) + 'static) -> Self {
        Self {
            scope: None,
            install: Box::new(install),
            child: PhantomData,
        }
    }

    fn map(self, change: Rc<dyn Fn(T) -> T>) -> Self {
        let DynamicSegment { scope, install, .. } = self;
        Self {
            scope,
            install: Box::new(move |slot: ChildSlot<T>| install(slot.mapping(change))),
            child: PhantomData,
        }
    }

    fn mount(self, slot: ChildSlot<T>) {
        let DynamicSegment { scope, install, .. } = self;
        match scope {
            Some(scope) => {
                let held = slot.clone();
                scope.context().run(move || install(slot));
                held.adopt_scope(scope);
            }
            None => install(slot),
        }
    }
}

impl<T: SlotChild> ChildValue for DynamicSegment<T> {
    fn anchor(&self) -> Option<NodeId> {
        None
    }

    fn adopt_scope(&mut self, scope: Scope) {
        self.scope = Some(scope);
    }
}

impl<T: SlotChild> IntoSegment<T> for DynamicSegment<T> {
    fn into_segment(self) -> ChildSegment<T> {
        ChildSegment::Dynamic(self)
    }
}

#[diagnostic::on_unimplemented(
    message = "a `{Self}` is not something a parent can keep a run of",
    label = "say how a run of these is kept with `child_type!` or `value_child_type!`"
)]
pub trait SlotChild: Sized + 'static {
    type Stored: Clone + 'static;

    fn store(self, parent: Option<NodeId>) -> Self::Stored;
    fn stored_node(stored: &Self::Stored) -> Option<NodeId>;
}

#[diagnostic::on_unimplemented(
    message = "a run of `{Self}` children is not kept by a node",
    label = "this parent keeps its children in a node, and `{Self}` is no node"
)]
pub(crate) trait NodeSlot: SlotChild {
    type Host: ChildHost<Stored = Self::Stored>;

    fn open_slot(parent: NodeId) -> SlotId {
        with_document(|document| document.open_child_slot::<Self::Host>(parent))
    }

    fn fill_slot(parent: NodeId, slot: SlotId, items: Vec<Self::Stored>) {
        with_document(|document| document.fill_child_slot::<Self::Host>(parent, slot, items));
    }

    fn append(parent: NodeId, stored: Self::Stored) {
        with_document(|document| document.append_child_item::<Self::Host>(parent, stored));
    }
}

impl SlotChild for ListChild {
    type Stored = ListItem;

    fn store(self, parent: Option<NodeId>) -> ListItem {
        let (child, size) = self.watch(parent);
        ListItem { child, size }
    }

    fn stored_node(stored: &ListItem) -> Option<NodeId> {
        Some(stored.child)
    }
}

impl NodeSlot for ListChild {
    type Host = ListNode;
}

impl SlotChild for NodeId {
    type Stored = NodeId;

    fn store(self, _parent: Option<NodeId>) -> NodeId {
        self
    }

    fn stored_node(stored: &NodeId) -> Option<NodeId> {
        Some(*stored)
    }
}

impl NodeSlot for NodeId {
    type Host = OffsetNode;
}

struct RunState<S> {
    items: RefCell<ChildList<S>>,
    held: RefCell<Vec<Scope>>,
    filled: ReadSignal<u64>,
    fill: WriteSignal<u64>,
}

impl<S> RunState<S> {
    fn new() -> Self {
        let (filled, fill) = create_signal(0);
        Self {
            items: RefCell::new(ChildList::default()),
            held: RefCell::new(Vec::new()),
            filled,
            fill,
        }
    }

    fn changed(&self) {
        self.fill.update(|filled| *filled += 1);
    }
}

pub struct Run<C: SlotChild> {
    state: Rc<RunState<C::Stored>>,
}

impl<C: SlotChild> Clone for Run<C> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl<C: SlotChild> Default for Run<C> {
    fn default() -> Self {
        Self {
            state: Rc::new(RunState::new()),
        }
    }
}

impl<C: SlotChild> Run<C> {
    pub fn items(&self) -> Vec<C::Stored> {
        self.state.filled.get();
        self.peek()
    }

    pub fn peek(&self) -> Vec<C::Stored> {
        self.state.items.borrow().iter().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.state.filled.get();
        self.state.items.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn build<R: SlotChild, U: IntoChild<R>>(
        &self,
        rows: impl Fn(Vec<C::Stored>) -> Vec<U> + 'static,
    ) -> DynamicSegment<R> {
        let run = self.clone();
        DynamicSegment::new(move |slot: ChildSlot<R>| {
            let held: HeldRows<R> = Rc::new(RefCell::new(None));
            let owned = held.clone();
            on_cleanup(move || drop(owned));
            create_effect(move || {
                let items = run.items();
                let scope = Scope::detached();
                let built: Vec<R::Stored> = scope.context().run(|| {
                    rows(items)
                        .into_iter()
                        .map(|row| slot.store(row.into_child()))
                        .collect()
                });
                slot.fill(built.clone());
                let previous = held.borrow_mut().replace((built, scope));
                if let Some((stored, scope)) = previous {
                    for row in &stored {
                        ChildSlot::<R>::discard(row);
                    }
                    drop(scope);
                }
            });
        })
    }
}

impl<C: SlotChild> IntoProp<Vec<C::Stored>> for Run<C> {
    fn into_prop(self) -> Prop<Vec<C::Stored>> {
        Prop::Dynamic(Rc::new(move || self.items()))
    }
}

enum Place<S> {
    Node {
        parent: NodeId,
        slot: SlotId,
        fill: fn(NodeId, SlotId, Vec<S>),
    },
    Run {
        run: Rc<RunState<S>>,
        slot: SlotId,
    },
}

impl<S> Clone for Place<S> {
    fn clone(&self) -> Self {
        match self {
            Place::Node { parent, slot, fill } => Place::Node {
                parent: *parent,
                slot: *slot,
                fill: *fill,
            },
            Place::Run { run, slot } => Place::Run {
                run: run.clone(),
                slot: *slot,
            },
        }
    }
}

pub struct ChildSlot<C: SlotChild> {
    place: Place<C::Stored>,
    change: Option<Rc<dyn Fn(C) -> C>>,
}

impl<C: SlotChild> Clone for ChildSlot<C> {
    fn clone(&self) -> Self {
        Self {
            place: self.place.clone(),
            change: self.change.clone(),
        }
    }
}

impl<C: SlotChild> ChildSlot<C> {
    fn in_node(parent: NodeId) -> Self
    where
        C: NodeSlot,
    {
        Self {
            place: Place::Node {
                parent,
                slot: C::open_slot(parent),
                fill: C::fill_slot,
            },
            change: None,
        }
    }

    fn in_run(run: &Run<C>) -> Self {
        let slot = run.state.items.borrow_mut().open();
        Self {
            place: Place::Run {
                run: run.state.clone(),
                slot,
            },
            change: None,
        }
    }

    fn mapping(self, change: Rc<dyn Fn(C) -> C>) -> Self {
        let inner = self.change;
        let change: Rc<dyn Fn(C) -> C> = match inner {
            Some(inner) => Rc::new(move |child| change(inner(child))),
            None => change,
        };
        Self {
            place: self.place,
            change: Some(change),
        }
    }

    pub fn parent(&self) -> Option<NodeId> {
        match &self.place {
            Place::Node { parent, .. } => Some(*parent),
            Place::Run { .. } => None,
        }
    }

    pub fn store(&self, child: C) -> C::Stored {
        let child = match &self.change {
            Some(change) => change(child),
            None => child,
        };
        child.store(self.parent())
    }

    pub fn fill(&self, items: Vec<C::Stored>) {
        match &self.place {
            Place::Node { parent, slot, fill } => fill(*parent, *slot, items),
            Place::Run { run, slot } => {
                run.items.borrow_mut().fill(*slot, items);
                run.changed();
            }
        }
    }

    pub fn discard(stored: &C::Stored) {
        let Some(node) = C::stored_node(stored) else {
            return;
        };
        try_with_document(|document| document.remove_node(node));
    }

    fn adopt_scope(&self, scope: Scope) {
        match &self.place {
            Place::Node { parent, .. } => {
                let parent = *parent;
                with_document(|document| document.register_node_scope(parent, scope));
            }
            Place::Run { run, .. } => run.held.borrow_mut().push(scope),
        }
    }
}

#[diagnostic::on_unimplemented(
    message = "a `{Self}` cannot be written between these tags",
    label = "this component takes `{T}` children"
)]
pub trait IntoSegment<T: SlotChild> {
    fn into_segment(self) -> ChildSegment<T>;
}

pub fn into_segment<T: SlotChild>(child: impl IntoSegment<T>) -> ChildSegment<T> {
    child.into_segment()
}

fn one_segment<T: SlotChild>(child: impl IntoChild<T>) -> ChildSegment<T> {
    ChildSegment::One(child.into_child())
}

impl<T: SlotChild> IntoSegment<T> for Children<T> {
    fn into_segment(self) -> ChildSegment<T> {
        ChildSegment::Nested(self.0)
    }
}

impl IntoSegment<NodeId> for NodeId {
    fn into_segment(self) -> ChildSegment<NodeId> {
        one_segment(self)
    }
}

impl IntoSegment<ListChild> for NodeId {
    fn into_segment(self) -> ChildSegment<ListChild> {
        one_segment(self)
    }
}

impl<T: AcceptsSizing + SlotChild> IntoSegment<T> for ListChild {
    fn into_segment(self) -> ChildSegment<T> {
        one_segment(self)
    }
}

#[macro_export]
macro_rules! child_type {
    ($ty:ty) => {
        impl $crate::reactive::IntoChild<$ty> for $ty {
            fn into_child(self) -> $ty {
                self
            }
        }

        impl $crate::reactive::IntoSegment<$ty> for $ty {
            fn into_segment(self) -> $crate::reactive::ChildSegment<$ty> {
                $crate::reactive::ChildSegment::One(self)
            }
        }

        impl ::core::convert::From<$ty> for $crate::reactive::Children<$ty> {
            fn from(child: $ty) -> Self {
                Self::from($crate::reactive::ChildSegment::One(child))
            }
        }
    };
}

#[macro_export]
macro_rules! value_child_type {
    ($ty:ty) => {
        $crate::child_type!($ty);

        impl $crate::reactive::SlotChild for $ty {
            type Stored = ::std::rc::Rc<$ty>;

            fn store(self, _parent: ::core::option::Option<$crate::node::NodeId>) -> Self::Stored {
                ::std::rc::Rc::new(self)
            }

            fn stored_node(_stored: &Self::Stored) -> ::core::option::Option<$crate::node::NodeId> {
                ::core::option::Option::None
            }
        }
    };
}

pub struct Children<T: SlotChild>(Vec<ChildSegment<T>>);

#[diagnostic::on_unimplemented(
    message = "a `{Self}` does not name a kind of child",
    label = "type this prop `Children<_>`"
)]
pub trait ChildrenSlot {
    type Child: SlotChild;
}

impl<T: SlotChild> ChildrenSlot for Children<T> {
    type Child = T;
}

impl<T: SlotChild> Default for Children<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: SlotChild, const N: usize> From<[ChildSegment<T>; N]> for Children<T> {
    fn from(segments: [ChildSegment<T>; N]) -> Self {
        Self(Vec::from(segments))
    }
}

impl<T: SlotChild> From<ChildSegment<T>> for Children<T> {
    fn from(segment: ChildSegment<T>) -> Self {
        Self(vec![segment])
    }
}

impl From<NodeId> for Children<NodeId> {
    fn from(node: NodeId) -> Self {
        Self::from(node.into_segment())
    }
}

impl From<NodeId> for Children<ListChild> {
    fn from(node: NodeId) -> Self {
        Self::from(IntoSegment::<ListChild>::into_segment(node))
    }
}

impl<T: AcceptsSizing + SlotChild> From<ListChild> for Children<T> {
    fn from(child: ListChild) -> Self {
        Self::from(child.into_segment())
    }
}

impl<T: SlotChild> From<Run<T>> for Children<T> {
    fn from(run: Run<T>) -> Self {
        Self::from(ChildSegment::Dynamic(DynamicSegment::new(
            move |slot: ChildSlot<T>| {
                let held = slot.clone();
                held.fill(run.peek());
                create_effect(move || slot.fill(run.items()));
            },
        )))
    }
}

impl<T: SlotChild> From<DynamicSegment<T>> for Children<T> {
    fn from(segment: DynamicSegment<T>) -> Self {
        Self::from(segment.into_segment())
    }
}

impl<T: SlotChild> Children<T> {
    pub fn map(self, change: impl Fn(T) -> T + 'static) -> Self {
        let change: Rc<dyn Fn(T) -> T> = Rc::new(change);
        Self(
            self.0
                .into_iter()
                .map(|segment| segment.map(&change))
                .collect(),
        )
    }

    pub fn into_run(self) -> Run<T> {
        let run = Run::default();
        for segment in self.0 {
            fill_run(&run, segment);
        }
        run
    }
}

fn fill_run<T: SlotChild>(run: &Run<T>, segment: ChildSegment<T>) {
    match segment {
        ChildSegment::One(child) => run.state.items.borrow_mut().push(child.store(None)),
        ChildSegment::Many(children) => {
            for child in children {
                run.state.items.borrow_mut().push(child.store(None));
            }
        }
        ChildSegment::Nested(segments) => {
            for segment in segments {
                fill_run(run, segment);
            }
        }
        ChildSegment::Dynamic(segment) => segment.mount(ChildSlot::in_run(run)),
    }
}

impl<T: SlotChild> Children<T> {
    pub(crate) fn mount(self, parent: NodeId)
    where
        T: NodeSlot,
    {
        for segment in self.0 {
            mount_segment(parent, segment);
        }
    }
}

fn mount_segment<T: NodeSlot>(parent: NodeId, segment: ChildSegment<T>) {
    match segment {
        ChildSegment::One(child) => T::append(parent, child.store(Some(parent))),
        ChildSegment::Many(children) => {
            for child in children {
                T::append(parent, child.store(Some(parent)));
            }
        }
        ChildSegment::Nested(segments) => {
            for segment in segments {
                mount_segment(parent, segment);
            }
        }
        ChildSegment::Dynamic(segment) => segment.mount(ChildSlot::in_node(parent)),
    }
}

#[diagnostic::on_unimplemented(
    message = "this component builds exactly one child",
    label = "write one child between these tags"
)]
pub trait OneChild<T> {
    fn one_child(self) -> T;
}

impl<T: SlotChild> OneChild<T> for [ChildSegment<T>; 1] {
    fn one_child(self) -> T {
        let [child] = self;
        let mut items = child.items();
        assert_eq!(
            items.len(),
            1,
            "this component builds exactly one child, and what is written between its tags builds {} of them",
            items.len()
        );
        items.remove(0)
    }
}

#[diagnostic::on_unimplemented(
    message = "this component builds at most one child",
    label = "write one child between these tags, or none at all"
)]
pub trait AtMostOneChild<T> {
    fn at_most_one_child(self) -> Option<T>;
}

impl<T: SlotChild> AtMostOneChild<T> for [ChildSegment<T>; 0] {
    fn at_most_one_child(self) -> Option<T> {
        None
    }
}

impl<T: SlotChild> AtMostOneChild<T> for [ChildSegment<T>; 1] {
    fn at_most_one_child(self) -> Option<T> {
        let [child] = self;
        let mut items = child.items();
        assert!(
            items.len() <= 1,
            "this component builds at most one child, and what is written between its tags builds {} of them",
            items.len()
        );
        items.pop()
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
pub use crate::base::drawing::{Draw, Drawing};
pub use crate::base::embed::{Embed, EmbedPlacement, EmbedSlot};
pub use crate::base::focusable::Focusable;
pub use crate::base::frame::Frame;
pub use crate::base::offset::{Offset, VirtualOffset};
pub use crate::base::picture::Picture;
pub use crate::base::portal::Portal;
pub use crate::base::stroke::Stroke;
pub use crate::base::text::Text;
pub use crate::base::viewport::Viewport;

#[component]
pub fn List(
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = Align::Stretch)] align: Prop<Align>,
    #[prop(default = false)] wrap: Prop<bool>,
    spacing: Prop<f32>,
    children: Children<ListChild>,
) -> NodeId {
    let list = with_document(|document| document.create_list(direction.peek(), 0.0));
    create_effect(move || {
        with_document(|document| document.set_list_direction(list, direction.get()))
    });
    create_effect(move || with_document(|document| document.set_list_align(list, align.get())));
    create_effect(move || with_document(|document| document.set_list_wrap(list, wrap.get())));
    create_effect(move || with_document(|document| document.set_list_spacing(list, spacing.get())));
    children.mount(list);
    list
}

#[component]
pub fn Spacer() -> NodeId {
    with_document(Document::create_frame)
}

type HeldChild<C> = Rc<RefCell<Option<(<C as SlotChild>::Stored, Scope)>>>;
type KeyedChild<K, C> = Rc<RefCell<Option<(K, <C as SlotChild>::Stored, Scope)>>>;
type HeldRows<R> = Rc<RefCell<Option<(Vec<<R as SlotChild>::Stored>, Scope)>>>;

#[component]
pub fn Show<C>(condition: Prop<bool>, #[prop(children)] then: Render<(), C>) -> DynamicSegment<C>
where
    C: SlotChild,
{
    DynamicSegment::new(move |slot: ChildSlot<C>| {
        let held: HeldChild<C> = Rc::new(RefCell::new(None));
        let parked = held.clone();
        on_cleanup(move || {
            let held = parked.borrow_mut().take();
            if let Some((stored, _)) = held {
                ChildSlot::<C>::discard(&stored);
            }
        });
        let mut then = Some(then);
        create_effect(move || {
            let visible = condition.get();
            let mut held = held.borrow_mut();
            if visible
                && held.is_none()
                && let Some(build) = then.take()
            {
                let scope = Scope::detached();
                let stored = scope.context().run(|| slot.store(build.call(())));
                *held = Some((stored, scope));
            }
            let items = match (visible, held.as_ref()) {
                (true, Some((stored, _))) => vec![stored.clone()],
                _ => Vec::new(),
            };
            slot.fill(items);
        });
    })
}

#[component]
pub fn Dynamic<T, C>(value: Prop<T>, #[prop(children)] view: RenderFn<T, C>) -> DynamicSegment<C>
where
    T: Clone + 'static,
    C: SlotChild,
{
    DynamicSegment::new(move |slot: ChildSlot<C>| {
        let held: HeldChild<C> = Rc::new(RefCell::new(None));
        let owned = held.clone();
        on_cleanup(move || drop(owned));
        create_effect(move || {
            let value = value.get();
            let mut held = held.borrow_mut();
            let built = build_in_slot(&slot, || view.call(value));
            discard_previous::<C>(held.replace(built));
        });
    })
}

#[component]
pub fn ForEach<K, C>(
    keys: Prop<Vec<K>>,
    #[prop(children)] view: RenderFn<K, C>,
) -> DynamicSegment<C>
where
    K: Clone + Hash + Eq + 'static,
    C: SlotChild,
{
    DynamicSegment::new(move |slot: ChildSlot<C>| {
        let built = slot.clone();
        let rows = Rc::new(KeyedItems::new(move |key: K| built.store(view.call(key))));
        let owned = rows.clone();
        on_cleanup(move || drop(owned));
        create_effect(move || {
            let keys = keys.get();
            rows.map(keys).commit(|items, removed| {
                slot.fill(items);
                for row in &removed {
                    ChildSlot::<C>::discard(row);
                }
            });
        });
    })
}

#[component]
pub fn Keyed<T, K, C>(
    value: Prop<T>,
    key: Func<T, K>,
    #[prop(children)] view: RenderFn<ReadSignal<T>, C>,
) -> DynamicSegment<C>
where
    T: Clone + PartialEq + 'static,
    K: PartialEq + 'static,
    C: SlotChild,
{
    DynamicSegment::new(move |slot: ChildSlot<C>| {
        let (current, set_current) = create_signal(value.peek());
        let held: KeyedChild<K, C> = Rc::new(RefCell::new(None));
        let owned = held.clone();
        on_cleanup(move || drop(owned));
        create_effect(move || {
            let value = value.get();
            let next = key.call(value.clone());
            set_current.set(value);
            let mut held = held.borrow_mut();
            if held.as_ref().is_some_and(|(key, _, _)| *key == next) {
                return;
            }
            let current = current.clone();
            let view = view.clone();
            let (stored, scope) = build_in_slot(&slot, move || view.call(current));
            let previous = held
                .replace((next, stored, scope))
                .map(|(_, stored, scope)| (stored, scope));
            discard_previous::<C>(previous);
        });
    })
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

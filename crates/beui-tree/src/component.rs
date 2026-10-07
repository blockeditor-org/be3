use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::rc::Rc;

use reactive::Scope;

use crate::children::IntoChild;

thread_local! {
    static CURRENT_COMPONENT: RefCell<Option<Rc<ComponentContext>>> = const { RefCell::new(None) };
}

pub struct ComponentContext {
    name: &'static str,
    extensions: RefCell<Vec<(TypeId, Rc<dyn Any>)>>,
}

impl ComponentContext {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            extensions: RefCell::new(Vec::new()),
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn extension<T: Default + 'static>(&self) -> Rc<T> {
        if let Some(found) = self.find_extension::<T>() {
            return found;
        }
        let made = Rc::new(T::default());
        self.extensions
            .borrow_mut()
            .push((TypeId::of::<T>(), made.clone()));
        made
    }

    pub fn find_extension<T: 'static>(&self) -> Option<Rc<T>> {
        let found = self
            .extensions
            .borrow()
            .iter()
            .find(|(id, _)| *id == TypeId::of::<T>())
            .map(|(_, extension)| extension.clone())?;
        Some(
            found
                .downcast::<T>()
                .unwrap_or_else(|_| unreachable!("an extension is kept under its own type id")),
        )
    }

    pub fn has_extensions(&self) -> bool {
        !self.extensions.borrow().is_empty()
    }
}

pub fn current_component() -> Rc<ComponentContext> {
    try_current_component().expect("a component binding was used outside of a #[component] body")
}

pub fn try_current_component() -> Option<Rc<ComponentContext>> {
    CURRENT_COMPONENT.with(|cell| cell.borrow().clone())
}

#[diagnostic::on_unimplemented(
    message = "a `#[component]` function returns a node, a value built around one, or a value that keeps a `ChildScope`",
    label = "implement `ChildValue` for this component's return type"
)]
pub trait ChildValue {
    fn adopt_scope(&mut self, scope: Scope);

    fn finish_component(&mut self, component: &ComponentContext, _scope: &Scope) {
        assert!(
            !component.has_extensions(),
            "component `{}` returns a value that is no node, so the bindings its body made \
             (such as `component_size` or `component_state`) have nothing to attach to",
            component.name(),
        );
    }
}

#[derive(Clone, Default)]
pub struct ChildScope(Vec<Rc<Scope>>);

impl ChildScope {
    pub fn adopt(&mut self, scope: Scope) {
        self.0.push(Rc::new(scope));
    }

    pub fn is_alive(&self) -> bool {
        self.0.last().is_some_and(|scope| !scope.is_disposed())
    }
}

pub fn component<T: ChildValue>(name: &'static str, f: impl FnOnce() -> T) -> T {
    let scope = Scope::new();
    let context = Rc::new(ComponentContext::new(name));
    let mut value = in_component(Some(context.clone()), || scope.run(f));
    value.finish_component(&context, &scope);
    value.adopt_scope(scope);
    value
}

pub trait ComponentBuilder: Sized {
    type Output;

    fn with_built(self, bind: impl FnOnce(&Self::Output) + 'static) -> Self;
}

pub struct MissingProp;

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

pub struct Render<H, C> {
    owner: Option<Rc<ComponentContext>>,
    render: Box<dyn FnOnce(H) -> C>,
}

impl<H, C> Render<H, C> {
    pub fn new(render: impl FnOnce(H) -> C + 'static) -> Self {
        Self {
            owner: try_current_component(),
            render: Box::new(render),
        }
    }

    pub fn call(self, handle: H) -> C {
        in_component(self.owner, || (self.render)(handle))
    }
}

pub struct RenderFn<H, C> {
    owner: Option<Rc<ComponentContext>>,
    render: Rc<dyn Fn(H) -> C>,
}

impl<H, C> RenderFn<H, C> {
    pub fn new(render: impl Fn(H) -> C + 'static) -> Self {
        Self {
            owner: try_current_component(),
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

#[diagnostic::on_unimplemented(
    message = "this component hands a value to the children it builds",
    label = "write these children as a single closure taking that value"
)]
pub trait UnitHandle<H> {}

impl<F> UnitHandle<()> for F {}

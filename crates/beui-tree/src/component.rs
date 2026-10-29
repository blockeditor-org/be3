use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::rc::Rc;

use reactive::{ScopeContext, Stored, child_scope, component_body, store};

use crate::children::IntoChild;

thread_local! {
    static CURRENT_COMPONENT: RefCell<Option<Rc<ComponentContext>>> = const { RefCell::new(None) };
}

#[derive(Debug)]
pub struct ComponentInfo {
    pub name: &'static str,
    pub module: &'static str,
}

impl ComponentInfo {
    pub fn label(&self) -> String {
        let (prefix, rest) = self.module.split_once("::").unwrap_or((self.module, ""));
        match prefix {
            "beui_components_styled" => format!("styled::{}", self.name),
            "beui_components_unstyled" => format!("unstyled::{}", self.name),
            "beui_view" => format!("primitives::{}", self.name),
            "beui_dock" | "beui_keyboard" => {
                let layer = rest.split("::").next().unwrap_or(rest);
                let facade = prefix.trim_start_matches("beui_");
                format!("{facade}::{layer}::{}", self.name)
            }
            _ if rest.is_empty() => format!("{prefix}::{}", self.name),
            _ => format!("{}::{}", self.module, self.name),
        }
    }

    pub fn internal_by_default(&self) -> bool {
        self.module
            .split("::")
            .next()
            .is_some_and(|name| name.starts_with("beui_") && name != "beui_host")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ComponentRecord {
    pub info: &'static ComponentInfo,
    pub id: u64,
    pub owner: Option<u64>,
}

pub struct ComponentContext {
    record: ComponentRecord,
    extensions: RefCell<Vec<(TypeId, Rc<dyn Any>)>>,
}

impl ComponentContext {
    fn new(info: &'static ComponentInfo) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self {
            record: ComponentRecord {
                info,
                id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                owner: try_current_component().map(|owner| owner.record.id),
            },
            extensions: RefCell::new(Vec::new()),
        }
    }

    pub fn name(&self) -> &'static str {
        self.record.info.name
    }

    pub fn record(&self) -> ComponentRecord {
        self.record
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
    message = "a `#[component]` function returns a node, a value built around one, or a child value",
    label = "implement `ChildValue` for this component's return type"
)]
pub trait ChildValue {
    fn finish_component(&mut self, component: &ComponentContext, _scope: &ScopeContext) {
        assert!(
            !component.has_extensions(),
            "component `{}` returns a value that is no node, so the bindings its body made \
             (such as `component_placed` or `component_state`) have nothing to attach to",
            component.name(),
        );
    }
}

impl<T: ChildValue> ChildValue for Option<T> {
    fn finish_component(&mut self, component: &ComponentContext, scope: &ScopeContext) {
        if let Some(value) = self {
            value.finish_component(component, scope);
        } else {
            assert!(
                !component.has_extensions(),
                "an empty component has no node to attach bindings to"
            );
        }
    }
}

pub fn component<T: ChildValue>(info: &'static ComponentInfo, f: impl FnOnce() -> T) -> T {
    let context = Rc::new(ComponentContext::new(info));
    let (mut value, scope) =
        child_scope(|| in_component(Some(context.clone()), || component_body(f)));
    value.finish_component(&context, &scope);
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

    pub fn once(self) -> RenderFn<H, C>
    where
        H: 'static,
        C: 'static,
    {
        let held = std::cell::Cell::new(Some(self));
        RenderFn::new(move |handle| {
            held.take()
                .expect("a render made with `once` is built at most once, by a slot that keeps what it built")
                .call(handle)
        })
    }
}

type RenderFnSlot<H, C> = (Option<Rc<ComponentContext>>, Box<dyn Fn(H) -> C>);

pub struct RenderFn<H, C>(Stored<RenderFnSlot<H, C>>);

impl<H: 'static, C: 'static> RenderFn<H, C> {
    pub fn new(render: impl Fn(H) -> C + 'static) -> Self {
        Self(store((try_current_component(), Box::new(render))))
    }

    pub fn call(&self, handle: H) -> C {
        let slot = self.0.value();
        in_component(slot.0.clone(), || (slot.1)(handle))
    }
}

impl<H, C> Clone for RenderFn<H, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<H, C> Copy for RenderFn<H, C> {}

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

impl<H: 'static, C: 'static, R: IntoChild<C>, F: Fn(H) -> R + 'static> IntoRenderFn<H, C> for F {
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
    message = "this component hands nothing to its children, so there is no value to name",
    label = "write these children without `|value|`"
)]
pub trait HandsAValue {
    fn bind<T>(self) -> T;
}

#[diagnostic::on_unimplemented(
    message = "this component takes its children already built, so they cannot be a block that builds them",
    label = "write these children as tags or as `{{expr}}`, without `|| {{...}}`"
)]
pub trait BuildsItsChildren {
    fn build<T>(self) -> T;
}

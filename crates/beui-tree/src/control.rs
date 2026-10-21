use std::cell::RefCell;
use std::hash::Hash;

use beui_macros::component;
use reactive::{
    Boundary, Get, KeyedItems, Pending, ReadSignal, Scope, Set, Stored, create_binding,
    create_signal, store,
};

use crate::children::{ChildSlot, Children, DynamicSegment, NestedHost, SlotChild};
use crate::component::{Render, RenderFn};
use crate::prop::{Func, Prop};

fn build_in_slot<C: SlotChild>(
    slot: &ChildSlot<C>,
    build: impl FnOnce() -> C,
) -> (C::Stored, Scope) {
    let scope = Scope::detached();
    let stored = scope.context().run(|| slot.store(build()));
    slot.fill(vec![stored.clone()]);
    (stored, scope)
}

type HeldChild<C> = Stored<RefCell<Option<(<C as SlotChild>::Stored, Scope)>>>;
type KeyedChild<K, C> = Stored<RefCell<Option<(K, <C as SlotChild>::Stored, Scope)>>>;

#[component]
pub fn Show<C>(
    condition: Prop<bool>,
    #[prop(children)] then: RenderFn<(), Children<C>>,
) -> DynamicSegment<C>
where
    C: SlotChild,
{
    DynamicSegment::new(move |slot: ChildSlot<C>| {
        let held = store(RefCell::new(None::<Scope>));
        create_binding(move || {
            let visible = condition.get();
            let held = held.value();
            let mut held = held.borrow_mut();
            if !visible {
                slot.fill(Vec::new());
                drop(held.take());
            } else if held.is_none() {
                let scope = Scope::detached();
                scope.context().run(|| then.call(()).mount_in(slot.clone()));
                *held = Some(scope);
            }
        });
    })
}

#[component]
pub fn ShowKeepAlive<C>(
    condition: Prop<bool>,
    #[prop(children)] then: Render<(), Children<C>>,
) -> DynamicSegment<C>
where
    C: SlotChild,
{
    DynamicSegment::new(move |slot: ChildSlot<C>| {
        let held = store(RefCell::new(None::<(std::rc::Rc<NestedHost<C>>, Scope)>));
        let mut then = Some(then);
        create_binding(move || {
            let visible = condition.get();
            let held = held.value();
            let mut held = held.borrow_mut();
            if visible
                && held.is_none()
                && let Some(build) = then.take()
            {
                let scope = Scope::detached().pausable();
                let host = scope
                    .context()
                    .run(|| build.call(()).mount_in(slot.clone()));
                *held = Some((host, scope));
            }
            if let Some((host, scope)) = held.as_ref() {
                host.set_visible(visible);
                if visible {
                    scope.resume();
                } else {
                    scope.pause();
                }
            } else {
                slot.fill(Vec::new());
            }
        });
    })
}

#[component]
pub fn Loading<C>(
    fallback: RenderFn<Pending, C>,
    #[prop(children)] content: Render<(), Children<C>>,
) -> DynamicSegment<C>
where
    C: SlotChild,
{
    DynamicSegment::new(move |slot: ChildSlot<C>| {
        let scope = Scope::detached().pausable_effects();
        scope.pause();
        let (boundary, host) = scope.context().run(|| {
            let boundary = Boundary::provide();
            (boundary, content.call(()).mount_hidden(slot.clone()))
        });
        let content = store((host, scope));
        let shown = store(RefCell::new(None::<(Pending, C::Stored, Scope)>));
        let revealed = store(std::cell::Cell::new(false));
        create_binding(move || {
            if revealed.value().get() {
                return;
            }
            let state = boundary.state().and_then(|()| {
                content.value().1.flush_owned();
                boundary.state()
            });
            let shown = shown.value();
            let mut shown = shown.borrow_mut();
            match state {
                Ok(()) => {
                    revealed.value().set(true);
                    boundary.reveal();
                    drop(shown.take());
                    let content = content.value();
                    content.0.set_visible(true);
                    content.1.resume();
                }
                Err(pending) => {
                    if shown.as_ref().is_some_and(|(held, _, _)| *held == pending) {
                        return;
                    }
                    let (stored, scope) = build_in_slot(&slot, || fallback.call(pending.clone()));
                    drop(shown.replace((pending, stored, scope)));
                }
            }
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
        let held: HeldChild<C> = store(RefCell::new(None));
        create_binding(move || {
            let value = value.get();
            let held = held.value();
            let mut held = held.borrow_mut();
            let built = build_in_slot(&slot, || view.call(value));
            drop(held.replace(built));
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
        let rows = store(KeyedItems::new(move |key: K| built.store(view.call(key))));
        create_binding(move || {
            let keys = keys.get();
            rows.value().map(keys).commit(|items, _| slot.fill(items));
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
        let current = create_signal(value.get_untracked());
        let held: KeyedChild<K, C> = store(RefCell::new(None));
        create_binding(move || {
            let value = value.get();
            let next = key.call(value.clone());
            current.set(value);
            let held = held.value();
            let mut held = held.borrow_mut();
            if held.as_ref().is_some_and(|(key, _, _)| *key == next) {
                return;
            }
            let (stored, scope) = build_in_slot(&slot, move || view.call(current.read_only()));
            drop(held.replace((next, stored, scope)));
        });
    })
}

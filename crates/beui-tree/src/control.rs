use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

use beui_macros::component;
use reactive::{KeyedItems, ReadSignal, Scope, create_effect, create_signal, on_cleanup};

use crate::children::{ChildSlot, DynamicSegment, SlotChild};
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

fn discard_previous<C: SlotChild>(previous: Option<(C::Stored, Scope)>) {
    let Some((stored, scope)) = previous else {
        return;
    };
    C::discard(&stored);
    drop(scope);
}

type HeldChild<C> = Rc<RefCell<Option<(<C as SlotChild>::Stored, Scope)>>>;
type KeyedChild<K, C> = Rc<RefCell<Option<(K, <C as SlotChild>::Stored, Scope)>>>;

#[component]
pub fn Show<C>(condition: Prop<bool>, #[prop(children)] then: RenderFn<(), C>) -> DynamicSegment<C>
where
    C: SlotChild,
{
    DynamicSegment::new(move |slot: ChildSlot<C>| {
        let held: HeldChild<C> = Rc::new(RefCell::new(None));
        let owned = held.clone();
        on_cleanup(move || drop(owned));
        create_effect(move || {
            let visible = condition.get();
            let mut held = held.borrow_mut();
            if !visible {
                slot.fill(Vec::new());
                discard_previous::<C>(held.take());
            } else if held.is_none() {
                *held = Some(build_in_slot(&slot, || then.call(())));
            }
        });
    })
}

#[component]
pub fn ShowKeepAlive<C>(
    condition: Prop<bool>,
    #[prop(children)] then: Render<(), C>,
) -> DynamicSegment<C>
where
    C: SlotChild,
{
    DynamicSegment::new(move |slot: ChildSlot<C>| {
        let held: HeldChild<C> = Rc::new(RefCell::new(None));
        let parked = held.clone();
        on_cleanup(move || {
            let held = parked.borrow_mut().take();
            if let Some((stored, _)) = held {
                C::discard(&stored);
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
                    C::discard(row);
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

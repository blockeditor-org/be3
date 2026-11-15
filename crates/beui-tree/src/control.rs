use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

use beui_macros::component;
use beui_reactive::{
    Memo, create_memo, untrack, use_context, provide_context,
    Boundary, Get, KeyedItems, Pending, ReadSignal, Scope, Set, Signal, StoredCell, create_binding,
    create_signal, store,
};

use crate::children::{
    Branch, ChildSlot, Children, Count, Counted, DynamicSegment, Fallback, FormOf, IntoChild,
    NestedHost, Or, SlotChild, ValueFallback,
};
use crate::component::{Render, RenderFn};
use crate::prop::{Func, Prop};

type Built<C> = RenderFn<(), Children<C>>;

fn built<C: SlotChild, H: 'static, B: Branch<C>>(
    render: RenderFn<H, B>,
) -> RenderFn<H, Children<C>> {
    RenderFn::new(move |handle| render.call(handle).into_children())
}

/// Builds `branch` in a scope of its own and puts what it builds in `slot`, replacing what
/// was there.
fn mount<C: SlotChild>(slot: &ChildSlot<C>, branch: impl FnOnce() -> Children<C>) -> Scope {
    let scope = Scope::detached();
    scope.context().run(|| branch().mount_in(slot.clone()));
    scope
}

type Kept<C> = Option<(Rc<NestedHost<C>>, Scope)>;

/// Shows `then` while `condition` holds, and `fallback`, if given, while it does not. By
/// default a branch is built each time it is shown and dropped when it is not. With
/// `keep_alive` each is built once, the first time it shows, and kept while hidden, paused,
/// so nothing in it runs until it shows again.
///
/// A `Show` fills a run among its parent's children. When it has a `fallback` and both
/// branches build exactly one child, it also fills a slot that takes one `Child`; without a
/// `fallback`, one that takes an optional child (`Option<Child>`).
#[component]
pub fn Show<C, B, F>(
    condition: Prop<bool>,
    #[prop(default = false)] keep_alive: bool,
    fallback: F,
    #[prop(children)] then: RenderFn<(), B>,
) -> FormOf<C, <B as Counted>::Count, F::Count>
where
    C: SlotChild,
    B: Branch<C>,
    F: Fallback<C>,
    <B as Counted>::Count: Or<F::Count>,
{
    let then = built(then);
    let fallback = fallback.into_render();
    <<<B as Counted>::Count as Or<F::Count>>::Count as Count>::form(DynamicSegment::new(
        move |slot: ChildSlot<C>| match keep_alive {
            false => show(condition, [fallback, Some(then)], slot),
            true => keep(condition, [fallback, Some(then)], slot),
        },
    ))
}

fn show<C: SlotChild>(condition: Prop<bool>, branches: [Option<Built<C>>; 2], slot: ChildSlot<C>) {
    let held = store(RefCell::new(None::<(bool, Option<Scope>)>));
    create_binding(move || {
        let visible = condition.get();
        let held = held.value();
        let mut held = held.borrow_mut();
        if held.as_ref().is_some_and(|(shown, _)| *shown == visible) {
            return;
        }
        let built = branches[usize::from(visible)].map(|branch| mount(&slot, || branch.call(())));
        if built.is_none() {
            slot.fill(Vec::new());
        }
        drop(held.replace((visible, built)));
    });
}

/// Whether the branch of a `Show` with `keep_alive` around the caller is shown, and every such
/// branch around that: `None` outside any. What a kept branch holds outside itself while hidden (a
/// dock it joined) lets go by it, and takes back when it shows again.
pub fn kept_shown() -> Option<Memo<bool>> {
    use_context::<KeptShown>().map(|KeptShown(shown)| shown)
}

#[derive(Clone, Copy)]
struct KeptShown(Memo<bool>);

fn keep<C: SlotChild>(condition: Prop<bool>, branches: [Option<Built<C>>; 2], slot: ChildSlot<C>) {
    let held = store(RefCell::new([None, None] as [Kept<C>; 2]));
    let shown = StoredCell::new(None::<bool>);
    let outer = kept_shown();
    let showing = [create_signal(false), create_signal(false)];
    let branch_shown = showing.map(|own| {
        create_memo(move || own.get() && outer.is_none_or(|outer| outer.get()))
    });
    create_binding(move || {
        let visible = condition.get();
        if shown.get() == Some(visible) {
            return;
        }
        shown.set(Some(visible));
        let visible = usize::from(visible);
        let held = held.value();
        let mut held = held.borrow_mut();
        if let Some((host, scope)) = held[1 - visible].as_ref() {
            host.set_visible(false);
            // Told before it pauses, so an effect it wakes still runs.
            untrack(|| showing[1 - visible].set(false));
            scope.pause();
        }
        // Shown before it is built, so what a first build claims (a dock it joins) holds on.
        untrack(|| showing[visible].set(true));
        if held[visible].is_none()
            && let Some(branch) = branches[visible]
        {
            let scope = Scope::detached().pausable();
            let shown_here = branch_shown[visible];
            let host = scope.context().run(|| {
                provide_context(KeptShown(shown_here));
                branch.call(()).mount_in(slot.clone())
            });
            held[visible] = Some((host, scope));
        }
        match held[visible].as_ref() {
            Some((host, scope)) => {
                host.set_visible(true);
                scope.resume();
            }
            None => slot.fill(Vec::new()),
        }
    });
}

/// Builds `content` at once but hidden, shows `fallback` while anything it made is
/// pending, then shows the content for good. It fills a slot that takes one `Child` when
/// both build exactly one child.
#[component]
pub fn Loading<C, B, F>(
    fallback: RenderFn<Pending, F>,
    #[prop(children)] content: Render<(), B>,
) -> FormOf<C, <B as Counted>::Count, <F as Counted>::Count>
where
    C: SlotChild,
    B: Branch<C>,
    F: Branch<C>,
    <B as Counted>::Count: Or<<F as Counted>::Count>,
{
    let fallback = built(fallback);
    <<<B as Counted>::Count as Or<<F as Counted>::Count>>::Count as Count>::form(
        DynamicSegment::new(move |slot: ChildSlot<C>| {
            let scope = Scope::detached().pausable_effects();
            scope.pause();
            let (boundary, host) = scope.context().run(|| {
                let boundary = Boundary::provide();
                (
                    boundary,
                    content.call(()).into_children().mount_hidden(slot.clone()),
                )
            });
            let content = store((host, scope));
            let shown = store(RefCell::new(None::<(Pending, Scope)>));
            let revealed = StoredCell::new(false);
            create_binding(move || {
                if revealed.get() {
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
                        revealed.set(true);
                        boundary.reveal();
                        let content = content.value();
                        content.0.set_visible(true);
                        drop(shown.take());
                        content.1.resume();
                    }
                    Err(pending) => {
                        if shown.as_ref().is_some_and(|(held, _)| *held == pending) {
                            return;
                        }
                        let built = mount(&slot, || fallback.call(pending.clone()));
                        drop(shown.replace((pending, built)));
                    }
                }
            });
        }),
    )
}

/// Builds `view` from `value`, again each time it changes. Given a `fallback`, `value` is
/// an `Option`: `view` takes what is in it, and the fallback shows while it is `None`. It
/// fills a slot that takes one `Child` when what it builds is exactly one child.
#[component]
pub fn Dynamic<V, C, B, F>(
    value: Prop<V>,
    fallback: F,
    #[prop(children)] view: RenderFn<F::Handed, B>,
) -> FormOf<C, <B as Counted>::Count, F::Count>
where
    V: Clone + 'static,
    C: SlotChild,
    B: Branch<C>,
    F: ValueFallback<V, C>,
    <B as Counted>::Count: Or<F::Count>,
{
    let view = built(view);
    let fallback = fallback.into_render();
    <<<B as Counted>::Count as Or<F::Count>>::Count as Count>::form(DynamicSegment::new(
        move |slot: ChildSlot<C>| {
            let held = store(RefCell::new(None::<(bool, Option<Scope>)>));
            create_binding(move || {
                let value = F::split(value.get());
                let held = held.value();
                let mut held = held.borrow_mut();
                if value.is_none() && held.as_ref().is_some_and(|(some, _)| !some) {
                    return;
                }
                let some = value.is_some();
                let built = match value {
                    Some(value) => Some(mount(&slot, || view.call(value))),
                    None => fallback.map(|fallback| mount(&slot, || fallback.call(()))),
                };
                if built.is_none() {
                    slot.fill(Vec::new());
                }
                drop(held.replace((some, built)));
            });
        },
    ))
}

#[component]
pub fn ForEach<K, C, R>(
    keys: Prop<Vec<K>>,
    #[prop(children)] view: RenderFn<K, R>,
) -> DynamicSegment<C>
where
    K: Clone + Hash + Eq + 'static,
    C: SlotChild,
    R: IntoChild<C> + 'static,
{
    DynamicSegment::new(move |slot: ChildSlot<C>| {
        let built = slot.clone();
        let rows = store(KeyedItems::new(move |key: K| {
            built.store(view.call(key).into_child())
        }));
        create_binding(move || {
            let keys = keys.get();
            rows.value().map(keys).commit(|items, _| slot.fill(items));
        });
    })
}

/// Builds `view` once for each `key` of `value`, handing it the value as a signal, so a
/// change that keeps the key updates in place. Given a `fallback`, `value` is an `Option`
/// and the fallback shows while it is `None`.
#[component]
pub fn Keyed<V, K, C, B, F>(
    value: Prop<V>,
    fallback: F,
    key: Func<F::Handed, K>,
    #[prop(children)] view: RenderFn<ReadSignal<F::Handed>, B>,
) -> FormOf<C, <B as Counted>::Count, F::Count>
where
    V: Clone + 'static,
    K: PartialEq + 'static,
    C: SlotChild,
    B: Branch<C>,
    F: ValueFallback<V, C>,
    F::Handed: PartialEq,
    <B as Counted>::Count: Or<F::Count>,
{
    let view = built(view);
    let fallback = fallback.into_render();
    <<<B as Counted>::Count as Or<F::Count>>::Count as Count>::form(DynamicSegment::new(
        move |slot: ChildSlot<C>| {
            let current = store(RefCell::new(None::<Signal<F::Handed>>));
            let held = store(RefCell::new(None::<(Option<K>, Option<Scope>)>));
            create_binding(move || {
                let value = F::split(value.get());
                let next = value.clone().map(|value| key.call(value));
                let held = held.value();
                let mut held = held.borrow_mut();
                if held.as_ref().is_some_and(|(key, _)| *key == next) {
                    if let (Some(value), Some(signal)) = (value, *current.value().borrow()) {
                        signal.set(value);
                    }
                    return;
                }
                let built = match value {
                    Some(value) => Some(mount(&slot, move || {
                        let signal = create_signal(value);
                        *current.value().borrow_mut() = Some(signal);
                        view.call(signal.read_only())
                    })),
                    None => {
                        *current.value().borrow_mut() = None;
                        fallback.map(|fallback| mount(&slot, || fallback.call(())))
                    }
                };
                if built.is_none() {
                    slot.fill(Vec::new());
                }
                drop(held.replace((next, built)));
            });
        },
    ))
}

//! `KeepAlive` and `Kept`: a subtree built once and kept, paused, while nothing shows it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use be_lib_stable_hash::{HashMap, HashSet};
use beui_macros::component;
use beui_reactive::{ScopeContext, current_scope};

use crate::reactive::{
    AnyKey, Get, KeepSet, KeptKey, Memo, NodeId, Prop, ReadSignal, Build, Scope, Set, Signal,
    Unwrapped, create_effect, create_memo, create_signal, kept_shown, on_cleanup, provide_context,
    provide_kept_shown, untrack, use_context,
};

struct Entry {
    node: NodeId,
    scope: Rc<Scope>,
    /// The `Kept` that shows it now, if any.
    borrower: Signal<Option<u64>>,
    /// Whether what the borrower sits in is shown.
    around: Signal<Option<Memo<bool>>>,
}

struct Cache {
    owner: Option<ScopeContext>,
    entries: RefCell<HashMap<AnyKey, Entry>>,
    next: Cell<u64>,
    /// The keys kept while nothing shows them, or `None` when every key is.
    keeps: Option<Memo<HashSet<AnyKey>>>,
    /// The keys whose subtree keeps running, and counts as shown, while nothing shows it.
    awake: Option<Memo<HashSet<AnyKey>>>,
}

impl Cache {
    fn kept(&self, key: &AnyKey) -> bool {
        self.keeps
            .is_none_or(|keeps| keeps.with_untracked(|keeps| keeps.contains(key)))
    }

    fn awake(&self, key: &AnyKey) -> bool {
        self.awake
            .is_some_and(|awake| awake.with_untracked(|awake| awake.contains(key)))
    }

    /// Pauses every entry nothing shows, or resumes it while its key is awake.
    fn wake(&self) {
        let hidden: Vec<(Rc<Scope>, bool)> = self
            .entries
            .borrow()
            .iter()
            .filter(|(_, entry)| entry.borrower.get_untracked().is_none())
            .map(|(key, entry)| (entry.scope.clone(), self.awake(key)))
            .collect();
        for (scope, awake) in hidden {
            match awake {
                true => scope.resume(),
                false => scope.pause(),
            }
        }
    }

    /// Forgets every entry its keep set dropped that nothing shows.
    fn trim(&self) {
        let dropped: Vec<Entry> = {
            let mut entries = self.entries.borrow_mut();
            let gone: Vec<AnyKey> = entries
                .iter()
                .filter(|(key, entry)| entry.borrower.get_untracked().is_none() && !self.kept(key))
                .map(|(key, _)| key.clone())
                .collect();
            gone.iter().filter_map(|key| entries.remove(key)).collect()
        };
        for entry in dropped {
            entry.scope.dispose();
        }
    }
}

#[derive(Clone)]
struct KeepAliveCache(Rc<Cache>);

/// Keeps the subtrees the `Kept`s inside it build, by key, while nothing shows them: a
/// subtree is built the first time a `Kept` with its key is, paused (nothing in it runs)
/// while no `Kept` shows it, and resumed and shown again by the next `Kept` with that key,
/// wherever it is. Given `keep`, a key it no longer lists is dropped once nothing shows it.
/// Two `Kept`s with one key shown at once show it where the one built last is. A key `awake`
/// lists keeps running while hidden, and counts as shown: what it serves is shown elsewhere (a
/// dock tab whose own dock joined the one around it).
///
/// A kept subtree belongs to the `KeepAlive`, so it reads the context provided around the
/// `KeepAlive`, not what is provided between it and a `Kept`; it goes when the `KeepAlive`
/// does.
#[component]
pub fn KeepAlive<B, KS, AS>(
    keep: KS,
    awake: AS,
    #[prop(children)] content: Build<B>,
) -> B::Output
where
    B: Unwrapped + 'static,
    KS: KeepSet,
    AS: KeepSet,
{
    let cache = Rc::new(Cache {
        owner: current_scope(),
        entries: RefCell::default(),
        next: Cell::new(0),
        keeps: untrack(|| keep.keys().is_some())
            .then(|| create_memo(move || keep.keys().unwrap_or_default().into_iter().collect())),
        // Left out, `awake` lists nothing.
        awake: untrack(|| awake.keys().is_some())
            .then(|| create_memo(move || awake.keys().unwrap_or_default().into_iter().collect())),
    });
    if let Some(awake) = cache.awake {
        let watched = cache.clone();
        create_effect(move || {
            awake.with(|_| ());
            untrack(|| watched.wake());
        });
    }
    if let Some(keeps) = cache.keeps {
        let watched = cache.clone();
        create_effect(move || {
            keeps.with(|_| ());
            untrack(|| watched.trim());
        });
    }
    let held = cache.clone();
    on_cleanup(move || {
        let entries: Vec<Entry> = held
            .entries
            .borrow_mut()
            .drain()
            .map(|(_, entry)| entry)
            .collect();
        for entry in entries {
            entry.scope.dispose();
        }
    });
    provide_context(KeepAliveCache(cache));
    content.call().unwrapped()
}

/// A subtree its `KeepAlive` keeps by `key` (without one, the one unkeyed subtree of that
/// `KeepAlive`): built the first time, shown here through a portal, and kept, paused, after
/// this `Kept` goes, until another `Kept` with that key shows it again. Inside it,
/// `kept_shown()` says whether it is shown, so what it holds outside itself (a dock it
/// joined) lets go while it is hidden. A `Kept` inside another one's subtree pauses with it.
#[component]
pub fn Kept<K>(key: K, #[prop(children)] content: Build<NodeId>) -> NodeId
where
    K: KeptKey,
{
    let KeepAliveCache(cache) =
        use_context::<KeepAliveCache>().expect("a `Kept` is built inside a `KeepAlive`");
    let key = key.into_any_key();
    let me = cache.next.get();
    cache.next.set(me + 1);
    let around = kept_shown();
    let found = cache.entries.borrow().get(&key).map(|entry| {
        (
            entry.node,
            entry.scope.clone(),
            entry.borrower,
            entry.around,
        )
    });
    let (node, borrower) = match found {
        Some((node, scope, borrower, held_around)) => {
            borrower.set(Some(me));
            held_around.set(around);
            scope.resume();
            (node, borrower)
        }
        None => {
            let scope = Rc::new(
                cache
                    .owner
                    .as_ref()
                    .and_then(ScopeContext::child)
                    .unwrap_or_else(Scope::detached)
                    .pausable(),
            );
            let borrower = create_signal_in(&scope, Some(me));
            let held_around = create_signal_in(&scope, around);
            let node = scope.context().run(|| {
                let awake = cache.awake;
                let named = key.clone();
                let shown = create_memo(move || {
                    let woken =
                        awake.is_some_and(|awake| awake.with(|awake| awake.contains(&named)));
                    (borrower.get().is_some() || woken)
                        && held_around.get().is_none_or(|around| around.get())
                });
                provide_kept_shown(shown);
                content.call()
            });
            cache.entries.borrow_mut().insert(
                key.clone(),
                Entry {
                    node,
                    scope,
                    borrower,
                    around: held_around,
                },
            );
            (node, borrower)
        }
    };
    // Hidden with the subtree around it (a `Kept` of another `KeepAlive`), and shown with it.
    if let Some(around) = around {
        let followed = cache.clone();
        let key = key.clone();
        create_effect(move || {
            let shown = around.get();
            let entries = followed.entries.borrow();
            let Some(entry) = entries
                .get(&key)
                .filter(|entry| entry.borrower.get_untracked() == Some(me))
            else {
                return;
            };
            let scope = entry.scope.clone();
            drop(entries);
            match shown {
                true => scope.resume(),
                false => scope.pause(),
            }
        });
    }
    let released = cache.clone();
    on_cleanup(move || {
        let entries = released.entries.borrow();
        let Some(entry) = entries.get(&key) else {
            return;
        };
        if entry.borrower.get_untracked() != Some(me) {
            return;
        }
        entry.borrower.set(None);
        entry.around.set(None);
        let keep = released.kept(&key);
        let scope = entry.scope.clone();
        drop(entries);
        match keep {
            true if released.awake(&key) => {}
            true => scope.pause(),
            false => {
                // Taken out before it is disposed: a `Kept` in it releases into these entries.
                let removed = released.entries.borrow_mut().remove(&key);
                if let Some(entry) = removed {
                    entry.scope.dispose();
                }
            }
        }
    });
    let shown: ReadSignal<Option<u64>> = borrower.read_only();
    crate::core::portal(Prop::dynamic(move || {
        (shown.get() == Some(me)).then_some(node)
    }))
}

/// A signal owned by `scope`, so it goes with the kept subtree rather than with the `Kept`
/// that first built it.
fn create_signal_in<T: 'static>(scope: &Rc<Scope>, value: T) -> Signal<T> {
    scope.context().run(|| create_signal(value))
}

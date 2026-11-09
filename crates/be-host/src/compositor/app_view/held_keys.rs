//! Keys typed while an editor gives the keyboard to a child whose view has not taken it yet (a
//! child still loading). The editor asked (`EditorHost::focus_child`), so the keys would reach
//! the editor, which has nothing focused for them. Every gift ends explicitly, never by a
//! timeout:
//!
//! - granted: the child's view takes the keyboard (`given`). The keys held go to the child, in
//!   order, once its app has drawn a frame that took the focus, so the field it focuses then
//!   hears them; keys typed straight into the child meanwhile wait behind them.
//! - refused: the gift falls through (`refused`), the person presses in the editor or takes the
//!   keyboard elsewhere, or the child, the editor or its app goes (`child_focus_pending` turns
//!   false). The keys held go back to the editor's view.

use std::cell::RefCell;
use std::collections::HashMap;

use be_sdk_api::EditorInstanceId;
use beui::{Event, Forwarded, ForwardedInput, Modifiers, NodeId, PointerPhase};

/// Keys an editor's view heard while its gift was pending.
struct Held {
    owner: NodeId,
    events: Vec<Forwarded>,
    modifiers: Modifiers,
}

/// Keys owed to a view. `ready` once it can take them; until then its own typed keys queue
/// behind them. `after` is the frame request whose answer took the focus.
struct Due {
    events: Vec<Forwarded>,
    modifiers: Modifiers,
    after: Option<u64>,
    ready: bool,
}

thread_local! {
    static HELD: RefCell<HashMap<(String, EditorInstanceId), Held>> = RefCell::new(HashMap::new());
    static DUE: RefCell<HashMap<NodeId, Due>> = RefCell::new(HashMap::new());
}

fn typed(event: &Forwarded) -> bool {
    matches!(
        event,
        Forwarded::Input(Event::Key { .. } | Event::Text(_) | Event::Ime(_) | Event::Paste(_))
    )
}

fn pressed(event: &Forwarded) -> bool {
    matches!(event, Forwarded::Pointer { event, .. } if matches!(event.phase, PointerPhase::Down(_)))
}

/// What routing a view's input asks of the caller.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Routed {
    /// The person moved on while the editor's gift was pending: refuse it.
    pub(super) refuse: bool,
}

/// Holds the typed events of input forwarded to an editor's view while the editor gives the
/// keyboard to a child (`pending`), and hands the view the keys owed to it once it can take
/// them. `frames` is the view's app's `(asked, answered)` frame count.
pub(super) fn route(
    app: &str,
    instance: EditorInstanceId,
    own: Option<NodeId>,
    pending: bool,
    frames: (u64, u64),
    input: &mut ForwardedInput,
) -> Routed {
    let Some(own) = own else {
        return Routed::default();
    };
    let mut routed = Routed::default();
    let key = (app.to_owned(), instance);
    let moved_on =
        input.events.iter().any(pressed) || input.events.contains(&Forwarded::Focus(false));
    let released = HELD.with_borrow_mut(|held| {
        let holding = held.get(&key).is_some_and(|held| held.owner == own);
        if pending && moved_on && (holding || input.focused) {
            routed.refuse = true;
        }
        if !pending || routed.refuse {
            return match held.remove(&key) {
                Some(held) if held.owner == own => held.events,
                Some(held) => {
                    owe(held.owner, held.events, held.modifiers, true);
                    Vec::new()
                }
                None => Vec::new(),
            };
        }
        if !input.focused && !holding {
            return Vec::new();
        }
        let entry = held.entry(key).or_insert_with(|| Held {
            owner: own,
            events: Vec::new(),
            modifiers: input.modifiers,
        });
        if entry.owner == own {
            let (typed, kept): (Vec<_>, Vec<_>) = input.events.drain(..).partition(typed);
            entry.events.extend(typed);
            entry.modifiers = input.modifiers;
            input.events = kept;
        }
        Vec::new()
    });
    deliver(input, released);
    let (asked, answered) = frames;
    let owed = DUE.with_borrow_mut(|due| {
        let owed = due.get_mut(&own)?;
        if !owed.ready {
            if owed.after.is_none() && input.events.contains(&Forwarded::Focus(true)) {
                owed.after = Some(asked + 1);
            }
            owed.ready = owed.after.is_some_and(|after| answered >= after)
                || input.events.contains(&Forwarded::Focus(false));
        }
        if !owed.ready {
            let (typed, kept): (Vec<_>, Vec<_>) = input.events.drain(..).partition(typed);
            owed.events.extend(typed);
            input.events = kept;
            return None;
        }
        due.remove(&own).map(|owed| owed.events)
    });
    deliver(input, owed.unwrap_or_default());
    routed
}

/// Puts `events` after the input's gained focus and before the rest, so they reach the view
/// with the keyboard it has now and ahead of what was typed after them.
fn deliver(input: &mut ForwardedInput, events: Vec<Forwarded>) {
    if events.is_empty() {
        return;
    }
    let (focus, rest): (Vec<_>, Vec<_>) = input
        .events
        .drain(..)
        .partition(|event| matches!(event, Forwarded::Focus(true)));
    input.events = focus.into_iter().chain(events).chain(rest).collect();
}

fn owe(node: NodeId, events: Vec<Forwarded>, modifiers: Modifiers, ready: bool) {
    DUE.with_borrow_mut(|due| {
        let owed = due.entry(node).or_insert(Due {
            events: Vec::new(),
            modifiers,
            after: None,
            ready,
        });
        owed.events.extend(events);
        owed.ready |= ready;
    });
}

/// The editor's gift reached the child's view at `child`: the keys held for it are owed to it,
/// and keys typed into it wait behind them until its app has drawn a frame that took the focus.
pub(super) fn given(app: &str, instance: EditorInstanceId, child: NodeId) {
    let held = HELD.with_borrow_mut(|held| held.remove(&(app.to_owned(), instance)));
    let (events, modifiers) = held.map_or((Vec::new(), Modifiers::NONE), |held| {
        (held.events, held.modifiers)
    });
    owe(child, events, modifiers, false);
}

/// The editor's gift fell through: the keys held go back to the editor's own view.
pub(super) fn refused(app: &str, instance: EditorInstanceId) {
    if let Some(held) = HELD.with_borrow_mut(|held| held.remove(&(app.to_owned(), instance))) {
        owe(held.owner, held.events, held.modifiers, true);
    }
}

/// Settles what the view at `node`, of the editor `instance` whose gift is `pending` or not, is
/// owed now that its app answered `answered` frames: keys it held for a gift that ended without
/// reaching a child come back to it. Answers the modifiers to forward it an empty input with,
/// to carry the keys it can take now.
pub(super) fn owed_now(
    app: &str,
    instance: EditorInstanceId,
    node: NodeId,
    pending: bool,
    answered: u64,
) -> Option<Modifiers> {
    if !pending {
        let key = (app.to_owned(), instance);
        let holds = HELD.with_borrow(|held| held.get(&key).is_some_and(|held| held.owner == node));
        if holds {
            refused(app, instance);
        }
    }
    DUE.with_borrow_mut(|due| {
        let owed = due.get_mut(&node)?;
        owed.ready |= owed.after.is_some_and(|after| answered >= after);
        (owed.ready && !owed.events.is_empty()).then_some(owed.modifiers)
    })
}

/// The view at `node` is gone: what it held and was owed goes with it.
pub(super) fn forget(node: NodeId) {
    DUE.with_borrow_mut(|due| due.remove(&node));
    HELD.with_borrow_mut(|held| held.retain(|_, held| held.owner != node));
}

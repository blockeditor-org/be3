//! Keys typed while an editor gives the keyboard to a child that is still loading. The editor
//! asked (`EditorHost::focus_child`) but the child's view has not taken the keyboard yet, so the
//! keys would reach the editor, which has nothing focused for them. They are held for the child
//! and handed to it once its view has the keyboard and a text field to type into (its view asks
//! for an input method), or after `GRACE` for a child that never asks; or back to the editor if
//! the gift falls through or takes longer than `LIMIT`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use be_sdk_api::EditorInstanceId;
use beui::{Event, Forwarded, ForwardedInput, NodeId};

const LIMIT: Duration = Duration::from_secs(2);
const GRACE: Duration = Duration::from_millis(500);

struct Held {
    since: Instant,
    owner: NodeId,
    events: Vec<Forwarded>,
    expired: bool,
}

struct Due {
    events: Vec<Forwarded>,
    since: Instant,
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

/// Takes the typed events out of input forwarded to an editor's view while the editor gives the
/// keyboard to a child (`pending`), and hands the view what it is owed once it is ready for them:
/// keys held for it as the child given the keyboard, or held in it when the gift fell through.
pub(super) fn route(
    app: &str,
    instance: EditorInstanceId,
    own: Option<NodeId>,
    pending: bool,
    now: Instant,
    mut input: ForwardedInput,
) -> ForwardedInput {
    let Some(own) = own else {
        return input;
    };
    let key = (app.to_owned(), instance);
    let released = HELD.with_borrow_mut(|held| {
        if !pending {
            if let Some(held) = held.remove(&key) {
                owe(held.owner, held.events, now, true);
            }
            return Vec::new();
        }
        if !input.focused && !held.contains_key(&key) {
            return Vec::new();
        }
        let entry = held.entry(key).or_insert_with(|| Held {
            since: now,
            owner: own,
            events: Vec::new(),
            expired: false,
        });
        if entry.expired {
            return Vec::new();
        }
        if now.duration_since(entry.since) > LIMIT {
            entry.expired = true;
            return std::mem::take(&mut entry.events);
        }
        if input.focused && entry.owner == own {
            let (typed, kept): (Vec<_>, Vec<_>) = input.events.drain(..).partition(typed);
            entry.events.extend(typed);
            input.events = kept;
        }
        Vec::new()
    });
    deliver(&mut input, released);
    if input.focused {
        let due = DUE.with_borrow_mut(|due| match due.get(&own) {
            Some(owed) if owed.ready => due.remove(&own).map(|owed| owed.events),
            _ => None,
        });
        deliver(&mut input, due.unwrap_or_default());
    }
    input
}

fn deliver(input: &mut ForwardedInput, events: Vec<Forwarded>) {
    if events.is_empty() {
        return;
    }
    let (focus, rest): (Vec<_>, Vec<_>) = input
        .events
        .drain(..)
        .partition(|event| matches!(event, Forwarded::Focus(_)));
    input.events = focus.into_iter().chain(events).chain(rest).collect();
}

fn owe(node: NodeId, events: Vec<Forwarded>, now: Instant, ready: bool) {
    if events.is_empty() {
        return;
    }
    DUE.with_borrow_mut(|due| {
        let owed = due.entry(node).or_insert(Due {
            events: Vec::new(),
            since: now,
            ready,
        });
        owed.events.extend(events);
        owed.ready |= ready;
    });
}

/// The editor's gift reached the child's view at `child`: the keys held for it are owed to it.
pub(super) fn given(app: &str, instance: EditorInstanceId, child: NodeId, now: Instant) {
    if let Some(held) = HELD.with_borrow_mut(|held| held.remove(&(app.to_owned(), instance))) {
        owe(child, held.events, now, false);
    }
}

/// The editor's gift fell through: the keys held go back to the editor's own view.
pub(super) fn refused(app: &str, instance: EditorInstanceId, now: Instant) {
    if let Some(held) = HELD.with_borrow_mut(|held| held.remove(&(app.to_owned(), instance))) {
        owe(held.owner, held.events, now, true);
    }
}

/// Whether the view at `node` is owed keys it is now ready for: it has a text field to type into
/// (`typing`), or the grace is over. The caller then forwards it an empty input to carry them.
pub(super) fn owed_now(node: NodeId, typing: bool, now: Instant) -> bool {
    DUE.with_borrow_mut(|due| {
        let Some(owed) = due.get_mut(&node) else {
            return false;
        };
        owed.ready |= typing || now.duration_since(owed.since) > GRACE;
        owed.ready
    })
}

pub(super) fn forget(node: NodeId) {
    DUE.with_borrow_mut(|due| due.remove(&node));
}

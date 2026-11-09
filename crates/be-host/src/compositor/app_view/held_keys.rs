//! Keys typed while an editor gives the keyboard to a child whose view has not taken it yet (a
//! child still loading). The editor asked (`EditorHost::focus_child`, `focus_block_child`), so
//! the keys would reach the editor, which has nothing focused for them. Every gift ends
//! explicitly, never by a timeout:
//!
//! - granted: the child's view takes the keyboard (`given`). The keys held go to the child, in
//!   order, once a text field in it has the keyboard (or, for a child with none, two frames after
//!   it had its content and the focus); keys typed into the child meanwhile wait behind them.
//! - refused: the gift falls through (`refused`), the person presses in the editor or takes the
//!   keyboard elsewhere, or the child, the editor or its app goes (`child_focus_pending` turns
//!   false). The keys held go back to the editor's view if it still has the keyboard, and are
//!   dropped otherwise, as typing into a view without the keyboard would be.
//!
//! Keys are held from the editor's view that heard the first of them; keys typed into another
//! view of the same editor meanwhile pass, which only a joined dock's several tabs could do.

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

/// Keys owed to a child's view. Until it is ready (`Due::ready`) its own typed keys queue
/// behind them.
struct Due {
    events: Vec<Forwarded>,
    modifiers: Modifiers,
    focused: bool,
    after: Option<u64>,
}

thread_local! {
    static HELD: RefCell<HashMap<(String, EditorInstanceId), Held>> = RefCell::new(HashMap::new());
    static DUE: RefCell<HashMap<NodeId, Due>> = RefCell::new(HashMap::new());
    static RETURNED: RefCell<HashMap<NodeId, (Vec<Forwarded>, Modifiers)>> =
        RefCell::new(HashMap::new());
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

/// Where a view and its app stand, as the runtime reports them.
#[derive(Clone, Copy, Debug)]
pub(super) struct App {
    /// Frames asked of the app and answered by it.
    pub(super) frames: (u64, u64),
    /// The block content of the view's editor has reached it.
    pub(super) content: bool,
    /// The view asks for an input method: a text field in it has the keyboard.
    pub(super) typing: bool,
}

impl Due {
    /// A child's view can take the keys owed to it once a text field in it has the keyboard,
    /// or, for a child with none, once its app has drawn two frames after it had its content
    /// and the focus: the first can still be the one that shows the content arrive.
    fn ready(&mut self, app: App) -> bool {
        let (asked, answered) = app.frames;
        if self.after.is_none() && self.focused && app.content {
            self.after = Some(asked + 2);
        }
        (self.focused && app.typing) || self.after.is_some_and(|after| answered >= after)
    }
}

/// What routing a view's input asks of the caller.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Routed {
    /// The person moved on while the editor's gift was pending: refuse it.
    pub(super) refuse: bool,
}

/// Holds the typed events of input forwarded to an editor's view while the editor gives the
/// keyboard to a child (`pending`), and hands the view the keys owed to it once it can take them.
pub(super) fn route(
    editor: (&str, EditorInstanceId),
    own: Option<NodeId>,
    pending: bool,
    app: App,
    input: &mut ForwardedInput,
) -> Routed {
    let Some(own) = own else {
        return Routed::default();
    };
    let key = (editor.0.to_owned(), editor.1);
    let mut routed = Routed::default();
    let moved_on =
        input.events.iter().any(pressed) || input.events.contains(&Forwarded::Focus(false));
    let released = HELD.with_borrow_mut(|held| {
        let holding = held.get(&key).is_some_and(|held| held.owner == own);
        if pending && moved_on && (holding || input.focused) {
            routed.refuse = true;
        }
        if !pending || routed.refuse {
            if !holding {
                return Vec::new();
            }
            let events = held.remove(&key).map(|held| held.events).unwrap_or_default();
            let had_keyboard = input.focused || input.events.contains(&Forwarded::Focus(false));
            return if had_keyboard { events } else { Vec::new() };
        }
        if !input.focused {
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
    if input.focused {
        let returned = RETURNED.with_borrow_mut(|returned| returned.remove(&own));
        deliver(input, returned.map(|(events, _)| events).unwrap_or_default());
    }
    let owed = DUE.with_borrow_mut(|due| {
        let owed = due.get_mut(&own)?;
        owed.focused |= input.focused;
        let leaving = input.events.contains(&Forwarded::Focus(false));
        if leaving || (input.focused && owed.ready(app)) {
            return due.remove(&own).map(|owed| owed.events);
        }
        let (typed, kept): (Vec<_>, Vec<_>) = input.events.drain(..).partition(typed);
        owed.events.extend(typed);
        input.events = kept;
        None
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

/// The editor's gift reached the child's view at `child`, which has the keyboard already
/// (`focused`) or is just being given it: the keys held for it are owed to it, and a child just
/// being given it hears its own keys only once it is ready for them. A child that already had
/// the keyboard, with nothing held for it, is left as it is.
pub(super) fn given(editor: (&str, EditorInstanceId), child: NodeId, focused: bool) {
    let key = (editor.0.to_owned(), editor.1);
    let held = HELD.with_borrow_mut(|held| held.remove(&key));
    let (events, modifiers) = held.map_or((Vec::new(), Modifiers::NONE), |held| {
        (held.events, held.modifiers)
    });
    if focused && events.is_empty() {
        return;
    }
    DUE.with_borrow_mut(|due| {
        due.insert(
            child,
            Due {
                events,
                modifiers,
                focused,
                after: None,
            },
        )
    });
}

/// The editor's gift fell through: the keys held go back to its view if `has_keyboard` says it
/// still has the keyboard, and are dropped otherwise.
pub(super) fn refused(editor: (&str, EditorInstanceId), has_keyboard: impl FnOnce(NodeId) -> bool) {
    let key = (editor.0.to_owned(), editor.1);
    let Some(held) = HELD.with_borrow_mut(|held| held.remove(&key)) else {
        return;
    };
    if !held.events.is_empty() && has_keyboard(held.owner) {
        RETURNED.with_borrow_mut(|returned| {
            returned.insert(held.owner, (held.events, held.modifiers))
        });
    }
}

/// Settles what the view at `node` of `editor` holds and is owed, after its app moved on: keys
/// it held for a gift that is no longer `pending` come back to it (if `focused`), and keys owed
/// to it as a child become due once it is ready. Answers the modifiers to forward it an empty
/// input with, to carry what it can take now.
pub(super) fn owed_now(
    editor: (&str, EditorInstanceId),
    node: NodeId,
    pending: bool,
    app: App,
    focused: bool,
) -> Option<Modifiers> {
    if !pending {
        refused(editor, |owner| owner == node && focused);
    }
    if !focused {
        RETURNED.with_borrow_mut(|returned| returned.remove(&node));
        return None;
    }
    if let Some(modifiers) = RETURNED.with_borrow(|returned| returned.get(&node).map(|r| r.1)) {
        return Some(modifiers);
    }
    DUE.with_borrow_mut(|due| {
        let owed = due.get_mut(&node)?;
        owed.ready(app).then_some(owed.modifiers)
    })
}

/// The view at `node` is gone: what it held and was owed goes with it.
pub(super) fn forget(node: NodeId) {
    DUE.with_borrow_mut(|due| due.remove(&node));
    RETURNED.with_borrow_mut(|returned| returned.remove(&node));
    HELD.with_borrow_mut(|held| held.retain(|_, held| held.owner != node));
}

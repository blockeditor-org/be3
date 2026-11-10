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
//! A press is treated the same way until the editor's app has drawn a frame after it: the
//! press may be what asks for the gift (a button that makes a block to type into), and the ask
//! arrives with that frame, so keys typed right after it wait for the answer instead of reaching
//! the editor first. They go to the editor as soon as the frame shows it asked for nothing.
//!
//! Keys are held from the editor's view that heard the first of them; keys typed into another
//! view of the same editor meanwhile pass, which only a joined dock's several tabs could do.

use std::cell::RefCell;
use std::collections::HashMap;

use be_sdk_api::EditorInstanceId;
use be_sdk_api::ViewId;
use beui::{Event, Forwarded, ForwardedInput, Modifiers, PointerPhase};

/// A view of an app's editor instance. Keys are kept by view, not by the node that catches its
/// input, which the host builds again when the view moves (a child placed in a new pane).
pub(super) type View = (String, EditorInstanceId, ViewId);

/// Keys an editor's view heard while its gift was pending.
struct Held {
    owner: View,
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
    static DUE: RefCell<HashMap<View, Due>> = RefCell::new(HashMap::new());
    static RETURNED: RefCell<HashMap<View, (Vec<Forwarded>, Modifiers)>> =
        RefCell::new(HashMap::new());
    /// Views that forwarded a press their app has yet to answer: the frame request whose
    /// answer took it.
    static PRESSED: RefCell<HashMap<View, u64>> = RefCell::new(HashMap::new());
}

/// Whether the view `own` forwarded a press its app has not drawn a frame after yet. The
/// press may ask to give the keyboard to a child (a button that makes a block to type into), and
/// the ask arrives with that frame, so keys typed meanwhile wait for it as for a gift: they go
/// to the child if the frame asked, and to the view if it did not.
fn answering(own: &View, app: App) -> bool {
    PRESSED.with_borrow_mut(|pressed| match pressed.get(own) {
        Some(after) if app.frames.1 >= *after => {
            pressed.remove(own);
            false
        }
        Some(_) => true,
        None => false,
    })
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
pub(super) fn route(own: &View, pending: bool, app: App, input: &mut ForwardedInput) -> Routed {
    let key = (own.0.clone(), own.1);
    let mut routed = Routed::default();
    let presses = input.events.iter().any(pressed);
    let moved_on = presses || input.events.contains(&Forwarded::Focus(false));
    let holds = pending || answering(own, app);
    let released = HELD.with_borrow_mut(|held| {
        let holding = held.get(&key).is_some_and(|held| held.owner == *own);
        if pending && moved_on && (holding || input.focused) {
            routed.refuse = true;
        }
        if !holds || moved_on {
            if !holding {
                return Vec::new();
            }
            let events = held
                .remove(&key)
                .map(|held| held.events)
                .unwrap_or_default();
            let had_keyboard = input.focused || input.events.contains(&Forwarded::Focus(false));
            return if had_keyboard { events } else { Vec::new() };
        }
        if !input.focused {
            return Vec::new();
        }
        let entry = held.entry(key).or_insert_with(|| Held {
            owner: own.clone(),
            events: Vec::new(),
            modifiers: input.modifiers,
        });
        if entry.owner == *own {
            let (typed, kept): (Vec<_>, Vec<_>) = input.events.drain(..).partition(typed);
            entry.events.extend(typed);
            entry.modifiers = input.modifiers;
            input.events = kept;
        }
        Vec::new()
    });
    deliver(input, released);
    if input.focused {
        let returned = RETURNED.with_borrow_mut(|returned| returned.remove(own));
        deliver(
            input,
            returned.map(|(events, _)| events).unwrap_or_default(),
        );
    }
    let owed = DUE.with_borrow_mut(|due| {
        let owed = due.get_mut(own)?;
        owed.focused |= input.focused;
        let leaving = input.events.contains(&Forwarded::Focus(false));
        if leaving || (input.focused && owed.ready(app)) {
            return due.remove(own).map(|owed| owed.events);
        }
        let (typed, kept): (Vec<_>, Vec<_>) = input.events.drain(..).partition(typed);
        owed.events.extend(typed);
        input.events = kept;
        None
    });
    deliver(input, owed.unwrap_or_default());
    if presses {
        PRESSED.with_borrow_mut(|pressed| pressed.insert(own.clone(), app.frames.0 + 1));
    }
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

/// The editor's gift reached the child's `child` view, which has the keyboard already
/// (`focused`) or is just being given it: the keys held for it are owed to it, and a child just
/// being given it hears its own keys only once it is ready for them. A child that already had
/// the keyboard, with nothing held for it, is left as it is.
pub(super) fn given(editor: (&str, EditorInstanceId), child: View, focused: bool) {
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
pub(super) fn refused(editor: (&str, EditorInstanceId), has_keyboard: impl FnOnce(&View) -> bool) {
    let key = (editor.0.to_owned(), editor.1);
    let Some(held) = HELD.with_borrow_mut(|held| held.remove(&key)) else {
        return;
    };
    if !held.events.is_empty() && has_keyboard(&held.owner) {
        RETURNED
            .with_borrow_mut(|returned| returned.insert(held.owner, (held.events, held.modifiers)));
    }
}

/// Settles what `view` holds and is owed, after its app moved on: keys
/// it held for a gift that is no longer `pending` come back to it (if `focused`), and keys owed
/// to it as a child become due once it is ready. Answers the modifiers to forward it an empty
/// input with, to carry what it can take now.
pub(super) fn owed_now(view: &View, pending: bool, app: App, focused: bool) -> Option<Modifiers> {
    if !pending && !answering(view, app) {
        refused((&view.0, view.1), |owner| owner == view && focused);
    }
    if !focused {
        RETURNED.with_borrow_mut(|returned| returned.remove(view));
        return None;
    }
    if let Some(modifiers) = RETURNED.with_borrow(|returned| returned.get(view).map(|r| r.1)) {
        return Some(modifiers);
    }
    DUE.with_borrow_mut(|due| {
        let owed = due.get_mut(view)?;
        owed.ready(app).then_some(owed.modifiers)
    })
}

/// The view is gone: what it held and was owed goes with it.
pub(super) fn forget(view: &View) {
    PRESSED.with_borrow_mut(|pressed| pressed.remove(view));
    DUE.with_borrow_mut(|due| due.remove(view));
    RETURNED.with_borrow_mut(|returned| returned.remove(view));
    HELD.with_borrow_mut(|held| held.retain(|_, held| held.owner != *view));
}

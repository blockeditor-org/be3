//! An editor giving the keyboard to a child (`EditorHost::focus_child`, `focus_block_child`), and
//! the keys typed while the child's view cannot take them yet. Each editor instance holds at most
//! one gift (`Instance::gift`): what it is given to, the keys held and whose they are. It moves
//! pending → given → ready → done, and `Instances::end_gift` ends it however it ends.
//!
//! - Pending: the editor asked for a block no child shows yet (`GiftTo::Block`) or a child whose
//!   view has not taken the keyboard (`GiftTo::Child`). Keys typed into the editor's view are
//!   held for the child; the first view of the editor to hear one is the holder. A press the
//!   editor's view forwarded is pending the same way (`GiftTo::Answer`) until the app draws the
//!   frame that answers it, as that frame may ask for a gift ("Add notes" asks as it is clicked).
//!   At each layout commit a block gift whose block a child shows turns into a gift to that child,
//!   and the placer's views hear `InstanceEvent::KeyboardGift` naming it.
//! - Given: the child's view took the keyboard (`take_gift`): the keys held are owed to it, and
//!   keys typed into it queue behind them. Its view must be seen with the keyboard by the next
//!   host pass; one that is not lost it before it saw it, and the gift ends.
//! - Ready: a text field in the child has the keyboard (its view asks for an input method in a
//!   frame answered after it had the focus), or, for a child with none, its app drew two frames
//!   after it had its content and the focus. The next input forwarded to it carries the keys.
//! - Done: the keys reached the child, or the gift ended without it. Keys a gift that ended still
//!   holds go back to the holder if it has the keyboard by the next host pass, and are dropped
//!   otherwise.
//!
//! A pending gift ends when the person presses in the editor or takes the keyboard elsewhere, when
//! the child stops being placed in a mode that runs an editor, when the editor has no view left,
//! when the host refused to make its block, and when it is not given after `GIFT_COMMITS` of the
//! asking instance's own layout commits (the commits that carry its children or one of its frame
//! reports), or `GIFT_WAIT` on the frame clock, whichever comes first: the clock bounds a gift to
//! an editor that commits no layout at all. A gift whose instance or app goes ends with it.

use std::time::Duration;

use be_host_base::host;
use be_sdk_api::{ChildId, ChildMode, EditorInstanceId, ViewId};
use beui::{Event, Forwarded, ForwardedInput, Modifiers, PointerPhase};
use uuid::Uuid;

use super::{Instance, Instances, shown_block};
use crate::events::InstanceEvent;

/// A view of an app's editor instance. Keys are kept by view, not by the node that catches its
/// input, which the host builds again when the view moves (a child placed in a new pane).
pub type GiftView = (String, EditorInstanceId, ViewId);

/// How many of its own layout commits an editor has to show the child it gives the keyboard to,
/// or for that child's view to take it. Making the block and placing its child usually takes one
/// or two; an editor that animates commits more, so this leaves room.
pub(crate) const GIFT_COMMITS: u32 = 8;

/// How long on the frame clock a gift waits to be given, for an editor that commits no layout.
pub(crate) const GIFT_WAIT: Duration = Duration::from_secs(3);

/// What an editor gives the keyboard to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GiftTo {
    /// Nothing yet: its view forwarded a press, and the frame that answers it (the frames asked of
    /// the app have to reach this count) may ask for a gift.
    Answer(u64),
    /// The child that will show this block.
    Block(Uuid),
    /// This child, whose view has yet to take the keyboard.
    Child(ChildId),
    /// The view of the child that took it.
    View(GiftView),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Pending,
    /// Given in host pass `pass`. `focused` is the count of frames asked of the child's app when
    /// its view was first seen with the keyboard, and `after` the frame its app has to answer to
    /// be ready without a text field.
    Given {
        pass: u64,
        focused: Option<u64>,
        after: Option<u64>,
    },
    Ready,
    /// Ended in host pass `pass`, with keys the holder takes back if it has the keyboard by the
    /// next pass.
    Done {
        pass: u64,
    },
}

/// The one gift of the keyboard an editor instance makes.
#[derive(Debug)]
pub(crate) struct Gift {
    pub(crate) to: GiftTo,
    stage: Stage,
    /// The editor's view whose keys are held.
    holder: Option<GiftView>,
    keys: Vec<Forwarded>,
    modifiers: Modifiers,
    /// The asking instance's layout commits since it asked, or since a block gift became a gift
    /// to a child.
    commits: u32,
    /// When it was asked, on the frame clock.
    since: Duration,
}

impl Gift {
    fn new(to: GiftTo, holder: Option<GiftView>, now: Duration) -> Self {
        Self {
            to,
            stage: Stage::Pending,
            holder,
            keys: Vec::new(),
            modifiers: Modifiers::NONE,
            commits: 0,
            since: now,
        }
    }

    pub(crate) fn pending(&self) -> bool {
        self.stage == Stage::Pending
    }

    /// Whether the child's view can take the keys owed to it.
    fn ready(&mut self, app: KeyProgress) -> bool {
        let (asked, answered) = app.frames;
        match &mut self.stage {
            Stage::Ready => true,
            Stage::Given {
                focused: Some(focused),
                after,
                ..
            } => {
                if !app.content {
                    // The content went away again (the editor takes a new one): its frames count
                    // from when it is back, as the editor has none of it to take keys until then.
                    *after = None;
                } else if after.is_none() {
                    // The first frame can still be the one that shows the content arrive.
                    *after = Some(asked + 2);
                }
                // An input method asked for in a frame drawn before the focus is a stale one.
                let typing = app.typing && answered > *focused;
                let ready = typing || after.is_some_and(|after| answered >= after);
                if ready {
                    self.stage = Stage::Ready;
                }
                ready
            }
            _ => false,
        }
    }
}

/// Where a view's app stands, as the host sees it.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeyProgress {
    /// Frames asked of the app and answered by it.
    pub frames: (u64, u64),
    /// The block content of the view's editor has reached it.
    pub content: bool,
    /// The view asks for an input method: a text field in it has the keyboard.
    pub typing: bool,
}

/// How a gift that ends treats the keys it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Keys {
    /// They go back to the holder if it has the keyboard.
    Return,
    /// They are dropped: the keyboard is elsewhere.
    Drop,
}

fn typed(event: &Forwarded) -> bool {
    matches!(
        event,
        Forwarded::Input(Event::Key { .. } | Event::Text(_) | Event::Ime(_) | Event::Paste(_))
    )
}

pub fn is_press(event: &Forwarded) -> bool {
    matches!(event, Forwarded::Pointer { event, .. } if matches!(event.phase, PointerPhase::Down(_)))
}

/// Puts `events` after the input's gained focus and before the rest, so they reach the view with
/// the keyboard it has now and ahead of what was typed after them.
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

fn has_views(entry: &Instance) -> bool {
    !(entry.screens.is_empty() && entry.anchored.is_empty())
}

/// The children the instance places in a mode that runs an editor, with the block each shows.
fn placed(entry: &Instance) -> Vec<(ChildId, Option<Uuid>)> {
    let anchored = entry.anchored.iter().map(|held| &held.screen);
    entry
        .screens
        .values()
        .chain(anchored)
        .flat_map(|screen| &screen.children.children)
        .filter(|placed| placed.mode != ChildMode::Preview)
        .map(|placed| (placed.child, shown_block(&placed.content)))
        .collect()
}

impl Instances {
    /// The instances a layout commit carries something of: a frame report of one of their views,
    /// or their children. These are the commits a gift counts.
    pub(crate) fn committed(
        &self,
        reports: &[be_sdk_api::FrameReport],
        children: &[be_sdk_api::ChildPlacements],
    ) -> Vec<EditorInstanceId> {
        let mut committed: Vec<EditorInstanceId> =
            children.iter().map(|placed| placed.instance).collect();
        for (instance, entry) in &self.entries {
            let mut screens = entry
                .screens
                .values()
                .chain(entry.anchored.iter().map(|held| &held.screen));
            if screens.any(|screen| {
                reports
                    .iter()
                    .any(|report| report.screen == screen.request.screen)
            }) {
                committed.push(*instance);
            }
        }
        committed.sort_by_key(|instance| instance.0);
        committed.dedup();
        committed
    }

    /// The instance asks to give the keyboard to `to`. Only the latest ask stands: the person can
    /// type into one place. Keys it holds already (for a press whose frame asked) stay held.
    pub(crate) fn ask_gift(&mut self, instance: EditorInstanceId, to: GiftTo) {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return;
        };
        let now = host::now();
        let gift = match entry.gift.take() {
            Some(mut held) if matches!(held.stage, Stage::Pending | Stage::Done { .. }) => {
                held.to = to.clone();
                held.stage = Stage::Pending;
                held.commits = 0;
                held.since = now;
                held
            }
            _ => Gift::new(to.clone(), None, now),
        };
        entry.gift = Some(gift);
        if let GiftTo::Child(child) = to {
            super::super::events::emit(
                &self.app_id,
                instance,
                InstanceEvent::KeyboardGift(Some(child)),
            );
        }
    }

    /// Ends the instance's gift: the keys it holds go back to the holder or are dropped (`keys`).
    /// Every gift ends here, however it ends.
    pub(crate) fn end_gift(&mut self, instance: EditorInstanceId, keys: Keys) {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return;
        };
        let Some(mut gift) = entry.gift.take() else {
            return;
        };
        let told = !matches!(gift.to, GiftTo::Answer(_)) && !matches!(gift.stage, Stage::Done { .. });
        if keys == Keys::Return && !gift.keys.is_empty() {
            if !matches!(gift.stage, Stage::Done { .. }) {
                gift.stage = Stage::Done { pass: host::pass() };
            }
            entry.gift = Some(gift);
        }
        if told {
            // The placer's views stop waiting for a child to take it, and the holder takes back
            // what it held.
            super::super::events::emit(&self.app_id, instance, InstanceEvent::KeyboardGift(None));
        }
    }

    /// Whether the instance's gift is pending: keys typed into its view wait for the child.
    #[cfg(test)]
    pub(crate) fn gift_pending(&self, instance: EditorInstanceId) -> bool {
        self.entries
            .get(&instance)
            .and_then(|entry| entry.gift.as_ref())
            .is_some_and(Gift::pending)
    }

    /// Whether the instance gives the keyboard to `child`, whose view has yet to take it.
    pub(crate) fn gives_to(&self, instance: EditorInstanceId, child: ChildId) -> bool {
        self.entries
            .get(&instance)
            .and_then(|entry| entry.gift.as_ref())
            .is_some_and(|gift| gift.pending() && gift.to == GiftTo::Child(child))
    }

    /// The child's view `view` took the keyboard the instance gives `child`, in host pass `pass`,
    /// already having it (`focused`) or being given it now. Its app had been asked `asked` frames.
    /// Answers whether the gift was the instance's to take.
    pub(crate) fn take_gift(
        &mut self,
        instance: EditorInstanceId,
        child: ChildId,
        view: GiftView,
        focused: bool,
        (asked, pass): (u64, u64),
    ) -> bool {
        if !self.gives_to(instance, child) {
            return false;
        }
        let Some(gift) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.gift.as_mut())
        else {
            return false;
        };
        if focused && gift.keys.is_empty() {
            // A child that already has the keyboard, with nothing held for it, is left as it is.
            self.end_gift(instance, Keys::Drop);
            return true;
        }
        gift.to = GiftTo::View(view);
        gift.stage = Stage::Given {
            pass,
            focused: focused.then_some(asked),
            after: None,
        };
        true
    }

    /// The view `view` of the instance forwarded a press while `asked` frames had been asked of
    /// its app: the next frame it draws answers it, and keys typed meanwhile wait for that frame.
    /// Only a press the app was sent counts, as the app draws no frame for another.
    pub(crate) fn sent_press(&mut self, instance: EditorInstanceId, view: GiftView, asked: u64) {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return;
        };
        if entry.gift.as_ref().is_some_and(|gift| {
            gift.pending() || matches!(gift.stage, Stage::Given { .. } | Stage::Ready)
        }) {
            return;
        }
        entry.gift = Some(Gift::new(
            GiftTo::Answer(asked + 1),
            Some(view),
            host::now(),
        ));
    }

    /// Holds the typed events of input forwarded to `view` of the instance while its gift is
    /// pending, and hands it keys its gift gave back. `app` is where its app stands, and `pass`
    /// the host pass.
    pub(crate) fn hold_keys(
        &mut self,
        instance: EditorInstanceId,
        view: &GiftView,
        app: KeyProgress,
        input: &mut ForwardedInput,
        pass: u64,
    ) {
        let Some(gift) = self
            .entries
            .get(&instance)
            .and_then(|entry| entry.gift.as_ref())
        else {
            return;
        };
        let holding = gift.holder.as_ref() == Some(view);
        let moved_on = input.events.iter().any(is_press)
            || input.events.contains(&Forwarded::Focus(false));
        if gift.pending() {
            let answered = matches!(gift.to, GiftTo::Answer(after) if app.frames.1 >= after);
            if answered || (moved_on && (holding || input.focused)) {
                self.end_gift(instance, Keys::Return);
            }
        }
        let Some(entry) = self.entries.get_mut(&instance) else {
            return;
        };
        let Some(gift) = entry.gift.as_mut() else {
            return;
        };
        if let Stage::Done { pass: ended } = gift.stage {
            if gift.holder.as_ref() != Some(view) {
                return;
            }
            let had_keyboard = input.focused || input.events.contains(&Forwarded::Focus(false));
            let keys = entry.gift.take().map(|gift| gift.keys).unwrap_or_default();
            if had_keyboard && pass <= ended + 1 {
                deliver(input, keys);
            }
            return;
        }
        if !gift.pending() || !input.focused {
            return;
        }
        let holder = gift.holder.get_or_insert_with(|| view.clone());
        if holder == view {
            let (typed, kept): (Vec<_>, Vec<_>) = input.events.drain(..).partition(typed);
            gift.keys.extend(typed);
            gift.modifiers = input.modifiers;
            input.events = kept;
        }
    }

    /// Hands the child's view `view` the keys the instance's gift owes it once it can take them,
    /// and queues its own typed keys behind them until then. `app` is where the child's app
    /// stands, and `pass` the host pass.
    pub(crate) fn owe_keys(
        &mut self,
        instance: EditorInstanceId,
        view: &GiftView,
        app: KeyProgress,
        input: &mut ForwardedInput,
        pass: u64,
    ) {
        let leaving = input.events.contains(&Forwarded::Focus(false));
        if !self.sees_gift(instance, view, input.focused || leaving, app, pass) {
            return;
        }
        let Some(gift) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.gift.as_mut())
        else {
            return;
        };
        if leaving || (input.focused && gift.ready(app)) {
            let keys = std::mem::take(&mut gift.keys);
            self.end_gift(instance, Keys::Drop);
            deliver(input, keys);
            return;
        }
        let (typed, kept): (Vec<_>, Vec<_>) = input.events.drain(..).partition(typed);
        gift.keys.extend(typed);
        input.events = kept;
    }

    /// Whether the instance's gift is given to `view`, now seen with the keyboard (`focused`) or
    /// not. A view not seen with it by the pass after it was given lost it before it saw it, and
    /// the gift ends: the keys go back to the holder, which would not hear them otherwise.
    fn sees_gift(
        &mut self,
        instance: EditorInstanceId,
        view: &GiftView,
        focused: bool,
        app: KeyProgress,
        pass: u64,
    ) -> bool {
        let Some(gift) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.gift.as_mut())
        else {
            return false;
        };
        if gift.to != GiftTo::View(view.clone()) {
            return false;
        }
        match &mut gift.stage {
            Stage::Given {
                pass: given,
                focused: seen @ None,
                ..
            } => {
                if focused && pass <= *given + 1 {
                    *seen = Some(app.frames.0);
                    return true;
                }
            }
            Stage::Given { .. } | Stage::Ready => return true,
            _ => return false,
        }
        self.end_gift(instance, Keys::Return);
        false
    }

    /// Settles what `view` of the instance holds and, as a child its placer `placer` gave the
    /// keyboard to, is owed, after its app moved on: `focused` says whether it has the keyboard.
    /// Answers the modifiers to forward it an empty input with, to carry what it can take now.
    pub(crate) fn settle_own_keys(
        &mut self,
        instance: EditorInstanceId,
        view: &GiftView,
        app: KeyProgress,
        focused: bool,
        pass: u64,
    ) -> Option<Modifiers> {
        let gift = self.entries.get(&instance)?.gift.as_ref()?;
        if matches!(gift.to, GiftTo::Answer(after) if app.frames.1 >= after) && gift.pending() {
            self.end_gift(instance, Keys::Return);
        }
        let gift = self.entries.get_mut(&instance)?.gift.as_mut()?;
        let Stage::Done { pass: ended } = gift.stage else {
            return None;
        };
        if gift.holder.as_ref() != Some(view) {
            return None;
        }
        if focused && pass <= ended + 1 {
            return Some(gift.modifiers);
        }
        self.entries.get_mut(&instance)?.gift = None;
        None
    }

    /// The child's side of `settle_own_keys`: the keys the instance's gift owes `view`, once it
    /// can take them.
    pub(crate) fn settle_owed_keys(
        &mut self,
        instance: EditorInstanceId,
        view: &GiftView,
        app: KeyProgress,
        focused: bool,
        pass: u64,
    ) -> Option<Modifiers> {
        if !self.sees_gift(instance, view, focused, app, pass) {
            return None;
        }
        if !focused {
            // It had the keyboard and lost it without an input saying so: what it was owed is
            // dropped, as typing into a view without the keyboard would be.
            self.end_gift(instance, Keys::Drop);
            return None;
        }
        let gift = self.entries.get_mut(&instance)?.gift.as_mut()?;
        gift.ready(app).then_some(gift.modifiers)
    }

    /// The view `view` is gone: what it held and was owed goes with it.
    pub(crate) fn forget_keys(&mut self, view: &GiftView) {
        let mut ended = Vec::new();
        for (instance, entry) in &mut self.entries {
            let Some(gift) = &mut entry.gift else {
                continue;
            };
            if gift.to == GiftTo::View(view.clone()) {
                ended.push(*instance);
            } else if gift.holder.as_ref() == Some(view) {
                gift.holder = None;
                gift.keys.clear();
                if !gift.pending() || matches!(gift.to, GiftTo::Answer(_)) {
                    entry.gift = None;
                }
            }
        }
        for instance in ended {
            self.end_gift(instance, Keys::Drop);
        }
    }

    /// After a layout commit that carried what the instances in `committed` laid out: a block gift
    /// whose block a child now shows becomes a gift to that child, and a pending gift ends when its
    /// child is no longer placed, when the editor has no view left, or when the editor has
    /// committed `GIFT_COMMITS` layouts since it asked without the gift moving on.
    pub(crate) fn settle_gifts(&mut self, committed: &[EditorInstanceId]) {
        let mut ended = Vec::new();
        let mut given = Vec::new();
        for (instance, entry) in &mut self.entries {
            let Some(gift) = &entry.gift else {
                continue;
            };
            if !gift.pending() || matches!(gift.to, GiftTo::Answer(_)) {
                continue;
            }
            if !has_views(entry) {
                ended.push(*instance);
                continue;
            }
            let placed = placed(entry);
            let gift = entry.gift.as_mut().expect("checked above");
            match gift.to {
                GiftTo::Block(block) => {
                    if let Some((child, _)) = placed.iter().find(|(_, shows)| *shows == Some(block))
                    {
                        given.push((*instance, *child));
                        continue;
                    }
                }
                GiftTo::Child(child) => {
                    if !placed.iter().any(|(placed, _)| *placed == child) {
                        ended.push(*instance);
                        continue;
                    }
                }
                _ => {}
            }
            if committed.contains(instance) {
                gift.commits += 1;
                if gift.commits >= GIFT_COMMITS {
                    ended.push(*instance);
                }
            }
        }
        for (instance, child) in given {
            self.ask_gift(instance, GiftTo::Child(child));
        }
        for instance in ended {
            self.end_gift(instance, Keys::Return);
        }
    }

    /// Ends the pending gifts that waited `GIFT_WAIT` on the frame clock, at `now`. Answers how
    /// long until the next one would, to wake the host for it.
    pub(crate) fn expire_gifts(&mut self, now: Duration) -> Option<Duration> {
        let mut ended = Vec::new();
        let mut next: Option<Duration> = None;
        for (instance, entry) in &self.entries {
            let Some(gift) = entry.gift.as_ref().filter(|gift| gift.pending()) else {
                continue;
            };
            let waited = now.saturating_sub(gift.since);
            match GIFT_WAIT.checked_sub(waited).filter(|left| !left.is_zero()) {
                Some(left) => next = Some(next.map_or(left, |next| next.min(left))),
                None => ended.push(*instance),
            }
        }
        for instance in ended {
            self.end_gift(instance, Keys::Return);
        }
        next
    }

    /// Notes that the host refused to make `block` for the instance, so a gift of the keyboard to
    /// the child that would have shown it never starts, or ends.
    pub(crate) fn refuse_block_gift(&mut self, instance: EditorInstanceId, block: Uuid) {
        let asked = self
            .entries
            .get(&instance)
            .and_then(|entry| entry.gift.as_ref())
            .is_some_and(|gift| gift.pending() && gift.to == GiftTo::Block(block));
        if asked {
            self.end_gift(instance, Keys::Return);
        }
        if let Some(entry) = self.entries.get_mut(&instance) {
            entry.block_refused.push(block);
            let extra = entry.block_refused.len().saturating_sub(8);
            entry.block_refused.drain(..extra);
        }
    }

    /// The instance asks to give the keyboard to the child that will show `block`, unless the host
    /// already refused to make it (the ask comes after the failed call).
    pub(crate) fn ask_block_gift(&mut self, instance: EditorInstanceId, block: Uuid) {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return;
        };
        if let Some(at) = entry
            .block_refused
            .iter()
            .position(|refused| *refused == block)
        {
            entry.block_refused.remove(at);
            return;
        }
        self.ask_gift(instance, GiftTo::Block(block));
    }
}

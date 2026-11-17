//! Which of a block's editor messages its tentative gestures hold, followed from the messages
//! alone, so the host keeping them to give again and the replica counting them agree on every
//! one (plans/tentative_gestures.md, Counting and replay).
//!
//! A tentative gesture's begin, commit and cancel never count as local edits. Its operations
//! count once the layer commits: when the last open tentative gesture of the block ends, every
//! operation of the gestures that committed counts, in the order they arrived, before whatever
//! message ended it. Until then each is held: counted by neither side, and kept in the host's
//! replay where it arrived, so a replica opened again rebuilds the layer.

use be_block_content::history::GestureId;
use be_sdk_type_api::EditorId;

use crate::editor::{FromEditor, Gesture};

/// The id every gesture and edit of an editor goes under until the app protocol names its
/// gestures (plans/tentative_gestures.md, step 3): an editor holds one gesture at a time.
pub const EDITOR_GESTURE: u64 = 0;

/// The gesture `editor`'s gesture messages name.
pub fn gesture_of(editor: EditorId) -> GestureId {
    GestureId {
        editor: Some(editor),
        id: EDITOR_GESTURE,
    }
}

#[derive(Clone, Copy, Debug)]
struct HeldMessage {
    serial: u64,
    gesture: GestureId,
    operation: bool,
}

/// The tentative gestures a block's editors hold open, and the messages they hold.
#[derive(Clone, Debug, Default)]
pub struct Holds {
    /// Open, in the order they began.
    open: Vec<GestureId>,
    /// The messages held, in the order they arrived.
    held: Vec<HeldMessage>,
    next: u64,
}

/// What became of one message.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum This {
    /// It counts as a local edit, after `Took::counted`, if it is one.
    #[default]
    Counted,
    /// Held under this serial.
    Held(u64),
    /// A commit or cancel that ended its gesture: neither counted nor kept.
    Ended,
}

/// What one message (or an undo, or an editor leaving) did to the holds.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Took {
    pub this: This,
    /// The tentative gestures it ended, in order, and whether each committed.
    pub ended: Vec<(GestureId, bool)>,
    /// The tentative gesture it opened.
    pub opened: Option<GestureId>,
    /// Whether the layer settled: the last open tentative gesture ended.
    pub settled: bool,
    /// The held operations that count as local edits now, in the order they arrived.
    pub counted: Vec<u64>,
    /// Every held message let go: those of `counted`, and the rest of the ended gestures'.
    pub released: Vec<u64>,
}

impl Holds {
    /// Follows a message `editor` sent (none when it does not decode).
    pub fn message(&mut self, editor: EditorId, message: Option<&FromEditor>) -> Took {
        let gesture = gesture_of(editor);
        let open = self.open.contains(&gesture);
        let mut took = Took::default();
        match message {
            Some(FromEditor::Gesture(Gesture::BeginTentative)) => {
                // One open under this name commits first, as the replica's history does.
                if open {
                    self.end(gesture, true, &mut took);
                }
                self.open.push(gesture);
                took.opened = Some(gesture);
                took.this = This::Held(self.hold(gesture, false));
            }
            Some(FromEditor::Gesture(Gesture::Begin)) if open => {
                self.end(gesture, true, &mut took);
            }
            Some(FromEditor::Gesture(Gesture::Commit)) if open => {
                let serial = self.hold(gesture, false);
                self.end(gesture, true, &mut took);
                took.this = match took.released.contains(&serial) {
                    true => This::Ended,
                    false => This::Held(serial),
                };
            }
            Some(FromEditor::Gesture(Gesture::Cancel)) if open => {
                self.hold(gesture, false);
                self.end(gesture, false, &mut took);
                took.this = This::Ended;
            }
            Some(FromEditor::Operate(_)) if open => {
                took.this = This::Held(self.hold(gesture, true));
            }
            _ => {}
        }
        took
    }

    /// An undo while a layer is held cancels all of it, committed gestures waiting for the rest
    /// too: nothing of it counts.
    pub fn undo(&mut self) -> Took {
        let mut took = Took::default();
        if self.open.is_empty() {
            return took;
        }
        took.ended = std::mem::take(&mut self.open)
            .into_iter()
            .map(|gesture| (gesture, false))
            .collect();
        took.released = std::mem::take(&mut self.held)
            .into_iter()
            .map(|held| held.serial)
            .collect();
        took.settled = true;
        took
    }

    /// `editor` left: its open tentative gesture is cancelled.
    pub fn detach(&mut self, editor: EditorId) -> Took {
        let mut took = Took::default();
        let gesture = gesture_of(editor);
        if self.open.contains(&gesture) {
            self.end(gesture, false, &mut took);
        }
        took
    }

    /// Whether a tentative gesture is open.
    pub fn holding(&self) -> bool {
        !self.open.is_empty()
    }

    /// The open tentative gestures, in the order they began.
    pub fn open(&self) -> &[GestureId] {
        &self.open
    }

    fn hold(&mut self, gesture: GestureId, operation: bool) -> u64 {
        self.next += 1;
        self.held.push(HeldMessage {
            serial: self.next,
            gesture,
            operation,
        });
        self.next
    }

    fn end(&mut self, gesture: GestureId, committed: bool, took: &mut Took) {
        self.open.retain(|open| *open != gesture);
        took.ended.push((gesture, committed));
        if !committed {
            self.held.retain(|held| {
                let ours = held.gesture == gesture;
                if ours {
                    took.released.push(held.serial);
                }
                !ours
            });
        }
        if self.open.is_empty() {
            took.settled = true;
            for held in std::mem::take(&mut self.held) {
                if held.operation {
                    took.counted.push(held.serial);
                }
                took.released.push(held.serial);
            }
        }
    }
}

#[cfg(test)]
mod tests;

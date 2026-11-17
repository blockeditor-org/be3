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

/// The gesture `message`, from `editor`, names: the one it begins or ends, or the one an edit
/// was made in.
pub fn gesture_of(editor: EditorId, message: &FromEditor) -> Option<GestureId> {
    let id = match message {
        FromEditor::Gesture { id, .. } => *id,
        FromEditor::Operate {
            gesture: Some(id), ..
        } => *id,
        _ => return None,
    };
    Some(GestureId {
        editor: Some(editor),
        id,
    })
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
    /// Open ones an undo cancelled: their editors have yet to end them, and nothing of them
    /// counts.
    voided: Vec<GestureId>,
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
    /// The gesture the message named.
    pub gesture: Option<GestureId>,
    /// The tentative gestures it ended, in order, and whether each committed.
    pub ended: Vec<(GestureId, bool)>,
    /// The open tentative gestures an undo cancelled, which stay open until their editors end
    /// them, holding what they send and counting none of it.
    pub voided: Vec<GestureId>,
    /// An undo found a tentative gesture open: it cancels the layer and does nothing else.
    pub undone: bool,
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
        let mut took = Took::default();
        let Some((message, gesture)) =
            message.and_then(|message| Some((message, gesture_of(editor, message)?)))
        else {
            return took;
        };
        took.gesture = Some(gesture);
        let open = self.open.contains(&gesture);
        let phase = match message {
            FromEditor::Gesture { gesture, .. } => Some(*gesture),
            _ => None,
        };
        match phase {
            Some(Gesture::BeginTentative) => {
                // One open under this name commits first, as the replica's history does.
                if open {
                    self.end(gesture, true, &mut took);
                }
                self.open.push(gesture);
                took.opened = Some(gesture);
                took.this = This::Held(self.hold(gesture, false));
            }
            Some(Gesture::Begin) if open => {
                self.end(gesture, true, &mut took);
            }
            Some(Gesture::Commit) if open => {
                let serial = self.hold(gesture, false);
                self.end(gesture, true, &mut took);
                took.this = match took.released.contains(&serial) {
                    true => This::Ended,
                    false => This::Held(serial),
                };
            }
            Some(Gesture::Cancel) if open => {
                self.hold(gesture, false);
                self.end(gesture, false, &mut took);
                took.this = This::Ended;
            }
            None if open => {
                took.this = This::Held(self.hold(gesture, true));
            }
            _ => {}
        }
        took
    }

    /// An undo while a layer is held cancels all of it, committed gestures waiting for the rest
    /// too: nothing of it counts. The open gestures stay open, voided, since their editors go on
    /// sending until they hear: what they send is held, and released uncounted when they end.
    pub fn undo(&mut self) -> Took {
        let mut took = Took::default();
        if self.open.is_empty() {
            return took;
        }
        took.undone = true;
        for gesture in &self.open {
            if !self.voided.contains(gesture) {
                self.voided.push(*gesture);
                took.voided.push(*gesture);
            }
        }
        took.released = std::mem::take(&mut self.held)
            .into_iter()
            .map(|held| held.serial)
            .collect();
        took
    }

    /// `editor` left: its open tentative gestures are cancelled, in the order they began.
    pub fn detach(&mut self, editor: EditorId) -> Took {
        let mut took = Took::default();
        let leaving: Vec<GestureId> = self
            .open
            .iter()
            .copied()
            .filter(|gesture| gesture.editor == Some(editor))
            .collect();
        for gesture in leaving {
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
        // A voided gesture keeps nothing, however it ends.
        let voided = self.voided.contains(&gesture);
        self.voided.retain(|open| *open != gesture);
        if !committed || voided {
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

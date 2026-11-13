//! Where the text was left: the line at the top of the view and the caret, kept in the view's
//! state so a document reopened from the files list, or after the app was killed, opens where it
//! was. The shell keeps a closed tab's view (guides/blocks/workspaces.md, Sessions), so this
//! outlives the tab.
//!
//! Each place is kept as its byte and the text around it, so it follows edits made while the
//! text was closed (`Place`).

use std::cell::RefCell;
use std::time::Duration;

use be_sdk_beui::be_block_content::ViewState;
use beui::reactive::*;
use beui::styled::ScrollAnchor;
use beui_text_editor_core::EditorCommand;
use serde::{Deserialize, Serialize};

use super::state::State;

pub(crate) const POSITION: &str = "position";

/// How long the view and caret hold still before where they are is written.
const SAVE_AFTER: Duration = Duration::from_millis(500);

/// How much of the text on each side of a place is kept to find it again.
const CONTEXT: usize = 16;

/// How far from its byte a place's text is looked for.
const SEARCH: usize = 64 * 1024;

/// A place in the text: its byte, and the text on either side of it, which finds it again when
/// edits made while the document was closed (a peer's, a rewrite from disk) moved it. A position
/// in the text would not do: the text is stored as its bytes, so every load numbers it anew.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Place {
    pub byte: u64,
    pub before: String,
    pub after: String,
}

impl Place {
    fn at(bytes: &[u8], byte: usize) -> Self {
        let byte = boundary(bytes, byte.min(bytes.len()));
        let start = boundary(bytes, byte.saturating_sub(CONTEXT));
        let end = boundary(bytes, (byte + CONTEXT).min(bytes.len()));
        Self {
            byte: byte as u64,
            before: String::from_utf8_lossy(&bytes[start..byte]).into_owned(),
            after: String::from_utf8_lossy(&bytes[byte..end]).into_owned(),
        }
    }

    /// The byte it is at now: where the text around it is found nearest its old byte (both
    /// sides, else either), else its old byte, moved back to the start of the character it falls
    /// in.
    fn find(&self, bytes: &[u8]) -> usize {
        let byte = boundary(
            bytes,
            usize::try_from(self.byte)
                .unwrap_or(usize::MAX)
                .min(bytes.len()),
        );
        let both = format!("{}{}", self.before, self.after);
        let tries = [
            (both.as_bytes(), self.before.len()),
            (self.before.as_bytes(), self.before.len()),
            (self.after.as_bytes(), 0),
        ];
        let from = byte.saturating_sub(SEARCH);
        let to = (byte + SEARCH).min(bytes.len());
        for (needle, split) in tries {
            if needle.is_empty() {
                continue;
            }
            let nearest = memchr::memmem::find_iter(&bytes[from..to], needle)
                .map(|at| from + at + split)
                .min_by_key(|found| found.abs_diff(byte));
            if let Some(found) = nearest {
                return found;
            }
        }
        byte
    }
}

/// `byte`, moved back to the start of the character it falls in.
fn boundary(bytes: &[u8], mut byte: usize) -> usize {
    while byte > 0 && bytes.get(byte).is_some_and(|next| next & 0xc0 == 0x80) {
        byte -= 1;
    }
    byte
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Position {
    pub line_start: Place,
    pub within: f32,
    pub anchor: Place,
    pub focus: Place,
}

impl Position {
    /// Where the text is now; none while a `scroll_to` is still on its way, whose top line is
    /// not yet where the view will be.
    fn of(state: State) -> Option<Self> {
        if state.text.pending_scroll().is_some() {
            return None;
        }
        let scrolled = state.text.scrolled().get_untracked()?;
        let bytes = state.text.bytes();
        let (anchor, focus) = state.text.with_core(|core| {
            let cursor = core.cursor_positions().first()?;
            Some((
                core.position_index(cursor.pos.anchor)?,
                core.position_index(cursor.pos.focus)?,
            ))
        })?;
        Some(Self {
            line_start: Place::at(&bytes, scrolled.line_start),
            within: scrolled.within,
            anchor: Place::at(&bytes, anchor),
            focus: Place::at(&bytes, focus),
        })
    }

    fn restore(&self, state: State) {
        let bytes = state.text.bytes();
        let line = self.line_start.find(&bytes);
        let line_start = bytes[..line]
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |newline| newline + 1);
        let (anchor, focus) = state.text.with_core(|core| {
            (
                core.position(self.anchor.find(&bytes)),
                core.position(self.focus.find(&bytes)),
            )
        });
        state
            .text
            .execute(EditorCommand::SetSelection { anchor, focus });
        state.text.scroll_to(ScrollAnchor {
            line_start,
            within: self.within,
        });
    }
}

/// Restores the saved position once the text and its view have loaded, and saves it again when
/// the view or caret has held still for a moment, or the app is hidden. Nothing is saved as the
/// editor closes: the view may move as it is taken down, and a place left less than
/// `SAVE_AFTER` before closing is not kept.
pub(crate) fn keep_position(state: State) {
    let editor = state.editor;
    let loaded = state.loaded;
    let restored = StoredCell::new(false);
    let saved = store(RefCell::new(None::<Position>));
    create_binding(move || {
        if restored.get() || !loaded.get() {
            return;
        }
        let Some(view) = editor.view_content() else {
            restored.set(true);
            return;
        };
        let Ok(position) = view.read(|held| {
            held.root()
                .state(POSITION)
                .and_then(ViewState::value::<Position>)
        }) else {
            return;
        };
        restored.set(true);
        if let Some(position) = &position {
            untrack(|| position.restore(state));
        }
        *saved.value().borrow_mut() = position;
    });
    let save = move || {
        if !restored.get() {
            return;
        }
        let Some(position) = Position::of(state) else {
            return;
        };
        if saved.value().borrow().as_ref() != Some(&position) {
            editor.set_view_state(POSITION, Some(&ViewState::new(&position, Vec::new())));
            *saved.value().borrow_mut() = Some(position);
        }
    };
    let saving = create_timer(move || {
        save();
        None
    });
    let scrolled = state.text.scrolled();
    let cursors = state.text.cursors();
    // Not on typing alone: each save is an edit to the view block, kept in its history. The
    // text around the caret finds it again when typing moved it since.
    create_effect(move || {
        scrolled.get();
        cursors.get();
        untrack(|| saving.start(SAVE_AFTER));
    });
    let visible = editor.presence_visible();
    create_effect(move || {
        if !visible.get() {
            untrack(save);
        }
    });
}

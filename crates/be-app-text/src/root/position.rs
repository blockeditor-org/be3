//! Where the text was left: the line at the top of the view and the caret, kept in the view's
//! state so a document reopened from the files list, or after the app was killed, opens where it
//! was. The shell keeps a closed tab's view (guides/blocks/workspaces.md, Sessions), so this
//! outlives the tab.
//!
//! Each place is kept as its byte and the text around it, so it follows edits made while the
//! text was closed (`Place`).

use be_sdk_beui::be_block_content::ViewStateKind;
use beui::reactive::*;
use beui::styled::ScrollAnchor;
use beui_text_editor_core::EditorCommand;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::state::State;

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

impl ViewStateKind for Position {
    const ID: Uuid = Uuid::from_u128(0x7465_7874_2d70_6f73_6974_696f_6e00_0001);
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

/// Restores the saved position once the text and its view have loaded, and saves where the
/// view and caret are whenever they change.
pub(crate) fn keep_position(state: State) {
    let saved = state.editor.view_state::<Position>();
    let loaded = state.loaded;
    let restored = state.position_restored;
    create_binding(move || {
        if restored.get_untracked() || !loaded.get() {
            return;
        }
        let position = match saved.get() {
            Ok(position) => position,
            // Not read yet: it is tried again once it is. A place the text was opened at waits
            // for it, or putting the position back would move the caret off that place.
            Err(Pending::Loading) => return,
            // A view that cannot be read holds no position to put back.
            Err(Pending::Failed(_)) => None,
        };
        if let Some(position) = &position {
            untrack(|| position.restore(state));
        }
        restored.set(true);
    });
    let scrolled = state.text.scrolled();
    let cursors = state.text.cursors();
    // The text around each place is read untracked: it is what finds the place again after
    // edits, so typing alone saves only as it moves the caret.
    let position = create_memo(move || {
        scrolled.track();
        cursors.track();
        if !restored.get() {
            return None;
        }
        untrack(|| Position::of(state))
    });
    create_effect(move || {
        if let Some(position) = position.get() {
            untrack(|| saved.set(Some(position)));
        }
    });
}

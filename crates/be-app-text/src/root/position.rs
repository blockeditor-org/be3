//! Where the text was left: the line at the top of the view and the caret, kept in the view's
//! state so a document reopened from the files list, or after the app was killed, opens where it
//! was. The shell keeps a closed tab's view (guides/blocks/workspaces.md, Sessions), so this
//! outlives the tab.
//!
//! Places are kept as positions in the text, not byte offsets, so they follow a peer's edits
//! and a rewrite from disk made while the text was closed.

use std::time::Duration;

use be_sdk_beui::be_block::ViewState;
use beui::reactive::*;
use beui::styled::ScrollAnchor;
use beui_text_editor_core::{EditorCommand, Position as TextPosition};
use serde::{Deserialize, Serialize};

use super::state::State;

pub(crate) const POSITION: &str = "position";

/// How long the view and caret hold still before where they are is written.
const SAVE_AFTER: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Position {
    pub line_start: TextPosition,
    pub within: f32,
    pub anchor: TextPosition,
    pub focus: TextPosition,
}

impl Position {
    /// Where the text is now; none while a `scroll_to` is still on its way, whose top line is
    /// not yet where the view will be.
    fn of(state: State) -> Option<Self> {
        if state.text.pending_scroll().is_some() {
            return None;
        }
        let scrolled = state.text.scrolled().get_untracked()?;
        state.text.with_core(|core| {
            let cursor = core.cursor_positions().first()?;
            Some(Self {
                line_start: core.position(scrolled.line_start),
                within: scrolled.within,
                anchor: cursor.pos.anchor,
                focus: cursor.pos.focus,
            })
        })
    }

    fn restore(self, state: State) {
        state.text.execute(EditorCommand::SetSelection {
            anchor: self.anchor,
            focus: self.focus,
        });
        let Some(line_start) = state.text.with_core(|core| core.position_index(self.line_start))
        else {
            return;
        };
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
    let saved = StoredCell::new(None::<Position>);
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
        saved.set(position);
        if let Some(position) = position {
            untrack(|| position.restore(state));
        }
    });
    let save = move || {
        if !restored.get() {
            return;
        }
        let Some(position) = Position::of(state) else {
            return;
        };
        if saved.get() != Some(position) {
            saved.set(Some(position));
            editor.set_view_state(POSITION, Some(&ViewState::new(&position, Vec::new())));
        }
    };
    let saving = create_timer(move || {
        save();
        None
    });
    let scrolled = state.text.scrolled();
    let cursors = state.text.cursors();
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

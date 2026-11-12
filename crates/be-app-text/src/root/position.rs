//! Where the text was left: the line at the top of the view and the caret, kept in the view's
//! state so a document reopened from the files list, or after the app was killed, opens where it
//! was. The shell keeps a closed tab's view (guides/blocks/workspaces.md, Sessions), so this
//! outlives the tab.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use be_sdk_beui::be_block::ViewState;
use beui::reactive::*;
use beui::styled::ScrollAnchor;
use beui_text_editor_core::EditorCommand;
use serde::{Deserialize, Serialize};

use super::state::State;

pub(crate) const POSITION: &str = "position";

/// How long the view and caret hold still before where they are is written.
const SAVE_AFTER: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Position {
    pub line_start: u64,
    pub within: f32,
    pub anchor: u64,
    pub focus: u64,
}

impl Position {
    fn of(state: State) -> Option<Self> {
        let scrolled = state.text.scrolled().get_untracked()?;
        let (anchor, focus) = state.text.with_core(|core| {
            let cursor = core.cursor_positions().first()?;
            Some((
                core.position_index(cursor.pos.anchor)?,
                core.position_index(cursor.pos.focus)?,
            ))
        })?;
        Some(Self {
            line_start: scrolled.line_start as u64,
            within: scrolled.within,
            anchor: anchor as u64,
            focus: focus as u64,
        })
    }

    fn restore(self, state: State) {
        let len = state.text.bytes().len();
        let at = |byte: u64| usize::try_from(byte).unwrap_or(usize::MAX).min(len);
        let (anchor, focus) =
            state.text.with_core(|core| (core.position(at(self.anchor)), core.position(at(self.focus))));
        state
            .text
            .execute(EditorCommand::SetSelection { anchor, focus });
        state.text.scroll_to(ScrollAnchor {
            line_start: at(self.line_start),
            within: self.within,
        });
    }
}

/// Restores the saved position once the text and its view have loaded, and saves it again when
/// the view or caret has held still for a moment, or the app is hidden or closed.
pub(crate) fn keep_position(state: State) {
    let editor = state.editor;
    let core = editor.core();
    let loaded = state.loaded;
    // Plain cells, not reactive ones: the last save runs while the editor is torn down.
    let restored = Rc::new(Cell::new(false));
    let latest = Rc::new(Cell::new(None::<Position>));
    let saved = Rc::new(Cell::new(None::<Position>));
    create_binding({
        let (restored, saved) = (Rc::clone(&restored), Rc::clone(&saved));
        move || {
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
        }
    });
    let save = Rc::new({
        let latest = Rc::clone(&latest);
        move || {
        let Some(position) = latest.get() else {
            return;
        };
        if saved.replace(Some(position)) != Some(position) {
            core.set_view_state(POSITION, Some(&ViewState::new(&position, Vec::new())));
        }
        }
    });
    let saving = create_timer({
        let save = Rc::clone(&save);
        move || {
            save();
            None
        }
    });
    let scrolled = state.text.scrolled();
    let cursors = state.text.cursors();
    create_effect({
        let latest = Rc::clone(&latest);
        move || {
            scrolled.get();
            cursors.get();
            if !restored.get() {
                return;
            }
            untrack(|| {
                latest.set(Position::of(state));
                saving.start(SAVE_AFTER);
            });
        }
    });
    let visible = editor.presence_visible();
    create_effect({
        let save = Rc::clone(&save);
        move || {
            if !visible.get() {
                save();
            }
        }
    });
    on_cleanup(move || save());
}

use be_block::presence::PresenceColor;
use serde::{Deserialize, Serialize};
use text_editor_core::Position;

use crate::presence_color;
use beui::styled::{RemoteTextCursor, TextAreaState};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SharedCaret {
    pub anchor: Position,
    pub focus: Position,
}

impl SharedCaret {
    pub fn of(area: TextAreaState) -> Option<Self> {
        area.with_core(|core| {
            let cursor = core.cursor_positions().first()?;
            Some(Self {
                anchor: cursor.pos.anchor,
                focus: cursor.pos.focus,
            })
        })
    }
}

pub fn remote_carets(
    area: TextAreaState,
    carets: impl IntoIterator<Item = (SharedCaret, PresenceColor)>,
) -> Vec<RemoteTextCursor> {
    area.with_core(|core| {
        carets
            .into_iter()
            .filter_map(|(caret, color)| {
                let anchor = core.position_index(caret.anchor)?;
                let focus = core.position_index(caret.focus)?;
                Some(RemoteTextCursor {
                    selection: anchor.min(focus)..anchor.max(focus),
                    caret: focus,
                    color: presence_color(color),
                })
            })
            .collect()
    })
}

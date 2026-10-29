use be_block::presence::PresenceColor;
use serde::{Deserialize, Serialize};
use text_editor_core::Position;

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
                    color: presence_color(color).into(),
                })
            })
            .collect()
    })
}

pub fn presence_color(color: PresenceColor) -> beui::Color32 {
    match color {
        PresenceColor::Red => beui::Color32::from_rgb(224, 82, 82),
        PresenceColor::Orange => beui::Color32::from_rgb(230, 140, 50),
        PresenceColor::Yellow => beui::Color32::from_rgb(214, 179, 41),
        PresenceColor::Green => beui::Color32::from_rgb(84, 171, 90),
        PresenceColor::Teal => beui::Color32::from_rgb(46, 173, 168),
        PresenceColor::Blue => beui::Color32::from_rgb(74, 134, 227),
        PresenceColor::Purple => beui::Color32::from_rgb(150, 100, 214),
        PresenceColor::Pink => beui::Color32::from_rgb(224, 104, 168),
    }
}

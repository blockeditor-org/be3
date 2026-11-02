use crate::{EntryId, EntryKind, Handle, Point, Snapshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Over {
    pub entry: EntryId,
    pub point: Point,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub position: Point,
    pub shown: bool,
    pub window: Option<Over>,
    pub display: Option<Over>,
}

impl Cursor {
    pub fn locate(snapshot: &Snapshot, position: Point, shown: bool, hit: Option<Handle>) -> Self {
        let over = |entry: &crate::Entry| Over {
            entry: entry.id,
            point: Point::new(position.x - entry.frame.left, position.y - entry.frame.top),
        };
        let window = hit.and_then(|hit| {
            snapshot.entries.iter().find(|entry| {
                entry.kind == EntryKind::Window
                    && (entry.handle == hit || entry.popups.iter().any(|popup| popup.handle == hit))
            })
        });
        let display = snapshot.entries.iter().find(|entry| {
            entry.kind == EntryKind::Display && entry.frame.contains(position.x, position.y)
        });
        Self {
            position,
            shown,
            window: window.map(over),
            display: display.map(over),
        }
    }
}

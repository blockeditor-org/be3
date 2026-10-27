use crate::Handle;

pub const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
pub const EVENT_SYSTEM_CAPTURESTART: u32 = 0x0008;
pub const EVENT_SYSTEM_CAPTUREEND: u32 = 0x0009;
pub const EVENT_SYSTEM_MOVESIZESTART: u32 = 0x000A;
pub const EVENT_SYSTEM_MOVESIZEEND: u32 = 0x000B;
pub const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x0016;
pub const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x0017;
pub const EVENT_SYSTEM_DESKTOPSWITCH: u32 = 0x0020;
pub const EVENT_OBJECT_CREATE: u32 = 0x8000;
pub const EVENT_OBJECT_DESTROY: u32 = 0x8001;
pub const EVENT_OBJECT_SHOW: u32 = 0x8002;
pub const EVENT_OBJECT_HIDE: u32 = 0x8003;
pub const EVENT_OBJECT_REORDER: u32 = 0x8004;
pub const EVENT_OBJECT_LOCATIONCHANGE: u32 = 0x800B;
pub const EVENT_OBJECT_NAMECHANGE: u32 = 0x800C;
pub const EVENT_OBJECT_CLOAKED: u32 = 0x8017;
pub const EVENT_OBJECT_UNCLOAKED: u32 = 0x8018;
pub const OBJID_WINDOW: i32 = 0;
pub const OBJID_CURSOR: i32 = -9;
pub const CHILDID_SELF: i32 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Created(Handle),
    Destroyed(Handle),
    Shown(Handle),
    Hidden(Handle),
    NameChanged(Handle),
    Foreground(Handle),
    Minimized(Handle),
    Restored(Handle),
    Moved(Handle),
    Cloaked(Handle),
    Uncloaked(Handle),
    MoveSizeStarted(Handle),
    MoveSizeEnded(Handle),
    CaptureChanged,
    DesktopSwitched,
    DisplaysChanged,
    Tick,
}

pub fn is_any_window_event(event: u32) -> bool {
    matches!(
        event,
        EVENT_OBJECT_DESTROY
            | EVENT_SYSTEM_DESKTOPSWITCH
            | EVENT_SYSTEM_CAPTURESTART
            | EVENT_SYSTEM_CAPTUREEND
    )
}

pub fn is_cursor_event(event: u32, object: i32) -> bool {
    object == OBJID_CURSOR
        && matches!(
            event,
            EVENT_OBJECT_LOCATIONCHANGE | EVENT_OBJECT_SHOW | EVENT_OBJECT_HIDE
        )
}

impl Event {
    pub fn from_win_event(event: u32, window: Handle, object: i32, child: i32) -> Option<Self> {
        match event {
            EVENT_SYSTEM_FOREGROUND if window != Handle(0) => {
                return Some(Self::Foreground(window));
            }
            EVENT_SYSTEM_DESKTOPSWITCH => return Some(Self::DesktopSwitched),
            EVENT_SYSTEM_CAPTURESTART | EVENT_SYSTEM_CAPTUREEND => {
                return Some(Self::CaptureChanged);
            }
            _ => {}
        }
        if object != OBJID_WINDOW || child != CHILDID_SELF || window == Handle(0) {
            return None;
        }
        Some(match event {
            EVENT_SYSTEM_MINIMIZESTART => Self::Minimized(window),
            EVENT_SYSTEM_MINIMIZEEND => Self::Restored(window),
            EVENT_SYSTEM_MOVESIZESTART => Self::MoveSizeStarted(window),
            EVENT_SYSTEM_MOVESIZEEND => Self::MoveSizeEnded(window),
            EVENT_OBJECT_CREATE => Self::Created(window),
            EVENT_OBJECT_DESTROY => Self::Destroyed(window),
            EVENT_OBJECT_SHOW => Self::Shown(window),
            EVENT_OBJECT_HIDE => Self::Hidden(window),
            EVENT_OBJECT_REORDER | EVENT_OBJECT_LOCATIONCHANGE => Self::Moved(window),
            EVENT_OBJECT_NAMECHANGE => Self::NameChanged(window),
            EVENT_OBJECT_CLOAKED => Self::Cloaked(window),
            EVENT_OBJECT_UNCLOAKED => Self::Uncloaked(window),
            _ => return None,
        })
    }
}

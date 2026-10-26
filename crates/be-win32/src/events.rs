use crate::Handle;

pub const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
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
pub const CHILDID_SELF: i32 = 0;

/// What changed on the desktop. The model reads the windows again after any
/// of these; the kinds only say which of its caches are stale.
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
    DesktopSwitched,
    DisplaysChanged,
    /// A periodic check of what has no event: whether windows respond.
    Tick,
}

impl Event {
    /// The event a `SetWinEventHook` callback was given, if it is about a
    /// whole window or the session.
    pub fn from_win_event(event: u32, window: Handle, object: i32, child: i32) -> Option<Self> {
        match event {
            EVENT_SYSTEM_FOREGROUND if window != Handle(0) => {
                return Some(Self::Foreground(window));
            }
            EVENT_SYSTEM_DESKTOPSWITCH => return Some(Self::DesktopSwitched),
            _ => {}
        }
        if object != OBJID_WINDOW || child != CHILDID_SELF || window == Handle(0) {
            return None;
        }
        Some(match event {
            EVENT_SYSTEM_MINIMIZESTART => Self::Minimized(window),
            EVENT_SYSTEM_MINIMIZEEND => Self::Restored(window),
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

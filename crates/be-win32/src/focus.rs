use std::fmt;

use crate::{Entry, EntryId, EntryKind, Event, Handle, Hotkey, HotkeyError, Point, Rect, Snapshot};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Foreground {
    #[default]
    None,
    Own,
    Entry(EntryId),
    Other,
}

pub const ARRIVAL_TICKS: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    Hotkey,
    Released,
    Replaced,
    Closed,
    Minimized,
    Returned,
    NeverArrived,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusError {
    NoSuchEntry(EntryId),
    SecureDesktop,
    OffScreen,
    ForegroundRefused,
    Returned,
    Failed(String),
}

impl fmt::Display for FocusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSuchEntry(id) => write!(f, "entry {} is gone", id.0),
            Self::SecureDesktop => write!(
                f,
                "Windows is asking for permission on the secure desktop, so nothing else can take focus"
            ),
            Self::OffScreen => write!(f, "the window is not on any display"),
            Self::ForegroundRefused => {
                write!(f, "Windows refused to bring the window to the front")
            }
            Self::Returned => write!(f, "Block came back before the window did"),
            Self::Failed(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for FocusError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Clip(Rect),
    Unclip,
    MoveCursor(Point),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusEvent {
    Started(EntryId),
    Followed { from: EntryId, to: EntryId },
    Ended { entry: EntryId, reason: EndReason },
    Refused { entry: EntryId, error: FocusError },
    Suspended(EntryId),
    Resumed(EntryId),
    Hotkey,
    HostForegroundRefused,
    HotkeySet(Option<Hotkey>),
    HotkeyRefused(HotkeyError),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub commands: Vec<Command>,
    pub events: Vec<FocusEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Focused {
    pub entry: EntryId,
    pub kind: EntryKind,
    pub handle: Handle,
    pub clip: Option<Rect>,
    pub suspended: bool,
    pub arrived: bool,
    restored: bool,
    moving: bool,
    waited: u32,
    cursor: Option<Option<Point>>,
}

impl Focused {
    fn new(entry: &Entry, cursor: Option<Option<Point>>, arrived: bool) -> Self {
        Self {
            entry: entry.id,
            kind: entry.kind,
            handle: entry.handle,
            clip: None,
            suspended: false,
            arrived,
            restored: arrived,
            moving: false,
            waited: 0,
            cursor,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FocusMode {
    focused: Option<Focused>,
    hotkey: bool,
    returns: u64,
    last: Foreground,
}

impl Default for FocusMode {
    fn default() -> Self {
        Self {
            focused: None,
            hotkey: false,
            returns: 0,
            last: Foreground::Own,
        }
    }
}

impl FocusMode {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn focused(&self) -> Option<&Focused> {
        self.focused.as_ref()
    }

    pub fn returns(&self) -> u64 {
        self.returns
    }

    pub fn clips(&self) -> bool {
        self.hotkey
    }

    pub fn held_clip(&self) -> Option<Rect> {
        let focused = self.focused.as_ref()?;
        match focused.arrived && !focused.suspended && !focused.moving {
            true => focused.clip,
            false => None,
        }
    }

    pub fn reclip(&self, current: Rect) -> Option<Command> {
        self.held_clip()
            .filter(|held| *held != current)
            .map(Command::Clip)
    }

    pub fn set_hotkey(&mut self, registered: bool) -> Output {
        let mut out = Output::default();
        self.hotkey = registered;
        if !registered
            && let Some(focused) = self.focused.as_mut()
            && focused.clip.take().is_some()
        {
            out.commands.push(Command::Unclip);
        }
        out
    }

    pub fn check(snapshot: &Snapshot, id: EntryId) -> Result<&Entry, FocusError> {
        let entry = snapshot.entry(id).ok_or(FocusError::NoSuchEntry(id))?;
        if snapshot.secure_desktop() {
            return Err(FocusError::SecureDesktop);
        }
        if !entry.minimized && clip(snapshot, entry).is_none() {
            return Err(FocusError::OffScreen);
        }
        Ok(entry)
    }

    pub fn start(
        &mut self,
        snapshot: &Snapshot,
        id: EntryId,
        at: Option<Point>,
        since: u64,
    ) -> Result<Output, FocusError> {
        if since != self.returns {
            return Err(FocusError::Returned);
        }
        let entry = Self::check(snapshot, id)?;
        let mut out = Output::default();
        if let Some(old) = self.focused.take() {
            if old.clip.is_some() {
                out.commands.push(Command::Unclip);
            }
            out.events.push(FocusEvent::Ended {
                entry: old.entry,
                reason: EndReason::Replaced,
            });
        }
        self.focused = Some(Focused::new(entry, Some(at), false));
        out.events.push(FocusEvent::Started(id));
        Ok(out)
    }

    pub fn release(&mut self, reason: EndReason) -> Output {
        let mut out = Output::default();
        self.end(&mut out, reason);
        out
    }

    pub fn observe(&mut self, foreground: Foreground) -> Output {
        let mut out = Output::default();
        match foreground {
            Foreground::None => {}
            Foreground::Own => {
                if self.last != Foreground::Own {
                    self.returns += 1;
                    self.end(&mut out, EndReason::Returned);
                }
                self.last = Foreground::Own;
            }
            other => self.last = other,
        }
        out
    }

    pub fn step(
        &mut self,
        snapshot: &Snapshot,
        foreground: Foreground,
        events: &[Event],
    ) -> Output {
        let mut out = self.observe(foreground);
        let Some(focused) = self.focused.as_mut() else {
            return out;
        };
        if snapshot.secure_desktop() {
            if !focused.suspended {
                focused.suspended = true;
                focused.clip = None;
                out.events.push(FocusEvent::Suspended(focused.entry));
            }
            return out;
        }
        let mut reapply = false;
        if focused.suspended {
            focused.suspended = false;
            reapply = true;
            out.events.push(FocusEvent::Resumed(focused.entry));
        }
        let root = focused.entry;
        let ours = |id: EntryId| related(snapshot, id, root);
        match (focused.kind, foreground) {
            (EntryKind::Window, Foreground::Entry(id)) if ours(id) => focused.arrived = true,
            (EntryKind::Display, Foreground::Entry(_) | Foreground::Other) => {
                focused.arrived = true;
            }
            _ => {}
        }
        if focused.arrived && foreground == Foreground::Own {
            self.end(&mut out, EndReason::Returned);
            return out;
        }
        if !focused.arrived && events.contains(&Event::Tick) {
            focused.waited += 1;
            if focused.waited >= ARRIVAL_TICKS {
                self.end(&mut out, EndReason::NeverArrived);
                return out;
            }
        }
        for event in events {
            match *event {
                Event::MoveSizeStarted(handle) if handle == focused.handle => {
                    focused.moving = true;
                }
                Event::MoveSizeEnded(handle) if handle == focused.handle => {
                    focused.moving = false;
                }
                _ => {}
            }
        }
        let entry = snapshot.entry(focused.entry);
        if focused.kind == EntryKind::Window {
            let follow = match foreground {
                Foreground::Entry(id) if focused.arrived && !ours(id) => snapshot
                    .entry(id)
                    .filter(|entry| entry.kind == EntryKind::Window && !entry.minimized),
                _ => None,
            };
            let lost = match entry {
                None => Some(EndReason::Closed),
                Some(entry) if entry.minimized && focused.restored => Some(EndReason::Minimized),
                Some(_) => None,
            };
            match (follow, lost) {
                (Some(to), _) => {
                    out.events.push(FocusEvent::Followed {
                        from: focused.entry,
                        to: to.id,
                    });
                    *focused = Focused::new(to, None, true);
                }
                (None, Some(reason)) => {
                    self.end(&mut out, reason);
                    return out;
                }
                (None, None) => {}
            }
        } else if entry.is_none() {
            self.end(&mut out, EndReason::Closed);
            return out;
        }

        let hotkey = self.hotkey;
        let Some(focused) = self.focused.as_mut() else {
            return out;
        };
        let Some(entry) = snapshot.entry(focused.entry) else {
            return out;
        };
        reapply |= events.iter().any(|event| match *event {
            Event::Foreground(_)
            | Event::DesktopSwitched
            | Event::DisplaysChanged
            | Event::CaptureChanged => true,
            Event::Moved(handle) | Event::Restored(handle) | Event::MoveSizeEnded(handle) => {
                handle == focused.handle
            }
            _ => false,
        });
        if entry.minimized {
            return out;
        }
        focused.restored = true;
        if !focused.arrived {
            return out;
        }
        let Some(rect) = clip(snapshot, entry) else {
            return out;
        };
        if hotkey && !focused.moving && (reapply || focused.clip != Some(rect)) {
            out.commands.push(Command::Clip(rect));
            focused.clip = Some(rect);
        }
        if let Some(at) = focused.cursor.take() {
            let point = at.map_or_else(
                || entry.frame.centre(),
                |at| Point::new(entry.frame.left + at.x, entry.frame.top + at.y),
            );
            out.commands.push(Command::MoveCursor(rect.clamp(point)));
        }
        out
    }

    fn end(&mut self, out: &mut Output, reason: EndReason) {
        if let Some(focused) = self.focused.take() {
            out.commands.push(Command::Unclip);
            out.events.push(FocusEvent::Ended {
                entry: focused.entry,
                reason,
            });
        }
    }
}

pub fn related(snapshot: &Snapshot, id: EntryId, root: EntryId) -> bool {
    let mut next = Some(id);
    for _ in 0..=snapshot.entries.len() {
        match next {
            Some(id) if id == root => return true,
            Some(id) => next = snapshot.entry(id).and_then(|entry| entry.parent),
            None => return false,
        }
    }
    false
}

pub fn clip(snapshot: &Snapshot, entry: &Entry) -> Option<Rect> {
    if entry.kind == EntryKind::Display {
        return (!entry.frame.is_empty()).then_some(entry.frame);
    }
    let with_popups = |entry: &Entry| {
        entry
            .popups
            .iter()
            .fold(entry.frame, |rect, popup| rect.union(&popup.frame))
    };
    let rect = snapshot
        .entries
        .iter()
        .filter(|other| {
            other.id != entry.id
                && other.kind == EntryKind::Window
                && !other.minimized
                && related(snapshot, other.id, entry.id)
        })
        .fold(with_popups(entry), |rect, other| {
            rect.union(&with_popups(other))
        });
    let screen = snapshot
        .monitors
        .iter()
        .fold(Rect::default(), |screen, monitor| {
            screen.union(&monitor.rect)
        });
    if screen.is_empty() {
        return (!rect.is_empty()).then_some(rect);
    }
    rect.intersection(&screen)
}

pub fn display_window<'a>(
    snapshot: &'a Snapshot,
    display: &Entry,
    z_order: &[Handle],
) -> Option<&'a Entry> {
    z_order.iter().find_map(|handle| {
        snapshot.entries.iter().find(|entry| {
            entry.kind == EntryKind::Window && entry.handle == *handle && !entry.minimized && {
                let centre = entry.frame.centre();
                display.frame.contains(centre.x, centre.y)
            }
        })
    })
}

use std::fmt;

use crate::{Entry, EntryId, EntryKind, Event, Handle, Point, Rect, Snapshot};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Foreground {
    #[default]
    None,
    Own,
    Entry(EntryId),
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    Hotkey,
    Released,
    Replaced,
    Closed,
    Minimized,
    Returned,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusError {
    NoSuchEntry(EntryId),
    SecureDesktop,
    OffScreen,
    ForegroundRefused,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusEvent {
    Started(EntryId),
    Followed { from: EntryId, to: EntryId },
    Ended { entry: EntryId, reason: EndReason },
    Suspended(EntryId),
    Resumed(EntryId),
    Hotkey,
    HostForegroundRefused,
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
    arrived: bool,
    restored: bool,
    cursor: Option<Option<Point>>,
}

#[derive(Clone, Debug, Default)]
pub struct FocusMode {
    focused: Option<Focused>,
}

impl FocusMode {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn focused(&self) -> Option<&Focused> {
        self.focused.as_ref()
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
    ) -> Result<Output, FocusError> {
        let entry = Self::check(snapshot, id)?;
        let mut out = Output::default();
        if let Some(old) = self.focused.take() {
            out.events.push(FocusEvent::Ended {
                entry: old.entry,
                reason: EndReason::Replaced,
            });
        }
        self.focused = Some(Focused {
            entry: id,
            kind: entry.kind,
            handle: entry.handle,
            clip: None,
            suspended: false,
            arrived: false,
            restored: false,
            cursor: Some(at),
        });
        out.events.push(FocusEvent::Started(id));
        Ok(out)
    }

    pub fn release(&mut self, reason: EndReason) -> Output {
        let mut out = Output::default();
        self.end(&mut out, reason);
        out
    }

    pub fn step(
        &mut self,
        snapshot: &Snapshot,
        foreground: Foreground,
        events: &[Event],
    ) -> Output {
        let mut out = Output::default();
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
                    *focused = Focused {
                        entry: to.id,
                        kind: to.kind,
                        handle: to.handle,
                        clip: None,
                        suspended: false,
                        arrived: true,
                        restored: true,
                        cursor: None,
                    };
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

        let Some(focused) = self.focused.as_mut() else {
            return out;
        };
        let Some(entry) = snapshot.entry(focused.entry) else {
            return out;
        };
        reapply |= events.iter().any(|event| match *event {
            Event::Foreground(_) | Event::DesktopSwitched | Event::DisplaysChanged => true,
            Event::Moved(handle) | Event::Restored(handle) => handle == focused.handle,
            _ => false,
        });
        if entry.minimized {
            return out;
        }
        focused.restored = true;
        let Some(rect) = clip(snapshot, entry) else {
            return out;
        };
        if reapply || focused.clip != Some(rect) {
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

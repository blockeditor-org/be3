//! The X11 CLIPBOARD selection, owned from a thread of its own so a copy
//! offers every representation it has at once: arboard offers one. Under
//! Wayland, XWayland carries the selection to Wayland programs.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use x11rb::connection::Connection as _;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, CreateWindowAux, EventMask,
    PropMode, Property, SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, SelectionRequestEvent,
    Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, CURRENT_TIME, NONE};

#[cfg(test)]
mod tests;

/// Answers larger than this go in pieces of it (the INCR protocol), as
/// toolkits do, rather than in one request the size of the data.
const CHUNK: usize = 256 * 1024;

/// A transfer in pieces the requestor stopped taking for this long is dropped.
const IDLE: Duration = Duration::from_secs(5);

/// How long closing waits for a clipboard manager to take the clip.
const SAVE_WAIT: Duration = Duration::from_secs(1);

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        CLIPBOARD,
        CLIPBOARD_MANAGER,
        SAVE_TARGETS,
        BEUI_SAVED,
        TARGETS,
        INCR,
        UTF8_STRING,
        TEXT,
        TEXT_PLAIN_UTF8: b"text/plain;charset=utf-8",
        TEXT_PLAIN: b"text/plain",
        PNG: b"image/png",
    }
}

/// What is offered, behind the lock that also orders taking the selection
/// against checking, after a SelectionClear, who has it.
type Offered = Arc<Mutex<Vec<(Atom, Arc<[u8]>)>>>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// This program's ownership of the CLIPBOARD selection, and what it offers.
pub struct Owner {
    connection: Arc<RustConnection>,
    window: Window,
    atoms: Atoms,
    offered: Offered,
    saved: Receiver<()>,
}

impl Owner {
    /// Connects to `display`, or to `$DISPLAY` when `None`; `None` when
    /// there is no X server.
    pub fn connect(display: Option<&str>) -> Option<Self> {
        let (connection, screen) = x11rb::connect(display).ok()?;
        let root = connection.setup().roots.get(screen)?.root;
        let window = connection.generate_id().ok()?;
        connection
            .create_window(
                COPY_DEPTH_FROM_PARENT,
                window,
                root,
                0,
                0,
                1,
                1,
                0,
                WindowClass::INPUT_ONLY,
                COPY_FROM_PARENT,
                &CreateWindowAux::new().event_mask(EventMask::STRUCTURE_NOTIFY),
            )
            .ok()?;
        let atoms = Atoms::new(&connection).ok()?.reply().ok()?;
        connection.flush().ok()?;
        let connection = Arc::new(connection);
        let offered = Offered::default();
        let (save, saved) = channel();
        let server = Server {
            connection: Arc::clone(&connection),
            window,
            atoms,
            offered: Arc::clone(&offered),
            transfers: Transfers::default(),
            save,
        };
        std::thread::Builder::new()
            .name("beui-x11-clipboard".to_owned())
            .spawn(move || server.run())
            .ok()?;
        Some(Self {
            connection,
            window,
            atoms,
            offered,
            saved,
        })
    }

    /// Takes the selection, offering `text` under the text targets and `png`
    /// as `image/png`, whichever there are; with neither, gives it up.
    pub fn offer(&self, text: Option<&str>, png: Option<&[u8]>) {
        let atoms = &self.atoms;
        let mut offer = Vec::new();
        if let Some(text) = text {
            let text: Arc<[u8]> = text.as_bytes().into();
            for target in [
                atoms.UTF8_STRING,
                atoms.TEXT_PLAIN_UTF8,
                atoms.TEXT_PLAIN,
                atoms.TEXT,
            ] {
                offer.push((target, Arc::clone(&text)));
            }
        }
        if let Some(png) = png {
            offer.push((atoms.PNG, png.into()));
        }
        let owner = if offer.is_empty() { NONE } else { self.window };
        let mut offered = lock(&self.offered);
        *offered = offer;
        let _ = self
            .connection
            .set_selection_owner(owner, atoms.CLIPBOARD, CURRENT_TIME);
        // A round trip while still holding the lock: the server has applied
        // the ownership before a paste, or a SelectionClear's check, asks.
        let _ = self
            .connection
            .get_input_focus()
            .map(|cookie| cookie.reply());
    }

    /// Asks a clipboard manager, if one runs, to copy what this program
    /// offers so it outlives the program (ICCCM's SAVE_TARGETS); waits
    /// `SAVE_WAIT` at most for it to say it has.
    fn hand_over(&self) {
        if lock(&self.offered).is_empty() {
            return;
        }
        let owner_of = |selection| {
            self.connection
                .get_selection_owner(selection)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .map(|reply| reply.owner)
        };
        if owner_of(self.atoms.CLIPBOARD) != Some(self.window)
            || owner_of(self.atoms.CLIPBOARD_MANAGER).is_none_or(|owner| owner == NONE)
        {
            return;
        }
        let asked = self.connection.convert_selection(
            self.window,
            self.atoms.CLIPBOARD_MANAGER,
            self.atoms.SAVE_TARGETS,
            self.atoms.BEUI_SAVED,
            CURRENT_TIME,
        );
        if asked.is_ok() && self.connection.flush().is_ok() {
            let _ = self.saved.recv_timeout(SAVE_WAIT);
        }
    }
}

impl Drop for Owner {
    /// Hands the clip to a clipboard manager, then destroys the window,
    /// which ends the thread serving it.
    fn drop(&mut self) {
        self.hand_over();
        let _ = self.connection.destroy_window(self.window);
        let _ = self.connection.flush();
    }
}

/// An answer going out in pieces.
#[derive(Debug)]
struct Sending {
    requestor: Window,
    property: Atom,
    kind: Atom,
    data: Arc<[u8]>,
    sent: usize,
    touched: Instant,
}

/// The next piece of a transfer to write, and whether it is the empty one
/// that ends it.
#[derive(Debug, PartialEq)]
struct Piece {
    kind: Atom,
    data: Arc<[u8]>,
    range: std::ops::Range<usize>,
    last: bool,
}

/// The transfers in pieces under way, apart from the X connection.
#[derive(Default, Debug)]
struct Transfers {
    sending: Vec<Sending>,
}

impl Transfers {
    fn start(
        &mut self,
        requestor: Window,
        property: Atom,
        kind: Atom,
        data: Arc<[u8]>,
        now: Instant,
    ) {
        self.sending
            .retain(|sending| (sending.requestor, sending.property) != (requestor, property));
        self.sending.push(Sending {
            requestor,
            property,
            kind,
            data,
            sent: 0,
            touched: now,
        });
    }

    /// The requestor deleted `property`, taking the last piece: the next,
    /// which once all is sent is an empty one that ends the transfer.
    fn next(&mut self, requestor: Window, property: Atom, now: Instant) -> Option<Piece> {
        let index = self
            .sending
            .iter()
            .position(|sending| (sending.requestor, sending.property) == (requestor, property))?;
        let sending = &mut self.sending[index];
        let end = (sending.sent + CHUNK).min(sending.data.len());
        let piece = Piece {
            kind: sending.kind,
            data: Arc::clone(&sending.data),
            range: sending.sent..end,
            last: sending.sent == end,
        };
        sending.sent = end;
        sending.touched = now;
        if piece.last {
            self.sending.remove(index);
        }
        Some(piece)
    }

    /// Drops every transfer to a requestor that went away.
    fn forget(&mut self, requestor: Window) {
        self.sending
            .retain(|sending| sending.requestor != requestor);
    }

    /// Drops transfers idle since before `IDLE` ago, answering the
    /// requestors no transfer names any more.
    fn expire(&mut self, now: Instant) -> Vec<Window> {
        let (idle, kept) = std::mem::take(&mut self.sending)
            .into_iter()
            .partition::<Vec<_>, _>(|sending| now.duration_since(sending.touched) >= IDLE);
        self.sending = kept;
        let mut left = idle
            .into_iter()
            .map(|sending| sending.requestor)
            .filter(|requestor| !self.names(*requestor))
            .collect::<Vec<_>>();
        left.dedup();
        left
    }

    fn names(&self, requestor: Window) -> bool {
        self.sending
            .iter()
            .any(|sending| sending.requestor == requestor)
    }
}

struct Server {
    connection: Arc<RustConnection>,
    window: Window,
    atoms: Atoms,
    offered: Offered,
    transfers: Transfers,
    save: Sender<()>,
}

impl Server {
    fn run(mut self) {
        while let Ok(event) = self.connection.wait_for_event() {
            let now = Instant::now();
            match event {
                Event::SelectionRequest(request) => self.answer(request, now),
                Event::SelectionClear(clear) if clear.selection == self.atoms.CLIPBOARD => {
                    self.cleared()
                }
                Event::SelectionNotify(notify) if notify.target == self.atoms.SAVE_TARGETS => {
                    let _ = self.save.send(());
                }
                Event::PropertyNotify(notify) if notify.state == Property::DELETE => {
                    self.send_next(notify.window, notify.atom, now)
                }
                Event::DestroyNotify(destroyed) if destroyed.window == self.window => return,
                Event::DestroyNotify(destroyed) => self.transfers.forget(destroyed.window),
                _ => {}
            }
            for requestor in self.transfers.expire(now) {
                self.stop_watching(requestor);
            }
            let _ = self.connection.flush();
        }
    }

    /// Another program copied, or this one took the selection again: what
    /// was offered goes unless this program still owns it. The lock orders
    /// the check after any `offer` under way, so a stale SelectionClear
    /// cannot clear a fresh offer.
    fn cleared(&self) {
        let mut offered = lock(&self.offered);
        let owner = self
            .connection
            .get_selection_owner(self.atoms.CLIPBOARD)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map(|reply| reply.owner);
        if owner != Some(self.window) {
            offered.clear();
        }
    }

    fn answer(&mut self, request: SelectionRequestEvent, now: Instant) {
        // A client from before ICCCM 2 names no property: the target is it.
        let property = match request.property {
            NONE => request.target,
            property => property,
        };
        let answered = request.selection == self.atoms.CLIPBOARD
            && self.write(request.requestor, property, request.target, now);
        let _ = self.connection.send_event(
            false,
            request.requestor,
            EventMask::NO_EVENT,
            SelectionNotifyEvent {
                response_type: SELECTION_NOTIFY_EVENT,
                sequence: 0,
                time: request.time,
                requestor: request.requestor,
                selection: request.selection,
                target: request.target,
                property: if answered { property } else { NONE },
            },
        );
    }

    /// Puts `target` on the requestor's `property`, or starts sending it in
    /// pieces; false when it is not offered.
    fn write(&mut self, requestor: Window, property: Atom, target: Atom, now: Instant) -> bool {
        let offered = lock(&self.offered).clone();
        if offered.is_empty() {
            return false;
        }
        if target == self.atoms.TARGETS {
            let targets = std::iter::once(self.atoms.TARGETS)
                .chain(offered.iter().map(|(target, _)| *target))
                .collect::<Vec<_>>();
            return self
                .connection
                .change_property32(
                    PropMode::REPLACE,
                    requestor,
                    property,
                    AtomEnum::ATOM,
                    &targets,
                )
                .is_ok();
        }
        let Some((_, data)) = offered.into_iter().find(|(held, _)| *held == target) else {
            return false;
        };
        // TEXT asks the owner to choose an encoding; this one's is UTF-8.
        let kind = match target == self.atoms.TEXT {
            true => self.atoms.UTF8_STRING,
            false => target,
        };
        if data.len() <= CHUNK {
            return self
                .connection
                .change_property8(PropMode::REPLACE, requestor, property, kind, &data)
                .is_ok();
        }
        let _ = self.connection.change_window_attributes(
            requestor,
            &ChangeWindowAttributesAux::new()
                .event_mask(EventMask::PROPERTY_CHANGE | EventMask::STRUCTURE_NOTIFY),
        );
        let size = u32::try_from(data.len()).unwrap_or(u32::MAX);
        if self
            .connection
            .change_property32(
                PropMode::REPLACE,
                requestor,
                property,
                self.atoms.INCR,
                &[size],
            )
            .is_err()
        {
            return false;
        }
        self.transfers.start(requestor, property, kind, data, now);
        true
    }

    fn send_next(&mut self, requestor: Window, property: Atom, now: Instant) {
        let Some(piece) = self.transfers.next(requestor, property, now) else {
            return;
        };
        let _ = self.connection.change_property8(
            PropMode::REPLACE,
            requestor,
            property,
            piece.kind,
            &piece.data[piece.range],
        );
        if piece.last && !self.transfers.names(requestor) {
            self.stop_watching(requestor);
        }
    }

    /// Stops hearing a requestor's events once no transfer needs them.
    fn stop_watching(&self, requestor: Window) {
        let _ = self.connection.change_window_attributes(
            requestor,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::NO_EVENT),
        );
    }
}

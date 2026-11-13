//! The X11 CLIPBOARD selection, owned from a thread of its own so a copy
//! offers every representation it has at once: arboard offers one. Under
//! Wayland, XWayland carries the selection to Wayland programs.

use std::sync::{Arc, Mutex, PoisonError};

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

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        CLIPBOARD,
        TARGETS,
        INCR,
        UTF8_STRING,
        TEXT,
        TEXT_PLAIN_UTF8: b"text/plain;charset=utf-8",
        TEXT_PLAIN: b"text/plain",
        PNG: b"image/png",
    }
}

type Offered = Arc<Mutex<Vec<(Atom, Arc<[u8]>)>>>;

/// This program's ownership of the CLIPBOARD selection, and what it offers.
pub struct Owner {
    connection: Arc<RustConnection>,
    window: Window,
    atoms: Atoms,
    offered: Offered,
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
        let server = Server {
            connection: Arc::clone(&connection),
            window,
            atoms,
            offered: Arc::clone(&offered),
            sending: Vec::new(),
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
        })
    }

    /// Takes the selection, offering `text` under the text targets and `png`
    /// as `image/png`, whichever there are; with neither, gives it up.
    pub fn offer(&self, text: Option<&str>, png: Option<&[u8]>) {
        let atoms = &self.atoms;
        let mut offered = Vec::new();
        if let Some(text) = text {
            let text: Arc<[u8]> = text.as_bytes().into();
            for target in [
                atoms.UTF8_STRING,
                atoms.TEXT_PLAIN_UTF8,
                atoms.TEXT_PLAIN,
                atoms.TEXT,
            ] {
                offered.push((target, Arc::clone(&text)));
            }
        }
        if let Some(png) = png {
            offered.push((atoms.PNG, png.into()));
        }
        let owner = if offered.is_empty() {
            NONE
        } else {
            self.window
        };
        *self.offered.lock().unwrap_or_else(PoisonError::into_inner) = offered;
        let _ = self
            .connection
            .set_selection_owner(owner, atoms.CLIPBOARD, CURRENT_TIME);
        // A round trip, so the server has applied the ownership before a
        // paste that follows the copy asks for it.
        let _ = self
            .connection
            .get_input_focus()
            .map(|cookie| cookie.reply());
    }
}

impl Drop for Owner {
    /// Destroying the window ends the thread serving it.
    fn drop(&mut self) {
        let _ = self.connection.destroy_window(self.window);
        let _ = self.connection.flush();
    }
}

/// An answer going out in pieces.
struct Sending {
    requestor: Window,
    property: Atom,
    target: Atom,
    data: Arc<[u8]>,
    sent: usize,
}

struct Server {
    connection: Arc<RustConnection>,
    window: Window,
    atoms: Atoms,
    offered: Offered,
    sending: Vec<Sending>,
}

impl Server {
    fn run(mut self) {
        while let Ok(event) = self.connection.wait_for_event() {
            match event {
                Event::SelectionRequest(request) => self.answer(request),
                Event::SelectionClear(clear) if clear.selection == self.atoms.CLIPBOARD => {
                    self.cleared()
                }
                Event::PropertyNotify(notify) if notify.state == Property::DELETE => {
                    self.send_next(notify.window, notify.atom)
                }
                Event::DestroyNotify(destroyed) if destroyed.window == self.window => return,
                _ => {}
            }
            let _ = self.connection.flush();
        }
    }

    /// Another program copied: what was offered goes, unless this program
    /// took the selection again meanwhile.
    fn cleared(&self) {
        let owner = self
            .connection
            .get_selection_owner(self.atoms.CLIPBOARD)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map(|reply| reply.owner);
        if owner != Some(self.window) {
            self.offered
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clear();
        }
    }

    fn answer(&mut self, request: SelectionRequestEvent) {
        // A client from before ICCCM 2 names no property: the target is it.
        let property = match request.property {
            NONE => request.target,
            property => property,
        };
        let answered = request.selection == self.atoms.CLIPBOARD
            && self.write(request.requestor, property, request.target);
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
    fn write(&mut self, requestor: Window, property: Atom, target: Atom) -> bool {
        let offered = self
            .offered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
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
        if data.len() <= CHUNK {
            return self
                .connection
                .change_property8(PropMode::REPLACE, requestor, property, target, &data)
                .is_ok();
        }
        let _ = self.connection.change_window_attributes(
            requestor,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
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
        self.sending
            .retain(|sending| (sending.requestor, sending.property) != (requestor, property));
        self.sending.push(Sending {
            requestor,
            property,
            target,
            data,
            sent: 0,
        });
        true
    }

    /// The requestor took the last piece: the next goes, and an empty one
    /// once all is sent.
    fn send_next(&mut self, window: Window, property: Atom) {
        let Some(index) = self
            .sending
            .iter()
            .position(|sending| (sending.requestor, sending.property) == (window, property))
        else {
            return;
        };
        let sending = &mut self.sending[index];
        let end = (sending.sent + CHUNK).min(sending.data.len());
        let _ = self.connection.change_property8(
            PropMode::REPLACE,
            window,
            property,
            sending.target,
            &sending.data[sending.sent..end],
        );
        if sending.sent == end {
            self.sending.remove(index);
            let _ = self.connection.change_window_attributes(
                window,
                &ChangeWindowAttributesAux::new().event_mask(EventMask::NO_EVENT),
            );
        } else {
            sending.sent = end;
        }
    }
}

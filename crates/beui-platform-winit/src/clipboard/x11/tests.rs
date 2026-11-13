use super::*;
use std::io::BufRead as _;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use beui_types::ClipItem;

mod a_be3_format_crosses_to_another_program_with_its_refs;
mod a_transfer_in_pieces_ends_with_an_empty_one;
mod closing_hands_the_clip_to_a_clipboard_manager;
mod idle_and_abandoned_transfers_are_dropped;
mod one_copy_offers_its_text_and_its_image;

/// Fails the whole test binary, stopping its Xvfb first, if a test that talks
/// to an X server has not finished within a minute: a broken owner otherwise
/// hangs the read.
struct Watchdog {
    _alive: std::sync::mpsc::Sender<()>,
}

impl Watchdog {
    fn start(server: &Xvfb) -> Self {
        let (alive, watched) = std::sync::mpsc::channel::<()>();
        let child = Arc::clone(&server.child);
        std::thread::spawn(move || {
            if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) =
                watched.recv_timeout(Duration::from_secs(60))
            {
                eprintln!("an X clipboard test did not finish within a minute");
                let _ = lock(&child).kill();
                std::process::abort();
            }
        });
        Self { _alive: alive }
    }
}

/// An Xvfb server of the test's own, stopped when dropped; `None`, with a
/// message, where Xvfb is not installed.
struct Xvfb {
    child: Arc<Mutex<Child>>,
    display: String,
}

impl Xvfb {
    fn start() -> Option<Self> {
        let spawned = Command::new("Xvfb")
            .args([
                "-displayfd",
                "1",
                "-nolisten",
                "tcp",
                "-screen",
                "0",
                "64x64x24",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) => {
                eprintln!(
                    "skipped: Xvfb could not start ({error}); install it with apt install xvfb"
                );
                return None;
            }
        };
        // Xvfb writes its display number once it accepts connections.
        let mut line = String::new();
        let read = std::io::BufReader::new(child.stdout.take()?).read_line(&mut line);
        if read.is_err() || line.trim().is_empty() {
            let _ = child.kill();
            eprintln!("skipped: Xvfb started but named no display");
            return None;
        }
        Some(Self {
            display: format!(":{}", line.trim()),
            child: Arc::new(Mutex::new(child)),
        })
    }
}

impl Drop for Xvfb {
    fn drop(&mut self) {
        let mut child = lock(&self.child);
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Another X client, which reads the clipboard as a program pasting would.
struct Reader {
    connection: RustConnection,
    window: Window,
    property: Atom,
    incr: Atom,
}

impl Reader {
    fn connect(display: &str) -> Self {
        let (connection, screen) = x11rb::connect(Some(display)).expect("the reader connects");
        let root = connection.setup().roots[screen].root;
        let window = connection.generate_id().expect("an id");
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
                &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
            )
            .expect("the reader's window");
        let property = Self::atom_of(&connection, "BEUI_TEST_PASTE");
        let incr = Self::atom_of(&connection, "INCR");
        Self {
            connection,
            window,
            property,
            incr,
        }
    }

    fn atom_of(connection: &RustConnection, name: &str) -> Atom {
        connection
            .intern_atom(false, name.as_bytes())
            .expect("intern")
            .reply()
            .expect("an atom")
            .atom
    }

    fn atom(&self, name: &str) -> Atom {
        Self::atom_of(&self.connection, name)
    }

    /// The clipboard as `target`, or `None` when the owner refuses it.
    fn read(&self, target: &str) -> Option<Vec<u8>> {
        self.read_typed(target).map(|(_, data)| data)
    }

    /// The clipboard as `target`, with the type the owner gave it.
    fn read_typed(&self, target: &str) -> Option<(Atom, Vec<u8>)> {
        let clipboard = self.atom("CLIPBOARD");
        self.connection
            .convert_selection(
                self.window,
                clipboard,
                self.atom(target),
                self.property,
                CURRENT_TIME,
            )
            .expect("convert");
        self.connection.flush().expect("flush");
        let notified = loop {
            if let Event::SelectionNotify(notify) =
                self.connection.wait_for_event().expect("an event")
            {
                break notify;
            }
        };
        if notified.property == NONE {
            return None;
        }
        let first = self.take();
        if first.type_ != self.incr {
            return Some((first.type_, first.value));
        }
        let mut kind = NONE;
        let mut data = Vec::new();
        loop {
            match self.connection.wait_for_event().expect("an event") {
                Event::PropertyNotify(notify)
                    if notify.atom == self.property && notify.state == Property::NEW_VALUE =>
                {
                    let piece = self.take();
                    if piece.value.is_empty() {
                        return Some((kind, data));
                    }
                    kind = piece.type_;
                    data.extend(piece.value);
                }
                _ => {}
            }
        }
    }

    fn take(&self) -> x11rb::protocol::xproto::GetPropertyReply {
        let reply = self
            .connection
            .get_property(true, self.window, self.property, AtomEnum::ANY, 0, u32::MAX)
            .expect("get property")
            .reply()
            .expect("the property");
        self.connection.flush().expect("flush");
        reply
    }

    fn targets(&self) -> Vec<String> {
        let data = self.read("TARGETS").expect("the owner lists its targets");
        data.chunks_exact(4)
            .map(|atom| u32::from_ne_bytes(atom.try_into().expect("four bytes")))
            .map(|atom| {
                let name = self
                    .connection
                    .get_atom_name(atom)
                    .expect("name")
                    .reply()
                    .expect("a name")
                    .name;
                String::from_utf8(name).expect("an atom's name is text")
            })
            .collect()
    }
}

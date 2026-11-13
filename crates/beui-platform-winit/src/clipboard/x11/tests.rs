use super::*;
use std::io::BufRead as _;
use std::process::{Child, Command, Stdio};

mod one_copy_offers_its_text_and_its_image;

/// An Xvfb server of the test's own, stopped when dropped; `None`, with a
/// message, where Xvfb is not installed.
struct Xvfb {
    child: Child,
    display: String,
}

impl Xvfb {
    fn start() -> Option<Self> {
        let spawned = Command::new("Xvfb")
            .args(["-displayfd", "1", "-nolisten", "tcp", "-screen", "0", "64x64x24"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) => {
                eprintln!("skipped: Xvfb could not start ({error}); install it with apt install xvfb");
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
            child,
        })
    }
}

impl Drop for Xvfb {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
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
            return Some(first.value);
        }
        let mut data = Vec::new();
        loop {
            match self.connection.wait_for_event().expect("an event") {
                Event::PropertyNotify(notify)
                    if notify.atom == self.property && notify.state == Property::NEW_VALUE =>
                {
                    let piece = self.take().value;
                    if piece.is_empty() {
                        return Some(data);
                    }
                    data.extend(piece);
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

use std::{io::Read, thread, time::Duration};

const TIMEOUT: Duration = Duration::from_secs(30);

/// The most bytes one body chunk carries.
const CHUNK_BYTES: usize = 64 * 1024;

/// How far `send`'s reading may run ahead, and whether it was aborted. Clones share it.
#[derive(Clone)]
pub struct Flow(std::sync::Arc<(std::sync::Mutex<Room>, std::sync::Condvar)>);

struct Room {
    chunks: usize,
    aborted: bool,
}

impl Default for Flow {
    fn default() -> Self {
        let room = Room {
            chunks: super::CHUNKS_AHEAD,
            aborted: false,
        };
        Self(std::sync::Arc::new((
            std::sync::Mutex::new(room),
            std::sync::Condvar::new(),
        )))
    }
}

impl Flow {
    fn room(&self) -> std::sync::MutexGuard<'_, Room> {
        self.0
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Stops the request: nothing more is delivered, and its reading ends.
    pub fn abort(&self) {
        self.room().aborted = true;
        self.0.1.notify_all();
    }

    pub fn aborted(&self) -> bool {
        self.room().aborted
    }

    /// The caller passed one delivered chunk on, so one more may be read.
    pub fn release(&self) {
        self.room().chunks += 1;
        self.0.1.notify_all();
    }

    /// Waits until another chunk may be read and takes its room; false once aborted.
    fn take(&self) -> bool {
        let mut room = self.room();
        while room.chunks == 0 && !room.aborted {
            room = self
                .0
                .1
                .wait(room)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        if room.aborted {
            return false;
        }
        room.chunks -= 1;
        true
    }
}

/// Sends `request` on a thread of its own and delivers its response as it arrives (`Event`),
/// a chunk of the body at a time and no more than `CHUNKS_AHEAD` chunks ahead of
/// `flow.release`, so the body has no limit.
pub fn send(
    request: super::Request,
    flow: Flow,
    mut deliver: impl FnMut(super::Event) + Send + 'static,
) {
    use super::Event;
    thread::spawn(move || {
        let mut deliver = |event| {
            if !flow.aborted() {
                deliver(event);
            }
        };
        let response = match exchange(&request) {
            Ok(response) => response,
            Err(error) => return deliver(Event::Failed(error)),
        };
        let headers = response
            .headers_names()
            .into_iter()
            .flat_map(|name| {
                let values: Vec<String> = response
                    .all(&name)
                    .into_iter()
                    .map(str::to_owned)
                    .collect();
                values.into_iter().map(move |value| (name.clone(), value))
            })
            .collect();
        deliver(Event::Head(super::Head {
            status: response.status(),
            headers,
        }));
        let mut body = response.into_reader();
        let mut buffer = vec![0; CHUNK_BYTES];
        loop {
            if !flow.take() {
                return;
            }
            let read = loop {
                match body.read(&mut buffer) {
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    read => break read,
                }
            };
            match read {
                Ok(0) => return deliver(Event::Done),
                Ok(length) => deliver(Event::Chunk(buffer[..length].to_vec())),
                Err(error) => return deliver(Event::Failed(error.to_string())),
            }
        }
    });
}

fn exchange(request: &super::Request) -> Result<ureq::Response, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(TIMEOUT)
        .timeout_read(TIMEOUT)
        .timeout_write(TIMEOUT)
        .build();
    let mut call = agent.request(&request.method, &request.url);
    for (name, value) in &request.headers {
        call = call.set(name, value);
    }
    let sent = match request.body.is_empty() {
        true => call.call(),
        false => call.send_bytes(&request.body),
    };
    match sent {
        Ok(response) | Err(ureq::Error::Status(_, response)) => Ok(response),
        Err(error) => Err(error.to_string()),
    }
}

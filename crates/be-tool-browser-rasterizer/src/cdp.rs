//! A small client for the Chrome DevTools Protocol on `--remote-debugging-pipe`: Chromium reads
//! commands from its file descriptor 3 and writes responses and events to 4, each one JSON
//! message ended by a NUL byte. Commands are synchronous: [`Cdp::call`] waits for its response
//! and hands every event that arrives meanwhile to the [`Events`] it is given, which may answer
//! with commands of its own (whose responses are dropped).

use std::io::{BufRead as _, BufReader, PipeReader, PipeWriter, Write as _};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Instant;

use serde_json::{Value, json};

/// What a page's events change, handed each event as it arrives.
pub trait Events {
    fn event(&mut self, cdp: &mut Sender, session: Option<&str>, method: &str, params: &Value);
}

/// The half of the client that writes commands.
pub struct Sender {
    pipe: PipeWriter,
    next: u64,
}

impl Sender {
    /// Sends a command without waiting for its response, and returns its id.
    pub fn send(
        &mut self,
        session: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<u64, String> {
        self.next += 1;
        let mut message = json!({"id": self.next, "method": method, "params": params});
        if let Some(session) = session {
            message["sessionId"] = session.into();
        }
        let mut bytes = serde_json::to_vec(&message).map_err(|error| error.to_string())?;
        bytes.push(0);
        // No deadline: this blocks only if Chromium is alive but has stopped reading fd 3.
        self.pipe
            .write_all(&bytes)
            .map_err(|error| format!("could not send {method} to Chromium: {error}"))?;
        Ok(self.next)
    }
}

pub struct Cdp {
    sender: Sender,
    messages: Receiver<Result<Value, String>>,
}

impl Cdp {
    /// Talks to Chromium over the pipe it writes to (`from`, its 4) and reads from (`to`, its 3).
    pub fn new(from: PipeReader, to: PipeWriter) -> Self {
        let (send, messages) = channel();
        std::thread::spawn(move || {
            let mut from = BufReader::new(from);
            loop {
                let mut message = Vec::new();
                let read = from.read_until(0, &mut message);
                let parsed = match read {
                    Ok(0) => Err("Chromium closed its pipe".to_owned()),
                    Ok(_) => {
                        message.pop_if(|byte| *byte == 0);
                        serde_json::from_slice(&message).map_err(|error| error.to_string())
                    }
                    Err(error) => Err(error.to_string()),
                };
                let failed = parsed.is_err();
                if send.send(parsed).is_err() || failed {
                    return;
                }
            }
        });
        Self {
            sender: Sender { pipe: to, next: 0 },
            messages,
        }
    }

    /// Sends a command and waits until `deadline` for its result, handing events to `events`.
    pub fn call(
        &mut self,
        events: &mut impl Events,
        session: Option<&str>,
        method: &str,
        params: Value,
        deadline: Instant,
    ) -> Result<Value, String> {
        let id = self.sender.send(session, method, params)?;
        let mut response = None;
        self.wait(events, deadline, method, |message| {
            if message["id"].as_u64() == Some(id) {
                response = Some(message.clone());
                true
            } else {
                false
            }
        })?;
        let response = response.expect("waited for the response");
        if let Some(error) = response.get("error") {
            return Err(format!(
                "{method} failed: {}",
                error["message"].as_str().unwrap_or("")
            ));
        }
        Ok(response["result"].clone())
    }

    /// Waits until `deadline` for a message that `done` accepts, handing every event to `events`
    /// first.
    pub fn wait(
        &mut self,
        events: &mut impl Events,
        deadline: Instant,
        waiting_for: &str,
        mut done: impl FnMut(&Value) -> bool,
    ) -> Result<(), String> {
        loop {
            let timeout = deadline.saturating_duration_since(Instant::now());
            let message = match self.messages.recv_timeout(timeout) {
                Ok(message) => message?,
                Err(RecvTimeoutError::Timeout) => {
                    return Err(format!("timed out waiting for {waiting_for}"));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("Chromium closed its pipe".into());
                }
            };
            if let Some(method) = message["method"].as_str() {
                let session = message["sessionId"].as_str();
                events.event(&mut self.sender, session, method, &message["params"]);
            }
            if done(&message) {
                return Ok(());
            }
        }
    }
}

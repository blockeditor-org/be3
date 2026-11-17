//! The host's half of the root guest's entry (plans/host_as_platform_adapter.md, The root
//! guest), natively: `Root` runs be-host's own module on the calling thread, which is the
//! host's frame thread, and serves it the sandbox (be-wasm-sandbox-host) and `be3:root`'s
//! `host` (crates/be-wasm-root-api).
//!
//! `Root::frame` is the synchronous, resumable frame entry: it asks the root for a frame
//! (`frames.frame`, an op the root serves) and steps it until it answers. A step that leaves the
//! frame unanswered is the root's `Pending`: what it waits on is the ops it left open, its
//! children's frames (`instance.next`) and `deadline`. The host then waits until one of them
//! has something for it, steps the root again with it, and answers `deadline` once the frame's
//! deadline passes. So the root writes ordinary async code that awaits its children, and
//! answers within one host frame without ever blocking its own thread.

use std::{
    sync::{Arc, Condvar, Mutex},
    time::Instant,
};

use be_wasm_guest_host::STEP_TIMEOUT;
use be_wasm_host::App;
use be_wasm_ops::{Connection, Incoming, OpError, OpId, Outcome, Reply, Side};
use be_wasm_root_api::{FrameDone, FrameRequest, embed};
use be_wasm_sandbox_host::{Loader, Server};

/// The root guest, run inline on the thread that holds it. Dropping it stops the root and every
/// instance it spawned.
pub struct Root {
    app: App,
    connection: Connection,
    sandbox: Server,
    bell: Arc<Bell>,
    /// The `deadline` calls of the frame being drawn, answered once its deadline passes.
    deadlines: Vec<Reply<()>>,
    /// The frame being drawn: its op, and whether its deadline passed.
    drawing: Option<(OpId, bool)>,
    /// How the frame being drawn ended, once it did.
    drawn: Option<Outcome>,
    /// Why the root stopped, once it did: it never runs again.
    failure: Option<String>,
}

/// Rung whenever something the root waits on may have arrived: an instance it spawned sent
/// frames or stopped, a compile finished, or the root woke itself.
#[derive(Default)]
struct Bell {
    rung: Mutex<u64>,
    ringing: Condvar,
}

impl Bell {
    fn rung(&self) -> u64 {
        *self.rung.lock().unwrap_or_else(|held| held.into_inner())
    }

    fn ring(&self) {
        let mut rung = self.rung.lock().unwrap_or_else(|held| held.into_inner());
        *rung = rung.wrapping_add(1);
        self.ringing.notify_all();
    }

    /// Waits until it rings after `since` or `until` passes.
    fn wait(&self, since: u64, until: Instant) {
        let rung = self.rung.lock().unwrap_or_else(|held| held.into_inner());
        let left = until.saturating_duration_since(Instant::now());
        drop(
            self.ringing
                .wait_timeout_while(rung, left, |rung| *rung == since)
                .unwrap_or_else(|held| held.into_inner()),
        );
    }
}

impl Root {
    /// Starts `app`, the root guest, loaded on the calling thread, whose staged apps it spawns
    /// by id are found by `apps`.
    pub fn start(mut app: App, apps: Loader) -> Result<Self, String> {
        let bell = Arc::new(Bell::default());
        let ringer = Arc::clone(&bell);
        app.on_wake(move || ringer.ring());
        let ringer = Arc::clone(&bell);
        let sandbox = Server::new(Arc::new(move || ringer.ring()), apps);
        app.start()?;
        let mut root = Self {
            app,
            connection: Connection::new(Side::Host),
            sandbox,
            bell,
            deadlines: Vec::new(),
            drawing: None,
            drawn: None,
            failure: None,
        };
        root.received()?;
        Ok(root)
    }

    /// Draws a frame for `request`, waiting for the root's children until `deadline` and for the
    /// root itself as long as it takes, up to `STEP_TIMEOUT` past the deadline. A root that
    /// trapped, panicked or took longer says why, and draws nothing again.
    pub fn frame(&mut self, request: FrameRequest, deadline: Instant) -> Result<FrameDone, String> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let drawn = self.draw(request, deadline);
        self.drawing = None;
        self.drawn = None;
        for reply in std::mem::take(&mut self.deadlines) {
            reply.send(&mut self.connection, &());
        }
        if let Err(failure) = &drawn {
            self.failure = Some(failure.clone());
        }
        drawn
    }

    fn draw(&mut self, request: FrameRequest, deadline: Instant) -> Result<FrameDone, String> {
        let ticket = embed::exports::frames::frame(&mut self.connection, request);
        self.drawing = Some((ticket.op(), false));
        let give_up = deadline.max(Instant::now()) + STEP_TIMEOUT;
        loop {
            let since = self.bell.rung();
            let now = Instant::now();
            if now >= deadline {
                self.pass_deadline();
            }
            self.serve();
            if let Some(outcome) = self.drawn.take() {
                return ticket
                    .decode(outcome)
                    .map_err(|error| format!("the root did not draw its frame: {error}"));
            }
            if self.step()? {
                continue;
            }
            if now >= give_up {
                return Err(format!(
                    "the root's frame waited {}s past its deadline",
                    STEP_TIMEOUT.as_secs()
                ));
            }
            // Pending: wait until what the root waits on has something for it.
            self.bell.wait(since, if now >= deadline { give_up } else { deadline });
        }
    }

    /// The frame's deadline passed: the root hears it, and every later `deadline` call of the
    /// frame answers at once.
    fn pass_deadline(&mut self) {
        if let Some((_, passed)) = &mut self.drawing {
            *passed = true;
        }
        for reply in std::mem::take(&mut self.deadlines) {
            reply.send(&mut self.connection, &());
        }
    }

    /// Steps the root with what the host has for it, if anything, and takes what it sent.
    /// Answers whether it stepped.
    fn step(&mut self) -> Result<bool, String> {
        let frames = self.connection.take_outbound();
        let woken = self.app.take_wake();
        if frames.is_empty() && !woken {
            return Ok(false);
        }
        self.app.deliver(frames);
        self.app.step()?;
        self.received()?;
        Ok(true)
    }

    /// Takes the frames the root sent.
    fn received(&mut self) -> Result<(), String> {
        for frame in self.app.take_outbound() {
            self.connection
                .receive(&frame)
                .map_err(|error| format!("the root sent a frame the host cannot read: {error}"))?;
        }
        Ok(())
    }

    /// Answers the root's calls, and whatever its instances and compiles have for it.
    fn serve(&mut self) {
        while let Some(event) = self.connection.next_event() {
            match event {
                Incoming::Started(started) => {
                    if be_wasm_sandbox_api::embed::sandbox::Call::accepts(&started) {
                        if let Some(call) = be_wasm_sandbox_api::embed::sandbox::Call::accept(
                            &mut self.connection,
                            started,
                        ) {
                            self.sandbox.call(call, &mut self.connection);
                        }
                    } else if let Some(call) = embed::Call::accept(&mut self.connection, started)
                    {
                        self.call(call);
                    }
                }
                Incoming::Canceled { op } => {
                    if !self.sandbox.canceled(op, &mut self.connection)
                        && let Some(at) = self.deadlines.iter().position(|reply| reply.op() == op)
                    {
                        self.deadlines
                            .remove(at)
                            .fail(&mut self.connection, OpError::Canceled);
                    }
                }
                Incoming::Completed { op, outcome }
                    if self.drawing.is_some_and(|(drawing, _)| drawing == op) =>
                {
                    self.drawn = Some(outcome);
                }
                Incoming::Completed { .. }
                | Incoming::Yielded { .. }
                | Incoming::Credited { .. } => {}
            }
        }
        self.sandbox.poll(&mut self.connection);
    }

    fn call(&mut self, call: embed::Call) {
        match call {
            embed::Call::Host(embed::host::Call::Deadline { reply }) => match self.drawing {
                Some((_, false)) => self.deadlines.push(reply),
                Some((_, true)) | None => reply.send(&mut self.connection, &()),
            },
        }
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        self.app.stop();
    }
}

#[cfg(test)]
mod tests;

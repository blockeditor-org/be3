//! The host's half of the root guest's entry (plans/host_as_platform_adapter.md, The root
//! guest): `Root` runs be-host's own module inline on the host's frame thread, and serves it the
//! sandbox (be-wasm-sandbox-host) and `be3:root`'s `host` (crates/be-wasm-root-api).
//!
//! Input is the root's `input.deliver`, an op the host starts and steps the root through with the
//! frame being drawn, or else the next one, so the root routes it by the layers it drew last.
//!
//! A frame is the root's `frames.frame`, an op the root serves, which the host starts and steps
//! the root through until it answers. A step that leaves the frame unanswered is the root's
//! `Pending`: what it waits on is the ops it left open, its children's frames (`instance.next`)
//! and `deadline`. The host steps the root again once one of them has something for it, and
//! answers `deadline` once the frame's deadline passes. So the root writes ordinary async code
//! that awaits its children, and answers within one host frame without ever blocking its own
//! thread. `Core` is that stepping, the same on every platform; how the host waits between steps
//! is not:
//!
//! - natively (`native`), `Root::frame` waits on a bell its instances, compiles and own wakes
//!   ring, up to the deadline, so it returns the drawn frame;
//! - in the browser (`web`), the page may never wait (decisions/the_web_page_never_waits.md), so
//!   `Root::frame`, called from `requestAnimationFrame`, steps the root for a slice of the page's
//!   thread at most and returns; a task resumes it whenever its workers answer, a compile
//!   finishes, it wakes itself, its slice ran out or the deadline passes, and the frame is handed
//!   over once the root answers.

use std::time::{Duration, Instant};

use be_wasm_guest_host::STEP_TIMEOUT;
use be_wasm_ops::{Connection, Incoming, OpError, OpId, Outcome, Reply, Side, Ticket};
use be_wasm_root_api::{FontFace, FrameDone, FrameRequest, embed};
use be_wasm_sandbox_host::Server;
pub use be_wasm_sandbox_host::{Loader, Staged};

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::Root;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::Root;

/// The root guest as its host steps it, inline on the host's thread.
trait Inline {
    /// Queues `frames` for the root's next step.
    fn deliver(&mut self, frames: Vec<Vec<u8>>);
    /// Steps the root once, or says why it trapped or panicked.
    fn step(&mut self) -> Result<(), String>;
    /// The frames the root sent.
    fn take_outbound(&mut self) -> Vec<Vec<u8>>;
    /// Whether the root woke itself since the last call.
    fn take_wake(&mut self) -> bool;
}

/// Where a frame got to.
enum Progress {
    /// The root answered the frame, or why it never will: it trapped, panicked or took longer
    /// than its patience.
    Drawn(Result<FrameDone, String>),
    /// The root waits for what its host has not had yet: step it again once something it waits
    /// on rings, or at `until` at the latest.
    Pending { until: Instant },
    /// The root used its slice of the thread while it still had work: step it again soon, after
    /// whatever else the thread has to do.
    Yielded,
}

/// The frame being drawn.
struct Drawing {
    ticket: Ticket<FrameDone>,
    deadline: Instant,
    /// When the root itself counts as stopped: its patience past the deadline.
    give_up: Instant,
    /// Whether the deadline passed.
    passed: bool,
}

/// The stepping both platforms share: the root, its connection, its sandbox and the frame it
/// draws.
struct Core<G> {
    guest: G,
    connection: Connection,
    sandbox: Server,
    /// The `deadline` calls of the frame being drawn, answered once its deadline passes.
    deadlines: Vec<Reply<()>>,
    drawing: Option<Drawing>,
    /// How the frame being drawn ended, once it did.
    drawn: Option<Outcome>,
    /// Why the root stopped, once it did: it never runs again.
    failure: Option<String>,
    /// How long past a frame's deadline the root itself may take before it counts as stopped.
    patience: Duration,
    /// The fonts the host bundles, which `host.fonts` answers.
    fonts: Vec<FontFace>,
    /// The paintings asked of the root and not answered yet.
    painting: Option<Ticket<Vec<u8>, be_wasm_ops::wire::Bytes>>,
    /// What the root answered it painted, until the host takes it.
    painted: Option<Vec<u8>>,
}

impl<G: Inline> Core<G> {
    /// Takes over `guest`, the root, which has started, and what it sent while it started.
    fn new(guest: G, sandbox: Server) -> Result<Self, String> {
        let mut core = Self {
            guest,
            connection: Connection::new(Side::Host),
            sandbox,
            deadlines: Vec::new(),
            drawing: None,
            drawn: None,
            failure: None,
            patience: STEP_TIMEOUT,
            fonts: Vec::new(),
            painting: None,
            painted: None,
        };
        core.received()?;
        Ok(core)
    }

    /// Whether a frame is being drawn, which on the web may wait between calls.
    #[cfg(target_arch = "wasm32")]
    fn drawing(&self) -> bool {
        self.drawing.is_some()
    }

    /// Hands the root `window`'s input, which it takes in with the steps of the frame being
    /// drawn, or else of the next one, ahead of drawing it, so the root routes it by the layers
    /// it drew last. Its answer, which the root gives once it has routed, is not waited for. A root that stopped hears nothing, and says why.
    fn input(&mut self, window: Option<u64>, events: Vec<u8>) -> Result<(), String> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let _answered = embed::exports::input::deliver(&mut self.connection, window, events);
        Ok(())
    }

    /// Asks the root what its own surfaces painted, which it answers as it steps through the
    /// next frame (`take_paintings`). A root that stopped says why.
    fn ask_paintings(&mut self) -> Result<(), String> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        if self.painting.is_none() {
            self.painting = Some(embed::exports::paintings::paintings(&mut self.connection));
        }
        Ok(())
    }

    /// What the root answered `ask_paintings` with, once it did.
    fn take_paintings(&mut self) -> Option<Vec<u8>> {
        self.painted.take()
    }

    /// Asks the root for a frame for `request`, drawn by `deadline`, or says why the root drew
    /// nothing since it stopped.
    fn begin(&mut self, request: FrameRequest, deadline: Instant) -> Result<(), String> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let ticket = embed::exports::frames::frame(&mut self.connection, request);
        self.drawing = Some(Drawing {
            ticket,
            deadline,
            give_up: deadline.max(Instant::now()) + self.patience,
            passed: false,
        });
        Ok(())
    }

    /// Steps the root as far as it goes without waiting: until it answers the frame, or waits
    /// for something its host has not had yet, or, given a `slice`, until it stepped that long.
    fn advance(&mut self, slice: Option<Duration>) -> Progress {
        let yield_at = slice.map(|slice| Instant::now() + slice);
        loop {
            let Some(drawing) = &self.drawing else {
                return Progress::Drawn(Err("no frame is being drawn".to_owned()));
            };
            let now = Instant::now();
            // Checked before stepping, so a root that keeps waking itself, or children that keep
            // sending, cannot hold the frame past it.
            if now >= drawing.give_up {
                let failure = format!(
                    "the root's frame took over {}ms past its deadline",
                    self.patience.as_millis()
                );
                return self.finish(Err(failure));
            }
            if now >= drawing.deadline {
                self.pass_deadline();
            }
            self.serve();
            let Some(drawing) = &self.drawing else {
                continue;
            };
            let ticket = drawing.ticket;
            // Past the deadline, the root itself may take until its patience runs out.
            let until = match drawing.passed {
                true => drawing.give_up,
                false => drawing.deadline,
            };
            if let Some(outcome) = self.drawn.take() {
                let drawn = ticket
                    .decode(outcome)
                    .map_err(|error| format!("the root did not draw its frame: {error}"));
                return self.finish(drawn);
            }
            match self.step() {
                Ok(true) if yield_at.is_some_and(|at| Instant::now() >= at) => {
                    return Progress::Yielded;
                }
                Ok(true) => {}
                Ok(false) => return Progress::Pending { until },
                Err(failure) => return self.finish(Err(failure)),
            }
        }
    }

    /// Ends the frame being drawn with `drawn`: every `deadline` call of it answers, and a root
    /// that failed draws nothing again.
    fn finish(&mut self, drawn: Result<FrameDone, String>) -> Progress {
        self.drawing = None;
        self.drawn = None;
        for reply in std::mem::take(&mut self.deadlines) {
            reply.send(&mut self.connection, &());
        }
        if let Err(failure) = &drawn {
            self.failure = Some(failure.clone());
        }
        Progress::Drawn(drawn)
    }

    /// The frame's deadline passed: the root hears it, and every later `deadline` call of the
    /// frame answers at once.
    fn pass_deadline(&mut self) {
        if let Some(drawing) = &mut self.drawing {
            drawing.passed = true;
        }
        for reply in std::mem::take(&mut self.deadlines) {
            reply.send(&mut self.connection, &());
        }
    }

    /// Steps the root with what the host has for it, if anything, and takes what it sent.
    /// Answers whether it stepped.
    fn step(&mut self) -> Result<bool, String> {
        let frames = self.connection.take_outbound();
        let woken = self.guest.take_wake();
        if frames.is_empty() && !woken {
            return Ok(false);
        }
        self.guest.deliver(frames);
        self.guest.step()?;
        self.received()?;
        Ok(true)
    }

    /// Takes the frames the root sent.
    fn received(&mut self) -> Result<(), String> {
        for frame in self.guest.take_outbound() {
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
                    } else if let Some(call) = embed::Call::accept(&mut self.connection, started) {
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
                Incoming::Completed { op, outcome } if self.drawing_op() == Some(op) => {
                    self.drawn = Some(outcome);
                }
                Incoming::Completed { op, outcome }
                    if self.painting.is_some_and(|ticket| ticket.op() == op) =>
                {
                    if let Some(ticket) = self.painting.take() {
                        // A root that cannot answer paints nothing the host can show.
                        self.painted = Some(ticket.decode(outcome).unwrap_or_default());
                    }
                }
                // Includes a frame answered after the host gave up on it: no one waits for it.
                Incoming::Completed { .. }
                | Incoming::Yielded { .. }
                | Incoming::Credited { .. } => {}
            }
        }
        self.sandbox.poll(&mut self.connection);
    }

    fn drawing_op(&self) -> Option<OpId> {
        self.drawing.as_ref().map(|drawing| drawing.ticket.op())
    }

    fn call(&mut self, call: embed::Call) {
        match call {
            embed::Call::Host(embed::host::Call::Deadline { reply }) => match &self.drawing {
                Some(drawing) if !drawing.passed => self.deadlines.push(reply),
                Some(_) | None => reply.send(&mut self.connection, &()),
            },
            embed::Call::Host(embed::host::Call::Fonts { reply }) => {
                reply.send(&mut self.connection, &self.fonts);
            }
        }
    }
}

#[cfg(test)]
mod tests;

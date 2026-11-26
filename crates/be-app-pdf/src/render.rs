use std::{cell::Cell, collections::HashMap, time::Instant};

use be_sdk_beui::frame_time;

use be_sdk_beui::worker::{Shared, Ticket};
use be_sdk_beui::{
    PerformanceReporter, Waker,
    beui::{Image, Pos2, Vec2},
};
pub(crate) use be_worker_pdf::Unrendered;
use be_worker_pdf::{Answer, Document, Job, Measure, Rendered};
pub(crate) use be_worker_pdf::{DETAIL_MAX_DIM, MIN_SCALE, Target as RenderTarget};

pub(crate) struct RenderedTile {
    pub(crate) page_count: usize,
    pub(crate) page_index: usize,
    pub(crate) page_size_pts: Vec2,
    pub(crate) scale: f32,
    pub(crate) origin_pts: Pos2,
    pub(crate) size_pts: Vec2,
    pub(crate) image: Image,
}

#[derive(Clone, Copy)]
pub(crate) struct RenderRequest {
    pub(crate) revision: u64,
    pub(crate) page: usize,
    pub(crate) target: RenderTarget,
}

pub(crate) type RenderJobResult = Result<RenderedTile, Unrendered>;

thread_local! {
    /// The id the next client of the renderer takes, which names the documents it sends.
    static NEXT_CLIENT: Cell<u64> = const { Cell::new(1) };
}

/// One pane's way to the renderer (be-worker-pdf-render), which every PDF the app shows shares:
/// it sends the pane's jobs and hears only their answers, in order.
pub(crate) struct RenderClient {
    renderer: Shared<Job, Answer>,
    /// The frame each job still unanswered was sent in, by the frame clock, and whether it
    /// carried the PDF's bytes.
    sent: HashMap<Ticket, (Option<Instant>, bool)>,
    /// This client's id, which its documents carry (`Document::client`).
    client: u64,
    /// The revision whose bytes it sent the renderer last, which the renderer is taken to hold
    /// until it answers that it does not or stops.
    opened: Option<u64>,
}

impl RenderClient {
    pub(crate) fn new(waker: &Waker) -> Self {
        let waker = waker.clone();
        Self {
            renderer: Shared::new(be_worker_pdf::MODULE, move || waker.wake()),
            sent: HashMap::new(),
            client: NEXT_CLIENT.with(|next| next.replace(next.get() + 1)),
            opened: None,
        }
    }

    /// Sends a job to render `target` of page `page` of `revision`, with the PDF's bytes from
    /// `data` only when the renderer is not taken to hold that revision already. Answers whether it
    /// sent one: not when it needed the bytes and `data` had none.
    pub(crate) fn send(
        &mut self,
        revision: u64,
        page: usize,
        target: RenderTarget,
        performance: &PerformanceReporter,
        data: impl FnOnce() -> Option<Vec<u8>>,
    ) -> bool {
        let data = match self.opened == Some(revision) {
            true => None,
            false => {
                let copied = Instant::now();
                let Some(data) = data() else {
                    return false;
                };
                performance.record_duration("PDF data copy", copied.elapsed());
                performance.record_count("PDF bytes", data.len() as u64);
                self.opened = Some(revision);
                Some(data)
            }
        };
        let document = Document {
            client: self.client,
            revision,
        };
        let opens = data.is_some();
        let ticket = self.renderer.send(&Job {
            document,
            page,
            target,
            data,
        });
        self.sent.insert(ticket, (frame_time(), opens));
        true
    }

    /// The next answer to a job, once it came, with what the renderer measured recorded to
    /// `performance`. A job the renderer stopped before answering fails alone: what was sent since is answered
    /// by the renderer started anew. Each job that fails is its own `Err`, which is as it should
    /// be: `Pane::ensure` keeps one job in flight, so a stop fails at most one. A renderer that
    /// does not hold the job's document answers `Unopened`, and the next job sends its bytes.
    pub(crate) fn receive(&mut self, performance: &PerformanceReporter) -> Option<RenderJobResult> {
        let (ticket, answer) = self.renderer.try_receive()?;
        let (sent, opened) = self.sent.remove(&ticket).unwrap_or((None, false));
        let answer = match answer {
            Ok(answer) => answer,
            Err(why) => {
                // A renderer started anew holds nothing.
                self.opened = None;
                return Some(Err(Unrendered::Failed(why)));
            }
        };
        // A document it does not hold, or one the job sent that could not be opened: the next
        // job sends the bytes again rather than asking for what is not there.
        match &answer.result {
            Err(Unrendered::Unopened) => self.opened = None,
            Err(Unrendered::Failed(_)) if opened => self.opened = None,
            _ => {}
        }
        if let (Some(sent), Some(now)) = (sent, frame_time()) {
            performance.record_duration("Round trip", now.saturating_duration_since(sent));
        }
        for measure in answer.measures {
            match measure {
                Measure::Duration(name, took) => performance.record_duration(name, took),
                Measure::Count(name, count) => performance.record_count(name, count),
            }
        }
        Some(answer.result.map(|rendered| {
            let Rendered {
                page_count,
                page_index,
                page_size_pts,
                scale,
                origin_pts,
                size_pts,
                width,
                height,
                rgba,
            } = rendered;
            RenderedTile {
                page_count,
                page_index,
                page_size_pts,
                scale,
                origin_pts,
                size_pts,
                image: Image::from_rgba(width, height, rgba),
            }
        }))
    }
}

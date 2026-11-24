use std::{collections::HashMap, time::Instant};

use be_sdk_beui::frame_time;

use be_sdk_beui::worker::{Shared, Ticket};
use be_sdk_beui::{
    PerformanceReporter, Waker,
    beui::{Image, Pos2, Vec2},
};
use be_worker_pdf::{Answer, Job, Measure, Rendered};
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

pub(crate) type RenderJobResult = Result<RenderedTile, String>;

/// One pane's way to the renderer (be-worker-pdf-render), which every PDF the app shows shares:
/// it sends the pane's jobs and hears only their answers, in order.
pub(crate) struct RenderClient {
    renderer: Shared<Job, Answer>,
    /// The frame each job still unanswered was sent in, by the frame clock.
    sent: HashMap<Ticket, Option<Instant>>,
}

impl RenderClient {
    pub(crate) fn new(waker: &Waker) -> Self {
        let waker = waker.clone();
        Self {
            renderer: Shared::new(be_worker_pdf::MODULE, move || waker.wake()),
            sent: HashMap::new(),
        }
    }

    pub(crate) fn send(&mut self, data: Vec<u8>, page: usize, target: RenderTarget) {
        let ticket = self.renderer.send(&Job { page, target, data });
        self.sent.insert(ticket, frame_time());
    }

    /// The answer to the oldest job still unanswered, once it came, with what the renderer
    /// measured recorded to `performance`.
    /// A job the renderer stopped before answering fails alone: what was sent since is answered
    /// by the renderer started anew.
    pub(crate) fn receive(&mut self, performance: &PerformanceReporter) -> Option<RenderJobResult> {
        let (ticket, answer) = self.renderer.try_receive()?;
        let sent = self.sent.remove(&ticket).flatten();
        let answer = match answer {
            Ok(answer) => answer,
            Err(why) => return Some(Err(why)),
        };
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

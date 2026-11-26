//! The PDF app's renderer (crates/be-worker-pdf says what it is sent and answers): a worker that
//! starts PDFium once and renders each job with it, keeping the documents it opened last
//! (`OPEN_DOCUMENTS`) so a job need not carry its PDF again. `worker.rs` exports `Renderer` as the
//! module `render.wasm`; only it and the PDF app's tests, which serve `Renderer` in their own
//! instance, depend on this crate, so the app itself links no PDFium.

use std::time::Instant;

use be_sdk_worker::{App, Work};
use be_worker_pdf::{Answer, Document, Job, OPEN_DOCUMENTS, Unrendered};

mod pdfium;

pub struct Renderer {
    /// The documents it holds open, the least recently used first.
    open: Vec<(Document, pdfium::Pdf)>,
}

impl Renderer {
    /// The document `document`, opened from `data` when sent, which lets go of what the same
    /// client held before and of the least recently used past `OPEN_DOCUMENTS`.
    fn document(
        &mut self,
        document: Document,
        data: Option<Vec<u8>>,
        measures: &mut Vec<be_worker_pdf::Measure>,
    ) -> Result<&pdfium::Pdf, Unrendered> {
        if let Some(data) = data {
            self.open.retain(|(held, _)| held.client != document.client);
            let started = Instant::now();
            let pdf = pdfium::Pdf::open(data).map_err(Unrendered::Failed)?;
            pdfium::record(measures, "Document load", started);
            if self.open.len() >= OPEN_DOCUMENTS {
                self.open.remove(0);
            }
            self.open.push((document, pdf));
        } else {
            let index = self
                .open
                .iter()
                .position(|(held, _)| *held == document)
                .ok_or(Unrendered::Unopened)?;
            let used = self.open.remove(index);
            self.open.push(used);
        }
        Ok(&self.open.last().expect("it holds the document it opened").1)
    }
}

impl Work for Renderer {
    type In = Job;
    type Out = Answer;

    fn start(_app: &mut App<Answer>) -> Result<Self, String> {
        pdfium::start();
        Ok(Self { open: Vec::new() })
    }

    fn receive(&mut self, job: Job, app: &mut App<Answer>) -> Result<(), String> {
        let mut measures = Vec::new();
        let started = Instant::now();
        let result = self
            .document(job.document, job.data, &mut measures)
            .and_then(|pdf| {
                pdfium::render(pdf, job.page, job.target, &mut measures).map_err(Unrendered::Failed)
            });
        pdfium::record(&mut measures, "Worker total", started);
        app.send(&Answer { result, measures });
        Ok(())
    }
}

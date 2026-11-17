//! The PDF app's renderer (crates/be-worker-pdf says what it is sent and answers): a worker that
//! starts PDFium once and renders each job with it. `worker.rs` exports `Renderer` as the module
//! `render.wasm`; only it and the PDF app's tests, which serve `Renderer` in their own instance,
//! depend on this crate, so the app itself links no PDFium.

use be_sdk_worker::{App, Work};
use be_worker_pdf::{Answer, Job};

mod pdfium;

pub struct Renderer;

impl Work for Renderer {
    fn start(_app: &mut App) -> Result<Self, String> {
        pdfium::start();
        Ok(Self)
    }

    fn receive(&mut self, message: Vec<u8>, app: &mut App) -> Result<(), String> {
        let job = Job::decode(&message)?;
        let mut measures = Vec::new();
        let result = pdfium::render(&job.data, job.page, job.target, &mut measures);
        let answer = Answer { result, measures };
        app.send(answer.encode());
        Ok(())
    }
}

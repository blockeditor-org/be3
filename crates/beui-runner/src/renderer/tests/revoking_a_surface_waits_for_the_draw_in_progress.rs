use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, mpsc};

use super::*;

/// Presents only once told to, and logs when it is done.
struct Blocking {
    drawing: mpsc::Sender<()>,
    revoking: Arc<AtomicBool>,
    log: Arc<Mutex<Vec<&'static str>>>,
}

impl Renderer for Blocking {
    fn info(&self) -> RendererInfo {
        RendererInfo::default()
    }

    fn resize(&mut self, _width: u32, _height: u32) {}

    fn physical(&self) -> Option<Vec2> {
        Some(Vec2::new(10.0, 10.0))
    }

    fn prepare(&mut self, _output: &WindowOutput, _scale: f32, _background: Color32) -> bool {
        true
    }

    fn present(&mut self, _background: Color32) -> bool {
        self.drawing.send(()).unwrap();
        while !self.revoking.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        // Gives the revoking thread every chance to get past the lease, were it free.
        for _ in 0..10_000 {
            std::thread::yield_now();
        }
        self.log.lock().unwrap().push("presented");
        false
    }
}

#[test]
fn revoking_a_surface_waits_for_the_draw_in_progress() {
    let lease = SurfaceLease::new();
    let log = Arc::new(Mutex::new(Vec::new()));
    let revoking = Arc::new(AtomicBool::new(false));
    let (drawing, drawn) = mpsc::channel();
    let (mut renderer, attaching) = Leased::new(
        Box::new(Blocking {
            drawing,
            revoking: Arc::clone(&revoking),
            log: log.clone(),
        }),
        lease.clone(),
    );
    attaching.set(lease.current());
    renderer.attach(Arc::new(Window), 10, 10).unwrap();

    let other = (lease.clone(), revoking, log.clone());
    std::thread::scope(|threads| {
        threads.spawn(move || {
            let (lease, revoking, log) = other;
            drawn.recv().unwrap();
            revoking.store(true, Ordering::SeqCst);
            lease.revoke();
            log.lock().unwrap().push("revoked");
        });
        renderer.present(Color32::BLACK);
    });
    assert_eq!(
        *log.lock().unwrap(),
        ["presented", "revoked"],
        "the surface is revoked only once the present drawing to it is done"
    );
}

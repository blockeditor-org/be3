use super::*;

/// Checks the lease is held for as long as it draws.
struct Watching(Arc<SurfaceLease>);

impl Renderer for Watching {
    fn info(&self) -> RendererInfo {
        RendererInfo::default()
    }

    fn resize(&mut self, _width: u32, _height: u32) {}

    fn physical(&self) -> Option<Vec2> {
        Some(Vec2::new(10.0, 10.0))
    }

    fn prepare(&mut self, _output: &WindowOutput, _scale: f32, _background: Color32) -> bool {
        assert!(self.0.current.try_lock().is_err(), "preparing holds the lease");
        true
    }

    fn present(&mut self, _background: Color32) -> bool {
        assert!(
            self.0.current.try_lock().is_err(),
            "presenting holds the lease, so a revoke waits for it"
        );
        false
    }
}

#[test]
fn revoking_a_surface_waits_for_the_draw_in_progress() {
    let lease = SurfaceLease::new();
    let (mut renderer, attaching) =
        Leased::new(Box::new(Watching(lease.clone())), lease.clone());
    attaching.set(lease.current());
    renderer.attach(Arc::new(Window), 10, 10).unwrap();
    renderer.present(Color32::BLACK);
    assert!(
        lease.current.try_lock().is_ok(),
        "the lease is free between draws, so a revoke never waits for a frame"
    );
}

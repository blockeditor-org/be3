use super::*;

#[test]
fn a_device_lost_before_wgpu_reports_it_draws_nothing_rather_than_panicking() {
    let mut target = Target::new();
    // The validation errors a lost device's calls raise are not this test's: a platform's
    // handler ignores them once the loss is reported.
    target
        .device
        .on_uncaptured_error(std::sync::Arc::new(|_| {}));
    let paint = |painter: &Painter| {
        painter.rect_filled(everything(), 0.0, Color32::WHITE);
        painter.text(
            pos2(4.0, 4.0),
            "lost",
            FontId::proportional(14.0),
            Color32::BLACK,
        );
    };
    target.draw(Color32::BLACK, Repaint::Everything, paint);

    target.device.destroy();
    // wgpu calls the lost callback only at the next poll, so this frame's uploads meet
    // buffers with no mapped range. Without the wgpu and vello_gpu overlays
    // (third-party/rust/fixups.bzl) this panics.
    assert!(!target.renderer.resources().lost());
    target.draw(Color32::BLACK, Repaint::Everything, paint);

    let _ = target.device.poll(wgpu::PollType::wait_indefinitely());
    assert!(target.renderer.resources().lost(), "the loss is reported");
}

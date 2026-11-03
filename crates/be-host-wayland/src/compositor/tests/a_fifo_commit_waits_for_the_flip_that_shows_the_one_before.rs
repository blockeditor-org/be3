use super::*;

fn buffer_width(harness: &mut Harness, id: WindowId) -> f64 {
    harness.compositor().server().state.layers(id)[0].buffer.w
}

#[test]
fn a_fifo_commit_waits_for_the_flip_that_shows_the_one_before() {
    let (mut harness, device, queue) = Harness::with_gpu();
    harness.compositor().pace_to_flips();
    let (window, id) = harness.open();
    let mut painter = Painter::new(&device, &queue);
    let fifo = harness.client.fifo(&window.surface);

    fifo.set_barrier();
    harness.client.attach_unsent(&window, 300, 200);
    harness.frame(Vec::new());
    painter.paint(&harness);
    harness.frame(Vec::new());
    assert_eq!(buffer_width(&mut harness, id), 300.0);

    fifo.wait_barrier();
    fifo.set_barrier();
    harness.client.attach_unsent(&window, 280, 200);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        buffer_width(&mut harness, id),
        300.0,
        "the commit waits while the one that set the barrier is not yet on screen"
    );

    harness
        .compositor()
        .presented(Rect::from_min_size(Pos2::ZERO, SCREEN), flip(1));
    harness.frame(Vec::new());
    assert_eq!(
        buffer_width(&mut harness, id),
        280.0,
        "the flip that shows it lets the next commit through"
    );
}

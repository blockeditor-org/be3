use super::*;

fn allocated(device: &wgpu::Device) -> u64 {
    device
        .generate_allocator_report()
        .expect("the device reports its allocations")
        .total_allocated_bytes
}

#[test]
fn uploads_are_submitted_while_nothing_is_painted() {
    let (mut harness, device, _queue) = Harness::with_gpu();
    let (window, _) = harness.open();
    let (width, height) = (700, 500);
    let frame = (width * height * 4) as u64;
    harness.client.attach_unsent(&window, width, height);
    harness.settle();

    let before = allocated(&device);
    let commits = 24;
    for _ in 0..commits {
        harness.client.attach_unsent(&window, width, height);
        harness.frame(Vec::new());
    }
    harness.settle();
    let grown = allocated(&device).saturating_sub(before);

    assert!(
        grown < frame * 4,
        "{commits} commits of a {width} by {height} window grew GPU memory by {grown} bytes"
    );
}

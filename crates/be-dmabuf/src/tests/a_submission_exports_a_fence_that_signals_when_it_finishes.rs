use super::*;

#[test]
fn a_submission_exports_a_fence_that_signals_when_it_finishes() {
    let (device, queue) = vulkan_device();
    let vulkan = Vulkan::of(&device).expect("the device can import dmabufs");
    let Some(sync) = vulkan.sync_files() else {
        eprintln!("this Vulkan device cannot export sync files, so there is nothing to check");
        return;
    };
    for _ in 0..3 {
        let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let fence = sync
            .submit(&queue, encoder.finish())
            .expect("the fence exports");
        if let Some(fence) = fence {
            let mut polled = [rustix::event::PollFd::new(
                &fence,
                rustix::event::PollFlags::IN,
            )];
            let ready = rustix::event::poll(&mut polled, None).expect("the fence can be polled");
            assert_eq!(ready, 1, "the fence signals once the work is done");
        }
    }
}

use super::*;

#[test]
fn exiting_takes_the_compositor_down_before_the_thread_ends() {
    let adapter = pollster::block_on(wgpu::Instance::default().request_adapter(
        &wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        },
    ))
    .expect("no graphics adapter is available");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits::downlevel_defaults(),
        ..Default::default()
    }))
    .expect("the adapter did not provide a device");
    let document = beui::reactive::build(|| {
        create();
        view! {
            <Frame />
        }
    });
    let mut setup = Setup::new(beui::Waker::new(|| {}));
    setup.provide(beui::GpuSetup {
        device,
        queue,
        format: wgpu::TextureFormat::Rgba8Unorm,
    });
    start(&setup);
    assert!(running(), "the compositor starts on the GPU it is given");
    assert!(windows().is_some());

    exiting();
    assert!(
        !running(),
        "the compositor, with the GPU textures and Wayland clients it holds, is gone once the app \
         has exited, rather than left for the thread's destructors at process exit"
    );
    assert!(
        windows().is_none(),
        "the window list, whose drawings hold GPU buffers, is gone too"
    );
    drop(document);
}

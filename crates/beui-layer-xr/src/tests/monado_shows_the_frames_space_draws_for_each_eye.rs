use beui_layer_space::Space;

use super::*;
use crate::{Link, Status};

#[test]
fn monado_shows_the_frames_space_draws_for_each_eye() {
    let (_held, directory) = Monado::directory("shows");
    let monado = Monado::start(&directory);
    let link = Link::new("beui-layer-xr test");
    assert!(
        matches!(link.status(), Status::Found(_)),
        "a running Monado is found at once: {:?}",
        link.status()
    );
    let gpu = pollster::block_on(beui_renderer_wgpu::present::create_offscreen(
        beui_renderer_wgpu::offscreen::FORMAT,
        link.open_device(&[]),
    ))
    .expect("OpenXR's Vulkan device opens");
    let mut host = XrSpace::new(Space::new(), link);
    setup(&mut host, &gpu, Waker::new(|| {}));
    assert!(host.headset().is_some(), "the device opened through Monado");
    let output = app();
    let layered = show_frames(&mut host, &output);
    assert_eq!(
        host.space.notice(),
        None,
        "no notice while the headset shows"
    );
    assert_eq!(
        layered, 1,
        "the main panel, straight ahead, is shown as a layer of its own"
    );
    drop(host);
    drop(gpu);
    drop(monado);
    remove(&directory);
}

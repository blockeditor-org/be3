use std::sync::mpsc;

use beui_layer_space::Space;

use super::*;
use crate::WAITING;
use crate::{Link, Status};

fn found(link: &Link, woken: &mpsc::Receiver<()>) {
    while !matches!(link.status(), Status::Found(_)) {
        woken
            .recv_timeout(Duration::from_secs(60))
            .unwrap_or_else(|_| {
                panic!(
                    "the link wakes the loop once Monado runs: {:?}",
                    link.status()
                )
            });
    }
}

fn waiting(host: &XrSpace) -> String {
    let notice = host.space.notice().expect("the mirror shows a notice");
    assert_eq!(notice.title, WAITING);
    notice.detail.clone()
}

fn connect(host: &mut XrSpace, gpu: &mut Gpu, waker: &Waker, output: &FrameOutput) {
    assert!(
        gpu.stale(),
        "the device the headset needs replaces the one in use"
    );
    pollster::block_on(gpu.reopen(None)).expect("the device reopens through OpenXR");
    setup(host, gpu, waker.clone());
    assert!(host.headset().is_some(), "the layer takes the headset");
    show_frames(host, output);
    assert_eq!(
        host.space.notice(),
        None,
        "the notice goes once frames show"
    );
}

#[test]
fn a_headset_found_after_the_start_is_shown_and_waited_for_again_when_its_runtime_stops() {
    let (_held, directory) = Monado::directory("waits");
    let link = Link::new("beui-layer-xr test");
    assert!(
        matches!(link.status(), Status::Missing(_)),
        "no runtime runs yet: {:?}",
        link.status()
    );
    let mut gpu = pollster::block_on(beui_renderer_wgpu::present::create_offscreen(
        beui_renderer_wgpu::offscreen::FORMAT,
        link.open_device(&[]),
    ))
    .expect("a plain device opens while there is no headset");
    assert!(!gpu.stale(), "and is kept while there is none");
    let (wake, woken) = mpsc::channel();
    let waker = Waker::new(move || {
        let _ = wake.send(());
    });
    let mut host = XrSpace::new(Space::new(), link.clone());
    setup(&mut host, &gpu, waker.clone());
    let output = app();
    let mut last = None;
    step(&mut host, &output, &mut last);
    assert!(host.headset().is_none());
    assert!(
        waiting(&host).contains("not running"),
        "the notice says why: {}",
        waiting(&host)
    );

    let monado = Monado::start(&directory);
    found(&link, &woken);
    connect(&mut host, &mut gpu, &waker, &output);

    drop(monado);
    let mut last = None;
    for _ in 0..10_000 {
        if host.headset().is_none() {
            break;
        }
        step(&mut host, &output, &mut last);
    }
    assert!(
        host.headset().is_none(),
        "a stopped runtime lets the headset go"
    );
    assert!(
        host.space.tracked().is_none(),
        "and the mirror goes back to the simulator's head"
    );
    step(&mut host, &output, &mut last);
    assert!(!waiting(&host).is_empty());

    let monado = Monado::start(&directory);
    found(&link, &woken);
    connect(&mut host, &mut gpu, &waker, &output);
    drop(host);
    drop(gpu);
    drop(monado);
    remove(&directory);
}

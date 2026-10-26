use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use beui_adapter_space::Space;
use beui_core::context::Context;
use beui_core::geometry::vec2;
use beui_core::runner::{HostView, WindowHost};

use crate::{Xr, XrSpace};

const FRAMES: u64 = 30;

#[test]
fn monado_shows_the_frames_space_draws_for_each_eye() {
    let runtime = std::env::temp_dir().join(format!("beui-xr-{}", std::process::id()));
    std::fs::create_dir_all(&runtime).expect("a runtime directory");
    let log = runtime.join("monado.log");
    let mut service = Command::new(std::env::var_os("MONADO_SERVICE").expect("MONADO_SERVICE"))
        .env("XRT_COMPOSITOR_NULL", "1")
        .env("XDG_RUNTIME_DIR", &runtime)
        .stdin(Stdio::piped())
        .stdout(std::fs::File::create(&log).expect("a log"))
        .stderr(std::fs::File::create(runtime.join("monado.err")).expect("a log"))
        .spawn()
        .expect("monado-service starts");
    let socket = runtime.join("monado_comp_ipc");
    let listening = Instant::now() + Duration::from_secs(30);
    while !socket.exists() {
        assert!(Instant::now() < listening, "monado-service never listened");
        assert!(
            service.try_wait().expect("monado-service runs").is_none(),
            "monado-service stopped: {}{}",
            std::fs::read_to_string(&log).unwrap_or_default(),
            std::fs::read_to_string(runtime.join("monado.err")).unwrap_or_default()
        );
        std::thread::yield_now();
    }
    // unsafe(env)
    unsafe { std::env::set_var("XDG_RUNTIME_DIR", &runtime) };

    let xr = Xr::open("beui-xr test").expect("Monado opens");
    let gpu = pollster::block_on(beui_renderer_wgpu::present::create_offscreen(
        beui_renderer_wgpu::offscreen::FORMAT,
        xr.open_device(&[]),
    ))
    .expect("OpenXR's Vulkan device opens");
    let mut setup = beui_core::app::Setup::new(beui_core::app::Waker::new(|| {}));
    setup.provide(gpu.setup());
    let mut host = XrSpace::new(Space::new(), xr.headset());
    host.setup(&setup);
    let fonts = beui_font_freetype::FreetypeFonts::new(beui_font_freetype::FontLibrary::new(
        beui_font_freetype::FontSources::default(),
    ));
    let output = Context::new(fonts).run(|_| {});
    let mut last = None;
    for _ in 0..10_000 {
        if host.headset().shown() >= FRAMES {
            break;
        }
        let paced = host.pace();
        if let (Some(now), Some(last)) = (paced, last) {
            assert!(now > last, "each frame is shown later than the one before");
        }
        last = paced.or(last);
        let view = HostView {
            platform: vec2(400.0, 300.0),
            pixels_per_point: 1.0,
            now: paced.unwrap_or_else(Instant::now),
        };
        let mut incoming = Vec::new();
        host.input(&mut incoming, &view);
        if let Some(tracked) = host.space.tracked() {
            assert_eq!(
                tracked.views.len(),
                2,
                "space tracks both of the headset's eyes"
            );
            assert_eq!(
                Some(tracked.display_time),
                paced,
                "and draws them for the paced time"
            );
        }
        host.frame(&output, &view);
        assert!(!host.headset().lost(), "the session goes on");
    }
    assert!(
        host.headset().shown() >= FRAMES,
        "Monado showed {} frames",
        host.headset().shown()
    );
    drop(host);
    drop(gpu);
    let _ = service.kill();
    let _ = service.wait();
    let _ = std::fs::remove_dir_all(&runtime);
}

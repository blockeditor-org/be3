use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use beui_core::app::{Setup, Waker};
use beui_core::context::{Context, FrameOutput};
use beui_core::geometry::vec2;
use beui_runner::LayerView;
use beui_renderer_wgpu::present::Gpu;

use crate::XrSpace;

mod a_headset_found_after_the_start_is_shown_and_waited_for_again_when_its_runtime_stops;
mod monado_shows_the_frames_space_draws_for_each_eye;

const FRAMES: u64 = 30;

static MONADO: Mutex<()> = Mutex::new(());

struct Monado {
    service: Child,
}

impl Monado {
    fn directory(name: &str) -> (MutexGuard<'static, ()>, PathBuf) {
        let held = MONADO
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let directory =
            std::env::temp_dir().join(format!("beui-layer-xr-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("a runtime directory");
        // unsafe(env)
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", &directory) };
        (held, directory)
    }

    fn start(directory: &Path) -> Self {
        let log = directory.join("monado.log");
        let errors = directory.join("monado.err");
        let socket = directory.join("monado_comp_ipc");
        let _ = std::fs::remove_file(&socket);
        let mut service = Command::new(std::env::var_os("MONADO_SERVICE").expect("MONADO_SERVICE"))
            .env("XRT_COMPOSITOR_NULL", "1")
            .env("XDG_RUNTIME_DIR", directory)
            .stdin(Stdio::piped())
            .stdout(std::fs::File::create(&log).expect("a log"))
            .stderr(std::fs::File::create(&errors).expect("a log"))
            .spawn()
            .expect("monado-service starts");
        let listening = Instant::now() + Duration::from_secs(30);
        while !socket.exists() {
            assert!(Instant::now() < listening, "monado-service never listened");
            assert!(
                service.try_wait().expect("monado-service runs").is_none(),
                "monado-service stopped: {}{}",
                std::fs::read_to_string(&log).unwrap_or_default(),
                std::fs::read_to_string(&errors).unwrap_or_default()
            );
            std::thread::yield_now();
        }
        Self { service }
    }
}

fn remove(directory: &Path) {
    let _ = std::fs::remove_dir_all(directory);
}

impl Drop for Monado {
    fn drop(&mut self) {
        let _ = self.service.kill();
        let _ = self.service.wait();
    }
}

fn setup(host: &mut XrSpace, gpu: &Gpu, waker: Waker) {
    let mut setup = Setup::new(waker);
    setup.provide(gpu.setup());
    host.setup(&setup);
}

fn app() -> FrameOutput {
    let fonts = beui_font_freetype::FreetypeFonts::new(beui_font_freetype::FontLibrary::new(
        beui_font_freetype::FontSources::default(),
    ));
    let context = Context::new(fonts);
    context.run(|_| {})
}

fn step(host: &mut XrSpace, output: &FrameOutput, last: &mut Option<Instant>) {
    let paced = host.pace();
    if let (Some(now), Some(last)) = (paced, *last) {
        assert!(now > last, "each frame is shown later than the one before");
    }
    *last = paced.or(*last);
    let view = LayerView {
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
    host.frame(output, &view);
}

fn shown(host: &XrSpace) -> u64 {
    host.headset().map_or(0, crate::Headset::shown)
}

fn show_frames(host: &mut XrSpace, output: &FrameOutput) -> usize {
    let mut last = None;
    let mut layered = 0;
    let from = shown(host);
    for _ in 0..10_000 {
        if shown(host) >= from + FRAMES {
            break;
        }
        step(host, output, &mut last);
        let headset = host.headset().expect("the headset stays");
        assert_eq!(headset.lost(), None, "the session goes on");
        layered = layered.max(headset.layered());
    }
    assert!(
        shown(host) >= from + FRAMES,
        "Monado showed {} frames",
        shown(host) - from
    );
    layered
}

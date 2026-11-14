use std::os::fd::AsRawFd;
use std::os::unix::net::UnixListener;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use beui_core::context::{Context, FrameOutput};
use beui_core::program::{Setup, Waker};
use beui_renderer_wgpu::present::Gpu;
use beui_runner::LayerView;
use beui_types::vec2;

use crate::XrSpace;

mod a_headset_found_after_the_start_is_shown_and_waited_for_again_when_its_runtime_stops;
mod monado_shows_the_frames_space_draws_for_each_eye;

const FRAMES: u64 = 30;

/// The tests share one Monado service directory (`XDG_RUNTIME_DIR`), so each holds this lock.
static MONADO: Mutex<()> = Mutex::new(());

struct Monado {
    service: Child,
    socket: PathBuf,
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

    /// Starts monado-service by socket activation, as systemd's `monado.socket` does: the
    /// socket listens before the service starts, so a client that connects at any point after
    /// this returns is queued until the service accepts it. The service binding its own socket
    /// would leave a moment, between its `bind` and `listen`, in which the file is there but a
    /// client is refused.
    fn start(directory: &Path) -> Self {
        let socket = directory.join("monado_comp_ipc");
        let _ = std::fs::remove_file(&socket);
        let listening = UnixListener::bind(&socket).expect("Monado's socket listens");
        let passed = listening.as_raw_fd();
        let mut command = Command::new("sh");
        // `sd_listen_fds` takes the socket only if `LISTEN_PID` is the service's own pid, which
        // the shell knows and keeps when it `exec`s the service.
        command
            .args(["-c", "export LISTEN_PID=$$; exec \"$0\""])
            .arg(std::env::var_os("MONADO_SERVICE").expect("MONADO_SERVICE"))
            .env("LISTEN_FDS", "1")
            .env("XRT_COMPOSITOR_NULL", "1")
            .env("XDG_RUNTIME_DIR", directory)
            .stdin(Stdio::null())
            .stdout(std::fs::File::create(directory.join("monado.log")).expect("a log"))
            .stderr(std::fs::File::create(directory.join("monado.err")).expect("a log"));
        // unsafe(ffi)
        unsafe {
            // The first socket passed is fd 3 (`SD_LISTEN_FDS_START`). `dup2` clears
            // close-on-exec on the copy, but does nothing if the socket is fd 3 already.
            command.pre_exec(move || {
                let moved = if passed == 3 {
                    libc::fcntl(3, libc::F_SETFD, 0)
                } else {
                    libc::dup2(passed, 3)
                };
                match moved {
                    -1 => Err(std::io::Error::last_os_error()),
                    _ => Ok(()),
                }
            });
        }
        let service = command.spawn().expect("monado-service starts");
        // Only the service holds the socket now, so if it ends, a client is refused rather
        // than queued for a service that will never accept it.
        drop(listening);
        Self { service, socket }
    }
}

/// What the service printed, for a failure that may be its.
fn log(directory: &Path) -> String {
    ["monado.log", "monado.err"]
        .map(|name| std::fs::read_to_string(directory.join(name)).unwrap_or_default())
        .concat()
}

fn remove(directory: &Path) {
    let _ = std::fs::remove_dir_all(directory);
}

impl Drop for Monado {
    /// Stops the service and its socket with it, so a client finds no runtime, as when systemd
    /// stops both.
    fn drop(&mut self) {
        let _ = self.service.kill();
        let _ = self.service.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

fn setup(host: &mut XrSpace, gpu: &Gpu, waker: Waker) {
    let mut setup = Setup::new(waker);
    setup.provide(gpu.setup());
    host.setup(&setup);
}

fn program() -> FrameOutput {
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

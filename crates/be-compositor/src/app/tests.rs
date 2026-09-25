use super::*;

mod a_dmabuf_window_samples_the_clients_pixels;
mod a_new_gpu_redraws_windows_from_the_buffers_they_still_hold;
mod a_new_window_floats_at_the_size_it_drew;
mod closing_a_window_removes_its_tab;
mod keys_follow_the_focus_between_beui_and_a_window;

use beui::{FrameOutput, RawInput, Vec2, pos2};

use crate::clients::tab_of;
use crate::test_client::{TestClient, TestWindow};

const SCREEN: Vec2 = Vec2::new(1000.0, 700.0);

struct Harness {
    app: Compositor,
    context: Context,
    client: TestClient,
    output: Option<FrameOutput>,
}

impl Harness {
    fn new() -> Self {
        let mut app = Compositor::new(
            Server::headless().expect("a headless display starts"),
            Vec::new(),
        );
        let client = TestClient::connect(app.server());
        let context = Context::new();
        context.set_test_ids_published(true);
        Self {
            app,
            context,
            client,
            output: None,
        }
    }

    fn with_gpu() -> (Self, wgpu::Device, wgpu::Queue) {
        let (device, queue) = crate::test_client::vulkan_device();
        let mut app = Compositor::new(
            Server::headless().expect("a headless display starts"),
            Vec::new(),
        );
        app.start(
            device.clone(),
            queue.clone(),
            wgpu::TextureFormat::Bgra8Unorm,
            Waker::new(|| {}),
        );
        let client = TestClient::connect(app.server());
        let context = Context::new();
        context.set_test_ids_published(true);
        let harness = Self {
            app,
            context,
            client,
            output: None,
        };
        (harness, device, queue)
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.client.flush();
        let app = &mut self.app;
        let output = self.context.run(RawInput { events }, |context| {
            app.update(context, Rect::from_min_size(Pos2::ZERO, SCREEN));
        });
        self.output = Some(output);
        self.client.receive();
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.frame(Vec::new());
        }
    }

    fn open(&mut self) -> (TestWindow, WindowId) {
        let window = self.client.toplevel();
        self.settle();
        let id = *self
            .app
            .server()
            .state
            .windows()
            .last()
            .expect("the toplevel opened a window");
        let serial = self
            .client
            .received
            .configured
            .take()
            .expect("the window was configured");
        window.xdg_surface.ack_configure(serial);
        let (width, height) = match self.client.received.size {
            Some((width, height)) if width > 0 && height > 0 => (width, height),
            _ => (300, 200),
        };
        self.client.attach_unsent(&window, width, height);
        self.settle();
        (window, id)
    }

    fn rect(&self, test_id: &str) -> Rect {
        self.output
            .as_ref()
            .and_then(|output| output.test_id_rect(test_id))
            .unwrap_or_else(|| panic!("{test_id} was laid out"))
    }

    fn click(&mut self, test_id: &str) {
        let center = self.rect(test_id).center();
        self.frame(vec![Event::PointerMoved(center)]);
        self.frame(vec![Event::PointerButton {
            pos: center,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: beui::Modifiers::NONE,
        }]);
        self.frame(vec![Event::PointerButton {
            pos: center,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: beui::Modifiers::NONE,
        }]);
        self.settle();
    }

    fn key(&mut self, code: u32) {
        self.frame(vec![
            Event::PhysicalKey {
                code,
                pressed: true,
            },
            Event::PhysicalKey {
                code,
                pressed: false,
            },
        ]);
        self.settle();
    }
}

fn outside() -> Pos2 {
    pos2(-10.0, -10.0)
}

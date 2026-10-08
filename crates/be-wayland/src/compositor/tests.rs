use super::*;

mod a_closed_window_leaves_the_list;
mod a_dmabuf_window_samples_the_clients_pixels;
mod a_drawn_window_is_listed_and_fitted_to_where_it_is_shown;
mod a_shown_dmabuf_is_released_once_a_newer_one_is_painted;
mod a_maximized_window_keeps_its_place_and_is_told_it_is_maximized;
mod a_window_asking_for_fullscreen_covers_the_screen;
mod keys_follow_the_focus_between_beui_and_a_window;
mod leaving_fullscreen_returns_the_window_to_where_it_was_shown;
mod super_f_toggles_fullscreen_on_the_focused_window;

use beui::reactive::{ForEach, Frame, Layers, List, build, create_signal, view};
use beui::styled::TextInput;
use beui::{FrameOutput, NodeId, RawInput, pos2};

use crate::test_client::{TestClient, TestWindow};
use crate::view::{FullscreenWindow, WindowView};

const SCREEN: Vec2 = Vec2::new(1000.0, 700.0);
const SHOWN: Vec2 = Vec2::new(400.0, 300.0);

struct Harness {
    app: Compositor,
    document: Document,
    context: Context,
    client: TestClient,
    output: Option<FrameOutput>,
}

fn shown(windows: Windows) -> impl FnOnce() -> NodeId {
    move || {
        let (text, set_text) = create_signal(String::new());
        let ids = beui::reactive::create_memo(clone_list(&windows));
        let covering = windows.clone();
        view! {
            <Layers>
            <List spacing=0.0>
                <TextInput
                    @test_id={"test.input"}
                    value={text}
                    on_change={move |line: String| set_text.set(line)}
                />
                <ForEach keys={ids}>
                    {move |id: WindowId| {
                        let windows = windows.clone();
                        view! {
                            <Frame width={SHOWN.x} height={SHOWN.y}>
                                <WindowView windows id />
                            </Frame>
                        }
                    }}
                </ForEach>
            </List>
            <FullscreenWindow windows={covering} />
            </Layers>
        }
    }
}

fn clone_list(windows: &Windows) -> impl Fn() -> Vec<WindowId> + use<> {
    let list = windows.list();
    move || list.get().iter().map(|info| info.id).collect()
}

impl Harness {
    fn new() -> Self {
        Self::started(None)
    }

    fn with_gpu() -> (Self, wgpu::Device, wgpu::Queue) {
        let (device, queue) = crate::test_client::vulkan_device();
        let harness = Self::started(Some((device.clone(), queue.clone())));
        (harness, device, queue)
    }

    fn started(gpu: Option<(wgpu::Device, wgpu::Queue)>) -> Self {
        let mut windows = None;
        let document = build(|| {
            let store = Windows::new();
            windows = Some(store.clone());
            shown(store)()
        });
        let mut app = Compositor::new(
            Server::headless().expect("a headless display starts"),
            windows.expect("the windows are made while the document is built"),
        );
        if let Some((device, queue)) = gpu {
            app.start(
                device,
                queue,
                wgpu::TextureFormat::Bgra8Unorm,
                Waker::new(|| {}),
            );
        }
        let client = TestClient::connect(app.server());
        let context = beui::context();
        context.set_test_ids_published(true);
        Self {
            app,
            document,
            context,
            client,
            output: None,
        }
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.client.flush();
        let app = &mut self.app;
        let document = &mut self.document;
        let output = self.context.run(RawInput { events }, |context| {
            let rect = Rect::from_min_size(Pos2::ZERO, SCREEN);
            app.before(context, rect, document);
            document.show(context, rect);
            app.after(context, document);
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

    fn window(&self, id: WindowId) -> Rect {
        self.rect(&format!("wayland.window.{}", id.0))
    }

    fn acknowledge(&mut self, window: &TestWindow) {
        if let Some(serial) = self.client.received.configured.take() {
            window.xdg_surface.ack_configure(serial);
        }
        self.settle();
    }

    fn rect(&self, test_id: &str) -> Rect {
        self.output
            .as_ref()
            .and_then(|output| output.test_id_rect(test_id))
            .unwrap_or_else(|| panic!("{test_id} was laid out"))
    }

    fn click(&mut self, test_id: &str) {
        let center = self.rect(test_id).center();
        self.frame(vec![
            Event::PointerMoved(center),
            Event::PointerButton {
                pos: center,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: beui::Modifiers::NONE,
            },
            Event::PointerButton {
                pos: center,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: beui::Modifiers::NONE,
            },
        ]);
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

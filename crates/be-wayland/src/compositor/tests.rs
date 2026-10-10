use super::*;

mod a_click_on_beui_dismisses_a_grabbed_popup;
mod a_closed_window_leaves_the_list;
mod a_dmabuf_window_samples_the_clients_pixels;
mod a_drag_begun_on_the_ui_over_a_window_never_reaches_the_window;
mod a_drawn_window_is_listed_and_fitted_to_where_it_is_shown;
mod a_fullscreen_window_covers_only_its_own_screen;
mod a_global_action_that_does_not_intercept_leaves_the_window_its_keys;
mod a_locked_session_gives_no_window_the_keyboard_or_the_pointer;
mod a_maximized_window_keeps_its_place_and_is_told_it_is_maximized;
mod a_press_the_ui_claims_with_super_never_reaches_the_window;
mod a_process_the_compositor_launched_can_be_ended;
mod a_program_that_is_not_found_is_reported;
mod a_shown_dmabuf_is_released_once_a_newer_one_is_painted;
mod a_shown_window_that_inhibits_idle_keeps_the_session_awake;
mod a_wake_from_the_host_counts_as_activity;
mod a_window_asking_for_fullscreen_before_it_is_shown_is_answered;
mod a_window_asking_for_fullscreen_covers_the_screen;
mod a_window_that_stops_answering_pings_is_not_responding_until_it_answers;
mod an_inhibitor_on_a_window_that_is_not_shown_does_not_keep_the_session_awake;
mod closing_a_window_that_is_not_responding_disconnects_its_client;
mod idle_notifications_tell_clients_when_the_user_idles_and_resumes;
mod keys_follow_the_focus_between_beui_and_a_window;
mod leaving_fullscreen_returns_the_window_to_where_it_was_shown;
mod only_an_input_idle_notification_ignores_inhibitors;
mod super_f_toggles_fullscreen_on_the_focused_window;
mod the_f_of_super_f_released_elsewhere_is_not_held_against_the_next;
mod the_lock_time_runs_on_its_own_and_ignores_inhibitors_once_locked;
mod the_session_idles_after_the_blank_time_and_wakes_on_input;
mod the_ui_can_make_a_window_fullscreen_and_take_it_back;

use beui::reactive::{
    ForEach, Frame, Interactive, List, Overlay, OverlayAnchor, Placement, WriteSignal, build,
    clone, component, create_memo, create_signal, view, with_reactive_scope,
};
use beui::styled::TextInput;
use beui::{FrameOutput, Key, NodeId, RawInput, pos2};

use crate::test_client::{TestClient, TestWindow};
use crate::view::WindowView;

const SCREEN: Vec2 = Vec2::new(1000.0, 700.0);
const SHOWN: Vec2 = Vec2::new(400.0, 300.0);

struct Harness {
    app: Compositor,
    document: Document,
    context: Context,
    client: TestClient,
    output: Option<FrameOutput>,
}

#[component]
fn Shown(windows: Windows) -> NodeId {
    {
        let toggling = windows.clone();
        beui::reactive::Action::new("test.fullscreen", "Toggle fullscreen", move || {
            let Some(id) = toggling.focused() else {
                return;
            };
            let fullscreen = toggling
                .list()
                .get_untracked()
                .iter()
                .any(|info| info.id == id && info.fullscreen.is_some());
            toggling.request_fullscreen(id, !fullscreen);
            beui::reactive::try_with_document(|document| {
                document.request_repaint_after(std::time::Duration::ZERO);
            });
        })
        .shortcut(beui::reactive::Chord::logo(Key::F))
        .intercepts()
        .register();
        beui::reactive::Action::new("test.global", "Global", || {
            GLOBAL_RAN.with(|ran| ran.set(ran.get() + 1))
        })
        .shortcut(beui::reactive::Chord::logo(Key::G))
        .global()
        .register();
        let (text, set_text) = create_signal(String::new());
        let (locked, set_locked) = create_signal(false);
        LOCK.with(|lock| *lock.borrow_mut() = Some(set_locked));
        let ids = create_memo(clone_list(&windows));
        view! {
            <Interactive
                claim_modifiers={Some(beui::Modifiers::LOGO)}
                claims_touch=false
                on_press={|press: beui::PointerPress| {
                    if press.modifiers.logo {
                        CLAIMED.with(|claimed| claimed.set(claimed.get() + 1));
                    }
                }}
            >
                <List spacing=0.0>
                    <TextInput
                        @test_id={"test.input"}
                        value={text}
                        on_change={move |line: String| set_text.set(line)}
                    />
                    <ForEach keys={ids}>
                        {move |id: WindowId| {
                            let windows = windows.clone();
                            let list = windows.list();
                            let size = create_memo(move || {
                                list.with(|list| {
                                    list.iter()
                                        .find(|info| info.id == id)
                                        .and_then(|info| info.fullscreen)
                                        .map_or(SHOWN, |area| area.size())
                                })
                            });
                            let width = create_memo(clone!(size -> move || Some(size.get().x)));
                            let height = create_memo(move || Some(size.get().y));
                            view! {
                                <Frame width height>
                                    <WindowView windows id />
                                </Frame>
                            }
                        }}
                    </ForEach>
                    <Overlay
                        anchor=OverlayAnchor::Point(Pos2::ZERO)
                        placement=Placement::Fill
                        locks=true
                        open={locked}
                    >
                        <TextInput @test_id={"test.lock"} value="" />
                    </Overlay>
                </List>
            </Interactive>
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
        Self::showing(gpu, |windows| {
            view! {
                <Shown windows />
            }
        })
    }

    fn showing(
        gpu: Option<(wgpu::Device, wgpu::Queue)>,
        view: impl FnOnce(Windows) -> NodeId,
    ) -> Self {
        let mut windows = None;
        let document = build(|| {
            let store = Windows::new();
            windows = Some(store.clone());
            view(store)
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

    fn lock(&mut self, locked: bool) {
        let set = LOCK
            .with(|lock| lock.borrow().clone())
            .expect("the harness has a lock");
        with_reactive_scope(&mut self.document, move || set.set(locked));
        self.app.set_locked(locked);
        self.settle();
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

    fn responding(&self, id: WindowId) -> bool {
        self.app
            .windows()
            .list()
            .get_untracked()
            .iter()
            .find(|info| info.id == id)
            .expect("the window is listed")
            .responding
    }
}

const KEY_F: u32 = 33;
const KEY_G: u32 = 34;
const KEY_LEFTMETA: u32 = 125;

thread_local! {
    static LOCK: std::cell::RefCell<Option<WriteSignal<bool>>> = const { std::cell::RefCell::new(None) };
    static GLOBAL_RAN: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static CLAIMED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn physical(code: u32, pressed: bool) -> Event {
    Event::PhysicalKey { code, pressed }
}

fn super_key(pressed: bool) -> Vec<Event> {
    let held = match pressed {
        true => beui::Modifiers::LOGO,
        false => beui::Modifiers::NONE,
    };
    vec![physical(KEY_LEFTMETA, pressed), Event::Modifiers(held)]
}

fn f_key(pressed: bool, modifiers: beui::Modifiers) -> Vec<Event> {
    vec![
        physical(KEY_F, pressed),
        Event::Key {
            key: Key::F,
            pressed,
            repeat: false,
            modifiers,
        },
    ]
}

fn outside() -> Pos2 {
    pos2(-10.0, -10.0)
}

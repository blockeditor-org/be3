use super::*;
use beui::KeyAt;

mod a_button_beyond_the_five_reaches_the_window_with_its_own_code;
mod a_click_on_beui_dismisses_a_grabbed_popup;
mod a_closed_window_leaves_the_list;
mod a_cursor_shape_a_client_names_is_shown_over_its_window;
mod a_dialog_that_closes_gives_the_keyboard_back_to_the_window_that_had_it;
mod a_dmabuf_window_samples_the_clients_pixels;
mod a_drag_begun_on_the_ui_over_a_window_never_reaches_the_window;
mod a_drag_does_not_drop_onto_a_window_a_modal_dialog_blocks;
mod a_drag_reaches_a_popup_past_its_windows_edge;
mod a_drawn_window_is_listed_and_fitted_to_where_it_is_shown;
mod a_frame_callback_waits_for_the_flip_of_the_screen_the_window_is_on;
mod a_fullscreen_window_covers_only_its_own_screen;
mod a_global_action_that_does_not_intercept_leaves_the_window_its_keys;
mod a_launched_program_is_given_a_token_that_brings_a_window_forward;
mod a_locked_session_gives_no_window_the_keyboard_or_the_pointer;
mod a_maximized_window_keeps_its_place_and_is_told_it_is_maximized;
mod a_modal_dialog_keeps_presses_and_keys_from_its_parent;
mod a_pen_over_a_window_reaches_it_as_a_tablet_tool_with_its_pressure;
mod a_pen_over_a_window_that_takes_no_tablets_moves_and_presses_its_pointer;
mod a_popup_reaching_past_its_window_keeps_its_size_and_is_shown_outside_it;
mod a_press_between_two_popups_reaches_what_is_under_them;
mod a_press_held_when_a_dialog_turns_modal_is_still_released;
mod a_press_on_a_window_reports_it_raised_even_when_it_has_the_keyboard;
mod a_press_the_ui_claims_with_super_never_reaches_the_window;
mod a_process_the_compositor_launched_can_be_ended;
mod a_program_that_is_not_found_is_reported;
mod a_repositioned_popup_is_configured_again;
mod a_resized_window_is_drawn_unscaled_until_it_commits_the_new_size;
mod a_shown_dmabuf_is_released_once_a_newer_one_is_painted;
mod a_shown_window_that_inhibits_idle_keeps_the_session_awake;
mod a_single_pixel_buffer_fills_its_window_with_one_color;
mod a_stale_or_foreign_token_does_not_take_the_focus;
mod a_token_from_a_click_in_the_focused_window_brings_another_window_forward;
mod a_token_is_lost_once_the_keyboard_leaves_its_window;
mod a_wake_from_the_host_counts_as_activity;
mod a_window_asking_for_fullscreen_before_it_is_shown_is_answered;
mod a_window_asking_for_fullscreen_covers_the_screen;
mod a_window_brought_forward_while_it_is_not_shown_is_reported_raised;
mod a_window_dragged_from_drops_onto_the_window_under_the_pointer;
mod a_window_no_shell_owns_gets_no_input_and_is_still_pinged;
mod a_window_asked_fullscreen_on_an_output_fills_that_monitor;
mod a_window_on_no_screen_has_its_presentation_discarded;
mod each_monitor_is_an_output_and_a_window_enters_the_one_it_is_on;
mod a_window_that_copies_keeps_its_own_selection;
mod a_window_that_moves_without_resizing_is_not_configured_again;
mod a_window_that_picks_its_own_size_is_shown_at_it_and_never_told_another;
mod a_window_that_stops_answering_pings_is_not_responding_until_it_answers;
mod a_window_whose_first_focus_was_refused_gets_the_keyboard_when_raised;
mod a_window_whose_text_field_is_enabled_asks_for_the_keyboard_and_takes_typed_text;
mod a_windows_surfaces_are_drawn_back_to_front;
mod activating_a_window_with_a_modal_dialog_raises_the_dialog;
mod an_app_that_inhibits_shortcuts_gets_super_chords_until_super_escape;
mod an_inhibitor_on_a_window_that_is_not_shown_does_not_keep_the_session_awake;
mod closing_a_window_that_is_not_responding_disconnects_its_client;
mod hiding_the_tab_of_the_window_with_the_keyboard_takes_the_keyboard_from_it;
mod idle_notifications_tell_clients_when_the_user_idles_and_resumes;
mod keys_follow_the_focus_between_beui_and_a_window;
mod leaving_fullscreen_returns_the_window_to_where_it_was_shown;
mod only_an_input_idle_notification_ignores_inhibitors;
mod presentation_feedback_carries_the_flip_time;
mod pressing_a_windows_bar_gives_it_the_keyboard;
mod raising_a_window_that_does_not_redraw_shows_it_over_the_one_it_covered;
mod super_f_toggles_fullscreen_on_the_focused_window;
mod text_a_program_copies_reaches_the_host;
mod text_the_host_copies_is_offered_to_programs;
mod the_f_of_super_f_released_elsewhere_is_not_held_against_the_next;
mod the_lock_time_runs_on_its_own_and_ignores_inhibitors_once_locked;
mod the_pointer_shows_the_cursor_surface_a_client_sets_over_its_window;
mod the_session_idles_after_the_blank_time_and_wakes_on_input;
mod the_ui_can_make_a_window_fullscreen_and_take_it_back;
mod with_middle_click_paste_off_a_selection_is_cancelled_and_offered_to_no_one;
mod with_middle_click_paste_on_a_selection_is_offered_until_it_is_switched_off;

use beui::automation::WindowSize;
use beui::styled::*;
use beui::testing::Stepped;
use beui::{FrameOutput, Input, Key, Keystroke, NodeId, pos2};

use crate::test_client::{TestClient, TestWindow};
use crate::view::WindowView;

const SCREEN: Vec2 = Vec2::new(1000.0, 700.0);
const SHOWN: Vec2 = Vec2::new(400.0, 300.0);

struct Shell {
    compositor: Compositor,
    document: Document,
    copy: Option<String>,
}

impl beui::App for Shell {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.compositor.before(context, rect);
        self.document.show(context, rect);
        if let Some(text) = self.copy.take() {
            context.copy_text(text);
        }
        self.compositor.after(context, &mut self.document);
    }
}

struct Harness {
    stepped: Stepped<Shell>,
    client: TestClient,
    output: Option<FrameOutput>,
}

#[component]
fn Shown(windows: Windows) -> NodeId {
    {
        beui::reactive::Action::new("test.fullscreen", "Toggle fullscreen", move || {
            let Some(id) = windows.focused() else {
                return;
            };
            let fullscreen = windows
                .list()
                .get_untracked()
                .iter()
                .any(|info| info.id == id && info.fullscreen.is_some());
            windows.request_fullscreen(id, !fullscreen);
            beui::reactive::try_with_document(|document| {
                document.request_repaint_after(std::time::Duration::ZERO);
            });
        })
        .shortcut(beui::reactive::Chord::logo(Key::F))
        .at(KeyAt::Window)
        .capture()
        .register();
        beui::reactive::Action::new("test.global", "Global", || {
            GLOBAL_RAN.with(|ran| ran.set(ran.get() + 1))
        })
        .shortcut(beui::reactive::Chord::logo(Key::G))
        .at(KeyAt::Window)
        .register();
        crate::view::register_restore_shortcuts(windows);
        let text = create_signal(String::new());
        let locked = create_signal(false);
        LOCK.with(|lock| *lock.borrow_mut() = Some(locked));
        let asking = create_signal(false);
        DIALOG.with(|dialog| *dialog.borrow_mut() = Some(asking));
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
                    <TextInput @test_id={"test.input"} value={text} />
                    <ForEach keys={ids}>|id: WindowId| {
                        let list = windows.list();
                        let size = create_memo(move || {
                            list.with(|list| {
                                list.iter()
                                    .find(|info| info.id == id)
                                    .and_then(|info| info.fullscreen)
                                    .map_or(SHOWN, |area| area.size())
                            })
                        });
                        let width = create_memo(move || Some(size.get().x));
                        let height = create_memo(move || Some(size.get().y));
                        view! {
                            <Frame width=width height=height>
                                <WindowView windows id />
                            </Frame>
                        }
                    }</ForEach>
                    <Overlay
                        anchor=OverlayAnchor::Point(Pos2::ZERO)
                        placement=Placement::Fill
                        level=LayerLevel::Lock
                        open={locked}
                    >
                        <TextInput @test_id={"test.lock"} value="" />
                    </Overlay>
                    <Overlay
                        anchor=OverlayAnchor::Point(Pos2::ZERO)
                        placement=Placement::Center
                        open={asking}
                    >
                        <TextInput @test_id={"test.dialog"} value="" focused={asking} />
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
            windows = Some(store);
            let content = view(store);
            view! {
                <Root>{content}</Root>
            }
        });
        let mut compositor = Compositor::new(
            Server::headless().expect("a headless display starts"),
            windows.expect("the windows are made while the document is built"),
        );
        if let Some((device, queue)) = gpu {
            compositor.start(
                device,
                queue,
                wgpu::TextureFormat::Bgra8Unorm,
                Waker::new(|| {}),
            );
        }
        let client = TestClient::connect(compositor.server());
        let shell = Shell {
            compositor,
            document,
            copy: None,
        };
        let window = WindowSize {
            size: SCREEN,
            scale: 1.0,
        };
        Self {
            stepped: Stepped::new(beui::context(), shell, window, None),
            client,
            output: None,
        }
    }

    fn compositor(&mut self) -> &mut Compositor {
        &mut self.stepped.app_mut().compositor
    }

    fn context(&self) -> &Context {
        self.stepped.context()
    }

    fn frame(&mut self, inputs: Vec<Input>) {
        self.client.flush();
        self.output = Some(self.stepped.frame(inputs));
        self.client.receive();
    }

    fn frame_at(&mut self, at: std::time::Instant) {
        let next = self.context().now() + beui::testing::FRAME_INTERVAL;
        self.context().advance_clock(at - next);
        self.frame(Vec::new());
    }

    fn lock(&mut self, locked: bool) {
        let set = LOCK
            .with(|lock| *lock.borrow())
            .expect("the harness has a lock");
        set.set(locked);
        self.compositor().set_locked(locked);
        self.settle();
    }

    fn ask(&mut self, open: bool) {
        let set = DIALOG
            .with(|dialog| *dialog.borrow())
            .expect("the harness has a dialog");
        set.set(open);
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
            .compositor()
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
        self.click_at(center);
    }

    fn click_at(&mut self, center: Pos2) {
        self.frame(vec![
            Input::PointerMoved(center),
            Input::PointerButton {
                pos: center,
                button: PointerButton::Primary,
                pressed: true,
            },
            Input::PointerButton {
                pos: center,
                button: PointerButton::Primary,
                pressed: false,
            },
        ]);
        self.settle();
    }

    fn key(&mut self, code: u32) {
        self.frame(vec![physical(code, true), physical(code, false)]);
        self.settle();
    }

    fn responding(&mut self, id: WindowId) -> bool {
        self.compositor()
            .windows()
            .list()
            .get_untracked()
            .iter()
            .find(|info| info.id == id)
            .expect("the window is listed")
            .responding
    }
}

struct Painter {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: beui::Renderer,
    view: wgpu::TextureView,
}

impl Painter {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let format = wgpu::TextureFormat::Bgra8Unorm;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("screen"),
            size: wgpu::Extent3d {
                width: SCREEN.x as u32,
                height: SCREEN.y as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        Self {
            device: device.clone(),
            queue: queue.clone(),
            renderer: beui::Renderer::new(device, format),
            view: target.create_view(&wgpu::TextureViewDescriptor::default()),
        }
    }

    fn paint(&mut self, harness: &Harness) {
        use beui::Repainting as _;
        let output = harness.output.as_ref().expect("a frame ran");
        let repaint = output.main().repaint(beui::Color32::BLACK);
        self.renderer.prepare(
            &self.device,
            &self.queue,
            output.main(),
            SCREEN,
            1.0,
            repaint,
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        self.renderer.render(
            &self.device,
            &self.queue,
            &mut encoder,
            &self.view,
            (SCREEN.x as u32, SCREEN.y as u32),
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        );
        let index = self.queue.submit([encoder.finish()]);
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: None,
            })
            .expect("the frame finishes");
    }
}

fn flip(seconds: u64) -> crate::presentation::Presented {
    crate::presentation::Presented {
        time: std::time::Duration::from_secs(seconds),
        refresh: std::time::Duration::from_nanos(16_666_667),
        sequence: seconds,
        vsync: true,
        hardware_clock: true,
    }
}

const KEY_F: u32 = 33;
const KEY_G: u32 = 34;
const KEY_ESC: u32 = 1;
const KEY_LEFTMETA: u32 = 125;

thread_local! {
    static LOCK: std::cell::RefCell<Option<Signal<bool>>> = const { std::cell::RefCell::new(None) };
    static DIALOG: std::cell::RefCell<Option<Signal<bool>>> = const { std::cell::RefCell::new(None) };
    static GLOBAL_RAN: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static CLAIMED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn stroke(key: Option<Key>, code: u32, pressed: bool) -> Input {
    Input::Key(Keystroke {
        key,
        pressed,
        repeat: false,
        evdev: Some(code),
        text: None,
    })
}

fn physical(code: u32, pressed: bool) -> Input {
    stroke(None, code, pressed)
}

fn super_key(pressed: bool) -> Vec<Input> {
    let held = match pressed {
        true => beui::Modifiers::LOGO,
        false => beui::Modifiers::NONE,
    };
    vec![
        stroke(Some(Key::Logo), KEY_LEFTMETA, pressed),
        Input::Modifiers(held),
    ]
}

fn f_key(pressed: bool) -> Vec<Input> {
    vec![stroke(Some(Key::F), KEY_F, pressed)]
}

fn outside() -> Pos2 {
    pos2(-10.0, -10.0)
}

fn monitors(rects: &[Rect]) -> Vec<Monitor> {
    rects
        .iter()
        .enumerate()
        .map(|(index, rect)| Monitor {
            id: format!("monitor {index}"),
            name: format!("Monitor {index}"),
            rect: *rect,
            scale: 1.0,
        })
        .collect()
}

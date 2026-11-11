use std::cell::{Cell, RefCell};
use std::rc::Rc;

use be_sdk::{EditorInstanceId, FrameSpec, Input, Module};
use beui::Event;
use beui::NodeId;
use beui::native::with_reactive_scope;
use beui::reactive::*;
use beui::styled;
use beui::styled::{Window, WindowHandle};
use beui::{Pos2, pos2};
use beui_core::document::Document;

use std::any::Any;

use super::*;
use beui_host::native::build;

mod a_copy_in_a_view_reaches_the_host;
mod a_field_focused_in_a_views_popover_is_its_anchored_views_focus;
mod a_keyboard_toggle_reaches_the_host_once_from_the_view_it_was_asked_in;
mod a_lazy_representation_renders_only_when_the_host_asks;
mod a_sheet_open_before_its_view_has_an_anchor_space_moves_into_an_anchored_view;
mod a_trackpad_scroll_held_past_the_start_stretches_and_springs_back_when_lifted;
mod a_view_describes_itself_and_does_what_a_screen_reader_asks;
mod a_view_drawn_at_a_finer_raster_paints_its_text_sharper_without_laying_it_out;
mod a_view_given_the_keyboard_that_never_shows_does_not_hold_it_for_ever;
mod a_view_lays_out_over_its_whole_rect_not_just_what_shows;
mod a_view_tells_the_host_it_locks_the_pointer_and_when_it_stops;
mod a_view_that_closes_with_the_keyboard_tells_the_program_and_one_without_does_not;
mod a_view_that_loses_the_keyboard_mid_press_keeps_the_press;
mod a_views_keyboard_reaches_the_host_with_its_asks;
mod a_views_menu_is_an_anchored_view_it_paints_describes_and_takes_input_on;
mod a_window_opened_where_the_host_places_windows_is_the_hosts_own;
mod a_window_the_program_floats_inside_a_view_stays_in_that_view;
mod a_window_the_program_opens_is_painted_in_the_frame_that_opens_it;
mod a_zoomed_out_views_menu_paints_all_of_itself;
mod an_anchored_view_left_empty_keeps_a_finite_place;
mod each_view_reads_its_own_viewport_size;
mod every_view_is_a_window_painted_in_the_frame_that_first_asks_for_it;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);

fn region(size: Vec2, visible: Vec2, pixels: [u32; 2]) -> ShownView {
    ShownView {
        instance: INSTANCE,
        view: be_sdk::View::MAIN,
        block: None,
        rect: Rect::from_min_size(Pos2::ZERO, size),
        visible,
        scale_factor: 1.0,
        raster: 1.0,
        pixels,
        age: 0,
        spec: FrameSpec::default(),
        describe: None,
        seen: true,
        windows: false,
        fit: false,
        docked: false,
        host_place: None,
        origin: None,
        anchor_space: None,
    }
}

fn whole(width: f32, height: f32) -> ShownView {
    region(
        vec2(width, height),
        Vec2::ZERO,
        [width as u32, height as u32],
    )
}

fn anchored_shown(owner: &ShownView, anchored: &be_sdk::Anchored) -> ShownView {
    let rect = anchored.placement.rect;
    ShownView {
        view: be_sdk::View::anchored(anchored.view),
        rect: Rect::from_min_size(Pos2::ZERO, rect.size()),
        visible: Vec2::ZERO,
        pixels: [rect.width().ceil() as u32, rect.height().ceil() as u32],
        anchor_space: owner.anchor_space.map(|space| be_sdk::AnchorSpace {
            to_host: be_sdk::HostTransform {
                origin: rect.min,
                scale: Vec2::splat(1.0),
            },
            bounds: space.bounds,
        }),
        ..owner.clone()
    }
}

fn one_anchored(surface: &mut Driver<ModuleRunner>, region: &ShownView) -> Frame {
    surface
        .frame_opened(std::slice::from_ref(region))
        .pop()
        .expect("one view framed gets one frame")
}

fn one(surface: &mut Driver<ModuleRunner>, region: &ShownView) -> Frame {
    surface
        .frame(std::slice::from_ref(region))
        .pop()
        .expect("one region framed gets one frame")
}

type Content = Rc<dyn Fn(ViewRef) -> NodeId>;
type Hook = Box<dyn FnMut(&Context)>;

struct Windowed {
    content: Content,
    hook: Option<Hook>,
    document: Rc<RefCell<Option<Document>>>,
    views: Option<Views>,
    requested: OwnedSignal<Vec<View>>,
}

impl Windowed {
    fn new(content: impl Fn(ViewRef) -> NodeId + 'static) -> Self {
        Self {
            content: Rc::new(content),
            hook: None,
            document: Rc::default(),
            views: None,
            requested: OwnedSignal::new(Vec::new()),
        }
    }

    fn empty() -> Self {
        Self::new(|_| {
            view! {
                <styled::Frame />
            }
        })
    }

    fn hooked(mut self, hook: impl FnMut(&Context) + 'static) -> Self {
        self.hook = Some(Box::new(hook));
        self
    }

    fn sharing(mut self, document: &Rc<RefCell<Option<Document>>>) -> Self {
        self.document = Rc::clone(document);
        self
    }
}

impl Program for Windowed {
    fn setup(&mut self, setup: &Setup) {
        let views = setup
            .get::<Views>()
            .cloned()
            .expect("the app platform provides its views");
        let requested = self.requested.handle();
        let watched = views.clone();
        views.watch(move || requested.set(watched.views()));
        self.views = Some(views);
    }

    fn update(&mut self, context: &Context, rect: Rect) {
        let views = self.views.clone().expect("set up before the first frame");
        let mut held = self.document.borrow_mut();
        let requested = self.requested.read_only();
        let content = Rc::clone(&self.content);
        let document = held.get_or_insert_with(move || {
            build(move || {
                let keys = create_memo(move || {
                    requested.with(|views| {
                        views
                            .iter()
                            .filter(|view| view.placement != ViewPlacement::Host(HostView::Window))
                            .map(|view| view.id)
                            .collect::<Vec<_>>()
                    })
                });
                view! {
                    <styled::Root>
                        <styled::List spacing=styled::Space::None>
                            <ForEach keys>|id: ViewRef|
                                <Shown
                                    id
                                    requested
                                    views={views.clone()}
                                    content={Rc::clone(&content)}
                                />
                            </ForEach>
                        </styled::List>
                    </styled::Root>
                }
            })
        });
        with_reactive_scope(document, || {});
        document.show(context, rect);
        drop(held);
        if let Some(hook) = &mut self.hook {
            hook(context);
        }
    }
}

#[component]
fn Shown(id: ViewRef, requested: ReadSignal<Vec<View>>, views: Views, content: Content) -> NodeId {
    let size = create_memo(move || {
        requested.with(|views| {
            views
                .iter()
                .find(|view| view.id == id)
                .map_or(Vec2::ZERO, |view| view.size)
        })
    });
    view! {
        <Window size={size} placement={views.placement(id)}>|_window: WindowHandle| {
            content(id)
        }</Window>
    }
}

fn launched(host: &EditorHost, program: impl Program + 'static) -> Box<Driver<ModuleRunner>> {
    let module = Module::default();
    host.join_module(&module);
    launch(
        host,
        RunOptions::default(),
        beui_host::native::context(),
        Box::new(program),
    )
    .expect("an app's program starts");
    let handler: Box<dyn Any> = module
        .take_handler()
        .expect("the platform registers its handler with the module");
    handler
        .downcast()
        .expect("the app platform's handler drives a ModuleRunner")
}

fn preview() -> ShownView {
    ShownView {
        view: be_sdk::View::PREVIEW,
        ..whole(80.0, 60.0)
    }
}

fn focus_events(events: &Rc<std::cell::RefCell<Vec<Event>>>) -> Vec<bool> {
    events
        .borrow()
        .iter()
        .filter_map(|event| match event {
            Event::Focus(focused) => Some(*focused),
            _ => None,
        })
        .collect()
}

fn watched() -> (Windowed, Rc<std::cell::RefCell<Vec<Event>>>) {
    let events = Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = events.clone();
    let program = Windowed::empty().hooked(move |context| {
        context.input(|input| seen.borrow_mut().extend(input.events.iter().cloned()));
    });
    (program, events)
}

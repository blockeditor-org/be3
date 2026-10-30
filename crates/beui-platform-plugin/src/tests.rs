use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui::native::with_reactive_scope;
use beui::reactive::*;
use beui::styled;
use beui::styled::{Window, WindowHandle};
use beui_core::document::Document;
use beui_core::geometry::{Pos2, pos2};
use beui_core::input::Event;
use beui_core::node::NodeId;
use block_editor_plugin::{EditorInstanceId, FrameSpec, Input, Module};

use super::*;
use beui_host::native::build;

mod a_copy_in_a_region_reaches_the_host;
mod a_region_describes_itself_and_does_what_a_screen_reader_asks;
mod a_region_drawn_at_a_finer_raster_paints_its_text_sharper_without_laying_it_out;
mod a_region_lays_out_over_its_whole_rect_not_just_what_shows;
mod a_region_tells_the_host_it_locks_the_pointer_and_when_it_stops;
mod a_region_that_loses_the_keyboard_mid_press_keeps_the_press;
mod a_regions_keyboard_reaches_the_host_with_its_asks;
mod a_regions_menu_is_an_anchored_view_it_paints_describes_and_takes_input_on;
mod a_sheet_open_before_its_region_has_a_layer_space_moves_into_an_anchored_view;
mod a_trackpad_scroll_held_past_the_start_stretches_and_springs_back_when_lifted;
mod a_window_opened_where_the_host_places_windows_is_the_hosts_own;
mod a_window_the_app_floats_inside_a_region_stays_in_that_region;
mod a_window_the_app_opens_is_painted_in_the_frame_that_opens_it;
mod a_zoomed_out_regions_menu_paints_all_of_itself;
mod an_image_copied_in_a_region_reaches_the_host;
mod each_region_reads_its_own_viewport_size;
mod every_region_is_a_window_painted_in_the_frame_that_first_asks_for_it;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);

fn region(size: Vec2, visible: Vec2, pixels: [u32; 2]) -> Region {
    Region {
        instance: INSTANCE,
        view: block_editor_plugin::View::MAIN,
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
        docked: false,
        host_place: None,
        layers: None,
    }
}

fn whole(width: f32, height: f32) -> Region {
    region(
        vec2(width, height),
        Vec2::ZERO,
        [width as u32, height as u32],
    )
}

fn anchored_region(owner: &Region, anchored: &block_editor_plugin::Anchored) -> Region {
    let rect = anchored.placement.rect;
    Region {
        view: block_editor_plugin::View::anchored(anchored.view),
        rect: Rect::from_min_size(Pos2::ZERO, rect.size()),
        visible: Vec2::ZERO,
        pixels: [rect.width().ceil() as u32, rect.height().ceil() as u32],
        layers: owner.layers.map(|space| block_editor_plugin::LayerSpace {
            to_host: block_editor_plugin::HostTransform {
                origin: rect.min,
                scale: Vec2::splat(1.0),
            },
            bounds: space.bounds,
        }),
        ..owner.clone()
    }
}

fn one_anchored(surface: &mut ModuleRunner, region: &Region) -> Frame {
    surface
        .frame_opened(std::slice::from_ref(region))
        .pop()
        .expect("one view framed gets one frame")
}

fn one(surface: &mut ModuleRunner, region: &Region) -> Frame {
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

impl App for Windowed {
    fn setup(&mut self, setup: &Setup) {
        let views = setup
            .get::<Views>()
            .cloned()
            .expect("the plugin platform provides its views");
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

fn launched(host: &EditorHost, app: impl App + 'static) -> Box<ModuleRunner> {
    let module = Module::default();
    host.join_module(&module);
    launch(
        host,
        RunOptions::default(),
        beui_host::native::context(),
        Box::new(app),
    )
    .expect("a plugin app starts");
    let handler: Box<dyn Any> = module
        .take_handler()
        .expect("the platform registers its handler with the module");
    handler
        .downcast()
        .expect("the plugin platform's handler is a ModuleRunner")
}

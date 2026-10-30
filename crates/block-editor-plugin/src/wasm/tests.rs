use std::pin::pin;
use std::task::{Context, Poll, Waker};

mod a_buffer_mapped_for_reading_holds_what_was_copied_into_it;
mod a_buffer_mapped_for_writing_keeps_what_was_written;
mod a_child_placed_in_an_anchored_view_is_placed_in_its_own_points;
mod a_cleared_range_of_a_buffer_reads_back_as_zeros;
mod a_compute_pass_is_timed_by_its_timestamp_writes;
mod a_compute_pipeline_made_without_a_layout_writes_through_the_layout_its_shader_implies;
mod a_compute_pipeline_writes_the_values_given_to_its_overridable_constants;
mod a_destroyed_buffer_can_no_longer_be_mapped;
mod a_destroyed_device_is_reported_lost_and_drops_what_it_is_asked;
mod a_map_still_pending_when_its_buffer_is_unmapped_fails;
mod a_pipeline_made_without_a_layout_draws_with_the_layout_its_shaders_imply;
mod a_plugin_device_has_only_the_features_and_limits_it_requested;
mod a_rejected_shader_is_caught_by_an_error_scope;
mod a_render_bundle_replays_its_draws_in_a_pass;
mod a_render_bundle_that_draws_without_a_pipeline_is_refused;
mod a_render_pipeline_draws_with_the_values_given_to_its_overridable_constants;
mod a_shader_module_says_where_its_wgsl_went_wrong;
mod a_surface_the_host_does_not_render_runs_its_frames_without_a_gpu;
mod a_texture_view_has_only_the_usages_it_asks_for;
mod a_texture_whose_format_needs_an_unrequested_feature_is_refused;
mod a_timestamp_query_set_needs_its_feature_requested;
mod a_view_without_an_anchor_space_anchors_no_views;
mod a_window_is_committed_with_its_own_screen_in_the_frame_that_opens_it;
mod an_anchored_view_is_a_screen_committed_in_the_frame_that_opens_it;
mod an_anchored_views_surface_is_kept_while_it_is_open_and_released_once_it_closes;
mod an_indexed_indirect_draw_paints_with_the_arguments_in_its_buffer;
mod an_indirect_dispatch_runs_the_workgroups_its_buffer_holds;
mod an_indirect_draw_paints_with_the_arguments_in_its_buffer;
mod an_occlusion_query_counts_only_the_samples_its_draws_pass;
mod debug_groups_reach_the_host_and_must_balance;
mod dismissed_anchored_views_are_heard_before_the_batchs_events;
mod submitted_work_calls_back_once_the_gpu_has_done_it;
mod texels_copied_through_two_textures_read_back_unchanged;
mod timestamps_resolve_to_nanoseconds_coarsened_to_a_tenth_of_a_millisecond;

use std::cell::RefCell;
use std::rc::Rc;

use block_plugin_api::{
    AnchorSpace, AnchoredPlacement, ChildContent, ChildLayer, ChildMode, EditorInstanceId,
    FrameCommit, HelloAccepted, HostTransform, OpenBlock, Opening, ParentView, ScreenId,
    ScreenRequest, Size, SurfaceFormat, SurfaceSpec, TimedInput, TopBar, View, ViewAnchor, ViewId,
    ViewPlacement, ViewRef, ViewportMetrics,
};
use geometry::{Rect, pos2, vec2};
use uuid::Uuid;

use crate::editor_session::EditorSession;
use crate::screens::Screens;
use crate::wasm::Surfaces;
use crate::wire::Delivery;
use crate::{Anchored, EditorHost, Frame, Instance, ShownView, ViewHandler};

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const SCREEN: ScreenId = ScreenId(1);

#[derive(Default)]
struct Seen {
    spaces: Vec<Option<AnchorSpace>>,
    painted: Vec<(ViewId, [u32; 2])>,
    heard: Vec<String>,
}

#[derive(Default)]
struct Shared {
    seen: RefCell<Seen>,
    open: RefCell<Vec<Anchored>>,
    child_in: RefCell<Option<ViewId>>,
    window: RefCell<Option<block_plugin_api::WindowOpen>>,
}

struct Layered {
    host: EditorHost,
    shared: Rc<Shared>,
}

impl Instance for Layered {
    fn connect(&mut self, _block_id: Uuid) {}

    fn open_view(&mut self, _view: View) {
        if !self.host.handles_views() {
            self.host.handle_views(Box::new(AnchoringViews {
                host: self.host.clone(),
                shared: Rc::clone(&self.shared),
            }));
        }
    }
}

struct AnchoringViews {
    host: EditorHost,
    shared: Rc<Shared>,
}

impl ViewHandler for AnchoringViews {
    fn input(&mut self, _region: &ShownView, event: &TimedInput, _over: Option<ViewRef>) {
        let heard = format!("input on {:?} at {:?}", _region.view.id, event.input);
        self.shared.seen.borrow_mut().heard.push(heard);
    }

    fn dismiss_anchored(&mut self, _region: &ShownView, views: &[ViewId]) {
        let heard = format!("dismissed {views:?}");
        self.shared.seen.borrow_mut().heard.push(heard);
    }

    fn frame(&mut self, regions: &[ShownView]) -> Vec<Frame> {
        regions
            .iter()
            .map(|region| {
                self.shared
                    .seen
                    .borrow_mut()
                    .spaces
                    .push(region.anchor_space.clone());
                if let Some(layer) = *self.shared.child_in.borrow() {
                    self.host.in_view(View::anchored(layer), || {
                        let rect = Rect::from_min_size(pos2(230.0, 140.0), vec2(30.0, 20.0));
                        self.host.place_child(
                            ChildContent::Rename([9; 16]),
                            rect,
                            rect,
                            ChildMode::Active,
                            ChildLayer::Above,
                            false,
                            TopBar::Hidden,
                            0.0,
                            1.0,
                            None,
                            false,
                            None,
                            None,
                            false,
                            None,
                        )
                    });
                }
                Frame {
                    changed: true,
                    painted: vec![region.rect],
                    anchored: self.shared.open.borrow().clone(),
                    asks: self
                        .shared
                        .window
                        .borrow_mut()
                        .take()
                        .map(block_plugin_api::WindowRequest::Open)
                        .into_iter()
                        .collect(),
                    ..Frame::default()
                }
            })
            .collect()
    }

    fn frame_opened(&mut self, regions: &[ShownView]) -> Vec<Frame> {
        regions
            .iter()
            .map(|region| Frame {
                painted: vec![region.rect],
                floating: vec![region.rect],
                ..Frame::default()
            })
            .collect()
    }

    fn paint(
        &mut self,
        region: ViewRef,
        target: &crate::PaintTarget<'_>,
    ) -> Vec<block_plugin_api::SurfaceRect> {
        if self
            .shared
            .open
            .borrow()
            .iter()
            .any(|open| open.view == region.view)
        {
            let size = [target.width, target.height];
            self.shared
                .seen
                .borrow_mut()
                .painted
                .push((region.view, size));
        }
        target.whole()
    }
}

fn space() -> AnchorSpace {
    AnchorSpace {
        to_host: HostTransform {
            origin: pos2(200.0, 100.0),
            scale: vec2(2.0, 1.0),
        },
        bounds: Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 800.0)),
    }
}

fn layer(id: u64, parent: u64, rect: Rect) -> Anchored {
    Anchored {
        view: ViewId(id),
        placement: AnchoredPlacement {
            parent: ViewId(parent),
            anchor: ViewAnchor::Rect(Rect::from_min_size(pos2(10.0, 10.0), vec2(20.0, 20.0))),
            rect,
            dismiss_on_outside_press: true,
            keyboard: false,
            blocks_owner: false,
        },
    }
}

struct Rig {
    screens: Screens,
    surfaces: Surfaces,
    requests: Vec<ScreenRequest>,
    shared: Rc<Shared>,
}

impl Rig {
    fn new(layers: Option<AnchorSpace>) -> Self {
        let spec = SurfaceSpec {
            format: SurfaceFormat::Rgba8Unorm,
            render: true,
        };
        let mut screens = Screens::new(Vec::new(), crate::Waker::default());
        screens.deliver(Delivery::Accepted(HelloAccepted {
            host_name: "test host".into(),
            surface: Some(spec),
            preferences: block_plugin_api::Preferences::default(),
        }));
        let host = EditorHost::new(crate::Waker::default());
        let shared = Rc::new(Shared::default());
        let app = Layered {
            host: host.clone(),
            shared: Rc::clone(&shared),
        };
        screens.adopt(
            INSTANCE,
            EditorSession::adopt(INSTANCE, Box::new(app), host),
        );
        let requests = vec![ScreenRequest {
            screen: SCREEN,
            instance: INSTANCE,
            view: ViewId::MAIN,
            placement: ViewPlacement::Parent(ParentView::Main),
            frame: None,
            metrics: ViewportMetrics {
                logical: Size {
                    width: 100.0,
                    height: 80.0,
                },
                visible: block_plugin_api::Pos2::ZERO,
                pixel_width: 200,
                pixel_height: 160,
                scale_factor: 2.0,
                raster: 2.0,
            },
            windows: false,
            host_place: None,
            docked: false,
            anchor_space: layers,
        }];
        screens.deliver(Delivery::Open(Opening::Block(OpenBlock {
            instance: INSTANCE,
            editor: [6; 16],
            block_id: [1; 16],
            block_type: [2; 16],
            view_block: None,
            account_id: [3; 16],
            organization_id: [4; 16],
            client_id: [5; 16],
            editable: true,
        })));
        let surfaces = Surfaces::new(spec).expect("the test runner has a gpu");
        Self {
            screens,
            surfaces,
            requests,
            shared,
        }
    }

    fn open(&self, layers: Vec<Anchored>) {
        *self.shared.open.borrow_mut() = layers;
    }

    fn draw(&mut self) -> FrameCommit {
        self.screens
            .deliver(Delivery::Screens(self.requests.clone()));
        self.screens.deliver(Delivery::Configure(1));
        self.surfaces.place(self.screens.layout().clone());
        self.surfaces
            .render(&mut self.screens)
            .expect("the frame runs")
            .expect("the plugin drew a frame")
    }

    fn screen_surface(&self) -> u32 {
        self.placed(SCREEN).surface
    }

    fn placed(&self, screen: ScreenId) -> block_plugin_api::ScreenPlacement {
        *self
            .screens
            .layout()
            .placement(screen)
            .expect("the screen is laid out")
    }

    fn anchored(commit: &FrameCommit) -> Vec<(ScreenId, AnchoredPlacement)> {
        commit
            .reports
            .iter()
            .filter_map(|report| Some((report.screen, report.anchored?)))
            .collect()
    }

    fn anchored_screens(&self) -> Vec<(ViewId, u32)> {
        self.screens
            .layout()
            .screens
            .iter()
            .filter(|placement| placement.screen.is_opened())
            .map(|placement| (placement.view, placement.surface))
            .collect()
    }
}

fn gpu() -> (wgpu::Device, wgpu::Queue) {
    block_gpu_guest::request_device(&wgpu::DeviceDescriptor {
        required_limits: block_gpu_guest::limits(),
        ..Default::default()
    })
    .unwrap()
}

type Answer = std::sync::Arc<std::sync::Mutex<Option<Result<(), wgpu::BufferAsyncError>>>>;

fn map(buffer: &wgpu::Buffer, mode: wgpu::MapMode) -> Answer {
    let answer = Answer::default();
    let answered = std::sync::Arc::clone(&answer);
    buffer.map_async(mode, .., move |result| {
        *answered.lock().unwrap() = Some(result);
    });
    answer
}

fn read_back(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Vec<u8> {
    let answer = map(buffer, wgpu::MapMode::Read);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(
        *answer.lock().unwrap(),
        Some(Ok(())),
        "the read map settled"
    );
    let bytes = buffer.get_mapped_range(..).to_vec();
    buffer.unmap();
    bytes
}

fn readable(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("read back"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

const FILL: &str = "
@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corners = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(corners[index], 0.0, 1.0);
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return vec4(1.0, 0.0, 1.0, 1.0);
}
";

const MAGENTA: [u8; 4] = [255, 0, 255, 255];

const FILLED: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn filling(device: &wgpu::Device) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("fill"),
        source: wgpu::ShaderSource::Wgsl(FILL.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("fill"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: None,
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: None,
            compilation_options: Default::default(),
            targets: &[Some(FILLED.into())],
        }),
        multiview_mask: None,
        cache: None,
    })
}

const SIDE: u32 = 4;

fn filled_target(device: &wgpu::Device) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("filled"),
        size: wgpu::Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FILLED,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn cleared(view: &wgpu::TextureView) -> Option<wgpu::RenderPassColorAttachment<'_>> {
    Some(wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            store: wgpu::StoreOp::Store,
        },
    })
}

fn texels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mut encoder: wgpu::CommandEncoder,
    target: &wgpu::Texture,
) -> Vec<[u8; 4]> {
    let row = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let destination = readable(device, u64::from(row * SIDE));
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &destination,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: None,
            },
        },
        target.size(),
    );
    queue.submit([encoder.finish()]);
    let read = read_back(device, &destination);
    (0..SIDE)
        .flat_map(|y| (0..SIDE).map(move |x| (y * row + x * 4) as usize))
        .map(|at| [read[at], read[at + 1], read[at + 2], read[at + 3]])
        .collect()
}

fn resolved(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mut encoder: wgpu::CommandEncoder,
    queries: &wgpu::QuerySet,
    count: u32,
) -> Vec<u64> {
    let size = u64::from(count) * 8;
    let resolve = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("resolve"),
        size,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let destination = readable(device, size);
    encoder.resolve_query_set(queries, 0..count, &resolve, 0);
    encoder.copy_buffer_to_buffer(&resolve, 0, &destination, 0, size);
    queue.submit([encoder.finish()]);
    read_back(device, &destination)
        .chunks_exact(8)
        .map(|value| u64::from_le_bytes(value.try_into().unwrap()))
        .collect()
}

fn refusal(scope: wgpu::ErrorScopeGuard) -> String {
    let popped = pin!(scope.pop()).poll(&mut Context::from_waker(Waker::noop()));
    match popped {
        Poll::Ready(Some(error)) => error.to_string(),
        _ => panic!("nothing was refused"),
    }
}

use block_plugin_api::{
    EditorInstanceId, HelloAccepted, OpenBlock, Opening, ParentView, ScreenId, ScreenRequest,
    ViewId, ViewPlacement, ViewportMetrics,
    embed::{
        exports::plugin::{self, Editor},
        host::Call,
    },
    ops::{Connection, Incoming, OpId, Outcome, Side},
};
use block_wasm_host::{Host, Plugin};

mod a_standalone_triangle_paints_what_the_triangle_linked_against_the_sdk_paints;

const SURFACE: u32 = 1;
const WIDTH: u32 = 320;
const HEIGHT: u32 = 200;

fn device() -> (wgpu::Device, wgpu::Queue) {
    let adapter = pollster::block_on(wgpu::Instance::default().request_adapter(
        &wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        },
    ))
    .expect("no graphics adapter is available");
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .expect("the adapter did not provide a device")
}

struct Session {
    connection: Connection,
    draw: Option<OpId>,
    drawn: bool,
}

impl Session {
    fn new() -> Self {
        Self {
            connection: Connection::new(Side::Host),
            draw: None,
            drawn: false,
        }
    }

    fn step(&mut self, plugin: &mut Plugin) {
        plugin
            .deliver(self.connection.take_outbound())
            .expect("the plugin takes what the host sends");
        plugin.step().expect("the plugin steps");
        self.hear(plugin);
    }

    fn hear(&mut self, plugin: &mut Plugin) {
        for item in plugin.take_outbound() {
            self.connection
                .receive(&item)
                .expect("the host reads what the plugin sends");
        }
        let mut answered = false;
        while let Some(event) = self.connection.next_event() {
            match event {
                Incoming::Started(started) if Call::accepts(&started) => {
                    if let Some(Call::Connect { reply, .. }) =
                        Call::accept(&mut self.connection, started)
                    {
                        reply.send(&mut self.connection, &accepted());
                        answered = true;
                    }
                }
                Incoming::Completed { op, outcome } if Some(op) == self.draw => {
                    self.drawn = matches!(outcome, Outcome::Ok(_));
                }
                _ => {}
            }
        }
        if answered {
            self.step(plugin);
        }
    }
}

fn accepted() -> HelloAccepted {
    HelloAccepted {
        host_name: "test host".to_owned(),
        surface: Some(block_plugin_api::SurfaceSpec {
            format: block_plugin_api::SurfaceFormat::Rgba8Unorm,
            render: true,
        }),
        preferences: block_plugin_api::Preferences::default(),
    }
}

fn opening(editor: [u8; 16]) -> Opening {
    Opening::Block(OpenBlock {
        instance: EditorInstanceId(1),
        editor,
        block_id: [1; 16],
        block_type: [2; 16],
        view_block: None,
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
        editable: true,
    })
}

fn screens() -> Vec<ScreenRequest> {
    vec![ScreenRequest {
        screen: ScreenId(1),
        instance: EditorInstanceId(1),
        view: ViewId::MAIN,
        placement: ViewPlacement::Parent(ParentView::Main),
        frame: None,
        metrics: ViewportMetrics {
            logical: block_plugin_api::Size {
                width: WIDTH as f32,
                height: HEIGHT as f32,
            },
            visible: block_plugin_api::Pos2::ZERO,
            pixel_width: WIDTH,
            pixel_height: HEIGHT,
            scale_factor: 1.0,
            raster: 1.0,
        },
        windows: false,
        host_place: None,
        docked: false,
        anchor_space: None,
    }]
}

fn painted(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mut plugin: Plugin,
    editor: [u8; 16],
) -> Vec<u8> {
    let mut session = Session::new();
    plugin.start().expect("the plugin starts");
    session.hear(&mut plugin);
    let _editor = Editor::new(&mut session.connection, opening(editor));
    plugin::screens(&mut session.connection, screens());
    session.step(&mut plugin);
    session.draw = Some(plugin::draw(&mut session.connection, 0, true).op());
    session.step(&mut plugin);
    assert!(session.drawn, "the plugin answered the draw with a frame");
    assert!(
        plugin.take_presented().contains(&SURFACE),
        "the plugin presented its surface"
    );
    let (texture, _) = plugin.surface(SURFACE).expect("the plugin asked for a surface");
    let pixels = read_back(device, queue, &texture);
    plugin.stop();
    pixels
}

fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let (width, height) = (texture.width(), texture.height());
    let row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(row * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(height),
            },
        },
        texture.size(),
    );
    queue.submit(Some(encoder.finish()));
    readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("the device never finished the copy");
    let mapped = readback.slice(..).get_mapped_range();
    mapped
        .chunks(row as usize)
        .flat_map(|line| line[..(width * 4) as usize].to_vec())
        .collect()
}

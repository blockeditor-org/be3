use std::cell::RefCell;

use super::*;

use crate::test_client::{pattern, read};

thread_local! {
    static PANE: RefCell<Option<Signal<Vec2>>> = const { RefCell::new(None) };
}

#[component]
fn Pane(windows: Windows) -> NodeId {
    let size = create_signal(SHOWN);
    PANE.with(|pane| *pane.borrow_mut() = Some(size));
    let ids = create_memo(clone_list(&windows));
    let width = create_memo(move || Size::Custom(size.get().x));
    let height = create_memo(move || Size::Custom(size.get().y));
    view! {
        <List spacing=Space::None>
            <ForEach keys={ids}>|id: WindowId|
                <Frame width=width height=height>
                    <WindowView windows id />
                </Frame>
            </ForEach>
        </List>
    }
}

struct Screen {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: beui::Renderer,
    target: wgpu::Texture,
}

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
const BACKGROUND: [u8; 4] = [0, 0, 0, 255];

impl Screen {
    fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let renderer = beui::Renderer::new(&device, FORMAT);
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
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Self {
            device,
            queue,
            renderer,
            target,
        }
    }

    fn paint(&mut self, harness: &Harness) -> Vec<u8> {
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
        let view = self
            .target
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.renderer.render(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            (SCREEN.x as u32, SCREEN.y as u32),
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        );
        self.queue.submit([encoder.finish()]);
        read(&self.device, &self.queue, &self.target)
    }
}

fn resize(harness: &mut Harness, size: Vec2) {
    let set = PANE.with(|pane| *pane.borrow()).expect("the pane is shown");
    set.set(size);
    harness.frame(Vec::new());
    assert_eq!(
        harness.client.received.size,
        Some((size.x as i32, size.y as i32)),
        "the client is told the pane's new size in the frame the pane changed"
    );
}

fn at(pixels: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    let index = (y * width + x) * 4;
    pixels[index..index + 4].try_into().unwrap()
}

fn assert_shows(screen: &[u8], pane: Rect, buffer: &[u8], size: (usize, usize), when: &str) {
    let screen_width = SCREEN.x as usize;
    let (left, top) = (pane.min.x as usize, pane.min.y as usize);
    for y in 0..pane.height() as usize {
        for x in 0..pane.width() as usize {
            let expected = match x < size.0 && y < size.1 {
                true => at(buffer, size.0, x, y),
                false => BACKGROUND,
            };
            let shown = at(screen, screen_width, left + x, top + y);
            assert_eq!(
                shown, expected,
                "{when}: the pane at {x},{y} shows the buffer's own pixel there, or nothing past its edge"
            );
        }
    }
    let past = (left + pane.width() as usize + 10, top + 5);
    assert_eq!(
        at(screen, screen_width, past.0, past.1),
        BACKGROUND,
        "{when}: nothing of the buffer is drawn past the pane"
    );
}

#[test]
fn a_resized_window_is_drawn_unscaled_until_it_commits_the_new_size() {
    let (device, queue) = crate::test_client::vulkan_device();
    let mut harness = Harness::showing(Some((device.clone(), queue.clone())), |windows| {
        view! {
            <Pane windows />
        }
    });
    let mut screen = Screen::new(device, queue);
    let (window, id) = harness.open();
    let first = pattern(SHOWN.x as u32, SHOWN.y as u32);
    harness
        .client
        .attach_pixels_unsent(&window, SHOWN.x as i32, SHOWN.y as i32, &first);
    harness.settle();
    let size = (SHOWN.x as usize, SHOWN.y as usize);

    resize(&mut harness, Vec2::new(500.0, 360.0));
    harness.settle();
    let pane = harness.window(id);
    assert_eq!(pane.size(), Vec2::new(500.0, 360.0));
    assert_shows(
        &screen.paint(&harness),
        pane,
        &first,
        size,
        "grown before the client answers",
    );

    resize(&mut harness, Vec2::new(300.0, 200.0));
    harness.settle();
    let pane = harness.window(id);
    assert_shows(
        &screen.paint(&harness),
        pane,
        &first,
        size,
        "shrunk before the client answers",
    );

    harness.acknowledge(&window);
    let answer = pattern(300, 200);
    harness
        .client
        .attach_pixels_unsent(&window, 300, 200, &answer);
    harness.settle();
    assert_shows(
        &screen.paint(&harness),
        harness.window(id),
        &answer,
        (300, 200),
        "once the client draws at the pane's size",
    );
}

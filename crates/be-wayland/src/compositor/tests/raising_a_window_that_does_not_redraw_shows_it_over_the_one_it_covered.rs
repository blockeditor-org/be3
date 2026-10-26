use std::cell::RefCell;

use super::*;
use beui::styled::*;

use crate::test_client::read;

thread_local! {
    static ORDER: RefCell<Option<Signal<Vec<WindowId>>>> = const { RefCell::new(None) };
}

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
const RED: [u8; 4] = [0, 0, 255, 255];
const BLUE: [u8; 4] = [255, 0, 0, 255];

#[component]
fn Stacked(windows: Windows) -> NodeId {
    let order = create_signal(Vec::new());
    ORDER.with(|held| *held.borrow_mut() = Some(order));
    view! {
        <Layers>
            <ForEach keys={order}>|id: WindowId|
                <Frame width={Size::Custom(SHOWN.x)} height={Size::Custom(SHOWN.y)}>
                    <WindowView windows id />
                </Frame>
            </ForEach>
        </Layers>
    }
}

struct Screen {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: beui::Renderer,
    target: wgpu::Texture,
}

impl Screen {
    fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let renderer = beui::Renderer::new(&device, FORMAT);
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("kept screen"),
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

    fn frame(&mut self, harness: &mut Harness) {
        use beui::Repainting as _;
        harness.frame(Vec::new());
        let output = harness.output.as_ref().expect("a frame ran");
        let repaint = output.main().repaint(beui::Color32::BLACK);
        let repaint = self.renderer.prepare(
            &self.device,
            &self.queue,
            output.main(),
            SCREEN,
            1.0,
            repaint,
        );
        let load = match repaint {
            beui::Repaint::Region { .. } => wgpu::LoadOp::Load,
            beui::Repaint::Everything => wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        };
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
            load,
        );
        self.queue.submit([encoder.finish()]);
    }

    fn settle(&mut self, harness: &mut Harness) {
        for _ in 0..3 {
            self.frame(harness);
        }
    }

    fn at(&self, point: Pos2) -> [u8; 4] {
        let pixels = read(&self.device, &self.queue, &self.target);
        let index = (point.y as usize * SCREEN.x as usize + point.x as usize) * 4;
        pixels[index..index + 4].try_into().unwrap()
    }
}

fn stack(order: Vec<WindowId>) {
    let set = ORDER
        .with(|held| *held.borrow())
        .expect("the stack is shown");
    set.set(order);
}

fn opened(harness: &mut Harness, screen: &mut Screen, color: [u8; 4]) -> (TestWindow, WindowId) {
    let window = harness.client.toplevel();
    screen.settle(harness);
    let id = *harness
        .compositor()
        .server()
        .state
        .windows()
        .last()
        .expect("the toplevel opened a window");
    harness.acknowledge(&window);
    let (width, height) = (SHOWN.x as i32, SHOWN.y as i32);
    let pixels: Vec<u8> = std::iter::repeat_n(color, (width * height) as usize)
        .flatten()
        .collect();
    harness
        .client
        .attach_pixels_unsent(&window, width, height, &pixels);
    screen.settle(harness);
    (window, id)
}

#[test]
fn raising_a_window_that_does_not_redraw_shows_it_over_the_one_it_covered() {
    let (device, queue) = crate::test_client::vulkan_device();
    let mut harness = Harness::showing(Some((device.clone(), queue.clone())), |windows| {
        view! {
            <Stacked windows />
        }
    });
    let mut screen = Screen::new(device, queue);
    let (_red, first) = opened(&mut harness, &mut screen, RED);
    stack(vec![first]);
    screen.settle(&mut harness);
    let (_blue, second) = opened(&mut harness, &mut screen, BLUE);
    stack(vec![first, second]);
    screen.settle(&mut harness);
    let middle = harness.window(first).center();
    assert_eq!(screen.at(middle), BLUE, "the later window is on top");

    stack(vec![second, first]);
    screen.settle(&mut harness);
    assert_eq!(
        screen.at(middle),
        RED,
        "raised, the window that drew nothing new shows over the one it covered"
    );
}

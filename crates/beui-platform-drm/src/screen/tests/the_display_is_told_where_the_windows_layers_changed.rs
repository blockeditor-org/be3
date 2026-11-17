use std::cell::{Cell, RefCell};
use std::rc::Rc;

use be_lib_layers::{LayerId, Surface};
use beui::native::compose::{Placed, layer_of};
use beui::native::texture_quad::{Hole, Pipelines, Quad, Quads};
use beui::native::{Draw, DrawAt};

use super::*;

/// Shows a texture as a screen, at `at` (in pixels) within the drawing's rect.
struct Shows {
    texture: wgpu::Texture,
    at: Cell<f32>,
    quads: Quads,
    placed: RefCell<Vec<Placed>>,
}

impl Draw for Shows {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let left = at.rect[0] + self.at.get();
        let quad = Quad {
            rect: [left, at.rect[1], left + 4.0, at.rect[1] + 4.0],
            clip: at.clip,
            ..Quad::default()
        };
        let layer = layer_of(
            &quad,
            (1, 1),
            at.pixels_per_point,
            LayerId(5),
            Surface::Screen {
                instance: 5,
                surface: 0,
            },
        )
        .expect("the screen shows");
        *self.placed.borrow_mut() = vec![Placed {
            layer,
            texture: Pipelines::of(device, &at).bind(&self.texture),
            opaque: true,
            srgb_encoded: false,
        }];
        let hole = Hole::of(device, queue, &at);
        self.quads.prepare(
            &hole.pipelines,
            queue,
            at.format,
            [at.screen.x, at.screen.y],
            [(&hole.white, quad)],
        );
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        self.quads.paint(pass);
    }

    fn layers(&self) -> Vec<Placed> {
        self.placed.borrow().clone()
    }
}

#[test]
fn the_display_is_told_where_the_windows_layers_changed() {
    beui::native::compose::composite_windows();
    let (device, queue) = vulkan_device();
    let gpu = Gpu::new(device, queue, FORMAT);
    let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("screen test surface"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let shows = Rc::new(Shows {
        texture,
        at: Cell::new(0.0),
        quads: Quads::default(),
        placed: RefCell::new(Vec::new()),
    });
    let draw: Rc<dyn Draw> = shows.clone();
    let drawing = beui::native::Drawing::erased(draw);
    let context = beui_host::native::context();
    let paint = || {
        context
            .run(|context| {
                context.painter().drawing(
                    Rect::from_min_size(pos2(0.0, 0.0), vec2(16.0, 8.0)),
                    &drawing,
                );
            })
            .main()
            .clone()
    };
    let mut screen = Screen::new(&gpu, (WIDTH, HEIGHT), Rotation::Normal);
    screen.rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(32.0, 24.0));
    let target = target(&gpu);
    let frame = Frame {
        clear: Color32::BLACK,
        pointer: pos2(0.0, 0.0),
        sprite: Pointer::Hidden,
    };
    let first = paint();
    screen.prepare(&gpu, &first);
    let (commands, damage) = screen.draw(&gpu, &frame, &target);
    gpu.queue().submit([commands]);
    assert_eq!(damage, None, "the first frame changes the whole screen");

    // The screen moves within its drawing, while the host damages only a corner far from it.
    shows.at.set(8.0);
    let second = paint();
    screen.damage(Repaint::Region {
        region: Rect::from_min_size(pos2(28.0, 20.0), vec2(2.0, 2.0)).into(),
        background: Color32::BLACK,
    });
    screen.prepare(&gpu, &second);
    let (commands, damage) = screen.draw(&gpu, &frame, &target);
    gpu.queue().submit([commands]);
    let damage = damage.expect("a frame that changes a part names that part");
    for (x, what) in [(0, "where the screen was"), (8, "where it is")] {
        let shown = Rectangle::<i32, Physical>::new((x, 0).into(), (4, 4).into());
        assert!(
            damage.iter().any(|rect| rect.contains_rect(shown)),
            "{what} is in the damage: {damage:?}"
        );
    }
    let corner = Rectangle::<i32, Physical>::new((28, 20).into(), (2, 2).into());
    assert!(
        damage.iter().any(|rect| rect.contains_rect(corner)),
        "so is the host's own damage: {damage:?}"
    );
}

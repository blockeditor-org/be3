use std::rc::Rc;

use beui::native::painting::{Content, Frame, Painting, Paints, Primitive, Triangle, Vertex};
use beui::{Rect, native::Drawing};
use block_plugin_api::PreviewImage;

#[cfg(target_arch = "wasm32")]
mod gpu;

pub(crate) fn picture(image: PreviewImage, rotation: f32, opacity: f32) -> Drawing {
    let paints = Rc::new(PicturePaints {
        image: image.image,
        rotation,
        opacity,
    });
    #[cfg(target_arch = "wasm32")]
    {
        let draw: Rc<dyn beui::native::Draw> =
            Rc::new(gpu::PictureDraw::new(image, rotation, opacity));
        Drawing::painted(draw, paints)
    }
    #[cfg(not(target_arch = "wasm32"))]
    Drawing::painted((), paints)
}

struct PicturePaints {
    image: u64,
    rotation: f32,
    opacity: f32,
}

impl Paints for PicturePaints {
    fn painting(&self, rect: Rect) -> Option<Painting> {
        let key = beui::native::painting::preview_key(self.image);
        let center = rect.center();
        let (sin, cos) = self.rotation.sin_cos();
        let alpha = (self.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        let corner = |x: f32, y: f32| {
            let (dx, dy) = (x - center.x, y - center.y);
            Vertex {
                pos: [
                    center.x + dx * cos - dy * sin,
                    center.y + dx * sin + dy * cos,
                ],
                uv: [
                    f32::from(u8::from(x > center.x)),
                    f32::from(u8::from(y > center.y)),
                ],
                color: [255, 255, 255, alpha],
            }
        };
        let top_left = corner(rect.min.x, rect.min.y);
        let top_right = corner(rect.max.x, rect.min.y);
        let bottom_right = corner(rect.max.x, rect.max.y);
        let bottom_left = corner(rect.min.x, rect.max.y);
        Some(Painting {
            frame: Frame {
                size: [1, 1],
                pixels_per_point: 1.0,
                background: [0; 4],
                primitives: vec![Primitive {
                    clip: [f32::MIN, f32::MIN, f32::MAX, f32::MAX],
                    fade: beui::native::painting::Fade::NONE,
                    rounds: Vec::new(),
                    content: Content::Mesh(vec![
                        Triangle {
                            texture: key,
                            corners: [top_left, top_right, bottom_right],
                        },
                        Triangle {
                            texture: key,
                            corners: [top_left, bottom_right, bottom_left],
                        },
                    ]),
                }],
            },
            textures: Default::default(),
        })
    }
}

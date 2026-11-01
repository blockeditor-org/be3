use super::*;

mod a_buffer_larger_than_its_pane_is_clipped_not_scaled;
mod a_buffer_smaller_than_its_pane_is_drawn_unscaled_at_the_top_left;
mod a_scaled_buffer_is_drawn_at_its_logical_size;
mod a_viewport_destination_is_the_size_the_surface_is_drawn_at;

const SCREEN: beui::Vec2 = beui::Vec2::new(1000.0, 800.0);

fn pane(left: f32, top: f32, width: f32, height: f32, pixels_per_point: f32) -> DrawAt {
    DrawAt {
        rect: [left, top, left + width, top + height],
        clip: [0.0, 0.0, SCREEN.x, SCREEN.y],
        screen: SCREEN,
        pixels_per_point,
        format: wgpu::TextureFormat::Bgra8Unorm,
        renderer: 0,
    }
}

fn surface(width: f64, height: f64) -> Rectangle<f64, Logical> {
    Rectangle::from_size((width, height).into())
}

#[derive(Debug, PartialEq)]
struct Drawn {
    pixels: [f32; 4],
    uv: [f32; 4],
}

fn drawn(
    rect: Rectangle<f64, Logical>,
    source: Rectangle<f64, Logical>,
    buffer: (f64, f64),
    at: &DrawAt,
) -> Drawn {
    let quad = place(rect, source, buffer.into(), at);
    let shown = quad.visible().expect("the surface is drawn");
    let [left, top, right, bottom] = quad.rect;
    let u = |x: f32| quad.uv[0] + (x - left) / (right - left) * (quad.uv[2] - quad.uv[0]);
    let v = |y: f32| quad.uv[1] + (y - top) / (bottom - top) * (quad.uv[3] - quad.uv[1]);
    let round = |value: f32| (value * 1000.0).round() / 1000.0;
    Drawn {
        pixels: shown.map(f32::round),
        uv: [u(shown[0]), v(shown[1]), u(shown[2]), v(shown[3])].map(round),
    }
}

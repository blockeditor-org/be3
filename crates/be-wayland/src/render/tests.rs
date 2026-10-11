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
    let corners = place(rect, source, buffer.into(), at).expect("the surface is drawn");
    let (first, last) = (corners[0], corners[2]);
    let x = |ndc: f32| ((ndc + 1.0) / 2.0 * at.screen.x).round();
    let y = |ndc: f32| ((1.0 - ndc) / 2.0 * at.screen.y).round();
    let uv = |value: f32| (value * 1000.0).round() / 1000.0;
    Drawn {
        pixels: [
            x(first.position[0]),
            y(first.position[1]),
            x(last.position[0]),
            y(last.position[1]),
        ],
        uv: [
            uv(first.uv[0]),
            uv(first.uv[1]),
            uv(last.uv[0]),
            uv(last.uv[1]),
        ],
    }
}

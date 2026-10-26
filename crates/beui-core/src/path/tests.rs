use super::*;

mod a_filled_square_is_two_triangles_with_its_sides_on_the_outline;
mod a_stroke_is_filled_as_an_outline_around_its_path;
mod an_edge_on_a_seam_is_not_antialiased;
mod dashes_split_a_stroke_into_pieces;
mod even_odd_leaves_a_hole_where_non_zero_fills_it;

fn area(triangles: &[PathTriangle]) -> f32 {
    triangles
        .iter()
        .map(|triangle| {
            let [a, b, c] = triangle.corners;
            ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() / 2.0
        })
        .sum()
}

fn square(min: f32, max: f32) -> Path {
    Path::polygon([
        pos2(min, min),
        pos2(max, min),
        pos2(max, max),
        pos2(min, max),
    ])
}

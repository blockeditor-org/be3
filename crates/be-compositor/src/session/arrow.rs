pub const WIDTH: u32 = 12;
pub const HEIGHT: u32 = 19;

const OUTLINE: [(f32, f32); 7] = [
    (0.5, 0.5),
    (0.5, 16.5),
    (4.5, 12.5),
    (7.5, 18.5),
    (9.5, 17.5),
    (6.5, 11.5),
    (11.5, 11.5),
];

pub fn pixels() -> Vec<u8> {
    let inside = |x: i32, y: i32| {
        x >= 0
            && y >= 0
            && (x as u32) < WIDTH
            && (y as u32) < HEIGHT
            && contains(x as f32 + 0.5, y as f32 + 0.5)
    };
    let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for y in 0..HEIGHT as i32 {
        for x in 0..WIDTH as i32 {
            let pixel = if !inside(x, y) {
                [0, 0, 0, 0]
            } else if [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .iter()
                .all(|(dx, dy)| inside(x + dx, y + dy))
            {
                [255, 255, 255, 255]
            } else {
                [0, 0, 0, 255]
            };
            pixels.extend_from_slice(&pixel);
        }
    }
    pixels
}

fn contains(x: f32, y: f32) -> bool {
    let mut inside = false;
    let mut previous = OUTLINE[OUTLINE.len() - 1];
    for point in OUTLINE {
        if (point.1 > y) != (previous.1 > y)
            && x < (previous.0 - point.0) * (y - point.1) / (previous.1 - point.1) + point.0
        {
            inside = !inside;
        }
        previous = point;
    }
    inside
}

#[cfg(test)]
mod tests;

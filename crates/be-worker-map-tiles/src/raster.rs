use be_worker_map::{TILE_PIXELS, TileLabel, TileRaster};

use crate::mvt::{Feature, GeometryKind, Layer, TagValue, Tile};

const SUPERSAMPLE: usize = 2;
const CANVAS: usize = TILE_PIXELS * SUPERSAMPLE;

const LAND: [u8; 4] = [242, 239, 233, 255];
const WATER: [u8; 4] = [170, 211, 223, 255];
const BUILDING: [u8; 4] = [217, 208, 201, 255];
const SITE: [u8; 4] = [232, 229, 219, 255];
const URBAN: [u8; 4] = [224, 222, 219, 255];
const GRASS: [u8; 4] = [205, 235, 176, 255];
const PARK_EDGE: [u8; 4] = [110, 170, 90, 150];
const PEDESTRIAN_AREA: [u8; 4] = [237, 237, 237, 255];
const RUNWAY: [u8; 4] = [190, 190, 200, 255];
const BOUNDARY: [u8; 4] = [140, 110, 150, 170];
const PLACE_LABEL_COLOR: [u8; 3] = [60, 60, 60];
const REGION_LABEL_COLOR: [u8; 3] = [110, 100, 125];
const MAX_LABELS: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Window {
    pub factor: f32,
    pub offset: [f32; 2],
}

impl Window {
    #[cfg(test)]
    pub(crate) const WHOLE: Self = Self {
        factor: 1.0,
        offset: [0.0, 0.0],
    };

    fn place(self, point: [f32; 2], extent: u32, size: usize) -> [f32; 2] {
        let scale = size as f32 * self.factor / extent as f32;
        [
            point[0] * scale - self.offset[0] * size as f32,
            point[1] * scale - self.offset[1] * size as f32,
        ]
    }
}

/// Draws a tile of the OpenMapTiles schema (openmaptiles.org/schema), the one OpenFreeMap serves:
/// its areas, then its lines, then buildings and boundaries over them; place names become labels.
pub(crate) fn rasterize(tile: &Tile, zoom: u8, window: Window) -> TileRaster {
    let mut canvas = Canvas::new(window);
    let line_scale = line_width_scale(zoom);

    fill_layer(&mut canvas, tile.layer("landuse"), |layer, feature| {
        landuse_color(class(layer, feature))
    });
    fill_layer(&mut canvas, tile.layer("landcover"), |layer, feature| {
        landcover_color(class(layer, feature))
    });
    outline_layer(&mut canvas, tile.layer("park"), PARK_EDGE, 1.5);
    fill_layer(&mut canvas, tile.layer("water"), |layer, feature| {
        (!tunnel(layer, feature)).then_some(WATER)
    });
    stroke_layer(&mut canvas, tile.layer("waterway"), |layer, feature| {
        if tunnel(layer, feature) {
            return None;
        }
        let width = match class(layer, feature) {
            "river" => 3.5,
            "canal" => 2.5,
            _ => 1.2,
        };
        Some((WATER, width * line_scale))
    });
    fill_layer(
        &mut canvas,
        tile.layer("aeroway"),
        |layer, feature| match class(layer, feature) {
            "runway" | "taxiway" => Some(RUNWAY),
            "aerodrome" | "apron" => Some(SITE),
            _ => None,
        },
    );
    stroke_layer(
        &mut canvas,
        tile.layer("aeroway"),
        |layer, feature| match class(layer, feature) {
            "runway" => Some((RUNWAY, 6.0 * line_scale)),
            "taxiway" => Some((RUNWAY, 2.0 * line_scale)),
            _ => None,
        },
    );
    fill_layer(
        &mut canvas,
        tile.layer("transportation"),
        |layer, feature| match class(layer, feature) {
            "pier" => Some(LAND),
            "path" => Some(PEDESTRIAN_AREA),
            _ => Some([255, 255, 255, 255]),
        },
    );
    roads(&mut canvas, tile.layer("transportation"), line_scale);
    fill_layer(&mut canvas, tile.layer("building"), |_, _| Some(BUILDING));
    stroke_layer(&mut canvas, tile.layer("boundary"), |layer, feature| {
        if number(layer, feature, "maritime") == Some(1) {
            return None;
        }
        let admin_level = number(layer, feature, "admin_level").unwrap_or(2);
        let width = if admin_level <= 2 { 2.2 } else { 1.2 };
        Some((BOUNDARY, width))
    });

    TileRaster {
        pixels: canvas.into_pixels(),
        labels: labels(tile, window),
    }
}

fn text<'a>(layer: &'a Layer, feature: &Feature, key: &str) -> &'a str {
    layer
        .tag(feature, key)
        .and_then(TagValue::as_str)
        .unwrap_or("")
}

fn class<'a>(layer: &'a Layer, feature: &Feature) -> &'a str {
    text(layer, feature, "class")
}

fn number(layer: &Layer, feature: &Feature, key: &str) -> Option<i64> {
    layer.tag(feature, key).and_then(TagValue::as_i64)
}

fn tunnel(layer: &Layer, feature: &Feature) -> bool {
    text(layer, feature, "brunnel") == "tunnel"
}

fn landuse_color(class: &str) -> Option<[u8; 4]> {
    match class {
        "residential" | "suburb" | "quarter" | "neighbourhood" | "commercial" | "retail"
        | "industrial" | "garages" | "railway" | "quarry" | "dam" => Some(URBAN),
        "cemetery" | "playground" | "pitch" | "zoo" | "theme_park" => Some(GRASS),
        "school" | "university" | "college" | "kindergarten" | "hospital" | "military"
        | "stadium" | "track" | "bus_station" | "library" => Some(SITE),
        _ => None,
    }
}

fn landcover_color(class: &str) -> Option<[u8; 4]> {
    match class {
        "wood" => Some([157, 202, 138, 255]),
        "grass" => Some(GRASS),
        "farmland" => Some([238, 240, 213, 255]),
        "sand" => Some([245, 233, 198, 255]),
        "rock" => Some([219, 217, 214, 255]),
        "ice" => Some([221, 236, 240, 255]),
        "wetland" => Some([211, 230, 219, 255]),
        _ => None,
    }
}

fn line_width_scale(zoom: u8) -> f32 {
    match zoom {
        0..=7 => 0.35,
        8..=9 => 0.45,
        10..=11 => 0.55,
        12..=13 => 0.75,
        14 => 1.0,
        15 => 1.4,
        16 => 2.0,
        _ => 2.8,
    }
}

/// A line of the `transportation` layer's rank (higher draws over lower), color and width.
fn road_style(class: &str, subclass: &str) -> Option<(u8, [u8; 4], f32)> {
    match class {
        "motorway" => Some((8, [233, 144, 161, 255], 6.0)),
        "trunk" => Some((7, [250, 163, 141, 255], 5.5)),
        "primary" => Some((5, [252, 214, 164, 255], 5.5)),
        "secondary" => Some((4, [247, 250, 191, 255], 4.5)),
        "tertiary" => Some((3, [255, 255, 255, 255], 4.0)),
        "minor" | "busway" => Some((2, [255, 255, 255, 255], 3.0)),
        "service" => Some((1, [255, 255, 255, 255], 1.8)),
        "track" => Some((0, [172, 148, 109, 255], 1.6)),
        "path" => match subclass {
            "pedestrian" => Some((1, PEDESTRIAN_AREA, 2.0)),
            "cycleway" => Some((0, [130, 130, 220, 255], 1.2)),
            _ => Some((0, [200, 140, 120, 255], 1.2)),
        },
        "pier" => Some((0, LAND, 3.0)),
        "ferry" => Some((0, [100, 130, 200, 255], 1.0)),
        "rail" => Some((6, [140, 140, 140, 255], 1.6)),
        "transit" => Some((6, [150, 150, 150, 255], 1.4)),
        "aerialway" => Some((6, [110, 110, 110, 255], 1.0)),
        _ => None,
    }
}

fn roads(canvas: &mut Canvas, layer: Option<&Layer>, line_scale: f32) {
    let Some(layer) = layer else {
        return;
    };
    let mut styled: Vec<(u8, [u8; 4], f32, &Feature)> = layer
        .features
        .iter()
        .filter(|feature| feature.kind == GeometryKind::Line)
        .filter_map(|feature| {
            let subclass = text(layer, feature, "subclass");
            let (rank, color, width) = road_style(class(layer, feature), subclass)?;
            Some((rank, color, width * line_scale, feature))
        })
        .collect();
    styled.sort_by_key(|(rank, ..)| *rank);
    for (_, color, width, feature) in styled {
        let paths = canvas.transformed(feature, layer.extent);
        canvas.stroke(&paths, width * SUPERSAMPLE as f32, color);
    }
}

fn fill_layer(
    canvas: &mut Canvas,
    layer: Option<&Layer>,
    style: impl Fn(&Layer, &Feature) -> Option<[u8; 4]>,
) {
    let Some(layer) = layer else {
        return;
    };
    for feature in &layer.features {
        if feature.kind != GeometryKind::Polygon {
            continue;
        }
        let Some(color) = style(layer, feature) else {
            continue;
        };
        let paths = canvas.transformed(feature, layer.extent);
        canvas.fill(&paths, color);
    }
}

/// Strokes the edges of a layer's areas.
fn outline_layer(canvas: &mut Canvas, layer: Option<&Layer>, color: [u8; 4], width: f32) {
    let Some(layer) = layer else {
        return;
    };
    for feature in &layer.features {
        if feature.kind != GeometryKind::Polygon {
            continue;
        }
        let mut rings = canvas.transformed(feature, layer.extent);
        for ring in &mut rings {
            if let Some(first) = ring.first().copied() {
                ring.push(first);
            }
        }
        canvas.stroke(&rings, width * SUPERSAMPLE as f32, color);
    }
}

fn stroke_layer(
    canvas: &mut Canvas,
    layer: Option<&Layer>,
    style: impl Fn(&Layer, &Feature) -> Option<([u8; 4], f32)>,
) {
    let Some(layer) = layer else {
        return;
    };
    for feature in &layer.features {
        if feature.kind != GeometryKind::Line {
            continue;
        }
        let Some((color, width)) = style(layer, feature) else {
            continue;
        };
        let paths = canvas.transformed(feature, layer.extent);
        canvas.stroke(&paths, width * SUPERSAMPLE as f32, color);
    }
}

fn labels(tile: &Tile, window: Window) -> Vec<TileLabel> {
    let mut labels = Vec::new();
    if let Some(layer) = tile.layer("place") {
        for feature in &layer.features {
            let Some(name) = layer.tag(feature, "name").and_then(TagValue::as_str) else {
                continue;
            };
            let capital = number(layer, feature, "capital");
            let (font_size, color) = match class(layer, feature) {
                "continent" | "country" => (13.0, REGION_LABEL_COLOR),
                "state" => (11.0, REGION_LABEL_COLOR),
                "city" | "town" if matches!(capital, Some(2..=4)) => (15.0, PLACE_LABEL_COLOR),
                "city" => (14.0, PLACE_LABEL_COLOR),
                "town" => (12.0, PLACE_LABEL_COLOR),
                "village" => (11.0, PLACE_LABEL_COLOR),
                _ => (10.0, PLACE_LABEL_COLOR),
            };
            push_label(&mut labels, window, layer, feature, name, font_size, color);
        }
    }
    labels.sort_by(|a, b| b.font_size.total_cmp(&a.font_size));
    labels.truncate(MAX_LABELS);
    labels
}

fn push_label(
    labels: &mut Vec<TileLabel>,
    window: Window,
    layer: &Layer,
    feature: &Feature,
    name: &str,
    font_size: f32,
    color: [u8; 3],
) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    let Some(point) = feature.paths.first().and_then(|path| path.first()) else {
        return;
    };
    let position = window.place(*point, layer.extent, TILE_PIXELS);
    let bounds = 0.0..TILE_PIXELS as f32;
    if !bounds.contains(&position[0]) || !bounds.contains(&position[1]) {
        return;
    }
    labels.push(TileLabel {
        text: name.to_owned(),
        position,
        font_size,
        color,
    });
}

struct Canvas {
    pixels: Vec<[u8; 4]>,
    crossings: Vec<f32>,
    window: Window,
}

impl Canvas {
    fn new(window: Window) -> Self {
        Self {
            pixels: vec![LAND; CANVAS * CANVAS],
            crossings: Vec::new(),
            window,
        }
    }

    fn transformed(&self, feature: &Feature, extent: u32) -> Vec<Vec<[f32; 2]>> {
        feature
            .paths
            .iter()
            .map(|path| {
                path.iter()
                    .map(|point| self.window.place(*point, extent, CANVAS))
                    .collect()
            })
            .collect()
    }

    fn fill<P: AsRef<[[f32; 2]]>>(&mut self, paths: &[P], color: [u8; 4]) {
        let mut min = [f32::MAX; 2];
        let mut max = [f32::MIN; 2];
        for path in paths {
            for point in path.as_ref() {
                for axis in 0..2 {
                    min[axis] = min[axis].min(point[axis]);
                    max[axis] = max[axis].max(point[axis]);
                }
            }
        }
        if min[0] >= CANVAS as f32 || max[0] <= 0.0 || min[1] >= CANVAS as f32 || max[1] <= 0.0 {
            return;
        }
        let rows = (min[1].floor().max(0.0) as usize)..(max[1].ceil().min(CANVAS as f32) as usize);
        let mut crossings = std::mem::take(&mut self.crossings);
        for row in rows {
            let sample_y = row as f32 + 0.5;
            crossings.clear();
            for path in paths {
                let path = path.as_ref();
                if path.len() < 3 {
                    continue;
                }
                for index in 0..path.len() {
                    let a = path[index];
                    let b = path[(index + 1) % path.len()];
                    if (a[1] <= sample_y) != (b[1] <= sample_y) {
                        let t = (sample_y - a[1]) / (b[1] - a[1]);
                        crossings.push(a[0] + t * (b[0] - a[0]));
                    }
                }
            }
            crossings.sort_unstable_by(f32::total_cmp);
            for span in crossings.as_chunks::<2>().0 {
                let clamp = |edge: f32| ((edge - 0.5).ceil().max(0.0) as usize).min(CANVAS);
                let start = clamp(span[0]);
                let end = clamp(span[1]);
                for pixel in &mut self.pixels[row * CANVAS + start..row * CANVAS + end] {
                    blend(pixel, color);
                }
            }
        }
        self.crossings = crossings;
    }

    fn stroke<P: AsRef<[[f32; 2]]>>(&mut self, paths: &[P], width: f32, color: [u8; 4]) {
        let half = (width * 0.5).max(0.35);
        for path in paths {
            let path = path.as_ref();
            for segment in path.windows(2) {
                let [ax, ay] = segment[0];
                let [bx, by] = segment[1];
                let (dx, dy) = (bx - ax, by - ay);
                let length = (dx * dx + dy * dy).sqrt();
                if length < 1e-3 {
                    continue;
                }

                let (ex, ey) = (dx / length * half, dy / length * half);
                let (nx, ny) = (-ey, ex);
                let quad = [
                    [ax - ex + nx, ay - ey + ny],
                    [bx + ex + nx, by + ey + ny],
                    [bx + ex - nx, by + ey - ny],
                    [ax - ex - nx, ay - ey - ny],
                ];
                self.fill(&[quad], color);
            }
        }
    }

    fn into_pixels(self) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(TILE_PIXELS * TILE_PIXELS * 4);
        for y in 0..TILE_PIXELS {
            for x in 0..TILE_PIXELS {
                let mut sum = [0u32; 3];
                for sy in 0..SUPERSAMPLE {
                    for sx in 0..SUPERSAMPLE {
                        let sample =
                            self.pixels[(y * SUPERSAMPLE + sy) * CANVAS + x * SUPERSAMPLE + sx];
                        for channel in 0..3 {
                            sum[channel] += u32::from(sample[channel]);
                        }
                    }
                }
                let samples = (SUPERSAMPLE * SUPERSAMPLE) as u32;
                pixels.extend([
                    (sum[0] / samples) as u8,
                    (sum[1] / samples) as u8,
                    (sum[2] / samples) as u8,
                    255,
                ]);
            }
        }
        pixels
    }
}

fn blend(pixel: &mut [u8; 4], color: [u8; 4]) {
    let alpha = u32::from(color[3]);
    if alpha == 255 {
        *pixel = color;
        return;
    }
    for channel in 0..3 {
        let blended = u32::from(color[channel]) * alpha + u32::from(pixel[channel]) * (255 - alpha);
        pixel[channel] = (blended / 255) as u8;
    }
}

#[cfg(test)]
mod tests;

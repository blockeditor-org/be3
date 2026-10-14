struct Wheel {
    rect: vec4<f32>,
    clip: vec4<f32>,
    screen: vec2<f32>,
    hue: f32,
    model: f32,
    ring_width: f32,
    ring_gap: f32,
    gamut_line: f32,
    srgb: f32,
};

@group(0) @binding(0)
var<uniform> wheel: Wheel;

const TIP_CHROMA: f32 = 0.38;
const GAMUT_LINE_OPACITY: f32 = 0.7;
const SEARCH_STEPS: i32 = 24;
const CHROMA_LIMIT: f32 = 0.5;
const SLACK: f32 = 1e-4;
const DEGENERATE: f32 = 1e-6;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32(index & 1u), f32((index >> 1u) & 1u));
    let point = mix(wheel.rect.xy, wheel.rect.zw, corner);
    return vec4<f32>(point / wheel.screen * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
}

fn direction(hue: f32) -> vec2<f32> {
    let angle = radians(hue);
    return vec2<f32>(cos(angle), -sin(angle));
}

fn hsv(hue: f32, saturation: f32, value: f32) -> vec3<f32> {
    let sector = (hue % 360.0) / 60.0;
    let chroma = value * saturation;
    let second = chroma * (1.0 - abs(sector % 2.0 - 1.0));
    var rgb: vec3<f32>;
    switch u32(sector) {
        case 0u: { rgb = vec3<f32>(chroma, second, 0.0); }
        case 1u: { rgb = vec3<f32>(second, chroma, 0.0); }
        case 2u: { rgb = vec3<f32>(0.0, chroma, second); }
        case 3u: { rgb = vec3<f32>(0.0, second, chroma); }
        case 4u: { rgb = vec3<f32>(second, 0.0, chroma); }
        default: { rgb = vec3<f32>(chroma, 0.0, second); }
    }
    return rgb + (value - chroma);
}

fn cone_weights(ab: vec2<f32>) -> vec3<f32> {
    return vec3<f32>(
        0.39633778 * ab.x + 0.21580376 * ab.y,
        -0.105561346 * ab.x - 0.06385417 * ab.y,
        -0.08948418 * ab.x - 1.2914855 * ab.y,
    );
}

fn oklab_linear(lightness: f32, chroma: f32, ab: vec2<f32>) -> vec3<f32> {
    let cone = lightness + chroma * cone_weights(ab);
    let lms = cone * cone * cone;
    return vec3<f32>(
        4.0767417 * lms.x - 3.3077116 * lms.y + 0.23096994 * lms.z,
        -1.268438 * lms.x + 2.6097574 * lms.y - 0.34131938 * lms.z,
        -0.0041960863 * lms.x - 0.7034186 * lms.y + 1.7076147 * lms.z,
    );
}

fn in_gamut(rgb: vec3<f32>) -> bool {
    return all(rgb >= vec3<f32>(-SLACK)) && all(rgb <= vec3<f32>(1.0 + SLACK));
}

fn max_chroma(lightness: f32, ab: vec2<f32>) -> f32 {
    var inside = 0.0;
    var outside = CHROMA_LIMIT;
    for (var step = 0; step < SEARCH_STEPS; step++) {
        let middle = (inside + outside) / 2.0;
        if in_gamut(oklab_linear(lightness, middle, ab)) {
            inside = middle;
        } else {
            outside = middle;
        }
    }
    return inside;
}

fn max_saturation(ab: vec2<f32>) -> f32 {
    let a = ab.x;
    let b = ab.y;
    var fit: array<f32, 5>;
    var row: vec3<f32>;
    if -1.8817033 * a - 0.8093649 * b > 1.0 {
        fit = array<f32, 5>(1.1908628, 1.7657673, 0.5966264, 0.755152, 0.5677124);
        row = vec3<f32>(4.0767417, -3.3077116, 0.23096994);
    } else if 1.814441 * a - 1.1944528 * b > 1.0 {
        fit = array<f32, 5>(0.73956515, -0.45954404, 0.08285427, 0.1254107, 0.14503204);
        row = vec3<f32>(-1.268438, 2.6097574, -0.34131938);
    } else {
        fit = array<f32, 5>(1.3573365, -0.00915799, -1.1513021, -0.50559606, 0.00692167);
        row = vec3<f32>(-0.0041960863, -0.7034186, 1.7076147);
    }
    var saturation = fit[0] + fit[1] * a + fit[2] * b + fit[3] * a * a + fit[4] * a * b;
    let weights = cone_weights(ab);
    for (var step = 0; step < 2; step++) {
        let cone = 1.0 + saturation * weights;
        let value = dot(row, cone * cone * cone);
        let slope = dot(row, 3.0 * weights * cone * cone);
        let bend = dot(row, 6.0 * weights * weights * cone);
        saturation -= value * slope / (slope * slope - 0.5 * value * bend);
    }
    return saturation;
}

fn cusp(ab: vec2<f32>) -> vec2<f32> {
    let saturation = max_saturation(ab);
    let rgb = oklab_linear(1.0, saturation, ab);
    let lightness = pow(1.0 / max(rgb.x, max(rgb.y, rgb.z)), 1.0 / 3.0);
    return vec2<f32>(lightness, lightness * saturation);
}

fn encode(value: f32) -> f32 {
    let clamped = clamp(value, 0.0, 1.0);
    if clamped <= 0.0031308 {
        return clamped * 12.92;
    }
    return 1.055 * pow(clamped, 1.0 / 2.4) - 0.055;
}

fn decode(value: f32) -> f32 {
    if value <= 0.04045 {
        return value / 12.92;
    }
    return pow((value + 0.055) / 1.055, 2.4);
}

fn gamma(linear: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(encode(linear.x), encode(linear.y), encode(linear.z));
}

fn output(rgb: vec3<f32>, alpha: f32) -> vec4<f32> {
    if wheel.srgb > 0.5 {
        return vec4<f32>(decode(rgb.x), decode(rgb.y), decode(rgb.z), alpha);
    }
    return vec4<f32>(rgb, alpha);
}

fn coverage(depth: f32) -> f32 {
    return clamp(depth + 0.5, 0.0, 1.0);
}

fn ring_color(hue: f32) -> vec3<f32> {
    if wheel.model < 0.5 {
        return hsv(hue, 1.0, 1.0);
    }
    let angle = radians(hue);
    let ab = vec2<f32>(cos(angle), sin(angle));
    let tip = cusp(ab);
    return gamma(oklab_linear(tip.x, min(TIP_CHROMA, tip.y), ab));
}

struct Corners {
    tip: vec2<f32>,
    white: vec2<f32>,
    black: vec2<f32>,
};

fn corners(centre: vec2<f32>, inner: f32) -> Corners {
    return Corners(
        centre + direction(wheel.hue) * inner,
        centre + direction(wheel.hue + 120.0) * inner,
        centre + direction(wheel.hue + 240.0) * inner,
    );
}

fn inside_triangle(triangle: Corners, pos: vec2<f32>) -> f32 {
    let edges = array<vec2<f32>, 3>(triangle.tip, triangle.white, triangle.black);
    var nearest = 1e9;
    for (var index = 0; index < 3; index++) {
        let start = edges[index];
        let edge = edges[(index + 1) % 3] - start;
        let length = max(length(edge), DEGENERATE);
        nearest = min(nearest, (edge.y * (pos.x - start.x) - edge.x * (pos.y - start.y)) / length);
    }
    return nearest;
}

fn triangle_at(triangle: Corners, pos: vec2<f32>) -> vec2<f32> {
    let to_tip = triangle.tip - triangle.black;
    let to_white = triangle.white - triangle.black;
    let to_pos = pos - triangle.black;
    let area = to_tip.x * to_white.y - to_tip.y * to_white.x;
    if abs(area) < DEGENERATE {
        return vec2<f32>(0.0);
    }
    let tip_weight = (to_pos.x * to_white.y - to_pos.y * to_white.x) / area;
    let white_weight = (to_tip.x * to_pos.y - to_tip.y * to_pos.x) / area;
    let lit = tip_weight + white_weight;
    if abs(lit) < DEGENERATE {
        return vec2<f32>(0.0);
    }
    return vec2<f32>(clamp(tip_weight / lit, 0.0, 1.0), clamp(lit, 0.0, 1.0));
}

@fragment
fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    if position.x < wheel.clip.x || position.y < wheel.clip.y
        || position.x > wheel.clip.z || position.y > wheel.clip.w {
        discard;
    }
    let pos = position.xy;
    let centre = (wheel.rect.xy + wheel.rect.zw) / 2.0;
    let outer = min(wheel.rect.z - wheel.rect.x, wheel.rect.w - wheel.rect.y) / 2.0;
    let inner = max(outer - wheel.ring_width, 0.0);
    let offset = pos - centre;
    let distance = length(offset);

    let point = triangle_at(corners(centre, inner), pos);
    var rgb: vec3<f32>;
    var excess = 0.0;
    if wheel.model < 0.5 {
        rgb = hsv(wheel.hue, point.x, point.y);
    } else {
        let angle = radians(wheel.hue);
        let ab = vec2<f32>(cos(angle), sin(angle));
        let tip = point.y * point.x;
        let lightness = clamp(point.y - tip * (1.0 - cusp(ab).x), 0.0, 1.0);
        let chroma = tip * TIP_CHROMA;
        let limit = max_chroma(lightness, ab);
        excess = chroma - limit;
        rgb = gamma(oklab_linear(lightness, min(chroma, limit), ab));
    }
    let slope = length(vec2<f32>(dpdx(excess), dpdy(excess)));
    let triangle = coverage(inside_triangle(corners(centre, inner), pos));
    if wheel.model > 0.5 && slope > 1e-7 {
        let strength = coverage(wheel.gamut_line / 2.0 - abs(excess / slope)) * GAMUT_LINE_OPACITY;
        let luminance = dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
        let ink = select(1.0, 0.0, luminance > 140.0 / 255.0);
        rgb = mix(rgb, vec3<f32>(ink), strength);
    }

    let ring = coverage(min(distance - (inner + wheel.ring_gap), outer - distance));
    if ring > 0.0 {
        let hue = (degrees(atan2(-offset.y, offset.x)) + 360.0) % 360.0;
        return output(ring_color(hue), ring);
    }
    if triangle <= 0.0 {
        discard;
    }
    return output(rgb, triangle);
}

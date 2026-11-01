struct Placed {
    rect: vec4<f32>,
    uv: vec4<f32>,
    clip: vec4<f32>,
    screen: vec2<f32>,
    turn: vec2<f32>,
    opacity: f32,
    opaque: f32,
    decode_srgb: f32,
    corner_radius: f32,
};

@group(0) @binding(0)
var quad_texture: texture_2d<f32>;
@group(0) @binding(1)
var quad_sampler: sampler;
@group(1) @binding(0)
var<uniform> placed: Placed;

struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Output {
    let corner = vec2<f32>(f32(index & 1u), f32(index >> 1u));
    let center = (placed.rect.xy + placed.rect.zw) * 0.5;
    let offset = (corner * 2.0 - vec2<f32>(1.0)) * (placed.rect.zw - placed.rect.xy) * 0.5;
    let point = center + vec2<f32>(
        offset.x * placed.turn.x - offset.y * placed.turn.y,
        offset.x * placed.turn.y + offset.y * placed.turn.x,
    );
    var output: Output;
    output.position = vec4<f32>(
        point.x / placed.screen.x * 2.0 - 1.0,
        1.0 - point.y / placed.screen.y * 2.0,
        0.0,
        1.0,
    );
    output.uv = mix(placed.uv.xy, placed.uv.zw, corner);
    return output;
}

fn to_linear(gamma: vec3<f32>) -> vec3<f32> {
    let low = gamma / 12.92;
    let high = pow((gamma + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, gamma <= vec3<f32>(0.04045));
}

fn covered(uv: vec2<f32>) -> f32 {
    let size = abs((placed.rect.zw - placed.rect.xy) / (placed.uv.zw - placed.uv.xy));
    let half = size * 0.5;
    let corner = min(placed.corner_radius, min(half.x, half.y));
    let inside = abs(uv * size - half) - (half - vec2<f32>(corner));
    let distance = length(max(inside, vec2<f32>(0.0))) + min(max(inside.x, inside.y), 0.0) - corner;
    let pixel = max(fwidth(distance), 0.0001);
    return select(1.0, clamp(0.5 - distance / pixel, 0.0, 1.0), corner > 0.0);
}

@fragment
fn fragment(input: Output) -> @location(0) vec4<f32> {
    let coverage = covered(input.uv);
    let at = input.position.xy;
    if any(at < placed.clip.xy) || any(at >= placed.clip.zw) {
        discard;
    }
    let color = textureSampleLevel(quad_texture, quad_sampler, input.uv, 0.0);
    var rgb = color.rgb;
    var alpha = color.a;
    if placed.decode_srgb > 0.5 && alpha > 0.0 {
        rgb = to_linear(rgb / alpha) * alpha;
    }
    if placed.opaque > 0.5 {
        alpha = 1.0;
    }
    return vec4<f32>(rgb, alpha) * placed.opacity * coverage;
}

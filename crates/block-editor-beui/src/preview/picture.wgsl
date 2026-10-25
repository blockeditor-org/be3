struct Picture {
    rect: vec4<f32>,
    screen: vec2<f32>,
    turn: vec2<f32>,
    opacity: f32,
    srgb: f32,
    padding: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> picture: Picture;
@group(0) @binding(1)
var picture_texture: texture_2d<f32>;
@group(0) @binding(2)
var picture_sampler: sampler;

struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Output {
    let uv = vec2<f32>(f32(index & 1u), f32(index >> 1u));
    let center = (picture.rect.xy + picture.rect.zw) * 0.5;
    let half = (picture.rect.zw - picture.rect.xy) * 0.5;
    let offset = (uv * 2.0 - vec2<f32>(1.0)) * half;
    let point = center + vec2<f32>(
        offset.x * picture.turn.x - offset.y * picture.turn.y,
        offset.x * picture.turn.y + offset.y * picture.turn.x,
    );
    var output: Output;
    output.position = vec4<f32>(
        point.x / picture.screen.x * 2.0 - 1.0,
        1.0 - point.y / picture.screen.y * 2.0,
        0.0,
        1.0,
    );
    output.uv = uv;
    return output;
}

fn to_linear(gamma: vec3<f32>) -> vec3<f32> {
    let low = gamma / 12.92;
    let high = pow((gamma + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, gamma <= vec3<f32>(0.04045));
}

@fragment
fn fragment(input: Output) -> @location(0) vec4<f32> {
    let color = textureSampleLevel(picture_texture, picture_sampler, input.uv, 0.0);
    var rgb = color.rgb;
    if picture.srgb > 0.5 && color.a > 0.0 {
        rgb = to_linear(rgb / color.a) * color.a;
    }
    return vec4<f32>(rgb, color.a) * picture.opacity;
}
